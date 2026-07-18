//! `tdt test` command - Test protocol management (verification/validation)

use clap::{Subcommand, ValueEnum};
use console::style;
use miette::{IntoDiagnostic, Result};
use std::fs;

use crate::cli::commands::utils::format_link_with_title;
use crate::cli::filters::StatusFilter;
use crate::cli::helpers::{format_short_id, truncate_str};
use crate::cli::table::{CellValue, ColumnDef, TableRow};
use crate::cli::{GlobalOpts, OutputFormat};
use tdt_core::core::cache::EntityCache;
use tdt_core::core::entity::{Priority, Status};
use tdt_core::core::identity::{EntityId, EntityPrefix};
use tdt_core::core::project::Project;
use tdt_core::core::shortid::ShortIdIndex;
use tdt_core::core::Config;
use tdt_core::core::{CachedResult, CachedTest};
use tdt_core::entities::result::{Result as TestResult, StepResult, StepResultRecord, Verdict};
use tdt_core::entities::test::{Test, TestLevel, TestMethod, TestType};
use tdt_core::schema::wizard::SchemaWizard;
use tdt_core::services::{
    CommonFilter, CreateTest, SortDirection, TestFilter, TestService, TestSortField,
};

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

const TEST_LIST_SPEC: crate::cli::entity_cmd::CachedListSpec =
    crate::cli::entity_cmd::CachedListSpec {
        columns: TEST_COLUMNS,
        entity_name: "test",
        entity_prefix: "TEST",
        empty_message: "No tests found.",
    };

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

    /// Only show entities linked to these IDs (use - for stdin pipe)
    #[arg(long, value_delimiter = ',')]
    pub linked_to: Vec<String>,

    /// Filter by link type when using --linked-to (e.g., verified_by, satisfied_by)
    #[arg(long, requires = "linked_to")]
    pub via: Option<String>,
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

    /// Test objective - what this test aims to verify or validate
    #[arg(long, short = 'O')]
    pub objective: Option<String>,

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

