//! `tdt rslt` command - Test result management

use clap::{Subcommand, ValueEnum};
use console::style;
use miette::{IntoDiagnostic, Result};

use crate::cli::commands::utils::format_link_with_title;
use crate::cli::filters::StatusFilter;
use crate::cli::helpers::{format_short_id, format_short_id_str, truncate_str};
use crate::cli::table::{CellValue, ColumnDef, TableConfig, TableFormatter, TableRow};
use crate::cli::{GlobalOpts, OutputFormat};
use tdt_core::core::cache::EntityCache;
use tdt_core::core::identity::{EntityId, EntityPrefix};
use tdt_core::core::project::Project;
use tdt_core::core::shortid::ShortIdIndex;
use tdt_core::core::Config;
use tdt_core::entities::result::{Result as TestResult, Verdict};
use tdt_core::schema::wizard::SchemaWizard;
use tdt_core::services::{
    CommonFilter, CreateResult, ResultFilter, ResultService, ResultSortField, SortDirection,
};

#[derive(Subcommand, Debug)]
pub enum RsltCommands {
    /// List results with filtering
    List(ListArgs),

    /// Create a new test result
    New(NewArgs),

    /// Show a result's details
    Show(ShowArgs),

    /// Edit a result in your editor
    Edit(EditArgs),

    /// Delete a result
    Delete(DeleteArgs),

    /// Archive a result (soft delete)
    Archive(ArchiveArgs),

    /// Show test execution statistics and coverage
    Summary(SummaryArgs),
}

/// Verdict filter
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum VerdictFilter {
    Pass,
    Fail,
    Conditional,
    Incomplete,
    NotApplicable,
    /// All non-pass verdicts (fail, conditional, incomplete)
    Issues,
    /// All verdicts
    All,
}

/// Columns to display in list output
#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
pub enum ListColumn {
    Short,
    Id,
    Title,
    Test,
    Verdict,
    Status,
    Author,
    Created,
}

impl std::fmt::Display for ListColumn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ListColumn::Short => write!(f, "short"),
            ListColumn::Id => write!(f, "id"),
            ListColumn::Title => write!(f, "title"),
            ListColumn::Test => write!(f, "test"),
            ListColumn::Verdict => write!(f, "verdict"),
            ListColumn::Status => write!(f, "status"),
            ListColumn::Author => write!(f, "author"),
            ListColumn::Created => write!(f, "created"),
        }
    }
}

/// Column definitions for result list output
const RSLT_COLUMNS: &[ColumnDef] = &[
    ColumnDef::new("short", "SHORT", 8),
    ColumnDef::new("id", "ID", 17),
    ColumnDef::new("title", "TITLE", 25),
    ColumnDef::new("test", "TEST", 8),
    ColumnDef::new("verdict", "VERDICT", 12),
    ColumnDef::new("status", "STATUS", 10),
    ColumnDef::new("author", "AUTHOR", 15),
    ColumnDef::new("created", "CREATED", 12),
];

const RSLT_LIST_SPEC: crate::cli::entity_cmd::CachedListSpec =
    crate::cli::entity_cmd::CachedListSpec {
        columns: RSLT_COLUMNS,
        entity_name: "result",
        entity_prefix: "RSLT",
        empty_message: "No results found.",
    };

#[derive(clap::Args, Debug)]
pub struct ListArgs {
    /// Filter by verdict
    #[arg(long, default_value = "all")]
    pub verdict: VerdictFilter,

    /// Filter by status
    #[arg(long, short = 's', default_value = "all")]
    pub status: StatusFilter,

    /// Filter by test ID (shows results for a specific test)
    #[arg(long, short = 't')]
    pub test: Option<String>,

    /// Filter by category (case-insensitive)
    #[arg(long, short = 'c')]
    pub category: Option<String>,

    /// Filter by tag (case-insensitive)
    #[arg(long)]
    pub tag: Option<String>,

    /// Filter by executor (substring match)
    #[arg(long, short = 'e')]
    pub executed_by: Option<String>,

    /// Filter by author (substring match)
    #[arg(long, short = 'a')]
    pub author: Option<String>,

    /// Search in title and notes (case-insensitive substring)
    #[arg(long)]
    pub search: Option<String>,

    /// Show only results with failures
    #[arg(long)]
    pub with_failures: bool,

    /// Show only results with deviations
    #[arg(long)]
    pub with_deviations: bool,

    /// Show results executed in last N days
    #[arg(long)]
    pub recent: Option<u32>,

    /// Columns to display (comma-separated)
    #[arg(long, value_delimiter = ',', default_values_t = vec![ListColumn::Short, ListColumn::Test, ListColumn::Verdict, ListColumn::Status, ListColumn::Author, ListColumn::Created])]
    pub columns: Vec<ListColumn>,

    /// Sort by field (default: created)
    #[arg(long, default_value = "created")]
    pub sort: ListColumn,

    /// Reverse sort order
    #[arg(long, short = 'r')]
    pub reverse: bool,

    /// Limit output to N items
    #[arg(long, short = 'n')]
    pub limit: Option<usize>,

    /// Show count only, not the items
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
    /// Test ID to create a result for
    #[arg(long, short = 't')]
    pub test: Option<String>,

    /// Verdict (pass/fail/conditional/incomplete/not_applicable)
    #[arg(long, default_value = "pass")]
    pub verdict: String,

