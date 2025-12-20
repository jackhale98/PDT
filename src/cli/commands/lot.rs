//! `tdt lot` command - Production lot / batch (DHR) management

use clap::{Subcommand, ValueEnum};
use console::style;
use miette::{IntoDiagnostic, Result};
use std::fs;

use crate::cli::helpers::{escape_csv, format_short_id, truncate_str};
use crate::cli::{GlobalOpts, OutputFormat};
use crate::core::identity::{EntityId, EntityPrefix};
use crate::core::project::Project;
use crate::core::shortid::ShortIdIndex;
use crate::core::Config;
use crate::entities::lot::{ExecutionStatus, Lot, LotStatus};
use crate::schema::template::{TemplateContext, TemplateGenerator};
use crate::schema::wizard::SchemaWizard;

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

/// List column for display and sorting
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ListColumn {
    Id,
    Title,
    LotNumber,
    Quantity,
    LotStatus,
    Author,
    Created,
}

impl std::fmt::Display for ListColumn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ListColumn::Id => write!(f, "id"),
            ListColumn::Title => write!(f, "title"),
            ListColumn::LotNumber => write!(f, "lot-number"),
            ListColumn::Quantity => write!(f, "quantity"),
            ListColumn::LotStatus => write!(f, "lot-status"),
            ListColumn::Author => write!(f, "author"),
            ListColumn::Created => write!(f, "created"),
        }
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
        ListColumn::Id,
        ListColumn::Title,
        ListColumn::LotNumber,
        ListColumn::Quantity,
        ListColumn::LotStatus
    ])]
    pub columns: Vec<ListColumn>,

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
    #[arg(long, short = 'o')]
    pub operator: Option<String>,

    /// Notes about the step execution
    #[arg(long, short = 'n')]
    pub notes: Option<String>,

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
}

/// Directories where lots are stored
const LOT_DIRS: &[&str] = &["manufacturing/lots"];

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
    }
}

