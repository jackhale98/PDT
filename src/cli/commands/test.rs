//! `tdt test` command - Test protocol management (verification/validation)

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
use crate::core::entity::Priority;
use crate::core::identity::{EntityId, EntityPrefix};
use crate::core::project::Project;
use crate::core::shortid::ShortIdIndex;
use crate::core::CachedTest;
use crate::core::Config;
use crate::entities::result::{Result as TestResult, StepResult, StepResultRecord, Verdict};
use crate::entities::test::{Test, TestLevel, TestMethod, TestType};
use crate::schema::template::{TemplateContext, TemplateGenerator};
use crate::schema::wizard::SchemaWizard;

/// CLI-friendly test type enum
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CliTestType {
    Verification,
    Validation,
}

impl std::fmt::Display for CliTestType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CliTestType::Verification => write!(f, "verification"),
            CliTestType::Validation => write!(f, "validation"),
        }
    }
}

impl From<CliTestType> for TestType {
    fn from(cli: CliTestType) -> Self {
        match cli {
            CliTestType::Verification => TestType::Verification,
            CliTestType::Validation => TestType::Validation,
        }
    }
}

/// CLI-friendly test level enum
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CliTestLevel {
    Unit,
    Integration,
    System,
    Acceptance,
}

impl std::fmt::Display for CliTestLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CliTestLevel::Unit => write!(f, "unit"),
            CliTestLevel::Integration => write!(f, "integration"),
            CliTestLevel::System => write!(f, "system"),
            CliTestLevel::Acceptance => write!(f, "acceptance"),
        }
    }
}

impl From<CliTestLevel> for TestLevel {
    fn from(cli: CliTestLevel) -> Self {
        match cli {
            CliTestLevel::Unit => TestLevel::Unit,
            CliTestLevel::Integration => TestLevel::Integration,
            CliTestLevel::System => TestLevel::System,
            CliTestLevel::Acceptance => TestLevel::Acceptance,
        }
    }
}

/// CLI-friendly test method enum
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CliTestMethod {
    Inspection,
    Analysis,
    Demonstration,
    Test,
}

impl std::fmt::Display for CliTestMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CliTestMethod::Inspection => write!(f, "inspection"),
            CliTestMethod::Analysis => write!(f, "analysis"),
            CliTestMethod::Demonstration => write!(f, "demonstration"),
            CliTestMethod::Test => write!(f, "test"),
        }
    }
}

impl From<CliTestMethod> for TestMethod {
    fn from(cli: CliTestMethod) -> Self {
        match cli {
            CliTestMethod::Inspection => TestMethod::Inspection,
            CliTestMethod::Analysis => TestMethod::Analysis,
            CliTestMethod::Demonstration => TestMethod::Demonstration,
            CliTestMethod::Test => TestMethod::Test,
        }
    }
}

/// CLI-friendly priority enum
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CliPriority {
    Low,
    Medium,
    High,
    Critical,
}

impl std::fmt::Display for CliPriority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CliPriority::Low => write!(f, "low"),
            CliPriority::Medium => write!(f, "medium"),
            CliPriority::High => write!(f, "high"),
            CliPriority::Critical => write!(f, "critical"),
        }
    }
}

impl From<CliPriority> for Priority {
    fn from(cli: CliPriority) -> Self {
        match cli {
            CliPriority::Low => Priority::Low,
            CliPriority::Medium => Priority::Medium,
            CliPriority::High => Priority::High,
            CliPriority::Critical => Priority::Critical,
        }
    }
}

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

    /// Delete a test
    Delete(DeleteArgs),

    /// Archive a test (move to .tdt/archive/)
    Archive(ArchiveArgs),

    /// Execute a test and record a result
    Run(RunArgs),
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

/// Column definitions for test list output
const TEST_COLUMNS: &[ColumnDef] = &[
    ColumnDef::new("id", "ID", 17),
    ColumnDef::new("type", "TYPE", 12),
    ColumnDef::new("level", "LEVEL", 8),
    ColumnDef::new("method", "METHOD", 12),
    ColumnDef::new("title", "TITLE", 24),
    ColumnDef::new("status", "STATUS", 10),
    ColumnDef::new("priority", "PRIO", 8),
    ColumnDef::new("category", "CATEGORY", 12),
    ColumnDef::new("author", "AUTHOR", 16),
    ColumnDef::new("created", "CREATED", 16),
];

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

    /// Show tests with no results recorded
    #[arg(long)]
    pub no_results: bool,

    /// Show tests where most recent result failed
    #[arg(long)]
    pub last_failed: bool,

    /// Columns to display (comma-separated)
    #[arg(long, value_delimiter = ',', default_values_t = vec![
        ListColumn::Type,
        ListColumn::Level,
        ListColumn::Method,
        ListColumn::Title,
        ListColumn::Status,
        ListColumn::Priority,
    ])]
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

    /// Show the full ID column (hidden by default since SHORT is always shown)
    #[arg(long)]
    pub show_id: bool,
}

