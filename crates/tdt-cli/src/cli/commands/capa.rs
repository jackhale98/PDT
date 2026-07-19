//! `tdt capa` command - Corrective/Preventive Action management

use clap::{Subcommand, ValueEnum};
use console::style;
use miette::{IntoDiagnostic, Result};

use chrono::NaiveDate;

use crate::cli::commands::utils::format_link_with_title;
use crate::cli::helpers::format_short_id;
use crate::cli::table::{CellValue, ColumnDef, TableRow};
use crate::cli::{GlobalOpts, OutputFormat};
use tdt_core::core::cache::{CachedCapa, EntityCache};
use tdt_core::core::identity::EntityPrefix;
use tdt_core::core::project::Project;
use tdt_core::core::shortid::ShortIdIndex;
use tdt_core::core::Config;
use tdt_core::entities::capa::{
    ActionStatus, Capa, CapaStatus, CapaType, EffectivenessResult, SourceType,
};
use tdt_core::schema::wizard::SchemaWizard;
use tdt_core::services::{
    CapaFilter, CapaService, CapaSortField, CommonFilter, CreateCapa, SortDirection,
};

#[derive(Subcommand, Debug)]
pub enum CapaCommands {
    /// List CAPAs with filtering
    List(ListArgs),

    /// Create a new CAPA
    New(NewArgs),

    /// Show a CAPA's details
    Show(ShowArgs),

    /// Edit a CAPA in your editor
    Edit(EditArgs),

    /// Delete a CAPA
    Delete(DeleteArgs),

    /// Archive a CAPA (soft delete)
    Archive(ArchiveArgs),

    /// Record effectiveness verification
    Verify(VerifyArgs),

    /// Advance CAPA to the next workflow stage
    Advance(CapaAdvanceArgs),
}

#[derive(clap::Args, Debug)]
pub struct CapaAdvanceArgs {
    /// CAPA ID or short ID (CAPA@N)
    pub capa: String,

    /// Target status (if omitted, advances to the next stage)
    #[arg(long, short = 's')]
    pub status: Option<CliCapaStatus>,

    /// Notes about the status change
    #[arg(long, short = 'n')]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CliCapaStatus {
    Initiation,
    Investigation,
    Implementation,
    Verification,
    Closed,
}

/// CAPA type filter
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CapaTypeFilter {
    Corrective,
    Preventive,
    All,
}

/// CAPA status filter
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CapaStatusFilter {
    Initiation,
    Investigation,
    Implementation,
    Verification,
    Closed,
    All,
}

crate::list_columns! {
    /// List column selection
    pub enum ListColumn {
        Id => "id",
        Title => "title",
        CapaType => "capa-type",
        Status => "status",
        NextDue => "next-due",
        Author => "author",
        Created => "created",
    }
}

/// Column definitions for CAPA list output
const CAPA_COLUMNS: &[ColumnDef] = &[
    ColumnDef::new("id", "ID", 17),
    ColumnDef::new("title", "TITLE", 35),
    ColumnDef::new("capa-type", "TYPE", 11),
    ColumnDef::new("status", "STATUS", 14),
    ColumnDef::new("next-due", "DUE", 12),
    ColumnDef::new("author", "AUTHOR", 16),
    ColumnDef::new("created", "CREATED", 12),
];

const CAPA_LIST_SPEC: crate::cli::entity_cmd::CachedListSpec =
    crate::cli::entity_cmd::CachedListSpec {
        columns: CAPA_COLUMNS,
        entity_name: "CAPA",
        entity_prefix: "CAPA",
        empty_message: "No CAPAs found.",
    };

#[derive(clap::Args, Debug)]
pub struct ListArgs {
    /// Filter by CAPA type
    #[arg(long, short = 't', default_value = "all")]
    pub r#type: CapaTypeFilter,

    /// Filter by CAPA status
    #[arg(long, default_value = "all")]
    pub capa_status: CapaStatusFilter,

    /// Show only overdue CAPAs (based on timeline target date)
    #[arg(long)]
    pub overdue: bool,

    /// Show only CAPAs with overdue actions (action due date < today)
    #[arg(long)]
    pub overdue_actions: bool,

    /// Show only open CAPAs (status != closed) - shortcut filter
    #[arg(long)]
    pub open: bool,

    /// Search in title and problem statement
    #[arg(long)]
    pub search: Option<String>,

