//! `tdt risk` command - Risk/FMEA management

use clap::{Subcommand, ValueEnum};
use console::style;
use miette::{IntoDiagnostic, Result};

use crate::cli::commands::utils::format_link_with_title;
use crate::cli::filters::StatusFilter;
use crate::cli::helpers::{format_short_id, truncate_str};
use crate::cli::table::{CellValue, ColumnDef, TableConfig, TableFormatter, TableRow};
use crate::cli::{GlobalOpts, OutputFormat};
use tdt_core::core::cache::EntityCache;
use tdt_core::core::identity::{EntityId, EntityPrefix};
use tdt_core::core::project::Project;
use tdt_core::core::shortid::ShortIdIndex;
use tdt_core::core::Config;
use tdt_core::entities::risk::{Risk, RiskLevel, RiskType};
use tdt_core::schema::wizard::SchemaWizard;
use tdt_core::services::{
    CommonFilter, CreateRisk, RiskFilter, RiskService, RiskSortField, SortDirection,
};

/// CLI-friendly risk type enum
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CliRiskType {
    Design,
    Process,
    Use,
    Software,
}

impl std::fmt::Display for CliRiskType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CliRiskType::Design => write!(f, "design"),
            CliRiskType::Process => write!(f, "process"),
            CliRiskType::Use => write!(f, "use"),
            CliRiskType::Software => write!(f, "software"),
        }
    }
}

impl From<CliRiskType> for RiskType {
    fn from(cli: CliRiskType) -> Self {
        match cli {
            CliRiskType::Design => RiskType::Design,
            CliRiskType::Process => RiskType::Process,
            CliRiskType::Use => RiskType::Use,
            CliRiskType::Software => RiskType::Software,
        }
    }
}

#[derive(Subcommand, Debug)]
pub enum RiskCommands {
    /// List risks with filtering
    List(ListArgs),

    /// Create a new risk
    New(NewArgs),

    /// Show a risk's details
    Show(ShowArgs),

    /// Edit a risk in your editor
    Edit(EditArgs),

    /// Delete a risk
    Delete(DeleteArgs),

    /// Archive a risk (move to .tdt/archive/)
    Archive(ArchiveArgs),

    /// Show risk statistics summary
    Summary(SummaryArgs),

    /// Display severity × occurrence risk matrix
    Matrix(MatrixArgs),
}

/// Risk type filter
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum RiskTypeFilter {
    Design,
    Process,
    Use,
    Software,
    All,
}

/// Risk level filter
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum RiskLevelFilter {
    Low,
    Medium,
    High,
    Critical,
    /// High and critical only
    Urgent,
    /// All levels
    All,
}

crate::list_columns! {
    /// Columns to display in list output
    pub enum ListColumn {
        Id => "id",
        Type => "type",
        Title => "title",
        Status => "status",
        RiskLevel => "risk-level",
        Severity => "severity",
        Occurrence => "occurrence",
        Detection => "detection",
        Rpn => "rpn",
        Category => "category",
        Author => "author",
        Created => "created",
    }
}

/// Column definitions for risk list output
const RISK_COLUMNS: &[ColumnDef] = &[
    ColumnDef::new("id", "ID", 17),
    ColumnDef::new("type", "TYPE", 10),
    ColumnDef::new("title", "TITLE", 30),
    ColumnDef::new("status", "STATUS", 10),
    ColumnDef::new("risk-level", "LEVEL", 10),
    ColumnDef::new("severity", "SEV", 5),
    ColumnDef::new("occurrence", "OCC", 5),
    ColumnDef::new("detection", "DET", 5),
    ColumnDef::new("rpn", "RPN", 5),
    ColumnDef::new("category", "CATEGORY", 12),
    ColumnDef::new("author", "AUTHOR", 16),
    ColumnDef::new("created", "CREATED", 12),
];

#[derive(clap::Args, Debug)]
pub struct ListArgs {
    /// Filter by type
    #[arg(long, short = 't', default_value = "all")]
    pub r#type: RiskTypeFilter,

    /// Filter by status
    #[arg(long, short = 's', default_value = "all")]
    pub status: StatusFilter,

    /// Filter by risk level
    #[arg(long, short = 'l', default_value = "all")]
    pub level: RiskLevelFilter,

    /// Filter by category (case-insensitive)
    #[arg(long, short = 'c')]
    pub category: Option<String>,

    /// Filter by tag (case-insensitive)
    #[arg(long)]
    pub tag: Option<String>,

    /// Filter by minimum RPN
    #[arg(long)]
    pub min_rpn: Option<u16>,

    /// Filter by maximum RPN
    #[arg(long)]
    pub max_rpn: Option<u16>,

    /// Show risks above this RPN threshold (alias for --min-rpn)
    #[arg(long, value_name = "N")]
    pub above_rpn: Option<u16>,

    /// Filter by author (substring match)
    #[arg(long, short = 'a')]
    pub author: Option<String>,

    /// Search in title and description (case-insensitive substring)
    #[arg(long)]
    pub search: Option<String>,

    /// Show only risks without mitigations
    #[arg(long)]
    pub unmitigated: bool,

    /// Show risks with incomplete mitigations (not all verified/completed)
    #[arg(long)]
    pub open_mitigations: bool,

    /// Show only critical risks (shortcut for --level critical)
    #[arg(long)]
    pub critical: bool,

    /// Show risks created in last N days
    #[arg(long)]
    pub recent: Option<u32>,