#[derive(clap::Args, Debug)]
pub struct NewArgs {
    /// Test type
    #[arg(long, short = 't', default_value = "verification")]
    pub r#type: CliTestType,

    /// Test level
    #[arg(long, short = 'l', default_value = "system")]
    pub level: CliTestLevel,

    /// Test method
    #[arg(long, short = 'm', default_value = "test")]
    pub method: CliTestMethod,

    /// Title (if not provided, uses placeholder)
    #[arg(long, short = 'T')]
    pub title: Option<String>,

    /// Category
    #[arg(long, short = 'c')]
    pub category: Option<String>,

    /// Priority
    #[arg(long, short = 'p', default_value = "medium")]
    pub priority: CliPriority,

    /// Requirements this test verifies (comma-separated IDs, e.g., REQ@1,REQ@2)
    #[arg(long, short = 'R', value_delimiter = ',')]
    pub verifies: Vec<String>,

    /// Risks this test mitigates (comma-separated IDs, e.g., RISK@1,RISK@2)
    #[arg(long, short = 'M', value_delimiter = ',')]
    pub mitigates: Vec<String>,

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

#[derive(clap::Args, Debug)]
pub struct DeleteArgs {
    /// Test ID or short ID (TEST@N)
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
    /// Test ID or short ID (TEST@N)
    pub id: String,

    /// Force archive even if other entities reference this one
    #[arg(long)]
    pub force: bool,

    /// Suppress output
    #[arg(long, short = 'q')]
    pub quiet: bool,
}

/// Directories where tests are stored
const TEST_DIRS: &[&str] = &["verification/protocols", "validation/protocols"];

/// Verdict options for test execution
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CliVerdict {
    Pass,
    Fail,
    Conditional,
    Incomplete,
}

#[derive(clap::Args, Debug)]
pub struct RunArgs {
    /// Test ID or short ID (TEST@N)
    pub test: String,

    /// Test verdict
    #[arg(long)]
    pub verdict: Option<CliVerdict>,

    /// Executed by (default: from config)
    #[arg(long)]
    pub by: Option<String>,

    /// Open editor for full result details
    #[arg(long, short = 'e')]
    pub edit: bool,

    /// Skip editor (create minimal result)
    #[arg(long)]
    pub no_edit: bool,