    /// Filter by author
    #[arg(long)]
    pub author: Option<String>,

    /// Show only recent CAPAs (last 30 days)
    #[arg(long)]
    pub recent: bool,

    /// Columns to display
    #[arg(long, value_delimiter = ',', default_values_t = vec![
        ListColumn::CapaType,
        ListColumn::Title,
        ListColumn::Status,
        ListColumn::NextDue,
    ])]
    pub columns: Vec<ListColumn>,

    /// Sort by field
    #[arg(long, default_value = "created")]
    pub sort: ListColumn,

    /// Reverse sort order
    #[arg(long, short = 'r')]
    pub reverse: bool,

    /// Limit number of results
    #[arg(long, short = 'n')]
    pub limit: Option<usize>,

    /// Show only count
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
    /// CAPA title (required)
    #[arg(long, short = 't')]
    pub title: Option<String>,

    /// CAPA type
    #[arg(long, short = 'T', default_value = "corrective")]
    pub r#type: String,

    /// Source NCR ID (for corrective actions)
    #[arg(long)]
    pub ncr: Option<String>,

    /// Source type (ncr, audit, customer_complaint, trend_analysis, risk)
    #[arg(long, short = 's', default_value = "ncr")]
    pub source: String,

    /// Open in editor after creation
    #[arg(long, short = 'e')]
    pub edit: bool,

    /// Skip opening in editor
    #[arg(long)]
    pub no_edit: bool,

    /// Interactive mode (prompt for fields)
    #[arg(long, short = 'i')]
    pub interactive: bool,

    /// Link to another entity (auto-infers link type)
    #[arg(long, short = 'L')]
    pub link: Vec<String>,
}

#[derive(clap::Args, Debug)]
pub struct ShowArgs {
    /// CAPA ID or short ID (CAPA@N)
    pub id: String,
}

#[derive(clap::Args, Debug)]
pub struct EditArgs {
    /// CAPA ID or short ID (CAPA@N)
    pub id: String,
}

#[derive(clap::Args, Debug)]
pub struct DeleteArgs {
    /// CAPA ID or short ID (CAPA@N)
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
    /// CAPA ID or short ID (CAPA@N)
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
const CAPA_DIRS: &[&str] = tdt_core::core::project::Project::entity_search_directories(
    tdt_core::core::identity::EntityPrefix::Capa,
);

/// Entity configuration for CAPAs
const ENTITY_CONFIG: crate::cli::EntityConfig = crate::cli::EntityConfig {
    prefix: EntityPrefix::Capa,
    dirs: CAPA_DIRS,
    name: "CAPA",
    name_plural: "CAPAs",
};

/// Get the next due date for non-completed actions in a CAPA
/// Returns the earliest due date from actions that are not completed or verified
fn get_next_action_due_date(capa: &Capa) -> Option<NaiveDate> {
    capa.actions
        .iter()
        .filter(|a| a.status != ActionStatus::Completed && a.status != ActionStatus::Verified)
        .filter_map(|a| a.due_date)
        .min()
}

/// Check if a CAPA has any overdue actions
/// An overdue action is one where due_date < today and status is not completed/verified
fn has_overdue_actions(capa: &Capa) -> bool {
    let today = chrono::Local::now().date_naive();
    capa.actions.iter().any(|a| {
        a.status != ActionStatus::Completed
            && a.status != ActionStatus::Verified
            && a.due_date.map(|d| d < today).unwrap_or(false)
    })
}

/// Format a date for display, with optional overdue highlighting
fn format_due_date(due_date: Option<NaiveDate>, is_overdue: bool) -> String {
    match due_date {
        Some(date) => {
            let formatted = date.format("%Y-%m-%d").to_string();
            if is_overdue {
                format!("{}!", formatted) // Add indicator for overdue
            } else {
                formatted
            }
        }
        None => "-".to_string(),
    }
}

/// Verification result CLI option
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum VerifyResult {
    Effective,
    Partial,
    Ineffective,
}

#[derive(clap::Args, Debug)]
pub struct VerifyArgs {
    /// CAPA ID or short ID (CAPA@N)
    pub capa: String,

    /// Verification result
    #[arg(long, short = 'r')]
    pub result: VerifyResult,

    /// Evidence or notes (optional)
    #[arg(long, short = 'e')]
    pub evidence: Option<String>,