    /// Columns to display (can specify multiple)
    #[arg(long, value_delimiter = ',', default_values_t = vec![
        ListColumn::Type,
        ListColumn::Title,
        ListColumn::Status,
        ListColumn::RiskLevel,
        ListColumn::Rpn
    ])]
    pub columns: Vec<ListColumn>,

    /// Sort by field (default: created)
    #[arg(long, default_value = "created")]
    pub sort: ListColumn,

    /// Reverse sort order
    #[arg(long, short = 'r')]
    pub reverse: bool,

    /// Sort by RPN (highest first) - shorthand for --sort rpn --reverse
    #[arg(long)]
    pub by_rpn: bool,

    /// Limit output to N items
    #[arg(long, short = 'n')]
    pub limit: Option<usize>,

    /// Show count only, not the items
    #[arg(long)]
    pub count: bool,

    /// Wrap text at specified width (for mobile-friendly display)
    #[arg(long, short = 'w')]
    pub wrap: Option<usize>,

    /// Show full ID column (hidden by default since SHORT is always shown)
    #[arg(long)]
    pub show_id: bool,

    /// Only show entities linked to these IDs (use - for stdin pipe)
    #[arg(long, value_delimiter = ',')]
    pub linked_to: Vec<String>,

    /// Filter by link type when using --linked-to (e.g., verified_by, satisfied_by)
    #[arg(long, requires = "linked_to")]
    pub via: Option<String>,
}

#[derive(clap::Args, Debug)]
pub struct NewArgs {
    /// Risk type
    #[arg(long, short = 't', default_value = "design")]
    pub r#type: CliRiskType,

    /// Title (if not provided, uses placeholder)
    #[arg(long, short = 'T')]
    pub title: Option<String>,

    /// Category
    #[arg(long, short = 'c')]
    pub category: Option<String>,

    /// Initial severity rating (1-10)
    #[arg(long, short = 'S')]
    pub severity: Option<u8>,

    /// Initial occurrence rating (1-10)
    #[arg(long, short = 'O')]
    pub occurrence: Option<u8>,

    /// Initial detection rating (1-10)
    #[arg(long, short = 'D')]
    pub detection: Option<u8>,

    /// Risk description - detailed explanation of the risk and its consequences
    #[arg(long, short = 'd')]
    pub description: Option<String>,

    /// Use interactive wizard to fill in fields
    #[arg(long, short = 'i')]
    pub interactive: bool,

    /// Open in editor after creation
    #[arg(long, short = 'e')]
    pub edit: bool,

    /// Don't open in editor after creation
    #[arg(long, short = 'n')]
    pub no_edit: bool,

    /// Link to another entity (auto-infers link type)
    #[arg(long, short = 'L')]
    pub link: Vec<String>,
}

#[derive(clap::Args, Debug)]
pub struct ShowArgs {
    /// Risk ID or fuzzy search term
    pub id: String,

    /// Show linked entities too
    #[arg(long)]
    pub with_links: bool,
}

#[derive(clap::Args, Debug)]
pub struct EditArgs {
    /// Risk ID or fuzzy search term
    pub id: String,
}

#[derive(clap::Args, Debug)]
pub struct DeleteArgs {
    /// Risk ID or short ID (RISK@N)
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
    /// Risk ID or short ID (RISK@N)
    pub id: String,

    /// Force archive even if other entities reference this one
    #[arg(long)]
    pub force: bool,

    /// Suppress output
    #[arg(long, short = 'q')]
    pub quiet: bool,
}

/// Search directories for this entity type — single source of truth
/// lives in `Project::entity_search_directories`.
const RISK_DIRS: &[&str] = tdt_core::core::project::Project::entity_search_directories(
    tdt_core::core::identity::EntityPrefix::Risk,
);

#[derive(clap::Args, Debug)]
pub struct SummaryArgs {
    /// Show top N risks by RPN (default: 5)
    #[arg(long, short = 'n', default_value = "5")]
    pub top: usize,

    /// Include detailed breakdown by category
    #[arg(long)]
    pub detailed: bool,
}

#[derive(clap::Args, Debug)]
pub struct MatrixArgs {
    /// Filter by risk type (design, process)
    #[arg(long, short = 't')]
    pub risk_type: Option<CliRiskType>,

    /// Show risk IDs in cells instead of counts
    #[arg(long)]
    pub show_ids: bool,

    /// Use compact 5×5 matrix instead of 10×10
    #[arg(long)]
    pub compact: bool,
}

pub fn run(cmd: RiskCommands, global: &GlobalOpts) -> Result<()> {
    match cmd {
        RiskCommands::List(args) => run_list(args, global),
        RiskCommands::New(args) => run_new(args, global),
        RiskCommands::Show(args) => run_show(args, global),
        RiskCommands::Edit(args) => run_edit(args),
        RiskCommands::Delete(args) => run_delete(args),
        RiskCommands::Archive(args) => run_archive(args),
        RiskCommands::Summary(args) => run_summary(args, global),
        RiskCommands::Matrix(args) => run_matrix(args, global),
    }
}

fn run_delete(args: DeleteArgs) -> Result<()> {
    crate::cli::commands::utils::run_delete(&args.id, RISK_DIRS, args.force, false, args.quiet)
}

fn run_archive(args: ArchiveArgs) -> Result<()> {
    crate::cli::commands::utils::run_delete(&args.id, RISK_DIRS, args.force, true, args.quiet)
}

