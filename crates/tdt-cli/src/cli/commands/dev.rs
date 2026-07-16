//! `tdt dev` command - Process deviation management

use clap::{Subcommand, ValueEnum};
use console::style;
use miette::{IntoDiagnostic, Result};

use crate::cli::helpers::{escape_csv, truncate_str};
use crate::cli::{GlobalOpts, OutputFormat};
use tdt_core::core::cache::{CachedDeviation, EntityCache};
use tdt_core::core::identity::EntityPrefix;
use tdt_core::core::project::Project;
use tdt_core::core::shortid::ShortIdIndex;
use tdt_core::core::Config;
use tdt_core::entities::dev::{
    AuthorizationLevel, Dev, DevStatus, DeviationCategory, DeviationType, RiskLevel,
};
use tdt_core::schema::wizard::SchemaWizard;
use tdt_core::services::{
    CommonFilter, CreateDeviation, DeviationFilter, DeviationService, DeviationSortField,
    SortDirection,
};

/// CLI-friendly deviation type enum
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CliDeviationType {
    Temporary,
    Permanent,
    Emergency,
}

impl std::fmt::Display for CliDeviationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CliDeviationType::Temporary => write!(f, "temporary"),
            CliDeviationType::Permanent => write!(f, "permanent"),
            CliDeviationType::Emergency => write!(f, "emergency"),
        }
    }
}

impl From<CliDeviationType> for DeviationType {
    fn from(cli: CliDeviationType) -> Self {
        match cli {
            CliDeviationType::Temporary => DeviationType::Temporary,
            CliDeviationType::Permanent => DeviationType::Permanent,
            CliDeviationType::Emergency => DeviationType::Emergency,
        }
    }
}

/// CLI-friendly deviation status enum
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CliDevStatus {
    Pending,
    Approved,
    Active,
    Expired,
    Closed,
    Rejected,
}

impl std::fmt::Display for CliDevStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CliDevStatus::Pending => write!(f, "pending"),
            CliDevStatus::Approved => write!(f, "approved"),
            CliDevStatus::Active => write!(f, "active"),
            CliDevStatus::Expired => write!(f, "expired"),
            CliDevStatus::Closed => write!(f, "closed"),
            CliDevStatus::Rejected => write!(f, "rejected"),
        }
    }
}

impl From<CliDevStatus> for DevStatus {
    fn from(cli: CliDevStatus) -> Self {
        match cli {
            CliDevStatus::Pending => DevStatus::Pending,
            CliDevStatus::Approved => DevStatus::Approved,
            CliDevStatus::Active => DevStatus::Active,
            CliDevStatus::Expired => DevStatus::Expired,
            CliDevStatus::Closed => DevStatus::Closed,
            CliDevStatus::Rejected => DevStatus::Rejected,
        }
    }
}

/// CLI-friendly category enum
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CliCategory {
    Material,
    Process,
    Equipment,
    Tooling,
    Specification,
    Documentation,
}

impl std::fmt::Display for CliCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CliCategory::Material => write!(f, "material"),
            CliCategory::Process => write!(f, "process"),
            CliCategory::Equipment => write!(f, "equipment"),
            CliCategory::Tooling => write!(f, "tooling"),
            CliCategory::Specification => write!(f, "specification"),
            CliCategory::Documentation => write!(f, "documentation"),
        }
    }
}

impl From<CliCategory> for DeviationCategory {
    fn from(cli: CliCategory) -> Self {
        match cli {
            CliCategory::Material => DeviationCategory::Material,
            CliCategory::Process => DeviationCategory::Process,
            CliCategory::Equipment => DeviationCategory::Equipment,
            CliCategory::Tooling => DeviationCategory::Tooling,
            CliCategory::Specification => DeviationCategory::Specification,
            CliCategory::Documentation => DeviationCategory::Documentation,
        }
    }
}

/// CLI-friendly risk level enum
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CliRiskLevel {
    Low,
    Medium,
    High,
}

impl std::fmt::Display for CliRiskLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CliRiskLevel::Low => write!(f, "low"),
            CliRiskLevel::Medium => write!(f, "medium"),
            CliRiskLevel::High => write!(f, "high"),
        }
    }
}

impl From<CliRiskLevel> for RiskLevel {
    fn from(cli: CliRiskLevel) -> Self {
        match cli {
            CliRiskLevel::Low => RiskLevel::Low,
            CliRiskLevel::Medium => RiskLevel::Medium,
            CliRiskLevel::High => RiskLevel::High,
        }
    }
}

/// CLI-friendly authorization level enum
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CliAuthLevel {
    Engineering,
    Quality,
    Management,
}

impl std::fmt::Display for CliAuthLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CliAuthLevel::Engineering => write!(f, "engineering"),
            CliAuthLevel::Quality => write!(f, "quality"),
            CliAuthLevel::Management => write!(f, "management"),
        }
    }
}

impl From<CliAuthLevel> for AuthorizationLevel {
    fn from(cli: CliAuthLevel) -> Self {
        match cli {
            CliAuthLevel::Engineering => AuthorizationLevel::Engineering,
            CliAuthLevel::Quality => AuthorizationLevel::Quality,
            CliAuthLevel::Management => AuthorizationLevel::Management,
        }
    }
}

#[derive(Subcommand, Debug)]
pub enum DevCommands {
    /// List deviations with filtering
    List(ListArgs),