    /// Skip confirmation prompt
    #[arg(long, short = 'y')]
    pub yes: bool,
}

/// Run a CAPA subcommand
pub fn run(cmd: CapaCommands, global: &GlobalOpts) -> Result<()> {
    match cmd {
        CapaCommands::List(args) => run_list(args, global),
        CapaCommands::New(args) => run_new(args, global),
        CapaCommands::Show(args) => run_show(args, global),
        CapaCommands::Edit(args) => run_edit(args),
        CapaCommands::Delete(args) => run_delete(args),
        CapaCommands::Archive(args) => run_archive(args),
        CapaCommands::Verify(args) => run_verify(args, global),
        CapaCommands::Advance(args) => run_capa_advance(args, global),
    }
}

/// Build a CapaFilter from CLI ListArgs
fn build_capa_filter(args: &ListArgs) -> CapaFilter {
    // Map CAPA type
    let capa_type = match args.r#type {
        CapaTypeFilter::Corrective => Some(CapaType::Corrective),
        CapaTypeFilter::Preventive => Some(CapaType::Preventive),
        CapaTypeFilter::All => None,
    };

    // Map CAPA status
    let capa_status = match args.capa_status {
        CapaStatusFilter::Initiation => Some(CapaStatus::Initiation),
        CapaStatusFilter::Investigation => Some(CapaStatus::Investigation),
        CapaStatusFilter::Implementation => Some(CapaStatus::Implementation),
        CapaStatusFilter::Verification => Some(CapaStatus::Verification),
        CapaStatusFilter::Closed => Some(CapaStatus::Closed),
        CapaStatusFilter::All => None,
    };

    CapaFilter {
        common: CommonFilter {
            author: args.author.clone(),
            search: args.search.clone(),
            limit: None, // Apply limit after sorting
            ..Default::default()
        },
        capa_type,
        capa_status,
        overdue_only: args.overdue,
        open_only: args.open,
        recent_days: if args.recent { Some(30) } else { None },
        sort: build_capa_sort_field(&args.sort),
        sort_direction: if args.reverse {
            SortDirection::Descending
        } else {
            SortDirection::Ascending
        },
    }
}

/// Convert CLI sort column to CapaSortField
fn build_capa_sort_field(col: &ListColumn) -> CapaSortField {
    match col {
        ListColumn::Id => CapaSortField::Id,
        ListColumn::Title => CapaSortField::Title,
        ListColumn::CapaType => CapaSortField::CapaType,
        ListColumn::Status => CapaSortField::CapaStatus,
        ListColumn::NextDue => CapaSortField::TargetDate, // Closest available sort field
        ListColumn::Author => CapaSortField::Author,
        ListColumn::Created => CapaSortField::Created,
    }
}

/// Sort cached CAPAs according to CLI args
fn sort_cached_capas(capas: &mut Vec<CachedCapa>, args: &ListArgs) {
    match args.sort {
        ListColumn::Id => capas.sort_by(|a, b| a.id.cmp(&b.id)),
        ListColumn::Title => capas.sort_by(|a, b| a.title.cmp(&b.title)),
        ListColumn::CapaType => capas.sort_by(|a, b| {
            a.capa_type
                .as_deref()
                .unwrap_or("")
                .cmp(b.capa_type.as_deref().unwrap_or(""))
        }),
        ListColumn::Status => capas.sort_by(|a, b| {
            a.capa_status
                .as_deref()
                .unwrap_or("")
                .cmp(b.capa_status.as_deref().unwrap_or(""))
        }),
        // NextDue requires full entity loading; fallback to created when using cache
        ListColumn::NextDue => capas.sort_by(|a, b| a.created.cmp(&b.created)),
        ListColumn::Author => capas.sort_by(|a, b| a.author.cmp(&b.author)),
        ListColumn::Created => capas.sort_by(|a, b| a.created.cmp(&b.created)),
    }

    if args.reverse {
        capas.reverse();
    }

    if let Some(limit) = args.limit {
        capas.truncate(limit);
    }
}

