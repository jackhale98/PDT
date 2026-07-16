//! `tdt ncr` command - Non-conformance report management

use chrono::Utc;
use clap::{Subcommand, ValueEnum};
use console::style;
use miette::{IntoDiagnostic, Result};

use crate::cli::commands::utils::format_link_with_title;
use crate::cli::helpers::format_short_id;
use crate::cli::table::{CellValue, ColumnDef, TableConfig, TableFormatter, TableRow};
use crate::cli::{GlobalOpts, OutputFormat};
use tdt_core::core::cache::{CachedNcr, EntityCache};
use tdt_core::core::identity::{EntityId, EntityPrefix};
use tdt_core::core::project::Project;
use tdt_core::core::shortid::ShortIdIndex;
use tdt_core::core::Config;
use tdt_core::entities::ncr::{
    DispositionDecision, Ncr, NcrCategory, NcrSeverity, NcrStatus, NcrType,
};
use tdt_core::schema::wizard::SchemaWizard;
use tdt_core::services::{
    CommonFilter, CreateNcr, NcrFilter, NcrService, NcrSortField, SortDirection,
};

/// CLI-friendly NCR type enum
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CliNcrType {
    Internal,
    Supplier,
    Customer,
}

impl std::fmt::Display for CliNcrType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CliNcrType::Internal => write!(f, "internal"),
            CliNcrType::Supplier => write!(f, "supplier"),
            CliNcrType::Customer => write!(f, "customer"),
        }
    }
}

impl From<CliNcrType> for NcrType {
    fn from(cli: CliNcrType) -> Self {
        match cli {
            CliNcrType::Internal => NcrType::Internal,
            CliNcrType::Supplier => NcrType::Supplier,
            CliNcrType::Customer => NcrType::Customer,
        }
    }
}

/// CLI-friendly NCR severity enum
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CliNcrSeverity {
    Minor,
    Major,
    Critical,
}

impl std::fmt::Display for CliNcrSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CliNcrSeverity::Minor => write!(f, "minor"),
            CliNcrSeverity::Major => write!(f, "major"),
            CliNcrSeverity::Critical => write!(f, "critical"),
        }
    }
}

impl From<CliNcrSeverity> for NcrSeverity {
    fn from(cli: CliNcrSeverity) -> Self {
        match cli {
            CliNcrSeverity::Minor => NcrSeverity::Minor,
            CliNcrSeverity::Major => NcrSeverity::Major,
            CliNcrSeverity::Critical => NcrSeverity::Critical,
        }
    }
}

/// CLI-friendly NCR category enum
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CliNcrCategory {
    Dimensional,
    Cosmetic,
    Material,
    Functional,
    Documentation,
    Process,
    Packaging,
}

impl std::fmt::Display for CliNcrCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CliNcrCategory::Dimensional => write!(f, "dimensional"),
            CliNcrCategory::Cosmetic => write!(f, "cosmetic"),
            CliNcrCategory::Material => write!(f, "material"),
            CliNcrCategory::Functional => write!(f, "functional"),
            CliNcrCategory::Documentation => write!(f, "documentation"),
            CliNcrCategory::Process => write!(f, "process"),
            CliNcrCategory::Packaging => write!(f, "packaging"),
        }
    }
}

impl From<CliNcrCategory> for NcrCategory {
    fn from(cli: CliNcrCategory) -> Self {
        match cli {
            CliNcrCategory::Dimensional => NcrCategory::Dimensional,
            CliNcrCategory::Cosmetic => NcrCategory::Cosmetic,
            CliNcrCategory::Material => NcrCategory::Material,
            CliNcrCategory::Functional => NcrCategory::Functional,
            CliNcrCategory::Documentation => NcrCategory::Documentation,
            CliNcrCategory::Process => NcrCategory::Process,
            CliNcrCategory::Packaging => NcrCategory::Packaging,
        }
    }
}

#[derive(Subcommand, Debug)]
pub enum NcrCommands {
    /// List NCRs with filtering
    List(ListArgs),

    /// Create a new NCR
    New(NewArgs),

    /// Show an NCR's details
    Show(ShowArgs),

    /// Edit an NCR in your editor
    Edit(EditArgs),

    /// Delete an NCR
    Delete(DeleteArgs),

    /// Archive an NCR (soft delete)
    Archive(ArchiveArgs),

    /// Close an NCR with disposition
    Close(CloseArgs),

    /// Advance NCR to the next workflow stage
    Advance(AdvanceArgs),
}

/// NCR type filter
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum NcrTypeFilter {
    Internal,
    Supplier,
    Customer,
    All,
}

/// Severity filter
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum SeverityFilter {
    Minor,
    Major,
    Critical,
    All,
}

/// NCR status filter
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum NcrStatusFilter {
    Open,
    Containment,
    Investigation,
    Disposition,
    Closed,
    All,
}

/// List column for display and sorting
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ListColumn {
    Id,
    Title,
    NcrType,
    Severity,
    Status,
    DaysOpen,
    Author,
    Created,
}

