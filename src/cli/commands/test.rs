//! `pdt test` command - Test protocol management (verification/validation)

use clap::{Subcommand, ValueEnum};
use console::style;
use miette::{IntoDiagnostic, Result};
use std::fs;

use crate::cli::{GlobalOpts, OutputFormat};
use crate::core::identity::{EntityId, EntityPrefix};
use crate::core::project::Project;
use crate::core::shortid::ShortIdIndex;
use crate::core::Config;
use crate::entities::test::{Test, TestLevel, TestMethod, TestType};
use crate::schema::template::{TemplateContext, TemplateGenerator};
use crate::schema::wizard::SchemaWizard;

#[derive(Subcommand, Debug)]
pub enum TestCommands {
    /// List tests with filtering
    List(ListArgs),

    /// Create a new test protocol
    New(NewArgs),

    /// Show a test's details
    Show(ShowArgs),

    /// Edit a test in your editor
    Edit(EditArgs),
}

/// Test type filter
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum TestTypeFilter {
    Verification,
    Validation,
    All,
}

/// Test level filter
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum TestLevelFilter {
    Unit,
    Integration,
    System,
    Acceptance,
    All,
}

/// Test method filter (IADT)
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum TestMethodFilter {
    Inspection,
    Analysis,
    Demonstration,
    Test,
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
    Type,
    Level,
    Method,
    Title,
    Status,
    Priority,
    Category,
    Author,
    Created,
}

impl std::fmt::Display for ListColumn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ListColumn::Id => write!(f, "id"),
            ListColumn::Type => write!(f, "type"),
            ListColumn::Level => write!(f, "level"),
            ListColumn::Method => write!(f, "method"),
            ListColumn::Title => write!(f, "title"),
            ListColumn::Status => write!(f, "status"),
            ListColumn::Priority => write!(f, "priority"),
            ListColumn::Category => write!(f, "category"),
            ListColumn::Author => write!(f, "author"),
            ListColumn::Created => write!(f, "created"),
        }
    }
}

#[derive(clap::Args, Debug)]
pub struct ListArgs {
    /// Filter by type (verification/validation)
    #[arg(long, short = 't', default_value = "all")]
    pub r#type: TestTypeFilter,

    /// Filter by test level
    #[arg(long, short = 'l', default_value = "all")]
    pub level: TestLevelFilter,

    /// Filter by test method (IADT)
    #[arg(long, short = 'm', default_value = "all")]
    pub method: TestMethodFilter,

    /// Filter by status
    #[arg(long, short = 's', default_value = "all")]
    pub status: StatusFilter,

    /// Filter by priority (low/medium/high/critical)
    #[arg(long, short = 'p')]
    pub priority: Option<String>,

    /// Filter by category (case-insensitive)
    #[arg(long, short = 'c')]
    pub category: Option<String>,

    /// Filter by tag (case-insensitive)
    #[arg(long)]
    pub tag: Option<String>,

    /// Filter by author (substring match)
    #[arg(long, short = 'a')]
    pub author: Option<String>,

    /// Search in title and objective (case-insensitive substring)
    #[arg(long)]
    pub search: Option<String>,

    /// Show only tests with no linked requirements (orphans)
    #[arg(long)]
    pub orphans: bool,

    /// Show tests created in last N days
    #[arg(long)]
    pub recent: Option<u32>,

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
    /// Test type (verification/validation)
    #[arg(long, short = 't', default_value = "verification")]
    pub r#type: String,

    /// Test level (unit/integration/system/acceptance)
    #[arg(long, short = 'l', default_value = "system")]
    pub level: String,

    /// Test method (inspection/analysis/demonstration/test)
    #[arg(long, short = 'm', default_value = "test")]
    pub method: String,

    /// Title (if not provided, uses placeholder)
    #[arg(long)]
    pub title: Option<String>,

    /// Category
    #[arg(long, short = 'c')]
    pub category: Option<String>,

    /// Priority (low/medium/high/critical)
    #[arg(long, short = 'p', default_value = "medium")]
    pub priority: String,

    /// Use interactive wizard to fill in fields
    #[arg(long, short = 'i')]
    pub interactive: bool,

    /// Open in editor after creation
    #[arg(long, short = 'e')]
    pub edit: bool,