    /// Title (if not provided, uses test title + date)
    #[arg(long)]
    pub title: Option<String>,

    /// Category
    #[arg(long, short = 'c')]
    pub category: Option<String>,

    /// Person who executed the test
    #[arg(long, short = 'e')]
    pub executed_by: Option<String>,

    /// Use interactive wizard to fill in fields
    #[arg(long, short = 'i')]
    pub interactive: bool,

    /// Open in editor after creation
    #[arg(long)]
    pub edit: bool,

    /// Don't open in editor after creation
    #[arg(long)]
    pub no_edit: bool,

    /// Link to another entity (auto-infers link type)
    #[arg(long, short = 'L')]
    pub link: Vec<String>,
}

#[derive(clap::Args, Debug)]
pub struct ShowArgs {
    /// Result ID or fuzzy search term
    pub id: String,

    /// Show linked test too
    #[arg(long)]
    pub with_test: bool,
}

#[derive(clap::Args, Debug)]
pub struct EditArgs {
    /// Result ID or fuzzy search term
    pub id: String,
}

#[derive(clap::Args, Debug)]
pub struct DeleteArgs {
    /// Result ID or short ID (RSLT@N)
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
    /// Result ID or short ID (RSLT@N)
    pub id: String,

    /// Force archive even if other entities reference this one
    #[arg(long)]
    pub force: bool,

    /// Suppress output
    #[arg(long, short = 'q')]
    pub quiet: bool,
}

/// Directories where results are stored
const RESULT_DIRS: &[&str] = &["verification/results", "validation/results"];

#[derive(clap::Args, Debug)]
pub struct SummaryArgs {
    /// Show results for specific test only
    #[arg(long, short = 't')]
    pub test: Option<String>,

    /// Include detailed breakdown by test type
    #[arg(long, short = 'd')]
    pub detailed: bool,

    /// Show list of uncovered requirements (no test verification)
    #[arg(long, short = 'u')]
    pub uncovered: bool,

    /// Filter requirements by type (input/output) for coverage check
    #[arg(long)]
    pub req_type: Option<String>,
}

pub fn run(cmd: RsltCommands, global: &GlobalOpts) -> Result<()> {
    match cmd {
        RsltCommands::List(args) => run_list(args, global),
        RsltCommands::New(args) => run_new(args, global),
        RsltCommands::Show(args) => run_show(args, global),
        RsltCommands::Edit(args) => run_edit(args),
        RsltCommands::Delete(args) => run_delete(args),
        RsltCommands::Archive(args) => run_archive(args),
        RsltCommands::Summary(args) => run_summary(args, global),
    }
}

/// Build a ResultFilter from CLI ListArgs
fn build_rslt_filter(args: &ListArgs, short_ids: &ShortIdIndex) -> ResultFilter {
    use tdt_core::core::entity::Status;

    // Map verdict (Issues is handled as post-filter since it's multiple values)
    let verdict = match args.verdict {
        VerdictFilter::Pass => Some(Verdict::Pass),
        VerdictFilter::Fail => Some(Verdict::Fail),
        VerdictFilter::Conditional => Some(Verdict::Conditional),
        VerdictFilter::Incomplete => Some(Verdict::Incomplete),
        VerdictFilter::NotApplicable => Some(Verdict::NotApplicable),
        VerdictFilter::Issues | VerdictFilter::All => None, // Handled as post-filter
    };

    // Map status (Active is handled as post-filter since it's "all non-obsolete")
    let status = match args.status {
        StatusFilter::Draft => Some(vec![Status::Draft]),
        StatusFilter::Review => Some(vec![Status::Review]),
        StatusFilter::Approved => Some(vec![Status::Approved]),
        StatusFilter::Released => Some(vec![Status::Released]),
        StatusFilter::Obsolete => Some(vec![Status::Obsolete]),
        StatusFilter::Active | StatusFilter::All => None, // Handled as post-filter or no filter
    };

    // Resolve test ID filter if provided
    let test_id = args
        .test
        .as_ref()
        .map(|t| short_ids.resolve(t).unwrap_or_else(|| t.clone()));

    ResultFilter {
        common: CommonFilter {
            status,
            author: args.author.clone(),
            tags: args.tag.clone().map(|t| vec![t]),
            search: args.search.clone(),
            recent_days: None, // Use result-specific recent_days
            limit: None,       // Apply limit after post-filters
            ..Default::default()
        },
        verdict,
        test_id,
        category: args.category.clone(),
        executed_by: args.executed_by.clone(),
        with_failures: args.with_failures,
        with_deviations: args.with_deviations,
        recent_days: args.recent,
        sort: build_rslt_sort_field(&args.sort),
        sort_direction: if args.reverse {
            SortDirection::Descending
        } else {
            SortDirection::Ascending
        },
    }
}

/// Convert CLI sort column to ResultSortField
fn build_rslt_sort_field(col: &ListColumn) -> ResultSortField {
    match col {
        ListColumn::Short | ListColumn::Id => ResultSortField::Id,
        ListColumn::Title => ResultSortField::Title,
        ListColumn::Test => ResultSortField::Test,
        ListColumn::Verdict => ResultSortField::Verdict,
        ListColumn::Status => ResultSortField::Status,
        ListColumn::Author => ResultSortField::Author,
        ListColumn::Created => ResultSortField::Created,
    }
}

