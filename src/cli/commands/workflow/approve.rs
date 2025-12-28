//! Approve command - Approve entities under review
//!
//! Supports multi-signature approvals with configurable requirements per entity type.

use clap::Args;
use miette::{bail, IntoDiagnostic, Result};
use std::io::{self, BufRead};
use std::path::PathBuf;

use crate::cli::args::GlobalOpts;
use crate::core::entity::Status;
use crate::core::shortid::ShortIdIndex;
use crate::core::workflow::{
    get_approval_status, get_entity_info, get_prefix_from_id, record_approval_ext, truncate_id,
    would_be_duplicate_approval, ApprovalOptions,
};
use crate::core::{Config, Git, Project, Provider, ProviderClient, TeamRoster, WorkflowEngine};

/// Approve entities under review
#[derive(Debug, Args)]
pub struct ApproveArgs {
    /// Entity IDs to approve (accepts multiple, or - for stdin)
    #[arg(required_unless_present = "pr")]
    pub ids: Vec<String>,

    /// Approve all entities in a PR by PR number
    #[arg(long)]
    pub pr: Option<u64>,

    /// Approval comment/message
    #[arg(long, short = 'm')]
    pub message: Option<String>,

    /// Merge PR after approval (requires git)
    #[arg(long)]
    pub merge: bool,

    /// Skip merge even if auto_merge enabled
    #[arg(long)]
    pub no_merge: bool,

    /// Skip authorization check (admin only)
    #[arg(long)]
    pub force: bool,

    /// Skip confirmation prompt
    #[arg(long, short = 'y')]
    pub yes: bool,

    /// Show what would be done without making changes
    #[arg(long)]
    pub dry_run: bool,

    /// Print commands as they run
    #[arg(long, short = 'v')]
    pub verbose: bool,

    /// Skip git commit (just update YAML files)
    #[arg(long)]
    pub no_commit: bool,

    /// GPG-sign the approval commit (for 21 CFR Part 11 compliance)
    #[arg(long, short = 'S')]
    pub sign: bool,

    /// Show approval status without adding an approval
    #[arg(long)]
    pub status: bool,
}

