//! Submit command - Submit entities for review

use clap::Args;
use miette::{bail, IntoDiagnostic, Result};
use std::io::{self, BufRead};
use std::path::PathBuf;

use crate::cli::args::GlobalOpts;
use crate::core::entity::Status;
use crate::core::identity::EntityPrefix;
use crate::core::shortid::ShortIdIndex;
use crate::core::workflow::{
    get_entity_info, get_prefix_from_id, truncate_id, update_entity_status,
};
use crate::core::{Config, Git, Project, Provider, ProviderClient, TeamRoster};

/// Submit entities for review (creates PR if provider configured)
#[derive(Debug, Args)]
pub struct SubmitArgs {
    /// Entity IDs to submit (accepts multiple, or - for stdin)
    pub ids: Vec<String>,

    /// Submit message
    #[arg(long, short = 'm')]
    pub message: Option<String>,

    /// Submit all draft entities
    #[arg(long)]
    pub all: bool,

    /// Filter by entity type (req, risk, cmp, etc.)
    #[arg(long, short = 't')]
    pub entity_type: Option<String>,

    /// Filter by status (default: draft)
    #[arg(long, short = 's', default_value = "draft")]
    pub status: String,

    /// Skip PR creation (commit only)
    #[arg(long)]
    pub no_pr: bool,

    /// Create as draft PR
    #[arg(long)]
    pub draft: bool,

    /// Skip confirmation prompt
    #[arg(long, short = 'y')]
    pub yes: bool,

    /// Show what would be done without making changes
    #[arg(long)]
    pub dry_run: bool,

    /// Print commands as they run
    #[arg(long, short = 'v')]
    pub verbose: bool,
}

impl SubmitArgs {
    pub fn run(&self, _global: &GlobalOpts) -> Result<()> {
        let project = Project::discover().into_diagnostic()?;
        let config = Config::load();

        // Check if workflow is enabled
        if !config.workflow.enabled {
            bail!(
                "Workflow features are not enabled.\n\
                 Add the following to .tdt/config.yaml:\n\n\
                 workflow:\n\
                 \x20 enabled: true\n\
                 \x20 provider: github  # or gitlab, or none"
            );
        }

        let git = Git::new(project.root());

        // Verify we're in a git repo
        if !git.is_repo() {
            bail!("Not a git repository. Initialize with 'git init' first.");
        }

        // Check for uncommitted changes (warn only)
        if !git.is_clean() && self.verbose {
            eprintln!("Warning: Working directory has uncommitted changes");
        }

        // Collect entity IDs
        let ids = self.collect_entity_ids(&project)?;
        if ids.is_empty() {
            bail!("No entities to submit. Specify IDs or use --all");
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

            if status != Status::Draft {
                bail!(
                    "Entity {} is not in draft status (current: {})",
                    entity_id,
                    status
                );
            }

            entities.push((file_path, entity_id, title, status));
        }

        // Show what we're about to do
        println!("Submitting {} entities for review...", entities.len());
        if self.verbose || self.dry_run {
            for (_, id, title, _) in &entities {
                println!("  {}  {}", truncate_id(id), title);
            }
        }

        if self.dry_run {
            self.print_dry_run(&entities, &config, &git)?;
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

        // Execute the workflow
        self.execute_submit(&project, &config, &git, &entities)?;

        Ok(())
    }

    fn collect_entity_ids(&self, project: &Project) -> Result<Vec<String>> {
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

        // If --all or --entity_type, scan project
        if self.all || self.entity_type.is_some() {
            return self.scan_project_for_entities(project);
        }

        // Otherwise use provided IDs
        Ok(self.ids.clone())
    }

    fn scan_project_for_entities(&self, project: &Project) -> Result<Vec<String>> {
        use walkdir::WalkDir;

        let target_status: Status = self.status.parse().unwrap_or(Status::Draft);
        let target_prefix: Option<EntityPrefix> = self
            .entity_type
            .as_ref()
            .and_then(|t| t.to_uppercase().parse().ok());

        let mut ids = Vec::new();

        for entry in WalkDir::new(project.root())
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .map(|ext| ext == "yaml")
                    .unwrap_or(false)
            })
            .filter(|e| e.path().to_string_lossy().contains(".tdt.yaml"))
        {
            if let Ok((id, _, status)) = get_entity_info(entry.path()) {
                if status != target_status {
                    continue;
                }

                if let Some(ref prefix_filter) = target_prefix {
                    if let Some(prefix) = get_prefix_from_id(&id) {
                        if prefix != *prefix_filter {
                            continue;
                        }
                    } else {
                        continue;
                    }
                }

                ids.push(id);
            }
        }