    /// Create a new deviation
    New(NewArgs),

    /// Show a deviation's details
    Show(ShowArgs),

    /// Edit a deviation in your editor
    Edit(EditArgs),

    /// Delete a deviation
    Delete(DeleteArgs),

    /// Archive a deviation (soft delete)
    Archive(ArchiveArgs),

    /// Approve a deviation
    Approve(ApproveArgs),

    /// Expire/close a deviation
    Expire(ExpireArgs),
}

/// Deviation status filter
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum DevStatusFilter {
    Pending,
    Approved,
    Active,
    Expired,
    Closed,
    Rejected,
    All,
}

/// List column for display and sorting
#[derive(Debug, Clone, Copy, ValueEnum, PartialEq)]
pub enum ListColumn {
    #[value(name = "id")]
    Id,
    #[value(name = "title")]
    Title,
    #[value(name = "dev-number")]
    DevNumber,
    #[value(name = "dev-type")]
    DevType,
    #[value(name = "category")]
    Category,
    #[value(name = "risk")]
    Risk,
    #[value(name = "dev-status")]
    DevStatus,
    #[value(name = "author")]
    Author,
    #[value(name = "created")]
    Created,
}

impl std::fmt::Display for ListColumn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ListColumn::Id => write!(f, "id"),
            ListColumn::Title => write!(f, "title"),
            ListColumn::DevNumber => write!(f, "dev-number"),
            ListColumn::DevType => write!(f, "dev-type"),
            ListColumn::Category => write!(f, "category"),
            ListColumn::Risk => write!(f, "risk"),
            ListColumn::DevStatus => write!(f, "dev-status"),
            ListColumn::Author => write!(f, "author"),
            ListColumn::Created => write!(f, "created"),
        }
    }
}

#[derive(clap::Args, Debug)]
pub struct ListArgs {
    /// Filter by deviation status
    #[arg(long, short = 's', default_value = "all")]
    pub status: DevStatusFilter,

    /// Filter by deviation type
    #[arg(long, short = 'T')]
    pub dev_type: Option<CliDeviationType>,

    /// Filter by category
    #[arg(long, short = 'c')]
    pub category: Option<CliCategory>,

    /// Filter by risk level
    #[arg(long)]
    pub risk: Option<CliRiskLevel>,

    /// Filter by author
    #[arg(long)]
    pub author: Option<String>,

    /// Show only recent deviations (last 30 days)
    #[arg(long)]
    pub recent: bool,

    /// Search in title and deviation number
    #[arg(long)]
    pub search: Option<String>,

    /// Show only active deviations
    #[arg(long)]
    pub active: bool,

    /// Columns to display
    #[arg(long, value_delimiter = ',', default_values_t = vec![
        ListColumn::Title,
        ListColumn::DevType,
        ListColumn::Category,
        ListColumn::Risk,
        ListColumn::DevStatus
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
    /// Deviation title (required)
    #[arg(long, short = 't')]
    pub title: Option<String>,

    /// User-defined deviation number
    #[arg(long, short = 'd')]
    pub deviation_number: Option<String>,

    /// Deviation type
    #[arg(long, short = 'T', default_value = "temporary")]
    pub dev_type: CliDeviationType,

    /// Category
    #[arg(long, short = 'c', default_value = "material")]
    pub category: CliCategory,

    /// Risk level
    #[arg(long, short = 'R', default_value = "low")]
    pub risk: CliRiskLevel,

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
    /// Deviation ID (full or short)
    pub id: String,
}

#[derive(clap::Args, Debug)]
pub struct EditArgs {
    /// Deviation ID (full or short)
    pub id: String,
}

#[derive(clap::Args, Debug)]
pub struct DeleteArgs {
    /// Deviation ID (full or short)
    pub id: String,

    /// Force deletion even if entity is linked
    #[arg(long)]
    pub force: bool,

    /// Skip confirmation prompt
    #[arg(long, short = 'y')]
    pub quiet: bool,
}

#[derive(clap::Args, Debug)]
pub struct ArchiveArgs {
    /// Deviation ID (full or short)
    pub id: String,

    /// Force archival even if entity is linked
    #[arg(long)]
    pub force: bool,

    /// Skip confirmation prompt
    #[arg(long, short = 'y')]
    pub quiet: bool,
}

#[derive(clap::Args, Debug)]
pub struct ApproveArgs {
    /// Deviation ID (full or short)
    pub id: String,

    /// Approved by (defaults to config author)
    #[arg(long)]
    pub approved_by: Option<String>,

    /// Authorization level
    #[arg(long, short = 'a', default_value = "engineering")]
    pub authorization: CliAuthLevel,

    /// Set status to active (default: approved)
    #[arg(long)]
    pub activate: bool,

    /// Sign the approval (GPG/SSH/gitsign signature)
    #[arg(long, short = 'S')]
    pub sign: bool,

    /// Skip confirmation prompt
    #[arg(long, short = 'y')]
    pub yes: bool,
}

#[derive(clap::Args, Debug)]
pub struct ExpireArgs {
    /// Deviation ID (full or short)
    pub id: String,

    /// Reason for closing
    #[arg(long)]
    pub reason: Option<String>,

    /// Skip confirmation prompt
    #[arg(long, short = 'y')]
    pub yes: bool,
}