impl ApproveArgs {
    pub fn run(&self, _global: &GlobalOpts) -> Result<()> {
        let project = Project::discover().into_diagnostic()?;
        let config = Config::load();

        // Check if workflow is enabled
        if !config.workflow.enabled {
            bail!(
                "Workflow features are not enabled.\n\
                 Add the following to .tdt/config.yaml:\n\n\
                 workflow:\n\
                 \x20 enabled: true"
            );
        }

        let git = Git::new(project.root());
        let has_git = git.is_repo();

        // Track if we changed branches and need to restore
        let original_branch = if has_git {
            git.current_branch().ok()
        } else {
            None
        };
        let mut switched_branch = false;
        let mut stashed_changes = false;
        let mut pr_branch: Option<String> = None;

        // If --pr is specified, checkout the PR branch first
        if let Some(pr_number) = self.pr {
            if !has_git {
                bail!("--pr requires a git repository");
            }

            if config.workflow.provider == Provider::None {
                bail!("--pr requires a git provider (github or gitlab) to be configured");
            }

            let provider = ProviderClient::new(config.workflow.provider, project.root())
                .with_verbose(self.verbose);

            // Get PR info to find the branch
            let pr_info = provider
                .get_pr(pr_number)
                .map_err(|e| miette::miette!("Failed to get PR #{}: {}", pr_number, e))?;

            pr_branch = Some(pr_info.branch.clone());

            if self.verbose {
                eprintln!("PR #{} is on branch: {}", pr_number, pr_info.branch);
            }

            // Check if we're already on the PR branch
            let current = git.current_branch().unwrap_or_default();
            if current != pr_info.branch {
                // Check for uncommitted changes
                if !git.is_clean() {
                    let uncommitted = git.uncommitted_files().unwrap_or_default();
                    if self.yes {
                        // Stash changes automatically
                        println!("Stashing {} uncommitted changes...", uncommitted.len());
                        git.stash(Some("tdt approve: auto-stash before PR checkout"))
                            .into_diagnostic()?;
                        stashed_changes = true;
                    } else {
                        bail!(
                            "You have uncommitted changes:\n  {}\n\n\
                             Either commit/stash them first, or use --yes to auto-stash.",
                            uncommitted.join("\n  ")
                        );
                    }
                }

                // Checkout the PR branch (fetch if needed)
                if self.dry_run {
                    println!("Would execute:");
                    println!("  git fetch");
                    println!("  git checkout {}", pr_info.branch);
                } else {
                    println!("Checking out PR branch: {}", pr_info.branch);
                    git.fetch_and_checkout_branch(&pr_info.branch)
                        .map_err(|e| miette::miette!("Failed to checkout branch: {}", e))?;
                    switched_branch = true;

                    // Pull latest changes
                    if self.verbose {
                        eprintln!("Pulling latest changes...");
                    }
                    if let Err(e) = git.pull_rebase() {
                        eprintln!("Warning: Failed to pull latest changes: {}", e);
                    }
                }
            }
        }

        // Load team roster (optional but recommended for role-based approvals)
        let roster = TeamRoster::load(&project);
        let engine = WorkflowEngine::new(roster.clone(), config.workflow.clone());

        // Get current user - from roster, then git config, then fallback
        let current_user = engine.current_user();
        let approver_name = current_user
            .map(|u| u.name.clone())
            .or_else(|| if has_git { git.user_name().ok() } else { None })
            .or_else(|| std::env::var("USER").ok())
            .or_else(|| std::env::var("USERNAME").ok())
            .unwrap_or_else(|| "Unknown".to_string());

        let approver_email = current_user.map(|u| u.email.clone()).or_else(|| {
            if has_git {
                git.user_email().ok()
            } else {
                None
            }
        });

        let approver_role = current_user.and_then(|u| u.roles.first().copied());

        if self.verbose {
            if let Some(user) = current_user {
                eprintln!("Approving as {} ({:?})", user.name, user.roles);
            }
        }

        // Collect entity IDs
        let ids = self.collect_entity_ids(&project, &config)?;
        if ids.is_empty() {
            bail!("No entities to approve. Specify IDs or use --pr");
        }

        // Resolve short IDs to full IDs and validate
        let short_index = ShortIdIndex::load(&project);
        let mut entities: Vec<(PathBuf, String, String, Status)> = Vec::new();

        for id in &ids {
            let full_id = short_index
                .resolve(id)
                .ok_or_else(|| miette::miette!("Cannot resolve ID: {}", id))?;
            let file_path = self.find_entity_file(&project, &full_id)?;
            let (entity_id, title, status) = get_entity_info(&file_path).into_diagnostic()?;

            // For --status, show approval status and continue
            if self.status {
                self.show_approval_status(&file_path, &entity_id, &title, status, &config)?;
                continue;
            }

            if status != Status::Review {
                bail!(
                    "Entity {} is not in review status (current: {})",
                    entity_id,
                    status
                );
            }

            // Check for duplicate approval (if require_unique_approvers is enabled)
            if let Some(prefix) = get_prefix_from_id(&entity_id) {
                let requirements = config.workflow.get_approval_requirements(prefix);
                if requirements.require_unique_approvers {
                    let is_duplicate = would_be_duplicate_approval(&file_path, &approver_name)
                        .into_diagnostic()?;
                    if is_duplicate && !self.force {
                        bail!(
                            "You have already approved {}. Use --force to add a duplicate approval.",
                            entity_id
                        );
                    }
                }
            }

            // Check authorization
            if !self.force {
                if let Some(prefix) = get_prefix_from_id(&entity_id) {
                    if let Err(e) = engine.can_transition(
                        Status::Review,
                        Status::Approved,
                        prefix,
                        current_user,
                    ) {
                        bail!("{}", e);
                    }
                }
            }

            entities.push((file_path, entity_id, title, status));
        }

        // If --status was used, we're done
        if self.status {
            return Ok(());
        }

        // Show what we're about to do
        println!(
            "Approving {} entities as {}...",
            entities.len(),
            approver_name
        );
        if self.verbose || self.dry_run {
            for (path, id, title, _) in &entities {
                println!("  {}  {}", truncate_id(id), title);
                // Show current approval status
                if let Some(prefix) = get_prefix_from_id(id) {
                    let requirements = config.workflow.get_approval_requirements(prefix);
                    if let Ok(status) = get_approval_status(path, requirements) {
                        println!(
                            "      Approvals: {}/{} {}",
                            status.current_approvals,
                            status.required_approvals,
                            if status.requirements_met {
                                "(ready to approve)"
                            } else {
                                "(more approvals needed)"
                            }
                        );
                    }
                }
            }
        }

        if self.dry_run {
            self.print_dry_run(&entities, &config, has_git)?;
            println!("\nNo changes made (dry run).");
            return Ok(());
        }

        // Confirm if not --yes
        if !self.yes {
            print!("Proceed? [y/N] ");
            std::io::Write::flush(&mut std::io::stdout()).into_diagnostic()?;
            let mut input = String::new();
            std::io::stdin().read_line(&mut input).into_diagnostic()?;
            if !input.trim().eq_ignore_ascii_case("y") {
                println!("Aborted.");
                return Ok(());
            }
        }

        // Execute the approval
        let result = self.execute_approve(
            &project,
            &config,
            &git,
            has_git,
            &entities,
            &approver_name,
            approver_email.as_deref(),
            approver_role,
            pr_branch.as_deref(),
        );

        // Cleanup: restore original branch and pop stash if needed
        if switched_branch {
            if let Some(ref orig) = original_branch {
                if self.verbose {
                    eprintln!("Restoring original branch: {}", orig);
                }
                if let Err(e) = git.checkout_branch(orig) {
                    eprintln!("Warning: Failed to restore original branch: {}", e);
                }
            }
        }

        if stashed_changes {
            if self.verbose {
                eprintln!("Restoring stashed changes...");
            }
            if let Err(e) = git.stash_pop() {
                eprintln!("Warning: Failed to restore stashed changes: {}", e);
                eprintln!(
                    "Your changes are still in the stash. Run 'git stash pop' to restore them."
                );
            }
        }

        result
    }

