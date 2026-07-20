//! `tdt lot` command - Production lot / batch (DHR) management

use clap::{Subcommand, ValueEnum};
use console::style;
use miette::{IntoDiagnostic, Result};
use std::fs;

use crate::cli::helpers::{escape_csv, format_short_id, truncate_str};
use crate::cli::{GlobalOpts, OutputFormat};
use std::collections::HashMap;
use tdt_core::core::cache::EntityCache;
use tdt_core::core::identity::{EntityId, EntityPrefix};
use tdt_core::core::manufacturing::{step_requires_signature, LotWorkflow, LotWorkflowConfig};
use tdt_core::core::project::Project;
use tdt_core::core::shortid::ShortIdIndex;
use tdt_core::core::{Config, Git};
use tdt_core::entities::lot::{
    ApprovalStatus, ExecutionStatus, Lot, LotStatus, StepApproval, WorkInstructionRef,
};
use tdt_core::entities::process::Process;
use tdt_core::schema::wizard::SchemaWizard;
use tdt_core::services::{
    CommonFilter, CreateLot, LotFilter, LotService, LotSortField, SortDirection,
};

/// CLI-friendly lot status enum
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CliLotStatus {
    InProgress,
    OnHold,
    Completed,
    Scrapped,
}

impl std::fmt::Display for CliLotStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CliLotStatus::InProgress => write!(f, "in_progress"),
            CliLotStatus::OnHold => write!(f, "on_hold"),
            CliLotStatus::Completed => write!(f, "completed"),
            CliLotStatus::Scrapped => write!(f, "scrapped"),
        }
    }
}

impl From<CliLotStatus> for LotStatus {
    fn from(cli: CliLotStatus) -> Self {
        match cli {
            CliLotStatus::InProgress => LotStatus::InProgress,
            CliLotStatus::OnHold => LotStatus::OnHold,
            CliLotStatus::Completed => LotStatus::Completed,
            CliLotStatus::Scrapped => LotStatus::Scrapped,
        }
    }
}

#[derive(Subcommand, Debug)]
pub enum LotCommands {
    /// List lots with filtering
    List(ListArgs),

    /// Create a new production lot
    New(NewArgs),

    /// Show a lot's details
    Show(ShowArgs),

    /// Edit a lot in your editor
    Edit(EditArgs),

    /// Delete a lot
    Delete(DeleteArgs),

    /// Archive a lot (soft delete)
    Archive(ArchiveArgs),

    /// Update a process step in the lot
    Step(StepArgs),

    /// Complete a lot
    Complete(CompleteArgs),

    /// Execute/sign-off on a work instruction step (electronic router)
    #[command(name = "wi-step")]
    WiStep(WiStepArgs),

    /// View electronic router/traveler status
    Router(RouterArgs),

    /// Approve a work instruction step (quality sign-off)
    Approve(ApproveArgs),
}

/// Lot status filter
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum LotStatusFilter {
    InProgress,
    OnHold,
    Completed,
    Scrapped,
    All,
}

crate::list_columns! {
    /// List column for display and sorting
    pub enum ListColumn {
        Id => "id",
        Title => "title",
        LotNumber => "lot-number",
        Quantity => "quantity",
        LotStatus => "lot-status",
        Author => "author",
        Created => "created",
    }
}

#[derive(clap::Args, Debug)]
pub struct ListArgs {
    /// Filter by lot status
    #[arg(long, short = 's', default_value = "all")]
    pub status: LotStatusFilter,

    /// Filter by product (ASM or CMP ID)
    #[arg(long)]
    pub product: Option<String>,

    /// Filter by author
    #[arg(long)]
    pub author: Option<String>,

    /// Show only recent lots (last 30 days)
    #[arg(long)]
    pub recent: bool,

    /// Search in title and lot number
    #[arg(long)]
    pub search: Option<String>,

    /// Show only active lots (not completed or scrapped)
    #[arg(long)]
    pub active: bool,

    /// Columns to display
    #[arg(long, value_delimiter = ',', default_values_t = vec![
        ListColumn::Title,
        ListColumn::LotNumber,
        ListColumn::Quantity,
        ListColumn::LotStatus
    ])]
    pub columns: Vec<ListColumn>,

    /// Show full ID column (hidden by default since SHORT is always shown)
    #[arg(long)]
    pub show_id: bool,

    /// Sort by column
    #[arg(long, default_value = "created")]
    pub sort: ListColumn,

    /// Reverse sort order
    #[arg(long, short = 'r')]
    pub reverse: bool,

    /// Limit number of results
    #[arg(long, short = 'n')]
    pub limit: Option<usize>,

    /// Show only count
    #[arg(long)]
    pub count: bool,

    /// Only show entities linked to these IDs (use - for stdin pipe)
    #[arg(long, value_delimiter = ',')]
    pub linked_to: Vec<String>,

    /// Filter by link type when using --linked-to (e.g., verified_by, satisfied_by)
    #[arg(long, requires = "linked_to")]
    pub via: Option<String>,
}

#[derive(clap::Args, Debug)]
pub struct NewArgs {
    /// Lot title (required)
    #[arg(long, short = 't')]
    pub title: Option<String>,

    /// User-defined lot number
    #[arg(long, short = 'l')]
    pub lot_number: Option<String>,

    /// Quantity of units in this lot
    #[arg(long, short = 'Q')]
    pub quantity: Option<u32>,

    /// Product being made (ASM or CMP ID)
    #[arg(long, short = 'p')]
    pub product: Option<String>,

    /// Create a git branch for this lot (DHR workflow)
    #[arg(long, short = 'b')]
    pub branch: bool,

    /// Skip branch creation even if config enables it
    #[arg(long)]
    pub no_branch: bool,

    /// Auto-populate execution steps from product's manufacturing routing
    #[arg(long, short = 'r')]
    pub from_routing: bool,

    /// Open in editor after creation
    #[arg(long, short = 'e')]
    pub edit: bool,

    /// Skip opening in editor
    #[arg(long)]
    pub no_edit: bool,

    /// Interactive mode (prompt for fields)
    #[arg(long, short = 'i')]
    pub interactive: bool,

    /// Link to another entity (auto-infers link type)
    #[arg(long, short = 'L')]
    pub link: Vec<String>,
}

#[derive(clap::Args, Debug)]
pub struct ShowArgs {
    /// Lot ID or short ID (LOT@N)
    pub id: String,
}

#[derive(clap::Args, Debug)]
pub struct EditArgs {
    /// Lot ID or short ID (LOT@N)
    pub id: String,
}

#[derive(clap::Args, Debug)]
pub struct DeleteArgs {
    /// Lot ID or short ID (LOT@N)
    pub id: String,

    /// Force deletion even if other entities reference this one
    #[arg(long)]
    pub force: bool,

    /// Suppress output
    #[arg(long, short = 'q')]
    pub quiet: bool,
}

#[derive(clap::Args, Debug)]
pub struct ArchiveArgs {
    /// Lot ID or short ID (LOT@N)
    pub id: String,

    /// Force archive even if other entities reference this one
    #[arg(long)]
    pub force: bool,

    /// Suppress output
    #[arg(long, short = 'q')]
    pub quiet: bool,
}

/// CLI-friendly execution status enum
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CliExecutionStatus {
    Pending,
    InProgress,
    Completed,
    Skipped,
}

impl From<CliExecutionStatus> for ExecutionStatus {
    fn from(cli: CliExecutionStatus) -> Self {
        match cli {
            CliExecutionStatus::Pending => ExecutionStatus::Pending,
            CliExecutionStatus::InProgress => ExecutionStatus::InProgress,
            CliExecutionStatus::Completed => ExecutionStatus::Completed,
            CliExecutionStatus::Skipped => ExecutionStatus::Skipped,
        }
    }
}

#[derive(clap::Args, Debug)]
pub struct StepArgs {
    /// Lot ID or short ID (LOT@N)
    pub lot: String,

    /// Process step to update (PROC ID or index)
    #[arg(long, short = 'p')]
    pub process: Option<String>,

    /// New status for the step
    #[arg(long, short = 's')]
    pub status: Option<CliExecutionStatus>,

    /// Operator name (defaults to config author)
    #[arg(long, short = 'O')]
    pub operator: Option<String>,

    /// Notes about the step execution
    #[arg(long, short = 'n')]
    pub notes: Option<String>,

    /// Show work instructions for this step
    #[arg(long)]
    pub show_wi: bool,

    /// Record work instruction IDs used (comma-separated)
    #[arg(long, value_delimiter = ',')]
    pub wi_used: Vec<String>,

    /// Sign step completion (required for processes with require_signature)
    #[arg(long, short = 'S')]
    pub sign: bool,

    /// Skip git commit for this step
    #[arg(long)]
    pub no_commit: bool,

    /// Interactive mode (prompt for step details)
    #[arg(long, short = 'i')]
    pub interactive: bool,
}

#[derive(clap::Args, Debug)]
pub struct CompleteArgs {
    /// Lot ID or short ID (LOT@N)
    pub lot: String,

    /// Skip confirmation prompt
    #[arg(long, short = 'y')]
    pub yes: bool,

    /// Skip merging the lot branch to main
    #[arg(long)]
    pub no_merge: bool,

    /// Sign the merge commit
    #[arg(long, short = 'S')]
    pub sign: bool,
}

/// Execute/sign-off on a work instruction step (electronic router)
#[derive(clap::Args, Debug)]
pub struct WiStepArgs {
    /// Lot ID or short ID (LOT@N)
    pub lot: String,