fn run_list(args: ListArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let cache = EntityCache::open(&project).map_err(|e| miette::miette!("{}", e))?;
    let service = RiskService::new(&project, &cache);

    // Resolve linked-to filter via cache
    let short_ids = ShortIdIndex::load(&project);
    let allowed_ids = crate::cli::helpers::resolve_linked_to(
        &args.linked_to,
        args.via.as_deref(),
        &short_ids,
        &cache,
    );

    // Build filter from CLI args
    let filter = build_risk_filter(&args);

    // Build sort options
    let (sort_field, sort_dir) = build_risk_sort(&args);

    // Get results from service
    let result = service
        .list(&filter, sort_field, sort_dir)
        .map_err(|e| miette::miette!("{}", e))?;

    let mut risks = result.items;

    // Apply linked-to filter
    if let Some(ref ids) = allowed_ids {
        risks.retain(|e| ids.contains(&e.id.to_string()));
    }

    if risks.is_empty() {
        match global.output {
            OutputFormat::Json => println!("[]"),
            OutputFormat::Yaml => println!("[]"),
            _ => {
                println!("No risks found.");
                println!();
                println!("Create one with: {}", style("tdt risk new").yellow());
            }
        }
        return Ok(());
    }

    // Just count?
    if args.count {
        println!("{}", result.total_count);
        return Ok(());
    }

    // Update short ID index with current risks
    let mut short_ids = ShortIdIndex::load(&project);
    short_ids.ensure_all(risks.iter().map(|r| r.id.to_string()));
    super::utils::save_short_ids(&mut short_ids, &project);

    // Output based on format
    let format = global.list_format();

    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&risks).into_diagnostic()?;
            println!("{}", json);
        }
        OutputFormat::Yaml => {
            let yaml = serde_yml::to_string(&risks).into_diagnostic()?;
            print!("{}", yaml);
        }
        OutputFormat::Tsv
        | OutputFormat::Csv
        | OutputFormat::Md
        | OutputFormat::Id
        | OutputFormat::ShortId
        | OutputFormat::Table
        | OutputFormat::Dot
        | OutputFormat::Tree => {
            // Build visible columns list
            let mut visible: Vec<&str> = args
                .columns
                .iter()
                .map(|c| c.to_string().leak() as &str)
                .collect();
            if args.show_id && !visible.contains(&"id") {
                visible.insert(0, "id");
            }

            // Convert to TableRows
            let rows: Vec<TableRow> = risks
                .iter()
                .map(|risk| risk_to_row(risk, &short_ids))
                .collect();

            // Configure table
            let config = if let Some(width) = args.wrap {
                TableConfig::with_wrap(width)
            } else {
                TableConfig::default()
            };

            let formatter = TableFormatter::new(RISK_COLUMNS, "risk", "RISK").with_config(config);
            formatter.output(rows, format, &visible);
        }
        OutputFormat::Auto | OutputFormat::Path => {
            eprintln!("error: -o path is not supported for this view; use -o id to get entity IDs");
            std::process::exit(2);
        }
    }

    Ok(())
}

/// Convert CLI ListArgs to RiskFilter for service
fn build_risk_filter(args: &ListArgs) -> RiskFilter {
    use tdt_core::core::entity::Status;

    // Convert status filter
    let status = match args.status {
        StatusFilter::Draft => Some(vec![Status::Draft]),
        StatusFilter::Review => Some(vec![Status::Review]),
        StatusFilter::Approved => Some(vec![Status::Approved]),
        StatusFilter::Released => Some(vec![Status::Released]),
        StatusFilter::Obsolete => Some(vec![Status::Obsolete]),
        StatusFilter::Active => Some(vec![
            Status::Draft,
            Status::Review,
            Status::Approved,
            Status::Released,
        ]),
        StatusFilter::All => None,
    };

    // Convert risk type filter
    let risk_type = match args.r#type {
        RiskTypeFilter::Design => Some(RiskType::Design),
        RiskTypeFilter::Process => Some(RiskType::Process),
        RiskTypeFilter::Use => Some(RiskType::Use),
        RiskTypeFilter::Software => Some(RiskType::Software),
        RiskTypeFilter::All => None,
    };

    // Convert risk level filter (critical flag overrides)
    let risk_level = if args.critical {
        Some(vec![RiskLevel::Critical])
    } else {
        match args.level {
            RiskLevelFilter::Low => Some(vec![RiskLevel::Low]),
            RiskLevelFilter::Medium => Some(vec![RiskLevel::Medium]),
            RiskLevelFilter::High => Some(vec![RiskLevel::High]),
            RiskLevelFilter::Critical => Some(vec![RiskLevel::Critical]),
            RiskLevelFilter::Urgent => Some(vec![RiskLevel::High, RiskLevel::Critical]),
            RiskLevelFilter::All => None,
        }
    };

    RiskFilter {
        common: CommonFilter {
            status,
            author: args.author.clone(),
            tags: args.tag.clone().map(|t| vec![t]),
            search: args.search.clone(),
            recent_days: args.recent,
            limit: args.limit,
            ..Default::default()
        },
        risk_type,
        risk_level,
        min_rpn: args.above_rpn.or(args.min_rpn),
        max_rpn: args.max_rpn,
        unmitigated_only: args.unmitigated,
        needs_mitigation: args.open_mitigations,
        category: args.category.clone(),
        ..Default::default()
    }
}

/// Convert CLI sort args to service sort options
fn build_risk_sort(args: &ListArgs) -> (RiskSortField, SortDirection) {
    let field = if args.by_rpn {
        RiskSortField::Rpn
    } else {
        match args.sort {
            ListColumn::Id => RiskSortField::Id,
            ListColumn::Type => RiskSortField::Type,
            ListColumn::Title => RiskSortField::Title,
            ListColumn::Status => RiskSortField::Status,
            ListColumn::RiskLevel => RiskSortField::RiskLevel,
            ListColumn::Severity => RiskSortField::Severity,
            ListColumn::Occurrence => RiskSortField::Occurrence,
            ListColumn::Detection => RiskSortField::Detection,
            ListColumn::Rpn => RiskSortField::Rpn,
            ListColumn::Category => RiskSortField::Category,
            ListColumn::Author => RiskSortField::Author,
            ListColumn::Created => RiskSortField::Created,
        }
    };

    // RPN and severity-like fields default to descending
    let dir = if args.reverse {
        SortDirection::Ascending
    } else {
        match field {
            RiskSortField::Rpn
            | RiskSortField::Severity
            | RiskSortField::Occurrence
            | RiskSortField::Detection
            | RiskSortField::RiskLevel => SortDirection::Descending,
            _ => SortDirection::Ascending,
        }
    };

    (field, dir)
}