    /// Notes or observations
    #[arg(long)]
    pub notes: Option<String>,
}

pub fn run(cmd: TestCommands, global: &GlobalOpts) -> Result<()> {
    match cmd {
        TestCommands::List(args) => run_list(args, global),
        TestCommands::New(args) => run_new(args, global),
        TestCommands::Show(args) => run_show(args, global),
        TestCommands::Edit(args) => run_edit(args),
        TestCommands::Delete(args) => run_delete_cmd(args),
        TestCommands::Archive(args) => run_archive_cmd(args),
        TestCommands::Run(args) => run_run(args, global),
    }
}

fn run_delete_cmd(args: DeleteArgs) -> Result<()> {
    crate::cli::commands::utils::run_delete(&args.id, TEST_DIRS, args.force, false, args.quiet)
}

fn run_archive_cmd(args: ArchiveArgs) -> Result<()> {
    crate::cli::commands::utils::run_delete(&args.id, TEST_DIRS, args.force, true, args.quiet)
}

fn run_list(args: ListArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let short_ids = ShortIdIndex::load(&project);

    // Determine output format
    let format = match global.output {
        OutputFormat::Auto => OutputFormat::Tsv,
        f => f,
    };

    // Check if we can use the fast cache path:
    // - No complex filters that require full YAML (orphans, tag, no_results, last_failed)
    // - Not JSON/YAML output (which needs full entity serialization)
    let can_use_cache = !args.orphans
        && args.tag.is_none()
        && !args.no_results
        && !args.last_failed
        && args.recent.is_none()
        && !matches!(format, OutputFormat::Json | OutputFormat::Yaml);

    if can_use_cache {
        if let Ok(cache) = EntityCache::open(&project) {
            // Convert filter enums to strings for cache query
            let status_filter = match args.status {
                StatusFilter::Draft => Some("draft"),
                StatusFilter::Review => Some("review"),
                StatusFilter::Approved => Some("approved"),
                StatusFilter::Released => Some("released"),
                StatusFilter::Obsolete => Some("obsolete"),
                StatusFilter::Active | StatusFilter::All => None,
            };
            let type_filter = match args.r#type {
                TestTypeFilter::Verification => Some("verification"),
                TestTypeFilter::Validation => Some("validation"),
                TestTypeFilter::All => None,
            };
            let level_filter = match args.level {
                TestLevelFilter::Unit => Some("unit"),
                TestLevelFilter::Integration => Some("integration"),
                TestLevelFilter::System => Some("system"),
                TestLevelFilter::Acceptance => Some("acceptance"),
                TestLevelFilter::All => None,
            };
            let method_filter = match args.method {
                TestMethodFilter::Inspection => Some("inspection"),
                TestMethodFilter::Analysis => Some("analysis"),
                TestMethodFilter::Demonstration => Some("demonstration"),
                TestMethodFilter::Test => Some("test"),
                TestMethodFilter::All => None,
            };

            let mut tests = cache.list_tests(
                status_filter,
                type_filter,
                level_filter,
                method_filter,
                args.priority.as_deref(),
                args.category.as_deref(),
                args.author.as_deref(),
                args.search.as_deref(),
                None, // We'll apply limit after sorting
            );

            // Handle 'active' status filter (exclude obsolete)
            if matches!(args.status, StatusFilter::Active) {
                tests.retain(|t| t.status != crate::core::entity::Status::Obsolete);
            }

            // Sort
            match args.sort {
                ListColumn::Id => tests.sort_by(|a, b| a.id.cmp(&b.id)),
                ListColumn::Type => tests.sort_by(|a, b| {
                    a.test_type
                        .as_deref()
                        .unwrap_or("")
                        .cmp(b.test_type.as_deref().unwrap_or(""))
                }),
                ListColumn::Level => tests.sort_by(|a, b| {
                    let level_order = |l: Option<&str>| match l {
                        Some("unit") => 0,
                        Some("integration") => 1,
                        Some("system") => 2,
                        Some("acceptance") => 3,
                        _ => 4,
                    };
                    level_order(a.level.as_deref()).cmp(&level_order(b.level.as_deref()))
                }),
                ListColumn::Method => tests.sort_by(|a, b| {
                    a.method
                        .as_deref()
                        .unwrap_or("")
                        .cmp(b.method.as_deref().unwrap_or(""))
                }),
                ListColumn::Title => tests.sort_by(|a, b| a.title.cmp(&b.title)),
                ListColumn::Status => tests.sort_by(|a, b| a.status.cmp(&b.status)),
                ListColumn::Priority => tests.sort_by(|a, b| {
                    use crate::core::entity::Priority;
                    let priority_order = |p: Option<Priority>| match p {
                        Some(Priority::Critical) => 0,
                        Some(Priority::High) => 1,
                        Some(Priority::Medium) => 2,
                        Some(Priority::Low) => 3,
                        None => 4,
                    };
                    priority_order(a.priority).cmp(&priority_order(b.priority))
                }),
                ListColumn::Category => tests.sort_by(|a, b| {
                    a.category
                        .as_deref()
                        .unwrap_or("")
                        .cmp(b.category.as_deref().unwrap_or(""))
                }),
                ListColumn::Author => tests.sort_by(|a, b| a.author.cmp(&b.author)),
                ListColumn::Created => tests.sort_by(|a, b| a.created.cmp(&b.created)),
            }

            if args.reverse {
                tests.reverse();
            }

            if let Some(limit) = args.limit {
                tests.truncate(limit);
            }

            return output_cached_tests(&tests, &short_ids, &args, format);
        }
    }

    // Fall back to full YAML loading for complex filters or JSON/YAML output
    // Pre-load results if needed for result-based filters
    let results: Vec<crate::entities::result::Result> = if args.no_results || args.last_failed {
        load_all_results(&project)
    } else {
        Vec::new()
    };

    // Pre-compute HashSets for O(1) lookups instead of O(n) iterations per test
    use std::collections::{HashMap, HashSet};
    let tests_with_results: HashSet<&crate::core::identity::EntityId> =
        results.iter().map(|r| &r.test_id).collect();

    // For last_failed: find most recent result per test and check if it's Fail
    let last_failed_tests: HashSet<&crate::core::identity::EntityId> = {
        // Group results by test_id, keeping only the most recent
        let mut latest_by_test: HashMap<
            &crate::core::identity::EntityId,
            &crate::entities::result::Result,
        > = HashMap::new();
        for r in &results {
            latest_by_test
                .entry(&r.test_id)
                .and_modify(|existing| {
                    if r.executed_date > existing.executed_date {
                        *existing = r;
                    }
                })
                .or_insert(r);
        }
        // Collect test IDs where latest result is Fail
        latest_by_test
            .into_iter()
            .filter(|(_, r)| r.verdict == crate::entities::result::Verdict::Fail)
            .map(|(id, _)| id)
            .collect()
    };

    // Collect all test files from both verification and validation directories
    let mut tests: Vec<Test> = Vec::new();

