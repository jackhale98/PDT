//! `tdt cmp` command - Component management

use clap::{Subcommand, ValueEnum};
use console::style;
use miette::{IntoDiagnostic, Result};
use std::fs;

use crate::cli::commands::utils::format_link_with_title;
use crate::cli::filters::StatusFilter;
use crate::cli::helpers::resolve_id_arg;
use crate::cli::table::{CellValue, ColumnDef, TableConfig, TableFormatter, TableRow};
use crate::cli::{GlobalOpts, OutputFormat};
use tdt_core::core::cache::EntityCache;
use tdt_core::core::entity::Status;
use tdt_core::core::identity::EntityPrefix;
use tdt_core::core::project::Project;
use tdt_core::core::shortid::ShortIdIndex;
use tdt_core::core::Config;
use tdt_core::entities::assembly::ManufacturingConfig;
use tdt_core::entities::component::{Component, ComponentCategory, MakeBuy};
use tdt_core::schema::wizard::SchemaWizard;
use tdt_core::services::{
    CommonFilter, ComponentFilter, ComponentService, ComponentSortField, CreateComponent,
    QuoteService, SortDirection,
};

#[derive(Subcommand, Debug)]
pub enum CmpCommands {
    /// List components with filtering
    List(ListArgs),

    /// Create a new component
    New(NewArgs),

    /// Show a component's details
    Show(ShowArgs),

    /// Edit a component in your editor
    Edit(EditArgs),

    /// Delete a component
    Delete(DeleteArgs),

    /// Archive a component (soft delete)
    Archive(ArchiveArgs),

    /// Set the selected quote for pricing
    SetQuote(SetQuoteArgs),

    /// Clear the selected quote (revert to manual unit_cost)
    ClearQuote(ClearQuoteArgs),

    /// Manage manufacturing routing for component
    #[command(subcommand)]
    Routing(RoutingCommands),
}

/// Routing subcommands for manufacturing
#[derive(Subcommand, Debug)]
pub enum RoutingCommands {
    /// Add a process to the routing
    Add(RoutingAddArgs),
    /// Remove a process from the routing
    Rm(RoutingRmArgs),
    /// List current routing
    List(RoutingListArgs),
    /// Set complete routing (replaces existing)
    Set(RoutingSetArgs),
}

#[derive(clap::Args, Debug)]
pub struct RoutingAddArgs {
    /// Component ID (CMP-xxx or short ID like CMP@1)
    pub cmp: String,
    /// Process ID to add (PROC-xxx or short ID like PROC@1)
    pub proc: String,
    /// Position in routing (0-indexed, default: append)
    #[arg(long)]
    pub position: Option<usize>,
}

#[derive(clap::Args, Debug)]
pub struct RoutingRmArgs {
    /// Component ID (CMP-xxx or short ID like CMP@1)
    pub cmp: String,
    /// Process ID to remove (PROC-xxx or short ID) or position number (1-indexed)
    pub proc_or_position: String,
}

#[derive(clap::Args, Debug)]
pub struct RoutingListArgs {
    /// Component ID (CMP-xxx or short ID like CMP@1)
    pub cmp: String,
    /// Show full PROC IDs (default shows titles)
    #[arg(long)]
    pub ids: bool,
}

#[derive(clap::Args, Debug)]
pub struct RoutingSetArgs {
    /// Component ID (CMP-xxx or short ID like CMP@1)
    pub cmp: String,
    /// Ordered list of PROC IDs (full or short IDs)
    pub procs: Vec<String>,
}

/// Make/buy filter for list command
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum MakeBuyFilter {
    Make,
    Buy,
    All,
}

/// Make/buy choice for new command
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CliMakeBuy {
    Make,
    Buy,
}

impl std::fmt::Display for CliMakeBuy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CliMakeBuy::Make => write!(f, "make"),
            CliMakeBuy::Buy => write!(f, "buy"),
        }
    }
}

/// Category filter for list command
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CategoryFilter {
    Mechanical,
    Electrical,
    Software,
    Fastener,
    Consumable,
    All,
}

/// Category choice for new command
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CliComponentCategory {
    Mechanical,
    Electrical,
    Software,
    Fastener,
    Consumable,
}

impl std::fmt::Display for CliComponentCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CliComponentCategory::Mechanical => write!(f, "mechanical"),
            CliComponentCategory::Electrical => write!(f, "electrical"),
            CliComponentCategory::Software => write!(f, "software"),
            CliComponentCategory::Fastener => write!(f, "fastener"),
            CliComponentCategory::Consumable => write!(f, "consumable"),
        }
    }
}

#[derive(clap::Args, Debug)]
pub struct ListArgs {
    /// Filter by make/buy decision
    #[arg(long, short = 'm', default_value = "all")]
    pub make_buy: MakeBuyFilter,

    /// Filter by category
    #[arg(long, short = 'c', default_value = "all")]
    pub category: CategoryFilter,

    /// Filter by status
    #[arg(long, short = 's', default_value = "all")]
    pub status: StatusFilter,

    /// Search in part number and title
    #[arg(long)]
    pub search: Option<String>,

    /// Filter by author
    #[arg(long, short = 'a')]
    pub author: Option<String>,

    /// Show only components created in the last N days
    #[arg(long)]
    pub recent: Option<u32>,

    /// Show components with lead time exceeding N days
    #[arg(long, value_name = "DAYS")]
    pub long_lead: Option<u32>,

    /// Show components with only one supplier (supply chain risk)
    #[arg(long)]
    pub single_source: bool,

    /// Show components without any quotes
    #[arg(long)]
    pub no_quote: bool,

    /// Show components with unit cost above this amount
    #[arg(long, value_name = "AMOUNT")]
    pub high_cost: Option<f64>,

    /// Filter to components in this assembly's BOM (recursive)
    #[arg(long, short = 'A')]
    pub assembly: Option<String>,

    /// Show only components without a manufacturing routing
    #[arg(long)]
    pub no_routing: bool,

    /// Columns to display (can specify multiple)
    #[arg(long, value_delimiter = ',', default_values_t = vec![
        ListColumn::PartNumber,
        ListColumn::Title,
        ListColumn::MakeBuy,
        ListColumn::Category,
        ListColumn::Status
    ])]
    pub columns: Vec<ListColumn>,

    /// Sort by field
    #[arg(long, default_value = "part-number")]
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

/// Columns to display in list output
#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
pub enum ListColumn {
    Id,
    PartNumber,
    Revision,
    Title,
    MakeBuy,
    Category,
    Status,
    Author,
    Created,
}

