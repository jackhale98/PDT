//! `tdt risk` command - Risk/FMEA management

use clap::{Subcommand, ValueEnum};
use console::style;
use miette::{IntoDiagnostic, Result};
use std::fs;

use crate::cli::commands::utils::format_link_with_title;
use crate::cli::filters::StatusFilter;
use crate::cli::helpers::{format_short_id, truncate_str};
use crate::cli::table::{CellValue, ColumnDef, TableConfig, TableFormatter, TableRow};
use crate::cli::{GlobalOpts, OutputFormat};
use crate::core::cache::EntityCache;
use crate::core::identity::{EntityId, EntityPrefix};
use crate::core::project::Project;
use crate::core::shortid::ShortIdIndex;
use crate::core::Config;
use crate::entities::risk::{Risk, RiskLevel, RiskType};
use crate::schema::template::{TemplateContext, TemplateGenerator};
use crate::schema::wizard::SchemaWizard;

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

/// Columns to display in list output
#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
pub enum ListColumn {
    Id,
    Type,
    Title,
    Status,
    RiskLevel,
    Severity,
    Occurrence,
    Detection,
    Rpn,
    Category,
    Author,
    Created,
}

impl std::fmt::Display for ListColumn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ListColumn::Id => write!(f, "id"),
            ListColumn::Type => write!(f, "type"),
            ListColumn::Title => write!(f, "title"),
            ListColumn::Status => write!(f, "status"),
            ListColumn::RiskLevel => write!(f, "risk-level"),
            ListColumn::Severity => write!(f, "severity"),
            ListColumn::Occurrence => write!(f, "occurrence"),
            ListColumn::Detection => write!(f, "detection"),
            ListColumn::Rpn => write!(f, "rpn"),
            ListColumn::Category => write!(f, "category"),
            ListColumn::Author => write!(f, "author"),
            ListColumn::Created => write!(f, "created"),
        }
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