    // Check verification protocols
    let verification_dir = project.root().join("verification/protocols");
    if verification_dir.exists() {
        for entry in walkdir::WalkDir::new(&verification_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
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
            .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
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
        let priority_match = args
            .priority
            .as_ref()
            .is_none_or(|p| t.priority.to_string().to_lowercase() == p.to_lowercase());

        // Category filter (case-insensitive)
        let category_match = args.category.as_ref().is_none_or(|cat| {
            t.category
                .as_ref()
                .is_some_and(|c| c.to_lowercase() == cat.to_lowercase())
        });

        // Tag filter (case-insensitive)
        let tag_match = args.tag.as_ref().is_none_or(|tag| {
            t.tags
                .iter()
                .any(|tg| tg.to_lowercase() == tag.to_lowercase())
        });

        // Author filter
        let author_match = args
            .author
            .as_ref()
            .is_none_or(|author| t.author.to_lowercase().contains(&author.to_lowercase()));

        // Search filter
        let search_match = args.search.as_ref().is_none_or(|search| {
            let search_lower = search.to_lowercase();
            t.title.to_lowercase().contains(&search_lower)
                || t.objective.to_lowercase().contains(&search_lower)
        });

        // Orphans filter (no linked requirements)
        let orphans_match =
            !args.orphans || (t.links.verifies.is_empty() && t.links.validates.is_empty());

        // Recent filter (created in last N days)
        let recent_match = args.recent.is_none_or(|days| {
            let cutoff = chrono::Utc::now() - chrono::Duration::days(days as i64);
            t.created >= cutoff
        });

        // No results filter - tests with no results recorded (O(1) lookup)
        let no_results_match = if args.no_results {
            !tests_with_results.contains(&t.id)
        } else {
            true
        };

        // Last failed filter - tests where most recent result is fail (O(1) lookup)
        let last_failed_match = if args.last_failed {
            last_failed_tests.contains(&t.id)
        } else {
            true
        };

        type_match
            && level_match
            && method_match
            && status_match
            && priority_match
            && category_match
            && tag_match
            && author_match
            && search_match
            && orphans_match
            && recent_match
            && no_results_match
            && last_failed_match
    });

    if tests.is_empty() {
        match global.output {
            OutputFormat::Json => println!("[]"),
            OutputFormat::Yaml => println!("[]"),
            _ => {
                println!("No tests found.");
                println!();
                println!("Create one with: {}", style("tdt test new").yellow());
            }
        }
        return Ok(());
    }

    // Sort by specified column
    match args.sort {
        ListColumn::Id => tests.sort_by(|a, b| a.id.to_string().cmp(&b.id.to_string())),
        ListColumn::Type => {
            tests.sort_by(|a, b| a.test_type.to_string().cmp(&b.test_type.to_string()))
        }
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
            a.category
                .as_deref()
                .unwrap_or("")
                .cmp(b.category.as_deref().unwrap_or(""))
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

    // Update short ID index with current tests (preserves other entity types)
    let mut short_ids = short_ids;
    short_ids.ensure_all(tests.iter().map(|t| t.id.to_string()));
    super::utils::save_short_ids(&mut short_ids, &project);

    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&tests).into_diagnostic()?;
            println!("{}", json);
        }
        OutputFormat::Yaml => {
            let yaml = serde_yml::to_string(&tests).into_diagnostic()?;
            print!("{}", yaml);
        }
        OutputFormat::Csv | OutputFormat::Tsv | OutputFormat::Md | OutputFormat::Table | OutputFormat::Dot | OutputFormat::Tree => {
            // Build column list from args
            let mut columns: Vec<&str> = args
                .columns
                .iter()
                .map(|c| c.to_string().leak() as &str)
                .collect();

            // Add Id column if --show-id flag is set
            if args.show_id && !columns.contains(&"id") {
                columns.insert(0, "id");
            }

            // Build rows
            let rows: Vec<TableRow> = tests.iter().map(|t| test_to_row(t, &short_ids)).collect();

            let config = TableConfig {
                wrap_width: args.wrap,
                show_summary: true,
            };
            let formatter = TableFormatter::new(TEST_COLUMNS, "test", "TEST").with_config(config);
            formatter.output(rows, format, &columns);
        }
        OutputFormat::Id | OutputFormat::ShortId => {
            for test in &tests {
                if format == OutputFormat::ShortId {
                    let short_id = short_ids
                        .get_short_id(&test.id.to_string())
                        .unwrap_or_default();
                    println!("{}", short_id);
                } else {
                    println!("{}", test.id);
                }
            }
        }
        OutputFormat::Auto | OutputFormat::Path => unreachable!(),
    }

    Ok(())
}

/// Output cached tests (fast path - no YAML parsing needed)
fn output_cached_tests(
    tests: &[CachedTest],
    short_ids: &ShortIdIndex,
    args: &ListArgs,
    format: OutputFormat,
) -> Result<()> {
    if tests.is_empty() {
        println!("No tests found.");
        println!();
        println!("Create one with: {}", style("tdt test new").yellow());
        return Ok(());
    }

    if args.count {
        println!("{}", tests.len());
        return Ok(());
    }

    match format {
        OutputFormat::Csv | OutputFormat::Tsv | OutputFormat::Md | OutputFormat::Table | OutputFormat::Dot | OutputFormat::Tree => {
            // Build column list from args
            let mut columns: Vec<&str> = args
                .columns
                .iter()
                .map(|c| c.to_string().leak() as &str)
                .collect();

            // Add Id column if --show-id flag is set
            if args.show_id && !columns.contains(&"id") {
                columns.insert(0, "id");
            }

            // Build rows
            let rows: Vec<TableRow> = tests
                .iter()
                .map(|t| cached_test_to_row(t, short_ids))
                .collect();

            let config = TableConfig {
                wrap_width: args.wrap,
                show_summary: true,
            };
            let formatter = TableFormatter::new(TEST_COLUMNS, "test", "TEST").with_config(config);
            formatter.output(rows, format, &columns);
        }
        OutputFormat::Id | OutputFormat::ShortId => {
            for test in tests {
                if format == OutputFormat::ShortId {
                    let short_id = short_ids.get_short_id(&test.id).unwrap_or_default();
                    println!("{}", short_id);
                } else {
                    println!("{}", test.id);
                }
            }
        }
        OutputFormat::Json | OutputFormat::Yaml | OutputFormat::Auto | OutputFormat::Path => {
            // Should never reach here - JSON/YAML use full YAML path
            unreachable!()
        }
    }

    Ok(())
}