/// Search directories for this entity type — single source of truth
/// lives in `Project::entity_search_directories`.
const TEST_DIRS: &[&str] = tdt_core::core::project::Project::entity_search_directories(
    tdt_core::core::identity::EntityPrefix::Test,
);

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
    let cache = EntityCache::open(&project).map_err(|e| miette::miette!("{}", e))?;
    let service = TestService::new(&project, &cache);

    // Resolve linked-to filter via cache
    let short_ids = ShortIdIndex::load(&project);
    let allowed_ids = crate::cli::helpers::resolve_linked_to(
        &args.linked_to,
        args.via.as_deref(),
        &short_ids,
        &cache,
    );

    // Build filter and sort from CLI args
    let filter = build_test_filter(&args);
    let (sort_field, sort_dir) = build_test_sort(&args);

    // Determine output format
    let format = global.list_format();

    // Determine if we need full entities
    let needs_full_output = matches!(format, OutputFormat::Json | OutputFormat::Yaml);
    let needs_result_filters = args.no_results || args.last_failed;
    let needs_full_entities = needs_full_output || needs_result_filters || args.tag.is_some();

    if needs_full_entities {
        // Load full entities via service
        let result = service
            .list(&filter, sort_field, sort_dir)
            .map_err(|e| miette::miette!("{}", e))?;

        let mut tests = result.items;

        // Apply linked-to filter
        if let Some(ref ids) = allowed_ids {
            tests.retain(|e| ids.contains(&e.id.to_string()));
        }

        // Apply result-based filters (require cross-entity queries)
        // Use cache for fast lookups instead of walking directories
        if needs_result_filters {
            let cached_results = cache.list_results(None, None, None, None, None, None);

            use std::collections::{HashMap, HashSet};
            let tests_with_results: HashSet<String> = cached_results
                .iter()
                .filter_map(|r| r.test_id.clone())
                .collect();

            let last_failed_tests: HashSet<String> = {
                // Find latest result per test, then filter to failed ones
                let mut latest_by_test: HashMap<&str, &CachedResult> = HashMap::new();
                for r in &cached_results {
                    if let Some(ref test_id) = r.test_id {
                        latest_by_test
                            .entry(test_id.as_str())
                            .and_modify(|existing| {
                                // Compare executed_date strings (ISO format sorts correctly)
                                if r.executed_date > existing.executed_date {
                                    *existing = r;
                                }
                            })
                            .or_insert(r);
                    }
                }
                latest_by_test
                    .into_iter()
                    .filter(|(_, r)| r.verdict.as_deref() == Some("fail"))
                    .map(|(id, _)| id.to_string())
                    .collect()
            };

            tests.retain(|t| {
                let tid_str = t.id.to_string();
                let no_results_match = !args.no_results || !tests_with_results.contains(&tid_str);
                let last_failed_match = !args.last_failed || last_failed_tests.contains(&tid_str);
                no_results_match && last_failed_match
            });
        }

        // Apply tag filter (not in cache)
        if let Some(ref tag) = args.tag {
            let tag_lower = tag.to_lowercase();
            tests.retain(|t| t.tags.iter().any(|tg| tg.to_lowercase() == tag_lower));
        }

        // Apply limit
        if let Some(limit) = args.limit {
            tests.truncate(limit);
        }

        // Handle count-only mode
        if args.count {
            println!("{}", tests.len());
            return Ok(());
        }

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

        let mut short_ids = ShortIdIndex::load(&project);
        let mut visible: Vec<String> = args.columns.iter().map(|c| c.to_string()).collect();
        if args.show_id && !visible.iter().any(|c| c == "id") {
            visible.insert(0, "id".to_string());
        }
        crate::cli::entity_cmd::output_entity_list(
            &tests,
            &TEST_LIST_SPEC,
            &visible,
            args.wrap,
            args.count,
            format,
            &project,
            &mut short_ids,
            |x| x.id.to_string(),
            test_to_row,
        )
    } else {
        // Fast path: use cache via service
        let result = service
            .list_cached(&filter)
            .map_err(|e| miette::miette!("{}", e))?;

        let mut tests = result.items;

        // Apply linked-to filter
        if let Some(ref ids) = allowed_ids {
            tests.retain(|e| ids.contains(&e.id));
        }

        // Sort cached results
        sort_cached_tests(&mut tests, &args);

        // Apply limit
        if let Some(limit) = args.limit {
            tests.truncate(limit);
        }

        // Handle count-only mode
        if args.count {
            println!("{}", tests.len());
            return Ok(());
        }

        if tests.is_empty() {
            println!("No tests found.");
            println!();
            println!("Create one with: {}", style("tdt test new").yellow());
            return Ok(());
        }

        // Update short ID index
        let mut short_ids = ShortIdIndex::load(&project);
        short_ids.ensure_all(tests.iter().map(|t| t.id.clone()));
        super::utils::save_short_ids(&mut short_ids, &project);

        {
            let mut visible: Vec<String> = args.columns.iter().map(|c| c.to_string()).collect();
            if args.show_id && !visible.iter().any(|c| c == "id") {
                visible.insert(0, "id".to_string());
            }
            crate::cli::entity_cmd::output_cached_list(
                &tests,
                &TEST_LIST_SPEC,
                &visible,
                args.wrap,
                args.count,
                format,
                &project,
                |t| cached_test_to_row(t, &short_ids),
                |t| t.file_path.as_path(),
            )
        }
    }
}