impl std::fmt::Display for ListColumn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ListColumn::Id => write!(f, "id"),
            ListColumn::Title => write!(f, "title"),
            ListColumn::NcrType => write!(f, "ncr-type"),
            ListColumn::Severity => write!(f, "severity"),
            ListColumn::Status => write!(f, "status"),
            ListColumn::DaysOpen => write!(f, "days-open"),
            ListColumn::Author => write!(f, "author"),
            ListColumn::Created => write!(f, "created"),
        }
    }
}

/// Column definitions for NCR list output
const NCR_COLUMNS: &[ColumnDef] = &[
    ColumnDef::new("id", "ID", 17),
    ColumnDef::new("title", "TITLE", 35),
    ColumnDef::new("ncr-type", "TYPE", 10),
    ColumnDef::new("severity", "SEV", 10),
    ColumnDef::new("status", "STATUS", 12),
    ColumnDef::new("days-open", "DAYS", 6),
    ColumnDef::new("author", "AUTHOR", 16),
    ColumnDef::new("created", "CREATED", 12),
];

#[derive(clap::Args, Debug)]
pub struct ListArgs {
    /// Filter by NCR type
    #[arg(long, short = 't', default_value = "all")]
    pub r#type: NcrTypeFilter,

    /// Filter by severity
    #[arg(long, short = 'S', default_value = "all")]
    pub severity: SeverityFilter,

    /// Filter by NCR status
    #[arg(long, default_value = "all")]
    pub ncr_status: NcrStatusFilter,

    /// Filter by author
    #[arg(long)]
    pub author: Option<String>,

    /// Show only recent NCRs (last 30 days)
    #[arg(long)]
    pub recent: bool,

    /// Show only stale NCRs open longer than N days (e.g., --stale 30)
    #[arg(long)]
    pub stale: Option<u32>,

    /// Search in title and description
    #[arg(long)]
    pub search: Option<String>,

    /// Show only open NCRs (status != closed) - shortcut filter
    #[arg(long)]
    pub open: bool,

    /// Columns to display
    #[arg(long, value_delimiter = ',', default_values_t = vec![
        ListColumn::NcrType,
        ListColumn::Title,
        ListColumn::Severity,
        ListColumn::Status,
        ListColumn::DaysOpen
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

    /// Wrap text in columns (mobile-friendly output with specified width)
    #[arg(long, short = 'w')]
    pub wrap: Option<usize>,

    /// Only show entities linked to these IDs (use - for stdin pipe)
    #[arg(long, value_delimiter = ',')]
    pub linked_to: Vec<String>,

    /// Filter by link type when using --linked-to (e.g., verified_by, satisfied_by)
    #[arg(long, requires = "linked_to")]
    pub via: Option<String>,
}

#[derive(clap::Args, Debug)]
pub struct NewArgs {
    /// NCR title (required)
    #[arg(long, short = 't')]
    pub title: Option<String>,

    /// NCR type
    #[arg(long, short = 'T', default_value = "internal")]
    pub r#type: CliNcrType,

    /// Severity level
    #[arg(long, short = 'S', default_value = "minor")]
    pub severity: CliNcrSeverity,

    /// Category
    #[arg(long, short = 'c', default_value = "dimensional")]
    pub category: CliNcrCategory,

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
    /// NCR ID or short ID (NCR@N)
    pub id: String,
}

#[derive(clap::Args, Debug)]
pub struct EditArgs {
    /// NCR ID or short ID (NCR@N)
    pub id: String,
}

#[derive(clap::Args, Debug)]
pub struct DeleteArgs {
    /// NCR ID or short ID (NCR@N)
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
    /// NCR ID or short ID (NCR@N)
    pub id: String,

    /// Force archive even if other entities reference this one
    #[arg(long)]
    pub force: bool,

    /// Suppress output
    #[arg(long, short = 'q')]
    pub quiet: bool,
}

/// Directories where NCRs are stored
const NCR_DIRS: &[&str] = &["manufacturing/ncrs"];

/// Entity configuration for NCRs
const ENTITY_CONFIG: crate::cli::EntityConfig = crate::cli::EntityConfig {
    prefix: EntityPrefix::Ncr,
    dirs: NCR_DIRS,
    name: "NCR",
    name_plural: "NCRs",
};

/// Disposition decision for CLI
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CliDisposition {
    /// Use the part as-is
    UseAsIs,
    /// Rework the part to spec
    Rework,
    /// Scrap the part
    Scrap,
    /// Return to supplier
    Return,
}

#[derive(clap::Args, Debug)]
pub struct CloseArgs {
    /// NCR ID or short ID (NCR@N)
    pub ncr: String,

    /// Disposition decision
    #[arg(long, short = 'd')]
    pub disposition: CliDisposition,

    /// Disposition justification/rationale
    #[arg(long, short = 'r')]
    pub rationale: Option<String>,

    /// Link to CAPA (create if needed)
    #[arg(long)]
    pub capa: Option<String>,

    /// Skip confirmation prompt
    #[arg(long, short = 'y')]
    pub yes: bool,
}

#[derive(clap::Args, Debug)]
pub struct AdvanceArgs {
    /// NCR ID or short ID (NCR@N)
    pub ncr: String,

    /// Target status (if omitted, advances to the next stage)
    #[arg(long, short = 's')]
    pub status: Option<CliNcrStatus>,

