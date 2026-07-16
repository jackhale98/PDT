//! `tdt req` command - Requirement management

use clap::{Subcommand, ValueEnum};
use console::style;
use miette::{IntoDiagnostic, Result};

use crate::cli::commands::utils::format_link_with_title;
use crate::cli::entity_cmd::{CommonListArgs, ListConfig};
use crate::cli::filters::StatusFilter;
use crate::cli::helpers::resolve_id_arg;
use crate::cli::table::{CellValue, ColumnDef, TableRow};
use crate::cli::{GlobalOpts, OutputFormat};
use tdt_core::core::cache::EntityCache;
use tdt_core::core::entity::{Priority, Status};
use tdt_core::core::identity::EntityPrefix;
use tdt_core::core::project::Project;
use tdt_core::core::shortid::ShortIdIndex;
use tdt_core::core::Config;
use tdt_core::entities::requirement::{Level, Requirement, RequirementType};
use tdt_core::schema::wizard::SchemaWizard;
use tdt_core::services::{
    CommonFilter, CreateRequirement, RequirementFilter, RequirementService, RequirementSortField,
    SortDirection,
};

/// CLI-friendly requirement type enum
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CliReqType {
    Input,
    Output,
}

impl std::fmt::Display for CliReqType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CliReqType::Input => write!(f, "input"),
            CliReqType::Output => write!(f, "output"),
        }
    }
}

impl From<CliReqType> for RequirementType {
    fn from(cli: CliReqType) -> Self {
        match cli {
            CliReqType::Input => RequirementType::Input,
            CliReqType::Output => RequirementType::Output,
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

/// CLI-friendly level enum for V-model hierarchy
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CliLevel {
    Stakeholder,
    System,
    Subsystem,
    Component,
    Detail,
}

impl std::fmt::Display for CliLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CliLevel::Stakeholder => write!(f, "stakeholder"),
            CliLevel::System => write!(f, "system"),
            CliLevel::Subsystem => write!(f, "subsystem"),
            CliLevel::Component => write!(f, "component"),
            CliLevel::Detail => write!(f, "detail"),
        }
    }
}

impl From<CliLevel> for Level {
    fn from(cli: CliLevel) -> Self {
        match cli {
            CliLevel::Stakeholder => Level::Stakeholder,
            CliLevel::System => Level::System,
            CliLevel::Subsystem => Level::Subsystem,
            CliLevel::Component => Level::Component,
            CliLevel::Detail => Level::Detail,
        }
    }
}

#[derive(Subcommand, Debug)]
pub enum ReqCommands {
    /// List requirements with filtering
    List(ListArgs),

    /// Create a new requirement
    New(NewArgs),

    /// Show a requirement's details
    Show(ShowArgs),

    /// Edit a requirement in your editor
    Edit(EditArgs),

    /// Delete a requirement
    Delete(DeleteArgs),

    /// Archive a requirement (move to .tdt/archive/)
    Archive(ArchiveArgs),

    /// Show requirement statistics
    Stats(StatsArgs),
}

/// Requirement type filter
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ReqTypeFilter {
    Input,
    Output,
    All,
}

/// Priority filter
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum PriorityFilter {
    Low,
    Medium,
    High,
    Critical,
    /// High and critical only
    Urgent,
    /// All priorities
    All,
}

/// Level filter for V-model hierarchy
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum LevelFilter {
    Stakeholder,
    System,
    Subsystem,
    Component,
    Detail,
    /// All levels
    All,
}

/// Columns to display in list output
#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
pub enum ListColumn {
    Id,
    Type,
    Level,
    Title,
    Status,
    Priority,
    Category,
    Author,
    Created,
    Tags,
}

impl std::fmt::Display for ListColumn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ListColumn::Id => write!(f, "id"),
            ListColumn::Type => write!(f, "type"),
            ListColumn::Level => write!(f, "level"),
            ListColumn::Title => write!(f, "title"),
            ListColumn::Status => write!(f, "status"),
            ListColumn::Priority => write!(f, "priority"),
            ListColumn::Category => write!(f, "category"),
            ListColumn::Author => write!(f, "author"),
            ListColumn::Created => write!(f, "created"),
            ListColumn::Tags => write!(f, "tags"),
        }
    }
}