    /// Work instruction ID (WORK@N or WORK-xxx)
    #[arg(long, short = 'w')]
    pub wi: String,

    /// Step number within the work instruction
    #[arg(long, short = 's')]
    pub step: u32,

    /// Process step index (1-based) within lot execution
    #[arg(long, short = 'p')]
    pub process: Option<usize>,

    /// Operator name (defaults to config author)
    #[arg(long, short = 'O')]
    pub operator: Option<String>,

    /// Record data (key=value format, can be repeated)
    #[arg(long, short = 'd', value_parser = parse_key_value)]
    pub data: Vec<(String, String)>,

    /// Record equipment used (equipment=serial format, can be repeated)
    #[arg(long, short = 'E', value_parser = parse_key_value)]
    pub equipment: Vec<(String, String)>,

    /// Notes about the step execution
    #[arg(long, short = 'n')]
    pub notes: Option<String>,

    /// Sign step completion (GPG/SSH signature)
    #[arg(long, short = 'S')]
    pub sign: bool,

    /// Mark step as requiring approval (sets approval_status to pending)
    #[arg(long)]
    pub require_approval: bool,

    /// Skip git commit for this step
    #[arg(long)]
    pub no_commit: bool,

    /// Show step status only (no updates)
    #[arg(long)]
    pub show: bool,

    /// Mark the step as complete
    #[arg(long, short = 'c')]
    pub complete: bool,

    /// Approved deviation ID to bypass step order enforcement (DEV@N or DEV-xxx)
    #[arg(long)]
    pub deviation: Option<String>,
}

/// View electronic router/traveler status
#[derive(clap::Args, Debug)]
pub struct RouterArgs {
    /// Lot ID or short ID (LOT@N)
    pub lot: String,

    /// Show only pending steps (not yet completed)
    #[arg(long)]
    pub pending: bool,

    /// Show only steps requiring approval
    #[arg(long)]
    pub approval_needed: bool,

    /// Filter by work instruction ID
    #[arg(long, short = 'w')]
    pub wi: Option<String>,

    /// Filter by process step index (1-based)
    #[arg(long, short = 'p')]
    pub process: Option<usize>,
}

/// Approve a work instruction step (quality sign-off)
#[derive(clap::Args, Debug)]
pub struct ApproveArgs {
    /// Lot ID or short ID (LOT@N)
    pub lot: String,

    /// Work instruction ID (WORK@N or WORK-xxx)
    #[arg(long, short = 'w')]
    pub wi: String,

    /// Step number within the work instruction
    #[arg(long, short = 's')]
    pub step: u32,

    /// Process step index (1-based) within lot execution
    #[arg(long, short = 'p')]
    pub process: Option<usize>,

    /// Comment/reason for approval
    #[arg(long, short = 'c')]
    pub comment: Option<String>,

    /// Role performing the approval (e.g., quality, engineering)
    #[arg(long, short = 'r')]
    pub role: Option<String>,

    /// Sign the approval (GPG/SSH signature)
    #[arg(long, short = 'S')]
    pub sign: bool,

    /// Reject instead of approve
    #[arg(long)]
    pub reject: bool,

    /// Show pending approvals only (no action)
    #[arg(long)]
    pub show_pending: bool,
}

/// Parse key=value pairs for data/equipment arguments
fn parse_key_value(s: &str) -> Result<(String, String), String> {
    let parts: Vec<&str> = s.splitn(2, '=').collect();
    if parts.len() != 2 {
        return Err(format!("Invalid format '{}'. Use key=value", s));
    }
    Ok((parts[0].to_string(), parts[1].to_string()))
}

/// Get user email from git config
fn get_git_email() -> Option<String> {
    std::process::Command::new("git")
        .args(["config", "user.email"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                let email = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !email.is_empty() {
                    Some(email)
                } else {
                    None
                }
            } else {
                None
            }
        })
}

/// Search directories for this entity type — single source of truth
/// lives in `Project::entity_search_directories`.
const LOT_DIRS: &[&str] = tdt_core::core::project::Project::entity_search_directories(
    tdt_core::core::identity::EntityPrefix::Lot,
);

/// Entity configuration for lot commands
const ENTITY_CONFIG: crate::cli::EntityConfig = crate::cli::EntityConfig {
    prefix: EntityPrefix::Lot,
    dirs: LOT_DIRS,
    name: "lot",
    name_plural: "lots",
};

/// Run a lot subcommand
pub fn run(cmd: LotCommands, global: &GlobalOpts) -> Result<()> {
    match cmd {
        LotCommands::List(args) => run_list(args, global),
        LotCommands::New(args) => run_new(args, global),
        LotCommands::Show(args) => run_show(args, global),
        LotCommands::Edit(args) => run_edit(args),
        LotCommands::Delete(args) => run_delete(args),
        LotCommands::Archive(args) => run_archive(args),
        LotCommands::Step(args) => run_step(args, global),
        LotCommands::Complete(args) => run_complete(args, global),
        LotCommands::WiStep(args) => run_wi_step(args, global),
        LotCommands::Router(args) => run_router(args, global),
        LotCommands::Approve(args) => run_approve(args, global),
    }
}

/// Build a LotFilter from CLI ListArgs
fn build_lot_filter(args: &ListArgs) -> LotFilter {
    // Map lot status
    let lot_status = match args.status {
        LotStatusFilter::InProgress => Some(LotStatus::InProgress),
        LotStatusFilter::OnHold => Some(LotStatus::OnHold),
        LotStatusFilter::Completed => Some(LotStatus::Completed),
        LotStatusFilter::Scrapped => Some(LotStatus::Scrapped),
        LotStatusFilter::All => None,
    };

    LotFilter {
        common: CommonFilter {
            author: args.author.clone(),
            search: args.search.clone(),
            limit: None, // Apply limit after sorting
            ..Default::default()
        },
        lot_status,
        product: args.product.clone(),
        active_only: args.active,
        recent_days: if args.recent { Some(30) } else { None },
        sort: build_lot_sort_field(&args.sort),
        sort_direction: if args.reverse {
            SortDirection::Descending
        } else {
            SortDirection::Ascending
        },
    }
}

/// Convert CLI sort column to LotSortField
fn build_lot_sort_field(col: &ListColumn) -> LotSortField {
    match col {
        ListColumn::Id => LotSortField::Id,
        ListColumn::Title => LotSortField::Title,
        ListColumn::LotNumber => LotSortField::LotNumber,
        ListColumn::Quantity => LotSortField::Quantity,
        ListColumn::LotStatus => LotSortField::LotStatus,
        ListColumn::Author => LotSortField::Author,
        ListColumn::Created => LotSortField::Created,
    }
}

/// Output full lot entities
fn output_lots(
    lots: &[Lot],
    short_ids: &mut ShortIdIndex,
    args: &ListArgs,
    format: OutputFormat,
    project: &Project,
) -> Result<()> {
    if lots.is_empty() {
        if args.count {
            println!("0");
        } else {
            println!("No lots found.");
        }
        return Ok(());
    }

    if args.count {
        println!("{}", lots.len());
        return Ok(());
    }

    // Update short ID index
    short_ids.ensure_all(lots.iter().map(|l| l.id.to_string()));
    super::utils::save_short_ids(short_ids, project);

    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&lots).into_diagnostic()?;
            println!("{}", json);
        }
        OutputFormat::Yaml => {
            let yaml = serde_yml::to_string(&lots).into_diagnostic()?;
            print!("{}", yaml);
        }
        OutputFormat::Csv => {
            println!("short_id,id,title,lot_number,quantity,lot_status,author");
            for lot in lots {
                let short_id = short_ids
                    .get_short_id(&lot.id.to_string())
                    .unwrap_or_default();
                println!(
                    "{},{},{},{},{},{},{}",
                    short_id,
                    lot.id,
                    escape_csv(&lot.title),
                    lot.lot_number.as_deref().unwrap_or(""),
                    lot.quantity.map(|q| q.to_string()).unwrap_or_default(),
                    lot.lot_status,
                    escape_csv(&lot.author)
                );
            }
        }
        OutputFormat::Tsv | OutputFormat::Table | OutputFormat::Dot | OutputFormat::Tree => {
            // Build columns list, adding ID column if --show-id is set
            let columns: Vec<ListColumn> =
                if args.show_id && !args.columns.contains(&ListColumn::Id) {
                    let mut cols = vec![ListColumn::Id];
                    cols.extend(args.columns.iter().copied());
                    cols
                } else {
                    args.columns.clone()
                };

            // Build header
            let mut headers = vec![];
            let mut widths = vec![];

            for col in &columns {
                let (header, width) = match col {
                    ListColumn::Id => ("ID", 17),
                    ListColumn::Title => ("TITLE", 26),
                    ListColumn::LotNumber => ("LOT #", 14),
                    ListColumn::Quantity => ("QTY", 6),
                    ListColumn::LotStatus => ("STATUS", 12),
                    ListColumn::Author => ("AUTHOR", 16),
                    ListColumn::Created => ("CREATED", 20),
                };
                headers.push((header, *col));
                widths.push(width);
            }

            // Print header
            print!("{:<8} ", style("SHORT").bold().dim());
            for (i, (header, _)) in headers.iter().enumerate() {
                print!("{:<width$} ", style(header).bold(), width = widths[i]);
            }
            println!();
            println!(
                "{}",
                "-".repeat(8 + widths.iter().sum::<usize>() + widths.len())
            );

            // Print rows
            for lot in lots {
                let short_id = short_ids
                    .get_short_id(&lot.id.to_string())
                    .unwrap_or_default();
                print!("{:<8} ", style(&short_id).cyan());

                for (i, (_, col)) in headers.iter().enumerate() {
                    let cell = match col {
                        ListColumn::Id => format_short_id(&lot.id),
                        ListColumn::Title => truncate_str(&lot.title, widths[i] - 2),
                        ListColumn::LotNumber => {
                            lot.lot_number.as_deref().unwrap_or("").to_string()
                        }
                        ListColumn::Quantity => {
                            lot.quantity.map(|q| q.to_string()).unwrap_or_default()
                        }
                        ListColumn::LotStatus => {
                            let status_styled = match lot.lot_status {
                                LotStatus::InProgress => style(lot.lot_status.to_string()).green(),
                                LotStatus::OnHold => style(lot.lot_status.to_string()).yellow(),
                                LotStatus::Completed => style(lot.lot_status.to_string()).cyan(),
                                LotStatus::Scrapped => style(lot.lot_status.to_string()).red(),
                            };
                            print!("{:<width$} ", status_styled, width = widths[i]);
                            continue;
                        }
                        ListColumn::Author => truncate_str(&lot.author, widths[i] - 2),
                        ListColumn::Created => lot.created.format("%Y-%m-%d %H:%M").to_string(),
                    };
                    print!("{:<width$} ", cell, width = widths[i]);
                }
                println!();
            }

            println!();
            println!(
                "{} lot(s) found. Use {} to reference by short ID.",
                style(lots.len()).cyan(),
                style("LOT@N").cyan()
            );
        }
        OutputFormat::Id | OutputFormat::ShortId => {
            for lot in lots {
                if format == OutputFormat::ShortId {
                    let short_id = short_ids
                        .get_short_id(&lot.id.to_string())
                        .unwrap_or_default();
                    println!("{}", short_id);
                } else {
                    println!("{}", lot.id);
                }
            }
        }
        OutputFormat::Md => {
            println!("| Short | ID | Title | Lot # | Qty | Status | Author |");
            println!("|---|---|---|---|---|---|---|");
            for lot in lots {
                let short_id = short_ids
                    .get_short_id(&lot.id.to_string())
                    .unwrap_or_default();
                println!(
                    "| {} | {} | {} | {} | {} | {} | {} |",
                    short_id,
                    format_short_id(&lot.id),
                    lot.title,
                    lot.lot_number.as_deref().unwrap_or("-"),
                    lot.quantity
                        .map(|q| q.to_string())
                        .unwrap_or("-".to_string()),
                    lot.lot_status,
                    lot.author
                );
            }
        }
        OutputFormat::Auto | OutputFormat::Path => {
            eprintln!("error: -o path is not supported for this view; use -o id to get entity IDs");
            std::process::exit(2);
        }
    }

    Ok(())
}