fn run_list(args: ListArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let lot_dir = project.root().join("manufacturing/lots");

    if !lot_dir.exists() {
        if args.count {
            println!("0");
        } else {
            println!("No lots found.");
        }
        return Ok(());
    }

    let format = match global.format {
        OutputFormat::Auto => OutputFormat::Tsv,
        f => f,
    };

    // Load from files
    let mut lots: Vec<Lot> = Vec::new();

    for entry in fs::read_dir(&lot_dir).into_diagnostic()? {
        let entry = entry.into_diagnostic()?;
        let path = entry.path();

        if path.extension().is_some_and(|e| e == "yaml") {
            let content = fs::read_to_string(&path).into_diagnostic()?;
            if let Ok(lot) = serde_yml::from_str::<Lot>(&content) {
                lots.push(lot);
            }
        }
    }

    // Apply filters
    let lots: Vec<Lot> = lots
        .into_iter()
        .filter(|l| match args.status {
            LotStatusFilter::InProgress => l.lot_status == LotStatus::InProgress,
            LotStatusFilter::OnHold => l.lot_status == LotStatus::OnHold,
            LotStatusFilter::Completed => l.lot_status == LotStatus::Completed,
            LotStatusFilter::Scrapped => l.lot_status == LotStatus::Scrapped,
            LotStatusFilter::All => true,
        })
        .filter(|l| {
            if let Some(ref author) = args.author {
                l.author.to_lowercase().contains(&author.to_lowercase())
            } else {
                true
            }
        })
        .filter(|l| {
            if let Some(ref product) = args.product {
                l.links
                    .product
                    .as_ref()
                    .is_some_and(|p| p.contains(product))
            } else {
                true
            }
        })
        .filter(|l| {
            if args.recent {
                let thirty_days_ago = chrono::Utc::now() - chrono::Duration::days(30);
                l.created >= thirty_days_ago
            } else {
                true
            }
        })
        .filter(|l| {
            if let Some(ref search) = args.search {
                let search_lower = search.to_lowercase();
                l.title.to_lowercase().contains(&search_lower)
                    || l.lot_number
                        .as_ref()
                        .is_some_and(|n| n.to_lowercase().contains(&search_lower))
            } else {
                true
            }
        })
        .filter(|l| {
            if args.active {
                l.lot_status == LotStatus::InProgress || l.lot_status == LotStatus::OnHold
            } else {
                true
            }
        })
        .collect();

    // Sort
    let mut lots = lots;
    match args.sort {
        ListColumn::Id => lots.sort_by(|a, b| a.id.to_string().cmp(&b.id.to_string())),
        ListColumn::Title => lots.sort_by(|a, b| a.title.cmp(&b.title)),
        ListColumn::LotNumber => lots.sort_by(|a, b| {
            a.lot_number
                .as_deref()
                .unwrap_or("")
                .cmp(b.lot_number.as_deref().unwrap_or(""))
        }),
        ListColumn::Quantity => lots.sort_by(|a, b| a.quantity.cmp(&b.quantity)),
        ListColumn::LotStatus => {
            lots.sort_by(|a, b| format!("{:?}", a.lot_status).cmp(&format!("{:?}", b.lot_status)))
        }
        ListColumn::Author => lots.sort_by(|a, b| a.author.cmp(&b.author)),
        ListColumn::Created => lots.sort_by(|a, b| a.created.cmp(&b.created)),
    }

    if args.reverse {
        lots.reverse();
    }

    // Apply limit
    if let Some(limit) = args.limit {
        lots.truncate(limit);
    }

    // Count only
    if args.count {
        println!("{}", lots.len());
        return Ok(());
    }

    // No results
    if lots.is_empty() {
        println!("No lots found.");
        return Ok(());
    }

    // Update short ID index
    let mut short_ids = ShortIdIndex::load(&project);
    short_ids.ensure_all(lots.iter().map(|l| l.id.to_string()));
    super::utils::save_short_ids(&mut short_ids, &project);

    // Output based on format
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
            for lot in &lots {
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
        OutputFormat::Tsv => {
            // Build header
            let mut headers = vec![];
            let mut widths = vec![];

            for col in &args.columns {
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
            for lot in &lots {
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
                                LotStatus::InProgress => {
                                    style(lot.lot_status.to_string()).green()
                                }
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
            for lot in &lots {
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
            for lot in &lots {
                let short_id = short_ids
                    .get_short_id(&lot.id.to_string())
                    .unwrap_or_default();
                println!(
                    "| {} | {} | {} | {} | {} | {} | {} |",
                    short_id,
                    format_short_id(&lot.id),
                    lot.title,
                    lot.lot_number.as_deref().unwrap_or("-"),
                    lot.quantity.map(|q| q.to_string()).unwrap_or("-".to_string()),
                    lot.lot_status,
                    lot.author
                );
            }
        }
        OutputFormat::Auto | OutputFormat::Path => unreachable!(),
    }

    Ok(())
}

fn run_new(args: NewArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let config = Config::load();

    let title: String;
    let lot_number: Option<String>;
    let quantity: Option<u32>;
    let product: Option<String>;

    if args.interactive {
        let wizard = SchemaWizard::new();
        let result = wizard.run(EntityPrefix::Lot)?;

        title = result
            .get_string("title")
            .map(String::from)
            .unwrap_or_else(|| "New Production Lot".to_string());
        lot_number = result.get_string("lot_number").map(String::from);
        quantity = result.get_string("quantity").and_then(|s| s.parse().ok());
        product = result.get_string("product").map(String::from);
    } else {
        title = args
            .title
            .unwrap_or_else(|| "New Production Lot".to_string());
        lot_number = args.lot_number;
        quantity = args.quantity;
        product = args.product;
    }

    // Generate ID
    let id = EntityId::new(EntityPrefix::Lot);

    // Generate template
    let generator = TemplateGenerator::new().map_err(|e| miette::miette!("{}", e))?;
    let mut ctx = TemplateContext::new(id.clone(), config.author()).with_title(&title);

    if let Some(ref ln) = lot_number {
        ctx = ctx.with_lot_number(ln);
    }
    if let Some(q) = quantity {
        ctx = ctx.with_quantity(q);
    }

    let mut yaml_content = generator
        .generate_lot(&ctx)
        .map_err(|e| miette::miette!("{}", e))?;

    // Add product link if provided
    if let Some(ref prod) = product {
        // Resolve short ID if needed
        let short_ids = ShortIdIndex::load(&project);
        let resolved = short_ids.resolve(prod).unwrap_or_else(|| prod.clone());
        yaml_content = yaml_content.replace(
            "  product: null",
            &format!("  product: \"{}\"", resolved),
        );
    }

    // Write file
    let output_dir = project.root().join("manufacturing/lots");
    if !output_dir.exists() {
        fs::create_dir_all(&output_dir).into_diagnostic()?;
    }

    let file_path = output_dir.join(format!("{}.tdt.yaml", id));
    fs::write(&file_path, &yaml_content).into_diagnostic()?;

    // Add to short ID index
    let mut short_ids = ShortIdIndex::load(&project);
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
    match global.format {
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
        }
    }

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

    // Resolve short ID if needed
    let short_ids = ShortIdIndex::load(&project);
    let resolved_id = short_ids
        .resolve(&args.id)
        .unwrap_or_else(|| args.id.clone());

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

    let path = found_path.ok_or_else(|| miette::miette!("No lot found matching '{}'", args.id))?;

    // Read and parse lot
    let content = fs::read_to_string(&path).into_diagnostic()?;
    let lot: Lot = serde_yml::from_str(&content).into_diagnostic()?;

    match global.format {
        OutputFormat::Yaml => {
            print!("{}", content);
        }
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&lot).into_diagnostic()?;
            println!("{}", json);
        }
        OutputFormat::Id | OutputFormat::ShortId => {
            if global.format == OutputFormat::ShortId {
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
                    let proc = step.process.as_deref().unwrap_or("(unlinked)");
                    let status_styled = match step.status {
                        ExecutionStatus::Pending => style("pending").dim(),
                        ExecutionStatus::InProgress => style("in_progress").yellow(),
                        ExecutionStatus::Completed => style("completed").green(),
                        ExecutionStatus::Skipped => style("skipped").dim(),
                    };
                    print!("  {}. {} [{}]", i + 1, proc, status_styled);
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
                        .get_short_id(prod)
                        .unwrap_or_else(|| prod.clone());
                    println!("  {}: {}", style("Product").dim(), style(&display).cyan());
                }
                if !lot.links.processes.is_empty() {
                    let proc_list: Vec<_> = lot
                        .links
                        .processes
                        .iter()
                        .map(|p| {
                            short_ids
                                .get_short_id(p)
                                .unwrap_or_else(|| p.clone())
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
                                .get_short_id(n)
                                .unwrap_or_else(|| n.clone())
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

    let path =
        found_path.ok_or_else(|| miette::miette!("No lot found matching '{}'", args.lot))?;

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
            .position(|s| s.process.as_ref().is_some_and(|p| p.contains(&resolved_proc)))
            .or_else(|| proc.parse::<usize>().ok().map(|i| i.saturating_sub(1)))
    } else {
        // Find first non-completed step
        lot.execution
            .iter()
            .position(|s| s.status == ExecutionStatus::Pending || s.status == ExecutionStatus::InProgress)
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

    // Interactive mode
    let new_status: ExecutionStatus;
    let operator: String;
    let notes: Option<String>;

    if args.interactive {
        println!();
        println!("{}", style("Update Execution Step").bold().cyan());
        println!("{}", style("─".repeat(50)).dim());
        println!(
            "Lot: {} \"{}\"",
            style(&display_id).cyan(),
            &lot.title
        );
        println!(
            "Step {}: {}",
            step_idx + 1,
            lot.execution[step_idx]
                .process
                .as_deref()
                .unwrap_or("(unlinked)")
        );
        println!(
            "Current Status: {}",
            lot.execution[step_idx].status
        );
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
        std::io::stdin().read_line(&mut op_input).into_diagnostic()?;
        operator = if op_input.trim().is_empty() {
            config.author().to_string()
        } else {
            op_input.trim().to_string()
        };

        print!("Notes (optional): ");
        std::io::Write::flush(&mut std::io::stdout()).into_diagnostic()?;
        let mut notes_input = String::new();
        std::io::stdin().read_line(&mut notes_input).into_diagnostic()?;
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
        operator = args
            .operator
            .unwrap_or_else(|| config.author().to_string());
        notes = args.notes;
    }

    // Update the step
    lot.execution[step_idx].status = new_status;
    lot.execution[step_idx].operator = Some(operator.clone());
    if new_status == ExecutionStatus::Completed {
        lot.execution[step_idx].completed_date = Some(chrono::Local::now().date_naive());
    }
    if let Some(ref n) = notes {
        lot.execution[step_idx].notes = Some(n.clone());
    }

    // Increment revision
    lot.entity_revision += 1;

    // Write updated lot
    let yaml_content = serde_yml::to_string(&lot).into_diagnostic()?;
    fs::write(&path, &yaml_content).into_diagnostic()?;

    // Output
    match global.format {
        OutputFormat::Json => {
            let result = serde_json::json!({
                "lot": lot.id.to_string(),
                "step": step_idx + 1,
                "status": new_status.to_string(),
                "operator": operator,
            });
            println!("{}", serde_json::to_string_pretty(&result).unwrap_or_default());
        }
        OutputFormat::Yaml => {
            let result = serde_json::json!({
                "lot": lot.id.to_string(),
                "step": step_idx + 1,
                "status": new_status.to_string(),
            });
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
        }
    }

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

    let path =
        found_path.ok_or_else(|| miette::miette!("No lot found matching '{}'", args.lot))?;

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
        return Err(miette::miette!("Lot {} is scrapped and cannot be completed", display_id));
    }

    // Check for incomplete steps
    let incomplete_steps: Vec<_> = lot
        .execution
        .iter()
        .enumerate()
        .filter(|(_, s)| s.status != ExecutionStatus::Completed && s.status != ExecutionStatus::Skipped)
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

    // Update lot
    lot.lot_status = LotStatus::Completed;
    lot.completion_date = Some(chrono::Local::now().date_naive());
    lot.entity_revision += 1;

    // Write updated lot
    let yaml_content = serde_yml::to_string(&lot).into_diagnostic()?;
    fs::write(&path, &yaml_content).into_diagnostic()?;

    // Output
    match global.format {
        OutputFormat::Json => {
            let result = serde_json::json!({
                "id": lot.id.to_string(),
                "short_id": display_id,
                "lot_status": "completed",
                "completion_date": lot.completion_date,
            });
            println!("{}", serde_json::to_string_pretty(&result).unwrap_or_default());
        }
        OutputFormat::Yaml => {
            let result = serde_json::json!({
                "id": lot.id.to_string(),
                "lot_status": "completed",
            });
            println!("{}", serde_yml::to_string(&result).unwrap_or_default());
        }
        _ => {
            println!(
                "{} Lot {} completed",
                style("✓").green(),
                style(&display_id).cyan()
            );
            if let Some(date) = lot.completion_date {
                println!("  Completion date: {}", date);
            }
        }
    }

    Ok(())
}