/// Column definitions for requirement list output
const REQ_COLUMNS: &[ColumnDef] = &[
    ColumnDef::new("id", "ID", 17),
    ColumnDef::new("type", "TYPE", 8),
    ColumnDef::new("level", "LEVEL", 11),
    ColumnDef::new("title", "TITLE", 35),
    ColumnDef::new("status", "STATUS", 10),
    ColumnDef::new("priority", "PRI", 10),
    ColumnDef::new("category", "CATEGORY", 12),
    ColumnDef::new("author", "AUTHOR", 16),
    ColumnDef::new("created", "CREATED", 12),
    ColumnDef::new("tags", "TAGS", 20),
];

#[derive(clap::Args, Debug)]
pub struct ListArgs {
    // ========== FILTERING OPTIONS ==========
    // These let users filter without needing awk/grep
    /// Filter by type
    #[arg(long, short = 't', default_value = "all")]
    pub r#type: ReqTypeFilter,

    /// Filter by status
    #[arg(long, short = 's', default_value = "all")]
    pub status: StatusFilter,

    /// Filter by priority
    #[arg(long, short = 'p', default_value = "all")]
    pub priority: PriorityFilter,

    /// Filter by V-model level
    #[arg(long, short = 'l', default_value = "all")]
    pub level: LevelFilter,

    /// Filter by category (exact match)
    #[arg(long, short = 'c')]
    pub category: Option<String>,

    /// Filter by tag (requirements with this tag)
    #[arg(long)]
    pub tag: Option<String>,

    /// Filter by author
    #[arg(long, short = 'a')]
    pub author: Option<String>,

    /// Search in title and text (case-insensitive substring)
    #[arg(long)]
    pub search: Option<String>,

    // ========== COMMON FILTER SHORTCUTS ==========
    // Pre-built filters for common queries
    /// Show only unlinked requirements (no satisfied_by or verified_by)
    #[arg(long)]
    pub orphans: bool,

    /// Show requirements needing review (status=draft or review)
    #[arg(long)]
    pub needs_review: bool,

    /// Show requirements created in the last N days
    #[arg(long, value_name = "DAYS")]
    pub recent: Option<u32>,

    // ========== VERIFICATION STATUS FILTERS ==========
    /// Show unverified requirements (no verified_by links)
    #[arg(long)]
    pub unverified: bool,

    /// Show untested requirements (has tests but no results yet)
    #[arg(long)]
    pub untested: bool,

    /// Show requirements where linked tests have failed
    #[arg(long)]
    pub failed: bool,

    /// Show requirements where all linked tests pass
    #[arg(long)]
    pub passing: bool,

    // ========== OUTPUT CONTROL ==========
    /// Columns to display (can specify multiple)
    #[arg(long, value_delimiter = ',', default_values_t = vec![
        ListColumn::Type,
        ListColumn::Title,
        ListColumn::Status,
        ListColumn::Priority
    ])]
    pub columns: Vec<ListColumn>,

    /// Sort by field
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
    /// Requirement type
    #[arg(long, short = 't', default_value = "input")]
    pub r#type: CliReqType,

    /// Title (if not provided, uses placeholder)
    #[arg(long, short = 'T')]
    pub title: Option<String>,

    /// Category
    #[arg(long, short = 'c')]
    pub category: Option<String>,

    /// Priority
    #[arg(long, short = 'p', default_value = "medium")]
    pub priority: CliPriority,

    /// V-model hierarchy level
    #[arg(long, short = 'l', default_value = "system")]
    pub level: CliLevel,

    /// Tags (comma-separated)
    #[arg(long, value_delimiter = ',')]
    pub tags: Vec<String>,

    /// Requirement text - the full statement of the requirement
    #[arg(long, short = 'x')]
    pub text: Option<String>,

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
    /// Requirement ID or fuzzy search term (or pipe ID via stdin)
    pub id: Option<String>,

    /// Show linked entities too
    #[arg(long)]
    pub with_links: bool,

    /// Show revision history (from git)
    #[arg(long)]
    pub history: bool,
}

#[derive(clap::Args, Debug)]
pub struct EditArgs {
    /// Requirement ID or fuzzy search term (or pipe ID via stdin)
    pub id: Option<String>,
}

#[derive(clap::Args, Debug)]
pub struct DeleteArgs {
    /// Requirement ID or short ID (REQ@N)
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
    /// Requirement ID or short ID (REQ@N)
    pub id: String,

    /// Force archive even if other entities reference this one
    #[arg(long)]
    pub force: bool,

    /// Suppress output
    #[arg(long, short = 'q')]
    pub quiet: bool,
}

#[derive(clap::Args, Debug)]
pub struct StatsArgs {
    /// Show detailed breakdown by level
    #[arg(long)]
    pub by_level: bool,