    /// Notes about the status change
    #[arg(long, short = 'n')]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CliNcrStatus {
    Open,
    Containment,
    Investigation,
    Disposition,
    Closed,
}

/// Run an NCR subcommand
pub fn run(cmd: NcrCommands, global: &GlobalOpts) -> Result<()> {
    match cmd {
        NcrCommands::List(args) => run_list(args, global),
        NcrCommands::New(args) => run_new(args, global),
        NcrCommands::Show(args) => run_show(args, global),
        NcrCommands::Edit(args) => run_edit(args),
        NcrCommands::Delete(args) => run_delete(args),
        NcrCommands::Archive(args) => run_archive(args),
        NcrCommands::Close(args) => run_close(args, global),
        NcrCommands::Advance(args) => run_advance(args, global),
    }
}

/// Build a NcrFilter from CLI ListArgs
fn build_ncr_filter(args: &ListArgs) -> NcrFilter {
    // Map NCR type
    let ncr_type = match args.r#type {
        NcrTypeFilter::Internal => Some(NcrType::Internal),
        NcrTypeFilter::Supplier => Some(NcrType::Supplier),
        NcrTypeFilter::Customer => Some(NcrType::Customer),
        NcrTypeFilter::All => None,
    };

    // Map severity
    let severity = match args.severity {
        SeverityFilter::Minor => Some(NcrSeverity::Minor),
        SeverityFilter::Major => Some(NcrSeverity::Major),
        SeverityFilter::Critical => Some(NcrSeverity::Critical),
        SeverityFilter::All => None,
    };

    // Map NCR status
    let ncr_status = match args.ncr_status {
        NcrStatusFilter::Open => Some(NcrStatus::Open),
        NcrStatusFilter::Containment => Some(NcrStatus::Containment),
        NcrStatusFilter::Investigation => Some(NcrStatus::Investigation),
        NcrStatusFilter::Disposition => Some(NcrStatus::Disposition),
        NcrStatusFilter::Closed => Some(NcrStatus::Closed),
        NcrStatusFilter::All => None,
    };

    NcrFilter {
        common: CommonFilter {
            author: args.author.clone(),
            search: args.search.clone(),
            limit: None, // Apply limit after sorting
            ..Default::default()
        },
        ncr_type,
        severity,
        ncr_status,
        category: None, // Not exposed in CLI
        open_only: args.open,
        recent_days: if args.recent { Some(30) } else { None },
        sort: build_ncr_sort_field(&args.sort),
        sort_direction: if args.reverse {
            SortDirection::Descending
        } else {
            SortDirection::Ascending
        },
    }
}

/// Convert CLI sort column to NcrSortField
fn build_ncr_sort_field(col: &ListColumn) -> NcrSortField {
    match col {
        ListColumn::Id => NcrSortField::Id,
        ListColumn::Title => NcrSortField::Title,
        ListColumn::NcrType => NcrSortField::NcrType,
        ListColumn::Severity => NcrSortField::Severity,
        ListColumn::Status => NcrSortField::NcrStatus,
        ListColumn::DaysOpen => NcrSortField::Created, // Sort by age via created date
        ListColumn::Author => NcrSortField::Author,
        ListColumn::Created => NcrSortField::Created,
    }
}

/// Calculate days open for an NCR
/// Uses report_date if available, otherwise created date
fn calculate_days_open(ncr: &Ncr) -> i64 {
    let today = Utc::now().date_naive();
    let start_date = ncr.report_date.unwrap_or_else(|| ncr.created.date_naive());
    (today - start_date).num_days()
}

/// Calculate days open from a cached NCR (uses created date)
fn calculate_days_open_cached(created: &chrono::DateTime<Utc>) -> i64 {
    let today = Utc::now().date_naive();
    (today - created.date_naive()).num_days()
}

/// Check if an NCR is stale (open longer than threshold days)
fn is_stale(ncr: &Ncr, threshold_days: u32) -> bool {
    if ncr.ncr_status == NcrStatus::Closed {
        return false;
    }
    calculate_days_open(ncr) > threshold_days as i64
}

/// Format days open for display, with warning indicator for stale NCRs
fn format_days_open(days: i64, is_closed: bool) -> String {
    if is_closed {
        "-".to_string()
    } else if days > 30 {
        format!("{}!", days) // Warning indicator for > 30 days
    } else {
        days.to_string()
    }
}

/// Sort cached NCRs according to CLI args
fn sort_cached_ncrs(ncrs: &mut Vec<CachedNcr>, args: &ListArgs) {
    match args.sort {
        ListColumn::Id => ncrs.sort_by(|a, b| a.id.cmp(&b.id)),
        ListColumn::Title => ncrs.sort_by(|a, b| a.title.cmp(&b.title)),
        ListColumn::NcrType => ncrs.sort_by(|a, b| {
            a.ncr_type
                .as_deref()
                .unwrap_or("")
                .cmp(b.ncr_type.as_deref().unwrap_or(""))
        }),
        ListColumn::Severity => ncrs.sort_by(|a, b| {
            a.severity
                .as_deref()
                .unwrap_or("")
                .cmp(b.severity.as_deref().unwrap_or(""))
        }),
        ListColumn::Status => ncrs.sort_by(|a, b| {
            a.ncr_status
                .as_deref()
                .unwrap_or("")
                .cmp(b.ncr_status.as_deref().unwrap_or(""))
        }),
        // Sort by days open (older first = more days)
        ListColumn::DaysOpen => ncrs.sort_by(|a, b| {
            let days_a = calculate_days_open_cached(&a.created);
            let days_b = calculate_days_open_cached(&b.created);
            days_b.cmp(&days_a) // Descending: oldest (most days) first
        }),
        ListColumn::Author => ncrs.sort_by(|a, b| a.author.cmp(&b.author)),
        ListColumn::Created => ncrs.sort_by(|a, b| a.created.cmp(&b.created)),
    }

    if args.reverse {
        ncrs.reverse();
    }

    if let Some(limit) = args.limit {
        ncrs.truncate(limit);
    }
}