impl std::fmt::Display for ListColumn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ListColumn::Id => write!(f, "id"),
            ListColumn::PartNumber => write!(f, "part-number"),
            ListColumn::Revision => write!(f, "revision"),
            ListColumn::Title => write!(f, "title"),
            ListColumn::MakeBuy => write!(f, "make-buy"),
            ListColumn::Category => write!(f, "category"),
            ListColumn::Status => write!(f, "status"),
            ListColumn::Author => write!(f, "author"),
            ListColumn::Created => write!(f, "created"),
        }
    }
}

/// Column definitions for component list output
const CMP_COLUMNS: &[ColumnDef] = &[
    ColumnDef::new("id", "ID", 17),
    ColumnDef::new("part-number", "PART #", 18),
    ColumnDef::new("revision", "REV", 8),
    ColumnDef::new("title", "TITLE", 30),
    ColumnDef::new("make-buy", "M/B", 6),
    ColumnDef::new("category", "CATEGORY", 12),
    ColumnDef::new("status", "STATUS", 10),
    ColumnDef::new("author", "AUTHOR", 16),
    ColumnDef::new("created", "CREATED", 12),
];

#[derive(clap::Args, Debug)]
pub struct NewArgs {
    /// Part number (required)
    #[arg(long, short = 'p')]
    pub part_number: Option<String>,

    /// Title/description
    #[arg(long, short = 't')]
    pub title: Option<String>,

    /// Make or buy decision
    #[arg(long, short = 'm', default_value = "buy")]
    pub make_buy: CliMakeBuy,

    /// Component category
    #[arg(long, short = 'c', default_value = "mechanical")]
    pub category: CliComponentCategory,

    /// Part revision
    #[arg(long)]
    pub revision: Option<String>,

    /// Material specification
    #[arg(long)]
    pub material: Option<String>,

    /// Open in editor after creation
    #[arg(long, short = 'e')]
    pub edit: bool,

    /// Skip opening in editor
    #[arg(long, short = 'n')]
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
    /// Component ID or short ID (CMP@N), or pipe via stdin
    pub id: Option<String>,

    /// Show linked entities too
    #[arg(long)]
    pub with_links: bool,
}

#[derive(clap::Args, Debug)]
pub struct EditArgs {
    /// Component ID or short ID (CMP@N), or pipe via stdin
    pub id: Option<String>,
}

#[derive(clap::Args, Debug)]
pub struct DeleteArgs {
    /// Component ID or short ID (CMP@N)
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
    /// Component ID or short ID (CMP@N)
    pub id: String,

    /// Force archive even if other entities reference this one
    #[arg(long)]
    pub force: bool,

    /// Suppress output
    #[arg(long, short = 'q')]
    pub quiet: bool,
}

/// Directories where components are stored
const COMPONENT_DIRS: &[&str] = &["bom/components"];

/// Entity configuration for components
const ENTITY_CONFIG: crate::cli::EntityConfig = crate::cli::EntityConfig {
    prefix: EntityPrefix::Cmp,
    dirs: COMPONENT_DIRS,
    name: "component",
    name_plural: "components",
};

#[derive(clap::Args, Debug)]
pub struct SetQuoteArgs {
    /// Component ID or short ID (CMP@N)
    pub component: String,

    /// Quote ID or short ID (QUOT@N) to use for pricing
    pub quote: String,
}

#[derive(clap::Args, Debug)]
pub struct ClearQuoteArgs {
    /// Component ID or short ID (CMP@N)
    pub component: String,
}

/// Run a component subcommand
pub fn run(cmd: CmpCommands, global: &GlobalOpts) -> Result<()> {
    match cmd {
        CmpCommands::List(args) => run_list(args, global),
        CmpCommands::New(args) => run_new(args, global),
        CmpCommands::Show(args) => run_show(args, global),
        CmpCommands::Edit(args) => run_edit(args),
        CmpCommands::Delete(args) => run_delete(args),
        CmpCommands::Archive(args) => run_archive(args),
        CmpCommands::SetQuote(args) => run_set_quote(args),
        CmpCommands::ClearQuote(args) => run_clear_quote(args),
        CmpCommands::Routing(cmd) => run_routing(cmd),
    }
}