/// Sort cached results according to CLI args
fn sort_cached_results(results: &mut Vec<tdt_core::core::cache::CachedResult>, args: &ListArgs) {
    match args.sort {
        ListColumn::Short | ListColumn::Id => results.sort_by(|a, b| a.id.cmp(&b.id)),
        ListColumn::Title => results.sort_by(|a, b| a.title.cmp(&b.title)),
        ListColumn::Test => results.sort_by(|a, b| {
            a.test_id
                .as_deref()
                .unwrap_or("")
                .cmp(b.test_id.as_deref().unwrap_or(""))
        }),
        ListColumn::Verdict => results.sort_by(|a, b| {
            let verdict_order = |v: Option<&str>| match v {
                Some("fail") => 0,
                Some("conditional") => 1,
                Some("incomplete") => 2,
                Some("pass") => 3,
                _ => 4,
            };
            verdict_order(a.verdict.as_deref()).cmp(&verdict_order(b.verdict.as_deref()))
        }),
        ListColumn::Status => results.sort_by(|a, b| a.status.cmp(&b.status)),
        ListColumn::Author => results.sort_by(|a, b| a.author.cmp(&b.author)),
        ListColumn::Created => results.sort_by(|a, b| a.created.cmp(&b.created)),
    }

    if args.reverse {
        results.reverse();
    }

    if let Some(limit) = args.limit {
        results.truncate(limit);
    }
}