fn run_list(args: ListArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let cache = EntityCache::open(&project).map_err(|e| miette::miette!("{}", e))?;
    let service = LotService::new(&project, &cache);
    let mut short_ids = ShortIdIndex::load(&project);

    // Resolve linked-to filter via cache
    let allowed_ids = crate::cli::helpers::resolve_linked_to(
        &args.linked_to,
        args.via.as_deref(),
        &short_ids,
        &cache,
    );

    let format = global.list_format();

    // Build filter from CLI args
    let filter = build_lot_filter(&args);

    // Load and filter lots using service
    let mut lots = service
        .list(&filter)
        .map_err(|e| miette::miette!("{}", e))?;

    // Apply linked-to filter
    if let Some(ref ids) = allowed_ids {
        lots.retain(|e| ids.contains(&e.id.to_string()));
    }

    // Apply limit
    if let Some(limit) = args.limit {
        lots.truncate(limit);
    }

    output_lots(&lots, &mut short_ids, &args, format, &project)
}

fn run_new(args: NewArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let config = Config::load();
    let cache = EntityCache::open(&project).map_err(|e| miette::miette!("{}", e))?;

    let title: String;
    let lot_number: Option<String>;
    let quantity: Option<u32>;
    let product: Option<String>;
    let notes: Option<String>;

    // Load short IDs early since we need them for product resolution
    let mut short_ids = ShortIdIndex::load(&project);

    if args.interactive {
        let wizard = SchemaWizard::new();
        let result = wizard.run(EntityPrefix::Lot)?;

        title = result
            .get_string("title")
            .map(String::from)
            .unwrap_or_else(|| "New Production Lot".to_string());
        lot_number = result.get_string("lot_number").map(String::from);
        quantity = result.get_i64("quantity").map(|n| n as u32);
        product = result
            .get_string("product")
            .map(|p| short_ids.resolve(p).unwrap_or_else(|| p.to_string()));
        notes = result.get_string("notes").map(String::from);
    } else {
        title = args
            .title
            .unwrap_or_else(|| "New Production Lot".to_string());
        lot_number = args.lot_number;
        quantity = args.quantity;
        product = args.product.map(|p| short_ids.resolve(&p).unwrap_or(p));
        notes = None;
    }

    // Create lot via service
    let service = LotService::new(&project, &cache);
    let input = CreateLot {
        title: title.clone(),
        lot_number: lot_number.clone(),
        quantity,
        product: product.clone(),
        notes,
        start_date: None,
        tags: Vec::new(),
        status: None,
        author: config.author(),
        from_routing: args.from_routing,
    };

    let lot = service
        .create(input)
        .map_err(|e| miette::miette!("{}", e))?;
    let id = lot.id.clone();
    let file_path = project
        .root()
        .join(format!("manufacturing/lots/{}.tdt.yaml", id));

    // from_routing is now handled by the service layer in CreateLot
    let execution_steps_added = lot.execution.len();
    let mut git_branch_created: Option<String> = None;

    // Handle --branch: create git branch for lot workflow
    let should_create_branch = args.branch
        || (!args.no_branch
            && config
                .manufacturing
                .as_ref()
                .map(|m| m.lot_branch_enabled)
                .unwrap_or(false));

    if should_create_branch {
        // Reload the lot to get the latest version (may have execution steps added)
        let lot = service
            .get(&id.to_string())
            .map_err(|e| miette::miette!("{}", e))?
            .ok_or_else(|| miette::miette!("Failed to reload lot"))?;

        // Initialize git and workflow
        let git = Git::new(project.root());
        if git.is_repo() {
            let workflow_config = LotWorkflowConfig::from_config(&config);
            let workflow = LotWorkflow::new(&git, workflow_config);

            match workflow.init_lot_branch(&lot) {
                Ok(branch_name) => {
                    // Update lot with branch name using service
                    service
                        .set_git_branch(&id.to_string(), &branch_name)
                        .map_err(|e| miette::miette!("{}", e))?;
                    git_branch_created = Some(branch_name);
                }
                Err(e) => {
                    eprintln!(
                        "{} Warning: Could not create git branch: {}",
                        style("!").yellow(),
                        e
                    );
                }
            }
        } else {
            eprintln!(
                "{} Warning: Not a git repository, skipping branch creation",
                style("!").yellow()
            );
        }
    }

    // Add to short ID index
    let short_id = short_ids.add(id.to_string());
    super::utils::save_short_ids(&mut short_ids, &project);

    // Handle --link flags
    let added_links = crate::cli::entity_cmd::process_link_flags(
        &file_path,
        EntityPrefix::Lot,
        &args.link,
        &short_ids,
    );

    // Output based on format flag
    match global.output {
        OutputFormat::Id => {
            println!("{}", id);
        }
        OutputFormat::ShortId => {
            println!(
                "{}",
                short_id.clone().unwrap_or_else(|| format_short_id(&id))
            );
        }
        OutputFormat::Path => {
            println!("{}", file_path.display());
        }
        _ => {
            println!(
                "{} Created lot {}",
                style("✓").green(),
                style(short_id.clone().unwrap_or_else(|| format_short_id(&id))).cyan()
            );
            println!("   {}", style(file_path.display()).dim());
            println!(
                "   {} | {}",
                style(lot_number.as_deref().unwrap_or("(no lot #)")).yellow(),
                style(&title).white()
            );

            // Show added links
            for (link_type, target) in &added_links {
                println!(
                    "   {} --[{}]--> {}",
                    style("→").dim(),
                    style(link_type).cyan(),
                    style(format_short_id(&EntityId::parse(target).unwrap())).yellow()
                );
            }

            // Show routing steps if added
            if execution_steps_added > 0 {
                println!(
                    "   {} {} execution step{} from routing",
                    style("→").dim(),
                    style(execution_steps_added).cyan(),
                    if execution_steps_added == 1 { "" } else { "s" }
                );
            }

            // Show git branch if created
            if let Some(ref branch) = git_branch_created {
                println!(
                    "   {} Git branch: {}",
                    style("→").dim(),
                    style(branch).cyan()
                );
            }
        }
    }

    // Sync cache after creation
    super::utils::sync_cache(&project);

    // Open in editor if requested
    if args.edit || (!args.no_edit && !args.interactive) {
        println!();
        println!("Opening in {}...", style(config.editor()).yellow());

        config.run_editor(&file_path).into_diagnostic()?;
    }

    Ok(())
}