    /// Show detailed breakdown by category
    #[arg(long)]
    pub by_category: bool,
}

/// Search directories for this entity type — single source of truth
/// lives in `Project::entity_search_directories`.
const REQ_DIRS: &[&str] = tdt_core::core::project::Project::entity_search_directories(
    tdt_core::core::identity::EntityPrefix::Req,
);

/// Entity configuration for requirements
const ENTITY_CONFIG: crate::cli::EntityConfig = crate::cli::EntityConfig {
    prefix: EntityPrefix::Req,
    dirs: REQ_DIRS,
    name: "requirement",
    name_plural: "requirements",
};

pub fn run(cmd: ReqCommands, global: &GlobalOpts) -> Result<()> {
    match cmd {
        ReqCommands::List(args) => run_list(args, global),
        ReqCommands::New(args) => run_new(args, global),
        ReqCommands::Show(args) => run_show(args, global),
        ReqCommands::Edit(args) => run_edit(args),
        ReqCommands::Delete(args) => run_delete(args),
        ReqCommands::Archive(args) => run_archive(args),
        ReqCommands::Stats(args) => run_stats(args, global),
    }
}

fn run_list(args: ListArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let cache = EntityCache::open(&project).map_err(|e| miette::miette!("{}", e))?;
    let service = RequirementService::new(&project, &cache);

    // Build filter and sort from CLI args
    let filter = build_req_filter(&args);
    let (sort_field, sort_dir) = build_req_sort(&args);

    // Resolve linked-to filter via cache
    let short_ids = ShortIdIndex::load(&project);
    let allowed_ids = crate::cli::helpers::resolve_linked_to(
        &args.linked_to,
        args.via.as_deref(),
        &short_ids,
        &cache,
    );

    // Check if we need special post-filtering (test-result based filters)
    let needs_test_result_filters = args.untested || args.failed || args.passing;

    if needs_test_result_filters {
        // Special case: test-result filters require loading full entities and post-filtering
        run_list_with_test_filters(
            &args,
            &service,
            &filter,
            sort_field,
            sort_dir,
            global,
            &project,
            &cache,
            allowed_ids.as_ref(),
        )
    } else {
        // Standard case: use generic list infrastructure
        let config = ListConfig {
            columns: REQ_COLUMNS,
            entity_name: "requirement",
            prefix_str: "REQ",
            entity_to_row: requirement_to_row,
            cached_to_row: cached_requirement_to_row,
            cached_sort: Some(sort_cached_requirements_by_field),
        };

        let common_args = CommonListArgs {
            columns: args.columns.iter().map(|c| c.to_string()).collect(),
            limit: args.limit,
            reverse: args.reverse,
            count: args.count,
            wrap: args.wrap,
        };

        crate::cli::entity_cmd::run_list_generic(
            &config,
            &service,
            &filter,
            sort_field,
            sort_dir,
            &common_args,
            global,
            &project,
            false,
            allowed_ids.as_ref(),
        )
    }
}

