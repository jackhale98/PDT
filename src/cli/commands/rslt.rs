//! `pdt rslt` command - Test result management

use clap::{Subcommand, ValueEnum};
use console::style;
use miette::{IntoDiagnostic, Result};
use std::fs;

use crate::cli::{GlobalOpts, OutputFormat};
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
    Id,
    TestId,
    Title,
    Verdict,
    Status,
    ExecutedBy,
    ExecutedDate,
    Category,
    Author,
    Created,
}

impl std::fmt::Display for ListColumn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ListColumn::Id => write!(f, "id"),
            ListColumn::TestId => write!(f, "test_id"),
            ListColumn::Title => write!(f, "title"),
            ListColumn::Verdict => write!(f, "verdict"),
            ListColumn::Status => write!(f, "status"),
            ListColumn::ExecutedBy => write!(f, "executed_by"),
            ListColumn::ExecutedDate => write!(f, "executed_date"),
            ListColumn::Category => write!(f, "category"),
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

    /// Sort by field (default: executed_date)
    #[arg(long, default_value = "executed-date")]
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

pub fn run(cmd: RsltCommands, global: &GlobalOpts) -> Result<()> {
    match cmd {
        RsltCommands::List(args) => run_list(args, global),
        RsltCommands::New(args) => run_new(args),
        RsltCommands::Show(args) => run_show(args, global),
        RsltCommands::Edit(args) => run_edit(args),
    }
}

