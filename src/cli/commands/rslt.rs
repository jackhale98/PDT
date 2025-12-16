//! `tdt rslt` command - Test result management

use clap::{Subcommand, ValueEnum};
use console::style;
use miette::{IntoDiagnostic, Result};
use std::fs;

use crate::cli::commands::utils::format_link_with_title;
use crate::cli::helpers::{escape_csv, format_short_id, format_short_id_str, truncate_str};
use crate::cli::{GlobalOpts, OutputFormat};
use crate::core::cache::EntityCache;
use crate::core::identity::{EntityId, EntityPrefix};
use crate::core::project::Project;
use crate::core::shortid::ShortIdIndex;
use crate::core::Config;
use crate::entities::result::{Result as TestResult, Verdict};
use crate::schema::template::{TemplateContext, TemplateGenerator};
use crate::schema::wizard::SchemaWizard;

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

/// Status filter
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum StatusFilter {
    Draft,
    Review,
    Approved,
    Released,
    Obsolete,
    /// All active (not obsolete)
    Active,
    /// All statuses
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
        RsltCommands::New(args) => run_new(args),
        RsltCommands::Show(args) => run_show(args, global),
        RsltCommands::Edit(args) => run_edit(args),
        RsltCommands::Summary(args) => run_summary(args, global),
    }
}