    fn show_approval_status(
        &self,
        file_path: &std::path::Path,
        entity_id: &str,
        title: &str,
        current_status: Status,
        config: &Config,
    ) -> Result<()> {
        println!("{}  {}", truncate_id(entity_id), title);
        println!("  Status: {}", current_status);

        if let Some(prefix) = get_prefix_from_id(entity_id) {
            let requirements = config.workflow.get_approval_requirements(prefix);
            if let Ok(status) = get_approval_status(file_path, requirements) {
                println!(
                    "  Approvals: {}/{}",
                    status.current_approvals, status.required_approvals
                );
                if !status.approvers.is_empty() {
                    println!("  Approvers: {}", status.approvers.join(", "));
                }
                if !status.missing_roles.is_empty() {
                    let roles: Vec<String> =
                        status.missing_roles.iter().map(|r| r.to_string()).collect();
                    println!("  Missing roles: {}", roles.join(", "));
                }
                if status.requirements_met {
                    println!("  Ready for approval transition");
                } else {
                    let remaining = status
                        .required_approvals
                        .saturating_sub(status.current_approvals);
                    if remaining > 0 {
                        println!("  Need {} more approval(s)", remaining);
                    }
                }
            }
        }
        println!();
        Ok(())
    }

    fn collect_entity_ids(&self, project: &Project, config: &Config) -> Result<Vec<String>> {
        // Check for stdin
        if self.ids.len() == 1 && self.ids[0] == "-" {
            let stdin = io::stdin();
            return Ok(stdin
                .lock()
                .lines()
                .map_while(Result::ok)
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect());
        }

        // If --pr is set, fetch PR and extract entity IDs from branch name
        if let Some(pr_number) = self.pr {
            return self.extract_entities_from_pr(pr_number, project, config);
        }

        // Otherwise use provided IDs
        Ok(self.ids.clone())
    }