/// Search directories for this entity type — single source of truth
/// lives in `Project::entity_search_directories`.
const DEV_DIRS: &[&str] = tdt_core::core::project::Project::entity_search_directories(
    tdt_core::core::identity::EntityPrefix::Dev,
);

/// Entity configuration for deviation commands
const ENTITY_CONFIG: crate::cli::EntityConfig = crate::cli::EntityConfig {
    prefix: EntityPrefix::Dev,
    dirs: DEV_DIRS,
    name: "deviation",
    name_plural: "deviations",
};

/// Run a deviation command
pub fn run(cmd: DevCommands, global: &GlobalOpts) -> Result<()> {
    match cmd {
        DevCommands::List(args) => run_list(args, global),
        DevCommands::New(args) => run_new(args, global),
        DevCommands::Show(args) => run_show(args, global),
        DevCommands::Edit(args) => run_edit(args),
        DevCommands::Delete(args) => run_delete(args),
        DevCommands::Archive(args) => run_archive(args),
        DevCommands::Approve(args) => run_approve(args, global),
        DevCommands::Expire(args) => run_expire(args, global),
    }
}

/// Build a DeviationFilter from CLI ListArgs
fn build_dev_filter(args: &ListArgs) -> DeviationFilter {
    // Map dev status
    let dev_status = match args.status {
        DevStatusFilter::Pending => Some(DevStatus::Pending),
        DevStatusFilter::Approved => Some(DevStatus::Approved),
        DevStatusFilter::Active => Some(DevStatus::Active),
        DevStatusFilter::Expired => Some(DevStatus::Expired),
        DevStatusFilter::Closed => Some(DevStatus::Closed),
        DevStatusFilter::Rejected => Some(DevStatus::Rejected),
        DevStatusFilter::All => None,
    };

    DeviationFilter {
        common: CommonFilter {
            author: args.author.clone(),
            search: args.search.clone(),
            limit: None, // Apply limit after sorting
            ..Default::default()
        },
        dev_status,
        deviation_type: args.dev_type.map(DeviationType::from),
        category: args.category.map(DeviationCategory::from),
        risk_level: args.risk.map(RiskLevel::from),
        active_only: args.active,
        recent_days: if args.recent { Some(30) } else { None },
        sort: build_dev_sort_field(&args.sort),
        sort_direction: if args.reverse {
            SortDirection::Descending
        } else {
            SortDirection::Ascending
        },
    }
}

/// Convert CLI sort column to DeviationSortField
fn build_dev_sort_field(col: &ListColumn) -> DeviationSortField {
    match col {
        ListColumn::Id => DeviationSortField::Id,
        ListColumn::Title => DeviationSortField::Title,
        ListColumn::DevNumber => DeviationSortField::DeviationNumber,
        ListColumn::DevType => DeviationSortField::DeviationType,
        ListColumn::Category => DeviationSortField::Category,
        ListColumn::Risk => DeviationSortField::Risk,
        ListColumn::DevStatus => DeviationSortField::DevStatus,
        ListColumn::Author => DeviationSortField::Author,
        ListColumn::Created => DeviationSortField::Created,
    }
}

/// Sort cached deviations in place
fn sort_cached_deviations(deviations: &mut Vec<CachedDeviation>, args: &ListArgs) {
    match args.sort {
        ListColumn::Id => deviations.sort_by(|a, b| a.id.cmp(&b.id)),
        ListColumn::Title => deviations.sort_by(|a, b| a.title.cmp(&b.title)),
        ListColumn::DevNumber => deviations.sort_by(|a, b| {
            a.deviation_number
                .as_deref()
                .unwrap_or("")
                .cmp(b.deviation_number.as_deref().unwrap_or(""))
        }),
        ListColumn::DevType => deviations.sort_by(|a, b| {
            a.deviation_type
                .as_deref()
                .unwrap_or("")
                .cmp(b.deviation_type.as_deref().unwrap_or(""))
        }),
        ListColumn::Category => deviations.sort_by(|a, b| {
            a.category
                .as_deref()
                .unwrap_or("")
                .cmp(b.category.as_deref().unwrap_or(""))
        }),
        ListColumn::Risk => deviations.sort_by(|a, b| {
            let risk_order = |r: Option<&str>| match r {
                Some("high") => 0,
                Some("medium") => 1,
                Some("low") => 2,
                _ => 3,
            };
            risk_order(a.risk_level.as_deref()).cmp(&risk_order(b.risk_level.as_deref()))
        }),
        ListColumn::DevStatus => deviations.sort_by(|a, b| {
            a.dev_status
                .as_deref()
                .unwrap_or("")
                .cmp(b.dev_status.as_deref().unwrap_or(""))
        }),
        ListColumn::Author => deviations.sort_by(|a, b| a.author.cmp(&b.author)),
        ListColumn::Created => deviations.sort_by(|a, b| a.created.cmp(&b.created)),
    }

    if args.reverse {
        deviations.reverse();
    }

    if let Some(limit) = args.limit {
        deviations.truncate(limit);
    }
}