/// Handle list with test-result based post-filters (untested/failed/passing)
fn run_list_with_test_filters(
    args: &ListArgs,
    service: &RequirementService,
    filter: &RequirementFilter,
    sort_field: RequirementSortField,
    sort_dir: SortDirection,
    global: &GlobalOpts,
    project: &Project,
    cache: &EntityCache,
    allowed_ids: Option<&std::collections::HashSet<String>>,
) -> Result<()> {
    let output_format = match global.output {
        OutputFormat::Auto => OutputFormat::Tsv,
        f => f,
    };

    // Load full entities via service (needed for link access)
    let result = service
        .list(filter, sort_field, sort_dir)
        .map_err(|e| miette::miette!("{}", e))?;

    let mut reqs = result.items;

    // Apply linked-to filter
    if let Some(ids) = allowed_ids {
        reqs.retain(|r| ids.contains(&r.id.to_string()));
    }

    // Apply test-result based filters (require cross-entity queries)
    // Use cache for fast lookups instead of walking directories
    let cached_results = cache.list_results(None, None, None, None, None, None);

    // Pre-compute HashSets for O(1) lookups
    use std::collections::HashSet;
    let tested_test_ids: HashSet<&str> = cached_results
        .iter()
        .filter_map(|r| r.test_id.as_deref())
        .collect();
    let failed_test_ids: HashSet<&str> = cached_results
        .iter()
        .filter(|r| r.verdict.as_deref() == Some("fail"))
        .filter_map(|r| r.test_id.as_deref())
        .collect();
    let passing_test_ids: HashSet<&str> = cached_results
        .iter()
        .filter(|r| r.verdict.as_deref() == Some("pass"))
        .filter_map(|r| r.test_id.as_deref())
        .collect();

    reqs.retain(|req| {
        // Merge test IDs from both directions:
        // 1. req.links.verified_by (tests listed in requirement)
        // 2. cache.get_links_to (tests that have verifies -> this requirement)
        let mut all_test_ids: HashSet<String> = req
            .links
            .verified_by
            .iter()
            .map(|id| id.to_string())
            .collect();

        // Add tests from reverse lookup (test.verifies -> this req)
        let req_id_str = req.id.to_string();
        let reverse_test_ids = cache.get_links_to_of_type(&req_id_str, "verifies");
        for test_id in reverse_test_ids {
            all_test_ids.insert(test_id);
        }

        let untested_match = if args.untested {
            !all_test_ids.is_empty()
                && !all_test_ids
                    .iter()
                    .any(|tid| tested_test_ids.contains(tid.as_str()))
        } else {
            true
        };

        let failed_match = if args.failed {
            all_test_ids
                .iter()
                .any(|tid| failed_test_ids.contains(tid.as_str()))
        } else {
            true
        };

        let passing_match = if args.passing {
            !all_test_ids.is_empty()
                && all_test_ids
                    .iter()
                    .all(|tid| passing_test_ids.contains(tid.as_str()))
        } else {
            true
        };

        untested_match && failed_match && passing_match
    });

    // Apply reverse and limit
    if args.reverse {
        reqs.reverse();
    }
    if let Some(limit) = args.limit {
        reqs.truncate(limit);
    }

    // Handle count-only mode
    if args.count {
        println!("{}", reqs.len());
        return Ok(());
    }

    if reqs.is_empty() {
        match output_format {
            OutputFormat::Json => println!("[]"),
            OutputFormat::Yaml => println!("[]"),
            _ => {
                println!("No requirements found matching filters.");
                println!();
                println!("Create one with: {}", style("tdt req new").yellow());
            }
        }
        return Ok(());
    }

    // Update short ID index
    let mut short_ids = ShortIdIndex::load(project);
    short_ids.ensure_all(reqs.iter().map(|r| r.id.to_string()));
    super::utils::save_short_ids(&mut short_ids, project);

    // Use generic output
    let config = ListConfig {
        columns: REQ_COLUMNS,
        entity_name: "requirement",
        prefix_str: "REQ",
        entity_to_row: requirement_to_row,
        cached_to_row: cached_requirement_to_row,
        cached_sort: Some(sort_cached_requirements_by_field),
    };

    let common_args = CommonListArgs {
        columns: args.columns.iter().map(|c| c.to_string()).collect(),
        limit: None,    // Already applied
        reverse: false, // Already applied
        count: false,   // Already handled
        wrap: args.wrap,
    };

    crate::cli::entity_cmd::output_full_entities(
        &reqs,
        &config,
        &short_ids,
        &common_args,
        output_format,
    )
}

/// Build RequirementFilter from CLI args
fn build_req_filter(args: &ListArgs) -> RequirementFilter {
    // Convert status filter
    let status = match args.status {
        StatusFilter::Draft => Some(vec![Status::Draft]),
        StatusFilter::Review => Some(vec![Status::Review]),
        StatusFilter::Approved => Some(vec![Status::Approved]),
        StatusFilter::Released => Some(vec![Status::Released]),
        StatusFilter::Obsolete => Some(vec![Status::Obsolete]),
        StatusFilter::Active => None, // Active = not Obsolete, handled by service
        StatusFilter::All => None,
    };

    // Convert priority filter
    let priority = match args.priority {
        PriorityFilter::Low => Some(vec![Priority::Low]),
        PriorityFilter::Medium => Some(vec![Priority::Medium]),
        PriorityFilter::High => Some(vec![Priority::High]),
        PriorityFilter::Critical => Some(vec![Priority::Critical]),
        PriorityFilter::Urgent => Some(vec![Priority::High, Priority::Critical]),
        PriorityFilter::All => None,
    };

    // Convert type filter
    let req_type = match args.r#type {
        ReqTypeFilter::Input => Some(RequirementType::Input),
        ReqTypeFilter::Output => Some(RequirementType::Output),
        ReqTypeFilter::All => None,
    };

    // Convert level filter
    let level = match args.level {
        LevelFilter::Stakeholder => Some(Level::Stakeholder),
        LevelFilter::System => Some(Level::System),
        LevelFilter::Subsystem => Some(Level::Subsystem),
        LevelFilter::Component => Some(Level::Component),
        LevelFilter::Detail => Some(Level::Detail),
        LevelFilter::All => None,
    };

    RequirementFilter {
        common: CommonFilter {
            status,
            priority,
            author: args.author.clone(),
            tags: args.tag.clone().map(|t| vec![t]),
            search: args.search.clone(),
            recent_days: args.recent,
            limit: args.limit,
            ..Default::default()
        },
        req_type,
        level,
        category: args.category.clone(),
        orphans_only: args.orphans,
        needs_review: args.needs_review,
        unverified_only: args.unverified,
    }
}