    /// Extract entity IDs from a PR's branch name or changed files
    fn extract_entities_from_pr(
        &self,
        pr_number: u64,
        project: &Project,
        config: &Config,
    ) -> Result<Vec<String>> {
        if config.workflow.provider == Provider::None {
            bail!(
                "Cannot use --pr without a git provider configured.\n\
                 Set workflow.provider to 'github' or 'gitlab' in .tdt/config.yaml"
            );
        }

        let provider = ProviderClient::new(config.workflow.provider, project.root());

        // Get PR info to access the branch name
        let pr_info = provider
            .get_pr(pr_number)
            .map_err(|e| miette::miette!("Failed to get PR #{}: {}", pr_number, e))?;

        if self.verbose {
            eprintln!(
                "  PR #{}: {} (branch: {})",
                pr_info.number, pr_info.title, pr_info.branch
            );
        }

        // Try to extract entity ID from branch name
        // Branch patterns: "review/{PREFIX}-{short_id}" or "review/batch-{date}"
        let branch = &pr_info.branch;

        if let Some(entity_id) = self.parse_entity_from_branch(branch) {
            if self.verbose {
                eprintln!("  Extracted entity from branch: {}", entity_id);
            }
            return Ok(vec![entity_id]);
        }

        // For batch branches or unknown patterns, fetch changed files from PR
        if self.verbose {
            eprintln!("  Branch doesn't contain entity ID, fetching changed files...");
        }

        self.extract_entities_from_pr_files(pr_number, project, config)
    }

    /// Parse entity ID from branch name like "review/REQ-01KCWY20"
    fn parse_entity_from_branch(&self, branch: &str) -> Option<String> {
        // Strip "review/" prefix if present
        let branch = branch.strip_prefix("review/").unwrap_or(branch);

        // Skip batch branches
        if branch.starts_with("batch-") {
            return None;
        }

        // Try to parse as PREFIX-SHORTID format
        // Valid prefixes: REQ, RISK, TEST, CMP, etc.
        let parts: Vec<&str> = branch.splitn(2, '-').collect();
        if parts.len() == 2 {
            let prefix = parts[0].to_uppercase();
            let valid_prefixes = [
                "REQ", "RISK", "TEST", "RSLT", "CMP", "ASM", "FEAT", "MATE", "TOL", "PROC", "CTRL",
                "WORK", "LOT", "DEV", "NCR", "CAPA", "QUOT", "SUP",
            ];
            if valid_prefixes.contains(&prefix.as_str()) {
                // Return as PREFIX@shortid format for resolution
                return Some(format!("{}@{}", prefix, parts[1]));
            }
        }

        None
    }