fn run_list(args: ListArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let cache = EntityCache::open(&project).map_err(|e| miette::miette!("{}", e))?;
    let service = CapaService::new(&project, &cache);
    let mut short_ids = ShortIdIndex::load(&project);

    // Resolve linked-to filter via cache
    let allowed_ids = crate::cli::helpers::resolve_linked_to(
        &args.linked_to,
        args.via.as_deref(),
        &short_ids,
        &cache,
    );

    let format = global.list_format();

    // Build filter from CLI args
    let filter = build_capa_filter(&args);

    // Fast path: use cache when possible
    // Can't use cache for: overdue (requires timeline), overdue_actions (requires actions),
    // search (requires problem_statement), JSON/YAML (need full entity)
    let can_use_cache = !args.overdue
        && !args.overdue_actions
        && args.search.is_none()
        && !matches!(format, OutputFormat::Json | OutputFormat::Yaml);

    if can_use_cache {
        let mut cached_capas = service
            .list_cached(&filter)
            .map_err(|e| miette::miette!("{}", e))?;

        // Apply linked-to filter
        if let Some(ref ids) = allowed_ids {
            cached_capas.retain(|e| ids.contains(&e.id));
        }

        // Sort and limit
        sort_cached_capas(&mut cached_capas, &args);

        // Update short ID index
        short_ids.ensure_all(cached_capas.iter().map(|c| c.id.clone()));
        super::utils::save_short_ids(&mut short_ids, &project);

        let visible: Vec<String> = args.columns.iter().map(|c| c.to_string()).collect();
        return crate::cli::entity_cmd::output_cached_list(
            &cached_capas,
            &CAPA_LIST_SPEC,
            &visible,
            args.wrap,
            args.count,
            format,
            &project,
            |c| cached_capa_to_row(c, &short_ids),
            |c| c.file_path.as_path(),
        );
    }

    // Full entity loading path
    let mut capas = service
        .list(&filter)
        .map_err(|e| miette::miette!("{}", e))?;

    // Apply linked-to filter
    if let Some(ref ids) = allowed_ids {
        capas.retain(|e| ids.contains(&e.id.to_string()));
    }

    // Apply overdue actions filter (requires full entity data)
    if args.overdue_actions {
        capas.retain(has_overdue_actions);
    }

    // Apply limit
    if let Some(limit) = args.limit {
        capas.truncate(limit);
    }

    {
        let visible: Vec<String> = args.columns.iter().map(|c| c.to_string()).collect();
        crate::cli::entity_cmd::output_entity_list(
            &capas,
            &CAPA_LIST_SPEC,
            &visible,
            args.wrap,
            args.count,
            format,
            &project,
            &mut short_ids,
            |x| x.id.to_string(),
            capa_to_row,
        )
    }
}