fn run_show(args: ShowArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let cache = EntityCache::open(&project).map_err(|e| miette::miette!("{}", e))?;

    // Resolve short ID if needed
    let short_ids = ShortIdIndex::load(&project);
    let resolved_id = short_ids
        .resolve(&args.id)
        .unwrap_or_else(|| args.id.clone());

    // Use LotService to get the lot (cache-first lookup)
    let service = LotService::new(&project, &cache);
    let lot = service
        .get(&resolved_id)
        .map_err(|e| miette::miette!("{}", e))?
        .ok_or_else(|| miette::miette!("No lot found matching '{}'", args.id))?;

    match global.output {
        OutputFormat::Yaml => {
            let yaml = serde_yml::to_string(&lot).into_diagnostic()?;
            print!("{}", yaml);
        }
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&lot).into_diagnostic()?;
            println!("{}", json);
        }
        OutputFormat::Id | OutputFormat::ShortId => {
            if global.output == OutputFormat::ShortId {
                let short_id = short_ids
                    .get_short_id(&lot.id.to_string())
                    .unwrap_or_default();
                println!("{}", short_id);
            } else {
                println!("{}", lot.id);
            }
        }
        _ => {
            // Pretty format (default)
            println!("{}", style("─".repeat(60)).dim());
            println!(
                "{}: {}",
                style("ID").bold(),
                style(&lot.id.to_string()).cyan()
            );
            println!("{}: {}", style("Title").bold(), style(&lot.title).yellow());
            if let Some(ref ln) = lot.lot_number {
                println!("{}: {}", style("Lot Number").bold(), ln);
            }
            if let Some(q) = lot.quantity {
                println!("{}: {}", style("Quantity").bold(), q);
            }
            let status_styled = match lot.lot_status {
                LotStatus::InProgress => style(lot.lot_status.to_string()).green(),
                LotStatus::OnHold => style(lot.lot_status.to_string()).yellow(),
                LotStatus::Completed => style(lot.lot_status.to_string()).cyan(),
                LotStatus::Scrapped => style(lot.lot_status.to_string()).red(),
            };
            println!("{}: {}", style("Status").bold(), status_styled);

            if let Some(ref start) = lot.start_date {
                println!("{}: {}", style("Start Date").bold(), start);
            }
            if let Some(ref end) = lot.completion_date {
                println!("{}: {}", style("Completion Date").bold(), end);
            }
            println!("{}", style("─".repeat(60)).dim());

            // Materials used
            if !lot.materials_used.is_empty() {
                println!();
                println!(
                    "{} ({}):",
                    style("Materials Used").bold(),
                    lot.materials_used.len()
                );
                for mat in &lot.materials_used {
                    let comp = mat.component.as_deref().unwrap_or("(no CMP)");
                    let supplier_lot = mat.supplier_lot.as_deref().unwrap_or("-");
                    let qty = mat
                        .quantity
                        .map(|q| q.to_string())
                        .unwrap_or("-".to_string());
                    println!("  • {} | Lot: {} | Qty: {}", comp, supplier_lot, qty);
                }
            }

            // Execution steps
            if !lot.execution.is_empty() {
                println!();
                println!(
                    "{} ({}):",
                    style("Execution Steps").bold(),
                    lot.execution.len()
                );
                for (i, step) in lot.execution.iter().enumerate() {
                    let proc_id = step.process.as_deref().unwrap_or("(unlinked)");
                    let proc_display = short_ids
                        .get_short_id(proc_id)
                        .unwrap_or_else(|| proc_id.to_string());
                    // Try to load process title
                    let proc_title = {
                        let proc_file = project
                            .root()
                            .join("manufacturing/processes")
                            .join(format!("{}.tdt.yaml", proc_id));
                        if let Ok(content) = std::fs::read_to_string(&proc_file) {
                            if let Ok(val) = serde_yml::from_str::<serde_json::Value>(&content) {
                                val.get("title").and_then(|v| v.as_str()).map(String::from)
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    };
                    let status_styled = match step.status {
                        ExecutionStatus::Pending => style("pending").dim(),
                        ExecutionStatus::InProgress => style("in_progress").yellow(),
                        ExecutionStatus::Completed => style("completed").green(),
                        ExecutionStatus::Skipped => style("skipped").dim(),
                    };
                    if let Some(ref title) = proc_title {
                        print!(
                            "  {}. {} - {} [{}]",
                            i + 1,
                            style(&proc_display).cyan(),
                            title,
                            status_styled
                        );
                    } else {
                        print!(
                            "  {}. {} [{}]",
                            i + 1,
                            style(&proc_display).cyan(),
                            status_styled
                        );
                    }
                    if let Some(ref date) = step.completed_date {
                        print!(" - {}", date);
                    }
                    if let Some(ref op) = step.operator {
                        print!(" by {}", style(op).dim());
                    }
                    println!();
                    if let Some(ref notes) = step.notes {
                        if !notes.is_empty() {
                            println!("     {}", style(notes).dim());
                        }
                    }
                }
            }

            // Links
            let has_links = lot.links.product.is_some()
                || !lot.links.processes.is_empty()
                || !lot.links.work_instructions.is_empty()
                || !lot.links.ncrs.is_empty()
                || !lot.links.results.is_empty();

            if has_links {
                println!();
                println!("{}", style("Links:").bold());

                if let Some(ref prod) = lot.links.product {
                    let display = short_ids
                        .get_short_id(prod.id())
                        .unwrap_or_else(|| prod.id().to_string());
                    println!("  {}: {}", style("Product").dim(), style(&display).cyan());
                }
                if !lot.links.processes.is_empty() {
                    let proc_list: Vec<_> = lot
                        .links
                        .processes
                        .iter()
                        .map(|p| {
                            short_ids
                                .get_short_id(p.id())
                                .unwrap_or_else(|| p.id().to_string())
                        })
                        .collect();
                    println!(
                        "  {}: {}",
                        style("Processes").dim(),
                        style(proc_list.join(", ")).cyan()
                    );
                }
                if !lot.links.ncrs.is_empty() {
                    let ncr_list: Vec<_> = lot
                        .links
                        .ncrs
                        .iter()
                        .map(|n| {
                            short_ids
                                .get_short_id(n.id())
                                .unwrap_or_else(|| n.id().to_string())
                        })
                        .collect();
                    println!(
                        "  {}: {}",
                        style("NCRs").dim(),
                        style(ncr_list.join(", ")).red()
                    );
                }
            }

            // Notes
            if let Some(ref notes) = lot.notes {
                if !notes.is_empty() && !notes.starts_with('#') {
                    println!();
                    println!("{}", style("Notes:").bold());
                    println!("{}", notes);
                }
            }

            // Footer
            println!("{}", style("─".repeat(60)).dim());
            println!(
                "{}: {} | {}: {} | {}: {}",
                style("Author").dim(),
                lot.author,
                style("Created").dim(),
                lot.created.format("%Y-%m-%d %H:%M"),
                style("Revision").dim(),
                lot.entity_revision
            );
        }
    }

    Ok(())
}

fn run_edit(args: EditArgs) -> Result<()> {
    crate::cli::entity_cmd::run_edit_generic(&args.id, &ENTITY_CONFIG)
}

fn run_delete(args: DeleteArgs) -> Result<()> {
    crate::cli::commands::utils::run_delete(&args.id, LOT_DIRS, args.force, false, args.quiet)
}

fn run_archive(args: ArchiveArgs) -> Result<()> {
    crate::cli::commands::utils::run_delete(&args.id, LOT_DIRS, args.force, true, args.quiet)
}