fn run_list(args: ListArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;

    // Collect all result files from both verification and validation directories
    let mut results: Vec<TestResult> = Vec::new();

    // Check verification results
    let verification_dir = project.root().join("verification/results");
    if verification_dir.exists() {
        for entry in walkdir::WalkDir::new(&verification_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().to_string_lossy().ends_with(".pdt.yaml"))
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
            .filter(|e| e.path().to_string_lossy().ends_with(".pdt.yaml"))
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
            VerdictFilter::Issues => matches!(r.verdict, Verdict::Fail | Verdict::Conditional | Verdict::Incomplete),
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
        let test_match = args.test.as_ref().map_or(true, |test_query| {
            let test_id = r.test_id.to_string();
            test_id.contains(test_query) || test_id.starts_with(test_query)
        });

        // Category filter (case-insensitive)
        let category_match = args.category.as_ref().map_or(true, |cat| {
            r.category.as_ref().map_or(false, |c| c.to_lowercase() == cat.to_lowercase())
        });

        // Tag filter (case-insensitive)
        let tag_match = args.tag.as_ref().map_or(true, |tag| {
            r.tags.iter().any(|tg| tg.to_lowercase() == tag.to_lowercase())
        });

        // Executed by filter
        let executed_by_match = args.executed_by.as_ref().map_or(true, |ex| {
            r.executed_by.to_lowercase().contains(&ex.to_lowercase())
        });

        // Author filter
        let author_match = args.author.as_ref().map_or(true, |author| {
            r.author.to_lowercase().contains(&author.to_lowercase())
        });

        // Search filter
        let search_match = args.search.as_ref().map_or(true, |search| {
            let search_lower = search.to_lowercase();
            r.title.as_ref().map_or(false, |t| t.to_lowercase().contains(&search_lower))
                || r.notes.as_ref().map_or(false, |n| n.to_lowercase().contains(&search_lower))
        });

        // Failures filter
        let failures_match = !args.with_failures || r.has_failures();

        // Deviations filter
        let deviations_match = !args.with_deviations || r.has_deviations();

        // Recent filter (executed in last N days)
        let recent_match = args.recent.map_or(true, |days| {
            let cutoff = chrono::Utc::now() - chrono::Duration::days(days as i64);
            r.executed_date >= cutoff
        });

        verdict_match && status_match && test_match && category_match && tag_match
            && executed_by_match && author_match && search_match
            && failures_match && deviations_match && recent_match
    });

    if results.is_empty() {
        match global.format {
            OutputFormat::Json => println!("[]"),
            OutputFormat::Yaml => println!("[]"),
            _ => {
                println!("No results found.");
                println!();
                println!("Create one with: {}", style("pdt rslt new").yellow());
            }
        }
        return Ok(());
    }

    // Sort by specified column
    match args.sort {
        ListColumn::Id => results.sort_by(|a, b| a.id.to_string().cmp(&b.id.to_string())),
        ListColumn::TestId => results.sort_by(|a, b| a.test_id.to_string().cmp(&b.test_id.to_string())),
        ListColumn::Title => results.sort_by(|a, b| {
            a.title.as_deref().unwrap_or("").cmp(b.title.as_deref().unwrap_or(""))
        }),
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
        ListColumn::Status => results.sort_by(|a, b| a.status.to_string().cmp(&b.status.to_string())),
        ListColumn::ExecutedBy => results.sort_by(|a, b| a.executed_by.cmp(&b.executed_by)),
        ListColumn::ExecutedDate => results.sort_by(|a, b| a.executed_date.cmp(&b.executed_date)),
        ListColumn::Category => results.sort_by(|a, b| {
            a.category.as_deref().unwrap_or("").cmp(b.category.as_deref().unwrap_or(""))
        }),
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

    // Rebuild short ID index with current results
    let mut short_ids = ShortIdIndex::new();
    short_ids.rebuild(results.iter().map(|r| r.id.to_string()));
    let _ = short_ids.save(&project);

    // Output based on format
    let format = match global.format {
        OutputFormat::Auto => OutputFormat::Tsv,
        f => f,
    };

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
                let short_id = short_ids.get_short_id(&result.id.to_string()).unwrap_or(0);
                println!(
                    "@{},{},{},{},{},{},{}",
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
            println!(
                "{:<5} {:<17} {:<17} {:<12} {:<10} {:<15} {:<12}",
                style("@").bold().dim(),
                style("ID").bold(),
                style("TEST").bold(),
                style("VERDICT").bold(),
                style("STATUS").bold(),
                style("EXECUTED BY").bold(),
                style("DATE").bold()
            );
            println!("{}", "-".repeat(95));

            for result in &results {
                let short_id = short_ids.get_short_id(&result.id.to_string()).unwrap_or(0);
                let id_display = format_short_id(&result.id);
                let test_display = format_short_id(&result.test_id);
                let executor_truncated = truncate_str(&result.executed_by, 13);

                // Color verdict based on result
                let verdict_display = match result.verdict {
                    Verdict::Pass => style(result.verdict.to_string()).green().to_string(),
                    Verdict::Fail => style(result.verdict.to_string()).red().bold().to_string(),
                    Verdict::Conditional => style(result.verdict.to_string()).yellow().to_string(),
                    Verdict::Incomplete => style(result.verdict.to_string()).yellow().to_string(),
                    Verdict::NotApplicable => style("n/a").dim().to_string(),
                };

                println!(
                    "{:<5} {:<17} {:<17} {:<12} {:<10} {:<15} {:<12}",
                    style(format!("@{}", short_id)).cyan(),
                    id_display,
                    test_display,
                    verdict_display,
                    result.status,
                    executor_truncated,
                    result.executed_date.format("%Y-%m-%d")
                );
            }

            println!();
            println!(
                "{} result(s) found. Use {} to reference by short ID.",
                style(results.len()).cyan(),
                style("@N").cyan()
            );
        }
        OutputFormat::Id => {
            for result in &results {
                println!("{}", result.id);
            }
        }
        OutputFormat::Md => {
            println!("| @ | ID | Test | Verdict | Status | Executed By | Date |");
            println!("|---|---|---|---|---|---|---|");
            for result in &results {
                let short_id = short_ids.get_short_id(&result.id.to_string()).unwrap_or(0);
                println!(
                    "| @{} | {} | {} | {} | {} | {} | {} |",
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

fn escape_csv(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn run_new(args: NewArgs) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let config = Config::load();

    // Determine values - either from schema-driven wizard or args
    let (test_id, verdict, title, category, executed_by) = if args.interactive {
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

        let title = result
            .get_string("title")
            .map(String::from);

        let category = result
            .get_string("category")
            .map(String::from)
            .unwrap_or_default();

        let executed_by = result
            .get_string("executed_by")
            .map(String::from)
            .unwrap_or_else(|| config.author());

        (test_id, verdict, title, category, executed_by)
    } else {
        // Default mode - use args with defaults
        let test_id = if let Some(test_query) = &args.test {
            // Try to resolve the test ID
            let short_ids = ShortIdIndex::load(&project);
            let resolved = short_ids.resolve(test_query).unwrap_or_else(|| test_query.clone());
            EntityId::parse(&resolved).map_err(|e| miette::miette!("Invalid test ID '{}': {}", test_query, e))?
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

        (test_id, verdict, title, category, executed_by)
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

    let yaml_content = generator
        .generate_result(&ctx)
        .map_err(|e| miette::miette!("{}", e))?;

    // Determine output directory based on test type
    let output_dir = project.result_directory(&test_type);

    // Ensure directory exists
    if !output_dir.exists() {
        fs::create_dir_all(&output_dir).into_diagnostic()?;
    }

    let file_path = output_dir.join(format!("{}.pdt.yaml", id));

    // Write file
    fs::write(&file_path, &yaml_content).into_diagnostic()?;

    println!(
        "{} Created result {}",
        style("✓").green(),
        style(format_short_id(&id)).cyan()
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
        let editor = config.editor();
        println!();
        println!("Opening in {}...", style(&editor).yellow());

        std::process::Command::new(&editor)
            .arg(&file_path)
            .status()
            .into_diagnostic()?;
    }

    Ok(())
}

/// Determine the test type (verification or validation) by finding the test file
fn determine_test_type(project: &Project, test_id: &EntityId) -> Result<String> {
    // Check verification protocols
    let verification_path = project
        .root()
        .join(format!("verification/protocols/{}.pdt.yaml", test_id));
    if verification_path.exists() {
        return Ok("verification".to_string());
    }

    // Check validation protocols
    let validation_path = project
        .root()
        .join(format!("validation/protocols/{}.pdt.yaml", test_id));
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

    // Output based on format
    let format = match global.format {
        OutputFormat::Auto => OutputFormat::Yaml,
        f => f,
    };

    match format {
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
            // Human-readable format
            println!("{}", style("─".repeat(60)).dim());
            println!(
                "{}: {}",
                style("ID").bold(),
                style(&result.id.to_string()).cyan()
            );
            println!(
                "{}: {}",
                style("Test").bold(),
                style(&result.test_id.to_string()).yellow()
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
                let pass_count = result.step_results.iter()
                    .filter(|s| s.result == crate::entities::result::StepResult::Pass)
                    .count();
                let fail_count = result.step_results.iter()
                    .filter(|s| s.result == crate::entities::result::StepResult::Fail)
                    .count();
                let total = result.step_results.len();

                println!(
                    "  {} total | {} {} | {} {}",
                    total,
                    style(pass_count).green(),
                    "passed",
                    style(fail_count).red(),
                    "failed"
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
                        failure.step.map(|s| format!(" (step {})", s)).unwrap_or_default()
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
        .join(format!("{}/results/{}.pdt.yaml", test_type, result.id));

    if !file_path.exists() {
        return Err(miette::miette!(
            "File not found: {}",
            file_path.display()
        ));
    }

    let editor = config.editor();
    println!(
        "Opening {} in {}...",
        style(format_short_id(&result.id)).cyan(),
        style(&editor).yellow()
    );

    std::process::Command::new(&editor)
        .arg(&file_path)
        .status()
        .into_diagnostic()?;

    Ok(())
}

/// Find a result by ID prefix match or short ID (@N)
fn find_result(project: &Project, id_query: &str) -> Result<TestResult> {
    // First, try to resolve short ID (@N) to full ID
    let short_ids = ShortIdIndex::load(project);
    let resolved_query = short_ids.resolve(id_query).unwrap_or_else(|| id_query.to_string());

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
            .filter(|e| e.path().to_string_lossy().ends_with(".pdt.yaml"))
        {
            if let Ok(result) = crate::yaml::parse_yaml_file::<TestResult>(entry.path()) {
                // Check if ID matches (prefix or full)
                let id_str = result.id.to_string();
                if id_str.starts_with(&resolved_query) || id_str == resolved_query {
                    matches.push((result, entry.path().to_path_buf()));
                }
                // Also check title for fuzzy match (only if not a short ID lookup)
                else if !id_query.starts_with('@') && !id_query.chars().all(|c| c.is_ascii_digit()) {
                    if let Some(ref title) = result.title {
                        if title.to_lowercase().contains(&resolved_query.to_lowercase()) {
                            matches.push((result, entry.path().to_path_buf()));
                        }
                    }
                }
            }
        }
    }

    match matches.len() {
        0 => Err(miette::miette!(
            "No result found matching '{}'",
            id_query
        )),
        1 => Ok(matches.remove(0).0),
        _ => {
            println!(
                "{} Multiple matches found:",
                style("!").yellow()
            );
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

/// Format an entity ID for short display (prefix + first 8 chars of ULID)
fn format_short_id(id: &EntityId) -> String {
    let full = id.to_string();
    if full.len() > 13 {
        format!("{}...", &full[..13])
    } else {
        full
    }
}

/// Truncate a string to a maximum length, adding "..." if truncated
fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}