/// Convert a Risk to a TableRow
fn risk_to_row(risk: &Risk, short_ids: &ShortIdIndex) -> TableRow {
    TableRow::new(risk.id.to_string(), short_ids)
        .cell("id", CellValue::Id(risk.id.to_string()))
        .cell("type", CellValue::Type(risk.risk_type.to_string()))
        .cell("title", CellValue::Text(risk.title.clone()))
        .cell("status", CellValue::Status(risk.status))
        .cell(
            "risk-level",
            CellValue::Text(
                risk.get_risk_level()
                    .map_or("-".to_string(), |l| l.to_string()),
            ),
        )
        .cell(
            "severity",
            CellValue::Text(risk.severity.map_or("-".to_string(), |s| s.to_string())),
        )
        .cell(
            "occurrence",
            CellValue::Text(risk.occurrence.map_or("-".to_string(), |o| o.to_string())),
        )
        .cell(
            "detection",
            CellValue::Text(risk.detection.map_or("-".to_string(), |d| d.to_string())),
        )
        .cell(
            "rpn",
            CellValue::Text(risk.get_rpn().map_or("-".to_string(), |r| r.to_string())),
        )
        .cell(
            "category",
            CellValue::Text(risk.category.clone().unwrap_or_default()),
        )
        .cell("author", CellValue::Text(risk.author.clone()))
        .cell("created", CellValue::Date(risk.created))
}

