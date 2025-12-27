//! Review command - View pending reviews

use clap::{Args, Subcommand};
use miette::{IntoDiagnostic, Result};

use crate::cli::args::GlobalOpts;
use crate::core::entity::Status;
use crate::core::identity::EntityPrefix;
use crate::core::workflow::{get_entity_info, get_prefix_from_id, truncate_id};
use crate::core::{Config, Project, Provider, ProviderClient, TeamRoster, WorkflowEngine};

/// Review pending items
#[derive(Debug, Subcommand)]
pub enum ReviewCommands {
    /// List items pending your review
    List(ReviewListArgs),
    /// Show review queue summary
    Summary,
}

/// List items pending review
#[derive(Debug, Args)]
pub struct ReviewListArgs {
    /// Filter by entity type (req, risk, cmp, etc.)
    #[arg(long, short = 't')]
    pub entity_type: Option<String>,

    /// Show all pending reviews (not just yours)
    #[arg(long)]
    pub all: bool,

    /// Output style (table, short-id, json)
    #[arg(long, short = 'o', default_value = "table")]
    pub output: String,

    /// Print commands as they run
    #[arg(long)]
    pub verbose: bool,
}

impl ReviewCommands {
    pub fn run(&self, global: &GlobalOpts) -> Result<()> {
        match self {
            ReviewCommands::List(args) => args.run(global),
            ReviewCommands::Summary => run_summary(global),
        }
    }
}

impl ReviewListArgs {
    pub fn run(&self, _global: &GlobalOpts) -> Result<()> {
        let project = Project::discover().into_diagnostic()?;
        let config = Config::load();

        // Try to get pending reviews from provider first
        if config.workflow.provider != Provider::None && !self.all {
            if let Ok(pr_reviews) = self.get_provider_reviews(&project, &config) {
                if !pr_reviews.is_empty() {
                    self.print_pr_reviews(&pr_reviews)?;
                    return Ok(());
                }
            }
        }

        // Fall back to scanning local entities
        self.scan_local_reviews(&project, &config)?;

        Ok(())
    }

    fn get_provider_reviews(
        &self,
        project: &Project,
        config: &Config,
    ) -> Result<Vec<PrReviewItem>> {
        let provider = ProviderClient::new(config.workflow.provider, project.root())
            .with_verbose(self.verbose);

        let pending = provider.pending_reviews().into_diagnostic()?;
        let mut items = Vec::new();

        for pr in pending {
            // Extract entity ID from branch name (review/PREFIX-SHORTID)
            if let Some(entity_info) = self.extract_entity_from_branch(&pr.branch) {
                items.push(PrReviewItem {
                    short_id: entity_info.0,
                    entity_type: entity_info.1,
                    title: pr.title.clone(),
                    author: pr.author.clone(),
                    pr_number: pr.number,
                    pr_url: pr.url.clone(),
                });
            } else {
                // Batch PR or couldn't parse - show PR info
                items.push(PrReviewItem {
                    short_id: format!("PR#{}", pr.number),
                    entity_type: "BATCH".to_string(),
                    title: pr.title.clone(),
                    author: pr.author.clone(),
                    pr_number: pr.number,
                    pr_url: pr.url.clone(),
                });
            }
        }

        Ok(items)
    }

    fn extract_entity_from_branch(&self, branch: &str) -> Option<(String, String)> {
        // Branch format: review/PREFIX-SHORTID
        if !branch.starts_with("review/") {
            return None;
        }

        let entity_part = &branch[7..]; // Skip "review/"
        let parts: Vec<&str> = entity_part.splitn(2, '-').collect();
        if parts.len() == 2 {
            Some((entity_part.to_string(), parts[0].to_string()))
        } else {
            None
        }
    }

    fn print_pr_reviews(&self, items: &[PrReviewItem]) -> Result<()> {
        match self.output.as_str() {
            "short-id" => {
                for item in items {
                    println!("{}", item.short_id);
                }
            }
            "json" => {
                let json = serde_json::to_string_pretty(items).into_diagnostic()?;
                println!("{}", json);
            }
            _ => {
                // Table format
                println!("\nPending reviews:\n");
                println!(
                    "{:<12} {:<8} {:<40} {:<15} PR",
                    "SHORT", "TYPE", "TITLE", "AUTHOR"
                );
                println!("{}", "-".repeat(90));

                for item in items {
                    let title = if item.title.len() > 38 {
                        format!("{}...", &item.title[..35])
                    } else {
                        item.title.clone()
                    };
                    println!(
                        "{:<12} {:<8} {:<40} {:<15} #{}",
                        item.short_id, item.entity_type, title, item.author, item.pr_number
                    );
                }

                println!(
                    "\n{} items pending your review. Run `tdt approve <id>` to approve.",
                    items.len()
                );
            }
        }

        Ok(())
    }