    /// Extract entity IDs from files changed in a PR
    fn extract_entities_from_pr_files(
        &self,
        pr_number: u64,
        project: &Project,
        config: &Config,
    ) -> Result<Vec<String>> {
        use std::collections::HashSet;

        // Get list of changed files using gh/glab CLI
        let files = match config.workflow.provider {
            Provider::GitHub => {
                let output = std::process::Command::new("gh")
                    .args(["pr", "diff", &pr_number.to_string(), "--name-only"])
                    .current_dir(project.root())
                    .output()
                    .into_diagnostic()?;

                if !output.status.success() {
                    bail!(
                        "Failed to get PR files: {}",
                        String::from_utf8_lossy(&output.stderr)
                    );
                }

                String::from_utf8_lossy(&output.stdout).to_string()
            }
            Provider::GitLab => {
                let output = std::process::Command::new("glab")
                    .args(["mr", "diff", &pr_number.to_string(), "--name-only"])
                    .current_dir(project.root())
                    .output()
                    .into_diagnostic()?;

                if !output.status.success() {
                    bail!(
                        "Failed to get MR files: {}",
                        String::from_utf8_lossy(&output.stderr)
                    );
                }

                String::from_utf8_lossy(&output.stdout).to_string()
            }
            Provider::None => bail!("No provider configured"),
        };

        // Extract entity IDs from .tdt.yaml filenames
        let mut entity_ids: HashSet<String> = HashSet::new();

        for line in files.lines() {
            let filename = line.trim();
            // Match files like "requirements/REQ-01ABC123.tdt.yaml"
            if filename.ends_with(".tdt.yaml") {
                if let Some(basename) = std::path::Path::new(filename)
                    .file_stem()
                    .and_then(|s| s.to_str())
                {
                    // Remove .tdt suffix to get entity ID
                    if let Some(entity_id) = basename.strip_suffix(".tdt") {
                        entity_ids.insert(entity_id.to_string());
                    }
                }
            }
        }

        if entity_ids.is_empty() {
            bail!(
                "No entity files found in PR #{}.\n\
                 The PR may not contain any .tdt.yaml files.",
                pr_number
            );
        }

        if self.verbose {
            eprintln!("  Found {} entities in PR files", entity_ids.len());
        }

        Ok(entity_ids.into_iter().collect())
    }

    fn find_entity_file(&self, project: &Project, id: &str) -> Result<PathBuf> {
        use walkdir::WalkDir;

        let file_name = format!("{}.tdt.yaml", id);

        for entry in WalkDir::new(project.root())
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.file_name().to_string_lossy() == file_name {
                return Ok(entry.path().to_path_buf());
            }
        }