fn run_new(args: NewArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let cache = EntityCache::open(&project).map_err(|e| miette::miette!("{}", e))?;
    let service = RiskService::new(&project, &cache);
    let config = Config::load();

    // Determine values - either from schema-driven wizard or args
    let (
        risk_type,
        title,
        category,
        severity,
        occurrence,
        detection,
        description,
        failure_mode,
        cause,
        effect,
    ) = if args.interactive {
        // Use the schema-driven wizard
        let wizard = SchemaWizard::new();
        let result = wizard.run(EntityPrefix::Risk)?;

        let risk_type = result
            .get_string("type")
            .map(|s| match s {
                "process" => RiskType::Process,
                "use" => RiskType::Use,
                "software" => RiskType::Software,
                _ => RiskType::Design,
            })
            .unwrap_or(RiskType::Design);

        let title = result
            .get_string("title")
            .map(String::from)
            .unwrap_or_else(|| "New Risk".to_string());

        let category = result.get_string("category").map(String::from);

        let severity = result.get_i64("severity").map(|n| n as u8);

        let occurrence = result.get_i64("occurrence").map(|n| n as u8);

        let detection = result.get_i64("detection").map(|n| n as u8);

        // Extract FMEA text fields
        let description = result
            .get_string("description")
            .map(String::from)
            .unwrap_or_default();
        let failure_mode = result.get_string("failure_mode").map(String::from);
        let cause = result.get_string("cause").map(String::from);
        let effect = result.get_string("effect").map(String::from);

        (
            risk_type,
            title,
            category,
            severity,
            occurrence,
            detection,
            description,
            failure_mode,
            cause,
            effect,
        )
    } else {
        // Default mode - use args with defaults
        let risk_type: RiskType = args.r#type.into();
        let title = args.title.unwrap_or_else(|| "New Risk".to_string());
        let category = args.category;
        let severity = args.severity;
        let occurrence = args.occurrence;
        let detection = args.detection;
        let description = args.description.unwrap_or_default();

        (
            risk_type,
            title,
            category,
            severity,
            occurrence,
            detection,
            description,
            None,
            None,
            None,
        )
    };

    // Create risk using service
    let input = CreateRisk {
        risk_type,
        title: title.clone(),
        description,
        author: config.author(),
        category,
        failure_mode,
        cause,
        effect,
        severity,
        occurrence,
        detection,
        ..Default::default()
    };

    let risk = service
        .create(input)
        .map_err(|e| miette::miette!("{}", e))?;

    // Calculate RPN for display
    let rpn = risk.rpn.unwrap_or(0);
    let risk_level = risk.risk_level.as_ref().map_or("unknown", |l| match l {
        RiskLevel::Low => "low",
        RiskLevel::Medium => "medium",
        RiskLevel::High => "high",
        RiskLevel::Critical => "critical",
    });

    // Determine output directory based on type
    let file_path = project
        .risk_directory(&risk.risk_type.to_string())
        .join(format!("{}.tdt.yaml", risk.id));

    // Add to short ID index
    let mut short_ids = ShortIdIndex::load(&project);
    let short_id = short_ids.add(risk.id.to_string());
    super::utils::save_short_ids(&mut short_ids, &project);

    // Handle --link flags
    let added_links = crate::cli::entity_cmd::process_link_flags(
        &file_path,
        EntityPrefix::Risk,
        &args.link,
        &short_ids,
    );

    // Output based on format flag
    match global.output {
        OutputFormat::Id => {
            println!("{}", risk.id);
        }
        OutputFormat::ShortId => {
            println!(
                "{}",
                short_id
                    .clone()
                    .unwrap_or_else(|| format_short_id(&risk.id))
            );
        }
        OutputFormat::Path => {
            println!("{}", file_path.display());
        }
        _ => {
            println!(
                "{} Created risk {}",
                style("✓").green(),
                style(
                    short_id
                        .clone()
                        .unwrap_or_else(|| format_short_id(&risk.id))
                )
                .cyan()
            );
            println!("   {}", style(file_path.display()).dim());
            println!("   RPN: {} ({})", style(rpn).yellow(), risk_level);

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

    // Sync cache after creation
    super::utils::sync_cache(&project);

    // Open in editor if requested (or by default unless --no-edit)
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

    // Use RiskService to get the risk (cache-first lookup)
    let service = RiskService::new(&project, &cache);
    let risk = service
        .get(&resolved_id)
        .map_err(|e| miette::miette!("{}", e))?
        .ok_or_else(|| miette::miette!("No risk found matching '{}'", args.id))?;

    // Output based on format (pretty is default, yaml/json explicit)
    match global.output {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&risk).into_diagnostic()?;
            println!("{}", json);
        }
        OutputFormat::Yaml => {
            let yaml = serde_yml::to_string(&risk).into_diagnostic()?;
            print!("{}", yaml);
        }
        OutputFormat::Id
        | OutputFormat::ShortId
        | OutputFormat::Table
        | OutputFormat::Dot
        | OutputFormat::Tree => {
            if global.output == OutputFormat::ShortId {
                let short_id = short_ids
                    .get_short_id(&risk.id.to_string())
                    .unwrap_or_default();
                println!("{}", short_id);
            } else {
                println!("{}", risk.id);
            }
        }
        _ => {
            // Human-readable format

            println!("{}", style("─".repeat(60)).dim());
            println!(
                "{}: {}",
                style("ID").bold(),
                style(&risk.id.to_string()).cyan()
            );
            println!("{}: {}", style("Type").bold(), risk.risk_type);
            println!("{}: {}", style("Title").bold(), style(&risk.title).yellow());
            println!("{}: {}", style("Status").bold(), risk.status);
            // Use computed risk level for accurate display
            if let Some(level) = risk.get_risk_level() {
                let level_styled = match level {
                    RiskLevel::Critical => style(level.to_string()).red().bold(),
                    RiskLevel::High => style(level.to_string()).red(),
                    RiskLevel::Medium => style(level.to_string()).yellow(),
                    RiskLevel::Low => style(level.to_string()).green(),
                };
                println!("{}: {}", style("Risk Level").bold(), level_styled);
            }
            if let Some(ref cat) = risk.category {
                if !cat.is_empty() {
                    println!("{}: {}", style("Category").bold(), cat);
                }
            }
            println!("{}", style("─".repeat(60)).dim());

            // Description
            println!();
            println!("{}", style("Description:").bold());
            println!("{}", &risk.description);

            // FMEA details
            if risk.failure_mode.is_some() || risk.cause.is_some() || risk.effect.is_some() {
                println!();
                println!("{}", style("FMEA Analysis:").bold());
                if let Some(ref fm) = risk.failure_mode {
                    if !fm.is_empty() {
                        println!("  {}: {}", style("Failure Mode").dim(), fm.trim());
                    }
                }
                if let Some(ref cause) = risk.cause {
                    if !cause.is_empty() {
                        println!("  {}: {}", style("Cause").dim(), cause.trim());
                    }
                }
                if let Some(ref effect) = risk.effect {
                    if !effect.is_empty() {
                        println!("  {}: {}", style("Effect").dim(), effect.trim());
                    }
                }
            }

            // Risk ratings
            if risk.severity.is_some() || risk.occurrence.is_some() || risk.detection.is_some() {
                println!();
                println!("{}", style("Risk Assessment:").bold());
                if let Some(s) = risk.severity {
                    println!("  {}: {}/10", style("Severity").dim(), s);
                }
                if let Some(o) = risk.occurrence {
                    println!("  {}: {}/10", style("Occurrence").dim(), o);
                }
                if let Some(d) = risk.detection {
                    println!("  {}: {}/10", style("Detection").dim(), d);
                }
                // Use computed RPN for accurate display
                if let Some(rpn) = risk.get_rpn() {
                    let rpn_styled = match rpn {
                        r if r > 400 => style(r.to_string()).red().bold(),
                        r if r > 150 => style(r.to_string()).yellow(),
                        r => style(r.to_string()).green(),
                    };
                    println!("  {}: {}", style("RPN").bold(), rpn_styled);
                }
            }

            // Mitigations
            if !risk.mitigations.is_empty() {
                println!();
                println!("{}", style("Mitigations:").bold());
                for (i, m) in risk.mitigations.iter().enumerate() {
                    if !m.action.is_empty() {
                        let status_str = m.status.map(|s| format!(" [{}]", s)).unwrap_or_default();
                        println!("  {}. {}{}", i + 1, m.action, style(status_str).dim());
                    }
                }
            }

            // Links (only shown with --with-links flag)
            if args.with_links {
                let cache = EntityCache::open(&project).ok();
                let has_links = !risk.links.related_to.is_empty()
                    || !risk.links.mitigated_by.is_empty()
                    || !risk.links.verified_by.is_empty()
                    || !risk.links.affects.is_empty();

                if has_links {
                    println!();
                    println!("{}", style("Links:").bold());

                    if !risk.links.related_to.is_empty() {
                        println!("  {}:", style("Related To").dim());
                        for id in &risk.links.related_to {
                            let display =
                                format_link_with_title(&id.to_string(), &short_ids, &cache);
                            println!("    {}", style(&display).cyan());
                        }
                    }

                    if !risk.links.mitigated_by.is_empty() {
                        println!("  {}:", style("Mitigated By").dim());
                        for id in &risk.links.mitigated_by {
                            let display =
                                format_link_with_title(&id.to_string(), &short_ids, &cache);
                            println!("    {}", style(&display).cyan());
                        }
                    }

                    if !risk.links.verified_by.is_empty() {
                        println!("  {}:", style("Verified By").dim());
                        for id in &risk.links.verified_by {
                            let display =
                                format_link_with_title(&id.to_string(), &short_ids, &cache);
                            println!("    {}", style(&display).cyan());
                        }
                    }

                    if !risk.links.affects.is_empty() {
                        println!("  {}:", style("Affects").dim());
                        for id in &risk.links.affects {
                            let display =
                                format_link_with_title(&id.to_string(), &short_ids, &cache);
                            println!("    {}", style(&display).cyan());
                        }
                    }
                }
            }

            println!();
            println!("{}", style("─".repeat(60)).dim());
            println!(
                "{}: {} | {}: {} | {}: {}",
                style("Author").dim(),
                risk.author,
                style("Created").dim(),
                risk.created.format("%Y-%m-%d %H:%M"),
                style("Revision").dim(),
                risk.revision
            );
        }
    }

    Ok(())
}