/// Build sort field and direction from CLI args
fn build_req_sort(args: &ListArgs) -> (RequirementSortField, SortDirection) {
    let field = match args.sort {
        ListColumn::Id => RequirementSortField::Id,
        ListColumn::Type => RequirementSortField::Type,
        ListColumn::Level => RequirementSortField::Level,
        ListColumn::Title => RequirementSortField::Title,
        ListColumn::Status => RequirementSortField::Status,
        ListColumn::Priority => RequirementSortField::Priority,
        ListColumn::Category => RequirementSortField::Category,
        ListColumn::Author => RequirementSortField::Author,
        ListColumn::Created => RequirementSortField::Created,
        ListColumn::Tags => RequirementSortField::Title, // Tags sort not supported, fallback to title
    };

    let direction = if args.reverse {
        // Reverse the default direction
        match field {
            RequirementSortField::Created => SortDirection::Ascending,
            RequirementSortField::Priority => SortDirection::Ascending,
            _ => SortDirection::Descending,
        }
    } else {
        // Default direction: newest/highest priority first for dates/priority, ascending for others
        match field {
            RequirementSortField::Created => SortDirection::Descending,
            RequirementSortField::Priority => SortDirection::Descending,
            _ => SortDirection::Ascending,
        }
    };

    (field, direction)
}

/// Sort cached requirements by service sort field and direction
fn sort_cached_requirements_by_field(
    reqs: &mut [tdt_core::core::CachedRequirement],
    field: RequirementSortField,
    dir: SortDirection,
) {
    match field {
        RequirementSortField::Id => reqs.sort_by(|a, b| a.id.cmp(&b.id)),
        RequirementSortField::Type => reqs.sort_by(|a, b| a.req_type.cmp(&b.req_type)),
        RequirementSortField::Level => reqs.sort_by(|a, b| a.level.cmp(&b.level)),
        RequirementSortField::Title => reqs.sort_by(|a, b| a.title.cmp(&b.title)),
        RequirementSortField::Status => reqs.sort_by(|a, b| a.status.cmp(&b.status)),
        RequirementSortField::Priority => {
            let priority_order = |p: Option<Priority>| match p {
                Some(Priority::Critical) => 0,
                Some(Priority::High) => 1,
                Some(Priority::Medium) => 2,
                Some(Priority::Low) => 3,
                None => 4,
            };
            reqs.sort_by(|a, b| priority_order(a.priority).cmp(&priority_order(b.priority)));
        }
        RequirementSortField::Category => reqs.sort_by(|a, b| a.category.cmp(&b.category)),
        RequirementSortField::Author => reqs.sort_by(|a, b| a.author.cmp(&b.author)),
        RequirementSortField::Created => reqs.sort_by(|a, b| a.created.cmp(&b.created)),
    }

    if dir == SortDirection::Descending {
        reqs.reverse();
    }
}

/// Convert a Requirement to a TableRow
fn requirement_to_row(req: &Requirement, short_ids: &ShortIdIndex) -> TableRow {
    TableRow::new(req.id.to_string(), short_ids)
        .cell("id", CellValue::Id(req.id.to_string()))
        .cell("type", CellValue::Type(req.req_type.to_string()))
        .cell("level", CellValue::Text(req.level.to_string()))
        .cell("title", CellValue::Text(req.title.clone()))
        .cell("status", CellValue::Status(req.status))
        .cell("priority", CellValue::Priority(req.priority))
        .cell(
            "category",
            CellValue::Text(req.category.clone().unwrap_or_default()),
        )
        .cell("author", CellValue::Text(req.author.clone()))
        .cell("created", CellValue::Date(req.created))
        .cell("tags", CellValue::Tags(req.tags.clone()))
}