fn run_list(args: ListArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let cache = EntityCache::open(&project).map_err(|e| miette::miette!("{}", e))?;
    let service = ComponentService::new(&project, &cache);

    // Resolve linked-to filter via cache
    let short_ids = ShortIdIndex::load(&project);
    let allowed_ids = crate::cli::helpers::resolve_linked_to(
        &args.linked_to,
        args.via.as_deref(),
        &short_ids,
        &cache,
    );

    // Build filter and sort from CLI args
    let filter = build_cmp_filter(&args);
    let (sort_field, sort_dir) = build_cmp_sort(&args);

    // Determine output format
    let output_format = match global.output {
        OutputFormat::Auto => OutputFormat::Tsv,
        f => f,
    };

    // Determine if we need full entities (complex filters or full output)
    let needs_full_output = matches!(output_format, OutputFormat::Json | OutputFormat::Yaml);
    let needs_special_filters = args.long_lead.is_some()
        || args.single_source
        || args.no_quote
        || args.high_cost.is_some()
        || args.assembly.is_some()
        || args.no_routing;
    let needs_full_entities = needs_full_output || needs_special_filters;

    if needs_full_entities {
        // Load full entities via service
        let result = service
            .list(&filter, sort_field, sort_dir)
            .map_err(|e| miette::miette!("{}", e))?;

        let mut components = result.items;

        // Apply linked-to filter
        if let Some(ref ids) = allowed_ids {
            components.retain(|e| ids.contains(&e.id.to_string()));
        }

        // Apply special filters that require full entity data
        if needs_special_filters {
            // Use cache for quote lookup instead of walking directories
            let components_with_quotes: std::collections::HashSet<String> = if args.no_quote {
                cache
                    .list_quotes(None, None, None, None, None, None, None)
                    .into_iter()
                    .filter_map(|q| q.component_id)
                    .collect()
            } else {
                std::collections::HashSet::new()
            };

            components.retain(|c| {
                // Long lead time filter
                let long_lead_match = args.long_lead.is_none_or(|threshold| {
                    c.suppliers
                        .iter()
                        .any(|s| s.lead_time_days.is_some_and(|days| days > threshold))
                });

                // Single source filter
                let single_source_match = !args.single_source || c.suppliers.len() == 1;

                // No quote filter - uses cached quote data
                let no_quote_match = if args.no_quote {
                    !components_with_quotes.contains(&c.id.to_string())
                } else {
                    true
                };

                // High cost filter
                let high_cost_match = args
                    .high_cost
                    .is_none_or(|threshold| c.unit_cost.is_some_and(|cost| cost > threshold));

                // No routing filter - components without manufacturing routing
                let no_routing_match = !args.no_routing
                    || c.manufacturing.is_none()
                    || c.manufacturing
                        .as_ref()
                        .is_some_and(|m| m.routing.is_empty());

                long_lead_match
                    && single_source_match
                    && no_quote_match
                    && high_cost_match
                    && no_routing_match
            });

            // Filter by parent assembly (components in BOM only)
            // Uses cache for fast BOM traversal instead of walking directories
            if let Some(ref parent_asm) = args.assembly {
                let short_ids_tmp = ShortIdIndex::load(&project);
                let parent_id = short_ids_tmp
                    .resolve(parent_asm)
                    .unwrap_or_else(|| parent_asm.clone());

                // Use cache's recursive BOM lookup
                let bom_component_ids = cache.get_bom_components(&parent_id);

                if bom_component_ids.is_empty() {
                    // Check if assembly exists at all
                    if cache.get_entity(&parent_id).is_none() {
                        eprintln!(
                            "Warning: Assembly '{}' not found, showing all components",
                            parent_asm
                        );
                    }
                    // If assembly exists but has no components, the filter will show nothing (correct behavior)
                }

                components.retain(|c| bom_component_ids.contains(&c.id.to_string()));
            }
        }

        // Handle count-only mode
        if args.count {
            println!("{}", components.len());
            return Ok(());
        }

        if components.is_empty() {
            match global.output {
                OutputFormat::Json => println!("[]"),
                OutputFormat::Yaml => println!("[]"),
                _ => println!("No components found."),
            }
            return Ok(());
        }

        // Update short ID index
        let mut short_ids = ShortIdIndex::load(&project);
        short_ids.ensure_all(components.iter().map(|c| c.id.to_string()));
        super::utils::save_short_ids(&mut short_ids, &project);

        // Output based on format
        output_components(&components, &short_ids, &args, output_format)
    } else {
        // Fast path: use cache for simple list outputs
        let mut cached_cmps = cache.list_components(
            crate::cli::entity_cmd::status_filter_to_str(args.status),
            match args.make_buy {
                MakeBuyFilter::Make => Some("make"),
                MakeBuyFilter::Buy => Some("buy"),
                MakeBuyFilter::All => None,
            },
            match args.category {
                CategoryFilter::Mechanical => Some("mechanical"),
                CategoryFilter::Electrical => Some("electrical"),
                CategoryFilter::Software => Some("software"),
                CategoryFilter::Fastener => Some("fastener"),
                CategoryFilter::Consumable => Some("consumable"),
                CategoryFilter::All => None,
            },
            args.author.as_deref(),
            args.search.as_deref(),
            None,
        );

        // Apply linked-to filter
        if let Some(ref ids) = allowed_ids {
            cached_cmps.retain(|e| ids.contains(&e.id));
        }

        // Apply additional filters
        cached_cmps.retain(|c| {
            args.recent.is_none_or(|days| {
                let cutoff = chrono::Utc::now() - chrono::Duration::days(days as i64);
                c.created >= cutoff
            })
        });

        // Sort cached results
        sort_cached_components(&mut cached_cmps, &args);

        // Handle count-only mode
        if args.count {
            println!("{}", cached_cmps.len());
            return Ok(());
        }

        if cached_cmps.is_empty() {
            println!("No components found.");
            return Ok(());
        }

        // Update short ID index
        let mut short_ids = ShortIdIndex::load(&project);
        short_ids.ensure_all(cached_cmps.iter().map(|c| c.id.clone()));
        super::utils::save_short_ids(&mut short_ids, &project);

        output_cached_components(&cached_cmps, &short_ids, &args, output_format, &project)
    }
}

/// Build ComponentFilter from CLI args
fn build_cmp_filter(args: &ListArgs) -> ComponentFilter {
    // Convert status filter
    let status = match args.status {
        StatusFilter::Draft => Some(vec![Status::Draft]),
        StatusFilter::Review => Some(vec![Status::Review]),
        StatusFilter::Approved => Some(vec![Status::Approved]),
        StatusFilter::Released => Some(vec![Status::Released]),
        StatusFilter::Obsolete => Some(vec![Status::Obsolete]),
        StatusFilter::Active | StatusFilter::All => None,
    };

    // Convert make/buy filter
    let make_buy = match args.make_buy {
        MakeBuyFilter::Make => Some(MakeBuy::Make),
        MakeBuyFilter::Buy => Some(MakeBuy::Buy),
        MakeBuyFilter::All => None,
    };

    // Convert category filter
    let category = match args.category {
        CategoryFilter::Mechanical => Some(ComponentCategory::Mechanical),
        CategoryFilter::Electrical => Some(ComponentCategory::Electrical),
        CategoryFilter::Software => Some(ComponentCategory::Software),
        CategoryFilter::Fastener => Some(ComponentCategory::Fastener),
        CategoryFilter::Consumable => Some(ComponentCategory::Consumable),
        CategoryFilter::All => None,
    };

    ComponentFilter {
        common: CommonFilter {
            status,
            author: args.author.clone(),
            search: args.search.clone(),
            recent_days: args.recent,
            limit: args.limit,
            ..Default::default()
        },
        make_buy,
        category,
        ..Default::default()
    }
}

/// Build sort field and direction from CLI args
fn build_cmp_sort(args: &ListArgs) -> (ComponentSortField, SortDirection) {
    let field = match args.sort {
        ListColumn::Id => ComponentSortField::Id,
        ListColumn::PartNumber => ComponentSortField::PartNumber,
        ListColumn::Revision => ComponentSortField::Title, // No Revision sort, fallback to Title
        ListColumn::Title => ComponentSortField::Title,
        ListColumn::MakeBuy => ComponentSortField::MakeBuy,
        ListColumn::Category => ComponentSortField::Category,
        ListColumn::Status => ComponentSortField::Status,
        ListColumn::Author => ComponentSortField::Author,
        ListColumn::Created => ComponentSortField::Created,
    };

    let direction = if args.reverse {
        match field {
            ComponentSortField::Created => SortDirection::Ascending,
            _ => SortDirection::Descending,
        }
    } else {
        match field {
            ComponentSortField::Created => SortDirection::Descending,
            _ => SortDirection::Ascending,
        }
    };

    (field, direction)
}