fn run_edit(args: EditArgs) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let cache = EntityCache::open(&project).map_err(|e| miette::miette!("{}", e))?;
    let config = Config::load();

    // Resolve short ID if needed
    let short_ids = ShortIdIndex::load(&project);
    let resolved_id = short_ids
        .resolve(&args.id)
        .unwrap_or_else(|| args.id.clone());

    // Use RiskService to get the entity
    let service = RiskService::new(&project, &cache);
    let risk = service
        .get(&resolved_id)
        .map_err(|e| miette::miette!("{}", e))?
        .ok_or_else(|| miette::miette!("No risk found matching '{}'", args.id))?;

    // Get file path from cache
    let file_path = if let Some(cached) = cache.get_entity(&risk.id.to_string()) {
        if cached.file_path.is_absolute() {
            cached.file_path.clone()
        } else {
            project.root().join(&cached.file_path)
        }
    } else {
        // Fallback: compute path from risk type
        let risk_type = match risk.risk_type {
            RiskType::Design => "design",
            RiskType::Process => "process",
            RiskType::Use => "use",
            RiskType::Software => "software",
        };
        project
            .root()
            .join(format!("risks/{}/{}.tdt.yaml", risk_type, risk.id))
    };

    if !file_path.exists() {
        return Err(miette::miette!("File not found: {}", file_path.display()));
    }

    println!(
        "Opening {} in {}...",
        style(format_short_id(&risk.id)).cyan(),
        style(config.editor()).yellow()
    );

    config.run_editor(&file_path).into_diagnostic()?;

    Ok(())
}