    fn scan_local_reviews(&self, project: &Project, config: &Config) -> Result<()> {
        use walkdir::WalkDir;

        let target_prefix: Option<EntityPrefix> = self
            .entity_type
            .as_ref()
            .and_then(|t| t.to_uppercase().parse().ok());

        // Load roster to check what current user can approve
        let roster = TeamRoster::load(project);
        let engine = WorkflowEngine::new(roster.clone(), config.workflow.clone());
        let current_user = engine.current_user();

        let mut items: Vec<LocalReviewItem> = Vec::new();

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
            if let Ok((id, title, status)) = get_entity_info(entry.path()) {
                if status != Status::Review {
                    continue;
                }

                let prefix = get_prefix_from_id(&id);

                // Filter by entity type if specified
                if let Some(ref prefix_filter) = target_prefix {
                    if let Some(ref p) = prefix {
                        if p != prefix_filter {
                            continue;
                        }
                    } else {
                        continue;
                    }
                }

                // If not --all, filter to what user can approve
                if !self.all {
                    if let (Some(ref r), Some(ref p), Some(user)) = (&roster, &prefix, current_user)
                    {
                        if !r.can_approve(user, *p) {
                            continue;
                        }
                    }
                }

                let entity_type = prefix.map(|p| p.as_str().to_string()).unwrap_or_default();

                items.push(LocalReviewItem {
                    id: id.clone(),
                    short_id: truncate_id(&id),
                    entity_type,
                    title,
                    can_approve: prefix
                        .map(|p| {
                            roster
                                .as_ref()
                                .map(|r| current_user.map(|u| r.can_approve(u, p)).unwrap_or(true))
                                .unwrap_or(true)
                        })
                        .unwrap_or(true),
                });
            }
        }

        self.print_local_reviews(&items)?;

        Ok(())
    }

    fn print_local_reviews(&self, items: &[LocalReviewItem]) -> Result<()> {
        if items.is_empty() {
            println!("No items pending review.");
            return Ok(());
        }

        match self.output.as_str() {
            "short-id" => {
                for item in items {
                    println!("{}", item.short_id);
                }
            }
            "json" => {
                let json = serde_json::to_string_pretty(items).into_diagnostic()?;
                println!("{}", json);
            }
            _ => {
                // Table format
                println!("\nItems pending review:\n");
                println!("{:<15} {:<8} {:<50} CAN APPROVE", "SHORT", "TYPE", "TITLE");
                println!("{}", "-".repeat(85));

                for item in items {
                    let title = if item.title.len() > 48 {
                        format!("{}...", &item.title[..45])
                    } else {
                        item.title.clone()
                    };
                    let can_approve = if item.can_approve { "Yes" } else { "No" };
                    println!(
                        "{:<15} {:<8} {:<50} {}",
                        item.short_id, item.entity_type, title, can_approve
                    );
                }

                let approvable = items.iter().filter(|i| i.can_approve).count();
                println!(
                    "\n{} items pending review ({} you can approve).",
                    items.len(),
                    approvable
                );
                println!("Run `tdt approve <id>` to approve an item.");
            }
        }

        Ok(())
    }
}

fn run_summary(_global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().into_diagnostic()?;
    let config = Config::load();

    use walkdir::WalkDir;

    let mut by_status: std::collections::HashMap<Status, usize> = std::collections::HashMap::new();
    let mut by_type: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

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
            *by_status.entry(status).or_default() += 1;

            if status == Status::Review {
                let entity_type = get_prefix_from_id(&id)
                    .map(|p| p.as_str().to_string())
                    .unwrap_or_else(|| "Unknown".to_string());
                *by_type.entry(entity_type).or_default() += 1;
            }
        }
    }

    println!("\nWorkflow Summary\n");
    println!("Status        Count");
    println!("{}", "-".repeat(25));
    for status in [
        Status::Draft,
        Status::Review,
        Status::Approved,
        Status::Released,
        Status::Obsolete,
    ] {
        let count = by_status.get(&status).unwrap_or(&0);
        println!("{:<13} {}", status, count);
    }

    let review_count = by_status.get(&Status::Review).unwrap_or(&0);
    if *review_count > 0 {
        println!("\nPending Review by Type");
        println!("{}", "-".repeat(25));
        let mut types: Vec<_> = by_type.iter().collect();
        types.sort_by(|a, b| b.1.cmp(a.1));
        for (entity_type, count) in types {
            println!("{:<13} {}", entity_type, count);
        }
    }

    // Provider status
    if config.workflow.enabled {
        println!("\nWorkflow: enabled");
        println!("Provider: {}", config.workflow.provider);
    } else {
        println!("\nWorkflow: disabled");
        println!("Enable with: workflow.enabled: true in .tdt/config.yaml");
    }

    Ok(())
}

#[derive(Debug, serde::Serialize)]
struct PrReviewItem {
    short_id: String,
    entity_type: String,
    title: String,
    author: String,
    pr_number: u64,
    pr_url: String,
}

#[derive(Debug, serde::Serialize)]
struct LocalReviewItem {
    id: String,
    short_id: String,
    entity_type: String,
    title: String,
    can_approve: bool,
}