fn run_new(args: NewArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let cache = EntityCache::open(&project).map_err(|e| miette::miette!("{}", e))?;
    let config = Config::load();
    let service = CapaService::new(&project, &cache);

    let title: String;
    let capa_type: CapaType;
    let source_type: SourceType;
    let problem_statement: Option<String>;
    let capa_number: Option<String>;

    // Load short IDs early for NCR reference resolution
    let mut short_ids = ShortIdIndex::load(&project);

    if args.interactive {
        let wizard = SchemaWizard::new();
        let result = wizard.run(EntityPrefix::Capa)?;

        title = result
            .get_string("title")
            .map(String::from)
            .unwrap_or_else(|| "New CAPA".to_string());
        capa_type = result
            .get_string("capa_type")
            .and_then(|s| s.parse().ok())
            .unwrap_or(CapaType::Corrective);

        // Wizard can't handle nested fields like "source.type", so prompt explicitly
        use dialoguer::{theme::ColorfulTheme, Select};
        let theme = ColorfulTheme::default();

        println!();
        println!("{} Source information", console::style("◆").cyan());

        let source_options = [
            "ncr",
            "audit",
            "customer_complaint",
            "trend_analysis",
            "risk",
        ];
        let source_selection = Select::with_theme(&theme)
            .with_prompt("Source type")
            .items(&source_options)
            .default(0)
            .interact()
            .into_diagnostic()?;
        source_type = source_options[source_selection]
            .parse()
            .unwrap_or(SourceType::Ncr);

        problem_statement = result.get_string("problem_statement").map(String::from);
        capa_number = result.get_string("capa_number").map(String::from);
    } else {
        title = args.title.unwrap_or_else(|| "New CAPA".to_string());
        capa_type = args.r#type.parse().map_err(|e| miette::miette!("{}", e))?;
        source_type = args.source.parse().map_err(|e| miette::miette!("{}", e))?;
        problem_statement = None;
        capa_number = None;
    }

    // Resolve NCR reference if provided
    let source_reference = args
        .ncr
        .as_ref()
        .map(|n| short_ids.resolve(n).unwrap_or_else(|| n.clone()));

    // Create CAPA via service
    let input = CreateCapa {
        title: title.clone(),
        capa_type,
        capa_number,
        problem_statement,
        source_type: Some(source_type),
        source_reference: source_reference.clone(),
        target_date: None,
        tags: Vec::new(),
        author: config.author(),
    };

    let capa = service
        .create(input)
        .map_err(|e| miette::miette!("{}", e))?;

    // Get file path for the created CAPA
    let file_path = project
        .root()
        .join("quality/capas")
        .join(format!("{}.tdt.yaml", capa.id));

    // Add to short ID index
    let short_id = short_ids.add(capa.id.to_string());
    super::utils::save_short_ids(&mut short_ids, &project);

    // Create bidirectional links for --ncr flag
    let mut ncr_link_added = false;
    if let Some(ref ncr_id) = source_reference {
        // Add NCR to CAPA's links.ncrs
        if crate::cli::entity_cmd::add_inferred_link_to_file(
            &file_path,
            EntityPrefix::Capa,
            ncr_id,
            EntityPrefix::Ncr,
        )
        .is_ok()
        {
            ncr_link_added = true;
        }
        // Add CAPA to NCR's links.capa (find NCR file)
        let ncr_file = project
            .root()
            .join("manufacturing/ncrs")
            .join(format!("{}.tdt.yaml", ncr_id));
        if ncr_file.exists() {
            let _ = crate::cli::entity_cmd::add_inferred_link_to_file(
                &ncr_file,
                EntityPrefix::Ncr,
                &capa.id.to_string(),
                EntityPrefix::Capa,
            );
        }
    }

    // Handle --link flags
    let added_links = crate::cli::entity_cmd::process_link_flags(
        &file_path,
        EntityPrefix::Capa,
        &args.link,
        &short_ids,
    );

    // Output based on format flag
    match global.output {
        OutputFormat::Id => {
            println!("{}", capa.id);
        }
        OutputFormat::ShortId => {
            println!(
                "{}",
                short_id
                    .clone()
                    .unwrap_or_else(|| format_short_id(&capa.id))
            );
        }
        OutputFormat::Path => {
            println!("{}", file_path.display());
        }
        _ => {
            println!(
                "{} Created CAPA {}",
                style("✓").green(),
                style(
                    short_id
                        .clone()
                        .unwrap_or_else(|| format_short_id(&capa.id))
                )
                .cyan()
            );
            println!("   {}", style(file_path.display()).dim());
            println!(
                "   {} | {}",
                style(capa.capa_type.to_string()).yellow(),
                style(&title).white()
            );
            if let Some(ref ncr_id) = source_reference {
                let ncr_display = short_ids
                    .get_short_id(ncr_id)
                    .unwrap_or_else(|| ncr_id.clone());
                println!("   Source: {}", style(&ncr_display).cyan());
                if ncr_link_added {
                    println!(
                        "   {} --[ncrs]--> {}",
                        style("→").dim(),
                        style(&ncr_display).yellow()
                    );
                }
            }

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

    // Open in editor if requested
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

    // Use CapaService to get the CAPA (cache-first lookup)
    let service = CapaService::new(&project, &cache);
    let capa = service
        .get(&resolved_id)
        .map_err(|e| miette::miette!("{}", e))?
        .ok_or_else(|| miette::miette!("No CAPA found matching '{}'", args.id))?;

    match global.output {
        OutputFormat::Yaml => {
            let yaml = serde_yml::to_string(&capa).into_diagnostic()?;
            print!("{}", yaml);
        }
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&capa).into_diagnostic()?;
            println!("{}", json);
        }
        OutputFormat::Id | OutputFormat::ShortId => {
            if global.output == OutputFormat::ShortId {
                let short_id = short_ids
                    .get_short_id(&capa.id.to_string())
                    .unwrap_or_default();
                println!("{}", short_id);
            } else {
                println!("{}", capa.id);
            }
        }
        _ => {
            // Pretty format (default)
            println!("{}", style("─".repeat(60)).dim());
            println!(
                "{}: {}",
                style("ID").bold(),
                style(&capa.id.to_string()).cyan()
            );
            println!("{}: {}", style("Title").bold(), style(&capa.title).yellow());
            println!("{}: {}", style("CAPA Type").bold(), capa.capa_type);
            println!("{}: {}", style("Status").bold(), capa.capa_status);
            println!("{}", style("─".repeat(60)).dim());

            // Problem Statement
            if let Some(ref ps) = capa.problem_statement {
                if !ps.is_empty() && !ps.starts_with('#') {
                    println!();
                    println!("{}", style("Problem Statement:").bold());
                    println!("{}", ps);
                }
            }

            // Root Cause Analysis
            if let Some(ref rca) = capa.root_cause_analysis {
                if let Some(ref rc) = rca.root_cause {
                    if !rc.is_empty() && !rc.starts_with('#') {
                        println!();
                        println!("{}: {}", style("RCA Method").bold(), rca.method);
                        println!("{}", style("Root Cause:").bold());
                        println!("{}", rc);
                    }
                }
            }

            // Actions
            if !capa.actions.is_empty() {
                println!();
                println!("{} ({}):", style("Actions").bold(), capa.actions.len());
                for action in &capa.actions {
                    let status_style = match action.status {
                        tdt_core::entities::capa::ActionStatus::Completed
                        | tdt_core::entities::capa::ActionStatus::Verified => {
                            style(action.status.to_string()).green()
                        }
                        tdt_core::entities::capa::ActionStatus::InProgress => {
                            style(action.status.to_string()).yellow()
                        }
                        _ => style(action.status.to_string()).dim(),
                    };
                    println!(
                        "  {}. {} [{}]",
                        action.action_number, action.description, status_style
                    );
                }
            }

            // Tags
            if !capa.tags.is_empty() {
                println!();
                println!("{}: {}", style("Tags").bold(), capa.tags.join(", "));
            }

            // Links
            let cache = EntityCache::open(&project).ok();
            let has_links = !capa.links.ncrs.is_empty()
                || !capa.links.risks.is_empty()
                || !capa.links.processes_modified.is_empty()
                || !capa.links.controls_added.is_empty();

            if has_links {
                println!();
                println!("{}", style("Links:").bold());

                if !capa.links.ncrs.is_empty() {
                    println!("  {}:", style("NCRs").dim());
                    for id in &capa.links.ncrs {
                        let display = format_link_with_title(&id.to_string(), &short_ids, &cache);
                        println!("    {}", style(&display).cyan());
                    }
                }

                if !capa.links.risks.is_empty() {
                    println!("  {}:", style("Risks").dim());
                    for id in &capa.links.risks {
                        let display = format_link_with_title(&id.to_string(), &short_ids, &cache);
                        println!("    {}", style(&display).cyan());
                    }
                }

                if !capa.links.processes_modified.is_empty() {
                    println!("  {}:", style("Processes Modified").dim());
                    for id in &capa.links.processes_modified {
                        let display = format_link_with_title(&id.to_string(), &short_ids, &cache);
                        println!("    {}", style(&display).cyan());
                    }
                }

                if !capa.links.controls_added.is_empty() {
                    println!("  {}:", style("Controls Added").dim());
                    for id in &capa.links.controls_added {
                        let display = format_link_with_title(&id.to_string(), &short_ids, &cache);
                        println!("    {}", style(&display).cyan());
                    }
                }
            }

            // Footer
            println!("{}", style("─".repeat(60)).dim());
            println!(
                "{}: {} | {}: {} | {}: {}",
                style("Author").dim(),
                capa.author,
                style("Created").dim(),
                capa.created.format("%Y-%m-%d %H:%M"),
                style("Revision").dim(),
                capa.entity_revision
            );
        }
    }

    Ok(())
}