fn run_summary(args: SummaryArgs, global: &GlobalOpts) -> Result<()> {
    use tdt_core::entities::risk::MitigationStatus;

    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let cache = EntityCache::open(&project).map_err(|e| miette::miette!("{}", e))?;
    let short_ids = ShortIdIndex::load(&project);

    // Use service to get stats and load all risks
    let service = RiskService::new(&project, &cache);
    let stats = service.stats().map_err(|e| miette::miette!("{}", e))?;
    let risks = service.load_all().map_err(|e| miette::miette!("{}", e))?;

    if risks.is_empty() {
        println!("{}", style("No risks found in project.").yellow());
        return Ok(());
    }

    // Use stats from service
    let total = stats.total;
    let avg_rpn = stats.rpn_stats.avg;
    let max_rpn = stats.rpn_stats.max;
    let min_rpn = stats.rpn_stats.min;
    let unmitigated = stats.unmitigated;

    // Build level counts from service stats
    let mut by_level: std::collections::HashMap<RiskLevel, usize> =
        std::collections::HashMap::new();
    by_level.insert(RiskLevel::Critical, stats.by_level.critical);
    by_level.insert(RiskLevel::High, stats.by_level.high);
    by_level.insert(RiskLevel::Medium, stats.by_level.medium);
    by_level.insert(RiskLevel::Low, stats.by_level.low);

    // Build type counts from service stats
    let mut by_type: std::collections::HashMap<RiskType, usize> = std::collections::HashMap::new();
    by_type.insert(RiskType::Design, stats.by_type.design);
    by_type.insert(RiskType::Process, stats.by_type.process);
    by_type.insert(RiskType::Use, stats.by_type.r#use);
    by_type.insert(RiskType::Software, stats.by_type.software);

    // Count with open mitigations (not all verified)
    let open_mitigations = risks
        .iter()
        .filter(|r| {
            !r.mitigations.is_empty()
                && r.mitigations
                    .iter()
                    .any(|m| m.status != Some(MitigationStatus::Verified))
        })
        .count();

    // Sort by RPN for top N (risks without RPN go last)
    let mut sorted_risks: Vec<&Risk> = risks.iter().collect();
    sorted_risks.sort_by(|a, b| {
        let rpn_a = a.calculate_rpn().unwrap_or(0);
        let rpn_b = b.calculate_rpn().unwrap_or(0);
        rpn_b.cmp(&rpn_a)
    });

    // Output based on format
    match global.output {
        OutputFormat::Json => {
            let summary = serde_json::json!({
                "total": total,
                "by_level": {
                    "critical": by_level.get(&RiskLevel::Critical).unwrap_or(&0),
                    "high": by_level.get(&RiskLevel::High).unwrap_or(&0),
                    "medium": by_level.get(&RiskLevel::Medium).unwrap_or(&0),
                    "low": by_level.get(&RiskLevel::Low).unwrap_or(&0),
                },
                "by_type": {
                    "design": by_type.get(&RiskType::Design).unwrap_or(&0),
                    "process": by_type.get(&RiskType::Process).unwrap_or(&0),
                    "use": by_type.get(&RiskType::Use).unwrap_or(&0),
                    "software": by_type.get(&RiskType::Software).unwrap_or(&0),
                },
                "rpn": {
                    "average": avg_rpn,
                    "max": max_rpn,
                    "min": min_rpn,
                    "risks_with_rpn": stats.rpn_stats.count,
                },
                "unmitigated": unmitigated,
                "open_mitigations": open_mitigations,
                "top_risks": sorted_risks.iter().take(args.top).map(|r| {
                    let level = r.risk_level.or_else(|| r.determine_risk_level());
                    serde_json::json!({
                        "id": r.id.to_string(),
                        "title": r.title,
                        "rpn": r.calculate_rpn(),
                        "level": level.map(|l| format!("{:?}", l).to_lowercase()),
                    })
                }).collect::<Vec<_>>(),
            });
            println!(
                "{}",
                serde_json::to_string_pretty(&summary).unwrap_or_default()
            );
        }
        _ => {
            // Human-readable output
            println!("{}", style("Risk Summary").bold().underlined());
            println!();

            // Overview section
            println!("{:<20} {}", style("Total Risks:").bold(), total);
            if stats.rpn_stats.count > 0 {
                println!("{:<20} {:.1}", style("Average RPN:").bold(), avg_rpn);
                println!(
                    "{:<20} {} (max: {})",
                    style("RPN Range:").bold(),
                    min_rpn,
                    max_rpn
                );
            }
            println!();

            // By level
            println!("{}", style("By Risk Level:").bold());
            let critical = *by_level.get(&RiskLevel::Critical).unwrap_or(&0);
            let high = *by_level.get(&RiskLevel::High).unwrap_or(&0);
            let medium = *by_level.get(&RiskLevel::Medium).unwrap_or(&0);
            let low = *by_level.get(&RiskLevel::Low).unwrap_or(&0);

            if critical > 0 {
                println!("  {} {}", style("Critical:").red().bold(), critical);
            }
            if high > 0 {
                println!("  {} {}", style("High:").yellow().bold(), high);
            }
            println!("  {:<12} {}", style("Medium:").dim(), medium);
            println!("  {:<12} {}", style("Low:").dim(), low);
            println!();

            // By type
            println!("{}", style("By Risk Type:").bold());
            println!(
                "  {:<12} {}",
                "Design:",
                by_type.get(&RiskType::Design).unwrap_or(&0)
            );
            println!(
                "  {:<12} {}",
                "Process:",
                by_type.get(&RiskType::Process).unwrap_or(&0)
            );
            println!(
                "  {:<12} {}",
                "Use:",
                by_type.get(&RiskType::Use).unwrap_or(&0)
            );
            println!(
                "  {:<12} {}",
                "Software:",
                by_type.get(&RiskType::Software).unwrap_or(&0)
            );
            println!();

            // Mitigation status
            println!("{}", style("Mitigation Status:").bold());
            if unmitigated > 0 {
                println!("  {} {}", style("Unmitigated:").red(), unmitigated);
            } else {
                println!("  {} 0", style("Unmitigated:").green());
            }
            if open_mitigations > 0 {
                println!(
                    "  {} {}",
                    style("Open (unverified):").yellow(),
                    open_mitigations
                );
            }
            let fully_mitigated = total
                .saturating_sub(unmitigated)
                .saturating_sub(open_mitigations);
            println!("  {:<17} {}", "Fully mitigated:", fully_mitigated);
            println!();

            // Top N risks
            println!(
                "{} {}",
                style("Top").bold(),
                style(format!("{} Risks by RPN:", args.top)).bold()
            );
            println!("{}", "-".repeat(60));
            println!(
                "{:<10} {:<6} {:<10} {}",
                style("ID").bold(),
                style("RPN").bold(),
                style("LEVEL").bold(),
                style("TITLE").bold()
            );

            for risk in sorted_risks.iter().take(args.top) {
                let id_short = short_ids
                    .get_short_id(&risk.id.to_string())
                    .unwrap_or_else(|| truncate_str(&risk.id.to_string(), 8));
                let rpn = risk
                    .calculate_rpn()
                    .map(|r| r.to_string())
                    .unwrap_or_else(|| "-".to_string());
                let level = risk
                    .risk_level
                    .or_else(|| risk.determine_risk_level())
                    .unwrap_or(RiskLevel::Medium);
                let level_str = format!("{:?}", level).to_lowercase();
                let level_styled = match level {
                    RiskLevel::Critical => style(level_str).red().bold().to_string(),
                    RiskLevel::High => style(level_str).yellow().to_string(),
                    RiskLevel::Medium => style(level_str).dim().to_string(),
                    RiskLevel::Low => style(level_str).dim().to_string(),
                };
                println!(
                    "{:<10} {:<6} {:<10} {}",
                    style(id_short).cyan(),
                    rpn,
                    level_styled,
                    truncate_str(&risk.title, 35)
                );
            }

            // Detailed breakdown by category
            if args.detailed {
                println!();
                println!("{}", style("By Category:").bold());

                let mut by_category: std::collections::HashMap<String, Vec<&Risk>> =
                    std::collections::HashMap::new();
                for risk in &risks {
                    let cat = risk
                        .category
                        .clone()
                        .unwrap_or_else(|| "Uncategorized".to_string());
                    by_category.entry(cat).or_default().push(risk);
                }

                let mut categories: Vec<_> = by_category.keys().collect();
                categories.sort();

                for cat in categories {
                    let cat_risks = by_category.get(cat).unwrap();
                    let cat_rpns: Vec<u16> =
                        cat_risks.iter().filter_map(|r| r.calculate_rpn()).collect();
                    let cat_avg_rpn = if cat_rpns.is_empty() {
                        "-".to_string()
                    } else {
                        format!(
                            "{:.0}",
                            cat_rpns.iter().map(|&r| r as f64).sum::<f64>() / cat_rpns.len() as f64
                        )
                    };
                    println!(
                        "  {} ({} risks, avg RPN: {})",
                        style(cat).cyan(),
                        cat_risks.len(),
                        cat_avg_rpn
                    );
                }
            }
        }
    }

    Ok(())
}

#[allow(clippy::needless_range_loop)]
fn run_matrix(args: MatrixArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let short_ids = ShortIdIndex::load(&project);

    // Collect risks using cache for speed
    let cache = EntityCache::open(&project)?;

    let type_filter = args.risk_type.map(|t| match t {
        CliRiskType::Design => "design",
        CliRiskType::Process => "process",
        CliRiskType::Use => "use",
        CliRiskType::Software => "software",
    });

    let cached_risks = cache.list_risks(
        None,        // status
        type_filter, // type
        None,        // level
        None,        // category
        None,        // min_rpn
        None,        // author
        None,        // search
        None,        // limit
    );

    if cached_risks.is_empty() {
        println!("{}", style("No risks found in project.").yellow());
        return Ok(());
    }

    // Use either 10×10 or 5×5 (compact) matrix
    let size = if args.compact { 5 } else { 10 };

    // Build the matrix: [severity][occurrence] -> Vec<(id, short_id)>
    let mut matrix: Vec<Vec<Vec<(String, String)>>> = vec![vec![Vec::new(); size + 1]; size + 1];

    // Populate matrix from cached risks
    let mut skipped_count = 0;
    for risk in &cached_risks {
        // Skip risks without both severity and occurrence ratings
        let (sev, occ) = match (risk.severity, risk.occurrence) {
            (Some(s), Some(o)) => (s as usize, o as usize),
            _ => {
                skipped_count += 1;
                continue;
            }
        };

        // Map to matrix indices (1-10 or 1-5 for compact)
        let sev_idx = if args.compact {
            sev.div_ceil(2).min(size).max(1) // Map 1-2 -> 1, 3-4 -> 2, etc.
        } else {
            sev.min(size).max(1)
        };

        let occ_idx = if args.compact {
            occ.div_ceil(2).min(size).max(1)
        } else {
            occ.min(size).max(1)
        };

        let short_id = short_ids
            .get_short_id(&risk.id)
            .unwrap_or_else(|| truncate_str(&risk.id, 6));
        matrix[sev_idx][occ_idx].push((risk.id.clone(), short_id));
    }

    if skipped_count > 0 {
        eprintln!(
            "{} {} risk(s) without severity/occurrence ratings excluded from matrix",
            console::style("⚠").yellow(),
            skipped_count,
        );
    }

    // JSON/YAML output
    match global.output {
        OutputFormat::Json => {
            let mut json_matrix: Vec<serde_json::Value> = Vec::new();
            for sev in (1..=size).rev() {
                for occ in 1..=size {
                    if !matrix[sev][occ].is_empty() {
                        json_matrix.push(serde_json::json!({
                            "severity": sev,
                            "occurrence": occ,
                            "count": matrix[sev][occ].len(),
                            "risks": matrix[sev][occ].iter().map(|(id, _)| id).collect::<Vec<_>>()
                        }));
                    }
                }
            }
            let summary = serde_json::json!({
                "size": size,
                "compact": args.compact,
                "type_filter": type_filter,
                "total_risks": cached_risks.len(),
                "cells": json_matrix
            });
            println!(
                "{}",
                serde_json::to_string_pretty(&summary).unwrap_or_default()
            );
            return Ok(());
        }
        OutputFormat::Yaml => {
            let mut yaml_matrix: Vec<serde_json::Value> = Vec::new();
            for sev in (1..=size).rev() {
                for occ in 1..=size {
                    if !matrix[sev][occ].is_empty() {
                        yaml_matrix.push(serde_json::json!({
                            "severity": sev,
                            "occurrence": occ,
                            "count": matrix[sev][occ].len(),
                            "risks": matrix[sev][occ].iter().map(|(id, _)| id).collect::<Vec<_>>()
                        }));
                    }
                }
            }
            let summary = serde_json::json!({
                "size": size,
                "cells": yaml_matrix
            });
            println!("{}", serde_yml::to_string(&summary).unwrap_or_default());
            return Ok(());
        }
        _ => {}
    }

    // Human-readable matrix output
    println!();
    let title = if let Some(rt) = args.risk_type {
        format!("Risk Matrix ({} Risks)", rt)
    } else {
        "Risk Matrix".to_string()
    };
    println!("{}", style(title).bold().cyan());
    println!(
        "{}",
        style(format!("{} risks displayed", cached_risks.len())).dim()
    );
    println!();

    // Determine cell width based on content
    let cell_width = if args.show_ids { 8 } else { 4 };

    // Print header row (OCCURRENCE)
    print!("{:>4} ", ""); // Space for severity label
    print!("{}", style("│").dim());
    for occ in 1..=size {
        print!("{:^width$}", occ, width = cell_width);
    }
    println!();

    // Print separator
    print!("{:>4} ", "");
    print!("{}", style("┼").dim());
    println!("{}", style("─".repeat(size * cell_width)).dim());

    // Print rows (SEVERITY - high to low)
    for sev in (1..=size).rev() {
        // Severity label
        print!("{:>4} ", sev);
        print!("{}", style("│").dim());

        for occ in 1..=size {
            let cell = &matrix[sev][occ];
            let count = cell.len();

            // Calculate RPN for this cell to determine color
            // RPN = S × O × D (assuming D=5 for color coding purposes)
            let estimated_rpn = sev * occ * 5;

            let content = if args.show_ids && count > 0 {
                // Show first risk ID
                cell.first()
                    .map(|(_, short)| short.clone())
                    .unwrap_or_default()
            } else if count > 0 {
                count.to_string()
            } else {
                "-".to_string()
            };

            // Color based on risk level (using S×O as proxy)
            let styled_content = if count == 0 {
                style(content).dim()
            } else if estimated_rpn > 200 {
                // High: S×O > 40 (e.g., 7×7=49)
                style(content).red().bold()
            } else if estimated_rpn > 100 {
                // Medium-high
                style(content).red()
            } else if estimated_rpn > 50 {
                // Medium
                style(content).yellow()
            } else {
                // Low
                style(content).green()
            };

            print!("{:^width$}", styled_content, width = cell_width);
        }
        println!();
    }

    // Legend
    println!();
    print!("{:>4} ", "");
    for _ in 0..((size * cell_width) / 2 - 5) {
        print!(" ");
    }
    println!("{}", style("OCCURRENCE →").dim());

    // Severity label (vertical)
    println!();
    println!("{}", style("       ↑ SEVERITY").dim());

    // Color legend
    println!();
    println!("{}", style("Legend:").bold());
    println!(
        "  {} Low (S×O < 10)  {} Medium (10-20)  {} High (20-40)  {} Critical (>40)",
        style("■").green(),
        style("■").yellow(),
        style("■").red(),
        style("■").red().bold()
    );

    // Summary stats
    let mut critical_count = 0;
    let mut high_count = 0;
    let mut medium_count = 0;
    let mut low_count = 0;

    for sev in 1..=size {
        for occ in 1..=size {
            let count = matrix[sev][occ].len();
            if count > 0 {
                let so = sev * occ;
                if so > 40 {
                    critical_count += count;
                } else if so > 20 {
                    high_count += count;
                } else if so > 10 {
                    medium_count += count;
                } else {
                    low_count += count;
                }
            }
        }
    }

    println!();
    println!(
        "Total: {} | {} critical | {} high | {} medium | {} low",
        cached_risks.len(),
        style(critical_count).red().bold(),
        style(high_count).red(),
        style(medium_count).yellow(),
        style(low_count).green()
    );

    Ok(())
}