/// Output cached deviations (fast path for table output)
fn output_cached_deviations(
    deviations: &[CachedDeviation],
    short_ids: &mut ShortIdIndex,
    args: &ListArgs,
    format: OutputFormat,
    project: &Project,
) -> Result<()> {
    if deviations.is_empty() {
        if args.count {
            println!("0");
        } else {
            println!("No deviations found.");
        }
        return Ok(());
    }

    if args.count {
        println!("{}", deviations.len());
        return Ok(());
    }

    // Update short ID index
    short_ids.ensure_all(deviations.iter().map(|d| d.id.clone()));
    super::utils::save_short_ids(short_ids, project);

    match format {
        OutputFormat::Csv => {
            println!("short_id,id,title,dev_number,type,category,risk,dev_status,author");
            for dev in deviations {
                let short_id = short_ids.get_short_id(&dev.id).unwrap_or_default();
                println!(
                    "{},{},{},{},{},{},{},{},{}",
                    short_id,
                    dev.id,
                    escape_csv(&dev.title),
                    dev.deviation_number.as_deref().unwrap_or(""),
                    dev.deviation_type.as_deref().unwrap_or(""),
                    dev.category.as_deref().unwrap_or(""),
                    dev.risk_level.as_deref().unwrap_or(""),
                    dev.dev_status.as_deref().unwrap_or(""),
                    escape_csv(&dev.author)
                );
            }
        }
        OutputFormat::Tsv
        | OutputFormat::Auto
        | OutputFormat::Table
        | OutputFormat::Dot
        | OutputFormat::Tree => {
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
                    ListColumn::Title => ("TITLE", 30),
                    ListColumn::DevNumber => ("DEV #", 14),
                    ListColumn::DevType => ("TYPE", 10),
                    ListColumn::Category => ("CATEGORY", 14),
                    ListColumn::Risk => ("RISK", 8),
                    ListColumn::DevStatus => ("STATUS", 10),
                    ListColumn::Author => ("AUTHOR", 16),
                    ListColumn::Created => ("CREATED", 12),
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

            // Print rows
            for dev in deviations {
                let short_id = short_ids.get_short_id(&dev.id).unwrap_or_default();

                print!("{:<8} ", style(&short_id).cyan());

                for (i, (_, col)) in headers.iter().enumerate() {
                    let value = match col {
                        ListColumn::Id => dev.id.clone(),
                        ListColumn::Title => truncate_str(&dev.title, widths[i]),
                        ListColumn::DevNumber => dev.deviation_number.clone().unwrap_or_default(),
                        ListColumn::DevType => dev.deviation_type.clone().unwrap_or_default(),
                        ListColumn::Category => dev.category.clone().unwrap_or_default(),
                        ListColumn::Risk => {
                            let level = dev.risk_level.as_deref().unwrap_or("");
                            match level {
                                "high" => format!("{}", style(level).red()),
                                "medium" => format!("{}", style(level).yellow()),
                                "low" => format!("{}", style(level).green()),
                                _ => level.to_string(),
                            }
                        }
                        ListColumn::DevStatus => {
                            let status = dev.dev_status.as_deref().unwrap_or("");
                            match status {
                                "active" => format!("{}", style(status).green()),
                                "pending" => format!("{}", style(status).yellow()),
                                "approved" => format!("{}", style(status).cyan()),
                                "expired" | "closed" => format!("{}", style(status).dim()),
                                "rejected" => format!("{}", style(status).red()),
                                _ => status.to_string(),
                            }
                        }
                        ListColumn::Author => dev.author.clone(),
                        ListColumn::Created => dev.created.format("%Y-%m-%d").to_string(),
                    };
                    print!("{:<width$} ", value, width = widths[i]);
                }
                println!();
            }
        }
        OutputFormat::Md => {
            // Markdown table
            let headers: Vec<&str> = args
                .columns
                .iter()
                .map(|c| match c {
                    ListColumn::Id => "ID",
                    ListColumn::Title => "Title",
                    ListColumn::DevNumber => "Number",
                    ListColumn::DevType => "Type",
                    ListColumn::Category => "Category",
                    ListColumn::Risk => "Risk",
                    ListColumn::DevStatus => "Status",
                    ListColumn::Author => "Author",
                    ListColumn::Created => "Created",
                })
                .collect();
            println!("| {} |", headers.join(" | "));
            println!(
                "| {} |",
                headers
                    .iter()
                    .map(|_| "---")
                    .collect::<Vec<_>>()
                    .join(" | ")
            );

            for dev in deviations {
                let short_id = short_ids.get_short_id(&dev.id).unwrap_or_default();
                let values: Vec<String> = args
                    .columns
                    .iter()
                    .map(|c| match c {
                        ListColumn::Id => short_id.clone(),
                        ListColumn::Title => truncate_str(&dev.title, 40),
                        ListColumn::DevNumber => dev.deviation_number.clone().unwrap_or_default(),
                        ListColumn::DevType => dev.deviation_type.clone().unwrap_or_default(),
                        ListColumn::Category => dev.category.clone().unwrap_or_default(),
                        ListColumn::Risk => dev.risk_level.clone().unwrap_or_default(),
                        ListColumn::DevStatus => dev.dev_status.clone().unwrap_or_default(),
                        ListColumn::Author => dev.author.clone(),
                        ListColumn::Created => dev.created.format("%Y-%m-%d").to_string(),
                    })
                    .collect();
                println!("| {} |", values.join(" | "));
            }
        }
        OutputFormat::Id => {
            for dev in deviations {
                println!("{}", dev.id);
            }
        }
        OutputFormat::ShortId => {
            for dev in deviations {
                let short_id = short_ids.get_short_id(&dev.id).unwrap_or_default();
                println!("{}", short_id);
            }
        }
        OutputFormat::Path => {
            for dev in deviations {
                let path = if dev.file_path.is_absolute() {
                    dev.file_path.clone()
                } else {
                    project.root().join(&dev.file_path)
                };
                println!("{}", path.display());
            }
        }
        // JSON/YAML not handled here - they need full entity
        OutputFormat::Json | OutputFormat::Yaml => {
            unreachable!("JSON/YAML output requires full entity load")
        }
    }

    Ok(())
}