/// Convert a CachedRequirement to a TableRow
fn cached_requirement_to_row(
    req: &tdt_core::core::CachedRequirement,
    short_ids: &ShortIdIndex,
) -> TableRow {
    TableRow::new(req.id.clone(), short_ids)
        .cell("id", CellValue::Id(req.id.clone()))
        .cell(
            "type",
            CellValue::Type(req.req_type.clone().unwrap_or_else(|| "input".to_string())),
        )
        .cell(
            "level",
            CellValue::Text(req.level.clone().unwrap_or_else(|| "system".to_string())),
        )
        .cell("title", CellValue::Text(req.title.clone()))
        .cell("status", CellValue::Status(req.status))
        .cell("priority", CellValue::OptionalPriority(req.priority))
        .cell(
            "category",
            CellValue::Text(req.category.clone().unwrap_or_default()),
        )
        .cell("author", CellValue::Text(req.author.clone()))
        .cell("created", CellValue::Date(req.created))
        .cell("tags", CellValue::Tags(req.tags.clone()))
}

fn run_new(args: NewArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let cache = EntityCache::open(&project).map_err(|e| miette::miette!("{}", e))?;
    let service = RequirementService::new(&project, &cache);
    let config = Config::load();

    // Determine values - either from schema-driven wizard or args
    let (req_type, level, title, priority, category, tags, text, rationale, acceptance_criteria) =
        if args.interactive {
            // Use the schema-driven wizard
            let wizard = SchemaWizard::new();
            let result = wizard.run(EntityPrefix::Req)?;

            // Extract values from wizard result
            let req_type = result
                .get_string("type")
                .map(|s| match s {
                    "output" => RequirementType::Output,
                    _ => RequirementType::Input,
                })
                .unwrap_or(RequirementType::Input);

            let level = result
                .get_string("level")
                .map(|s| match s {
                    "stakeholder" => Level::Stakeholder,
                    "subsystem" => Level::Subsystem,
                    "component" => Level::Component,
                    "detail" => Level::Detail,
                    _ => Level::System,
                })
                .unwrap_or(Level::System);

            let title = result
                .get_string("title")
                .map(String::from)
                .unwrap_or_else(|| "New Requirement".to_string());

            let priority = result
                .get_string("priority")
                .map(|s| match s {
                    "low" => Priority::Low,
                    "high" => Priority::High,
                    "critical" => Priority::Critical,
                    _ => Priority::Medium,
                })
                .unwrap_or(Priority::Medium);

            let category = result.get_string("category").map(String::from);

            let tags: Vec<String> = result
                .values
                .get("tags")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str())
                        .map(String::from)
                        .collect()
                })
                .unwrap_or_default();

            // Extract text fields from wizard
            let text = result
                .get_string("text")
                .map(String::from)
                .unwrap_or_default();
            let rationale = result.get_string("rationale").map(String::from);
            let acceptance_criteria: Vec<String> = result
                .values
                .get("acceptance_criteria")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str())
                        .map(String::from)
                        .collect()
                })
                .unwrap_or_default();

            (
                req_type,
                level,
                title,
                priority,
                category,
                tags,
                text,
                rationale,
                acceptance_criteria,
            )
        } else {
            // Default mode - use args with defaults
            let req_type: RequirementType = args.r#type.into();
            let level: Level = args.level.into();
            let title = args.title.unwrap_or_else(|| "New Requirement".to_string());
            let priority: Priority = args.priority.into();
            let category = args.category;
            let tags = args.tags;
            let text = args.text.unwrap_or_default();

            (
                req_type,
                level,
                title,
                priority,
                category,
                tags,
                text,
                None,
                vec![],
            )
        };

    // Create requirement using service
    let input = CreateRequirement {
        req_type,
        title: title.clone(),
        text,
        author: config.author(),
        level,
        priority,
        category,
        tags,
        rationale,
        acceptance_criteria,
        source: None,
    };

    let requirement = service
        .create(input)
        .map_err(|e| miette::miette!("{}", e))?;

    // Determine output directory based on type
    let file_path = project
        .requirement_directory(&requirement.req_type.to_string())
        .join(format!("{}.tdt.yaml", requirement.id));

    // Add to short ID index
    let mut short_ids = ShortIdIndex::load(&project);
    let short_id = short_ids.add(requirement.id.to_string());
    super::utils::save_short_ids(&mut short_ids, &project);

    // Handle --link flags
    let added_links = crate::cli::entity_cmd::process_link_flags(
        &file_path,
        EntityPrefix::Req,
        &args.link,
        &short_ids,
    );

    // Output based on format flag
    crate::cli::entity_cmd::output_new_entity(
        &requirement.id,
        &file_path,
        short_id.clone(),
        ENTITY_CONFIG.name,
        &requirement.title,
        None,
        &added_links,
        global,
    );

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

    // Resolve ID from argument or stdin
    let id = resolve_id_arg(&args.id).map_err(|e| miette::miette!("{}", e))?;

    // Resolve short ID if needed
    let short_ids = ShortIdIndex::load(&project);
    let resolved_id = short_ids.resolve(&id).unwrap_or_else(|| id.clone());

    // Use RequirementService to get the requirement (cache-first lookup)
    let service = RequirementService::new(&project, &cache);
    let req = service
        .get(&resolved_id)
        .map_err(|e| miette::miette!("{}", e))?
        .ok_or_else(|| miette::miette!("No requirement found matching '{}'", id))?;

    // Output based on format (pretty is default, yaml/json explicit)
    match global.output {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&req).into_diagnostic()?;
            println!("{}", json);
        }
        OutputFormat::Yaml => {
            let yaml = serde_yml::to_string(&req).into_diagnostic()?;
            print!("{}", yaml);
        }
        OutputFormat::Id => {
            println!("{}", req.id);
        }
        _ => {
            // Reopen cache for title lookups (format_link_with_title expects Option<EntityCache>)
            let cache_opt = EntityCache::open(&project).ok();

            // Human-readable format (default for terminal)
            println!("{}", style("─".repeat(60)).dim());
            println!(
                "{}: {}",
                style("ID").bold(),
                style(&req.id.to_string()).cyan()
            );
            println!("{}: {}", style("Type").bold(), req.req_type);
            println!("{}: {}", style("Title").bold(), style(&req.title).yellow());
            println!("{}: {}", style("Status").bold(), req.status);
            println!("{}: {}", style("Priority").bold(), req.priority);
            if let Some(ref cat) = req.category {
                if !cat.is_empty() {
                    println!("{}: {}", style("Category").bold(), cat);
                }
            }
            if !req.tags.is_empty() {
                println!("{}: {}", style("Tags").bold(), req.tags.join(", "));
            }
            println!("{}", style("─".repeat(60)).dim());
            println!();
            println!("{}", &req.text);
            println!();

            if let Some(ref rationale) = req.rationale {
                if !rationale.is_empty() {
                    println!("{}", style("Rationale:").bold());
                    println!("{}", rationale);
                    println!();
                }
            }

            if !req.acceptance_criteria.is_empty()
                && !req.acceptance_criteria.iter().all(|c| c.is_empty())
            {
                println!("{}", style("Acceptance Criteria:").bold());
                for criterion in &req.acceptance_criteria {
                    if !criterion.is_empty() {
                        println!("  • {}", criterion);
                    }
                }
                println!();
            }

            // Links section
            if args.with_links {
                println!("{}", style("Links:").bold());
                if !req.links.satisfied_by.is_empty() {
                    println!(
                        "  {}: {}",
                        style("Satisfied by").dim(),
                        req.links
                            .satisfied_by
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
                if !req.links.verified_by.is_empty() {
                    println!(
                        "  {}: {}",
                        style("Verified by").dim(),
                        req.links
                            .verified_by
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
                if !req.links.derives_from.is_empty() {
                    println!(
                        "  {}: {}",
                        style("Derives from").dim(),
                        req.links
                            .derives_from
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
                if !req.links.derived_by.is_empty() {
                    println!(
                        "  {}: {}",
                        style("Derived by").dim(),
                        req.links
                            .derived_by
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
                if !req.links.allocated_to.is_empty() {
                    println!(
                        "  {}: {}",
                        style("Allocated to").dim(),
                        req.links
                            .allocated_to
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
                println!();
            }

            println!("{}", style("─".repeat(60)).dim());
            println!(
                "{}: {} | {}: {} | {}: {}",
                style("Author").dim(),
                req.author,
                style("Created").dim(),
                req.created.format("%Y-%m-%d %H:%M"),
                style("Revision").dim(),
                req.revision
            );
        }
    }

    Ok(())
}

fn run_edit(args: EditArgs) -> Result<()> {
    // Resolve ID from argument or stdin
    let id = resolve_id_arg(&args.id).map_err(|e| miette::miette!("{}", e))?;
    crate::cli::entity_cmd::run_edit_generic(&id, &ENTITY_CONFIG)
}

fn run_delete(args: DeleteArgs) -> Result<()> {
    crate::cli::commands::utils::run_delete(&args.id, REQ_DIRS, args.force, false, args.quiet)
}

fn run_archive(args: ArchiveArgs) -> Result<()> {
    crate::cli::commands::utils::run_delete(&args.id, REQ_DIRS, args.force, true, args.quiet)
}

/// Run stats command using the RequirementService
fn run_stats(args: StatsArgs, global: &GlobalOpts) -> Result<()> {
    use tdt_core::services::RequirementService;

    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let cache = EntityCache::open(&project)?;
    let service = RequirementService::new(&project, &cache);

    // Get stats from service layer
    let stats = service.stats().map_err(|e| miette::miette!("{}", e))?;

    // JSON output
    if matches!(global.output, OutputFormat::Json) {
        let json = serde_json::to_string_pretty(&stats).into_diagnostic()?;
        println!("{}", json);
        return Ok(());
    }

    // YAML output
    if matches!(global.output, OutputFormat::Yaml) {
        let yaml = serde_yml::to_string(&stats).into_diagnostic()?;
        print!("{}", yaml);
        return Ok(());
    }

    // Human-readable output
    println!("{}", style("Requirement Statistics").bold().underlined());
    println!();

    // Overview
    println!(
        "{:<20} {}",
        style("Total:").bold(),
        style(stats.total).cyan()
    );
    println!(
        "{:<20} {}",
        style("Inputs:").bold(),
        style(stats.inputs).green()
    );
    println!(
        "{:<20} {}",
        style("Outputs:").bold(),
        style(stats.outputs).blue()
    );
    println!();

    // Verification status
    println!("{}", style("Verification Status:").bold());
    let verified = stats.total.saturating_sub(stats.unverified);
    let verified_pct = if stats.total > 0 {
        (verified as f64 / stats.total as f64 * 100.0) as usize
    } else {
        0
    };
    println!(
        "  {:<18} {} ({}%)",
        "Verified:",
        style(verified).green(),
        verified_pct
    );
    println!(
        "  {:<18} {}",
        "Unverified:",
        if stats.unverified > 0 {
            style(stats.unverified).yellow()
        } else {
            style(stats.unverified).green()
        }
    );
    println!(
        "  {:<18} {}",
        "Orphaned:",
        if stats.orphaned > 0 {
            style(stats.orphaned).red()
        } else {
            style(stats.orphaned).green()
        }
    );
    println!();

    // Status breakdown
    println!("{}", style("By Status:").bold());
    println!("  {:<18} {}", "Draft:", stats.by_status.draft);
    println!("  {:<18} {}", "Review:", stats.by_status.review);
    println!("  {:<18} {}", "Approved:", stats.by_status.approved);
    println!("  {:<18} {}", "Released:", stats.by_status.released);
    println!("  {:<18} {}", "Obsolete:", stats.by_status.obsolete);

    // Additional breakdowns if requested
    if args.by_level || args.by_category {
        // Load full requirements for detailed breakdown
        let reqs = service.load_all().map_err(|e| miette::miette!("{}", e))?;

        if args.by_level {
            println!();
            println!("{}", style("By Level:").bold());
            let mut level_counts: std::collections::HashMap<String, usize> =
                std::collections::HashMap::new();
            for req in &reqs {
                *level_counts.entry(req.level.to_string()).or_default() += 1;
            }
            let mut levels: Vec<_> = level_counts.into_iter().collect();
            levels.sort_by(|a, b| b.1.cmp(&a.1));
            for (level, count) in levels {
                println!("  {:<18} {}", format!("{}:", level), count);
            }
        }

        if args.by_category {
            println!();
            println!("{}", style("By Category:").bold());
            let mut cat_counts: std::collections::HashMap<String, usize> =
                std::collections::HashMap::new();
            for req in &reqs {
                let cat = req
                    .category
                    .as_ref()
                    .filter(|c| !c.is_empty())
                    .cloned()
                    .unwrap_or_else(|| "(uncategorized)".to_string());
                *cat_counts.entry(cat).or_default() += 1;
            }
            let mut cats: Vec<_> = cat_counts.into_iter().collect();
            cats.sort_by(|a, b| b.1.cmp(&a.1));
            for (cat, count) in cats {
                println!("  {:<18} {}", format!("{}:", cat), count);
            }
        }
    }

    Ok(())
}
