//! `tdt req` command - Requirement management

use clap::{Subcommand, ValueEnum};
use console::style;
use miette::{IntoDiagnostic, Result};
use std::fs;

use crate::cli::commands::utils::format_link_with_title;
use crate::cli::helpers::{escape_csv, format_short_id, resolve_id_arg, truncate_str};
use crate::cli::{GlobalOpts, OutputFormat};
use crate::core::cache::EntityCache;
use crate::core::entity::Priority;
use crate::core::identity::{EntityId, EntityPrefix};
use crate::core::links::add_inferred_link;
use crate::core::project::Project;
use crate::core::shortid::ShortIdIndex;
use crate::core::Config;
use crate::entities::requirement::{Requirement, RequirementType};
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

/// Status filter
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum StatusFilter {
    Draft,
    Review,
    Approved,
    Obsolete,
    /// All active (not obsolete)
    Active,
    /// All statuses
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

/// Columns to display in list output
#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
pub enum ListColumn {
    Id,
    Type,
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
        ListColumn::Id,
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
        || args.passing;
    let needs_full_entities = needs_full_output || needs_complex_filters;

    // Pre-load test results if we need verification status filters
    let results: Vec<crate::entities::result::Result> =
        if args.untested || args.failed || args.passing {
            load_all_results(&project)
        } else {
            Vec::new()
        };

    // Fast path: use cache directly for simple list outputs without complex filters
    if !needs_full_entities {
        let cache = EntityCache::open(&project)?;

        // Convert filters to cache-compatible format
        let status_filter = match args.status {
            StatusFilter::Draft => Some("draft"),
            StatusFilter::Review => Some("review"),
            StatusFilter::Approved => Some("approved"),
            StatusFilter::Obsolete => Some("obsolete"),
            StatusFilter::Active | StatusFilter::All => None,
        };

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
        cached_reqs.retain(|r| {
            let status_match = match args.status {
                StatusFilter::Active => r.status != "obsolete",
                _ => true,
            };
            let priority_match = match args.priority {
                PriorityFilter::Urgent => {
                    r.priority.as_deref() == Some("high")
                        || r.priority.as_deref() == Some("critical")
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
                r.status == "draft" || r.status == "review"
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
            ListColumn::Title => cached_reqs.sort_by(|a, b| a.title.cmp(&b.title)),
            ListColumn::Status => cached_reqs.sort_by(|a, b| a.status.cmp(&b.status)),
            ListColumn::Priority => {
                let priority_order = |p: Option<&str>| match p {
                    Some("critical") => 0,
                    Some("high") => 1,
                    Some("medium") => 2,
                    Some("low") => 3,
                    _ => 4,
                };
                cached_reqs.sort_by(|a, b| {
                    priority_order(a.priority.as_deref())
                        .cmp(&priority_order(b.priority.as_deref()))
                });
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
        let _ = short_ids.save(&project);

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

        // Status filter (for full entity mode and Active filter)
        let status_match = match args.status {
            StatusFilter::Draft => req.status == crate::core::entity::Status::Draft,
            StatusFilter::Review => req.status == crate::core::entity::Status::Review,
            StatusFilter::Approved => req.status == crate::core::entity::Status::Approved,
            StatusFilter::Obsolete => req.status == crate::core::entity::Status::Obsolete,
            StatusFilter::Active => req.status != crate::core::entity::Status::Obsolete,
            StatusFilter::All => true,
        };

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

        // For untested/failed/passing, check test results
        let test_ids: Vec<_> = req.links.verified_by.iter().collect();

        // Untested: has tests linked but no results for those tests
        let untested_match = if args.untested {
            if test_ids.is_empty() {
                false // No tests linked, not "untested" - it's unverified
            } else {
                // Check if any linked test has a result
                !test_ids
                    .iter()
                    .any(|tid| results.iter().any(|r| &r.test_id == *tid))
            }
        } else {
            true
        };

        // Failed: has test results with verdict=fail
        let failed_match = if args.failed {
            test_ids.iter().any(|tid| {
                results.iter().any(|r| {
                    &r.test_id == *tid && r.verdict == crate::entities::result::Verdict::Fail
                })
            })
        } else {
            true
        };

        // Passing: all linked tests have results with pass verdict
        let passing_match = if args.passing {
            if test_ids.is_empty() {
                false // No tests = can't be passing
            } else {
                // All linked tests must have at least one passing result
                test_ids.iter().all(|tid| {
                    results.iter().any(|r| {
                        &r.test_id == *tid && r.verdict == crate::entities::result::Verdict::Pass
                    })
                })
            }
        } else {
            true
        };

        type_match
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
    let _ = short_ids.save(&project); // Ignore save errors

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
        OutputFormat::Csv => {
            println!("short_id,id,type,title,status,priority,category,author,created");
            for req in &reqs {
                let short_id = short_ids
                    .get_short_id(&req.id.to_string())
                    .unwrap_or_default();
                println!(
                    "{},{},{},{},{},{},{},{},{}",
                    short_id,
                    req.id,
                    req.req_type,
                    escape_csv(&req.title),
                    req.status,
                    req.priority,
                    req.category.as_deref().unwrap_or(""),
                    req.author,
                    req.created.format("%Y-%m-%dT%H:%M:%SZ")
                );
            }
        }
        OutputFormat::Tsv => {
            // Build header based on selected columns
            let mut header_parts = vec![format!("{:<8}", style("SHORT").bold().dim())];
            for col in &args.columns {
                let header = match col {
                    ListColumn::Id => format!("{:<16}", style("ID").bold()),
                    ListColumn::Type => format!("{:<8}", style("TYPE").bold()),
                    ListColumn::Title => format!("{:<34}", style("TITLE").bold()),
                    ListColumn::Status => format!("{:<10}", style("STATUS").bold()),
                    ListColumn::Priority => format!("{:<10}", style("PRIORITY").bold()),
                    ListColumn::Category => format!("{:<16}", style("CATEGORY").bold()),
                    ListColumn::Author => format!("{:<16}", style("AUTHOR").bold()),
                    ListColumn::Created => format!("{:<12}", style("CREATED").bold()),
                    ListColumn::Tags => format!("{:<20}", style("TAGS").bold()),
                };
                header_parts.push(header);
            }
            println!("{}", header_parts.join(" "));
            println!("{}", "-".repeat(90));

            for req in &reqs {
                let short_id = short_ids
                    .get_short_id(&req.id.to_string())
                    .unwrap_or_default();
                let mut row_parts = vec![format!("{:<8}", style(&short_id).cyan())];

                for col in &args.columns {
                    let value = match col {
                        ListColumn::Id => format!("{:<16}", format_short_id(&req.id)),
                        ListColumn::Type => format!("{:<8}", req.req_type),
                        ListColumn::Title => format!("{:<34}", truncate_str(&req.title, 32)),
                        ListColumn::Status => format!("{:<10}", req.status),
                        ListColumn::Priority => format!("{:<10}", req.priority),
                        ListColumn::Category => format!(
                            "{:<16}",
                            truncate_str(req.category.as_deref().unwrap_or(""), 14)
                        ),
                        ListColumn::Author => format!("{:<16}", truncate_str(&req.author, 14)),
                        ListColumn::Created => format!("{:<12}", req.created.format("%Y-%m-%d")),
                        ListColumn::Tags => {
                            format!("{:<20}", truncate_str(&req.tags.join(", "), 18))
                        }
                    };
                    row_parts.push(value);
                }
                println!("{}", row_parts.join(" "));
            }

            println!();
            println!(
                "{} requirement(s) found. Use {} to reference by short ID.",
                style(reqs.len()).cyan(),
                style("REQ@N").cyan()
            );
        }
        OutputFormat::Id | OutputFormat::ShortId => {
            for req in &reqs {
                if format == OutputFormat::ShortId {
                    let short_id = short_ids
                        .get_short_id(&req.id.to_string())
                        .unwrap_or_default();
                    println!("{}", short_id);
                } else {
                    println!("{}", req.id);
                }
            }
        }
        OutputFormat::Md => {
            println!("| Short | ID | Type | Title | Status | Priority |");
            println!("|---|---|---|---|---|---|");
            for req in &reqs {
                let short_id = short_ids
                    .get_short_id(&req.id.to_string())
                    .unwrap_or_default();
                println!(
                    "| {} | {} | {} | {} | {} | {} |",
                    short_id,
                    format_short_id(&req.id),
                    req.req_type,
                    req.title,
                    req.status,
                    req.priority
                );
            }
        }
        OutputFormat::Auto | OutputFormat::Path => unreachable!(), // Already handled above
    }

    Ok(())
}

/// Output requirements from cached data (fast path - no YAML parsing)
fn output_cached_requirements(
    reqs: &[crate::core::CachedRequirement],
    short_ids: &ShortIdIndex,
    args: &ListArgs,
    format: OutputFormat,
) -> Result<()> {
    match format {
        OutputFormat::Csv => {
            println!("short_id,id,type,title,status,priority,category,author,created");
            for req in reqs {
                let short_id = short_ids.get_short_id(&req.id).unwrap_or_default();
                println!(
                    "{},{},{},{},{},{},{},{},{}",
                    short_id,
                    req.id,
                    req.req_type.as_deref().unwrap_or("input"),
                    escape_csv(&req.title),
                    req.status,
                    req.priority.as_deref().unwrap_or("medium"),
                    req.category.as_deref().unwrap_or(""),
                    req.author,
                    req.created.format("%Y-%m-%dT%H:%M:%SZ")
                );
            }
        }
        OutputFormat::Tsv | OutputFormat::Auto => {
            // Build header based on selected columns
            let mut header_parts = vec![format!("{:<8}", style("SHORT").bold().dim())];
            for col in &args.columns {
                let header = match col {
                    ListColumn::Id => format!("{:<16}", style("ID").bold()),
                    ListColumn::Type => format!("{:<8}", style("TYPE").bold()),
                    ListColumn::Title => format!("{:<34}", style("TITLE").bold()),
                    ListColumn::Status => format!("{:<10}", style("STATUS").bold()),
                    ListColumn::Priority => format!("{:<10}", style("PRIORITY").bold()),
                    ListColumn::Category => format!("{:<16}", style("CATEGORY").bold()),
                    ListColumn::Author => format!("{:<16}", style("AUTHOR").bold()),
                    ListColumn::Created => format!("{:<12}", style("CREATED").bold()),
                    ListColumn::Tags => format!("{:<20}", style("TAGS").bold()),
                };
                header_parts.push(header);
            }
            println!("{}", header_parts.join(" "));
            println!("{}", "-".repeat(90));

            for req in reqs {
                let short_id = short_ids.get_short_id(&req.id).unwrap_or_default();
                let mut row_parts = vec![format!("{:<8}", style(&short_id).cyan())];

                for col in &args.columns {
                    let value = match col {
                        ListColumn::Id => format!("{:<16}", truncate_str(&req.id, 14)),
                        ListColumn::Type => {
                            format!("{:<8}", req.req_type.as_deref().unwrap_or("input"))
                        }
                        ListColumn::Title => format!("{:<34}", truncate_str(&req.title, 32)),
                        ListColumn::Status => format!("{:<10}", req.status),
                        ListColumn::Priority => {
                            format!("{:<10}", req.priority.as_deref().unwrap_or("medium"))
                        }
                        ListColumn::Category => format!(
                            "{:<16}",
                            truncate_str(req.category.as_deref().unwrap_or(""), 14)
                        ),
                        ListColumn::Author => format!("{:<16}", truncate_str(&req.author, 14)),
                        ListColumn::Created => format!("{:<12}", req.created.format("%Y-%m-%d")),
                        ListColumn::Tags => {
                            format!("{:<20}", truncate_str(&req.tags.join(", "), 18))
                        }
                    };
                    row_parts.push(value);
                }
                println!("{}", row_parts.join(" "));
            }

            println!();
            println!(
                "{} requirement(s) found. Use {} to reference by short ID.",
                style(reqs.len()).cyan(),
                style("REQ@N").cyan()
            );
        }
        OutputFormat::Id | OutputFormat::ShortId => {
            for req in reqs {
                if format == OutputFormat::ShortId {
                    let short_id = short_ids.get_short_id(&req.id).unwrap_or_default();
                    println!("{}", short_id);
                } else {
                    println!("{}", req.id);
                }
            }
        }
        OutputFormat::Md => {
            println!("| Short | ID | Type | Title | Status | Priority |");
            println!("|---|---|---|---|---|---|");
            for req in reqs {
                let short_id = short_ids.get_short_id(&req.id).unwrap_or_default();
                println!(
                    "| {} | {} | {} | {} | {} | {} |",
                    short_id,
                    truncate_str(&req.id, 14),
                    req.req_type.as_deref().unwrap_or("input"),
                    req.title,
                    req.status,
                    req.priority.as_deref().unwrap_or("medium")
                );
            }
        }
        OutputFormat::Json | OutputFormat::Yaml | OutputFormat::Path => unreachable!(), // These require full entities
    }
    Ok(())
}

fn run_new(args: NewArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let config = Config::load();

    // Determine values - either from schema-driven wizard or args
    let (req_type, title, priority, category, tags, text, rationale, acceptance_criteria) =
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
            let title = args.title.unwrap_or_else(|| "New Requirement".to_string());
            let priority: Priority = args.priority.into();
            let category = args.category.unwrap_or_default();
            let tags = args.tags;

            (
                req_type,
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
    let _ = short_ids.save(&project);

    // Handle --link flags
    let mut added_links = Vec::new();
    for link_target in &args.link {
        // Resolve target ID
        let resolved_target = short_ids
            .resolve(link_target)
            .unwrap_or_else(|| link_target.clone());

        // Parse target to get prefix
        if let Ok(target_entity_id) = EntityId::parse(&resolved_target) {
            match add_inferred_link(
                &file_path,
                EntityPrefix::Req,
                &resolved_target,
                target_entity_id.prefix(),
            ) {
                Ok(link_type) => {
                    added_links.push((link_type, resolved_target.clone()));
                }
                Err(e) => {
                    eprintln!(
                        "{} Failed to add link to {}: {}",
                        style("!").yellow(),
                        link_target,
                        e
                    );
                }
            }
        } else {
            eprintln!("{} Invalid entity ID: {}", style("!").yellow(), link_target);
        }
    }

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
                "{} Created requirement {}",
                style("✓").green(),
                style(short_id.clone().unwrap_or_else(|| format_short_id(&id))).cyan()
            );
            println!("   {}", style(file_path.display()).dim());

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
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let config = Config::load();

    // Resolve ID from argument or stdin
    let id = resolve_id_arg(&args.id).map_err(|e| miette::miette!("{}", e))?;

    // Find the requirement by ID prefix match
    let req = find_requirement(&project, &id)?;

    // Get the file path
    let req_type = match req.req_type {
        RequirementType::Input => "inputs",
        RequirementType::Output => "outputs",
    };
    let file_path = project
        .root()
        .join(format!("requirements/{}/{}.tdt.yaml", req_type, req.id));

    if !file_path.exists() {
        return Err(miette::miette!("File not found: {}", file_path.display()));
    }

    println!(
        "Opening {} in {}...",
        style(format_short_id(&req.id)).cyan(),
        style(config.editor()).yellow()
    );

    config.run_editor(&file_path).into_diagnostic()?;

    Ok(())
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