        bail!("Entity file not found: {}", id)
    }

    fn print_dry_run(
        &self,
        entities: &[(PathBuf, String, String, Status)],
        config: &Config,
        has_git: bool,
    ) -> Result<()> {
        println!("\nWould execute:");

        for (path, _id, _, _) in entities {
            let rel_path = path
                .strip_prefix(std::env::current_dir().into_diagnostic()?)
                .unwrap_or(path)
                .display();
            println!("  [record approval in {}]", rel_path);
        }

        // Git operations only if available and auto_commit enabled
        if has_git && config.workflow.auto_commit && !self.no_commit {
            for (path, _id, _, _) in entities {
                let rel_path = path
                    .strip_prefix(std::env::current_dir().into_diagnostic()?)
                    .unwrap_or(path)
                    .display();
                println!("  git add {}", rel_path);
            }

            let commit_message = if entities.len() == 1 {
                let (_, id, title, _) = &entities[0];
                config
                    .workflow
                    .format_approve_message(&truncate_id(id), title)
            } else {
                format!("Approve {} entities", entities.len())
            };
            println!("  git commit -m \"{}\"", commit_message);

            if config.workflow.provider != Provider::None {
                if let Some(pr) = self.pr {
                    let provider =
                        ProviderClient::new(config.workflow.provider, std::path::Path::new("."));
                    println!(
                        "  {}",
                        provider.format_command(&["pr", "review", &pr.to_string(), "--approve"])
                    );
                    if self.merge || (config.workflow.auto_merge && !self.no_merge) {
                        println!(
                            "  {}",
                            provider.format_command(&["pr", "merge", &pr.to_string()])
                        );
                    }
                }
            }
        }

        Ok(())
    }

    fn execute_approve(
        &self,
        project: &Project,
        config: &Config,
        git: &Git,
        has_git: bool,
        entities: &[(PathBuf, String, String, Status)],
        approver_name: &str,
        approver_email: Option<&str>,
        approver_role: Option<crate::core::team::Role>,
        pr_branch: Option<&str>,
    ) -> Result<()> {
        let mut fully_approved = 0;
        let mut pending_more_approvals = 0;

        // Check if any entity requires GPG signature
        let mut signature_required = false;
        for (_, id, _, _) in entities {
            if let Some(prefix) = get_prefix_from_id(id) {
                let reqs = config.workflow.get_approval_requirements(prefix);
                if reqs.require_signature {
                    signature_required = true;
                    break;
                }
            }
        }

        // If signature is required but --sign not provided, error
        if signature_required && !self.sign {
            bail!(
                "GPG signature required for this entity type.\n\
                 Use --sign (-S) flag to sign the approval.\n\n\
                 To set up GPG signing, see: https://docs.github.com/en/authentication/managing-commit-signature-verification"
            );
        }

        // Check GPG is configured if --sign is requested
        if self.sign && has_git && !git.signing_configured() {
            bail!(
                "GPG signing requested but not configured.\n\
                 Configure with: git config --global user.signingkey <KEY_ID>\n\n\
                 For setup instructions, see: https://docs.github.com/en/authentication/managing-commit-signature-verification"
            );
        }

        // Warn if require_signature is set but commit.gpgsign is not enabled
        // This helps ensure consistent signing across all commits, not just approvals
        if signature_required && has_git && !git.commit_gpgsign_enabled() {
            eprintln!(
                "Warning: GPG signing required but commit.gpgsign is not enabled.\n\
                 For consistent audit trail, consider enabling auto-signing:\n\
                 \n\
                   git config --global commit.gpgsign true\n\
                   git config --global tag.gpgSign true\n\
                 \n\
                 Or run: tdt team setup-signing\n"
            );
        }

        // Record approval in each entity (this is the "electronic signature")
        for (path, id, _, _) in entities {
            let prefix = get_prefix_from_id(id);
            let requirements = prefix
                .map(|p| config.workflow.get_approval_requirements(p))
                .cloned();

            // Build approval options (signature info added after commit)
            let mut options = ApprovalOptions {
                approver: approver_name.to_string(),
                email: approver_email.map(|s| s.to_string()),
                role: approver_role,
                comment: self.message.clone(),
                signature_verified: None,
                signing_key: None,
            };

            // If signing, get the key ID to record
            if self.sign && has_git {
                options.signing_key = git.signing_key();
            }

            record_approval_ext(path, &options, requirements.as_ref()).into_diagnostic()?;

            // Check if this entity is now fully approved
            if let Some(ref reqs) = requirements {
                let status = get_approval_status(path, reqs).into_diagnostic()?;
                if status.requirements_met {
                    fully_approved += 1;
                    if self.verbose {
                        eprintln!(
                            "  {} - fully approved ({}/{} approvals)",
                            truncate_id(id),
                            status.current_approvals,
                            status.required_approvals
                        );
                    }
                } else {
                    pending_more_approvals += 1;
                    if self.verbose {
                        eprintln!(
                            "  {} - approval recorded ({}/{} approvals)",
                            truncate_id(id),
                            status.current_approvals,
                            status.required_approvals
                        );
                    }
                }
            } else {
                fully_approved += 1;
            }
        }

        println!(
            "  Recorded approval by {} for {} entities",
            approver_name,
            entities.len()
        );

        // Git operations are optional - only if we have git and auto_commit is enabled
        let should_commit = has_git && config.workflow.auto_commit && !self.no_commit;

        if should_commit {
            // Stage files
            let paths: Vec<&std::path::Path> =
                entities.iter().map(|(p, _, _, _)| p.as_path()).collect();
            git.stage_files(&paths).into_diagnostic()?;

            // Commit (with or without GPG signature)
            let commit_message = if entities.len() == 1 {
                let (_, id, title, _) = &entities[0];
                config
                    .workflow
                    .format_approve_message(&truncate_id(id), title)
            } else {
                format!("Approve {} entities", entities.len())
            };

            let commit_hash = if self.sign {
                git.commit_signed(&commit_message).into_diagnostic()?
            } else {
                git.commit(&commit_message).into_diagnostic()?
            };

            if self.sign {
                println!("  Committed (GPG signed): \"{}\"", commit_message);

                // Verify the signature and show key info
                if self.verbose {
                    match git.verify_commit_signature(&commit_hash) {
                        Ok(Some(signer)) => eprintln!("  Signature verified: {}", signer),
                        Ok(None) => eprintln!("  Warning: Commit was not signed"),
                        Err(e) => eprintln!("  Warning: Signature verification failed: {}", e),
                    }
                }
            } else {
                println!("  Committed: \"{}\"", commit_message);
            }

            // Create git tags for approved entities (for audit trail)
            // Use signed tags when --sign is used for full compliance
            let date = chrono::Utc::now().format("%Y-%m-%d");
            for (_, id, title, _) in entities.iter() {
                let short_id = truncate_id(id);
                // Sanitize approver name for tag (replace spaces with underscores)
                let safe_approver = approver_name.replace(' ', "_").replace('@', "_at_");
                let tag_name = format!("approve/{}/{}/{}", short_id, safe_approver, date);

                if !git.tag_exists(&tag_name) {
                    let tag_message = format!(
                        "Approved by {}: {}",
                        approver_name,
                        self.message.as_deref().unwrap_or(title)
                    );
                    let result = git.create_tag_with_options(&tag_name, &tag_message, self.sign);
                    match result {
                        Ok(()) => {
                            if self.verbose {
                                if self.sign {
                                    eprintln!("  Created signed tag: {}", tag_name);
                                } else {
                                    eprintln!("  Created tag: {}", tag_name);
                                }
                            }
                        }
                        Err(e) => {
                            if self.verbose {
                                eprintln!("  Warning: Failed to create tag {}: {}", tag_name, e);
                            }
                        }
                    }
                }
            }

            // Push changes to remote (including tags)
            let current_branch = git.current_branch().unwrap_or_default();
            if !current_branch.is_empty() {
                if self.dry_run {
                    println!("Would execute:");
                    println!("  git push origin {}", current_branch);
                    println!("  git push --tags");
                } else {
                    // Push the branch
                    match git.push(&current_branch, false) {
                        Ok(()) => println!("  Pushed to origin/{}", current_branch),
                        Err(e) => {
                            eprintln!("  Warning: Failed to push: {}", e);
                            eprintln!(
                                "  You may need to push manually: git push origin {}",
                                current_branch
                            );
                        }
                    }

                    // Push tags
                    if let Err(e) = git.run_checked(&["push", "--tags"]) {
                        if self.verbose {
                            eprintln!("  Warning: Failed to push tags: {}", e);
                        }
                    }
                }
            }

            // PR operations if provider is configured
            if config.workflow.provider != Provider::None {
                let provider = ProviderClient::new(config.workflow.provider, project.root())
                    .with_verbose(self.verbose);

                // Find PR for current branch (use pr_branch if provided, otherwise current branch)
                let branch_to_check = pr_branch.unwrap_or(&current_branch);
                if let Ok(Some(pr_info)) = provider.get_pr_for_branch(branch_to_check) {
                    // Add approval review
                    if let Err(e) = provider.approve_pr(pr_info.number, self.message.as_deref()) {
                        eprintln!("  Warning: Failed to add PR approval: {}", e);
                    } else {
                        println!("  Added approval to PR #{}", pr_info.number);
                    }

                    // Only merge if all entities are fully approved
                    let should_merge = self.merge || (config.workflow.auto_merge && !self.no_merge);
                    if should_merge {
                        if pending_more_approvals > 0 {
                            println!(
                                "  Skipping merge: {} entities still need more approvals",
                                pending_more_approvals
                            );
                        } else if let Err(e) = provider.merge_pr(pr_info.number, true) {
                            eprintln!("  Warning: Failed to merge PR: {}", e);
                        } else {
                            println!("  Merged PR #{}", pr_info.number);
                        }
                    }
                }
            }
        }

        println!();
        if fully_approved > 0 {
            println!("{} entities fully approved.", fully_approved);
        }
        if pending_more_approvals > 0 {
            println!(
                "{} entities need more approvals before transitioning to 'approved' status.",
                pending_more_approvals
            );
        }

        Ok(())
    }
}