/// Sort cached components based on CLI args
fn sort_cached_components(cmps: &mut Vec<tdt_core::core::CachedComponent>, args: &ListArgs) {
    match args.sort {
        ListColumn::Id => cmps.sort_by(|a, b| a.id.cmp(&b.id)),
        ListColumn::PartNumber => cmps.sort_by(|a, b| a.part_number.cmp(&b.part_number)),
        ListColumn::Revision => cmps.sort_by(|a, b| a.revision.cmp(&b.revision)),
        ListColumn::Title => cmps.sort_by(|a, b| a.title.cmp(&b.title)),
        ListColumn::MakeBuy => cmps.sort_by(|a, b| a.make_buy.cmp(&b.make_buy)),
        ListColumn::Category => cmps.sort_by(|a, b| a.category.cmp(&b.category)),
        ListColumn::Status => cmps.sort_by(|a, b| a.status.cmp(&b.status)),
        ListColumn::Author => cmps.sort_by(|a, b| a.author.cmp(&b.author)),
        ListColumn::Created => cmps.sort_by(|a, b| a.created.cmp(&b.created)),
    }

    if args.reverse {
        cmps.reverse();
    }

    if let Some(limit) = args.limit {
        cmps.truncate(limit);
    }
}

/// Output full components
fn output_components(
    components: &[Component],
    short_ids: &ShortIdIndex,
    args: &ListArgs,
    format: OutputFormat,
) -> Result<()> {
    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(components).into_diagnostic()?;
            println!("{}", json);
        }
        OutputFormat::Yaml => {
            let yaml = serde_yml::to_string(components).into_diagnostic()?;
            print!("{}", yaml);
        }
        OutputFormat::Tsv
        | OutputFormat::Csv
        | OutputFormat::Md
        | OutputFormat::Id
        | OutputFormat::ShortId
        | OutputFormat::Table
        | OutputFormat::Dot
        | OutputFormat::Tree => {
            let mut visible: Vec<&str> = args
                .columns
                .iter()
                .map(|c| c.to_string().leak() as &str)
                .collect();
            if args.show_id && !visible.contains(&"id") {
                visible.insert(0, "id");
            }

            let rows: Vec<TableRow> = components
                .iter()
                .map(|cmp| component_to_row(cmp, short_ids))
                .collect();

            let config = if let Some(width) = args.wrap {
                TableConfig::with_wrap(width)
            } else {
                TableConfig::default()
            };

            let formatter =
                TableFormatter::new(CMP_COLUMNS, "component", "CMP").with_config(config);
            formatter.output(rows, format, &visible);
        }
        OutputFormat::Auto | OutputFormat::Path => {
            eprintln!("error: -o path is not supported for this view; use -o id to get entity IDs");
            std::process::exit(2);
        }
    }

    Ok(())
}

/// Convert a Component to a TableRow
fn component_to_row(cmp: &Component, short_ids: &ShortIdIndex) -> TableRow {
    TableRow::new(cmp.id.to_string(), short_ids)
        .cell("id", CellValue::Id(cmp.id.to_string()))
        .cell("part-number", CellValue::Text(cmp.part_number.clone()))
        .cell(
            "revision",
            CellValue::Text(cmp.revision.clone().unwrap_or_else(|| "-".to_string())),
        )
        .cell("title", CellValue::Text(cmp.title.clone()))
        .cell("make-buy", CellValue::Type(cmp.make_buy.to_string()))
        .cell("category", CellValue::Type(cmp.category.to_string()))
        .cell("status", CellValue::Status(cmp.status))
        .cell("author", CellValue::Text(cmp.author.clone()))
        .cell("created", CellValue::Date(cmp.created))
}

/// Output components from cached data (fast path - no YAML parsing)
fn output_cached_components(
    cmps: &[tdt_core::core::CachedComponent],
    short_ids: &ShortIdIndex,
    args: &ListArgs,
    format: OutputFormat,
    project: &Project,
) -> Result<()> {
    // The table formatter below can't render file paths; handle -o path here.
    if format == OutputFormat::Path {
        for e in cmps {
            let path = if e.file_path.is_absolute() {
                e.file_path.clone()
            } else {
                project.root().join(&e.file_path)
            };
            println!("{}", path.display());
        }
        return Ok(());
    }

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
    let rows: Vec<TableRow> = cmps
        .iter()
        .map(|cmp| cached_component_to_row(cmp, short_ids))
        .collect();

    // Configure table
    let config = if let Some(width) = args.wrap {
        TableConfig::with_wrap(width)
    } else {
        TableConfig::default()
    };

    let formatter = TableFormatter::new(CMP_COLUMNS, "component", "CMP").with_config(config);
    formatter.output(rows, format, &visible);

    Ok(())
}

/// Convert a CachedComponent to a TableRow
fn cached_component_to_row(
    cmp: &tdt_core::core::CachedComponent,
    short_ids: &ShortIdIndex,
) -> TableRow {
    TableRow::new(cmp.id.clone(), short_ids)
        .cell("id", CellValue::Id(cmp.id.clone()))
        .cell(
            "part-number",
            CellValue::Text(cmp.part_number.clone().unwrap_or_default()),
        )
        .cell(
            "revision",
            CellValue::Text(cmp.revision.clone().unwrap_or_else(|| "-".to_string())),
        )
        .cell("title", CellValue::Text(cmp.title.clone()))
        .cell(
            "make-buy",
            CellValue::Type(cmp.make_buy.clone().unwrap_or_else(|| "buy".to_string())),
        )
        .cell(
            "category",
            CellValue::Type(cmp.category.clone().unwrap_or_default()),
        )
        .cell("status", CellValue::Status(cmp.status))
        .cell("author", CellValue::Text(cmp.author.clone()))
        .cell("created", CellValue::Date(cmp.created))
}