/// Build TestFilter from CLI args
fn build_test_filter(args: &ListArgs) -> TestFilter {
    // Convert status filter
    let status = match args.status {
        StatusFilter::Draft => Some(vec![Status::Draft]),
        StatusFilter::Review => Some(vec![Status::Review]),
        StatusFilter::Approved => Some(vec![Status::Approved]),
        StatusFilter::Released => Some(vec![Status::Released]),
        StatusFilter::Obsolete => Some(vec![Status::Obsolete]),
        StatusFilter::Active | StatusFilter::All => None,
    };

    // Convert priority filter
    let priority = args
        .priority
        .as_ref()
        .and_then(|p| match p.to_lowercase().as_str() {
            "low" => Some(Priority::Low),
            "medium" => Some(Priority::Medium),
            "high" => Some(Priority::High),
            "critical" => Some(Priority::Critical),
            _ => None,
        });

    // Convert type filter
    let test_type = match args.r#type {
        TestTypeFilter::Verification => Some(TestType::Verification),
        TestTypeFilter::Validation => Some(TestType::Validation),
        TestTypeFilter::All => None,
    };

    // Convert level filter
    let test_level = match args.level {
        TestLevelFilter::Unit => Some(TestLevel::Unit),
        TestLevelFilter::Integration => Some(TestLevel::Integration),
        TestLevelFilter::System => Some(TestLevel::System),
        TestLevelFilter::Acceptance => Some(TestLevel::Acceptance),
        TestLevelFilter::All => None,
    };

    // Convert method filter
    let test_method = match args.method {
        TestMethodFilter::Inspection => Some(TestMethod::Inspection),
        TestMethodFilter::Analysis => Some(TestMethod::Analysis),
        TestMethodFilter::Demonstration => Some(TestMethod::Demonstration),
        TestMethodFilter::Test => Some(TestMethod::Test),
        TestMethodFilter::All => None,
    };

    TestFilter {
        common: CommonFilter {
            status,
            author: args.author.clone(),
            search: args.search.clone(),
            recent_days: args.recent,
            limit: None, // Apply limit after all filters
            ..Default::default()
        },
        test_type,
        test_level,
        test_method,
        priority,
        category: args.category.clone(),
        orphans_only: args.orphans,
        no_results: false, // Handled as post-filter with result loading
    }
}

/// Build sort field and direction from CLI args
fn build_test_sort(args: &ListArgs) -> (TestSortField, SortDirection) {
    let field = match args.sort {
        ListColumn::Id => TestSortField::Id,
        ListColumn::Type => TestSortField::Type,
        ListColumn::Level => TestSortField::Level,
        ListColumn::Method => TestSortField::Method,
        ListColumn::Title => TestSortField::Title,
        ListColumn::Status => TestSortField::Status,
        ListColumn::Priority => TestSortField::Priority,
        ListColumn::Category => TestSortField::Category,
        ListColumn::Author => TestSortField::Author,
        ListColumn::Created => TestSortField::Created,
    };

    let direction = if args.reverse {
        match field {
            TestSortField::Created | TestSortField::Priority => SortDirection::Ascending,
            _ => SortDirection::Descending,
        }
    } else {
        match field {
            TestSortField::Created | TestSortField::Priority => SortDirection::Descending,
            _ => SortDirection::Ascending,
        }
    };

    (field, direction)
}