fn run_edit(args: EditArgs) -> Result<()> {
    crate::cli::entity_cmd::run_edit_generic(&args.id, &ENTITY_CONFIG)
}

fn run_delete(args: DeleteArgs) -> Result<()> {
    crate::cli::commands::utils::run_delete(&args.id, CAPA_DIRS, args.force, false, args.quiet)
}

fn run_archive(args: ArchiveArgs) -> Result<()> {
    crate::cli::commands::utils::run_delete(&args.id, CAPA_DIRS, args.force, true, args.quiet)
}

/// Convert a full Capa entity to a TableRow
fn capa_to_row(capa: &Capa, short_ids: &ShortIdIndex) -> TableRow {
    let next_due = get_next_action_due_date(capa);
    let is_overdue = has_overdue_actions(capa);
    let due_display = format_due_date(next_due, is_overdue);

    TableRow::new(capa.id.to_string(), short_ids)
        .cell("id", CellValue::Id(capa.id.to_string()))
        .cell("title", CellValue::Text(capa.title.clone()))
        .cell("capa-type", CellValue::Type(capa.capa_type.to_string()))
        .cell("status", CellValue::Type(capa.capa_status.to_string()))
        .cell("next-due", CellValue::Text(due_display))
        .cell("author", CellValue::Text(capa.author.clone()))
        .cell("created", CellValue::DateTime(capa.created))
}