fn run_list(args: ListArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let mut short_ids = ShortIdIndex::load(&project);

    // Determine output format
    let format = match global.format {
        OutputFormat::Auto => OutputFormat::Tsv,
        f => f,
    };

    // Check if we can use the fast cache path:
    // - No category filter (not in cache)
    // - No tag filter (not in cache)
    // - No with_failures filter (requires step_results)
    // - No with_deviations filter (requires deviations)
    // - Not JSON/YAML output (need full entity for serialization)
    let can_use_cache = args.category.is_none()
        && args.tag.is_none()
        && !args.with_failures
        && !args.with_deviations
        && !matches!(format, OutputFormat::Json | OutputFormat::Yaml);

    if can_use_cache {
        if let Ok(cache) = EntityCache::open(&project) {
            let status_filter = match args.status {
                StatusFilter::Draft => Some("draft"),
                StatusFilter::Review => Some("review"),
                StatusFilter::Approved => Some("approved"),
                StatusFilter::Released => Some("released"),
                StatusFilter::Obsolete => Some("obsolete"),
                StatusFilter::Active | StatusFilter::All => None,
            };

            let verdict_filter = match args.verdict {
                VerdictFilter::Pass => Some("pass"),
                VerdictFilter::Fail => Some("fail"),
                VerdictFilter::Conditional => Some("conditional"),
                VerdictFilter::Incomplete => Some("incomplete"),
                VerdictFilter::NotApplicable => Some("not_applicable"),
                VerdictFilter::Issues | VerdictFilter::All => None,
            };

            // Resolve test ID filter if provided
            let test_filter = args
                .test
                .as_ref()
                .map(|t| short_ids.resolve(t).unwrap_or_else(|| t.clone()));

            let mut cached_results = cache.list_results(
                status_filter,
                test_filter.as_deref(),
                verdict_filter,
                args.author.as_deref(),
                args.search.as_deref(),
                None, // Apply limit after additional filtering
            );

            // Apply filters that need post-processing
            // Status: Active filter (all non-obsolete)
            if matches!(args.status, StatusFilter::Active) {
                cached_results.retain(|r| r.status != "obsolete");
            }

            // Verdict: Issues filter (fail, conditional, incomplete)
            if matches!(args.verdict, VerdictFilter::Issues) {
                cached_results.retain(|r| {
                    r.verdict
                        .as_deref()
                        .is_some_and(|v| v == "fail" || v == "conditional" || v == "incomplete")
                });
            }

            // Executed by filter (substring match)
            if let Some(ref exec_filter) = args.executed_by {
                let exec_lower = exec_filter.to_lowercase();
                cached_results.retain(|r| {
                    r.executed_by
                        .as_ref()
                        .is_some_and(|e| e.to_lowercase().contains(&exec_lower))
                });
            }

            // Recent filter (executed in last N days)
            if let Some(days) = args.recent {
                let cutoff = chrono::Utc::now() - chrono::Duration::days(days as i64);
                cached_results.retain(|r| {
                    r.executed_date.as_ref().is_some_and(|d| {
                        chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d")
                            .map(|nd| nd >= cutoff.date_naive())
                            .unwrap_or(false)
                    })
                });
            }

            // Sort
            match args.sort {
                ListColumn::Short | ListColumn::Id => {
                    cached_results.sort_by(|a, b| a.id.cmp(&b.id))
                }
                ListColumn::Title => cached_results.sort_by(|a, b| a.title.cmp(&b.title)),
                ListColumn::Test => cached_results.sort_by(|a, b| {
                    a.test_id
                        .as_deref()
                        .unwrap_or("")
                        .cmp(b.test_id.as_deref().unwrap_or(""))
                }),
                ListColumn::Verdict => cached_results.sort_by(|a, b| {
                    let verdict_order = |v: Option<&str>| match v {
                        Some("fail") => 0,
                        Some("conditional") => 1,
                        Some("incomplete") => 2,
                        Some("pass") => 3,
                        _ => 4,
                    };
                    verdict_order(a.verdict.as_deref()).cmp(&verdict_order(b.verdict.as_deref()))
                }),
                ListColumn::Status => cached_results.sort_by(|a, b| a.status.cmp(&b.status)),
                ListColumn::Author => cached_results.sort_by(|a, b| a.author.cmp(&b.author)),
                ListColumn::Created => cached_results.sort_by(|a, b| a.created.cmp(&b.created)),
            }

            if args.reverse {
                cached_results.reverse();
            }

            if let Some(limit) = args.limit {
                cached_results.truncate(limit);
            }

            return output_cached_results(&cached_results, &short_ids, &args, format);
        }
    }

    // Fall back to full YAML loading
    // Collect all result files from both verification and validation directories
    let mut results: Vec<TestResult> = Vec::new();

    // Check verification results
    let verification_dir = project.root().join("verification/results");
    if verification_dir.exists() {
        for entry in walkdir::WalkDir::new(&verification_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        {
            match crate::yaml::parse_yaml_file::<TestResult>(entry.path()) {
                Ok(result) => results.push(result),
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

    // Check validation results
    let validation_dir = project.root().join("validation/results");
    if validation_dir.exists() {
        for entry in walkdir::WalkDir::new(&validation_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        {
            match crate::yaml::parse_yaml_file::<TestResult>(entry.path()) {
                Ok(result) => results.push(result),
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

    // Apply filters
    results.retain(|r| {
        // Verdict filter
        let verdict_match = match args.verdict {
            VerdictFilter::Pass => r.verdict == Verdict::Pass,
            VerdictFilter::Fail => r.verdict == Verdict::Fail,
            VerdictFilter::Conditional => r.verdict == Verdict::Conditional,
            VerdictFilter::Incomplete => r.verdict == Verdict::Incomplete,
            VerdictFilter::NotApplicable => r.verdict == Verdict::NotApplicable,
            VerdictFilter::Issues => matches!(
                r.verdict,
                Verdict::Fail | Verdict::Conditional | Verdict::Incomplete
            ),
            VerdictFilter::All => true,
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

        // Test ID filter
        let test_match = args.test.as_ref().is_none_or(|test_query| {
            let test_id = r.test_id.to_string();
            test_id.contains(test_query) || test_id.starts_with(test_query)
        });

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
                .any(|tg| tg.to_lowercase() == tag.to_lowercase())
        });

        // Executed by filter
        let executed_by_match = args
            .executed_by
            .as_ref()
            .is_none_or(|ex| r.executed_by.to_lowercase().contains(&ex.to_lowercase()));

        // Author filter
        let author_match = args
            .author
            .as_ref()
            .is_none_or(|author| r.author.to_lowercase().contains(&author.to_lowercase()));

        // Search filter
        let search_match = args.search.as_ref().is_none_or(|search| {
            let search_lower = search.to_lowercase();
            r.title
                .as_ref()
                .is_some_and(|t| t.to_lowercase().contains(&search_lower))
                || r.notes
                    .as_ref()
                    .is_some_and(|n| n.to_lowercase().contains(&search_lower))
        });

        // Failures filter
        let failures_match = !args.with_failures || r.has_failures();

        // Deviations filter
        let deviations_match = !args.with_deviations || r.has_deviations();

        // Recent filter (executed in last N days)
        let recent_match = args.recent.is_none_or(|days| {
            let cutoff = chrono::Utc::now() - chrono::Duration::days(days as i64);
            r.executed_date >= cutoff
        });

        verdict_match
            && status_match
            && test_match
            && category_match
            && tag_match
            && executed_by_match
            && author_match
            && search_match
            && failures_match
            && deviations_match
            && recent_match
    });

    if results.is_empty() {
        match global.format {
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

    // Sort by specified column
    match args.sort {
        ListColumn::Short | ListColumn::Id => {
            results.sort_by(|a, b| a.id.to_string().cmp(&b.id.to_string()))
        }
        ListColumn::Title => results.sort_by(|a, b| {
            a.title
                .as_deref()
                .unwrap_or("")
                .cmp(b.title.as_deref().unwrap_or(""))
        }),
        ListColumn::Test => {
            results.sort_by(|a, b| a.test_id.to_string().cmp(&b.test_id.to_string()))
        }
        ListColumn::Verdict => results.sort_by(|a, b| {
            let verdict_order = |v: &Verdict| match v {
                Verdict::Fail => 0,
                Verdict::Conditional => 1,
                Verdict::Incomplete => 2,
                Verdict::Pass => 3,
                Verdict::NotApplicable => 4,
            };
            verdict_order(&a.verdict).cmp(&verdict_order(&b.verdict))
        }),
        ListColumn::Status => {
            results.sort_by(|a, b| a.status.to_string().cmp(&b.status.to_string()))
        }
        ListColumn::Author => results.sort_by(|a, b| a.author.cmp(&b.author)),
        ListColumn::Created => results.sort_by(|a, b| a.created.cmp(&b.created)),
    }

    // Reverse if requested
    if args.reverse {
        results.reverse();
    }

    // Apply limit
    if let Some(limit) = args.limit {
        results.truncate(limit);
    }

    // Just count?
    if args.count {
        println!("{}", results.len());
        return Ok(());
    }

    // Update short ID index with current results (preserves other entity types)
    short_ids.ensure_all(results.iter().map(|r| r.id.to_string()));
    let _ = short_ids.save(&project);

    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&results).into_diagnostic()?;
            println!("{}", json);
        }
        OutputFormat::Yaml => {
            let yaml = serde_yml::to_string(&results).into_diagnostic()?;
            print!("{}", yaml);
        }
        OutputFormat::Csv => {
            println!("short_id,id,test_id,verdict,status,executed_by,executed_date");
            for result in &results {
                let short_id = short_ids
                    .get_short_id(&result.id.to_string())
                    .unwrap_or_default();
                println!(
                    "{},{},{},{},{},{},{}",
                    short_id,
                    result.id,
                    format_short_id(&result.test_id),
                    result.verdict,
                    result.status,
                    escape_csv(&result.executed_by),
                    result.executed_date.format("%Y-%m-%d")
                );
            }
        }
        OutputFormat::Tsv => {
            // Build header parts based on columns
            let mut header_parts = Vec::new();
            let mut widths = Vec::new();

            for col in &args.columns {
                let (header, width) = match col {
                    ListColumn::Short => ("SHORT", 8),
                    ListColumn::Id => ("ID", 17),
                    ListColumn::Title => ("TITLE", 25),
                    ListColumn::Test => ("TEST", 8),
                    ListColumn::Verdict => ("VERDICT", 12),
                    ListColumn::Status => ("STATUS", 10),
                    ListColumn::Author => ("AUTHOR", 15),
                    ListColumn::Created => ("CREATED", 12),
                };
                header_parts.push(style(header).bold().to_string());
                widths.push(width);
            }

            // Print header
            let header_line = header_parts
                .iter()
                .zip(&widths)
                .map(|(h, w)| format!("{:<width$}", h, width = w))
                .collect::<Vec<_>>()
                .join(" ");
            println!("{}", header_line);

            let total_width: usize = widths.iter().sum::<usize>() + (widths.len() - 1);
            println!("{}", "-".repeat(total_width));

            for result in &results {
                // Build row parts based on columns
                let mut row_parts = Vec::new();

                for col in &args.columns {
                    let part = match col {
                        ListColumn::Short => short_ids
                            .get_short_id(&result.id.to_string())
                            .unwrap_or_else(|| "?".to_string()),
                        ListColumn::Id => format_short_id(&result.id),
                        ListColumn::Title => {
                            let title = result.title.as_deref().unwrap_or("Untitled");
                            truncate_str(title, 23)
                        }
                        ListColumn::Test => short_ids
                            .get_short_id(&result.test_id.to_string())
                            .unwrap_or_else(|| format_short_id(&result.test_id)),
                        ListColumn::Verdict => match result.verdict {
                            Verdict::Pass => style(result.verdict.to_string()).green().to_string(),
                            Verdict::Fail => {
                                style(result.verdict.to_string()).red().bold().to_string()
                            }
                            Verdict::Conditional => {
                                style(result.verdict.to_string()).yellow().to_string()
                            }
                            Verdict::Incomplete => {
                                style(result.verdict.to_string()).yellow().to_string()
                            }
                            Verdict::NotApplicable => style("n/a").dim().to_string(),
                        },
                        ListColumn::Status => result.status.to_string(),
                        ListColumn::Author => truncate_str(&result.author, 13),
                        ListColumn::Created => result.created.format("%Y-%m-%d").to_string(),
                    };
                    row_parts.push(part);
                }

                // Print row
                let row_line = row_parts
                    .iter()
                    .zip(&widths)
                    .map(|(p, w)| format!("{:<width$}", p, width = w))
                    .collect::<Vec<_>>()
                    .join(" ");
                println!("{}", row_line);
            }

            println!();
            println!("{} result(s) found.", style(results.len()).cyan());
        }
        OutputFormat::Id => {
            for result in &results {
                println!("{}", result.id);
            }
        }
        OutputFormat::Md => {
            println!("| Short | ID | Test | Verdict | Status | Executed By | Date |");
            println!("|---|---|---|---|---|---|---|");
            for result in &results {
                let short_id = short_ids
                    .get_short_id(&result.id.to_string())
                    .unwrap_or_default();
                println!(
                    "| {} | {} | {} | {} | {} | {} | {} |",
                    short_id,
                    format_short_id(&result.id),
                    format_short_id(&result.test_id),
                    result.verdict,
                    result.status,
                    result.executed_by,
                    result.executed_date.format("%Y-%m-%d")
                );
            }
        }
        OutputFormat::Auto => unreachable!(),
    }

    Ok(())
}

/// Output cached results (fast path - no YAML parsing needed)
fn output_cached_results(
    results: &[crate::core::cache::CachedResult],
    short_ids: &ShortIdIndex,
    args: &ListArgs,
    format: OutputFormat,
) -> Result<()> {
    if results.is_empty() {
        println!("No results found.");
        println!();
        println!("Create one with: {}", style("tdt rslt new").yellow());
        return Ok(());
    }

    if args.count {
        println!("{}", results.len());
        return Ok(());
    }

    match format {
        OutputFormat::Csv => {
            println!("short_id,id,test_id,verdict,status,executed_by,executed_date");
            for result in results {
                let short_id = short_ids.get_short_id(&result.id).unwrap_or_default();
                println!(
                    "{},{},{},{},{},{},{}",
                    short_id,
                    result.id,
                    result.test_id.as_deref().unwrap_or(""),
                    result.verdict.as_deref().unwrap_or(""),
                    result.status,
                    escape_csv(result.executed_by.as_deref().unwrap_or("")),
                    result.executed_date.as_deref().unwrap_or("")
                );
            }
        }
        OutputFormat::Tsv => {
            // Build header parts based on columns
            let mut header_parts = Vec::new();
            let mut widths = Vec::new();

            for col in &args.columns {
                let (header, width) = match col {
                    ListColumn::Short => ("SHORT", 8),
                    ListColumn::Id => ("ID", 17),
                    ListColumn::Title => ("TITLE", 25),
                    ListColumn::Test => ("TEST", 8),
                    ListColumn::Verdict => ("VERDICT", 12),
                    ListColumn::Status => ("STATUS", 10),
                    ListColumn::Author => ("AUTHOR", 15),
                    ListColumn::Created => ("CREATED", 12),
                };
                header_parts.push(style(header).bold().to_string());
                widths.push(width);
            }

            // Print header
            let header_line = header_parts
                .iter()
                .zip(&widths)
                .map(|(h, w)| format!("{:<width$}", h, width = w))
                .collect::<Vec<_>>()
                .join(" ");
            println!("{}", header_line);

            let total_width: usize = widths.iter().sum::<usize>() + (widths.len() - 1);
            println!("{}", "-".repeat(total_width));

            for result in results {
                let mut row_parts = Vec::new();

                for col in &args.columns {
                    let part = match col {
                        ListColumn::Short => short_ids
                            .get_short_id(&result.id)
                            .unwrap_or_else(|| "?".to_string()),
                        ListColumn::Id => truncate_str(&result.id, 15),
                        ListColumn::Title => truncate_str(&result.title, 23),
                        ListColumn::Test => result
                            .test_id
                            .as_ref()
                            .and_then(|t| short_ids.get_short_id(t))
                            .unwrap_or_else(|| {
                                result
                                    .test_id
                                    .as_ref()
                                    .map(|t| truncate_str(t, 6))
                                    .unwrap_or_else(|| "-".to_string())
                            }),
                        ListColumn::Verdict => {
                            let v = result.verdict.as_deref().unwrap_or("");
                            match v {
                                "pass" => style(v).green().to_string(),
                                "fail" => style(v).red().bold().to_string(),
                                "conditional" | "incomplete" => style(v).yellow().to_string(),
                                "not_applicable" => style("n/a").dim().to_string(),
                                _ => v.to_string(),
                            }
                        }
                        ListColumn::Status => result.status.clone(),
                        ListColumn::Author => truncate_str(&result.author, 13),
                        ListColumn::Created => result.created.format("%Y-%m-%d").to_string(),
                    };
                    row_parts.push(part);
                }

                // Print row
                let row_line = row_parts
                    .iter()
                    .zip(&widths)
                    .map(|(p, w)| format!("{:<width$}", p, width = w))
                    .collect::<Vec<_>>()
                    .join(" ");
                println!("{}", row_line);
            }

            println!();
            println!("{} result(s) found.", style(results.len()).cyan());
        }
        OutputFormat::Id => {
            for result in results {
                println!("{}", result.id);
            }
        }
        OutputFormat::Md => {
            println!("| Short | ID | Test | Verdict | Status | Executed By | Date |");
            println!("|---|---|---|---|---|---|---|");
            for result in results {
                let short_id = short_ids.get_short_id(&result.id).unwrap_or_default();
                println!(
                    "| {} | {} | {} | {} | {} | {} | {} |",
                    short_id,
                    truncate_str(&result.id, 15),
                    result.test_id.as_deref().unwrap_or("-"),
                    result.verdict.as_deref().unwrap_or("-"),
                    result.status,
                    result.executed_by.as_deref().unwrap_or("-"),
                    result.executed_date.as_deref().unwrap_or("-")
                );
            }
        }
        OutputFormat::Json | OutputFormat::Yaml | OutputFormat::Auto => {
            unreachable!()
        }
    }

    Ok(())
}

fn run_new(args: NewArgs) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let config = Config::load();

    // Determine values - either from schema-driven wizard or args
    let (test_id, verdict, title, category, executed_by, verdict_rationale) = if args.interactive {
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
            EntityId::parse(&test_id_str).map_err(|e| miette::miette!("Invalid test ID: {}", e))?
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

        let category = result
            .get_string("category")
            .map(String::from)
            .unwrap_or_default();

        let executed_by = result
            .get_string("executed_by")
            .map(String::from)
            .unwrap_or_else(|| config.author());

        let verdict_rationale = result.get_string("verdict_rationale").map(String::from);

        (
            test_id,
            verdict,
            title,
            category,
            executed_by,
            verdict_rationale,
        )
    } else {
        // Default mode - use args with defaults
        let test_id = if let Some(test_query) = &args.test {
            // Try to resolve the test ID
            let short_ids = ShortIdIndex::load(&project);
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
        let category = args.category.unwrap_or_default();
        let executed_by = args.executed_by.unwrap_or_else(|| config.author());

        (test_id, verdict, title, category, executed_by, None)
    };

    // Determine test type by looking up the test
    let test_type = determine_test_type(&project, &test_id)?;

    // Generate entity ID and create from template
    let id = EntityId::new(EntityPrefix::Rslt);
    let author = config.author();

    let generator = TemplateGenerator::new().map_err(|e| miette::miette!("{}", e))?;
    let mut ctx = TemplateContext::new(id.clone(), author)
        .with_test_id(test_id.clone())
        .with_verdict(verdict.to_string())
        .with_executed_by(&executed_by)
        .with_category(&category);

    if let Some(ref t) = title {
        ctx = ctx.with_title(t);
    }

    let mut yaml_content = generator
        .generate_result(&ctx)
        .map_err(|e| miette::miette!("{}", e))?;

    // Apply wizard values via string replacement (for interactive mode)
    if args.interactive {
        if let Some(ref rationale) = verdict_rationale {
            if !rationale.is_empty() {
                let indented = rationale
                    .lines()
                    .map(|line| format!("  {}", line))
                    .collect::<Vec<_>>()
                    .join("\n");
                yaml_content = yaml_content.replace(
                    "verdict_rationale: |\n  # Explain the verdict\n  # Especially important for fail or conditional results",
                    &format!("verdict_rationale: |\n{}", indented),
                );
            }
        }
    }

    // Determine output directory based on test type
    let output_dir = project.result_directory(&test_type);

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
    let _ = short_ids.save(&project);

    println!(
        "{} Created result {}",
        style("✓").green(),
        style(short_id.unwrap_or_else(|| format_short_id(&id))).cyan()
    );
    println!("   {}", style(file_path.display()).dim());
    println!(
        "   Test: {} | Verdict: {}",
        style(format_short_id(&test_id)).yellow(),
        match verdict {
            Verdict::Pass => style(verdict.to_string()).green(),
            Verdict::Fail => style(verdict.to_string()).red(),
            _ => style(verdict.to_string()).yellow(),
        }
    );

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

    // Find the result by ID prefix match
    let result = find_result(&project, &args.id)?;

    // Output based on format (pretty is default)
    match global.format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&result).into_diagnostic()?;
            println!("{}", json);
        }
        OutputFormat::Yaml => {
            let yaml = serde_yml::to_string(&result).into_diagnostic()?;
            print!("{}", yaml);
        }
        OutputFormat::Id => {
            println!("{}", result.id);
        }
        _ => {
            // Load cache and short IDs for title lookups
            let short_ids = ShortIdIndex::load(&project);
            let cache = EntityCache::open(&project).ok();

            // Human-readable format
            println!("{}", style("─".repeat(60)).dim());
            println!(
                "{}: {}",
                style("ID").bold(),
                style(&result.id.to_string()).cyan()
            );
            let test_display =
                format_link_with_title(&result.test_id.to_string(), &short_ids, &cache);
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
                    .filter(|s| s.result == crate::entities::result::StepResult::Pass)
                    .count();
                let fail_count = result
                    .step_results
                    .iter()
                    .filter(|s| s.result == crate::entities::result::StepResult::Fail)
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
    let config = Config::load();

    // Find the result by ID prefix match
    let result = find_result(&project, &args.id)?;

    // Determine test type to find correct directory
    let test_type = determine_test_type(&project, &result.test_id)?;

    let file_path = project
        .root()
        .join(format!("{}/results/{}.tdt.yaml", test_type, result.id));

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

/// Find a result by ID prefix match or short ID (@N)
fn find_result(project: &Project, id_query: &str) -> Result<TestResult> {
    // First, try to resolve short ID (@N) to full ID
    let short_ids = ShortIdIndex::load(project);
    let resolved_query = short_ids
        .resolve(id_query)
        .unwrap_or_else(|| id_query.to_string());

    let mut matches: Vec<(TestResult, std::path::PathBuf)> = Vec::new();

    // Search both verification and validation directories
    for subdir in &["verification/results", "validation/results"] {
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
            if let Ok(result) = crate::yaml::parse_yaml_file::<TestResult>(entry.path()) {
                // Check if ID matches (prefix or full)
                let id_str = result.id.to_string();
                if id_str.starts_with(&resolved_query) || id_str == resolved_query {
                    matches.push((result, entry.path().to_path_buf()));
                }
                // Also check title for fuzzy match (only if not a short ID lookup)
                else if !id_query.starts_with('@')
                    && !id_query.chars().all(|c| c.is_ascii_digit())
                {
                    if let Some(ref title) = result.title {
                        if title
                            .to_lowercase()
                            .contains(&resolved_query.to_lowercase())
                        {
                            matches.push((result, entry.path().to_path_buf()));
                        }
                    }
                }
            }
        }
    }

    match matches.len() {
        0 => Err(miette::miette!("No result found matching '{}'", id_query)),
        1 => Ok(matches.remove(0).0),
        _ => {
            println!("{} Multiple matches found:", style("!").yellow());
            for (result, _path) in &matches {
                println!(
                    "  {} - {}",
                    format_short_id(&result.id),
                    result.title.as_deref().unwrap_or("Untitled")
                );
            }
            Err(miette::miette!(
                "Ambiguous query '{}'. Please be more specific.",
                id_query
            ))
        }
    }
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
        match global.format {
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
    let mut recent_failures: Vec<&crate::core::cache::CachedResult> = Vec::new();

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
    match global.format {
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