/// Convert a Test to a TableRow
fn test_to_row(test: &Test, short_ids: &ShortIdIndex) -> TableRow {
    TableRow::new(test.id.to_string(), short_ids)
        .cell("id", CellValue::Id(test.id.to_string()))
        .cell("type", CellValue::Type(test.test_type.to_string()))
        .cell(
            "level",
            CellValue::Text(test.test_level.map_or("-".to_string(), |l| l.to_string())),
        )
        .cell(
            "method",
            CellValue::Text(test.test_method.map_or("-".to_string(), |m| m.to_string())),
        )
        .cell("title", CellValue::Text(test.title.clone()))
        .cell("status", CellValue::Status(test.status))
        .cell("priority", CellValue::Priority(test.priority))
        .cell(
            "category",
            CellValue::Text(test.category.as_deref().unwrap_or("-").to_string()),
        )
        .cell("author", CellValue::Text(test.author.clone()))
        .cell("created", CellValue::Date(test.created))
}

/// Convert a CachedTest to a TableRow
fn cached_test_to_row(test: &CachedTest, short_ids: &ShortIdIndex) -> TableRow {
    TableRow::new(test.id.clone(), short_ids)
        .cell("id", CellValue::Id(test.id.clone()))
        .cell(
            "type",
            CellValue::Type(test.test_type.as_deref().unwrap_or("-").to_string()),
        )
        .cell(
            "level",
            CellValue::Text(test.level.as_deref().unwrap_or("-").to_string()),
        )
        .cell(
            "method",
            CellValue::Text(test.method.as_deref().unwrap_or("-").to_string()),
        )
        .cell("title", CellValue::Text(test.title.clone()))
        .cell("status", CellValue::Status(test.status))
        .cell("priority", CellValue::OptionalPriority(test.priority))
        .cell(
            "category",
            CellValue::Text(test.category.as_deref().unwrap_or("-").to_string()),
        )
        .cell("author", CellValue::Text(test.author.clone()))
        .cell("created", CellValue::Date(test.created))
}