fn run_step(args: StepArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let config = Config::load();

    // Resolve short ID if needed
    let short_ids = ShortIdIndex::load(&project);
    let resolved_id = short_ids
        .resolve(&args.lot)
        .unwrap_or_else(|| args.lot.clone());

    // Find the lot file
    let lot_dir = project.root().join("manufacturing/lots");
    let mut found_path = None;

    if lot_dir.exists() {
        for entry in fs::read_dir(&lot_dir).into_diagnostic()? {
            let entry = entry.into_diagnostic()?;
            let path = entry.path();

            if path.extension().is_some_and(|e| e == "yaml") {
                let filename = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                if filename.contains(&resolved_id) || filename.starts_with(&resolved_id) {
                    found_path = Some(path);
                    break;
                }
            }
        }
    }

    let path = found_path.ok_or_else(|| miette::miette!("No lot found matching '{}'", args.lot))?;

    // Read and parse lot
    let content = fs::read_to_string(&path).into_diagnostic()?;
    let mut lot: Lot = serde_yml::from_str(&content).into_diagnostic()?;

    // Get display ID
    let display_id = short_ids
        .get_short_id(&lot.id.to_string())
        .unwrap_or_else(|| format_short_id(&lot.id));

    // Find the step to update
    let step_idx: Option<usize> = if let Some(ref proc) = args.process {
        // Try to find by process ID
        let resolved_proc = short_ids.resolve(proc).unwrap_or_else(|| proc.clone());
        lot.execution
            .iter()
            .position(|s| {
                s.process
                    .as_ref()
                    .is_some_and(|p| p.contains(&resolved_proc))
            })
            .or_else(|| proc.parse::<usize>().ok().map(|i| i.saturating_sub(1)))
    } else {
        // Find first non-completed step
        lot.execution.iter().position(|s| {
            s.status == ExecutionStatus::Pending || s.status == ExecutionStatus::InProgress
        })
    };

    let step_idx = step_idx.ok_or_else(|| {
        if lot.execution.is_empty() {
            miette::miette!("No execution steps defined in lot {}", display_id)
        } else {
            miette::miette!("No pending steps found in lot {}", display_id)
        }
    })?;

    if step_idx >= lot.execution.len() {
        return Err(miette::miette!(
            "Step index {} out of range (lot has {} steps)",
            step_idx + 1,
            lot.execution.len()
        ));
    }

    // Load all processes for signature requirement checking
    let mut processes: HashMap<String, Process> = HashMap::new();
    let proc_dir = project.root().join("manufacturing/processes");
    if proc_dir.exists() {
        for entry in walkdir::WalkDir::new(&proc_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        {
            if let Ok(content) = fs::read_to_string(entry.path()) {
                if let Ok(proc) = serde_yml::from_str::<Process>(&content) {
                    processes.insert(proc.id.to_string(), proc);
                }
            }
        }
    }

    // Check if process requires signature
    let step = &lot.execution[step_idx];
    let requires_signature = step_requires_signature(step, &processes);

    // Get the current process for WI display
    let current_process = step.process.as_ref().and_then(|p| processes.get(p));

    // Show work instructions if requested
    if args.show_wi {
        if let Some(proc) = current_process {
            println!();
            println!("{}", style("Work Instructions for Step").bold().cyan());
            println!("{}", style("─".repeat(50)).dim());
            if proc.links.work_instructions.is_empty() {
                println!("   {}", style("No work instructions linked").dim());
            } else {
                for wi_id in &proc.links.work_instructions {
                    let wi_short = short_ids
                        .get_short_id(&wi_id.to_string())
                        .unwrap_or_else(|| wi_id.to_string());
                    println!("   • {}", style(&wi_short).cyan());
                }
            }
            println!();
        } else {
            println!("{} No process linked to this step", style("!").yellow());
        }
    }

    // Interactive mode
    let new_status: ExecutionStatus;
    let operator: String;
    let notes: Option<String>;

    if args.interactive {
        println!();
        println!("{}", style("Update Execution Step").bold().cyan());
        println!("{}", style("─".repeat(50)).dim());
        println!("Lot: {} \"{}\"", style(&display_id).cyan(), &lot.title);
        println!(
            "Step {}: {}",
            step_idx + 1,
            lot.execution[step_idx]
                .process
                .as_deref()
                .unwrap_or("(unlinked)")
        );
        println!("Current Status: {}", lot.execution[step_idx].status);
        println!();

        // Simple prompts
        print!("New status (pending/in_progress/completed/skipped) [completed]: ");
        std::io::Write::flush(&mut std::io::stdout()).into_diagnostic()?;
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).into_diagnostic()?;
        let input = input.trim();
        new_status = if input.is_empty() {
            ExecutionStatus::Completed
        } else {
            input.parse().unwrap_or(ExecutionStatus::Completed)
        };

        print!("Operator [{}]: ", config.author());
        std::io::Write::flush(&mut std::io::stdout()).into_diagnostic()?;
        let mut op_input = String::new();
        std::io::stdin()
            .read_line(&mut op_input)
            .into_diagnostic()?;
        operator = if op_input.trim().is_empty() {
            config.author().to_string()
        } else {
            op_input.trim().to_string()
        };

        print!("Notes (optional): ");
        std::io::Write::flush(&mut std::io::stdout()).into_diagnostic()?;
        let mut notes_input = String::new();
        std::io::stdin()
            .read_line(&mut notes_input)
            .into_diagnostic()?;
        notes = if notes_input.trim().is_empty() {
            None
        } else {
            Some(notes_input.trim().to_string())
        };
    } else {
        new_status = args
            .status
            .map(ExecutionStatus::from)
            .unwrap_or(ExecutionStatus::Completed);
        operator = args.operator.unwrap_or_else(|| config.author().to_string());
        notes = args.notes;
    }

    // Check if signature is required when completing
    if new_status == ExecutionStatus::Completed && requires_signature && !args.sign {
        let proc_id = lot.execution[step_idx]
            .process
            .as_deref()
            .unwrap_or("(unknown)");
        return Err(miette::miette!(
            "Process {} requires operator signature. Use --sign flag.",
            proc_id
        ));
    }

    // Update the step
    lot.execution[step_idx].status = new_status;
    lot.execution[step_idx].operator = Some(operator.clone());

    // Set started_date if transitioning to in_progress
    if new_status == ExecutionStatus::InProgress && lot.execution[step_idx].started_date.is_none() {
        lot.execution[step_idx].started_date = Some(chrono::Local::now().date_naive());
    }

    if new_status == ExecutionStatus::Completed {
        lot.execution[step_idx].completed_date = Some(chrono::Local::now().date_naive());
    }

    if let Some(ref n) = notes {
        lot.execution[step_idx].notes = Some(n.clone());
    }

    // Record work instructions used
    if !args.wi_used.is_empty() {
        // User explicitly specified WIs
        lot.execution[step_idx].work_instructions_used = args
            .wi_used
            .iter()
            .map(|wi| {
                let resolved = short_ids.resolve(wi).unwrap_or_else(|| wi.clone());
                WorkInstructionRef {
                    id: resolved,
                    revision: None,
                }
            })
            .collect();
    } else if lot.execution[step_idx].work_instructions_used.is_empty() {
        // Auto-populate from process if not already set
        if let Some(proc) = current_process {
            lot.execution[step_idx].work_instructions_used = proc
                .links
                .work_instructions
                .iter()
                .map(|wi_id| WorkInstructionRef {
                    id: wi_id.to_string(),
                    revision: None,
                })
                .collect();
        }
    }

    // Handle signing
    if args.sign {
        lot.execution[step_idx].signature_verified = Some(true);
        // Note: signing_key would be populated by git commit signing
    }

    // Increment revision
    lot.entity_revision += 1;

    // Write updated lot
    let yaml_content = serde_yml::to_string(&lot).into_diagnostic()?;
    fs::write(&path, &yaml_content).into_diagnostic()?;

    // Create git commit if lot has a branch and not --no-commit
    let mut commit_sha: Option<String> = None;
    if !args.no_commit && lot.git_branch.is_some() {
        let git = Git::new(project.root());
        if git.is_repo() {
            let workflow_config = LotWorkflowConfig::from_config(&config);
            let workflow = LotWorkflow::new(&git, workflow_config);

            match workflow.commit_step_completion(
                &lot,
                step_idx,
                &operator,
                &[path.as_path()],
                args.sign,
            ) {
                Ok(sha) => {
                    commit_sha = Some(sha.clone());
                    // Update the step with commit SHA and re-save
                    lot.execution[step_idx].commit_sha = Some(sha);
                    let yaml_content = serde_yml::to_string(&lot).into_diagnostic()?;
                    fs::write(&path, &yaml_content).into_diagnostic()?;
                }
                Err(e) => {
                    eprintln!(
                        "{} Warning: Could not create commit: {}",
                        style("!").yellow(),
                        e
                    );
                }
            }
        }
    }

    // Output
    match global.output {
        OutputFormat::Json => {
            let mut result = serde_json::json!({
                "lot": lot.id.to_string(),
                "step": step_idx + 1,
                "status": new_status.to_string(),
                "operator": operator,
            });
            if let Some(ref sha) = commit_sha {
                result["commit_sha"] = serde_json::json!(sha);
            }
            if args.sign {
                result["signed"] = serde_json::json!(true);
            }
            println!(
                "{}",
                serde_json::to_string_pretty(&result).unwrap_or_default()
            );
        }
        OutputFormat::Yaml => {
            let mut result = serde_json::json!({
                "lot": lot.id.to_string(),
                "step": step_idx + 1,
                "status": new_status.to_string(),
            });
            if let Some(ref sha) = commit_sha {
                result["commit_sha"] = serde_json::json!(sha);
            }
            println!("{}", serde_yml::to_string(&result).unwrap_or_default());
        }
        _ => {
            let default_step = format!("Step {}", step_idx + 1);
            let step_desc = lot.execution[step_idx]
                .process
                .as_deref()
                .unwrap_or(&default_step);
            println!(
                "{} Updated {} step {} to {}",
                style("✓").green(),
                style(&display_id).cyan(),
                style(step_desc).yellow(),
                style(new_status.to_string()).green()
            );
            if args.sign {
                println!("   {} Signed", style("✓").green());
            }
            if let Some(ref sha) = commit_sha {
                println!(
                    "   {} Commit: {}",
                    style("→").dim(),
                    style(&sha[..8]).cyan()
                );
            }
        }
    }

    // Sync cache after mutation
    super::utils::sync_cache(&project);

    Ok(())
}