/// Output full deviation entities
fn output_deviations(
    deviations: &[Dev],
    short_ids: &mut ShortIdIndex,
    args: &ListArgs,
    format: OutputFormat,
    project: &Project,
) -> Result<()> {
    if deviations.is_empty() {
        if args.count {
            println!("0");
        } else {
            println!("No deviations found.");
        }
        return Ok(());
    }

    if args.count {
        println!("{}", deviations.len());
        return Ok(());
    }

    // Update short ID index
    short_ids.ensure_all(deviations.iter().map(|d| d.id.to_string()));
    super::utils::save_short_ids(short_ids, project);

    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&deviations).into_diagnostic()?;
            println!("{}", json);
        }
        OutputFormat::Yaml => {
            let yaml = serde_yml::to_string(&deviations).into_diagnostic()?;
            print!("{}", yaml);
        }
        OutputFormat::Csv => {
            println!("short_id,id,title,dev_number,type,category,risk,dev_status,author");
            for dev in deviations {
                let short_id = short_ids
                    .get_short_id(&dev.id.to_string())
                    .unwrap_or_default();
                println!(
                    "{},{},{},{},{},{},{},{},{}",
                    short_id,
                    dev.id,
                    escape_csv(&dev.title),
                    dev.deviation_number.as_deref().unwrap_or(""),
                    dev.deviation_type,
                    dev.category,
                    dev.risk.level,
                    dev.dev_status,
                    escape_csv(&dev.author)
                );
            }
        }
        OutputFormat::Tsv
        | OutputFormat::Auto
        | OutputFormat::Table
        | OutputFormat::Dot
        | OutputFormat::Tree => {
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
                    ListColumn::Title => ("TITLE", 30),
                    ListColumn::DevNumber => ("DEV #", 14),
                    ListColumn::DevType => ("TYPE", 10),
                    ListColumn::Category => ("CATEGORY", 14),
                    ListColumn::Risk => ("RISK", 8),
                    ListColumn::DevStatus => ("STATUS", 10),
                    ListColumn::Author => ("AUTHOR", 16),
                    ListColumn::Created => ("CREATED", 12),
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

            // Print rows
            for dev in deviations {
                let short_id = short_ids
                    .get_short_id(&dev.id.to_string())
                    .unwrap_or_default();

                print!("{:<8} ", style(&short_id).cyan());

                for (i, (_, col)) in headers.iter().enumerate() {
                    let value = match col {
                        ListColumn::Id => dev.id.to_string(),
                        ListColumn::Title => truncate_str(&dev.title, widths[i]),
                        ListColumn::DevNumber => dev.deviation_number.clone().unwrap_or_default(),
                        ListColumn::DevType => dev.deviation_type.to_string(),
                        ListColumn::Category => dev.category.to_string(),
                        ListColumn::Risk => {
                            let level = dev.risk.level.to_string();
                            match dev.risk.level {
                                RiskLevel::High => format!("{}", style(level).red()),
                                RiskLevel::Medium => format!("{}", style(level).yellow()),
                                RiskLevel::Low => format!("{}", style(level).green()),
                            }
                        }
                        ListColumn::DevStatus => {
                            let status = dev.dev_status.to_string();
                            match dev.dev_status {
                                DevStatus::Active => format!("{}", style(status).green()),
                                DevStatus::Pending => format!("{}", style(status).yellow()),
                                DevStatus::Approved => format!("{}", style(status).cyan()),
                                DevStatus::Expired | DevStatus::Closed => {
                                    format!("{}", style(status).dim())
                                }
                                DevStatus::Rejected => format!("{}", style(status).red()),
                            }
                        }
                        ListColumn::Author => dev.author.clone(),
                        ListColumn::Created => dev.created.format("%Y-%m-%d").to_string(),
                    };
                    print!("{:<width$} ", value, width = widths[i]);
                }
                println!();
            }
        }
        OutputFormat::Md => {
            // Markdown table
            let headers: Vec<&str> = args
                .columns
                .iter()
                .map(|c| match c {
                    ListColumn::Id => "ID",
                    ListColumn::Title => "Title",
                    ListColumn::DevNumber => "Number",
                    ListColumn::DevType => "Type",
                    ListColumn::Category => "Category",
                    ListColumn::Risk => "Risk",
                    ListColumn::DevStatus => "Status",
                    ListColumn::Author => "Author",
                    ListColumn::Created => "Created",
                })
                .collect();
            println!("| {} |", headers.join(" | "));
            println!(
                "| {} |",
                headers
                    .iter()
                    .map(|_| "---")
                    .collect::<Vec<_>>()
                    .join(" | ")
            );

            for dev in deviations {
                let short_id = short_ids
                    .get_short_id(&dev.id.to_string())
                    .unwrap_or_default();
                let values: Vec<String> = args
                    .columns
                    .iter()
                    .map(|c| match c {
                        ListColumn::Id => short_id.clone(),
                        ListColumn::Title => truncate_str(&dev.title, 40),
                        ListColumn::DevNumber => dev.deviation_number.clone().unwrap_or_default(),
                        ListColumn::DevType => dev.deviation_type.to_string(),
                        ListColumn::Category => dev.category.to_string(),
                        ListColumn::Risk => dev.risk.level.to_string(),
                        ListColumn::DevStatus => dev.dev_status.to_string(),
                        ListColumn::Author => dev.author.clone(),
                        ListColumn::Created => dev.created.format("%Y-%m-%d").to_string(),
                    })
                    .collect();
                println!("| {} |", values.join(" | "));
            }
        }
        OutputFormat::Id => {
            for dev in deviations {
                println!("{}", dev.id);
            }
        }
        OutputFormat::ShortId => {
            for dev in deviations {
                let short_id = short_ids
                    .get_short_id(&dev.id.to_string())
                    .unwrap_or_default();
                println!("{}", short_id);
            }
        }
        OutputFormat::Path => {
            let dev_dir = project.root().join("manufacturing/deviations");
            for dev in deviations {
                let path = dev_dir.join(format!("{}.tdt.yaml", dev.id));
                println!("{}", path.display());
            }
        }
    }

    Ok(())
}