    /// Don't open in editor after creation
    #[arg(long)]
    pub no_edit: bool,
}

#[derive(clap::Args, Debug)]
pub struct ShowArgs {
    /// Test ID or fuzzy search term
    pub id: String,

    /// Show linked entities too
    #[arg(long)]
    pub with_links: bool,
}

#[derive(clap::Args, Debug)]
pub struct EditArgs {
    /// Test ID or fuzzy search term
    pub id: String,
}

pub fn run(cmd: TestCommands, global: &GlobalOpts) -> Result<()> {
    match cmd {
        TestCommands::List(args) => run_list(args, global),
        TestCommands::New(args) => run_new(args),
        TestCommands::Show(args) => run_show(args, global),
        TestCommands::Edit(args) => run_edit(args),
    }
}

fn run_list(args: ListArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;

    // Collect all test files from both verification and validation directories
    let mut tests: Vec<Test> = Vec::new();

    // Check verification protocols
    let verification_dir = project.root().join("verification/protocols");
    if verification_dir.exists() {
        for entry in walkdir::WalkDir::new(&verification_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().to_string_lossy().ends_with(".pdt.yaml"))
        {
            match crate::yaml::parse_yaml_file::<Test>(entry.path()) {
                Ok(test) => tests.push(test),
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

    // Check validation protocols
    let validation_dir = project.root().join("validation/protocols");
    if validation_dir.exists() {
        for entry in walkdir::WalkDir::new(&validation_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().to_string_lossy().ends_with(".pdt.yaml"))
        {
            match crate::yaml::parse_yaml_file::<Test>(entry.path()) {
                Ok(test) => tests.push(test),
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
    tests.retain(|t| {
        // Type filter
        let type_match = match args.r#type {
            TestTypeFilter::Verification => t.test_type == TestType::Verification,
            TestTypeFilter::Validation => t.test_type == TestType::Validation,
            TestTypeFilter::All => true,
        };

        // Level filter
        let level_match = match args.level {
            TestLevelFilter::Unit => t.test_level == Some(TestLevel::Unit),
            TestLevelFilter::Integration => t.test_level == Some(TestLevel::Integration),
            TestLevelFilter::System => t.test_level == Some(TestLevel::System),
            TestLevelFilter::Acceptance => t.test_level == Some(TestLevel::Acceptance),
            TestLevelFilter::All => true,
        };

        // Method filter
        let method_match = match args.method {
            TestMethodFilter::Inspection => t.test_method == Some(TestMethod::Inspection),
            TestMethodFilter::Analysis => t.test_method == Some(TestMethod::Analysis),
            TestMethodFilter::Demonstration => t.test_method == Some(TestMethod::Demonstration),
            TestMethodFilter::Test => t.test_method == Some(TestMethod::Test),
            TestMethodFilter::All => true,
        };

        // Status filter
        let status_match = match args.status {
            StatusFilter::Draft => t.status == crate::core::entity::Status::Draft,
            StatusFilter::Review => t.status == crate::core::entity::Status::Review,
            StatusFilter::Approved => t.status == crate::core::entity::Status::Approved,
            StatusFilter::Released => t.status == crate::core::entity::Status::Released,
            StatusFilter::Obsolete => t.status == crate::core::entity::Status::Obsolete,
            StatusFilter::Active => t.status != crate::core::entity::Status::Obsolete,
            StatusFilter::All => true,
        };

        // Priority filter
        let priority_match = args.priority.as_ref().map_or(true, |p| {
            t.priority.to_string().to_lowercase() == p.to_lowercase()
        });

        // Category filter (case-insensitive)
        let category_match = args.category.as_ref().map_or(true, |cat| {
            t.category.as_ref().map_or(false, |c| c.to_lowercase() == cat.to_lowercase())
        });

        // Tag filter (case-insensitive)
        let tag_match = args.tag.as_ref().map_or(true, |tag| {
            t.tags.iter().any(|tg| tg.to_lowercase() == tag.to_lowercase())
        });

        // Author filter
        let author_match = args.author.as_ref().map_or(true, |author| {
            t.author.to_lowercase().contains(&author.to_lowercase())
        });

        // Search filter
        let search_match = args.search.as_ref().map_or(true, |search| {
            let search_lower = search.to_lowercase();
            t.title.to_lowercase().contains(&search_lower)
                || t.objective.to_lowercase().contains(&search_lower)
        });

        // Orphans filter (no linked requirements)
        let orphans_match = !args.orphans || (t.links.verifies.is_empty() && t.links.validates.is_empty());

        // Recent filter (created in last N days)
        let recent_match = args.recent.map_or(true, |days| {
            let cutoff = chrono::Utc::now() - chrono::Duration::days(days as i64);
            t.created >= cutoff
        });

        type_match && level_match && method_match && status_match && priority_match
            && category_match && tag_match && author_match && search_match
            && orphans_match && recent_match
    });

    if tests.is_empty() {
        match global.format {
            OutputFormat::Json => println!("[]"),
            OutputFormat::Yaml => println!("[]"),
            _ => {
                println!("No tests found.");
                println!();
                println!("Create one with: {}", style("pdt test new").yellow());
            }
        }
        return Ok(());
    }

    // Sort by specified column
    match args.sort {
        ListColumn::Id => tests.sort_by(|a, b| a.id.to_string().cmp(&b.id.to_string())),
        ListColumn::Type => tests.sort_by(|a, b| a.test_type.to_string().cmp(&b.test_type.to_string())),
        ListColumn::Level => tests.sort_by(|a, b| {
            let level_order = |l: &Option<TestLevel>| match l {
                Some(TestLevel::Unit) => 0,
                Some(TestLevel::Integration) => 1,
                Some(TestLevel::System) => 2,
                Some(TestLevel::Acceptance) => 3,
                None => 4,
            };
            level_order(&a.test_level).cmp(&level_order(&b.test_level))
        }),
        ListColumn::Method => tests.sort_by(|a, b| {
            let method_str = |m: &Option<TestMethod>| m.map(|m| m.to_string()).unwrap_or_default();
            method_str(&a.test_method).cmp(&method_str(&b.test_method))
        }),
        ListColumn::Title => tests.sort_by(|a, b| a.title.cmp(&b.title)),
        ListColumn::Status => tests.sort_by(|a, b| a.status.to_string().cmp(&b.status.to_string())),
        ListColumn::Priority => tests.sort_by(|a, b| {
            let priority_order = |p: &crate::core::entity::Priority| match p {
                crate::core::entity::Priority::Critical => 0,
                crate::core::entity::Priority::High => 1,
                crate::core::entity::Priority::Medium => 2,
                crate::core::entity::Priority::Low => 3,
            };
            priority_order(&a.priority).cmp(&priority_order(&b.priority))
        }),
        ListColumn::Category => tests.sort_by(|a, b| {
            a.category.as_deref().unwrap_or("").cmp(b.category.as_deref().unwrap_or(""))
        }),
        ListColumn::Author => tests.sort_by(|a, b| a.author.cmp(&b.author)),
        ListColumn::Created => tests.sort_by(|a, b| a.created.cmp(&b.created)),
    }

    // Reverse if requested
    if args.reverse {
        tests.reverse();
    }

    // Apply limit
    if let Some(limit) = args.limit {
        tests.truncate(limit);
    }

    // Just count?
    if args.count {
        println!("{}", tests.len());
        return Ok(());
    }

    // Rebuild short ID index with current tests
    let mut short_ids = ShortIdIndex::new();
    short_ids.rebuild(tests.iter().map(|t| t.id.to_string()));
    let _ = short_ids.save(&project);

    // Output based on format
    let format = match global.format {
        OutputFormat::Auto => OutputFormat::Tsv,
        f => f,
    };

    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&tests).into_diagnostic()?;
            println!("{}", json);
        }
        OutputFormat::Yaml => {
            let yaml = serde_yml::to_string(&tests).into_diagnostic()?;
            print!("{}", yaml);
        }
        OutputFormat::Csv => {
            println!("short_id,id,type,level,method,title,status,priority");
            for test in &tests {
                let short_id = short_ids.get_short_id(&test.id.to_string()).unwrap_or(0);
                println!(
                    "@{},{},{},{},{},{},{},{}",
                    short_id,
                    test.id,
                    test.test_type,
                    test.test_level.map_or("-".to_string(), |l| l.to_string()),
                    test.test_method.map_or("-".to_string(), |m| m.to_string()),
                    escape_csv(&test.title),
                    test.status,
                    test.priority
                );
            }
        }
        OutputFormat::Tsv => {
            println!(
                "{:<5} {:<17} {:<12} {:<8} {:<12} {:<28} {:<10} {:<8}",
                style("@").bold().dim(),
                style("ID").bold(),
                style("TYPE").bold(),
                style("LEVEL").bold(),
                style("METHOD").bold(),
                style("TITLE").bold(),
                style("STATUS").bold(),
                style("PRIO").bold()
            );
            println!("{}", "-".repeat(105));

            for test in &tests {
                let short_id = short_ids.get_short_id(&test.id.to_string()).unwrap_or(0);
                let id_display = format_short_id(&test.id);
                let title_truncated = truncate_str(&test.title, 26);
                let level_str = test.test_level.map_or("-".to_string(), |l| l.to_string());
                let method_str = test.test_method.map_or("-".to_string(), |m| m.to_string());

                println!(
                    "{:<5} {:<17} {:<12} {:<8} {:<12} {:<28} {:<10} {:<8}",
                    style(format!("@{}", short_id)).cyan(),
                    id_display,
                    test.test_type,
                    level_str,
                    method_str,
                    title_truncated,
                    test.status,
                    test.priority
                );
            }

            println!();
            println!(
                "{} test(s) found. Use {} to reference by short ID.",
                style(tests.len()).cyan(),
                style("@N").cyan()
            );
        }
        OutputFormat::Id => {
            for test in &tests {
                println!("{}", test.id);
            }
        }
        OutputFormat::Md => {
            println!("| @ | ID | Type | Level | Method | Title | Status | Priority |");
            println!("|---|---|---|---|---|---|---|---|");
            for test in &tests {
                let short_id = short_ids.get_short_id(&test.id.to_string()).unwrap_or(0);
                println!(
                    "| @{} | {} | {} | {} | {} | {} | {} | {} |",
                    short_id,
                    format_short_id(&test.id),
                    test.test_type,
                    test.test_level.map_or("-".to_string(), |l| l.to_string()),
                    test.test_method.map_or("-".to_string(), |m| m.to_string()),
                    test.title,
                    test.status,
                    test.priority
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
    let (test_type, test_level, test_method, title, category, priority) = if args.interactive {
        // Use the schema-driven wizard
        let wizard = SchemaWizard::new();
        let result = wizard.run(EntityPrefix::Test)?;

        let test_type = result
            .get_string("type")
            .map(|s| match s {
                "validation" => TestType::Validation,
                _ => TestType::Verification,
            })
            .unwrap_or(TestType::Verification);

        let test_level = result
            .get_string("test_level")
            .map(|s| match s {
                "unit" => TestLevel::Unit,
                "integration" => TestLevel::Integration,
                "acceptance" => TestLevel::Acceptance,
                _ => TestLevel::System,
            })
            .unwrap_or(TestLevel::System);

        let test_method = result
            .get_string("test_method")
            .map(|s| match s {
                "inspection" => TestMethod::Inspection,
                "analysis" => TestMethod::Analysis,
                "demonstration" => TestMethod::Demonstration,
                _ => TestMethod::Test,
            })
            .unwrap_or(TestMethod::Test);

        let title = result
            .get_string("title")
            .map(String::from)
            .unwrap_or_else(|| "New Test Protocol".to_string());

        let category = result
            .get_string("category")
            .map(String::from)
            .unwrap_or_default();

        let priority = result
            .get_string("priority")
            .map(String::from)
            .unwrap_or_else(|| "medium".to_string());

        (test_type, test_level, test_method, title, category, priority)
    } else {
        // Default mode - use args with defaults
        let test_type = match args.r#type.to_lowercase().as_str() {
            "verification" => TestType::Verification,
            "validation" => TestType::Validation,
            t => {
                return Err(miette::miette!(
                    "Invalid test type: '{}'. Use 'verification' or 'validation'",
                    t
                ))
            }
        };

        let test_level = match args.level.to_lowercase().as_str() {
            "unit" => TestLevel::Unit,
            "integration" => TestLevel::Integration,
            "system" => TestLevel::System,
            "acceptance" => TestLevel::Acceptance,
            l => {
                return Err(miette::miette!(
                    "Invalid test level: '{}'. Use 'unit', 'integration', 'system', or 'acceptance'",
                    l
                ))
            }
        };

        let test_method = match args.method.to_lowercase().as_str() {
            "inspection" => TestMethod::Inspection,
            "analysis" => TestMethod::Analysis,
            "demonstration" => TestMethod::Demonstration,
            "test" => TestMethod::Test,
            m => {
                return Err(miette::miette!(
                    "Invalid test method: '{}'. Use 'inspection', 'analysis', 'demonstration', or 'test'",
                    m
                ))
            }
        };

        let title = args.title.unwrap_or_else(|| "New Test Protocol".to_string());
        let category = args.category.unwrap_or_default();
        let priority = args.priority;

        (test_type, test_level, test_method, title, category, priority)
    };

    // Generate entity ID and create from template
    let id = EntityId::new(EntityPrefix::Test);
    let author = config.author();

    let generator = TemplateGenerator::new().map_err(|e| miette::miette!("{}", e))?;
    let ctx = TemplateContext::new(id.clone(), author)
        .with_title(&title)
        .with_test_type(test_type.to_string())
        .with_test_level(test_level.to_string())
        .with_test_method(test_method.to_string())
        .with_category(&category)
        .with_priority(&priority);

    let yaml_content = generator
        .generate_test(&ctx)
        .map_err(|e| miette::miette!("{}", e))?;

    // Determine output directory based on type
    let output_dir = project.test_directory(&test_type.to_string());

    // Ensure directory exists
    if !output_dir.exists() {
        fs::create_dir_all(&output_dir).into_diagnostic()?;
    }

    let file_path = output_dir.join(format!("{}.pdt.yaml", id));

    // Write file
    fs::write(&file_path, &yaml_content).into_diagnostic()?;

    println!(
        "{} Created test {}",
        style("✓").green(),
        style(format_short_id(&id)).cyan()
    );
    println!("   {}", style(file_path.display()).dim());
    println!(
        "   Type: {} | Level: {} | Method: {}",
        style(test_type.to_string()).yellow(),
        style(test_level.to_string()).yellow(),
        style(test_method.to_string()).yellow()
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

fn run_show(args: ShowArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;

    // Find the test by ID prefix match
    let test = find_test(&project, &args.id)?;

    // Output based on format
    let format = match global.format {
        OutputFormat::Auto => OutputFormat::Yaml,
        f => f,
    };

    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&test).into_diagnostic()?;
            println!("{}", json);
        }
        OutputFormat::Yaml => {
            let yaml = serde_yml::to_string(&test).into_diagnostic()?;
            print!("{}", yaml);
        }
        OutputFormat::Id => {
            println!("{}", test.id);
        }
        _ => {
            // Human-readable format
            println!("{}", style("─".repeat(60)).dim());
            println!(
                "{}: {}",
                style("ID").bold(),
                style(&test.id.to_string()).cyan()
            );
            println!("{}: {}", style("Type").bold(), test.test_type);
            if let Some(level) = &test.test_level {
                println!("{}: {}", style("Level").bold(), level);
            }
            if let Some(method) = &test.test_method {
                println!("{}: {}", style("Method").bold(), method);
            }
            println!(
                "{}: {}",
                style("Title").bold(),
                style(&test.title).yellow()
            );
            println!("{}: {}", style("Status").bold(), test.status);
            println!("{}: {}", style("Priority").bold(), test.priority);
            if let Some(ref cat) = test.category {
                if !cat.is_empty() {
                    println!("{}: {}", style("Category").bold(), cat);
                }
            }
            println!("{}", style("─".repeat(60)).dim());

            // Objective
            println!();
            println!("{}", style("Objective:").bold());
            println!("{}", &test.objective);

            // Description
            if let Some(ref desc) = test.description {
                if !desc.is_empty() {
                    println!();
                    println!("{}", style("Description:").bold());
                    println!("{}", desc);
                }
            }

            // Preconditions
            if !test.preconditions.is_empty() {
                println!();
                println!("{}", style("Preconditions:").bold());
                for (i, precond) in test.preconditions.iter().enumerate() {
                    println!("  {}. {}", i + 1, precond);
                }
            }

            // Procedure
            if !test.procedure.is_empty() {
                println!();
                println!("{}", style("Procedure:").bold());
                for step in &test.procedure {
                    println!("  {}: {}", style(format!("Step {}", step.step)).cyan(), step.action.trim());
                    if let Some(ref expected) = step.expected {
                        println!("      {}: {}", style("Expected").dim(), expected.trim());
                    }
                }
            }

            // Acceptance Criteria
            if !test.acceptance_criteria.is_empty() {
                println!();
                println!("{}", style("Acceptance Criteria:").bold());
                for (i, criterion) in test.acceptance_criteria.iter().enumerate() {
                    if !criterion.is_empty() {
                        println!("  {}. {}", i + 1, criterion);
                    }
                }
            }

            // Links
            if args.with_links {
                println!();
                println!("{}", style("Links:").bold());
                if !test.links.verifies.is_empty() {
                    println!("  {}: {}", style("Verifies").dim(),
                        test.links.verifies.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(", "));
                }
                if !test.links.validates.is_empty() {
                    println!("  {}: {}", style("Validates").dim(),
                        test.links.validates.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(", "));
                }
                if !test.links.mitigates.is_empty() {
                    println!("  {}: {}", style("Mitigates").dim(),
                        test.links.mitigates.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(", "));
                }
            }

            println!();
            println!("{}", style("─".repeat(60)).dim());
            println!(
                "{}: {} | {}: {} | {}: {}",
                style("Author").dim(),
                test.author,
                style("Created").dim(),
                test.created.format("%Y-%m-%d %H:%M"),
                style("Revision").dim(),
                test.revision
            );
        }
    }

    Ok(())
}

fn run_edit(args: EditArgs) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let config = Config::load();

    // Find the test by ID prefix match
    let test = find_test(&project, &args.id)?;

    // Get the file path based on test type
    let test_type = match test.test_type {
        TestType::Verification => "verification",
        TestType::Validation => "validation",
    };
    let file_path = project
        .root()
        .join(format!("{}/protocols/{}.pdt.yaml", test_type, test.id));

    if !file_path.exists() {
        return Err(miette::miette!(
            "File not found: {}",
            file_path.display()
        ));
    }

    let editor = config.editor();
    println!(
        "Opening {} in {}...",
        style(format_short_id(&test.id)).cyan(),
        style(&editor).yellow()
    );

    std::process::Command::new(&editor)
        .arg(&file_path)
        .status()
        .into_diagnostic()?;

    Ok(())
}

/// Find a test by ID prefix match or short ID (@N)
fn find_test(project: &Project, id_query: &str) -> Result<Test> {
    // First, try to resolve short ID (@N) to full ID
    let short_ids = ShortIdIndex::load(project);
    let resolved_query = short_ids.resolve(id_query).unwrap_or_else(|| id_query.to_string());

    let mut matches: Vec<(Test, std::path::PathBuf)> = Vec::new();

    // Search both verification and validation directories
    for subdir in &["verification/protocols", "validation/protocols"] {
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
            if let Ok(test) = crate::yaml::parse_yaml_file::<Test>(entry.path()) {
                // Check if ID matches (prefix or full)
                let id_str = test.id.to_string();
                if id_str.starts_with(&resolved_query) || id_str == resolved_query {
                    matches.push((test, entry.path().to_path_buf()));
                }
                // Also check title for fuzzy match (only if not a short ID lookup)
                else if !id_query.starts_with('@') && !id_query.chars().all(|c| c.is_ascii_digit()) {
                    if test.title.to_lowercase().contains(&resolved_query.to_lowercase()) {
                        matches.push((test, entry.path().to_path_buf()));
                    }
                }
            }
        }
    }

    match matches.len() {
        0 => Err(miette::miette!(
            "No test found matching '{}'",
            id_query
        )),
        1 => Ok(matches.remove(0).0),
        _ => {
            println!(
                "{} Multiple matches found:",
                style("!").yellow()
            );
            for (test, _path) in &matches {
                println!(
                    "  {} - {}",
                    format_short_id(&test.id),
                    test.title
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