fn run_complete(args: CompleteArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;

    // Resolve short ID if needed
    let short_ids = ShortIdIndex::load(&project);
    let resolved_id = short_ids
        .resolve(&args.lot)
        .unwrap_or_else(|| args.lot.clone());

    // Find the lot file
    let lot_dir = project.root().join("manufacturing/lots");
    let mut found_path = None;

    if lot_dir.exists() {
        for entry in fs::read_dir(&lot_dir).into_diagnostic()? {
            let entry = entry.into_diagnostic()?;
            let path = entry.path();

            if path.extension().is_some_and(|e| e == "yaml") {
                let filename = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                if filename.contains(&resolved_id) || filename.starts_with(&resolved_id) {
                    found_path = Some(path);
                    break;
                }
            }
        }
    }

    let path = found_path.ok_or_else(|| miette::miette!("No lot found matching '{}'", args.lot))?;

    // Read and parse lot
    let content = fs::read_to_string(&path).into_diagnostic()?;
    let mut lot: Lot = serde_yml::from_str(&content).into_diagnostic()?;

    // Get display ID
    let display_id = short_ids
        .get_short_id(&lot.id.to_string())
        .unwrap_or_else(|| format_short_id(&lot.id));

    // Check status
    if lot.lot_status == LotStatus::Completed {
        return Err(miette::miette!("Lot {} is already completed", display_id));
    }
    if lot.lot_status == LotStatus::Scrapped {
        return Err(miette::miette!(
            "Lot {} is scrapped and cannot be completed",
            display_id
        ));
    }

    // Check for incomplete steps
    let incomplete_steps: Vec<_> = lot
        .execution
        .iter()
        .enumerate()
        .filter(|(_, s)| {
            s.status != ExecutionStatus::Completed && s.status != ExecutionStatus::Skipped
        })
        .collect();

    if !incomplete_steps.is_empty() && !args.yes {
        println!();
        println!("{}", style("Warning: Incomplete steps").yellow().bold());
        for (i, step) in &incomplete_steps {
            println!(
                "  {}. {} [{}]",
                i + 1,
                step.process.as_deref().unwrap_or("(unlinked)"),
                step.status
            );
        }
        println!();
        print!("Complete lot anyway? [y/N] ");
        std::io::Write::flush(&mut std::io::stdout()).into_diagnostic()?;
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).into_diagnostic()?;
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Cancelled.");
            return Ok(());
        }
    }

    // Load processes to check signature requirements
    let config = Config::load();
    let mut processes: HashMap<String, Process> = HashMap::new();
    let proc_dir = project.root().join("manufacturing/processes");
    if proc_dir.exists() {
        for entry in walkdir::WalkDir::new(&proc_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        {
            if let Ok(content) = fs::read_to_string(entry.path()) {
                if let Ok(proc) = serde_yml::from_str::<Process>(&content) {
                    processes.insert(proc.id.to_string(), proc);
                }
            }
        }
    }

    // Validate signature requirements
    let unsigned_required: Vec<_> = lot
        .execution
        .iter()
        .enumerate()
        .filter(|(_, step)| {
            step.status == ExecutionStatus::Completed
                && step_requires_signature(step, &processes)
                && step.signature_verified != Some(true)
        })
        .collect();

    if !unsigned_required.is_empty() {
        println!();
        println!(
            "{}",
            style("Error: Steps require signature but are not signed")
                .red()
                .bold()
        );
        for (i, step) in &unsigned_required {
            println!(
                "  {}. {} - requires signature",
                i + 1,
                step.process.as_deref().unwrap_or("(unlinked)")
            );
        }
        return Err(miette::miette!(
            "Cannot complete lot: {} step(s) require signature verification",
            unsigned_required.len()
        ));
    }

    // Check for pending approvals on WI steps
    let mut pending_approvals: Vec<(usize, &str, u32)> = Vec::new();
    for (i, step) in lot.execution.iter().enumerate() {
        for wi_exec in &step.wi_step_executions {
            if wi_exec.approval_status == ApprovalStatus::Pending {
                let wi_display = short_ids
                    .get_short_id(&wi_exec.work_instruction)
                    .unwrap_or_else(|| wi_exec.work_instruction.clone());
                pending_approvals.push((i + 1, wi_display.leak(), wi_exec.step_number));
            }
        }
    }

    if !pending_approvals.is_empty() && !args.yes {
        println!();
        println!(
            "{}",
            style("Warning: Pending approvals on WI steps")
                .yellow()
                .bold()
        );
        for (proc_step, wi_id, step_num) in &pending_approvals {
            println!(
                "  Process step {} | {} step {} - awaiting approval",
                proc_step, wi_id, step_num
            );
        }
        println!();
        println!(
            "Approve these steps first with: tdt lot approve {} --wi <WI> --step <N>",
            display_id
        );
        println!();
        print!("Complete lot with unapproved steps? [y/N] ");
        std::io::Write::flush(&mut std::io::stdout()).into_diagnostic()?;
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).into_diagnostic()?;
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Cancelled.");
            return Ok(());
        }
    }

    // Update lot
    lot.lot_status = LotStatus::Completed;
    lot.completion_date = Some(chrono::Local::now().date_naive());
    lot.entity_revision += 1;

    // Handle git branch merge if lot has a branch
    let mut merge_sha: Option<String> = None;
    if !args.no_merge && lot.git_branch.is_some() {
        let git = Git::new(project.root());
        if git.is_repo() {
            let workflow_config = LotWorkflowConfig::from_config(&config);
            let workflow = LotWorkflow::new(&git, workflow_config);

            match workflow.complete_lot(&lot, args.sign) {
                Ok(sha) => {
                    merge_sha = Some(sha);
                    lot.branch_merged = true;
                }
                Err(e) => {
                    eprintln!(
                        "{} Warning: Could not merge lot branch: {}",
                        style("!").yellow(),
                        e
                    );
                }
            }
        }
    }

    // Write updated lot
    let yaml_content = serde_yml::to_string(&lot).into_diagnostic()?;
    fs::write(&path, &yaml_content).into_diagnostic()?;

    // Output
    match global.output {
        OutputFormat::Json => {
            let mut result = serde_json::json!({
                "id": lot.id.to_string(),
                "short_id": display_id,
                "lot_status": "completed",
                "completion_date": lot.completion_date,
            });
            if let Some(ref sha) = merge_sha {
                result["merge_sha"] = serde_json::json!(sha);
            }
            if lot.branch_merged {
                result["branch_merged"] = serde_json::json!(true);
            }
            println!(
                "{}",
                serde_json::to_string_pretty(&result).unwrap_or_default()
            );
        }
        OutputFormat::Yaml => {
            let mut result = serde_json::json!({
                "id": lot.id.to_string(),
                "lot_status": "completed",
            });
            if let Some(ref sha) = merge_sha {
                result["merge_sha"] = serde_json::json!(sha);
            }
            println!("{}", serde_yml::to_string(&result).unwrap_or_default());
        }
        _ => {
            println!(
                "{} Lot {} completed",
                style("✓").green(),
                style(&display_id).cyan()
            );
            if let Some(date) = lot.completion_date {
                println!("   Completion date: {}", date);
            }
            if let Some(ref sha) = merge_sha {
                println!(
                    "   {} Branch merged: {}",
                    style("✓").green(),
                    style(&sha[..8]).cyan()
                );
            }
            if args.sign {
                println!("   {} Signed", style("✓").green());
            }
        }
    }

    // Sync cache after mutation
    super::utils::sync_cache(&project);

    Ok(())
}