/// List deviations
fn run_list(args: ListArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let cache = EntityCache::open(&project).map_err(|e| miette::miette!("{}", e))?;
    let service = DeviationService::new(&project, &cache);
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
    let filter = build_dev_filter(&args);

    // Two-tier caching: use cache for fast path when possible
    // Can't use cache for: JSON/YAML output (need full entity data)
    // Cache CAN handle: dev_status, deviation_type, category, risk_level, active_only, recent_days
    let can_use_cache = !matches!(format, OutputFormat::Json | OutputFormat::Yaml);

    if can_use_cache {
        // Fast path: use cache
        let mut cached_deviations = service
            .list_cached(&filter)
            .map_err(|e| miette::miette!("{}", e))?;

        // Apply linked-to filter
        if let Some(ref ids) = allowed_ids {
            cached_deviations.retain(|e| ids.contains(&e.id));
        }

        // Sort and limit
        sort_cached_deviations(&mut cached_deviations, &args);

        return output_cached_deviations(
            &cached_deviations,
            &mut short_ids,
            &args,
            format,
            &project,
        );
    }

    // Full entity loading path (JSON/YAML output)
    let mut deviations = service
        .list(&filter)
        .map_err(|e| miette::miette!("{}", e))?;

    // Apply linked-to filter
    if let Some(ref ids) = allowed_ids {
        deviations.retain(|e| ids.contains(&e.id.to_string()));
    }

    // Apply limit
    if let Some(limit) = args.limit {
        deviations.truncate(limit);
    }

    output_deviations(&deviations, &mut short_ids, &args, format, &project)
}

/// Create a new deviation
fn run_new(args: NewArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let cache = EntityCache::open(&project).map_err(|e| miette::miette!("{}", e))?;
    let config = Config::load();
    let service = DeviationService::new(&project, &cache);

    let title: String;
    let deviation_number: Option<String>;
    let dev_type: DeviationType;
    let category: DeviationCategory;
    let risk_level: RiskLevel;

    if args.interactive {
        let wizard = SchemaWizard::new();
        let result = wizard.run(EntityPrefix::Dev)?;

        title = result
            .get_string("title")
            .map(String::from)
            .unwrap_or_else(|| "New Deviation".to_string());
        deviation_number = result.get_string("deviation_number").map(String::from);
        dev_type = result
            .get_string("deviation_type")
            .and_then(|s| s.parse().ok())
            .unwrap_or(DeviationType::Temporary);
        category = result
            .get_string("category")
            .and_then(|s| s.parse().ok())
            .unwrap_or(DeviationCategory::Material);
        risk_level = result
            .get_string("risk_level")
            .and_then(|s| s.parse().ok())
            .unwrap_or(RiskLevel::Low);
    } else {
        title = args.title.unwrap_or_else(|| "New Deviation".to_string());
        deviation_number = args.deviation_number;
        dev_type = DeviationType::from(args.dev_type);
        category = DeviationCategory::from(args.category);
        risk_level = RiskLevel::from(args.risk);
    }

    // Create deviation via service
    let input = CreateDeviation {
        title: title.clone(),
        deviation_number,
        deviation_type: dev_type,
        category,
        description: None,
        risk_level,
        risk_assessment: None,
        effective_date: None,
        expiration_date: None,
        notes: None,
        status: None,
        author: config.author(),
    };

    let dev = service
        .create(input)
        .map_err(|e| miette::miette!("{}", e))?;

    // Get file path for the created deviation
    let file_path = project
        .root()
        .join("manufacturing/deviations")
        .join(format!("{}.tdt.yaml", dev.id));

    // Add to short ID index
    let mut short_ids = ShortIdIndex::load(&project);
    let short_id = short_ids.add(dev.id.to_string());
    super::utils::save_short_ids(&mut short_ids, &project);

    // Handle --link flags
    let _added_links = crate::cli::entity_cmd::process_link_flags(
        &file_path,
        EntityPrefix::Dev,
        &args.link,
        &short_ids,
    );

    // Output
    if !global.quiet {
        let id_str = dev.id.to_string();
        let display_id = short_id.as_deref().unwrap_or(&id_str);
        println!(
            "{} Created deviation {}",
            style("✓").green(),
            style(display_id).cyan()
        );
        println!("  {}", file_path.display());
    }

    // Sync cache after creation
    super::utils::sync_cache(&project);

    // Open in editor if requested
    if args.edit && !args.no_edit {
        println!(
            "Opening {} in {}...",
            style(file_path.display()).cyan(),
            style(config.editor()).yellow()
        );
        config.run_editor(&file_path).into_diagnostic()?;
    }

    Ok(())
}