/// Output full result entities
fn output_results(
    results: &[TestResult],
    short_ids: &mut ShortIdIndex,
    args: &ListArgs,
    format: OutputFormat,
    project: &Project,
) -> Result<()> {
    if results.is_empty() {
        match format {
            OutputFormat::Json => println!("[]"),
            OutputFormat::Yaml => println!("[]"),
            _ => {
                println!("No results found.");
                println!();
                println!("Create one with: {}", style("tdt rslt new").yellow());
            }
        }
        return Ok(());
    }

    if args.count {
        println!("{}", results.len());
        return Ok(());
    }

    // Update short ID index with current results
    short_ids.ensure_all(results.iter().map(|r| r.id.to_string()));
    super::utils::save_short_ids(short_ids, project);

    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&results).into_diagnostic()?;
            println!("{}", json);
        }
        OutputFormat::Yaml => {
            let yaml = serde_yml::to_string(&results).into_diagnostic()?;
            print!("{}", yaml);
        }
        OutputFormat::Csv
        | OutputFormat::Tsv
        | OutputFormat::Md
        | OutputFormat::Table
        | OutputFormat::Dot
        | OutputFormat::Tree => {
            let columns: Vec<&str> = args
                .columns
                .iter()
                .filter(|c| !matches!(c, ListColumn::Short))
                .map(|c| c.to_string().leak() as &str)
                .collect();

            let rows: Vec<TableRow> = results
                .iter()
                .map(|r| result_to_row(r, short_ids))
                .collect();

            let config = TableConfig {
                wrap_width: args.wrap,
                show_summary: true,
            };
            let formatter = TableFormatter::new(RSLT_COLUMNS, "result", "RSLT").with_config(config);
            formatter.output(rows, format, &columns);
        }
        OutputFormat::Id | OutputFormat::ShortId => {
            for result in results {
                if format == OutputFormat::ShortId {
                    let short_id = short_ids
                        .get_short_id(&result.id.to_string())
                        .unwrap_or_default();
                    println!("{}", short_id);
                } else {
                    println!("{}", result.id);
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
    let mut short_ids = ShortIdIndex::load(&project);
    let cache = EntityCache::open(&project).map_err(|e| miette::miette!("{}", e))?;
    let service = ResultService::new(&project, &cache);

    // Resolve linked-to filter via cache
    let allowed_ids = crate::cli::helpers::resolve_linked_to(
        &args.linked_to,
        args.via.as_deref(),
        &short_ids,
        &cache,
    );

    // Determine output format
    let format = match global.output {
        OutputFormat::Auto => OutputFormat::Tsv,
        f => f,
    };

    // Build filter from CLI args
    let filter = build_rslt_filter(&args, &short_ids);

    // Determine if we can use fast cache path:
    // - No with_failures filter (requires step_results)
    // - No with_deviations filter (requires deviations)
    // - No tag filter (requires full entity)
    // - Not JSON/YAML output (need full entity for serialization)
    let can_use_cache = !args.with_failures
        && !args.with_deviations
        && args.tag.is_none()
        && !matches!(format, OutputFormat::Json | OutputFormat::Yaml);

    if can_use_cache {
        // Fast path using cache
        let mut cached_results = service
            .list_cached(&filter)
            .map_err(|e| miette::miette!("{}", e))?;

        // Apply linked-to filter
        if let Some(ref ids) = allowed_ids {
            cached_results.retain(|e| ids.contains(&e.id));
        }

        // Apply post-filters not handled by cache:
        // Status: Active filter (all non-obsolete)
        if matches!(args.status, StatusFilter::Active) {
            cached_results.retain(|r| r.status != tdt_core::core::entity::Status::Obsolete);
        }

        // Verdict: Issues filter (fail, conditional, incomplete)
        if matches!(args.verdict, VerdictFilter::Issues) {
            cached_results.retain(|r| {
                r.verdict
                    .as_deref()
                    .is_some_and(|v| v == "fail" || v == "conditional" || v == "incomplete")
            });
        }

        // Sort and limit
        sort_cached_results(&mut cached_results, &args);

        let visible: Vec<String> = args
            .columns
            .iter()
            .filter(|c| !matches!(c, ListColumn::Short))
            .map(|c| c.to_string())
            .collect();
        return crate::cli::entity_cmd::output_cached_list(
            &cached_results,
            &RSLT_LIST_SPEC,
            &visible,
            args.wrap,
            args.count,
            format,
            &project,
            |r| cached_result_to_row(r, &short_ids),
            |r| r.file_path.as_path(),
        );
    }

    // Full entity loading path
    let mut results = service
        .list(&filter)
        .map_err(|e| miette::miette!("{}", e))?;

    // Apply linked-to filter
    if let Some(ref ids) = allowed_ids {
        results.retain(|e| ids.contains(&e.id.to_string()));
    }

    // Apply post-filters not handled by service:
    // Status: Active filter (all non-obsolete)
    if matches!(args.status, StatusFilter::Active) {
        results.retain(|r| r.status != tdt_core::core::entity::Status::Obsolete);
    }

    // Verdict: Issues filter (fail, conditional, incomplete)
    if matches!(args.verdict, VerdictFilter::Issues) {
        results.retain(|r| {
            matches!(
                r.verdict,
                Verdict::Fail | Verdict::Conditional | Verdict::Incomplete
            )
        });
    }

    // Apply limit (service already sorted)
    if let Some(limit) = args.limit {
        results.truncate(limit);
    }

    output_results(&results, &mut short_ids, &args, format, &project)
}

/// Convert a TestResult to a TableRow
fn result_to_row(result: &TestResult, short_ids: &ShortIdIndex) -> TableRow {
    let test_short = short_ids
        .get_short_id(&result.test_id.to_string())
        .unwrap_or_else(|| format_short_id(&result.test_id));

    TableRow::new(result.id.to_string(), short_ids)
        .cell("id", CellValue::Id(result.id.to_string()))
        .cell(
            "title",
            CellValue::Text(result.title.as_deref().unwrap_or("Untitled").to_string()),
        )
        .cell("test", CellValue::ShortId(test_short))
        .cell("verdict", CellValue::Verdict(result.verdict.to_string()))
        .cell("status", CellValue::Status(result.status))
        .cell("author", CellValue::Text(result.author.clone()))
        .cell("created", CellValue::Date(result.created))
}

/// Convert a CachedResult to a TableRow
fn cached_result_to_row(
    result: &tdt_core::core::cache::CachedResult,
    short_ids: &ShortIdIndex,
) -> TableRow {
    let test_short = result
        .test_id
        .as_ref()
        .and_then(|t| short_ids.get_short_id(t))
        .unwrap_or_else(|| result.test_id.as_deref().unwrap_or("-").to_string());

    TableRow::new(result.id.clone(), short_ids)
        .cell("id", CellValue::Id(result.id.clone()))
        .cell("title", CellValue::Text(result.title.clone()))
        .cell("test", CellValue::ShortId(test_short))
        .cell(
            "verdict",
            CellValue::Verdict(result.verdict.as_deref().unwrap_or("-").to_string()),
        )
        .cell("status", CellValue::Status(result.status))
        .cell("author", CellValue::Text(result.author.clone()))
        .cell("created", CellValue::Date(result.created))
}

fn run_new(args: NewArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let cache = EntityCache::open(&project).map_err(|e| miette::miette!("{}", e))?;
    let config = Config::load();
    let service = ResultService::new(&project, &cache);

    // Load short IDs early for test resolution
    let mut short_ids = ShortIdIndex::load(&project);

    // Determine values - either from schema-driven wizard or args
    let (test_id, verdict, title, category, executed_by, verdict_rationale, duration, notes) =
        if args.interactive {
            // Use the schema-driven wizard
            let wizard = SchemaWizard::new();
            let result = wizard.run(EntityPrefix::Rslt)?;

            let test_id_str = result
                .get_string("test_id")
                .map(String::from)
                .unwrap_or_default();

            let test_id = if test_id_str.is_empty() {
                return Err(miette::miette!("Test ID is required"));
            } else {
                let resolved = short_ids
                    .resolve(&test_id_str)
                    .unwrap_or_else(|| test_id_str.clone());
                EntityId::parse(&resolved).map_err(|e| miette::miette!("Invalid test ID: {}", e))?
            };

            let verdict = result
                .get_string("verdict")
                .map(|s| match s {
                    "fail" => Verdict::Fail,
                    "conditional" => Verdict::Conditional,
                    "incomplete" => Verdict::Incomplete,
                    "not_applicable" => Verdict::NotApplicable,
                    _ => Verdict::Pass,
                })
                .unwrap_or(Verdict::Pass);

            let title = result.get_string("title").map(String::from);

            let category = result.get_string("category").map(String::from);

            let executed_by = result
                .get_string("executed_by")
                .map(String::from)
                .unwrap_or_else(|| config.author());

            let verdict_rationale = result.get_string("verdict_rationale").map(String::from);
            let duration = result.get_string("duration").map(String::from);
            let notes = result.get_string("notes").map(String::from);

            (
                test_id,
                verdict,
                title,
                category,
                executed_by,
                verdict_rationale,
                duration,
                notes,
            )
        } else {
            // Default mode - use args with defaults
            let test_id = if let Some(test_query) = &args.test {
                // Try to resolve the test ID
                let resolved = short_ids
                    .resolve(test_query)
                    .unwrap_or_else(|| test_query.clone());
                EntityId::parse(&resolved)
                    .map_err(|e| miette::miette!("Invalid test ID '{}': {}", test_query, e))?
            } else {
                return Err(miette::miette!("Test ID is required. Use --test <TEST_ID>"));
            };

            let verdict = match args.verdict.to_lowercase().as_str() {
                "pass" => Verdict::Pass,
                "fail" => Verdict::Fail,
                "conditional" => Verdict::Conditional,
                "incomplete" => Verdict::Incomplete,
                "not_applicable" | "na" | "n/a" => Verdict::NotApplicable,
                v => {
                    return Err(miette::miette!(
                        "Invalid verdict: '{}'. Use 'pass', 'fail', 'conditional', 'incomplete', or 'not_applicable'",
                        v
                    ))
                }
            };

            let title = args.title;
            let category = args.category;
            let executed_by = args.executed_by.unwrap_or_else(|| config.author());

            (
                test_id,
                verdict,
                title,
                category,
                executed_by,
                None,
                None,
                None,
            )
        };

    // Create result via service
    let input = CreateResult {
        test_id: test_id.clone(),
        test_revision: None,
        title,
        verdict,
        verdict_rationale,
        category,
        tags: Vec::new(),
        executed_by: executed_by.clone(),
        executed_date: None,
        sample_info: None,
        environment: None,
        duration,
        notes,
        status: None,
        author: config.author(),
    };

    let rslt = service
        .create(input)
        .map_err(|e| miette::miette!("{}", e))?;

    // Get file path for the created result (service determines correct directory)
    let file_path = service.get_file_path(&rslt);

    // Add to short ID index
    let short_id = short_ids.add(rslt.id.to_string());
    super::utils::save_short_ids(&mut short_ids, &project);

    // Handle --link flags
    let added_links = crate::cli::entity_cmd::process_link_flags(
        &file_path,
        EntityPrefix::Rslt,
        &args.link,
        &short_ids,
    );

    // Output based on format flag
    match global.output {
        OutputFormat::Id => {
            println!("{}", rslt.id);
        }
        OutputFormat::ShortId => {
            println!(
                "{}",
                short_id
                    .clone()
                    .unwrap_or_else(|| format_short_id(&rslt.id))
            );
        }
        OutputFormat::Path => {
            println!("{}", file_path.display());
        }
        _ => {
            println!(
                "{} Created result {}",
                style("✓").green(),
                style(
                    short_id
                        .clone()
                        .unwrap_or_else(|| format_short_id(&rslt.id))
                )
                .cyan()
            );
            println!("   {}", style(file_path.display()).dim());
            println!(
                "   Test: {} | Verdict: {}",
                style(format_short_id(&test_id)).yellow(),
                match rslt.verdict {
                    Verdict::Pass => style(rslt.verdict.to_string()).green(),
                    Verdict::Fail => style(rslt.verdict.to_string()).red(),
                    _ => style(rslt.verdict.to_string()).yellow(),
                }
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

    // Open in editor if requested (or by default unless --no-edit)
    if args.edit || (!args.no_edit && !args.interactive) {
        println!();
        println!("Opening in {}...", style(config.editor()).yellow());

        config.run_editor(&file_path).into_diagnostic()?;
    }

    Ok(())
}

/// Determine the test type (verification or validation) by finding the test file
fn determine_test_type(project: &Project, test_id: &EntityId) -> Result<String> {
    // Check verification protocols
    let verification_path = project
        .root()
        .join(format!("verification/protocols/{}.tdt.yaml", test_id));
    if verification_path.exists() {
        return Ok("verification".to_string());
    }

    // Check validation protocols
    let validation_path = project
        .root()
        .join(format!("validation/protocols/{}.tdt.yaml", test_id));
    if validation_path.exists() {
        return Ok("validation".to_string());
    }

    // Default to verification if test not found
    Ok("verification".to_string())
}

fn run_show(args: ShowArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let cache = EntityCache::open(&project).map_err(|e| miette::miette!("{}", e))?;

    // Resolve short ID if needed
    let short_ids = ShortIdIndex::load(&project);
    let resolved_id = short_ids
        .resolve(&args.id)
        .unwrap_or_else(|| args.id.clone());

    // Use ResultService to get the result (cache-first lookup)
    let service = ResultService::new(&project, &cache);
    let result = service
        .get(&resolved_id)
        .map_err(|e| miette::miette!("{}", e))?
        .ok_or_else(|| miette::miette!("No result found matching '{}'", args.id))?;

    // Output based on format (pretty is default)
    match global.output {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&result).into_diagnostic()?;
            println!("{}", json);
        }
        OutputFormat::Yaml => {
            let yaml = serde_yml::to_string(&result).into_diagnostic()?;
            print!("{}", yaml);
        }
        OutputFormat::Id | OutputFormat::ShortId => {
            if global.output == OutputFormat::ShortId {
                let short_id = short_ids
                    .get_short_id(&result.id.to_string())
                    .unwrap_or_default();
                println!("{}", short_id);
            } else {
                println!("{}", result.id);
            }
        }
        _ => {
            // Reopen cache for title lookups (format_link_with_title expects Option<EntityCache>)
            let cache_opt = EntityCache::open(&project).ok();

            // Human-readable format
            println!("{}", style("─".repeat(60)).dim());
            println!(
                "{}: {}",
                style("ID").bold(),
                style(&result.id.to_string()).cyan()
            );
            let test_display =
                format_link_with_title(&result.test_id.to_string(), &short_ids, &cache_opt);
            println!(
                "{}: {}",
                style("Test").bold(),
                style(&test_display).yellow()
            );
            if let Some(ref title) = result.title {
                println!("{}: {}", style("Title").bold(), title);
            }

            // Verdict with color
            let verdict_styled = match result.verdict {
                Verdict::Pass => style(result.verdict.to_string()).green().bold(),
                Verdict::Fail => style(result.verdict.to_string()).red().bold(),
                Verdict::Conditional => style(result.verdict.to_string()).yellow().bold(),
                Verdict::Incomplete => style(result.verdict.to_string()).yellow(),
                Verdict::NotApplicable => style(result.verdict.to_string()).dim(),
            };
            println!("{}: {}", style("Verdict").bold(), verdict_styled);

            if let Some(ref rationale) = result.verdict_rationale {
                if !rationale.is_empty() {
                    println!("{}: {}", style("Rationale").bold(), rationale.trim());
                }
            }

            println!("{}: {}", style("Status").bold(), result.status);
            println!(
                "{}: {} ({})",
                style("Executed").bold(),
                result.executed_by,
                result.executed_date.format("%Y-%m-%d %H:%M")
            );

            if let Some(ref cat) = result.category {
                if !cat.is_empty() {
                    println!("{}: {}", style("Category").bold(), cat);
                }
            }
            println!("{}", style("─".repeat(60)).dim());

            // Step Results Summary
            if !result.step_results.is_empty() {
                println!();
                println!("{}", style("Step Results:").bold());
                let pass_count = result
                    .step_results
                    .iter()
                    .filter(|s| s.result == tdt_core::entities::result::StepResult::Pass)
                    .count();
                let fail_count = result
                    .step_results
                    .iter()
                    .filter(|s| s.result == tdt_core::entities::result::StepResult::Fail)
                    .count();
                let total = result.step_results.len();

                println!(
                    "  {} total | {} passed | {} failed",
                    total,
                    style(pass_count).green(),
                    style(fail_count).red()
                );

                if let Some(rate) = result.pass_rate() {
                    println!("  Pass rate: {:.1}%", rate);
                }
            }

            // Failures
            if !result.failures.is_empty() {
                println!();
                println!("{}", style("Failures:").bold().red());
                for (i, failure) in result.failures.iter().enumerate() {
                    println!(
                        "  {}. {}{}",
                        i + 1,
                        failure.description,
                        failure
                            .step
                            .map(|s| format!(" (step {})", s))
                            .unwrap_or_default()
                    );
                    if let Some(ref cause) = failure.root_cause {
                        println!("     {}: {}", style("Cause").dim(), cause);
                    }
                }
            }

            // Deviations
            if !result.deviations.is_empty() {
                println!();
                println!("{}", style("Deviations:").bold().yellow());
                for (i, deviation) in result.deviations.iter().enumerate() {
                    println!("  {}. {}", i + 1, deviation.description);
                }
            }

            // Notes
            if let Some(ref notes) = result.notes {
                if !notes.is_empty() {
                    println!();
                    println!("{}", style("Notes:").bold());
                    println!("{}", notes);
                }
            }

            println!();
            println!("{}", style("─".repeat(60)).dim());
            println!(
                "{}: {} | {}: {} | {}: {}",
                style("Author").dim(),
                result.author,
                style("Created").dim(),
                result.created.format("%Y-%m-%d %H:%M"),
                style("Revision").dim(),
                result.revision
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

    // Use ResultService to get the entity
    let service = ResultService::new(&project, &cache);
    let result = service
        .get(&resolved_id)
        .map_err(|e| miette::miette!("{}", e))?
        .ok_or_else(|| miette::miette!("No result found matching '{}'", args.id))?;

    // Get file path from cache
    let file_path = if let Some(cached) = cache.get_entity(&result.id.to_string()) {
        if cached.file_path.is_absolute() {
            cached.file_path.clone()
        } else {
            project.root().join(&cached.file_path)
        }
    } else {
        // Fallback: compute path from test type
        let test_type = determine_test_type(&project, &result.test_id)?;
        project
            .root()
            .join(format!("{}/results/{}.tdt.yaml", test_type, result.id))
    };

    if !file_path.exists() {
        return Err(miette::miette!("File not found: {}", file_path.display()));
    }

    println!(
        "Opening {} in {}...",
        style(format_short_id(&result.id)).cyan(),
        style(config.editor()).yellow()
    );

    config.run_editor(&file_path).into_diagnostic()?;

    Ok(())
}

fn run_delete(args: DeleteArgs) -> Result<()> {
    crate::cli::commands::utils::run_delete(&args.id, RESULT_DIRS, args.force, false, args.quiet)
}

fn run_archive(args: ArchiveArgs) -> Result<()> {
    crate::cli::commands::utils::run_delete(&args.id, RESULT_DIRS, args.force, true, args.quiet)
}

fn run_summary(args: SummaryArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let cache = EntityCache::open(&project)?;
    let short_ids = ShortIdIndex::load(&project);

    // Resolve test filter if provided
    let test_filter = args
        .test
        .as_ref()
        .map(|t| short_ids.resolve(t).unwrap_or_else(|| t.clone()));

    // Get all results from cache
    let results = cache.list_results(
        None,                   // status
        test_filter.as_deref(), // test_id
        None,                   // verdict
        None,                   // author
        None,                   // search
        None,                   // limit
    );

    // Get uncovered requirements if requested or for display
    let uncovered_reqs = get_uncovered_requirements(&cache, args.req_type.as_deref());

    if results.is_empty() {
        match global.output {
            OutputFormat::Json => println!("{{}}"),
            OutputFormat::Yaml => println!("{{}}"),
            _ => {
                println!("No test results found.");
                println!();
                println!(
                    "Create one with: {}",
                    style("tdt rslt new --test <TEST_ID>").yellow()
                );
            }
        }
        return Ok(());
    }

    // Calculate statistics
    let total = results.len();
    let mut pass_count = 0usize;
    let mut fail_count = 0usize;
    let mut conditional_count = 0usize;
    let mut incomplete_count = 0usize;
    let mut na_count = 0usize;
    let mut verification_count = 0usize;
    let mut validation_count = 0usize;

    // Track recent failures (last 30 days)
    let thirty_days_ago = chrono::Utc::now() - chrono::Duration::days(30);
    let mut recent_failures: Vec<&tdt_core::core::cache::CachedResult> = Vec::new();

    for result in &results {
        // Count by verdict
        match result.verdict.as_deref() {
            Some("pass") => pass_count += 1,
            Some("fail") => {
                fail_count += 1;
                // Check if recent failure
                if let Some(date_str) = &result.executed_date {
                    if let Ok(date) = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
                        if date >= thirty_days_ago.date_naive() {
                            recent_failures.push(result);
                        }
                    }
                }
            }
            Some("conditional") => conditional_count += 1,
            Some("incomplete") => incomplete_count += 1,
            Some("not_applicable") => na_count += 1,
            _ => {}
        }

        // Count by test type (inferred from file path)
        let path_str = result.file_path.to_string_lossy();
        if path_str.contains("verification") {
            verification_count += 1;
        } else if path_str.contains("validation") {
            validation_count += 1;
        }
    }

    let pass_rate = if total > 0 {
        (pass_count as f64 / total as f64) * 100.0
    } else {
        0.0
    };

    // Get requirement coverage
    let coverage = cache.requirement_coverage();

    // Output based on format
    match global.output {
        OutputFormat::Json => {
            let summary = serde_json::json!({
                "total": total,
                "by_verdict": {
                    "pass": pass_count,
                    "fail": fail_count,
                    "conditional": conditional_count,
                    "incomplete": incomplete_count,
                    "not_applicable": na_count
                },
                "pass_rate": pass_rate,
                "by_type": {
                    "verification": verification_count,
                    "validation": validation_count
                },
                "recent_failures": recent_failures.iter().map(|r| {
                    serde_json::json!({
                        "id": r.id,
                        "test_id": r.test_id,
                        "title": r.title,
                        "executed_date": r.executed_date
                    })
                }).collect::<Vec<_>>(),
                "requirement_coverage": {
                    "total": coverage.total_requirements,
                    "with_tests": coverage.with_tests,
                    "without_tests": coverage.without_tests,
                    "percent": coverage.coverage_percent,
                    "uncovered_ids": uncovered_reqs.iter().map(|(id, _)| id.clone()).collect::<Vec<_>>()
                }
            });
            println!(
                "{}",
                serde_json::to_string_pretty(&summary).unwrap_or_default()
            );
        }
        OutputFormat::Yaml => {
            let summary = serde_json::json!({
                "total": total,
                "by_verdict": {
                    "pass": pass_count,
                    "fail": fail_count,
                    "conditional": conditional_count,
                    "incomplete": incomplete_count,
                    "not_applicable": na_count
                },
                "pass_rate": pass_rate,
                "by_type": {
                    "verification": verification_count,
                    "validation": validation_count
                },
                "requirement_coverage": {
                    "total": coverage.total_requirements,
                    "with_tests": coverage.with_tests,
                    "percent": coverage.coverage_percent,
                    "uncovered_ids": uncovered_reqs.iter().map(|(id, _)| id.clone()).collect::<Vec<_>>()
                }
            });
            println!("{}", serde_yml::to_string(&summary).unwrap_or_default());
        }
        _ => {
            // Human-readable output
            println!();
            println!("{}", style("Test Results Summary").bold().cyan());
            println!("{}", style("─".repeat(50)).dim());

            if let Some(ref test_id) = args.test {
                let display_id = short_ids
                    .get_short_id(&test_filter.clone().unwrap_or_default())
                    .unwrap_or_else(|| test_id.clone());
                println!("Filtered by test: {}", style(display_id).yellow());
                println!();
            }

            println!("{}: {}", style("Total Results").bold(), total);
            println!();

            // Verdict breakdown with colors
            let pass_pct = if total > 0 {
                (pass_count as f64 / total as f64) * 100.0
            } else {
                0.0
            };
            let fail_pct = if total > 0 {
                (fail_count as f64 / total as f64) * 100.0
            } else {
                0.0
            };
            let cond_pct = if total > 0 {
                (conditional_count as f64 / total as f64) * 100.0
            } else {
                0.0
            };
            let inc_pct = if total > 0 {
                (incomplete_count as f64 / total as f64) * 100.0
            } else {
                0.0
            };

            println!(
                "  {}: {:>4} ({:>5.1}%)",
                style("Pass").green(),
                pass_count,
                pass_pct
            );
            println!(
                "  {}: {:>4} ({:>5.1}%)",
                style("Fail").red(),
                fail_count,
                fail_pct
            );
            println!(
                "  {}: {:>4} ({:>5.1}%)",
                style("Conditional").yellow(),
                conditional_count,
                cond_pct
            );
            println!(
                "  {}: {:>4} ({:>5.1}%)",
                style("Incomplete").yellow(),
                incomplete_count,
                inc_pct
            );
            if na_count > 0 {
                let na_pct = (na_count as f64 / total as f64) * 100.0;
                println!(
                    "  {}: {:>4} ({:>5.1}%)",
                    style("N/A").dim(),
                    na_count,
                    na_pct
                );
            }

            if args.detailed {
                println!();
                println!("{}", style("By Type:").bold());
                println!("  Verification: {} results", verification_count);
                println!("  Validation:   {} results", validation_count);
            }

            // Recent failures
            if !recent_failures.is_empty() {
                println!();
                println!("{}", style("Recent Failures (last 30 days):").bold().red());
                // Sort by date descending
                let mut sorted_failures = recent_failures.clone();
                sorted_failures
                    .sort_by(|a, b| b.executed_date.as_ref().cmp(&a.executed_date.as_ref()));
                for failure in sorted_failures.iter().take(5) {
                    let short_id = short_ids
                        .get_short_id(&failure.id)
                        .unwrap_or_else(|| truncate_str(&failure.id, 8));
                    let test_short = failure
                        .test_id
                        .as_ref()
                        .and_then(|t| short_ids.get_short_id(t))
                        .unwrap_or_else(|| "?".to_string());
                    println!(
                        "  {} {} \"{}\" {}",
                        style(short_id).red(),
                        style(test_short).dim(),
                        truncate_str(&failure.title, 30),
                        style(failure.executed_date.as_deref().unwrap_or("-")).dim()
                    );
                }
                if sorted_failures.len() > 5 {
                    println!("  ... and {} more", sorted_failures.len() - 5);
                }
            }

            // Requirement coverage
            println!();
            println!("{}", style("Requirement Coverage:").bold());
            let cov_color = if coverage.coverage_percent >= 80.0 {
                console::Style::new().green()
            } else if coverage.coverage_percent >= 50.0 {
                console::Style::new().yellow()
            } else {
                console::Style::new().red()
            };
            println!(
                "  {} ({}/{} requirements have tests)",
                cov_color.apply_to(format!("{:.1}%", coverage.coverage_percent)),
                coverage.with_tests,
                coverage.total_requirements
            );

            // Show uncovered requirements if requested or if there are few
            if !uncovered_reqs.is_empty() && (args.uncovered || uncovered_reqs.len() <= 10) {
                println!();
                println!("{}", style("Uncovered Requirements:").bold().red());
                println!("{}", style("─".repeat(50)).dim());
                for (id, title) in &uncovered_reqs {
                    let short = short_ids
                        .get_short_id(id)
                        .unwrap_or_else(|| format_short_id_str(id));
                    println!(
                        "  {} {} - {}",
                        style("○").red(),
                        style(&short).cyan(),
                        truncate_str(title, 40)
                    );
                }
            } else if !uncovered_reqs.is_empty() {
                println!();
                println!(
                    "  {} uncovered requirements. Use {} to see the list",
                    style(uncovered_reqs.len()).red(),
                    style("tdt rslt summary --uncovered").yellow()
                );
            }
        }
    }

    Ok(())
}

/// Get list of uncovered requirements (no test verification)
fn get_uncovered_requirements(
    cache: &EntityCache,
    req_type_filter: Option<&str>,
) -> Vec<(String, String)> {
    // Get all requirements
    let requirements = cache.list_requirements(None, None, None, None, None, None, None);

    // Get all tests to check verifies links
    let tests = cache.list_tests(None, None, None, None, None, None, None, None, None);

    // Build set of requirement IDs verified by tests
    let mut verified_ids: std::collections::HashSet<String> = std::collections::HashSet::new();

    for test in &tests {
        let links = cache.get_links_from(&test.id);
        for link in links {
            if link.link_type == "verifies" && link.target_id.starts_with("REQ-") {
                verified_ids.insert(link.target_id);
            }
        }
    }

    // Also check reverse links (requirement.links.verified_by)
    for req in &requirements {
        let links = cache.get_links_to(&req.id);
        for link in links {
            if link.link_type == "verified_by" {
                verified_ids.insert(req.id.clone());
            }
        }
    }

    // Filter to uncovered and optionally by type
    let mut uncovered = Vec::new();
    for req in &requirements {
        // Apply type filter if specified
        if let Some(filter) = req_type_filter {
            let req_type = req.req_type.as_deref().unwrap_or("");
            if !req_type.eq_ignore_ascii_case(filter) {
                continue;
            }
        }

        if !verified_ids.contains(&req.id) {
            uncovered.push((req.id.clone(), req.title.clone()));
        }
    }

    uncovered
}