fn run_new(args: NewArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let config = Config::load();

    // Determine values - either from schema-driven wizard or args
    let (test_type, test_level, test_method, title, category, priority, objective, description) =
        if args.interactive {
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

            // Extract text fields
            let objective = result.get_string("objective").map(String::from);
            let description = result.get_string("description").map(String::from);

            (
                test_type,
                test_level,
                test_method,
                title,
                category,
                priority,
                objective,
                description,
            )
        } else {
            // Default mode - use args with defaults
            let test_type: TestType = args.r#type.into();
            let test_level: TestLevel = args.level.into();
            let test_method: TestMethod = args.method.into();
            let title = args
                .title
                .unwrap_or_else(|| "New Test Protocol".to_string());
            let category = args.category.unwrap_or_default();
            let priority = args.priority.to_string();

            (
                test_type,
                test_level,
                test_method,
                title,
                category,
                priority,
                None,
                None,
            )
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

    let mut yaml_content = generator
        .generate_test(&ctx)
        .map_err(|e| miette::miette!("{}", e))?;

    // Apply wizard text values via string replacement (for interactive mode)
    if args.interactive {
        if let Some(ref obj) = objective {
            if !obj.is_empty() {
                let indented = obj
                    .lines()
                    .map(|line| format!("  {}", line))
                    .collect::<Vec<_>>()
                    .join("\n");
                yaml_content = yaml_content.replace(
                    "objective: |\n  # What does this test verify or validate?\n  # Be specific about success criteria",
                    &format!("objective: |\n{}", indented),
                );
            }
        }
        if let Some(ref desc) = description {
            if !desc.is_empty() {
                let indented = desc
                    .lines()
                    .map(|line| format!("  {}", line))
                    .collect::<Vec<_>>()
                    .join("\n");
                yaml_content = yaml_content.replace(
                    "description: |\n  # Detailed description of the test\n  # Include any background or context",
                    &format!("description: |\n{}", indented),
                );
            }
        }
    }

    // Determine output directory based on type
    let output_dir = project.test_directory(&test_type.to_string());

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

    // Handle --verifies and --mitigates flags by updating the file with links
    if !args.verifies.is_empty() || !args.mitigates.is_empty() {
        // Parse the test we just created
        let mut test: Test = crate::yaml::parse_yaml_file(&file_path)
            .map_err(|e| miette::miette!("Failed to parse created test: {}", e))?;

        // Resolve short IDs and add verifies links
        for req_ref in &args.verifies {
            let full_id = if req_ref.contains('@') {
                // It's a short ID like REQ@1
                short_ids
                    .resolve(req_ref)
                    .ok_or_else(|| miette::miette!("Unknown short ID: {}", req_ref))?
            } else {
                // It's already a full ID
                req_ref.clone()
            };
            let entity_id = EntityId::parse(&full_id)
                .map_err(|_| miette::miette!("Invalid entity ID: {}", full_id))?;
            if !test.links.verifies.contains(&entity_id) {
                test.links.verifies.push(entity_id);
            }
        }

        // Resolve short IDs and add mitigates links
        for risk_ref in &args.mitigates {
            let full_id = if risk_ref.contains('@') {
                short_ids
                    .resolve(risk_ref)
                    .ok_or_else(|| miette::miette!("Unknown short ID: {}", risk_ref))?
            } else {
                risk_ref.clone()
            };
            let entity_id = EntityId::parse(&full_id)
                .map_err(|_| miette::miette!("Invalid entity ID: {}", full_id))?;
            if !test.links.mitigates.contains(&entity_id) {
                test.links.mitigates.push(entity_id);
            }
        }

        // Write back the updated test
        let updated_yaml = serde_yml::to_string(&test).into_diagnostic()?;
        fs::write(&file_path, &updated_yaml).into_diagnostic()?;
    }

    // Handle --link flags
    let added_links = crate::cli::entity_cmd::process_link_flags(
        &file_path,
        EntityPrefix::Test,
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
                "{} Created test {}",
                style("✓").green(),
                style(short_id.clone().unwrap_or_else(|| format_short_id(&id))).cyan()
            );
            println!("   {}", style(file_path.display()).dim());
            println!(
                "   Type: {} | Level: {} | Method: {}",
                style(test_type.to_string()).yellow(),
                style(test_level.to_string()).yellow(),
                style(test_method.to_string()).yellow()
            );

            // Show linked entities if any
            if !args.verifies.is_empty() {
                println!("   Verifies: {}", style(args.verifies.join(", ")).cyan());
            }
            if !args.mitigates.is_empty() {
                println!("   Mitigates: {}", style(args.mitigates.join(", ")).cyan());
            }

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

    // Find the test by ID prefix match
    let test = find_test(&project, &args.id)?;

    // Output based on format (pretty is default)
    match global.output {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&test).into_diagnostic()?;
            println!("{}", json);
        }
        OutputFormat::Yaml => {
            let yaml = serde_yml::to_string(&test).into_diagnostic()?;
            print!("{}", yaml);
        }
        OutputFormat::Id | OutputFormat::ShortId => {
            if global.output == OutputFormat::ShortId {
                let short_ids = ShortIdIndex::load(&project);
                let short_id = short_ids
                    .get_short_id(&test.id.to_string())
                    .unwrap_or_default();
                println!("{}", short_id);
            } else {
                println!("{}", test.id);
            }
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
                style(&test.id.to_string()).cyan()
            );
            println!("{}: {}", style("Type").bold(), test.test_type);
            if let Some(level) = &test.test_level {
                println!("{}: {}", style("Level").bold(), level);
            }
            if let Some(method) = &test.test_method {
                println!("{}: {}", style("Method").bold(), method);
            }
            println!("{}: {}", style("Title").bold(), style(&test.title).yellow());
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
                    println!(
                        "  {}: {}",
                        style(format!("Step {}", step.step)).cyan(),
                        step.action.trim()
                    );
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
                    println!(
                        "  {}: {}",
                        style("Verifies").dim(),
                        test.links
                            .verifies
                            .iter()
                            .map(|id| format_link_with_title(&id.to_string(), &short_ids, &cache))
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                }
                if !test.links.validates.is_empty() {
                    println!(
                        "  {}: {}",
                        style("Validates").dim(),
                        test.links
                            .validates
                            .iter()
                            .map(|id| format_link_with_title(&id.to_string(), &short_ids, &cache))
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                }
                if !test.links.mitigates.is_empty() {
                    println!(
                        "  {}: {}",
                        style("Mitigates").dim(),
                        test.links
                            .mitigates
                            .iter()
                            .map(|id| format_link_with_title(&id.to_string(), &short_ids, &cache))
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
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
        .join(format!("{}/protocols/{}.tdt.yaml", test_type, test.id));

    if !file_path.exists() {
        return Err(miette::miette!("File not found: {}", file_path.display()));
    }

    println!(
        "Opening {} in {}...",
        style(format_short_id(&test.id)).cyan(),
        style(config.editor()).yellow()
    );

    config.run_editor(&file_path).into_diagnostic()?;

    Ok(())
}

/// Find a test by ID prefix match or short ID (@N)
fn find_test(project: &Project, id_query: &str) -> Result<Test> {
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
            if entity.prefix == "TEST" {
                if let Ok(test) = crate::yaml::parse_yaml_file::<Test>(&entity.file_path) {
                    return Ok(test);
                }
            }
        }

        // Try prefix match via cache
        if lookup_id.starts_with("TEST-") {
            let filter = crate::core::EntityFilter {
                prefix: Some(crate::core::EntityPrefix::Test),
                search: Some(lookup_id.to_string()),
                ..Default::default()
            };
            let matches: Vec<_> = cache.list_entities(&filter);
            if matches.len() == 1 {
                if let Ok(test) = crate::yaml::parse_yaml_file::<Test>(&matches[0].file_path) {
                    return Ok(test);
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
            .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        {
            if let Ok(test) = crate::yaml::parse_yaml_file::<Test>(entry.path()) {
                // Check if ID matches (prefix or full) or title fuzzy matches
                let id_str = test.id.to_string();
                let id_matches = id_str.starts_with(&resolved_query) || id_str == resolved_query;
                let title_matches = !id_query.starts_with('@')
                    && !id_query.chars().all(|c| c.is_ascii_digit())
                    && test
                        .title
                        .to_lowercase()
                        .contains(&resolved_query.to_lowercase());

                if id_matches || title_matches {
                    matches.push((test, entry.path().to_path_buf()));
                }
            }
        }
    }

    match matches.len() {
        0 => Err(miette::miette!("No test found matching '{}'", id_query)),
        1 => Ok(matches.remove(0).0),
        _ => {
            println!("{} Multiple matches found:", style("!").yellow());
            for (test, _path) in &matches {
                println!("  {} - {}", format_short_id(&test.id), test.title);
            }
            Err(miette::miette!(
                "Ambiguous query '{}'. Please be more specific.",
                id_query
            ))
        }
    }
}

/// Load all test results from the project
fn load_all_results(project: &Project) -> Vec<crate::entities::result::Result> {
    let mut results = Vec::new();

    // Load from verification/results
    let ver_dir = project.root().join("verification/results");
    if ver_dir.exists() {
        for entry in walkdir::WalkDir::new(&ver_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        {
            if let Ok(result) =
                crate::yaml::parse_yaml_file::<crate::entities::result::Result>(entry.path())
            {
                results.push(result);
            }
        }
    }

    // Load from validation/results
    let val_dir = project.root().join("validation/results");
    if val_dir.exists() {
        for entry in walkdir::WalkDir::new(&val_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        {
            if let Ok(result) =
                crate::yaml::parse_yaml_file::<crate::entities::result::Result>(entry.path())
            {
                results.push(result);
            }
        }
    }

    results
}

fn run_run(args: RunArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let config = Config::load();

    // Resolve test ID
    let short_ids = ShortIdIndex::load(&project);
    let resolved_test_id = short_ids
        .resolve(&args.test)
        .unwrap_or_else(|| args.test.clone());

    // Find and load the test protocol
    let ver_dir = project.root().join("verification/protocols");
    let val_dir = project.root().join("validation/protocols");
    let mut test: Option<Test> = None;
    let mut test_type_str = "verification";

    // Search verification protocols
    if ver_dir.exists() {
        for entry in fs::read_dir(&ver_dir).into_diagnostic()?.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "yaml") {
                let filename = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                if filename.contains(&resolved_test_id) {
                    let content = fs::read_to_string(&path).into_diagnostic()?;
                    if let Ok(t) = serde_yml::from_str::<Test>(&content) {
                        test = Some(t);
                        test_type_str = "verification";
                        break;
                    }
                }
            }
        }
    }

    // Search validation protocols if not found
    if test.is_none() && val_dir.exists() {
        for entry in fs::read_dir(&val_dir).into_diagnostic()?.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "yaml") {
                let filename = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                if filename.contains(&resolved_test_id) {
                    let content = fs::read_to_string(&path).into_diagnostic()?;
                    if let Ok(t) = serde_yml::from_str::<Test>(&content) {
                        test = Some(t);
                        test_type_str = "validation";
                        break;
                    }
                }
            }
        }
    }

    let test = test.ok_or_else(|| miette::miette!("No test found matching '{}'", args.test))?;

    // Get display ID
    let test_short_id = short_ids
        .get_short_id(&test.id.to_string())
        .unwrap_or_else(|| format_short_id(&test.id));

    // Determine verdict - prompt if not provided
    let verdict = match args.verdict {
        Some(CliVerdict::Pass) => Verdict::Pass,
        Some(CliVerdict::Fail) => Verdict::Fail,
        Some(CliVerdict::Conditional) => Verdict::Conditional,
        Some(CliVerdict::Incomplete) => Verdict::Incomplete,
        None => {
            // Prompt for verdict interactively
            use dialoguer::{theme::ColorfulTheme, Select};
            let items = &["Pass", "Fail", "Conditional", "Incomplete"];
            let selection = Select::with_theme(&ColorfulTheme::default())
                .with_prompt("Test verdict")
                .items(items)
                .default(0)
                .interact()
                .into_diagnostic()?;
            match selection {
                0 => Verdict::Pass,
                1 => Verdict::Fail,
                2 => Verdict::Conditional,
                _ => Verdict::Incomplete,
            }
        }
    };

    // Determine executor
    let executed_by = args.by.unwrap_or_else(|| config.author().to_string());

    // Create result ID
    let result_id = EntityId::new(EntityPrefix::Rslt);

    // Scaffold step results from test procedure
    let step_results: Vec<StepResultRecord> = test
        .procedure
        .iter()
        .map(|step| {
            // Default step result based on overall verdict
            let step_result = match verdict {
                Verdict::Pass => StepResult::Pass,
                Verdict::Fail => StepResult::Pass, // User will mark specific failures
                Verdict::Conditional => StepResult::Pass,
                Verdict::Incomplete => StepResult::Skip,
                Verdict::NotApplicable => StepResult::NotApplicable,
            };

            StepResultRecord {
                step: step.step,
                result: step_result,
                observed: None, // To be filled in by user
                measurement: None,
                notes: None,
            }
        })
        .collect();

    // Create result entity
    let result = TestResult {
        id: result_id.clone(),
        test_id: test.id.clone(),
        test_revision: Some(test.revision),
        title: Some(format!("Result for {}", test.title)),
        verdict,
        verdict_rationale: None,
        category: test.category.clone(),
        tags: Vec::new(),
        executed_date: chrono::Utc::now(),
        executed_by: executed_by.clone(),
        reviewed_by: None,
        reviewed_date: None,
        sample_info: None,
        environment: None,
        equipment_used: Vec::new(),
        step_results,
        deviations: Vec::new(),
        failures: Vec::new(),
        attachments: Vec::new(),
        duration: None,
        notes: args.notes.clone(),
        status: crate::core::entity::Status::default(),
        links: Default::default(),
        created: chrono::Utc::now(),
        author: executed_by.clone(),
        revision: 1,
    };

    // Serialize to YAML
    let yaml_content = serde_yml::to_string(&result).into_diagnostic()?;

    // Determine output directory based on test type
    let output_dir = project.root().join(format!("{}/results", test_type_str));
    if !output_dir.exists() {
        fs::create_dir_all(&output_dir).into_diagnostic()?;
    }

    let file_path = output_dir.join(format!("{}.tdt.yaml", result_id));
    fs::write(&file_path, &yaml_content).into_diagnostic()?;

    // Add to short ID index
    let mut short_ids = ShortIdIndex::load(&project);
    let result_short_id = short_ids.add(result_id.to_string());
    super::utils::save_short_ids(&mut short_ids, &project);

    // Output based on format
    match global.output {
        OutputFormat::Json => {
            let output = serde_json::json!({
                "id": result_id.to_string(),
                "short_id": result_short_id,
                "test_id": test.id.to_string(),
                "test_short_id": test_short_id,
                "verdict": verdict.to_string(),
                "executed_by": executed_by,
                "file": file_path.display().to_string(),
            });
            println!(
                "{}",
                serde_json::to_string_pretty(&output).unwrap_or_default()
            );
        }
        OutputFormat::Yaml => {
            let output = serde_json::json!({
                "id": result_id.to_string(),
                "test_id": test.id.to_string(),
                "verdict": verdict.to_string(),
            });
            println!("{}", serde_yml::to_string(&output).unwrap_or_default());
        }
        OutputFormat::Id | OutputFormat::ShortId => {
            if global.output == OutputFormat::ShortId {
                let result_short = result_short_id.unwrap_or_else(|| format_short_id(&result_id));
                println!("{}", result_short);
            } else {
                println!("{}", result_id);
            }
        }
        _ => {
            println!(
                "{} Created result {} for test {} \"{}\"",
                style("✓").green(),
                style(result_short_id.unwrap_or_else(|| format_short_id(&result_id))).cyan(),
                style(&test_short_id).cyan(),
                truncate_str(&test.title, 35)
            );
            println!(
                "   Verdict: {}",
                match verdict {
                    Verdict::Pass => style("pass").green(),
                    Verdict::Fail => style("fail").red(),
                    Verdict::Conditional => style("conditional").yellow(),
                    Verdict::Incomplete => style("incomplete").yellow(),
                    Verdict::NotApplicable => style("n/a").dim(),
                }
            );
            println!("   Executed by: {}", executed_by);
            if !test.procedure.is_empty() {
                println!(
                    "   Steps scaffolded: {}",
                    style(test.procedure.len()).cyan()
                );
            }
            println!("   {}", style(file_path.display()).dim());
        }
    }

    // Open in editor if requested
    if args.edit && !args.no_edit {
        println!();
        println!("Opening in {}...", style(config.editor()).yellow());
        config.run_editor(&file_path).into_diagnostic()?;
    }

    Ok(())
}