/// Show deviation details
fn run_show(args: ShowArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let cache = EntityCache::open(&project).map_err(|e| miette::miette!("{}", e))?;

    // Resolve short ID if needed
    let short_ids = ShortIdIndex::load(&project);
    let resolved_id = short_ids
        .resolve(&args.id)
        .unwrap_or_else(|| args.id.clone());

    // Use DeviationService to get the deviation (cache-first lookup)
    let service = DeviationService::new(&project, &cache);
    let dev = service
        .get(&resolved_id)
        .map_err(|e| miette::miette!("{}", e))?
        .ok_or_else(|| miette::miette!("No deviation found matching '{}'", args.id))?;

    let format = match global.output {
        OutputFormat::Auto => OutputFormat::Tsv,
        f => f,
    };

    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&dev).into_diagnostic()?;
            println!("{}", json);
        }
        OutputFormat::Yaml => {
            let yaml = serde_yml::to_string(&dev).into_diagnostic()?;
            print!("{}", yaml);
        }
        OutputFormat::Csv => {
            let short_id = short_ids
                .get_short_id(&dev.id.to_string())
                .unwrap_or_default();
            println!("id,title,type,category,risk,dev_status,author,created");
            println!(
                "{},{},{},{},{},{},{},{}",
                escape_csv(&short_id),
                escape_csv(&dev.title),
                escape_csv(&dev.deviation_type.to_string()),
                escape_csv(&dev.category.to_string()),
                escape_csv(&dev.risk.level.to_string()),
                escape_csv(&dev.dev_status.to_string()),
                escape_csv(&dev.author),
                dev.created.format("%Y-%m-%d")
            );
        }
        OutputFormat::Tsv
        | OutputFormat::Auto
        | OutputFormat::Md
        | OutputFormat::Table
        | OutputFormat::Dot
        | OutputFormat::Tree => {
            let short_id = short_ids
                .get_short_id(&dev.id.to_string())
                .unwrap_or_default();

            println!("{}", style("Deviation").bold());
            println!("{}", style("─".repeat(60)).dim());
            println!("  {} {}", style("ID:").dim(), style(&short_id).cyan());
            println!("  {} {}", style("Title:").dim(), dev.title);

            if let Some(ref num) = dev.deviation_number {
                println!("  {} {}", style("Number:").dim(), num);
            }

            println!("  {} {}", style("Type:").dim(), dev.deviation_type);
            println!("  {} {}", style("Category:").dim(), dev.category);

            let status_style = match dev.dev_status {
                DevStatus::Active => style(dev.dev_status.to_string()).green(),
                DevStatus::Pending => style(dev.dev_status.to_string()).yellow(),
                DevStatus::Approved => style(dev.dev_status.to_string()).cyan(),
                DevStatus::Expired | DevStatus::Closed => style(dev.dev_status.to_string()).dim(),
                DevStatus::Rejected => style(dev.dev_status.to_string()).red(),
            };
            println!("  {} {}", style("Status:").dim(), status_style);

            println!();
            println!("{}", style("Risk Assessment").bold());
            println!("{}", style("─".repeat(60)).dim());
            let risk_style = match dev.risk.level {
                RiskLevel::High => style(dev.risk.level.to_string()).red(),
                RiskLevel::Medium => style(dev.risk.level.to_string()).yellow(),
                RiskLevel::Low => style(dev.risk.level.to_string()).green(),
            };
            println!("  {} {}", style("Level:").dim(), risk_style);
            if let Some(ref assessment) = dev.risk.assessment {
                println!("  {} {}", style("Assessment:").dim(), assessment);
            }
            if !dev.risk.mitigations.is_empty() {
                println!("  {}", style("Mitigations:").dim());
                for m in &dev.risk.mitigations {
                    println!("    - {}", m);
                }
            }

            if dev.approval.approved_by.is_some() || dev.approval.approval_date.is_some() {
                println!();
                println!("{}", style("Approval").bold());
                println!("{}", style("─".repeat(60)).dim());
                if let Some(ref by) = dev.approval.approved_by {
                    println!("  {} {}", style("Approved By:").dim(), by);
                }
                if let Some(ref date) = dev.approval.approval_date {
                    println!("  {} {}", style("Date:").dim(), date);
                }
                if let Some(ref level) = dev.approval.authorization_level {
                    println!("  {} {}", style("Authorization:").dim(), level);
                }
            }

            if dev.effective_date.is_some() || dev.expiration_date.is_some() {
                println!();
                println!("{}", style("Timing").bold());
                println!("{}", style("─".repeat(60)).dim());
                if let Some(ref date) = dev.effective_date {
                    println!("  {} {}", style("Effective:").dim(), date);
                }
                if let Some(ref date) = dev.expiration_date {
                    println!("  {} {}", style("Expires:").dim(), date);
                }
            }

            if let Some(ref desc) = dev.description {
                println!();
                println!("{}", style("Description").bold());
                println!("{}", style("─".repeat(60)).dim());
                for line in desc.lines() {
                    println!("  {}", line);
                }
            }

            println!();
            println!("{}", style("─".repeat(60)).dim());
            println!(
                "{}: {} | {}: {} | {}: {}",
                style("Author").dim(),
                dev.author,
                style("Created").dim(),
                dev.created.format("%Y-%m-%d %H:%M"),
                style("Revision").dim(),
                dev.entity_revision
            );
        }
        OutputFormat::Id => {
            println!("{}", dev.id);
        }
        OutputFormat::ShortId => {
            let short_id = short_ids
                .get_short_id(&dev.id.to_string())
                .unwrap_or_default();
            println!("{}", short_id);
        }
        OutputFormat::Path => {
            // Get file path from cache
            if let Some(cached) = cache.get_entity(&dev.id.to_string()) {
                let path = if cached.file_path.is_absolute() {
                    cached.file_path.clone()
                } else {
                    project.root().join(&cached.file_path)
                };
                println!("{}", path.display());
            } else {
                return Err(miette::miette!("Could not find file path for deviation"));
            }
        }
    }

    Ok(())
}