fn run_new(args: NewArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let cache = EntityCache::open(&project).map_err(|e| miette::miette!("{}", e))?;
    let service = ComponentService::new(&project, &cache);
    let config = Config::load();

    let part_number: String;
    let title: String;
    let make_buy: MakeBuy;
    let category: ComponentCategory;
    let material: Option<String>;
    let description: Option<String>;
    let mass_kg: Option<f64>;
    let unit_cost: Option<f64>;
    let revision: Option<String>;

    if args.interactive {
        // Use schema-driven wizard
        let wizard = SchemaWizard::new();
        let result = wizard.run(EntityPrefix::Cmp)?;

        part_number = result
            .get_string("part_number")
            .map(String::from)
            .unwrap_or_else(|| "NEW-PART".to_string());

        title = result
            .get_string("title")
            .map(String::from)
            .unwrap_or_else(|| "New Component".to_string());

        make_buy = result
            .get_string("make_buy")
            .and_then(|s| s.parse().ok())
            .unwrap_or(MakeBuy::Buy);

        category = result
            .get_string("category")
            .and_then(|s| s.parse().ok())
            .unwrap_or(ComponentCategory::Mechanical);

        // Extract additional fields from wizard
        revision = result.get_string("revision").map(String::from);
        material = result.get_string("material").map(String::from);
        description = result.get_string("description").map(String::from);

        // Try to get mass_kg and unit_cost from wizard first
        let wizard_mass = result.values.get("mass_kg").and_then(|v| {
            v.as_f64()
                .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
        });
        let wizard_cost = result.values.get("unit_cost").and_then(|v| {
            v.as_f64()
                .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
        });

        // If wizard didn't collect these (may be skipped), prompt explicitly
        use dialoguer::{theme::ColorfulTheme, Input};
        let theme = ColorfulTheme::default();

        println!();
        println!("{} Physical properties", console::style("◆").cyan());

        mass_kg = if wizard_mass.is_some() {
            wizard_mass
        } else {
            let mass_input: String = Input::with_theme(&theme)
                .with_prompt("Mass (kg) - leave empty to skip")
                .allow_empty(true)
                .interact_text()
                .into_diagnostic()?;
            if mass_input.is_empty() {
                None
            } else {
                mass_input.parse().ok()
            }
        };

        unit_cost = if wizard_cost.is_some() {
            wizard_cost
        } else {
            let cost_input: String = Input::with_theme(&theme)
                .with_prompt("Unit cost - leave empty to skip")
                .allow_empty(true)
                .interact_text()
                .into_diagnostic()?;
            if cost_input.is_empty() {
                None
            } else {
                cost_input.parse().ok()
            }
        };
    } else {
        part_number = args
            .part_number
            .ok_or_else(|| miette::miette!("Part number is required (use --part-number or -p)"))?;
        title = args
            .title
            .ok_or_else(|| miette::miette!("Title is required (use --title or -t)"))?;
        make_buy = args.make_buy.to_string().parse().unwrap_or(MakeBuy::Buy);
        category = args
            .category
            .to_string()
            .parse()
            .unwrap_or(ComponentCategory::Mechanical);
        revision = args.revision.clone();
        material = args.material.clone();
        description = None;
        mass_kg = None;
        unit_cost = None;
    }

    // Create component using service
    let input = CreateComponent {
        part_number: part_number.clone(),
        title: title.clone(),
        author: config.author(),
        make_buy,
        category,
        description,
        revision,
        material,
        mass_kg,
        unit_cost,
        ..Default::default()
    };

    let component = service
        .create(input)
        .map_err(|e| miette::miette!("{}", e))?;

    let file_path = project
        .root()
        .join("bom/components")
        .join(format!("{}.tdt.yaml", component.id));

    // Add to short ID index
    let mut short_ids = ShortIdIndex::load(&project);
    let short_id = short_ids.add(component.id.to_string());
    super::utils::save_short_ids(&mut short_ids, &project);

    // Handle --link flags
    let added_links = crate::cli::entity_cmd::process_link_flags(
        &file_path,
        EntityPrefix::Cmp,
        &args.link,
        &short_ids,
    );

    // Output based on format flag
    let extra_info = format!(
        "Part: {} | {}",
        style(&component.part_number).yellow(),
        style(&component.title).white()
    );
    crate::cli::entity_cmd::output_new_entity(
        &component.id,
        &file_path,
        short_id.clone(),
        ENTITY_CONFIG.name,
        &component.title,
        Some(&extra_info),
        &added_links,
        global,
    );

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

    // Resolve ID from argument or stdin
    let id = resolve_id_arg(&args.id).map_err(|e| miette::miette!("{}", e))?;

    // Resolve short ID if needed
    let short_ids = ShortIdIndex::load(&project);
    let resolved_id = short_ids.resolve(&id).unwrap_or_else(|| id.clone());

    // Use ComponentService to get the component (cache-first lookup)
    let service = ComponentService::new(&project, &cache);
    let cmp = service
        .get(&resolved_id)
        .map_err(|e| miette::miette!("{}", e))?
        .ok_or_else(|| miette::miette!("No component found matching '{}'", id))?;

    match global.output {
        OutputFormat::Yaml => {
            let yaml = serde_yml::to_string(&cmp).into_diagnostic()?;
            print!("{}", yaml);
        }
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&cmp).into_diagnostic()?;
            println!("{}", json);
        }
        OutputFormat::Id | OutputFormat::ShortId => {
            if global.output == OutputFormat::ShortId {
                let short_id = short_ids
                    .get_short_id(&cmp.id.to_string())
                    .unwrap_or_default();
                println!("{}", short_id);
            } else {
                println!("{}", cmp.id);
            }
        }
        _ => {
            // Pretty format (default)
            println!("{}", style("─".repeat(60)).dim());
            println!(
                "{}: {}",
                style("ID").bold(),
                style(&cmp.id.to_string()).cyan()
            );
            println!("{}: {}", style("Title").bold(), style(&cmp.title).yellow());
            if !cmp.part_number.is_empty() {
                println!("{}: {}", style("Part Number").bold(), cmp.part_number);
            }
            if let Some(ref rev) = cmp.revision {
                if !rev.is_empty() {
                    println!("{}: {}", style("Revision").bold(), rev);
                }
            }
            println!("{}: {}", style("Status").bold(), cmp.status);
            println!(
                "{}: {}",
                style("Make/Buy").bold(),
                match cmp.make_buy {
                    tdt_core::entities::component::MakeBuy::Make => style("MAKE").green(),
                    tdt_core::entities::component::MakeBuy::Buy => style("BUY").blue(),
                }
            );
            println!("{}: {}", style("Category").bold(), cmp.category);
            println!("{}", style("─".repeat(60)).dim());

            // Material and physical
            if let Some(ref mat) = cmp.material {
                if !mat.is_empty() {
                    println!();
                    println!("{}", style("Physical Properties:").bold());
                    println!("  {}: {}", style("Material").dim(), mat);
                    if let Some(mass) = cmp.mass_kg {
                        println!("  {}: {} kg", style("Mass").dim(), mass);
                    }
                    if let Some(cost) = cmp.unit_cost {
                        println!("  {}: ${:.2}", style("Unit Cost").dim(), cost);
                    }
                }
            }

            // Suppliers
            if !cmp.suppliers.is_empty() && cmp.suppliers.iter().any(|s| !s.name.is_empty()) {
                println!();
                println!("{}", style("Suppliers:").bold());
                for sup in &cmp.suppliers {
                    if !sup.name.is_empty() {
                        print!("  • {}", sup.name);
                        if let Some(ref pn) = sup.supplier_pn {
                            if !pn.is_empty() {
                                print!(" ({})", pn);
                            }
                        }
                        if let Some(lead) = sup.lead_time_days {
                            print!(" - {} day lead", lead);
                        }
                        if let Some(cost) = sup.unit_cost {
                            print!(" @ ${:.2}", cost);
                        }
                        println!();
                    }
                }
            }

            // Documents
            if !cmp.documents.is_empty() && cmp.documents.iter().any(|d| !d.path.is_empty()) {
                println!();
                println!("{}", style("Documents:").bold());
                for doc in &cmp.documents {
                    if !doc.path.is_empty() {
                        println!("  • [{}] {}", doc.doc_type, doc.path);
                    }
                }
            }

            // Tags
            if !cmp.tags.is_empty() {
                println!();
                println!("{}: {}", style("Tags").bold(), cmp.tags.join(", "));
            }

            // Used in assemblies
            if let Ok(cache) = EntityCache::open(&project) {
                let containing_asms = cache.get_links_to_of_type(&cmp.id.to_string(), "contains");
                if !containing_asms.is_empty() {
                    println!();
                    println!("{}", style("Used In Assemblies:").bold());
                    for asm_id in &containing_asms {
                        let short_id = short_ids
                            .get_short_id(asm_id)
                            .unwrap_or_else(|| asm_id.clone());
                        // Look up assembly part number and title from cache
                        let entity = cache.get_entity(asm_id);
                        let asm_info = cache.get_assembly_info(asm_id);
                        let part_number = asm_info.and_then(|(pn, _)| pn);
                        let title = entity.as_ref().map(|e| e.title.as_str()).unwrap_or("");

                        match (part_number.as_deref(), title) {
                            (Some(pn), t) if !pn.is_empty() && !t.is_empty() => {
                                println!("  • {} ({}) {}", style(&short_id).cyan(), pn, t);
                            }
                            (Some(pn), _) if !pn.is_empty() => {
                                println!("  • {} ({})", style(&short_id).cyan(), pn);
                            }
                            (_, t) if !t.is_empty() => {
                                println!("  • {} ({})", style(&short_id).cyan(), t);
                            }
                            _ => {
                                println!("  • {}", style(&short_id).cyan());
                            }
                        }
                    }
                }
            }

            // Description
            if let Some(ref desc) = cmp.description {
                if !desc.is_empty() && !desc.starts_with('#') {
                    println!();
                    println!("{}", style("Description:").bold());
                    println!("{}", desc);
                }
            }

            // Links (only with --with-links flag)
            if args.with_links {
                let cache = EntityCache::open(&project).ok();
                let has_links = !cmp.links.related_to.is_empty()
                    || !cmp.links.replaces.is_empty()
                    || !cmp.links.replaced_by.is_empty()
                    || !cmp.links.interchangeable_with.is_empty();

                if has_links {
                    println!();
                    println!("{}", style("Links:").bold());

                    if !cmp.links.related_to.is_empty() {
                        println!(
                            "  {}: {}",
                            style("Related to").dim(),
                            cmp.links
                                .related_to
                                .iter()
                                .map(|id| format_link_with_title(
                                    &id.to_string(),
                                    &short_ids,
                                    &cache
                                ))
                                .collect::<Vec<_>>()
                                .join(", ")
                        );
                    }
                    if !cmp.links.replaces.is_empty() {
                        println!(
                            "  {}: {}",
                            style("Replaces").dim(),
                            cmp.links
                                .replaces
                                .iter()
                                .map(|id| format_link_with_title(
                                    &id.to_string(),
                                    &short_ids,
                                    &cache
                                ))
                                .collect::<Vec<_>>()
                                .join(", ")
                        );
                    }
                    if !cmp.links.replaced_by.is_empty() {
                        println!(
                            "  {}: {}",
                            style("Replaced by").dim(),
                            cmp.links
                                .replaced_by
                                .iter()
                                .map(|id| format_link_with_title(
                                    &id.to_string(),
                                    &short_ids,
                                    &cache
                                ))
                                .collect::<Vec<_>>()
                                .join(", ")
                        );
                    }
                    if !cmp.links.interchangeable_with.is_empty() {
                        println!(
                            "  {}: {}",
                            style("Interchangeable with").dim(),
                            cmp.links
                                .interchangeable_with
                                .iter()
                                .map(|id| format_link_with_title(
                                    &id.to_string(),
                                    &short_ids,
                                    &cache
                                ))
                                .collect::<Vec<_>>()
                                .join(", ")
                        );
                    }
                }
            }

            // Footer
            println!("{}", style("─".repeat(60)).dim());
            println!(
                "{}: {} | {}: {} | {}: {}",
                style("Author").dim(),
                cmp.author,
                style("Created").dim(),
                cmp.created.format("%Y-%m-%d %H:%M"),
                style("Revision").dim(),
                cmp.entity_revision
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
    crate::cli::commands::utils::run_delete(&args.id, COMPONENT_DIRS, args.force, false, args.quiet)
}

fn run_archive(args: ArchiveArgs) -> Result<()> {
    crate::cli::commands::utils::run_delete(&args.id, COMPONENT_DIRS, args.force, true, args.quiet)
}

fn run_set_quote(args: SetQuoteArgs) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let cache = EntityCache::open(&project).map_err(|e| miette::miette!("{}", e))?;
    let short_ids = ShortIdIndex::load(&project);

    // Resolve IDs
    let cmp_id = short_ids
        .resolve(&args.component)
        .unwrap_or_else(|| args.component.clone());
    let quote_id = short_ids
        .resolve(&args.quote)
        .unwrap_or_else(|| args.quote.clone());

    // Use services
    let cmp_service = ComponentService::new(&project, &cache);
    let quote_service = QuoteService::new(&project, &cache);

    // Load quote via service
    let quote = quote_service
        .get_required(&quote_id)
        .map_err(|e| miette::miette!("{}", e))?;

    // Verify quote is for this component
    if let Some(ref quoted_cmp) = quote.component {
        if !quoted_cmp.contains(&cmp_id) && !cmp_id.contains(quoted_cmp) {
            return Err(miette::miette!(
                "Quote '{}' is for component '{}', not '{}'",
                args.quote,
                quoted_cmp,
                args.component
            ));
        }
    } else {
        return Err(miette::miette!(
            "Quote '{}' is not linked to a component",
            args.quote
        ));
    }

    // Get old quote before update for display
    let old_component = cmp_service
        .get_required(&cmp_id)
        .map_err(|e| miette::miette!("{}", e))?;
    let old_quote = old_component.selected_quote.clone();

    // Set quote via service
    let component = cmp_service
        .set_quote(&cmp_id, &quote.id.to_string())
        .map_err(|e| miette::miette!("{}", e))?;

    // Get display names
    let cmp_display = short_ids
        .get_short_id(&component.id.to_string())
        .unwrap_or_else(|| args.component.clone());
    let quote_display = short_ids
        .get_short_id(&quote.id.to_string())
        .unwrap_or_else(|| args.quote.clone());

    println!(
        "{} Set quote for {} to {}",
        style("✓").green(),
        style(&cmp_display).cyan(),
        style(&quote_display).yellow()
    );

    // Show price info
    if let Some(price) = quote.price_for_qty(1) {
        println!("   Base price: ${:.2}", price);
    }
    if !quote.price_breaks.is_empty() {
        println!("   Price breaks:");
        for pb in &quote.price_breaks {
            let lead = pb
                .lead_time_days
                .map(|d| format!(" ({}d)", d))
                .unwrap_or_default();
            println!(
                "     {} qty {} → ${:.2}{}",
                style("•").dim(),
                pb.min_qty,
                pb.unit_price,
                lead
            );
        }
    }

    if let Some(old) = old_quote {
        let old_display = short_ids.get_short_id(&old).unwrap_or(old);
        println!("   (Previously: {})", style(old_display).dim());
    }

    // Sync cache after quote selection change
    super::utils::sync_cache(&project);

    Ok(())
}