/// Convert a cached CAPA to a TableRow
/// Note: next-due column shows "-" for cached data since action details aren't in cache
fn cached_capa_to_row(capa: &CachedCapa, short_ids: &ShortIdIndex) -> TableRow {
    TableRow::new(capa.id.clone(), short_ids)
        .cell("id", CellValue::Id(capa.id.clone()))
        .cell("title", CellValue::Text(capa.title.clone()))
        .cell(
            "capa-type",
            CellValue::Type(capa.capa_type.clone().unwrap_or_default()),
        )
        .cell(
            "status",
            CellValue::Type(capa.capa_status.clone().unwrap_or_default()),
        )
        .cell("next-due", CellValue::Text("-".to_string())) // Requires full entity
        .cell("author", CellValue::Text(capa.author.clone()))
        .cell("created", CellValue::DateTime(capa.created))
}

fn run_verify(args: VerifyArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let cache = EntityCache::open(&project).map_err(|e| miette::miette!("{}", e))?;
    let service = CapaService::new(&project, &cache);
    let short_ids = ShortIdIndex::load(&project);

    // Resolve short ID if needed
    let resolved_id = short_ids
        .resolve(&args.capa)
        .unwrap_or_else(|| args.capa.clone());

    // Get display ID for user messages
    let display_id = short_ids
        .get_short_id(&resolved_id)
        .unwrap_or_else(|| args.capa.clone());

    // Get CAPA to validate status and show confirmation
    let capa = service
        .get(&resolved_id)
        .map_err(|e| miette::miette!("{}", e))?
        .ok_or_else(|| miette::miette!("No CAPA found matching '{}'", args.capa))?;

    // Validate status allows verification
    match capa.capa_status {
        CapaStatus::Closed => {
            return Err(miette::miette!(
                "CAPA {} is already closed and cannot be verified again",
                display_id
            ));
        }
        CapaStatus::Initiation | CapaStatus::Investigation => {
            return Err(miette::miette!(
                "CAPA {} is in {} status. Actions must be implemented before verification.",
                display_id,
                capa.capa_status
            ));
        }
        _ => {} // Implementation or Verification status is OK
    }

    // Convert CLI result to entity enum
    let effectiveness_result = match args.result {
        VerifyResult::Effective => EffectivenessResult::Effective,
        VerifyResult::Partial => EffectivenessResult::PartiallyEffective,
        VerifyResult::Ineffective => EffectivenessResult::Ineffective,
    };

    // Show current state and confirmation
    if !args.yes {
        println!();
        println!("{}", style("Verifying CAPA Effectiveness").bold().cyan());
        println!("{}", style("─".repeat(50)).dim());
        println!("CAPA: {} \"{}\"", style(&display_id).cyan(), &capa.title);
        println!("Current Status: {}", capa.capa_status);
        println!();
        println!(
            "Recording verification result: {}",
            style(format!("{:?}", args.result)).yellow()
        );
        if let Some(ref evidence) = args.evidence {
            println!("Evidence: {}", evidence);
        }

        // Auto-close if effective
        if matches!(args.result, VerifyResult::Effective) {
            println!();
            println!(
                "{}",
                style("Note: CAPA will be closed automatically (result = effective)").dim()
            );
        }
        println!();

        // Simple confirmation
        print!("Continue? [y/N] ");
        std::io::Write::flush(&mut std::io::stdout()).into_diagnostic()?;
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).into_diagnostic()?;
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Cancelled.");
            return Ok(());
        }
    }

    // Use service to verify effectiveness
    let capa = service
        .verify_effectiveness(&resolved_id, effectiveness_result, args.evidence.clone())
        .map_err(|e| miette::miette!("{}", e))?;

    // Output based on format
    match global.output {
        OutputFormat::Json => {
            let result = serde_json::json!({
                "id": capa.id.to_string(),
                "short_id": display_id,
                "verified": true,
                "result": effectiveness_result.to_string(),
                "status": capa.capa_status.to_string(),
            });
            println!(
                "{}",
                serde_json::to_string_pretty(&result).unwrap_or_default()
            );
        }
        OutputFormat::Yaml => {
            let result = serde_json::json!({
                "id": capa.id.to_string(),
                "verified": true,
                "result": effectiveness_result.to_string(),
                "status": capa.capa_status.to_string(),
            });
            println!("{}", serde_yml::to_string(&result).unwrap_or_default());
        }
        _ => {
            println!();
            println!(
                "{} {} verified as {}",
                style("✓").green(),
                style(&display_id).cyan(),
                style(format!("{:?}", args.result)).yellow()
            );
            if let Some(ref evidence) = args.evidence {
                println!("  Evidence: {}", evidence);
            }
            println!("  Status: {}", style(capa.capa_status.to_string()).white());
        }
    }

    // Sync cache after mutation
    super::utils::sync_cache(&project);

    Ok(())
}

