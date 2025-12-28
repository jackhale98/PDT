//! Release command - Release approved entities

use clap::Args;
use miette::{bail, IntoDiagnostic, Result};
use std::io::{self, BufRead};
use std::path::PathBuf;

use crate::cli::args::GlobalOpts;
use crate::core::entity::Status;
use crate::core::identity::EntityPrefix;
use crate::core::shortid::ShortIdIndex;
use crate::core::workflow::{get_entity_info, get_prefix_from_id, record_release, truncate_id};
use crate::core::{Config, Git, Project, TeamRoster, WorkflowEngine};

/// Release approved entities
#[derive(Debug, Args)]
pub struct ReleaseArgs {
    /// Entity IDs to release (accepts multiple, or - for stdin)
    pub ids: Vec<String>,

    /// Release all approved entities of a type
    #[arg(long, short = 't')]
    pub entity_type: Option<String>,

    /// Release all approved entities
    #[arg(long)]
    pub all: bool,

    /// Release message
    #[arg(long, short = 'm')]
    pub message: Option<String>,

    /// Skip authorization check
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

    /// GPG-sign the release commit and tag (for 21 CFR Part 11 compliance)
    #[arg(long, short = 'S')]
    pub sign: bool,
}

impl ReleaseArgs {
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
            bail!("Not a git repository.");
        }

        // Load team roster (optional)
        let roster = TeamRoster::load(&project);
        let engine = WorkflowEngine::new(roster.clone(), config.workflow.clone());

        // Get current user and check release authorization
        let current_user = engine.current_user();
        let releaser_name = current_user
            .map(|u| u.name.clone())
            .or_else(|| git.user_name().ok())
            .unwrap_or_else(|| "Unknown".to_string());

        // Check release authorization if roster exists and not forcing
        if !self.force {
            if let Some(ref r) = roster {
                if let Some(user) = current_user {
                    if !r.can_release(user) {
                        bail!(
                            "You ({}) do not have release authorization.\n\
                             Release requires: management role",
                            user.name
                        );
                    }
                } else {
                    bail!(
                        "You are not in the team roster. Add yourself with 'tdt team add' or use --force"
                    );
                }
            }
        }

        // Collect entity IDs
        let ids = self.collect_entity_ids(&project)?;
        if ids.is_empty() {
            bail!("No entities to release. Specify IDs or use --all");
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

            if status != Status::Approved {
                bail!(
                    "Entity {} is not in approved status (current: {})",
                    entity_id,
                    status
                );
            }

            entities.push((file_path, entity_id, title, status));
        }

        // Show what we're about to do
        println!(
            "Releasing {} entities as {}...",
            entities.len(),
            releaser_name
        );
        if self.verbose || self.dry_run {
            for (_, id, title, _) in &entities {
                println!("  {}  {}", truncate_id(id), title);
            }
        }

        if self.dry_run {
            self.print_dry_run(&entities)?;
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

        // Execute the release
        self.execute_release(&git, &entities, &releaser_name)?;

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
                if status != Status::Approved {
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

    fn print_dry_run(&self, entities: &[(PathBuf, String, String, Status)]) -> Result<()> {
        println!("\nWould execute:");

        for (path, _id, _, _) in entities {
            let rel_path = path
                .strip_prefix(std::env::current_dir().into_diagnostic()?)
                .unwrap_or(path)
                .display();
            println!("  [record release in {}]", rel_path);
            println!("  git add {}", rel_path);
        }

        let commit_message = if entities.len() == 1 {
            let (_, id, title, _) = &entities[0];
            format!("Release {}: {}", truncate_id(id), title)
        } else {
            format!("Release {} entities", entities.len())
        };
        println!("  git commit -m \"{}\"", commit_message);

        Ok(())
    }

    fn execute_release(
        &self,
        git: &Git,
        entities: &[(PathBuf, String, String, Status)],
        releaser_name: &str,
    ) -> Result<()> {
        // Record release in each entity
        for (path, id, _, _) in entities {
            record_release(path, releaser_name).into_diagnostic()?;
            if self.verbose {
                eprintln!("  Recorded release in {}", truncate_id(id));
            }
        }
        println!(
            "  Released {} entities (status: approved → released)",
            entities.len()
        );

        // Stage files
        let paths: Vec<&std::path::Path> =
            entities.iter().map(|(p, _, _, _)| p.as_path()).collect();
        git.stage_files(&paths).into_diagnostic()?;

        // Check GPG is configured if --sign is requested
        if self.sign && !git.signing_configured() {
            bail!(
                "GPG signing requested but not configured.\n\
                 Configure with: git config --global user.signingkey <KEY_ID>\n\n\
                 For setup instructions, see: https://docs.github.com/en/authentication/managing-commit-signature-verification"
            );
        }

        // Commit (with or without GPG signature)
        let commit_message = if entities.len() == 1 {
            let (_, id, title, _) = &entities[0];
            format!("Release {}: {}", truncate_id(id), title)
        } else {
            format!("Release {} entities", entities.len())
        };

        if self.sign {
            git.commit_signed(&commit_message).into_diagnostic()?;
            println!("  Committed (GPG signed): \"{}\"", commit_message);
        } else {
            git.commit(&commit_message).into_diagnostic()?;
            println!("  Committed: \"{}\"", commit_message);
        }

        // Create git tags for released entities (for audit trail)
        // Use signed tags when --sign is used for full compliance
        let date = chrono::Utc::now().format("%Y-%m-%d");
        for (_, id, title, _) in entities {
            let short_id = truncate_id(id);
            // Sanitize releaser name for tag (replace spaces with underscores)
            let safe_releaser = releaser_name.replace(' ', "_").replace('@', "_at_");
            let tag_name = format!("release/{}/{}/{}", short_id, safe_releaser, date);

            if !git.tag_exists(&tag_name) {
                let tag_message = format!(
                    "Released by {}: {}",
                    releaser_name,
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

        println!("\n{} entities released.", entities.len());

        Ok(())
    }
}