fn run_clear_quote(args: ClearQuoteArgs) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let cache = EntityCache::open(&project).map_err(|e| miette::miette!("{}", e))?;
    let short_ids = ShortIdIndex::load(&project);

    // Resolve component ID
    let cmp_id = short_ids
        .resolve(&args.component)
        .unwrap_or_else(|| args.component.clone());

    // Use service
    let service = ComponentService::new(&project, &cache);

    // Get component to check current state
    let old_component = service
        .get_required(&cmp_id)
        .map_err(|e| miette::miette!("{}", e))?;

    let cmp_display = short_ids
        .get_short_id(&old_component.id.to_string())
        .unwrap_or_else(|| args.component.clone());

    if old_component.selected_quote.is_none() {
        println!(
            "{} {} has no selected quote",
            style("•").dim(),
            style(&cmp_display).cyan()
        );
        return Ok(());
    }

    let old_quote = old_component.selected_quote.clone();

    // Clear quote via service
    let component = service
        .clear_quote(&cmp_id)
        .map_err(|e| miette::miette!("{}", e))?;

    println!(
        "{} Cleared quote for {}",
        style("✓").green(),
        style(&cmp_display).cyan()
    );

    if let Some(old) = old_quote {
        let old_display = short_ids.get_short_id(&old).unwrap_or(old);
        println!("   (Was: {})", style(old_display).dim());
    }

    if let Some(cost) = component.unit_cost {
        println!("   Will use manual unit_cost: ${:.2}", cost);
    } else {
        println!(
            "   {}",
            style("Note: No unit_cost set. BOM costing will show $0.00").yellow()
        );
    }

    // Sync cache after quote selection change
    super::utils::sync_cache(&project);

    Ok(())
}