        Ok(ids)
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
        _git: &Git,
    ) -> Result<()> {
        println!("\nWould execute:");

        let is_single = entities.len() == 1;
        let (branch_name, commit_message, pr_title) = if is_single {
            let (_, id, title, _) = &entities[0];
            let short = truncate_id(id);
            let prefix = id.split('-').next().unwrap_or("ENT");
            let ulid_part = id.split('-').nth(1).unwrap_or("");
            let short_ulid = if ulid_part.len() > 8 {
                &ulid_part[..8]
            } else {
                ulid_part
            };
            (
                config.workflow.format_branch(prefix, short_ulid),
                config.workflow.format_submit_message(&short, title),
                format!("Submit {}: {}", short, title),
            )
        } else {
            let date = chrono::Utc::now().format("%Y-%m-%d");
            (
                format!("review/batch-{}", date),
                format!("Submit {} entities for review", entities.len()),
                format!("Submit {} entities for review", entities.len()),
            )
        };

        println!("  git checkout -b {}", branch_name);

        for (path, _, _, _) in entities {
            let rel_path = path
                .strip_prefix(std::env::current_dir().into_diagnostic()?)
                .unwrap_or(path)
                .display();
            println!("  [update status: draft → review in {}]", rel_path);
            println!("  git add {}", rel_path);
        }

        println!("  git commit -m \"{}\"", commit_message);
        println!("  git push -u origin {}", branch_name);

        if !self.no_pr && config.workflow.provider != Provider::None {
            let provider = ProviderClient::new(config.workflow.provider, std::path::Path::new("."));
            let pr_cmd = if self.draft {
                provider.format_command(&[
                    "pr",
                    "create",
                    "--title",
                    &pr_title,
                    "--base",
                    &config.workflow.base_branch,
                    "--draft",
                ])
            } else {
                provider.format_command(&[
                    "pr",
                    "create",
                    "--title",
                    &pr_title,
                    "--base",
                    &config.workflow.base_branch,
                ])
            };
            println!("  {}", pr_cmd);
        }

        Ok(())
    }

    fn execute_submit(
        &self,
        project: &Project,
        config: &Config,
        git: &Git,
        entities: &[(PathBuf, String, String, Status)],
    ) -> Result<()> {
        let is_single = entities.len() == 1;

        // Determine branch name and messages
        let (branch_name, commit_message, pr_title, pr_body) = if is_single {
            let (_, id, title, _) = &entities[0];
            let short = truncate_id(id);
            let prefix = id.split('-').next().unwrap_or("ENT");
            let ulid_part = id.split('-').nth(1).unwrap_or("");
            let short_ulid = if ulid_part.len() > 8 {
                &ulid_part[..8]
            } else {
                ulid_part
            };
            (
                config.workflow.format_branch(prefix, short_ulid),
                config.workflow.format_submit_message(&short, title),
                format!("Submit {}: {}", short, title),
                format!("Submitting {} for review.\n\n**Title:** {}", short, title),
            )
        } else {
            let date = chrono::Utc::now().format("%Y-%m-%d");
            let entity_list: String = entities
                .iter()
                .map(|(_, id, title, _)| format!("- {}: {}", truncate_id(id), title))
                .collect::<Vec<_>>()
                .join("\n");
            (
                format!("review/batch-{}", date),
                format!("Submit {} entities for review", entities.len()),
                format!("Submit {} entities for review", entities.len()),
                format!(
                    "Submitting {} entities for review.\n\n**Entities:**\n{}",
                    entities.len(),
                    entity_list
                ),
            )
        };

        // Create feature branch
        if self.verbose {
            eprintln!("  Creating branch: {}", branch_name);
        }
        git.create_and_checkout_branch(&branch_name)
            .into_diagnostic()?;
        println!("  Created branch: {}", branch_name);

        // Update status in each entity
        for (path, id, _, _) in entities {
            update_entity_status(path, Status::Review).into_diagnostic()?;
            if self.verbose {
                eprintln!("  Updated status: draft → review in {}", truncate_id(id));
            }
        }
        println!(
            "  Changed status: draft → review ({} entities)",
            entities.len()
        );

        // Stage files
        let paths: Vec<&std::path::Path> =
            entities.iter().map(|(p, _, _, _)| p.as_path()).collect();
        git.stage_files(&paths).into_diagnostic()?;

        // Commit
        let _hash = git.commit(&commit_message).into_diagnostic()?;
        println!("  Committed: \"{}\"", commit_message);

        // Push
        git.push(&branch_name, true).into_diagnostic()?;
        println!("  Pushed to origin");

        // Create PR if provider is configured
        if !self.no_pr && config.workflow.provider != Provider::None {
            let provider = ProviderClient::new(config.workflow.provider, project.root())
                .with_verbose(self.verbose);

            match provider.create_pr(
                &pr_title,
                &pr_body,
                &config.workflow.base_branch,
                self.draft,
            ) {
                Ok(pr_info) => {
                    println!("  Created PR #{}: {}", pr_info.number, pr_info.url);
                }
                Err(e) => {
                    eprintln!("  Warning: Failed to create PR: {}", e);
                    eprintln!("  You can create it manually at your git provider.");
                }
            }
        } else if config.workflow.provider == Provider::None {
            println!("\n  Create PR manually at your git provider.");
        }

        // Load team roster and show who can approve
        if let Some(roster) = TeamRoster::load(project) {
            if let Some((_, id, _, _)) = entities.first() {
                if let Some(prefix) = get_prefix_from_id(id) {
                    if let Some(roles) = roster.required_roles(prefix) {
                        let role_names: Vec<_> = roles.iter().map(|r| r.to_string()).collect();
                        println!("\nApproval requires: {} role", role_names.join(" or "));
                    }
                }
            }
        }

        println!("\n{} entities submitted for review.", entities.len());

        Ok(())
    }
}