/// Directories where risks are stored
const RISK_DIRS: &[&str] = &["risks/design", "risks/process"];

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

    // Determine if we need full entity loading (for complex filters)
    let needs_full_entities = args.search.is_some()  // search in description
        || args.unmitigated
        || args.open_mitigations
        || args.tag.is_some();

    // Collect risks - use cache for basic filtering, full load for complex
    let mut risks: Vec<Risk> = if needs_full_entities {
        // Full entity loading (original approach)
        let mut risks: Vec<Risk> = Vec::new();

        // Check design risks
        let design_dir = project.root().join("risks/design");
        if design_dir.exists() {
            for entry in walkdir::WalkDir::new(&design_dir)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
                .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
            {
                match crate::yaml::parse_yaml_file::<Risk>(entry.path()) {
                    Ok(risk) => risks.push(risk),
                    Err(e) => {
                        eprintln!(
                            "{} Failed to parse {}: {}",
                            style("!").yellow(),
                            entry.path().display(),
                            e
                        );
                    }
                }
            }
        }

        // Check process risks
        let process_dir = project.root().join("risks/process");
        if process_dir.exists() {
            for entry in walkdir::WalkDir::new(&process_dir)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
                .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
            {
                match crate::yaml::parse_yaml_file::<Risk>(entry.path()) {
                    Ok(risk) => risks.push(risk),
                    Err(e) => {
                        eprintln!(
                            "{} Failed to parse {}: {}",
                            style("!").yellow(),
                            entry.path().display(),
                            e
                        );
                    }
                }
            }
        }
        risks
    } else {
        // Use cache for basic filtering (faster for large projects)
        let cache = EntityCache::open(&project)?;

        // Convert filters to cache-compatible format
        let status_filter = match args.status {
            StatusFilter::Draft => Some("draft"),
            StatusFilter::Review => Some("review"),
            StatusFilter::Approved => Some("approved"),
            StatusFilter::Released => Some("released"),
            StatusFilter::Obsolete => Some("obsolete"),
            StatusFilter::Active | StatusFilter::All => None,
        };

        let type_filter = match args.r#type {
            RiskTypeFilter::Design => Some("design"),
            RiskTypeFilter::Process => Some("process"),
            RiskTypeFilter::Use => Some("use"),
            RiskTypeFilter::Software => Some("software"),
            RiskTypeFilter::All => None,
        };

        let level_filter = match args.level {
            RiskLevelFilter::Low => Some("low"),
            RiskLevelFilter::Medium => Some("medium"),
            RiskLevelFilter::High => Some("high"),
            RiskLevelFilter::Critical => Some("critical"),
            RiskLevelFilter::Urgent | RiskLevelFilter::All => None,
        };

        // Effective min RPN
        let min_rpn = args.above_rpn.or(args.min_rpn).map(|v| v as i32);

        // Query cache with basic filters
        let cached_risks = cache.list_risks(
            status_filter,
            type_filter,
            level_filter,
            args.category.as_deref(),
            min_rpn,
            args.author.as_deref(),
            None, // No search (would need description field)
            None, // No limit yet
        );

        // Load full entities from cached file paths
        cached_risks
            .iter()
            .filter_map(|cr| {
                let full_path = project.root().join(&cr.file_path);
                fs::read_to_string(&full_path)
                    .ok()
                    .and_then(|content| serde_yml::from_str::<Risk>(&content).ok())
            })
            .collect()
    };

    // Apply filters
    risks.retain(|r| {
        // Type filter
        let type_match = match args.r#type {
            RiskTypeFilter::Design => r.risk_type == RiskType::Design,
            RiskTypeFilter::Process => r.risk_type == RiskType::Process,
            RiskTypeFilter::Use => r.risk_type == RiskType::Use,
            RiskTypeFilter::Software => r.risk_type == RiskType::Software,
            RiskTypeFilter::All => true,
        };

        // Status filter
        let status_match = match args.status {
            StatusFilter::Draft => r.status == crate::core::entity::Status::Draft,
            StatusFilter::Review => r.status == crate::core::entity::Status::Review,
            StatusFilter::Approved => r.status == crate::core::entity::Status::Approved,
            StatusFilter::Released => r.status == crate::core::entity::Status::Released,
            StatusFilter::Obsolete => r.status == crate::core::entity::Status::Obsolete,
            StatusFilter::Active => r.status != crate::core::entity::Status::Obsolete,
            StatusFilter::All => true,
        };

        // Level filter (use computed risk level for accurate filtering)
        let computed_level = r.get_risk_level();
        let level_match = match args.level {
            RiskLevelFilter::All => true,
            RiskLevelFilter::Urgent => matches!(
                computed_level,
                Some(RiskLevel::High) | Some(RiskLevel::Critical)
            ),
            RiskLevelFilter::Low => computed_level == Some(RiskLevel::Low),
            RiskLevelFilter::Medium => computed_level == Some(RiskLevel::Medium),
            RiskLevelFilter::High => computed_level == Some(RiskLevel::High),
            RiskLevelFilter::Critical => computed_level == Some(RiskLevel::Critical),
        };

        // RPN filters (use computed RPN for accurate filtering)
        let computed_rpn = r.get_rpn().unwrap_or(0);
        let effective_min_rpn = args.above_rpn.or(args.min_rpn);
        let min_rpn_match = effective_min_rpn.is_none_or(|min| computed_rpn >= min);
        let max_rpn_match = args.max_rpn.is_none_or(|max| computed_rpn <= max);

        // Category filter (case-insensitive)
        let category_match = args.category.as_ref().is_none_or(|cat| {
            r.category
                .as_ref()
                .is_some_and(|c| c.to_lowercase() == cat.to_lowercase())
        });

        // Tag filter (case-insensitive)
        let tag_match = args.tag.as_ref().is_none_or(|tag| {
            r.tags
                .iter()
                .any(|t| t.to_lowercase() == tag.to_lowercase())
        });

        // Author filter
        let author_match = args
            .author
            .as_ref()
            .is_none_or(|author| r.author.to_lowercase().contains(&author.to_lowercase()));

        // Search filter
        let search_match = args.search.as_ref().is_none_or(|search| {
            let search_lower = search.to_lowercase();
            r.title.to_lowercase().contains(&search_lower)
                || r.description.to_lowercase().contains(&search_lower)
        });

        // Unmitigated filter
        let unmitigated_match = !args.unmitigated || r.mitigations.is_empty();

        // Open mitigations filter (has mitigations but not all completed/verified)
        let open_mitigations_match = if args.open_mitigations {
            use crate::entities::risk::MitigationStatus;
            !r.mitigations.is_empty()
                && r.mitigations.iter().any(|m| {
                    match m.status {
                        Some(MitigationStatus::Completed) | Some(MitigationStatus::Verified) => {
                            false
                        }
                        _ => true, // Proposed, InProgress, or None
                    }
                })
        } else {
            true
        };

        // Critical shortcut filter (use computed risk level)
        let critical_match = !args.critical || computed_level == Some(RiskLevel::Critical);

        // Recent filter (created in last N days)
        let recent_match = args.recent.is_none_or(|days| {
            let cutoff = chrono::Utc::now() - chrono::Duration::days(days as i64);
            r.created >= cutoff
        });

        type_match
            && status_match
            && level_match
            && min_rpn_match
            && max_rpn_match
            && category_match
            && tag_match
            && author_match
            && search_match
            && unmitigated_match
            && open_mitigations_match
            && critical_match
            && recent_match
    });

    if risks.is_empty() {
        match global.format {
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

    // Sort by specified column (or RPN if --by-rpn is used)
    // Use computed values for accurate sorting
    if args.by_rpn {
        risks.sort_by(|a, b| b.get_rpn().unwrap_or(0).cmp(&a.get_rpn().unwrap_or(0)));
    } else {
        match args.sort {
            ListColumn::Id => risks.sort_by(|a, b| a.id.to_string().cmp(&b.id.to_string())),
            ListColumn::Type => {
                risks.sort_by(|a, b| a.risk_type.to_string().cmp(&b.risk_type.to_string()))
            }
            ListColumn::Title => risks.sort_by(|a, b| a.title.cmp(&b.title)),
            ListColumn::Status => {
                risks.sort_by(|a, b| a.status.to_string().cmp(&b.status.to_string()))
            }
            ListColumn::RiskLevel => {
                let level_order = |l: Option<RiskLevel>| match l {
                    Some(RiskLevel::Critical) => 0,
                    Some(RiskLevel::High) => 1,
                    Some(RiskLevel::Medium) => 2,
                    Some(RiskLevel::Low) => 3,
                    None => 4,
                };
                risks.sort_by(|a, b| {
                    level_order(a.get_risk_level()).cmp(&level_order(b.get_risk_level()))
                });
            }
            ListColumn::Severity => {
                risks.sort_by(|a, b| b.severity.unwrap_or(0).cmp(&a.severity.unwrap_or(0)))
            }
            ListColumn::Occurrence => {
                risks.sort_by(|a, b| b.occurrence.unwrap_or(0).cmp(&a.occurrence.unwrap_or(0)))
            }
            ListColumn::Detection => {
                risks.sort_by(|a, b| b.detection.unwrap_or(0).cmp(&a.detection.unwrap_or(0)))
            }
            ListColumn::Rpn => {
                risks.sort_by(|a, b| b.get_rpn().unwrap_or(0).cmp(&a.get_rpn().unwrap_or(0)))
            }
            ListColumn::Category => risks.sort_by(|a, b| {
                a.category
                    .as_deref()
                    .unwrap_or("")
                    .cmp(b.category.as_deref().unwrap_or(""))
            }),
            ListColumn::Author => risks.sort_by(|a, b| a.author.cmp(&b.author)),
            ListColumn::Created => risks.sort_by(|a, b| a.created.cmp(&b.created)),
        }
    }

    // Reverse if requested (unless by_rpn which is already reversed)
    if args.reverse && !args.by_rpn {
        risks.reverse();
    }

    // Apply limit
    if let Some(limit) = args.limit {
        risks.truncate(limit);
    }

    // Just count?
    if args.count {
        println!("{}", risks.len());
        return Ok(());
    }

    // Update short ID index with current risks (preserves other entity types)
    let mut short_ids = ShortIdIndex::load(&project);
    short_ids.ensure_all(risks.iter().map(|r| r.id.to_string()));
    super::utils::save_short_ids(&mut short_ids, &project);

    // Output based on format
    let format = match global.format {
        OutputFormat::Auto => OutputFormat::Tsv,
        f => f,
    };

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
        | OutputFormat::ShortId => {
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
        OutputFormat::Auto | OutputFormat::Path => unreachable!(),
    }

    Ok(())
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
                _ => RiskType::Design,
            })
            .unwrap_or(RiskType::Design);

        let title = result
            .get_string("title")
            .map(String::from)
            .unwrap_or_else(|| "New Risk".to_string());

        let category = result
            .get_string("category")
            .map(String::from)
            .unwrap_or_default();

        let severity = result
            .get_string("severity")
            .and_then(|s| s.parse().ok())
            .unwrap_or(5);

        let occurrence = result
            .get_string("occurrence")
            .and_then(|s| s.parse().ok())
            .unwrap_or(5);

        let detection = result
            .get_string("detection")
            .and_then(|s| s.parse().ok())
            .unwrap_or(5);

        // Extract FMEA text fields
        let description = result.get_string("description").map(String::from);
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
        let category = args.category.unwrap_or_default();
        let severity = args.severity.unwrap_or(5);
        let occurrence = args.occurrence.unwrap_or(5);
        let detection = args.detection.unwrap_or(5);

        (
            risk_type, title, category, severity, occurrence, detection, None, None, None, None,
        )
    };

    // Calculate RPN and determine risk level
    let rpn = severity as u16 * occurrence as u16 * detection as u16;
    let risk_level = match rpn {
        0..=50 => "low",
        51..=150 => "medium",
        151..=400 => "high",
        _ => "critical",
    };

    // Generate entity ID and create from template
    let id = EntityId::new(EntityPrefix::Risk);
    let author = config.author();

    let generator = TemplateGenerator::new().map_err(|e| miette::miette!("{}", e))?;
    let ctx = TemplateContext::new(id.clone(), author)
        .with_title(&title)
        .with_risk_type(risk_type.to_string())
        .with_category(&category)
        .with_severity(severity)
        .with_occurrence(occurrence)
        .with_detection(detection)
        .with_risk_level(risk_level);

    let mut yaml_content = generator
        .generate_risk(&ctx)
        .map_err(|e| miette::miette!("{}", e))?;

    // Apply wizard FMEA values via string replacement (for interactive mode)
    if args.interactive {
        if let Some(ref desc) = description {
            if !desc.is_empty() {
                let indented = desc
                    .lines()
                    .map(|line| format!("  {}", line))
                    .collect::<Vec<_>>()
                    .join("\n");
                yaml_content = yaml_content.replace(
                    "description: |\n  # Describe the risk scenario here\n  # What could go wrong? Under what conditions?",
                    &format!("description: |\n{}", indented),
                );
            }
        }
        if let Some(ref fm) = failure_mode {
            if !fm.is_empty() {
                let indented = fm
                    .lines()
                    .map(|line| format!("  {}", line))
                    .collect::<Vec<_>>()
                    .join("\n");
                yaml_content = yaml_content.replace(
                    "failure_mode: |\n  # How does this failure manifest?",
                    &format!("failure_mode: |\n{}", indented),
                );
            }
        }
        if let Some(ref c) = cause {
            if !c.is_empty() {
                let indented = c
                    .lines()
                    .map(|line| format!("  {}", line))
                    .collect::<Vec<_>>()
                    .join("\n");
                yaml_content = yaml_content.replace(
                    "cause: |\n  # What is the root cause or mechanism?",
                    &format!("cause: |\n{}", indented),
                );
            }
        }
        if let Some(ref e) = effect {
            if !e.is_empty() {
                let indented = e
                    .lines()
                    .map(|line| format!("  {}", line))
                    .collect::<Vec<_>>()
                    .join("\n");
                yaml_content = yaml_content.replace(
                    "effect: |\n  # What is the impact or consequence?",
                    &format!("effect: |\n{}", indented),
                );
            }
        }
    }

    // Determine output directory based on type
    let output_dir = project.risk_directory(&risk_type.to_string());

    // Ensure directory exists
    if !output_dir.exists() {
        fs::create_dir_all(&output_dir).into_diagnostic()?;
    }

    let file_path = output_dir.join(format!("{}.tdt.yaml", id));

    // Write file
    fs::write(&file_path, &yaml_content).into_diagnostic()?;

    // Add to short ID index
    let mut short_ids = ShortIdIndex::load(&project);
    let short_id = short_ids.add(id.to_string());
    super::utils::save_short_ids(&mut short_ids, &project);

    // Handle --link flags
    let added_links = crate::cli::entity_cmd::process_link_flags(
        &file_path,
        EntityPrefix::Risk,
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
                "{} Created risk {}",
                style("✓").green(),
                style(short_id.clone().unwrap_or_else(|| format_short_id(&id))).cyan()
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

    // Find the risk by ID prefix match
    let risk = find_risk(&project, &args.id)?;

    // Output based on format (pretty is default, yaml/json explicit)
    match global.format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&risk).into_diagnostic()?;
            println!("{}", json);
        }
        OutputFormat::Yaml => {
            let yaml = serde_yml::to_string(&risk).into_diagnostic()?;
            print!("{}", yaml);
        }
        OutputFormat::Id | OutputFormat::ShortId => {
            if global.format == OutputFormat::ShortId {
                let short_ids = ShortIdIndex::load(&project);
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
            let short_ids = ShortIdIndex::load(&project);

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
    let config = Config::load();

    // Find the risk by ID prefix match
    let risk = find_risk(&project, &args.id)?;

    // Get the file path
    let risk_type = match risk.risk_type {
        RiskType::Design => "design",
        RiskType::Process => "process",
        RiskType::Use => "use",
        RiskType::Software => "software",
    };
    let file_path = project
        .root()
        .join(format!("risks/{}/{}.tdt.yaml", risk_type, risk.id));

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

/// Find a risk by ID prefix match or short ID (@N)
fn find_risk(project: &Project, id_query: &str) -> Result<Risk> {
    use crate::core::cache::EntityCache;

    // Try cache-based lookup first (O(1) via SQLite)
    if let Ok(cache) = EntityCache::open(project) {
        // Resolve short ID if needed
        let full_id = if id_query.contains('@') {
            cache.resolve_short_id(id_query)
        } else {
            None
        };

        let lookup_id = full_id.as_deref().unwrap_or(id_query);

        // Try exact match via cache
        if let Some(entity) = cache.get_entity(lookup_id) {
            if entity.prefix == "RISK" {
                if let Ok(risk) = crate::yaml::parse_yaml_file::<Risk>(&entity.file_path) {
                    return Ok(risk);
                }
            }
        }

        // Try prefix match via cache
        if lookup_id.starts_with("RISK-") {
            let filter = crate::core::EntityFilter {
                prefix: Some(crate::core::EntityPrefix::Risk),
                search: Some(lookup_id.to_string()),
                ..Default::default()
            };
            let matches: Vec<_> = cache.list_entities(&filter);
            if matches.len() == 1 {
                if let Ok(risk) = crate::yaml::parse_yaml_file::<Risk>(&matches[0].file_path) {
                    return Ok(risk);
                }
            } else if matches.len() > 1 {
                println!("{} Multiple matches found:", style("!").yellow());
                for entity in &matches {
                    let short_id = cache
                        .get_short_id(&entity.id)
                        .unwrap_or_else(|| entity.id.clone());
                    println!("  {} - {}", short_id, entity.title);
                }
                return Err(miette::miette!(
                    "Ambiguous query '{}'. Please be more specific.",
                    id_query
                ));
            }
        }
    }

    // Fallback: filesystem search (for title matches or if cache unavailable)
    let short_ids = ShortIdIndex::load(project);
    let resolved_query = short_ids
        .resolve(id_query)
        .unwrap_or_else(|| id_query.to_string());

    let mut matches: Vec<(Risk, std::path::PathBuf)> = Vec::new();

    // Search both design and process directories
    for subdir in &["design", "process"] {
        let dir = project.root().join(format!("risks/{}", subdir));
        if !dir.exists() {
            continue;
        }

        for entry in walkdir::WalkDir::new(&dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        {
            if let Ok(risk) = crate::yaml::parse_yaml_file::<Risk>(entry.path()) {
                // Check if ID matches (prefix or full) or title fuzzy matches
                let id_str = risk.id.to_string();
                let id_matches = id_str.starts_with(&resolved_query) || id_str == resolved_query;
                let title_matches = !id_query.starts_with('@')
                    && !id_query.chars().all(|c| c.is_ascii_digit())
                    && risk
                        .title
                        .to_lowercase()
                        .contains(&resolved_query.to_lowercase());

                if id_matches || title_matches {
                    matches.push((risk, entry.path().to_path_buf()));
                }
            }
        }
    }

    match matches.len() {
        0 => Err(miette::miette!("No risk found matching '{}'", id_query)),
        1 => Ok(matches.remove(0).0),
        _ => {
            println!("{} Multiple matches found:", style("!").yellow());
            for (risk, _path) in &matches {
                println!("  {} - {}", format_short_id(&risk.id), risk.title);
            }
            Err(miette::miette!(
                "Ambiguous query '{}'. Please be more specific.",
                id_query
            ))
        }
    }
}

fn run_summary(args: SummaryArgs, global: &GlobalOpts) -> Result<()> {
    use crate::entities::risk::MitigationStatus;

    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let short_ids = ShortIdIndex::load(&project);

    // Collect all risks
    let mut risks: Vec<Risk> = Vec::new();

    for subdir in &["risks/design", "risks/process"] {
        let dir = project.root().join(subdir);
        if !dir.exists() {
            continue;
        }

        for entry in walkdir::WalkDir::new(&dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        {
            if let Ok(risk) = crate::yaml::parse_yaml_file::<Risk>(entry.path()) {
                risks.push(risk);
            }
        }
    }

    if risks.is_empty() {
        println!("{}", style("No risks found in project.").yellow());
        return Ok(());
    }

    // Calculate metrics
    let total = risks.len();

    // Count by level (using effective level - either explicit or calculated from RPN)
    let mut by_level: std::collections::HashMap<RiskLevel, usize> =
        std::collections::HashMap::new();
    for risk in &risks {
        let level = risk
            .risk_level
            .or_else(|| risk.determine_risk_level())
            .unwrap_or(RiskLevel::Medium);
        *by_level.entry(level).or_insert(0) += 1;
    }

    // Count by type
    let mut by_type: std::collections::HashMap<RiskType, usize> = std::collections::HashMap::new();
    for risk in &risks {
        *by_type.entry(risk.risk_type).or_insert(0) += 1;
    }

    // Calculate RPN statistics (only for risks that have RPN values)
    let rpns: Vec<u16> = risks.iter().filter_map(|r| r.calculate_rpn()).collect();

    let (avg_rpn, max_rpn, min_rpn) = if rpns.is_empty() {
        (0.0, 0u16, 0u16)
    } else {
        let avg = rpns.iter().map(|&r| r as f64).sum::<f64>() / rpns.len() as f64;
        let max = *rpns.iter().max().unwrap_or(&0);
        let min = *rpns.iter().min().unwrap_or(&0);
        (avg, max, min)
    };

    // Count unmitigated
    let unmitigated = risks.iter().filter(|r| r.mitigations.is_empty()).count();

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
    match global.format {
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
                },
                "rpn": {
                    "average": avg_rpn,
                    "max": max_rpn,
                    "min": min_rpn,
                    "risks_with_rpn": rpns.len(),
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
            if !rpns.is_empty() {
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
    for risk in &cached_risks {
        let sev = risk.severity.unwrap_or(0) as usize;
        let occ = risk.occurrence.unwrap_or(0) as usize;

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

    // JSON/YAML output
    match global.format {
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