/// Execute or show status of a WI step (electronic router)
fn run_wi_step(args: WiStepArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let config = Config::load();
    let cache = EntityCache::open(&project).map_err(|e| miette::miette!("{}", e))?;

    // Resolve short IDs
    let short_ids = ShortIdIndex::load(&project);
    let resolved_lot_id = short_ids
        .resolve(&args.lot)
        .unwrap_or_else(|| args.lot.clone());
    let resolved_wi_id = short_ids
        .resolve(&args.wi)
        .unwrap_or_else(|| args.wi.clone());

    let service = LotService::new(&project, &cache);

    // Show mode — read-only via service
    if args.show {
        let proc_idx = args.process.map(|p| p.saturating_sub(1));
        let lot = service
            .get(&resolved_lot_id)
            .map_err(|e| miette::miette!("{}", e))?
            .ok_or_else(|| miette::miette!("No lot found matching '{}'", args.lot))?;

        let display_id = short_ids
            .get_short_id(&lot.id.to_string())
            .unwrap_or_else(|| format_short_id(&lot.id));

        // Resolve process index
        let proc_idx = proc_idx.unwrap_or_else(|| {
            lot.execution
                .iter()
                .position(|step| {
                    step.work_instructions_used
                        .iter()
                        .any(|wi| wi.id == resolved_wi_id)
                })
                .unwrap_or(0)
        });

        let wi_exec = service
            .get_wi_step_status(&resolved_lot_id, proc_idx, &resolved_wi_id, args.step)
            .map_err(|e| miette::miette!("{}", e))?;

        match global.output {
            OutputFormat::Json | OutputFormat::Yaml => {
                let result = serde_json::json!({
                    "lot": lot.id.to_string(),
                    "process_step": proc_idx + 1,
                    "work_instruction": resolved_wi_id,
                    "step_number": args.step,
                    "execution": wi_exec,
                });
                if global.output == OutputFormat::Json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&result).unwrap_or_default()
                    );
                } else {
                    println!("{}", serde_yml::to_string(&result).unwrap_or_default());
                }
            }
            _ => {
                println!("{}", style("─".repeat(60)).dim());
                println!(
                    "{}: {} step {}",
                    style("WI Step").bold(),
                    style(&resolved_wi_id).cyan(),
                    style(args.step).yellow()
                );
                println!("{}: {}", style("Lot").bold(), style(&display_id).cyan());
                println!("{}: {}", style("Process Step").bold(), proc_idx + 1);

                if let Some(exec) = wi_exec {
                    println!(
                        "{}: {}",
                        style("Status").bold(),
                        if exec.is_completed() {
                            style("Completed").green()
                        } else {
                            style("Pending").yellow()
                        }
                    );
                    if let Some(ref op) = exec.operator {
                        println!("{}: {}", style("Operator").bold(), op);
                    }
                    if let Some(ref ts) = exec.completed_at {
                        println!("{}: {}", style("Completed At").bold(), ts);
                    }
                    if exec.operator_signature_verified == Some(true) {
                        println!("{}: {}", style("Signed").bold(), style("Yes").green());
                    }
                    let approval_styled = match exec.approval_status {
                        ApprovalStatus::NotRequired => style("Not Required").dim(),
                        ApprovalStatus::Pending => style("Pending").yellow(),
                        ApprovalStatus::Approved => style("Approved").green(),
                        ApprovalStatus::Rejected => style("Rejected").red(),
                    };
                    println!("{}: {}", style("Approval").bold(), approval_styled);
                    if !exec.data.is_empty() {
                        println!("{}: {:?}", style("Data").bold(), exec.data);
                    }
                    if !exec.equipment_used.is_empty() {
                        println!("{}: {:?}", style("Equipment").bold(), exec.equipment_used);
                    }
                } else {
                    println!("{}: {}", style("Status").bold(), style("Not Started").dim());
                }
                println!("{}", style("─".repeat(60)).dim());
            }
        }
        return Ok(());
    }

    // Execute mode — mutate via service
    let operator = args.operator.unwrap_or_else(|| config.author().to_string());

    // Resolve deviation short ID if provided
    let deviation_id = args
        .deviation
        .as_ref()
        .map(|dev_id| short_ids.resolve(dev_id).unwrap_or_else(|| dev_id.clone()));

    if deviation_id.is_some() && !global.quiet {
        eprintln!(
            "{} Using deviation {} to bypass step order enforcement",
            style("!").yellow(),
            style(args.deviation.as_deref().unwrap_or("")).cyan()
        );
    }

    // Build data map with type coercion
    let mut data = std::collections::HashMap::new();
    for (key, value) in &args.data {
        let json_val = if let Ok(num) = value.parse::<f64>() {
            serde_json::json!(num)
        } else if let Ok(bool_val) = value.parse::<bool>() {
            serde_json::json!(bool_val)
        } else {
            serde_json::json!(value)
        };
        data.insert(key.clone(), json_val);
    }

    let equipment: std::collections::HashMap<String, String> = args
        .equipment
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();

    let input = tdt_core::services::ExecuteWiStepInput {
        work_instruction_id: resolved_wi_id.clone(),
        step_number: args.step,
        process_index: args.process.map(|p| p.saturating_sub(1)),
        operator: operator.clone(),
        operator_email: get_git_email(),
        data,
        equipment,
        notes: args.notes.clone(),
        sign: args.sign,
        require_approval: args.require_approval,
        complete: args.complete,
        deviation_id,
    };

    let result = service
        .execute_wi_step(&resolved_lot_id, input)
        .map_err(|e| miette::miette!("{}", e))?;

    let display_id = short_ids
        .get_short_id(&result.lot.id.to_string())
        .unwrap_or_else(|| format_short_id(&result.lot.id));

    // Output
    let status = if args.complete {
        "completed"
    } else {
        "updated"
    };
    match global.output {
        OutputFormat::Json => {
            let json_result = serde_json::json!({
                "lot": result.lot.id.to_string(),
                "work_instruction": resolved_wi_id,
                "step_number": args.step,
                "operator": operator,
                "signed": args.sign,
                "completed": args.complete,
            });
            println!(
                "{}",
                serde_json::to_string_pretty(&json_result).unwrap_or_default()
            );
        }
        OutputFormat::Yaml => {
            let yaml_result = serde_json::json!({
                "lot": result.lot.id.to_string(),
                "step": args.step,
                "status": status,
            });
            println!("{}", serde_yml::to_string(&yaml_result).unwrap_or_default());
        }
        _ => {
            if args.complete {
                println!(
                    "{} Completed WI {} step {}",
                    style("✓").green(),
                    style(&resolved_wi_id).cyan(),
                    style(args.step).yellow()
                );
            } else {
                println!(
                    "{} Updated WI {} step {}",
                    style("•").blue(),
                    style(&resolved_wi_id).cyan(),
                    style(args.step).yellow()
                );
            }
            println!("   Lot: {}", style(&display_id).cyan());
            println!("   Operator: {}", operator);
            if args.sign {
                println!("   {} Signed", style("✓").green());
            }
            if args.require_approval {
                println!("   {} Approval Required", style("→").dim());
            }
            if !args.data.is_empty() {
                println!("   Data: {} field(s) recorded", args.data.len());
                for (key, value) in &args.data {
                    println!("     {} = {}", style(key).dim(), value);
                }
            }
            if !args.equipment.is_empty() {
                println!("   Equipment: {} item(s) logged", args.equipment.len());
            }
        }
    }

    // Sync cache
    super::utils::sync_cache(&project);

    Ok(())
}

/// View electronic router/traveler status
fn run_router(args: RouterArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let cache = EntityCache::open(&project).map_err(|e| miette::miette!("{}", e))?;

    // Resolve short ID
    let short_ids = ShortIdIndex::load(&project);
    let resolved_id = short_ids
        .resolve(&args.lot)
        .unwrap_or_else(|| args.lot.clone());

    // Load lot using service
    let service = LotService::new(&project, &cache);
    let lot = service
        .get(&resolved_id)
        .map_err(|e| miette::miette!("{}", e))?
        .ok_or_else(|| miette::miette!("No lot found matching '{}'", args.lot))?;

    let display_id = short_ids
        .get_short_id(&lot.id.to_string())
        .unwrap_or_else(|| format_short_id(&lot.id));

    // Resolve WI filter
    let wi_filter = args
        .wi
        .as_ref()
        .map(|wi| short_ids.resolve(wi).unwrap_or_else(|| wi.clone()));

    match global.output {
        OutputFormat::Json => {
            let mut router_data = serde_json::json!({
                "lot": lot.id.to_string(),
                "lot_number": lot.lot_number,
                "lot_status": lot.lot_status.to_string(),
                "processes": [],
            });

            let processes = router_data["processes"].as_array_mut().unwrap();
            for (idx, exec_step) in lot.execution.iter().enumerate() {
                if args.process.is_some() && args.process != Some(idx + 1) {
                    continue;
                }

                let mut proc_data = serde_json::json!({
                    "index": idx + 1,
                    "process": exec_step.process,
                    "status": exec_step.status.to_string(),
                    "wi_steps": [],
                });

                let wi_steps = proc_data["wi_steps"].as_array_mut().unwrap();
                for wi_exec in &exec_step.wi_step_executions {
                    if let Some(ref filter) = wi_filter {
                        if !wi_exec.work_instruction.contains(filter) {
                            continue;
                        }
                    }

                    if args.pending && wi_exec.is_completed() {
                        continue;
                    }

                    if args.approval_needed && wi_exec.approval_status != ApprovalStatus::Pending {
                        continue;
                    }

                    wi_steps.push(serde_json::json!({
                        "work_instruction": wi_exec.work_instruction,
                        "step_number": wi_exec.step_number,
                        "operator": wi_exec.operator,
                        "completed_at": wi_exec.completed_at,
                        "signed": wi_exec.operator_signature_verified.unwrap_or(false),
                        "approval_status": wi_exec.approval_status.to_string(),
                        "data": wi_exec.data,
                    }));
                }

                processes.push(proc_data);
            }

            println!(
                "{}",
                serde_json::to_string_pretty(&router_data).unwrap_or_default()
            );
        }
        OutputFormat::Yaml => {
            let yaml = serde_yml::to_string(&lot.execution).into_diagnostic()?;
            print!("{}", yaml);
        }
        _ => {
            // Pretty table format
            println!();
            println!(
                "{} {} - Electronic Router",
                style("LOT").bold().cyan(),
                style(&display_id).cyan()
            );
            if let Some(ref ln) = lot.lot_number {
                println!("Lot Number: {}", style(ln).yellow());
            }
            println!(
                "Status: {}",
                match lot.lot_status {
                    LotStatus::InProgress => style(lot.lot_status.to_string()).green(),
                    LotStatus::OnHold => style(lot.lot_status.to_string()).yellow(),
                    LotStatus::Completed => style(lot.lot_status.to_string()).cyan(),
                    LotStatus::Scrapped => style(lot.lot_status.to_string()).red(),
                }
            );
            println!("{}", style("═".repeat(70)).dim());

            for (idx, exec_step) in lot.execution.iter().enumerate() {
                if args.process.is_some() && args.process != Some(idx + 1) {
                    continue;
                }

                let proc_name = exec_step.process.as_deref().unwrap_or("(unlinked)");
                let proc_short = short_ids
                    .get_short_id(proc_name)
                    .unwrap_or_else(|| proc_name.to_string());

                println!();
                println!(
                    "{} {} - {}",
                    style(format!("Process {}:", idx + 1)).bold(),
                    style(&proc_short).cyan(),
                    match exec_step.status {
                        ExecutionStatus::Pending => style("Pending").dim(),
                        ExecutionStatus::InProgress => style("In Progress").yellow(),
                        ExecutionStatus::Completed => style("Completed").green(),
                        ExecutionStatus::Skipped => style("Skipped").dim(),
                    }
                );
                println!("{}", style("─".repeat(70)).dim());

                // Show WI steps
                if exec_step.wi_step_executions.is_empty() {
                    println!("   {} No WI steps recorded", style("○").dim());
                } else {
                    for wi_exec in &exec_step.wi_step_executions {
                        if let Some(ref filter) = wi_filter {
                            if !wi_exec.work_instruction.contains(filter) {
                                continue;
                            }
                        }

                        if args.pending && wi_exec.is_completed() {
                            continue;
                        }

                        if args.approval_needed
                            && wi_exec.approval_status != ApprovalStatus::Pending
                        {
                            continue;
                        }

                        let wi_short = short_ids
                            .get_short_id(&wi_exec.work_instruction)
                            .unwrap_or_else(|| wi_exec.work_instruction.clone());

                        let status_icon = if wi_exec.is_completed() {
                            if wi_exec.approval_status == ApprovalStatus::Pending {
                                style("⏳").yellow()
                            } else if wi_exec.approval_status == ApprovalStatus::Approved {
                                style("✅").green()
                            } else {
                                style("✓").green()
                            }
                        } else {
                            style("○").dim()
                        };

                        print!(
                            "   {} Step {:2} | {} ",
                            status_icon,
                            wi_exec.step_number,
                            style(&wi_short).cyan()
                        );

                        if let Some(ref op) = wi_exec.operator {
                            print!("| {} ", op);
                        }

                        if let Some(ref ts) = wi_exec.completed_at {
                            print!("| {} ", ts.format("%Y-%m-%d %H:%M"));
                        }

                        if wi_exec.operator_signature_verified == Some(true) {
                            print!("| {} ", style("SIGNED").green());
                        }

                        println!();

                        // Show approval status if pending
                        if wi_exec.approval_status == ApprovalStatus::Pending {
                            println!(
                                "         {} {}",
                                style("└─").dim(),
                                style("Awaiting approval").yellow()
                            );
                        } else if wi_exec.approval_status == ApprovalStatus::Rejected {
                            println!("         {} {}", style("└─").dim(), style("REJECTED").red());
                        }

                        // Show data if present
                        if !wi_exec.data.is_empty() {
                            let data_summary: Vec<String> = wi_exec
                                .data
                                .iter()
                                .take(3)
                                .map(|(k, v)| format!("{}={}", k, v))
                                .collect();
                            let more = if wi_exec.data.len() > 3 {
                                format!(" (+{})", wi_exec.data.len() - 3)
                            } else {
                                String::new()
                            };
                            println!(
                                "         {} {}{}",
                                style("└─").dim(),
                                style(data_summary.join(", ")).dim(),
                                more
                            );
                        }
                    }
                }
            }

            println!();
            println!("{}", style("═".repeat(70)).dim());

            // Summary
            let total_wi_steps: usize = lot
                .execution
                .iter()
                .map(|e| e.wi_step_executions.len())
                .sum();
            let completed_wi_steps: usize = lot
                .execution
                .iter()
                .flat_map(|e| &e.wi_step_executions)
                .filter(|w| w.is_completed())
                .count();
            let pending_approvals: usize = lot
                .execution
                .iter()
                .flat_map(|e| &e.wi_step_executions)
                .filter(|w| w.approval_status == ApprovalStatus::Pending)
                .count();

            println!(
                "WI Steps: {}/{} completed",
                style(completed_wi_steps).cyan(),
                total_wi_steps
            );
            if pending_approvals > 0 {
                println!("Pending Approvals: {}", style(pending_approvals).yellow());
            }
        }
    }

    Ok(())
}