/// Edit a deviation
fn run_edit(args: EditArgs) -> Result<()> {
    crate::cli::entity_cmd::run_edit_generic(&args.id, &ENTITY_CONFIG)
}

/// Delete a deviation
fn run_delete(args: DeleteArgs) -> Result<()> {
    crate::cli::commands::utils::run_delete(&args.id, DEV_DIRS, args.force, false, args.quiet)
}

/// Archive a deviation
fn run_archive(args: ArchiveArgs) -> Result<()> {
    crate::cli::commands::utils::run_delete(&args.id, DEV_DIRS, args.force, true, args.quiet)
}

/// Approve a deviation
fn run_approve(args: ApproveArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let cache = EntityCache::open(&project).map_err(|e| miette::miette!("{}", e))?;
    let guard = tdt_core::services::WorkflowGuard::load(&project);
    let service = DeviationService::new(&project, &cache).with_workflow(guard);
    let config = Config::load();
    let short_ids = ShortIdIndex::load(&project);

    // Resolve short ID if needed
    let resolved_id = short_ids
        .resolve(&args.id)
        .unwrap_or_else(|| args.id.clone());

    // Get approver
    let approved_by = args.approved_by.unwrap_or_else(|| config.author());

    // Confirm
    if !args.yes && !global.quiet {
        let short_id = short_ids
            .get_short_id(&resolved_id)
            .unwrap_or_else(|| args.id.clone());
        println!(
            "Approve deviation {} by {}?",
            style(&short_id).cyan(),
            style(&approved_by).cyan()
        );
        print!("Continue? [y/N] ");
        use std::io::{self, Write};
        io::stdout().flush().into_diagnostic()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input).into_diagnostic()?;
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Aborted.");
            return Ok(());
        }
    }

    // Use service to approve
    let authorization_level = AuthorizationLevel::from(args.authorization);
    let dev = service
        .approve(
            &resolved_id,
            approved_by.clone(),
            authorization_level,
            args.activate,
        )
        .map_err(|e| miette::miette!("{}", e))?;

    if !global.quiet {
        let short_id = short_ids
            .get_short_id(&dev.id.to_string())
            .unwrap_or_default();
        println!(
            "{} Approved deviation {} by {}",
            style("✓").green(),
            style(&short_id).cyan(),
            style(&approved_by).cyan()
        );
        if args.activate {
            println!("  Status: {}", style("active").green());
        } else {
            println!("  Status: {}", style("approved").cyan());
        }
    }

    // Sync cache after mutation
    super::utils::sync_cache(&project);

    Ok(())
}

/// Expire/close a deviation
fn run_expire(args: ExpireArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let cache = EntityCache::open(&project).map_err(|e| miette::miette!("{}", e))?;
    let service = DeviationService::new(&project, &cache);
    let short_ids = ShortIdIndex::load(&project);

    // Resolve short ID if needed
    let resolved_id = short_ids
        .resolve(&args.id)
        .unwrap_or_else(|| args.id.clone());

    // Confirm
    if !args.yes && !global.quiet {
        let short_id = short_ids
            .get_short_id(&resolved_id)
            .unwrap_or_else(|| args.id.clone());
        println!("Close deviation {}?", style(&short_id).cyan());
        print!("Continue? [y/N] ");
        use std::io::{self, Write};
        io::stdout().flush().into_diagnostic()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input).into_diagnostic()?;
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Aborted.");
            return Ok(());
        }
    }

    // Use service to close
    let dev = service
        .close(&resolved_id, args.reason)
        .map_err(|e| miette::miette!("{}", e))?;

    if !global.quiet {
        let short_id = short_ids
            .get_short_id(&dev.id.to_string())
            .unwrap_or_default();
        println!(
            "{} Closed deviation {}",
            style("✓").green(),
            style(&short_id).cyan()
        );
    }

    // Sync cache after mutation
    super::utils::sync_cache(&project);

    Ok(())
}