/// Sort cached tests based on CLI args
fn sort_cached_tests(tests: &mut [CachedTest], args: &ListArgs) {
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

/// Convert string to Priority
fn parse_priority(s: &str) -> Priority {
    match s.to_lowercase().as_str() {
        "low" => Priority::Low,
        "high" => Priority::High,
        "critical" => Priority::Critical,
        _ => Priority::Medium,
    }
}

fn run_new(args: NewArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let cache = EntityCache::open(&project).map_err(|e| miette::miette!("{}", e))?;
    let service = TestService::new(&project, &cache);
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

            let category = result.get_string("category").map(String::from);

            let priority = result
                .get_string("priority")
                .map(parse_priority)
                .unwrap_or(Priority::Medium);

            // Extract text fields
            let objective = result
                .get_string("objective")
                .map(String::from)
                .unwrap_or_default();
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
            let category = args.category;
            let priority: Priority = args.priority.into();
            let objective = args.objective.unwrap_or_default();

            (
                test_type,
                test_level,
                test_method,
                title,
                category,
                priority,
                objective,
                None,
            )
        };

    // Create test using service
    let input = CreateTest {
        title: title.clone(),
        author: config.author(),
        test_type,
        test_level: Some(test_level),
        test_method: Some(test_method),
        objective,
        description,
        category,
        priority,
        ..Default::default()
    };

    let test = service
        .create(input)
        .map_err(|e| miette::miette!("{}", e))?;

    // Determine output directory based on type
    let file_path = project
        .test_directory(&test.test_type.to_string())
        .join(format!("{}.tdt.yaml", test.id));

    // Add to short ID index
    let mut short_ids = ShortIdIndex::load(&project);
    let short_id = short_ids.add(test.id.to_string());
    super::utils::save_short_ids(&mut short_ids, &project);

    // Handle --verifies and --mitigates flags by updating the file with links
    if !args.verifies.is_empty() || !args.mitigates.is_empty() {
        // Parse the test we just created
        let mut test_entity: Test = tdt_core::yaml::parse_yaml_file(&file_path)
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
            if !test_entity.links.verifies.contains(&entity_id) {
                test_entity.links.verifies.push(entity_id);
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
            if !test_entity.links.mitigates.contains(&entity_id) {
                test_entity.links.mitigates.push(entity_id);
            }
        }

        // Write back the updated test
        let updated_yaml = serde_yml::to_string(&test_entity).into_diagnostic()?;
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
            println!("{}", test.id);
        }
        OutputFormat::ShortId => {
            println!(
                "{}",
                short_id
                    .clone()
                    .unwrap_or_else(|| format_short_id(&test.id))
            );
        }
        OutputFormat::Path => {
            println!("{}", file_path.display());
        }
        _ => {
            println!(
                "{} Created test {}",
                style("✓").green(),
                style(
                    short_id
                        .clone()
                        .unwrap_or_else(|| format_short_id(&test.id))
                )
                .cyan()
            );
            println!("   {}", style(file_path.display()).dim());
            println!(
                "   Type: {} | Level: {} | Method: {}",
                style(test.test_type.to_string()).yellow(),
                style(test.test_level.map_or("".to_string(), |l| l.to_string())).yellow(),
                style(test.test_method.map_or("".to_string(), |m| m.to_string())).yellow()
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

    // Use TestService to get the test (cache-first lookup)
    let service = TestService::new(&project, &cache);
    let test = service
        .get(&resolved_id)
        .map_err(|e| miette::miette!("{}", e))?
        .ok_or_else(|| miette::miette!("No test found matching '{}'", args.id))?;

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
                let short_id = short_ids
                    .get_short_id(&test.id.to_string())
                    .unwrap_or_default();
                println!("{}", short_id);
            } else {
                println!("{}", test.id);
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
                            .map(|id| format_link_with_title(
                                &id.to_string(),
                                &short_ids,
                                &cache_opt
                            ))
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
                            .map(|id| format_link_with_title(
                                &id.to_string(),
                                &short_ids,
                                &cache_opt
                            ))
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
                            .map(|id| format_link_with_title(
                                &id.to_string(),
                                &short_ids,
                                &cache_opt
                            ))
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
    let cache = EntityCache::open(&project).map_err(|e| miette::miette!("{}", e))?;
    let config = Config::load();

    // Resolve short ID if needed
    let short_ids = ShortIdIndex::load(&project);
    let resolved_id = short_ids
        .resolve(&args.id)
        .unwrap_or_else(|| args.id.clone());

    // Use TestService to get the entity
    let service = TestService::new(&project, &cache);
    let test = service
        .get(&resolved_id)
        .map_err(|e| miette::miette!("{}", e))?
        .ok_or_else(|| miette::miette!("No test found matching '{}'", args.id))?;

    // Get file path from cache
    let file_path = if let Some(cached) = cache.get_entity(&test.id.to_string()) {
        if cached.file_path.is_absolute() {
            cached.file_path.clone()
        } else {
            project.root().join(&cached.file_path)
        }
    } else {
        // Fallback: compute path from entity type
        let test_type = match test.test_type {
            TestType::Verification => "verification",
            TestType::Validation => "validation",
        };
        project
            .root()
            .join(format!("{}/protocols/{}.tdt.yaml", test_type, test.id))
    };

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
                Verdict::Fail => StepResult::Fail,
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
        status: tdt_core::core::entity::Status::default(),
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