// ============================================================================
// Routing subcommands
// ============================================================================

fn run_routing(cmd: RoutingCommands) -> Result<()> {
    match cmd {
        RoutingCommands::Add(args) => run_routing_add(args),
        RoutingCommands::Rm(args) => run_routing_rm(args),
        RoutingCommands::List(args) => run_routing_list(args),
        RoutingCommands::Set(args) => run_routing_set(args),
    }
}

/// Find a component file by ID and return the loaded component and path
fn find_component_file(project: &Project, id: &str) -> Result<(Component, std::path::PathBuf)> {
    let cmp_dir = project.root().join("bom/components");

    if !cmp_dir.exists() {
        return Err(miette::miette!("No components directory found"));
    }

    // Search for the component file
    for entry in walkdir::WalkDir::new(&cmp_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
    {
        if let Ok(content) = fs::read_to_string(entry.path()) {
            if let Ok(cmp) = serde_yml::from_str::<Component>(&content) {
                if cmp.id.to_string() == id {
                    return Ok((cmp, entry.path().to_path_buf()));
                }
            }
        }
    }

    Err(miette::miette!("Component {} not found", id))
}

fn run_routing_add(args: RoutingAddArgs) -> Result<()> {
    use tdt_core::entities::process::Process;

    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let short_ids = ShortIdIndex::load(&project);

    // Resolve component ID
    let cmp_id = short_ids
        .resolve(&args.cmp)
        .unwrap_or_else(|| args.cmp.clone());

    // Resolve process ID (short ID -> full ID for storage)
    let proc_id = short_ids
        .resolve(&args.proc)
        .unwrap_or_else(|| args.proc.clone());

    // Find and load the component
    let (mut component, path) = find_component_file(&project, &cmp_id)?;

    // Verify process exists
    let proc_dir = project.root().join("manufacturing/processes");
    let mut proc_title = proc_id.clone();
    if proc_dir.exists() {
        for entry in fs::read_dir(&proc_dir).into_diagnostic()? {
            let entry = entry.into_diagnostic()?;
            let entry_path = entry.path();
            if entry_path.extension().is_some_and(|e| e == "yaml") {
                if let Ok(content) = fs::read_to_string(&entry_path) {
                    if let Ok(proc) = serde_yml::from_str::<Process>(&content) {
                        if proc.id.to_string() == proc_id {
                            proc_title = proc.title.clone();
                            break;
                        }
                    }
                }
            }
        }
    }

    // Initialize manufacturing config if not present
    if component.manufacturing.is_none() {
        component.manufacturing = Some(ManufacturingConfig::default());
    }

    // Add at position or append
    let (position, new_len) = {
        let mfg = component.manufacturing.as_mut().unwrap();

        // Check if already in routing
        if mfg.routing.contains(&proc_id) {
            return Err(miette::miette!(
                "Process {} is already in the routing",
                args.proc
            ));
        }
        if let Some(pos) = args.position {
            if pos > mfg.routing.len() {
                return Err(miette::miette!(
                    "Position {} is out of range (routing has {} items)",
                    pos,
                    mfg.routing.len()
                ));
            }
            mfg.routing.insert(pos, proc_id.clone());
            (pos, mfg.routing.len())
        } else {
            let pos = mfg.routing.len();
            mfg.routing.push(proc_id.clone());
            (pos, mfg.routing.len())
        }
    };

    // Save
    let yaml = serde_yml::to_string(&component).into_diagnostic()?;
    fs::write(&path, yaml).into_diagnostic()?;

    println!(
        "{} Added {} to routing at position {}",
        style("✓").green(),
        style(&proc_title).cyan(),
        position + 1
    );
    println!(
        "   Routing now has {} step{}",
        new_len,
        if new_len == 1 { "" } else { "s" }
    );

    // Sync cache after routing change
    super::utils::sync_cache(&project);

    Ok(())
}

fn run_routing_rm(args: RoutingRmArgs) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let short_ids = ShortIdIndex::load(&project);

    // Resolve component ID
    let cmp_id = short_ids
        .resolve(&args.cmp)
        .unwrap_or_else(|| args.cmp.clone());

    // Find and load the component
    let (mut component, path) = find_component_file(&project, &cmp_id)?;

    // Remove from routing and capture results
    let (removed, new_len) = {
        let mfg = component.manufacturing.as_mut().ok_or_else(|| {
            miette::miette!(
                "Component {} has no manufacturing routing configured",
                args.cmp
            )
        })?;

        if mfg.routing.is_empty() {
            return Err(miette::miette!("Routing is empty"));
        }

        // Try to parse as position number first
        let removed = if let Ok(pos) = args.proc_or_position.parse::<usize>() {
            if pos == 0 || pos > mfg.routing.len() {
                return Err(miette::miette!(
                    "Position {} is out of range (routing has {} items)",
                    pos,
                    mfg.routing.len()
                ));
            }
            mfg.routing.remove(pos - 1)
        } else {
            // Treat as process ID
            let proc_id = short_ids
                .resolve(&args.proc_or_position)
                .unwrap_or_else(|| args.proc_or_position.clone());

            let pos = mfg
                .routing
                .iter()
                .position(|id| id == &proc_id)
                .ok_or_else(|| {
                    miette::miette!("Process {} not found in routing", args.proc_or_position)
                })?;
            mfg.routing.remove(pos)
        };
        (removed, mfg.routing.len())
    };

    // Save
    let yaml = serde_yml::to_string(&component).into_diagnostic()?;
    fs::write(&path, yaml).into_diagnostic()?;

    let removed_short = short_ids
        .get_short_id(&removed)
        .unwrap_or_else(|| removed.clone());
    println!(
        "{} Removed {} from routing",
        style("✓").green(),
        style(&removed_short).cyan()
    );
    println!(
        "   Routing now has {} step{}",
        new_len,
        if new_len == 1 { "" } else { "s" }
    );

    // Sync cache after routing change
    super::utils::sync_cache(&project);

    Ok(())
}