fn run_capa_advance(args: CapaAdvanceArgs, _global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let cache = EntityCache::open(&project).map_err(|e| miette::miette!("{}", e))?;
    let service = CapaService::new(&project, &cache);
    let short_ids = ShortIdIndex::load(&project);

    let resolved_id = short_ids
        .resolve(&args.capa)
        .unwrap_or_else(|| args.capa.clone());

    let display_id = short_ids
        .get_short_id(&resolved_id)
        .unwrap_or_else(|| args.capa.clone());

    let capa = service
        .get(&resolved_id)
        .map_err(|e| miette::miette!("{}", e))?
        .ok_or_else(|| miette::miette!("No CAPA found matching '{}'", args.capa))?;

    // Determine target status
    let target_status = if let Some(cli_status) = args.status {
        match cli_status {
            CliCapaStatus::Initiation => CapaStatus::Initiation,
            CliCapaStatus::Investigation => CapaStatus::Investigation,
            CliCapaStatus::Implementation => CapaStatus::Implementation,
            CliCapaStatus::Verification => CapaStatus::Verification,
            CliCapaStatus::Closed => {
                return Err(miette::miette!(
                    "Use 'tdt capa verify --result effective' to close a CAPA after verification"
                ));
            }
        }
    } else {
        // Auto-advance to next stage
        match capa.capa_status {
            CapaStatus::Initiation => CapaStatus::Investigation,
            CapaStatus::Investigation => CapaStatus::Implementation,
            CapaStatus::Implementation => CapaStatus::Verification,
            CapaStatus::Verification => {
                return Err(miette::miette!(
                    "CAPA {} is in verification stage. Use 'tdt capa verify' to record effectiveness.",
                    display_id
                ));
            }
            CapaStatus::Closed => {
                return Err(miette::miette!("CAPA {} is already closed", display_id));
            }
        }
    };

    // Update CAPA YAML directly
    let capa_file = project
        .root()
        .join("quality/capas")
        .join(format!("{}.tdt.yaml", capa.id));
    let content = std::fs::read_to_string(&capa_file).into_diagnostic()?;
    let mut capa_yaml: serde_json::Value = serde_yml::from_str(&content).into_diagnostic()?;

    capa_yaml["capa_status"] = serde_json::json!(target_status.to_string());

    if let Some(rev) = capa_yaml.get("entity_revision").and_then(|v| v.as_u64()) {
        capa_yaml["entity_revision"] = serde_json::json!(rev + 1);
    }

    let yaml_out = serde_yml::to_string(&capa_yaml).into_diagnostic()?;
    std::fs::write(&capa_file, yaml_out).into_diagnostic()?;

    println!(
        "{} CAPA {} advanced: {} → {}",
        style("✓").green(),
        style(&display_id).cyan(),
        style(capa.capa_status.to_string()).dim(),
        style(target_status.to_string()).yellow()
    );

    // Show next steps
    match target_status {
        CapaStatus::Investigation => {
            println!("   Next: Investigate root cause, then advance to implementation");
        }
        CapaStatus::Implementation => {
            println!("   Next: Implement corrective actions, then advance to verification");
        }
        CapaStatus::Verification => {
            println!(
                "   Next: Verify effectiveness: tdt capa verify {} --result <effective|partial|ineffective>",
                display_id
            );
        }
        _ => {}
    }

    super::utils::sync_cache(&project);
    Ok(())
}
