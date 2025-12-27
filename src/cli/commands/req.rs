//! `tdt req` command - Requirement management

use clap::{Subcommand, ValueEnum};
use console::style;
use miette::{IntoDiagnostic, Result};
use std::fs;

use crate::cli::commands::utils::format_link_with_title;
use crate::cli::filters::StatusFilter;
use crate::cli::helpers::{format_short_id, resolve_id_arg};
use crate::cli::table::{CellValue, ColumnDef, TableConfig, TableFormatter, TableRow};
use crate::cli::{GlobalOpts, OutputFormat};
use crate::core::cache::EntityCache;
use crate::core::entity::Priority;
use crate::core::identity::{EntityId, EntityPrefix};
use crate::core::project::Project;
use crate::core::shortid::ShortIdIndex;
use crate::core::Config;
use crate::entities::requirement::{Level, Requirement, RequirementType};
use crate::schema::template::{TemplateContext, TemplateGenerator};
use crate::schema::wizard::SchemaWizard;

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

/// Directories where requirements are stored
const REQ_DIRS: &[&str] = &["requirements/inputs", "requirements/outputs"];

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
    }
}

fn run_list(args: ListArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;

    // Determine if we need full entity loading (for complex filters or full output)
    let output_format = match global.format {
        OutputFormat::Auto => OutputFormat::Tsv,
        f => f,
    };
    let needs_full_output = matches!(output_format, OutputFormat::Json | OutputFormat::Yaml);
    let needs_complex_filters = args.search.is_some()  // search in text field
        || args.orphans
        || args.unverified
        || args.untested
        || args.failed
        || args.passing
        || !matches!(args.level, LevelFilter::All); // level not in cache yet
    let needs_full_entities = needs_full_output || needs_complex_filters;

    // Pre-load test results if we need verification status filters
    let results: Vec<crate::entities::result::Result> =
        if args.untested || args.failed || args.passing {
            load_all_results(&project)
        } else {
            Vec::new()
        };

    // Pre-compute HashSets for O(1) lookups instead of O(n) iterations
    // This changes O(n*m) filter to O(n+m) where n=requirements, m=results
    use std::collections::HashSet;
    let tested_test_ids: HashSet<&crate::core::identity::EntityId> =
        results.iter().map(|r| &r.test_id).collect();
    let failed_test_ids: HashSet<&crate::core::identity::EntityId> = results
        .iter()
        .filter(|r| r.verdict == crate::entities::result::Verdict::Fail)
        .map(|r| &r.test_id)
        .collect();
    let passing_test_ids: HashSet<&crate::core::identity::EntityId> = results
        .iter()
        .filter(|r| r.verdict == crate::entities::result::Verdict::Pass)
        .map(|r| &r.test_id)
        .collect();

    // Fast path: use cache directly for simple list outputs without complex filters
    if !needs_full_entities {
        let cache = EntityCache::open(&project)?;

        // Convert filters to cache-compatible format
        let status_filter = crate::cli::entity_cmd::status_filter_to_str(args.status);

        let priority_filter = match args.priority {
            PriorityFilter::Low => Some("low"),
            PriorityFilter::Medium => Some("medium"),
            PriorityFilter::High => Some("high"),
            PriorityFilter::Critical => Some("critical"),
            PriorityFilter::Urgent | PriorityFilter::All => None,
        };

        let type_filter = match args.r#type {
            ReqTypeFilter::Input => Some("input"),
            ReqTypeFilter::Output => Some("output"),
            ReqTypeFilter::All => None,
        };

        // Query cache with basic filters
        let mut cached_reqs = cache.list_requirements(
            status_filter,
            priority_filter,
            type_filter,
            args.category.as_deref(),
            args.author.as_deref(),
            None, // No search
            None, // No limit yet
        );

        // Apply post-filters for Active status and Urgent priority
        use crate::core::entity::{Priority, Status};
        cached_reqs.retain(|r| {
            let status_match = match args.status {
                StatusFilter::Active => r.status != Status::Obsolete,
                _ => true,
            };
            let priority_match = match args.priority {
                PriorityFilter::Urgent => {
                    r.priority == Some(Priority::High) || r.priority == Some(Priority::Critical)
                }
                _ => true,
            };
            let tag_match = args.tag.as_ref().is_none_or(|tag| {
                r.tags
                    .iter()
                    .any(|t| t.to_lowercase() == tag.to_lowercase())
            });
            let recent_match = args.recent.is_none_or(|days| {
                let cutoff = chrono::Utc::now() - chrono::Duration::days(days as i64);
                r.created >= cutoff
            });
            let needs_review_match = if args.needs_review {
                r.status == Status::Draft || r.status == Status::Review
            } else {
                true
            };
            status_match && priority_match && tag_match && recent_match && needs_review_match
        });

        // Handle count-only mode
        if args.count {
            println!("{}", cached_reqs.len());
            return Ok(());
        }

        if cached_reqs.is_empty() {
            println!("No requirements found matching filters.");
            println!();
            println!("Create one with: {}", style("tdt req new").yellow());
            return Ok(());
        }

        // Sort
        match args.sort {
            ListColumn::Id => cached_reqs.sort_by(|a, b| a.id.cmp(&b.id)),
            ListColumn::Type => cached_reqs.sort_by(|a, b| a.req_type.cmp(&b.req_type)),
            ListColumn::Level => {} // Level not in cache, uses full entity path when filtered
            ListColumn::Title => cached_reqs.sort_by(|a, b| a.title.cmp(&b.title)),
            ListColumn::Status => cached_reqs.sort_by(|a, b| a.status.cmp(&b.status)),
            ListColumn::Priority => {
                let priority_order = |p: Option<Priority>| match p {
                    Some(Priority::Critical) => 0,
                    Some(Priority::High) => 1,
                    Some(Priority::Medium) => 2,
                    Some(Priority::Low) => 3,
                    None => 4,
                };
                cached_reqs
                    .sort_by(|a, b| priority_order(a.priority).cmp(&priority_order(b.priority)));
            }
            ListColumn::Category => cached_reqs.sort_by(|a, b| a.category.cmp(&b.category)),
            ListColumn::Author => cached_reqs.sort_by(|a, b| a.author.cmp(&b.author)),
            ListColumn::Created => cached_reqs.sort_by(|a, b| a.created.cmp(&b.created)),
            ListColumn::Tags => cached_reqs.sort_by(|a, b| a.tags.join(",").cmp(&b.tags.join(","))),
        }

        if args.reverse {
            cached_reqs.reverse();
        }

        if let Some(limit) = args.limit {
            cached_reqs.truncate(limit);
        }

        // Update short ID index
        let mut short_ids = ShortIdIndex::load(&project);
        short_ids.ensure_all(cached_reqs.iter().map(|r| r.id.clone()));
        super::utils::save_short_ids(&mut short_ids, &project);

        // Output from cached data (no YAML parsing needed!)
        return output_cached_requirements(&cached_reqs, &short_ids, &args, output_format);
    }

    // Slow path: full entity loading for complex filters or JSON/YAML output
    let mut reqs: Vec<Requirement> = Vec::new();

    for path in project.iter_entity_files(EntityPrefix::Req) {
        match crate::yaml::parse_yaml_file::<Requirement>(&path) {
            Ok(req) => reqs.push(req),
            Err(e) => {
                eprintln!(
                    "{} Failed to parse {}: {}",
                    style("!").yellow(),
                    path.display(),
                    e
                );
            }
        }
    }

    // Also check outputs directory
    let outputs_dir = project.root().join("requirements/outputs");
    if outputs_dir.exists() {
        for entry in walkdir::WalkDir::new(&outputs_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        {
            match crate::yaml::parse_yaml_file::<Requirement>(entry.path()) {
                Ok(req) => reqs.push(req),
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

    // Apply filters that need full entity data
    reqs.retain(|req| {
        // Type filter (for full entity mode)
        let type_match = match args.r#type {
            ReqTypeFilter::Input => req.req_type == RequirementType::Input,
            ReqTypeFilter::Output => req.req_type == RequirementType::Output,
            ReqTypeFilter::All => true,
        };

        // Level filter (V-model hierarchy)
        let level_match = match args.level {
            LevelFilter::Stakeholder => req.level == Level::Stakeholder,
            LevelFilter::System => req.level == Level::System,
            LevelFilter::Subsystem => req.level == Level::Subsystem,
            LevelFilter::Component => req.level == Level::Component,
            LevelFilter::Detail => req.level == Level::Detail,
            LevelFilter::All => true,
        };

        // Status filter (for full entity mode and Active filter)
        let status_match =
            crate::cli::entity_cmd::status_enum_matches_filter(&req.status, args.status);

        // Priority filter (for full entity mode and Urgent filter)
        let priority_match = match args.priority {
            PriorityFilter::Low => req.priority == Priority::Low,
            PriorityFilter::Medium => req.priority == Priority::Medium,
            PriorityFilter::High => req.priority == Priority::High,
            PriorityFilter::Critical => req.priority == Priority::Critical,
            PriorityFilter::Urgent => {
                req.priority == Priority::High || req.priority == Priority::Critical
            }
            PriorityFilter::All => true,
        };

        // Category filter (for full entity mode)
        let category_match = args.category.as_ref().is_none_or(|cat| {
            req.category
                .as_ref()
                .is_some_and(|c| c.to_lowercase() == cat.to_lowercase())
        });

        // Tag filter
        let tag_match = args.tag.as_ref().is_none_or(|tag| {
            req.tags
                .iter()
                .any(|t| t.to_lowercase() == tag.to_lowercase())
        });

        // Author filter (for full entity mode)
        let author_match = args
            .author
            .as_ref()
            .is_none_or(|author| req.author.to_lowercase().contains(&author.to_lowercase()));

        // Search filter (in title and text)
        let search_match = args.search.as_ref().is_none_or(|search| {
            let search_lower = search.to_lowercase();
            req.title.to_lowercase().contains(&search_lower)
                || req.text.to_lowercase().contains(&search_lower)
        });

        // Orphans filter (no satisfied_by or verified_by links)
        let orphans_match = if args.orphans {
            req.links.satisfied_by.is_empty() && req.links.verified_by.is_empty()
        } else {
            true
        };

        // Needs review filter (status is draft or review)
        let needs_review_match = if args.needs_review {
            req.status == crate::core::entity::Status::Draft
                || req.status == crate::core::entity::Status::Review
        } else {
            true
        };

        // Recent filter (created in last N days)
        let recent_match = args.recent.is_none_or(|days| {
            let cutoff = chrono::Utc::now() - chrono::Duration::days(days as i64);
            req.created >= cutoff
        });

        // Unverified filter (no verified_by links)
        let unverified_match = if args.unverified {
            req.links.verified_by.is_empty()
        } else {
            true
        };

        // For untested/failed/passing, use pre-computed HashSets for O(1) lookups
        let test_ids = &req.links.verified_by;

        // Untested: has tests linked but no results for those tests
        let untested_match = if args.untested {
            if test_ids.is_empty() {
                false // No tests linked, not "untested" - it's unverified
            } else {
                // Check if any linked test has a result (O(1) per test)
                !test_ids.iter().any(|tid| tested_test_ids.contains(tid))
            }
        } else {
            true
        };

        // Failed: has test results with verdict=fail (O(1) per test)
        let failed_match = if args.failed {
            test_ids.iter().any(|tid| failed_test_ids.contains(tid))
        } else {
            true
        };

        // Passing: all linked tests have results with pass verdict (O(1) per test)
        let passing_match = if args.passing {
            if test_ids.is_empty() {
                false // No tests = can't be passing
            } else {
                test_ids.iter().all(|tid| passing_test_ids.contains(tid))
            }
        } else {
            true
        };

        type_match
            && level_match
            && status_match
            && priority_match
            && category_match
            && tag_match
            && author_match
            && search_match
            && orphans_match
            && needs_review_match
            && recent_match
            && unverified_match
            && untested_match
            && failed_match
            && passing_match
    });

    // Handle count-only mode
    if args.count {
        println!("{}", reqs.len());
        return Ok(());
    }

    if reqs.is_empty() {
        match global.format {
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

    // Sort by specified column
    match args.sort {
        ListColumn::Id => reqs.sort_by(|a, b| a.id.to_string().cmp(&b.id.to_string())),
        ListColumn::Type => {
            reqs.sort_by(|a, b| a.req_type.to_string().cmp(&b.req_type.to_string()))
        }
        ListColumn::Level => {
            // Sort by V-model level (stakeholder > system > subsystem > component > detail)
            let level_order = |l: &Level| match l {
                Level::Stakeholder => 0,
                Level::System => 1,
                Level::Subsystem => 2,
                Level::Component => 3,
                Level::Detail => 4,
            };
            reqs.sort_by(|a, b| level_order(&a.level).cmp(&level_order(&b.level)));
        }
        ListColumn::Title => reqs.sort_by(|a, b| a.title.cmp(&b.title)),
        ListColumn::Status => reqs.sort_by(|a, b| a.status.to_string().cmp(&b.status.to_string())),
        ListColumn::Priority => {
            // Sort by priority level (critical > high > medium > low)
            let priority_order = |p: &Priority| match p {
                Priority::Critical => 0,
                Priority::High => 1,
                Priority::Medium => 2,
                Priority::Low => 3,
            };
            reqs.sort_by(|a, b| priority_order(&a.priority).cmp(&priority_order(&b.priority)));
        }
        ListColumn::Category => reqs.sort_by(|a, b| {
            a.category
                .as_deref()
                .unwrap_or("")
                .cmp(b.category.as_deref().unwrap_or(""))
        }),
        ListColumn::Author => reqs.sort_by(|a, b| a.author.cmp(&b.author)),
        ListColumn::Created => reqs.sort_by(|a, b| a.created.cmp(&b.created)),
        ListColumn::Tags => reqs.sort_by(|a, b| a.tags.join(",").cmp(&b.tags.join(","))),
    }

    // Reverse if requested
    if args.reverse {
        reqs.reverse();
    }

    // Apply limit
    if let Some(limit) = args.limit {
        reqs.truncate(limit);
    }

    // Update short ID index with current requirements (preserves other entity types)
    let mut short_ids = ShortIdIndex::load(&project);
    short_ids.ensure_all(reqs.iter().map(|r| r.id.to_string()));
    super::utils::save_short_ids(&mut short_ids, &project);

    // Output based on format
    let format = match global.format {
        OutputFormat::Auto => OutputFormat::Tsv, // Default to TSV for list
        f => f,
    };

    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&reqs).into_diagnostic()?;
            println!("{}", json);
        }
        OutputFormat::Yaml => {
            let yaml = serde_yml::to_string(&reqs).into_diagnostic()?;
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
            let rows: Vec<TableRow> = reqs
                .iter()
                .map(|req| requirement_to_row(req, &short_ids))
                .collect();

            // Configure table
            let config = if let Some(width) = args.wrap {
                TableConfig::with_wrap(width)
            } else {
                TableConfig::default()
            };

            let formatter =
                TableFormatter::new(REQ_COLUMNS, "requirement", "REQ").with_config(config);
            formatter.output(rows, format, &visible);
        }
        OutputFormat::Auto | OutputFormat::Path => unreachable!(), // Already handled above
    }

    Ok(())
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

/// Output requirements from cached data (fast path - no YAML parsing)
fn output_cached_requirements(
    reqs: &[crate::core::CachedRequirement],
    short_ids: &ShortIdIndex,
    args: &ListArgs,
    format: OutputFormat,
) -> Result<()> {
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
    let rows: Vec<TableRow> = reqs
        .iter()
        .map(|req| cached_requirement_to_row(req, short_ids))
        .collect();

    // Configure table
    let config = if let Some(width) = args.wrap {
        TableConfig::with_wrap(width)
    } else {
        TableConfig::default()
    };

    let formatter = TableFormatter::new(REQ_COLUMNS, "requirement", "REQ").with_config(config);
    formatter.output(rows, format, &visible);

    Ok(())
}

/// Convert a CachedRequirement to a TableRow
fn cached_requirement_to_row(
    req: &crate::core::CachedRequirement,
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

            let category = result
                .get_string("category")
                .map(String::from)
                .unwrap_or_default();

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
            let text = result.get_string("text").map(String::from);
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
            let category = args.category.unwrap_or_default();
            let tags = args.tags;

            (
                req_type,
                level,
                title,
                priority,
                category,
                tags,
                None,
                None,
                vec![],
            )
        };

    // Generate entity ID and create from template
    let id = EntityId::new(EntityPrefix::Req);
    let author = config.author();

    let generator = TemplateGenerator::new().map_err(|e| miette::miette!("{}", e))?;
    let mut ctx = TemplateContext::new(id.clone(), author)
        .with_title(&title)
        .with_req_type(req_type.to_string())
        .with_level(level.to_string())
        .with_priority(priority.to_string())
        .with_category(&category);

    if !tags.is_empty() {
        ctx = ctx.with_tags(tags);
    }

    let mut yaml_content = generator
        .generate_requirement(&ctx)
        .map_err(|e| miette::miette!("{}", e))?;

    // Apply wizard text values via string replacement (for interactive mode)
    if args.interactive {
        if let Some(ref text_value) = text {
            if !text_value.is_empty() {
                // Indent multi-line text for YAML block scalar
                let indented_text = text_value
                    .lines()
                    .map(|line| format!("  {}", line))
                    .collect::<Vec<_>>()
                    .join("\n");
                // Replace the template's placeholder text block
                yaml_content = yaml_content.replace(
                    "text: |\n  # Enter requirement text here\n  # Use clear, testable language:\n  #   - \"shall\" for mandatory requirements\n  #   - \"should\" for recommended requirements\n  #   - \"may\" for optional requirements",
                    &format!("text: |\n{}", indented_text),
                );
            }
        }
        if let Some(ref rationale_value) = rationale {
            if !rationale_value.is_empty() {
                yaml_content = yaml_content.replace(
                    "rationale: \"\"",
                    &format!("rationale: \"{}\"", rationale_value),
                );
            }
        }
        if !acceptance_criteria.is_empty() {
            let criteria_yaml = acceptance_criteria
                .iter()
                .map(|c| format!("  - \"{}\"", c))
                .collect::<Vec<_>>()
                .join("\n");
            yaml_content = yaml_content.replace(
                "acceptance_criteria:\n  - \"\"",
                &format!("acceptance_criteria:\n{}", criteria_yaml),
            );
        }
    }

    // Determine output directory based on type
    let output_dir = project.requirement_directory(&req_type.to_string());
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
        EntityPrefix::Req,
        &args.link,
        &short_ids,
    );

    // Output based on format flag
    crate::cli::entity_cmd::output_new_entity(
        &id,
        &file_path,
        short_id.clone(),
        ENTITY_CONFIG.name,
        &title,
        None,
        &added_links,
        global,
    );

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

    // Resolve ID from argument or stdin
    let id = resolve_id_arg(&args.id).map_err(|e| miette::miette!("{}", e))?;

    // Find the requirement by ID prefix match
    let req = find_requirement(&project, &id)?;

    // Output based on format (pretty is default, yaml/json explicit)
    match global.format {
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
            // Load cache and short IDs for link lookups
            let short_ids = ShortIdIndex::load(&project);
            let cache = EntityCache::open(&project).ok();

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
                            .map(|id| format_link_with_title(&id.to_string(), &short_ids, &cache))
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
                            .map(|id| format_link_with_title(&id.to_string(), &short_ids, &cache))
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
                            .map(|id| format_link_with_title(&id.to_string(), &short_ids, &cache))
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
                            .map(|id| format_link_with_title(&id.to_string(), &short_ids, &cache))
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
                            .map(|id| format_link_with_title(&id.to_string(), &short_ids, &cache))
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

/// Find a requirement by ID prefix match or short ID (@N)
fn find_requirement(project: &Project, id_query: &str) -> Result<Requirement> {
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
            if entity.prefix == "REQ" {
                // Load full requirement from file
                if let Ok(req) = crate::yaml::parse_yaml_file::<Requirement>(&entity.file_path) {
                    return Ok(req);
                }
            }
        }

        // Try prefix match via cache (e.g., "REQ-01KC" matches "REQ-01KC...")
        if lookup_id.starts_with("REQ-") {
            let filter = crate::core::EntityFilter {
                prefix: Some(crate::core::EntityPrefix::Req),
                search: Some(lookup_id.to_string()),
                ..Default::default()
            };
            let matches: Vec<_> = cache.list_entities(&filter);
            if matches.len() == 1 {
                if let Ok(req) = crate::yaml::parse_yaml_file::<Requirement>(&matches[0].file_path)
                {
                    return Ok(req);
                }
            } else if matches.len() > 1 {
                println!("{} Multiple matches found:", style("!").yellow());
                for entity in &matches {
                    // entity.id is already the full ID string, get short ID from cache
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

    let mut matches: Vec<(Requirement, std::path::PathBuf)> = Vec::new();

    // Search both inputs and outputs
    for subdir in &["inputs", "outputs"] {
        let dir = project.root().join(format!("requirements/{}", subdir));
        if !dir.exists() {
            continue;
        }

        for entry in walkdir::WalkDir::new(&dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        {
            if let Ok(req) = crate::yaml::parse_yaml_file::<Requirement>(entry.path()) {
                // Check if ID matches (prefix or full) or title fuzzy matches
                let id_str = req.id.to_string();
                let id_matches = id_str.starts_with(&resolved_query) || id_str == resolved_query;
                let title_matches = !id_query.starts_with('@')
                    && !id_query.chars().all(|c| c.is_ascii_digit())
                    && req
                        .title
                        .to_lowercase()
                        .contains(&resolved_query.to_lowercase());

                if id_matches || title_matches {
                    matches.push((req, entry.path().to_path_buf()));
                }
            }
        }
    }

    match matches.len() {
        0 => Err(miette::miette!(
            "No requirement found matching '{}'",
            id_query
        )),
        1 => Ok(matches.remove(0).0),
        _ => {
            println!("{} Multiple matches found:", style("!").yellow());
            for (req, _path) in &matches {
                println!("  {} - {}", format_short_id(&req.id), req.title);
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

fn run_delete(args: DeleteArgs) -> Result<()> {
    crate::cli::commands::utils::run_delete(&args.id, REQ_DIRS, args.force, false, args.quiet)
}

fn run_archive(args: ArchiveArgs) -> Result<()> {
    crate::cli::commands::utils::run_delete(&args.id, REQ_DIRS, args.force, true, args.quiet)
}