fn run_routing_list(args: RoutingListArgs) -> Result<()> {
    use tdt_core::entities::process::Process;

    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let short_ids = ShortIdIndex::load(&project);

    // Resolve component ID
    let cmp_id = short_ids
        .resolve(&args.cmp)
        .unwrap_or_else(|| args.cmp.clone());

    // Find and load the component
    let (component, _path) = find_component_file(&project, &cmp_id)?;

    let mfg = component.manufacturing.as_ref();
    let routing = mfg.map(|m| m.routing.as_slice()).unwrap_or(&[]);

    if routing.is_empty() {
        println!("No routing configured for component {}", args.cmp);
        return Ok(());
    }

    // Load process titles if not using --ids
    let mut proc_map: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    if !args.ids {
        let proc_dir = project.root().join("manufacturing/processes");
        if proc_dir.exists() {
            for entry in walkdir::WalkDir::new(&proc_dir)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
                .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
            {
                if let Ok(content) = fs::read_to_string(entry.path()) {
                    if let Ok(proc) = serde_yml::from_str::<Process>(&content) {
                        proc_map.insert(proc.id.to_string(), proc.title.clone());
                    }
                }
            }
        }
    }

    println!(
        "Manufacturing routing for {} ({} step{}):",
        style(&args.cmp).cyan(),
        routing.len(),
        if routing.len() == 1 { "" } else { "s" }
    );

    for (i, proc_id) in routing.iter().enumerate() {
        let display = if args.ids {
            proc_id.clone()
        } else {
            let title = proc_map.get(proc_id).cloned().unwrap_or_default();
            let short = short_ids
                .get_short_id(proc_id)
                .unwrap_or_else(|| proc_id.clone());
            if title.is_empty() {
                short
            } else {
                format!("{} ({})", title, short)
            }
        };
        println!("  {}. {}", i + 1, display);
    }

    Ok(())
}

fn run_routing_set(args: RoutingSetArgs) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let short_ids = ShortIdIndex::load(&project);

    // Resolve component ID
    let cmp_id = short_ids
        .resolve(&args.cmp)
        .unwrap_or_else(|| args.cmp.clone());

    // Find and load the component
    let (mut component, path) = find_component_file(&project, &cmp_id)?;

    // Resolve all process IDs
    let proc_ids: Vec<String> = args
        .procs
        .iter()
        .map(|p| short_ids.resolve(p).unwrap_or_else(|| p.clone()))
        .collect();

    // Initialize or update manufacturing config
    if component.manufacturing.is_none() {
        component.manufacturing = Some(ManufacturingConfig::default());
    }

    let old_len = component
        .manufacturing
        .as_ref()
        .map(|m| m.routing.len())
        .unwrap_or(0);

    component.manufacturing.as_mut().unwrap().routing = proc_ids.clone();

    // Save
    let yaml = serde_yml::to_string(&component).into_diagnostic()?;
    fs::write(&path, yaml).into_diagnostic()?;

    println!(
        "{} Set routing for {} ({} step{})",
        style("✓").green(),
        style(&args.cmp).cyan(),
        proc_ids.len(),
        if proc_ids.len() == 1 { "" } else { "s" }
    );
    if old_len > 0 {
        println!(
            "   (Replaced {} previous step{})",
            old_len,
            if old_len == 1 { "" } else { "s" }
        );
    }

    // Sync cache after routing change
    super::utils::sync_cache(&project);

    Ok(())
}