/// Output full NCR entities
fn output_ncrs(
    ncrs: &[Ncr],
    short_ids: &mut ShortIdIndex,
    args: &ListArgs,
    format: OutputFormat,
    project: &Project,
) -> Result<()> {
    if ncrs.is_empty() {
        if args.count {
            println!("0");
        } else {
            println!("No NCRs found.");
        }
        return Ok(());
    }

    if args.count {
        println!("{}", ncrs.len());
        return Ok(());
    }

    // Update short ID index
    short_ids.ensure_all(ncrs.iter().map(|n| n.id.to_string()));
    super::utils::save_short_ids(short_ids, project);

    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&ncrs).into_diagnostic()?;
            println!("{}", json);
        }
        OutputFormat::Yaml => {
            let yaml = serde_yml::to_string(&ncrs).into_diagnostic()?;
            print!("{}", yaml);
        }
        OutputFormat::Csv
        | OutputFormat::Tsv
        | OutputFormat::Md
        | OutputFormat::Table
        | OutputFormat::Dot
        | OutputFormat::Tree => {
            // Build column list from args
            let columns: Vec<&str> = args
                .columns
                .iter()
                .map(|c| c.to_string().leak() as &str)
                .collect();

            // Build rows
            let rows: Vec<TableRow> = ncrs.iter().map(|n| ncr_to_row(n, short_ids)).collect();

            let config = TableConfig {
                wrap_width: args.wrap,
                show_summary: true,
            };
            let formatter = TableFormatter::new(NCR_COLUMNS, "NCR", "NCR").with_config(config);
            formatter.output(rows, format, &columns);
        }
        OutputFormat::Id | OutputFormat::ShortId => {
            for ncr in ncrs {
                if format == OutputFormat::ShortId {
                    let short_id = short_ids
                        .get_short_id(&ncr.id.to_string())
                        .unwrap_or_default();
                    println!("{}", short_id);
                } else {
                    println!("{}", ncr.id);
                }
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
    let service = NcrService::new(&project, &cache);
    let mut short_ids = ShortIdIndex::load(&project);

    // Resolve linked-to filter via cache
    let allowed_ids = crate::cli::helpers::resolve_linked_to(
        &args.linked_to,
        args.via.as_deref(),
        &short_ids,
        &cache,
    );

    let format = match global.output {
        OutputFormat::Auto => OutputFormat::Tsv,
        f => f,
    };

    // Build filter from CLI args
    let filter = build_ncr_filter(&args);

    // Fast path: use cache when possible
    // Can't use cache for: search (requires description), stale (requires report_date),
    // JSON/YAML (need full entity)
    let can_use_cache = args.search.is_none()
        && args.stale.is_none()
        && !matches!(format, OutputFormat::Json | OutputFormat::Yaml);

    if can_use_cache {
        let mut cached_ncrs = service
            .list_cached(&filter)
            .map_err(|e| miette::miette!("{}", e))?;

        // Apply linked-to filter
        if let Some(ref ids) = allowed_ids {
            cached_ncrs.retain(|e| ids.contains(&e.id));
        }

        // Sort and limit
        sort_cached_ncrs(&mut cached_ncrs, &args);

        // Update short ID index
        short_ids.ensure_all(cached_ncrs.iter().map(|n| n.id.clone()));
        super::utils::save_short_ids(&mut short_ids, &project);

        return output_cached_ncrs(&cached_ncrs, &args, &short_ids, format, &project);
    }

    // Full entity loading path
    let mut ncrs = service
        .list(&filter)
        .map_err(|e| miette::miette!("{}", e))?;

    // Apply linked-to filter
    if let Some(ref ids) = allowed_ids {
        ncrs.retain(|e| ids.contains(&e.id.to_string()));
    }

    // Apply stale filter (requires full entity with report_date)
    if let Some(threshold_days) = args.stale {
        ncrs.retain(|ncr| is_stale(ncr, threshold_days));
    }

    // Apply limit
    if let Some(limit) = args.limit {
        ncrs.truncate(limit);
    }

    output_ncrs(&ncrs, &mut short_ids, &args, format, &project)
}

fn run_new(args: NewArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let cache = EntityCache::open(&project).map_err(|e| miette::miette!("{}", e))?;
    let config = Config::load();
    let service = NcrService::new(&project, &cache);

    let title: String;
    let ncr_type: NcrType;
    let severity: NcrSeverity;
    let category: NcrCategory;
    let description: Option<String>;

    if args.interactive {
        let wizard = SchemaWizard::new();
        let result = wizard.run(EntityPrefix::Ncr)?;

        title = result
            .get_string("title")
            .map(String::from)
            .unwrap_or_else(|| "New NCR".to_string());
        ncr_type = result
            .get_string("ncr_type")
            .and_then(|s| s.parse().ok())
            .unwrap_or(NcrType::Internal);
        severity = result
            .get_string("severity")
            .and_then(|s| s.parse().ok())
            .unwrap_or(NcrSeverity::Minor);
        category = result
            .get_string("category")
            .and_then(|s| s.parse().ok())
            .unwrap_or(NcrCategory::Dimensional);
        description = result.get_string("description").map(String::from);
    } else {
        title = args.title.unwrap_or_else(|| "New NCR".to_string());
        ncr_type = NcrType::from(args.r#type);
        severity = NcrSeverity::from(args.severity);
        category = NcrCategory::from(args.category);
        description = None;
    }

    // Create NCR via service
    let input = CreateNcr {
        title: title.clone(),
        ncr_number: None,
        ncr_type,
        severity,
        category,
        description,
        report_date: None,
        tags: Vec::new(),
        status: None,
        author: config.author(),
        lot_ids: Vec::new(),
    };

    let ncr = service
        .create(input)
        .map_err(|e| miette::miette!("{}", e))?;

    // Get file path for the created NCR
    let file_path = project
        .root()
        .join("manufacturing/ncrs")
        .join(format!("{}.tdt.yaml", ncr.id));

    // Add to short ID index
    let mut short_ids = ShortIdIndex::load(&project);
    let short_id = short_ids.add(ncr.id.to_string());
    super::utils::save_short_ids(&mut short_ids, &project);

    // Handle --link flags
    let added_links = crate::cli::entity_cmd::process_link_flags(
        &file_path,
        EntityPrefix::Ncr,
        &args.link,
        &short_ids,
    );

    // Output based on format flag
    match global.output {
        OutputFormat::Id => {
            println!("{}", ncr.id);
        }
        OutputFormat::ShortId => {
            println!(
                "{}",
                short_id.clone().unwrap_or_else(|| format_short_id(&ncr.id))
            );
        }
        OutputFormat::Path => {
            println!("{}", file_path.display());
        }
        _ => {
            let severity_str = ncr.severity.to_string();
            let severity_styled = match ncr.severity {
                NcrSeverity::Critical => style(&severity_str).red().bold(),
                NcrSeverity::Major => style(&severity_str).yellow(),
                _ => style(&severity_str).white(),
            };

            println!(
                "{} Created NCR {}",
                style("✓").green(),
                style(short_id.clone().unwrap_or_else(|| format_short_id(&ncr.id))).cyan()
            );
            println!("   {}", style(file_path.display()).dim());
            println!(
                "   {} | {} | {}",
                style(ncr.ncr_type.to_string()).yellow(),
                severity_styled,
                style(&title).white()
            );

            // Show added links
            for (link_type, target) in &added_links {
                println!(
                    "   {} --[{}]--> {}",
                    style("→").dim(),
                    style(link_type).cyan(),
                    style(
                        &short_ids
                            .get_short_id(target)
                            .unwrap_or_else(|| target.clone())
                    )
                    .yellow()
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

    // Use NcrService to get the NCR (cache-first lookup)
    let service = NcrService::new(&project, &cache);
    let ncr = service
        .get(&resolved_id)
        .map_err(|e| miette::miette!("{}", e))?
        .ok_or_else(|| miette::miette!("No NCR found matching '{}'", args.id))?;

    match global.output {
        OutputFormat::Yaml => {
            let yaml = serde_yml::to_string(&ncr).into_diagnostic()?;
            print!("{}", yaml);
        }
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&ncr).into_diagnostic()?;
            println!("{}", json);
        }
        OutputFormat::Id | OutputFormat::ShortId => {
            if global.output == OutputFormat::ShortId {
                let short_ids = ShortIdIndex::load(&project);
                let short_id = short_ids
                    .get_short_id(&ncr.id.to_string())
                    .unwrap_or_default();
                println!("{}", short_id);
            } else {
                println!("{}", ncr.id);
            }
        }
        _ => {
            // Pretty format (default)
            println!("{}", style("─".repeat(60)).dim());
            println!(
                "{}: {}",
                style("ID").bold(),
                style(&ncr.id.to_string()).cyan()
            );
            println!("{}: {}", style("Title").bold(), style(&ncr.title).yellow());
            println!("{}: {}", style("NCR Type").bold(), ncr.ncr_type);
            let severity_style = match ncr.severity {
                tdt_core::entities::ncr::NcrSeverity::Critical => {
                    style(ncr.severity.to_string()).red().bold()
                }
                tdt_core::entities::ncr::NcrSeverity::Major => {
                    style(ncr.severity.to_string()).red()
                }
                tdt_core::entities::ncr::NcrSeverity::Minor => {
                    style(ncr.severity.to_string()).yellow()
                }
            };
            println!("{}: {}", style("Severity").bold(), severity_style);
            println!("{}: {}", style("NCR Status").bold(), ncr.ncr_status);
            if let Some(ref disp) = ncr.disposition {
                if let Some(decision) = disp.decision {
                    println!("{}: {}", style("Disposition").bold(), decision);
                }
            }
            println!("{}", style("─".repeat(60)).dim());

            // Description
            if let Some(ref desc) = ncr.description {
                if !desc.is_empty() && !desc.starts_with('#') {
                    println!();
                    println!("{}", style("Description:").bold());
                    println!("{}", desc);
                }
            }

            // Detection info
            if let Some(ref det) = ncr.detection {
                println!();
                println!("{}", style("Detection:").bold());
                println!("  Found at: {:?}", det.found_at);
                if let Some(ref by) = det.found_by {
                    println!("  Found by: {}", by);
                }
            }

            // Affected Items
            if let Some(ref items) = ncr.affected_items {
                println!();
                println!("{}", style("Affected Items:").bold());
                if let Some(ref pn) = items.part_number {
                    println!("  Part Number: {}", pn);
                }
                if let Some(ref lot) = items.lot_number {
                    println!("  Lot: {}", lot);
                }
                if let Some(qty) = items.quantity_affected {
                    println!("  Quantity: {}", qty);
                }
            }

            // Containment
            if !ncr.containment.is_empty() {
                println!();
                println!(
                    "{} ({}):",
                    style("Containment Actions").bold(),
                    ncr.containment.len()
                );
                for action in &ncr.containment {
                    println!("  • {} [{:?}]", action.action, action.status);
                }
            }

            // Tags
            if !ncr.tags.is_empty() {
                println!();
                println!("{}: {}", style("Tags").bold(), ncr.tags.join(", "));
            }

            // Links
            let cache = EntityCache::open(&project).ok();
            let has_links = ncr.links.component.is_some()
                || ncr.links.process.is_some()
                || ncr.links.control.is_some()
                || ncr.links.capa.is_some();

            if has_links {
                println!();
                println!("{}", style("Links:").bold());

                if let Some(ref id) = ncr.links.component {
                    let display = format_link_with_title(&id.to_string(), &short_ids, &cache);
                    println!("  {}: {}", style("Component").dim(), style(&display).cyan());
                }

                if let Some(ref id) = ncr.links.process {
                    let display = format_link_with_title(&id.to_string(), &short_ids, &cache);
                    println!("  {}: {}", style("Process").dim(), style(&display).cyan());
                }

                if let Some(ref id) = ncr.links.control {
                    let display = format_link_with_title(&id.to_string(), &short_ids, &cache);
                    println!("  {}: {}", style("Control").dim(), style(&display).cyan());
                }

                if let Some(ref id) = ncr.links.capa {
                    let display = format_link_with_title(&id.to_string(), &short_ids, &cache);
                    println!("  {}: {}", style("CAPA").dim(), style(&display).cyan());
                }
            }

            // Footer
            println!("{}", style("─".repeat(60)).dim());
            println!(
                "{}: {} | {}: {} | {}: {}",
                style("Author").dim(),
                ncr.author,
                style("Created").dim(),
                ncr.created.format("%Y-%m-%d %H:%M"),
                style("Revision").dim(),
                ncr.entity_revision
            );
        }
    }

    Ok(())
}

fn run_edit(args: EditArgs) -> Result<()> {
    crate::cli::entity_cmd::run_edit_generic(&args.id, &ENTITY_CONFIG)
}

fn run_delete(args: DeleteArgs) -> Result<()> {
    crate::cli::commands::utils::run_delete(&args.id, NCR_DIRS, args.force, false, args.quiet)
}

fn run_archive(args: ArchiveArgs) -> Result<()> {
    crate::cli::commands::utils::run_delete(&args.id, NCR_DIRS, args.force, true, args.quiet)
}

/// Output cached NCRs in the requested format
fn output_cached_ncrs(
    ncrs: &[CachedNcr],
    args: &ListArgs,
    short_ids: &ShortIdIndex,
    format: OutputFormat,
    project: &Project,
) -> Result<()> {
    // Count only
    if args.count {
        println!("{}", ncrs.len());
        return Ok(());
    }

    // No results
    if ncrs.is_empty() {
        println!("No NCRs found.");
        return Ok(());
    }

    match format {
        OutputFormat::Csv
        | OutputFormat::Tsv
        | OutputFormat::Md
        | OutputFormat::Table
        | OutputFormat::Dot
        | OutputFormat::Tree => {
            // Build column list from args
            let columns: Vec<&str> = args
                .columns
                .iter()
                .map(|c| c.to_string().leak() as &str)
                .collect();

            // Build rows
            let rows: Vec<TableRow> = ncrs
                .iter()
                .map(|n| cached_ncr_to_row(n, short_ids))
                .collect();

            let config = TableConfig {
                wrap_width: args.wrap,
                show_summary: true,
            };
            let formatter = TableFormatter::new(NCR_COLUMNS, "NCR", "NCR").with_config(config);
            formatter.output(rows, format, &columns);
        }
        OutputFormat::Id | OutputFormat::ShortId => {
            for ncr in ncrs {
                if format == OutputFormat::ShortId {
                    let short_id = short_ids.get_short_id(&ncr.id).unwrap_or_default();
                    println!("{}", short_id);
                } else {
                    println!("{}", ncr.id);
                }
            }
        }
        OutputFormat::Path => {
            for ncr in ncrs {
                let path = if ncr.file_path.is_absolute() {
                    ncr.file_path.clone()
                } else {
                    project.root().join(&ncr.file_path)
                };
                println!("{}", path.display());
            }
        }
        OutputFormat::Json | OutputFormat::Yaml | OutputFormat::Auto => {
            // Should not reach here - cache bypassed for these formats
            unreachable!();
        }
    }

    Ok(())
}

/// Convert an Ncr to a TableRow
fn ncr_to_row(ncr: &Ncr, short_ids: &ShortIdIndex) -> TableRow {
    let is_closed = ncr.ncr_status == NcrStatus::Closed;
    let days_open = calculate_days_open(ncr);
    let days_display = format_days_open(days_open, is_closed);

    TableRow::new(ncr.id.to_string(), short_ids)
        .cell("id", CellValue::Id(ncr.id.to_string()))
        .cell("title", CellValue::Text(ncr.title.clone()))
        .cell("ncr-type", CellValue::Type(ncr.ncr_type.to_string()))
        .cell("severity", CellValue::NcrSeverity(ncr.severity.to_string()))
        .cell("status", CellValue::Type(ncr.ncr_status.to_string()))
        .cell("days-open", CellValue::Text(days_display))
        .cell("author", CellValue::Text(ncr.author.clone()))
        .cell("created", CellValue::DateTime(ncr.created))
}

/// Convert a CachedNcr to a TableRow
fn cached_ncr_to_row(ncr: &CachedNcr, short_ids: &ShortIdIndex) -> TableRow {
    let is_closed = ncr.ncr_status.as_deref() == Some("closed");
    let days_open = calculate_days_open_cached(&ncr.created);
    let days_display = format_days_open(days_open, is_closed);

    TableRow::new(ncr.id.clone(), short_ids)
        .cell("id", CellValue::Id(ncr.id.clone()))
        .cell("title", CellValue::Text(ncr.title.clone()))
        .cell(
            "ncr-type",
            CellValue::Type(ncr.ncr_type.as_deref().unwrap_or("-").to_string()),
        )
        .cell(
            "severity",
            CellValue::NcrSeverity(ncr.severity.as_deref().unwrap_or("-").to_string()),
        )
        .cell(
            "status",
            CellValue::Type(ncr.ncr_status.as_deref().unwrap_or("-").to_string()),
        )
        .cell("days-open", CellValue::Text(days_display))
        .cell("author", CellValue::Text(ncr.author.clone()))
        .cell("created", CellValue::DateTime(ncr.created))
}

fn run_close(args: CloseArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let cache = EntityCache::open(&project).map_err(|e| miette::miette!("{}", e))?;
    let service = NcrService::new(&project, &cache);
    let config = Config::load();
    let short_ids = ShortIdIndex::load(&project);

    // Resolve short ID if needed
    let resolved_id = short_ids
        .resolve(&args.ncr)
        .unwrap_or_else(|| args.ncr.clone());

    // Get display ID for user messages
    let display_id = short_ids
        .get_short_id(&resolved_id)
        .unwrap_or_else(|| args.ncr.clone());

    // Get NCR to validate status and show confirmation
    let ncr = service
        .get(&resolved_id)
        .map_err(|e| miette::miette!("{}", e))?
        .ok_or_else(|| miette::miette!("No NCR found matching '{}'", args.ncr))?;

    // Validate status allows closing
    if ncr.ncr_status == NcrStatus::Closed {
        return Err(miette::miette!("NCR {} is already closed", display_id));
    }

    // Convert CLI disposition to entity enum
    let disposition_decision = match args.disposition {
        CliDisposition::UseAsIs => DispositionDecision::UseAsIs,
        CliDisposition::Rework => DispositionDecision::Rework,
        CliDisposition::Scrap => DispositionDecision::Scrap,
        CliDisposition::Return => DispositionDecision::ReturnToSupplier,
    };

    // Resolve CAPA link if provided
    let capa_ref = args
        .capa
        .as_ref()
        .map(|c| short_ids.resolve(c).unwrap_or_else(|| c.clone()));

    // Show current state and confirmation
    if !args.yes {
        println!();
        println!("{}", style("Closing NCR").bold().cyan());
        println!("{}", style("─".repeat(50)).dim());
        println!("NCR: {} \"{}\"", style(&display_id).cyan(), &ncr.title);
        println!("Current Status: {}", ncr.ncr_status);
        println!("Severity: {}", ncr.severity);
        println!();
        println!(
            "Disposition: {}",
            style(format!("{:?}", args.disposition)).yellow()
        );
        if let Some(ref rationale) = args.rationale {
            println!("Rationale: {}", rationale);
        }
        if let Some(ref capa) = capa_ref {
            let capa_display = short_ids.get_short_id(capa).unwrap_or_else(|| capa.clone());
            println!("Linked CAPA: {}", style(&capa_display).cyan());
        }
        println!();

        // Simple confirmation
        print!("Confirm close? [y/N] ");
        std::io::Write::flush(&mut std::io::stdout()).into_diagnostic()?;
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).into_diagnostic()?;
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Cancelled.");
            return Ok(());
        }
    }

    // Use service to close NCR
    let mut ncr = service
        .close(
            &resolved_id,
            disposition_decision,
            args.rationale.clone(),
            config.author(),
        )
        .map_err(|e| miette::miette!("{}", e))?;

    // Add CAPA link if provided (separate operation)
    if let Some(ref capa_id) = capa_ref {
        if let Ok(entity_id) = capa_id.parse::<EntityId>() {
            ncr = service
                .set_capa_link(&ncr.id.to_string(), entity_id)
                .map_err(|e| miette::miette!("{}", e))?;
        }
    }

    // Output based on format
    match global.output {
        OutputFormat::Json => {
            let result = serde_json::json!({
                "id": ncr.id.to_string(),
                "short_id": display_id,
                "ncr_status": "closed",
                "disposition": disposition_decision.to_string(),
                "capa": capa_ref,
            });
            println!(
                "{}",
                serde_json::to_string_pretty(&result).unwrap_or_default()
            );
        }
        OutputFormat::Yaml => {
            let result = serde_json::json!({
                "id": ncr.id.to_string(),
                "ncr_status": "closed",
                "disposition": disposition_decision.to_string(),
            });
            println!("{}", serde_yml::to_string(&result).unwrap_or_default());
        }
        _ => {
            println!();
            println!(
                "{} NCR {} closed",
                style("✓").green(),
                style(&display_id).cyan()
            );
            println!(
                "  Disposition: {}",
                style(format!("{:?}", args.disposition)).yellow()
            );
            if let Some(ref capa_id) = capa_ref {
                let capa_display = short_ids
                    .get_short_id(capa_id)
                    .unwrap_or_else(|| capa_id.clone());
                println!("  Linked CAPA: {}", style(&capa_display).cyan());
            }
        }
    }

    // Sync cache after mutation
    super::utils::sync_cache(&project);

    Ok(())
}

fn run_advance(args: AdvanceArgs, _global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let cache = EntityCache::open(&project).map_err(|e| miette::miette!("{}", e))?;
    let service = NcrService::new(&project, &cache);
    let short_ids = ShortIdIndex::load(&project);

    let resolved_id = short_ids
        .resolve(&args.ncr)
        .unwrap_or_else(|| args.ncr.clone());

    let display_id = short_ids
        .get_short_id(&resolved_id)
        .unwrap_or_else(|| args.ncr.clone());

    let ncr = service
        .get(&resolved_id)
        .map_err(|e| miette::miette!("{}", e))?
        .ok_or_else(|| miette::miette!("No NCR found matching '{}'", args.ncr))?;

    // Determine target status
    let target_status = if let Some(cli_status) = args.status {
        match cli_status {
            CliNcrStatus::Open => NcrStatus::Open,
            CliNcrStatus::Containment => NcrStatus::Containment,
            CliNcrStatus::Investigation => NcrStatus::Investigation,
            CliNcrStatus::Disposition => NcrStatus::Disposition,
            CliNcrStatus::Closed => {
                return Err(miette::miette!(
                    "Use 'tdt ncr close' to close an NCR (requires disposition decision)"
                ));
            }
        }
    } else {
        // Auto-advance to next stage
        match ncr.ncr_status {
            NcrStatus::Open => NcrStatus::Containment,
            NcrStatus::Containment => NcrStatus::Investigation,
            NcrStatus::Investigation => NcrStatus::Disposition,
            NcrStatus::Disposition => {
                return Err(miette::miette!(
                    "NCR {} is in disposition stage. Use 'tdt ncr close' to close with a disposition decision.",
                    display_id
                ));
            }
            NcrStatus::Closed => {
                return Err(miette::miette!("NCR {} is already closed", display_id));
            }
        }
    };

    // Update NCR YAML directly
    let ncr_file = project
        .root()
        .join("manufacturing/ncrs")
        .join(format!("{}.tdt.yaml", ncr.id));
    let content = std::fs::read_to_string(&ncr_file).into_diagnostic()?;
    let mut ncr_yaml: serde_json::Value = serde_yml::from_str(&content).into_diagnostic()?;

    ncr_yaml["ncr_status"] = serde_json::json!(target_status.to_string());

    if let Some(rev) = ncr_yaml.get("entity_revision").and_then(|v| v.as_u64()) {
        ncr_yaml["entity_revision"] = serde_json::json!(rev + 1);
    }

    let yaml_out = serde_yml::to_string(&ncr_yaml).into_diagnostic()?;
    std::fs::write(&ncr_file, yaml_out).into_diagnostic()?;

    println!(
        "{} NCR {} advanced: {} → {}",
        style("✓").green(),
        style(&display_id).cyan(),
        style(ncr.ncr_status.to_string()).dim(),
        style(target_status.to_string()).yellow()
    );

    // Show next steps
    match target_status {
        NcrStatus::Containment => {
            println!("   Next: Define containment actions, then advance to investigation");
        }
        NcrStatus::Investigation => {
            println!("   Next: Investigate root cause, then advance to disposition");
        }
        NcrStatus::Disposition => {
            println!(
                "   Next: Close with disposition: tdt ncr close {} --disposition <decision>",
                display_id
            );
        }
        _ => {}
    }

    super::utils::sync_cache(&project);
    Ok(())
}