/// Approve or reject a WI step
fn run_approve(args: ApproveArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let config = Config::load();
    let cache = EntityCache::open(&project).map_err(|e| miette::miette!("{}", e))?;

    // Resolve short IDs
    let short_ids = ShortIdIndex::load(&project);
    let resolved_lot_id = short_ids
        .resolve(&args.lot)
        .unwrap_or_else(|| args.lot.clone());
    let resolved_wi_id = short_ids
        .resolve(&args.wi)
        .unwrap_or_else(|| args.wi.clone());

    // Load lot using service (with workflow guard for authorization)
    let guard = tdt_core::services::WorkflowGuard::load(&project);
    let service = LotService::new(&project, &cache).with_workflow(guard);
    let mut lot = service
        .get(&resolved_lot_id)
        .map_err(|e| miette::miette!("{}", e))?
        .ok_or_else(|| miette::miette!("No lot found matching '{}'", args.lot))?;

    let display_id = short_ids
        .get_short_id(&lot.id.to_string())
        .unwrap_or_else(|| format_short_id(&lot.id));

    // Show pending mode
    if args.show_pending {
        let pending: Vec<_> = lot
            .execution
            .iter()
            .enumerate()
            .flat_map(|(idx, step)| {
                step.wi_step_executions
                    .iter()
                    .filter(|w| w.approval_status == ApprovalStatus::Pending)
                    .map(move |w| (idx + 1, w))
            })
            .collect();

        match global.output {
            OutputFormat::Json => {
                let result: Vec<_> = pending
                    .iter()
                    .map(|(proc_idx, w)| {
                        serde_json::json!({
                            "process_step": proc_idx,
                            "work_instruction": w.work_instruction,
                            "step_number": w.step_number,
                            "operator": w.operator,
                            "completed_at": w.completed_at,
                        })
                    })
                    .collect();
                println!(
                    "{}",
                    serde_json::to_string_pretty(&result).unwrap_or_default()
                );
            }
            _ => {
                if pending.is_empty() {
                    println!("No pending approvals for lot {}", style(&display_id).cyan());
                } else {
                    println!(
                        "{} pending approval(s) for lot {}",
                        style(pending.len()).yellow(),
                        style(&display_id).cyan()
                    );
                    println!();
                    for (proc_idx, w) in &pending {
                        let wi_short = short_ids
                            .get_short_id(&w.work_instruction)
                            .unwrap_or_else(|| w.work_instruction.clone());
                        println!(
                            "  {} Process {} | {} step {} | by {}",
                            style("→").dim(),
                            proc_idx,
                            style(&wi_short).cyan(),
                            w.step_number,
                            w.operator.as_deref().unwrap_or("?")
                        );
                    }
                }
            }
        }
        return Ok(());
    }

    // Find the process step
    let proc_idx = if let Some(idx) = args.process {
        idx.saturating_sub(1)
    } else {
        lot.execution
            .iter()
            .position(|step| {
                step.wi_step_executions.iter().any(|w| {
                    (w.work_instruction == resolved_wi_id
                        || w.work_instruction.contains(&resolved_wi_id))
                        && w.step_number == args.step
                        && w.approval_status == ApprovalStatus::Pending
                })
            })
            .ok_or_else(|| {
                miette::miette!(
                    "No pending approval found for WI {} step {} in lot {}",
                    args.wi,
                    args.step,
                    display_id
                )
            })?
    };

    if proc_idx >= lot.execution.len() {
        return Err(miette::miette!("Process step not found"));
    }

    // Find the WI step execution
    let exec_step = &mut lot.execution[proc_idx];
    let wi_exec_idx = exec_step
        .wi_step_executions
        .iter()
        .position(|w| {
            (w.work_instruction == resolved_wi_id || w.work_instruction.contains(&resolved_wi_id))
                && w.step_number == args.step
        })
        .ok_or_else(|| {
            miette::miette!(
                "WI step {} step {} not found in process step {}",
                args.wi,
                args.step,
                proc_idx + 1
            )
        })?;

    let wi_exec = &mut exec_step.wi_step_executions[wi_exec_idx];

    // Check if already approved/rejected
    if wi_exec.approval_status == ApprovalStatus::Approved && !args.reject {
        return Err(miette::miette!("This step is already approved"));
    }
    if wi_exec.approval_status == ApprovalStatus::Rejected && args.reject {
        return Err(miette::miette!("This step is already rejected"));
    }

    // Create approval record
    let approval = StepApproval {
        approver: config.author().to_string(),
        email: get_git_email(),
        role: args.role.clone(),
        timestamp: chrono::Utc::now(),
        comment: args.comment.clone(),
        signature_verified: if args.sign { Some(true) } else { None },
        signing_key: None, // Would be set by git commit signing
    };

    // Update status
    if args.reject {
        wi_exec.approval_status = ApprovalStatus::Rejected;
    } else {
        wi_exec.approval_status = ApprovalStatus::Approved;
    }
    wi_exec.approvals.push(approval);

    // Increment lot revision
    lot.entity_revision += 1;

    // Save lot
    let lot_path = project
        .root()
        .join(format!("manufacturing/lots/{}.tdt.yaml", lot.id));
    let yaml_content = serde_yml::to_string(&lot).into_diagnostic()?;
    fs::write(&lot_path, &yaml_content).into_diagnostic()?;

    // Output
    let action = if args.reject { "Rejected" } else { "Approved" };
    match global.output {
        OutputFormat::Json => {
            let result = serde_json::json!({
                "lot": lot.id.to_string(),
                "work_instruction": resolved_wi_id,
                "step_number": args.step,
                "action": action.to_lowercase(),
                "approver": config.author(),
                "signed": args.sign,
            });
            println!(
                "{}",
                serde_json::to_string_pretty(&result).unwrap_or_default()
            );
        }
        OutputFormat::Yaml => {
            let result = serde_json::json!({
                "lot": lot.id.to_string(),
                "step": args.step,
                "action": action.to_lowercase(),
            });
            println!("{}", serde_yml::to_string(&result).unwrap_or_default());
        }
        _ => {
            let action_styled = if args.reject {
                style(action).red()
            } else {
                style(action).green()
            };
            println!(
                "{} {} WI {} step {}",
                style("✓").green(),
                action_styled,
                style(&resolved_wi_id).cyan(),
                style(args.step).yellow()
            );
            println!("   Lot: {}", style(&display_id).cyan());
            println!("   Approver: {}", config.author());
            if let Some(ref role) = args.role {
                println!("   Role: {}", role);
            }
            if args.sign {
                println!("   {} Signed", style("✓").green());
            }
            if let Some(ref comment) = args.comment {
                println!("   Comment: {}", comment);
            }
        }
    }

    // Sync cache
    super::utils::sync_cache(&project);

    Ok(())
}
