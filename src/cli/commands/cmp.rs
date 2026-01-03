//! `tdt cmp` command - Component management

use clap::{Subcommand, ValueEnum};
use console::style;
use miette::{IntoDiagnostic, Result};
use std::collections::HashSet;
use std::fs;

use crate::cli::commands::utils::format_link_with_title;
use crate::cli::filters::StatusFilter;
use crate::cli::helpers::resolve_id_arg;
use crate::cli::table::{CellValue, ColumnDef, TableConfig, TableFormatter, TableRow};
use crate::cli::{GlobalOpts, OutputFormat};
use crate::core::cache::EntityCache;
use crate::core::identity::{EntityId, EntityPrefix};
use crate::core::project::Project;
use crate::core::shortid::ShortIdIndex;
use crate::core::Config;
use crate::entities::assembly::{Assembly, ManufacturingConfig};
use crate::entities::component::{Component, ComponentCategory, MakeBuy};
use crate::schema::template::{TemplateContext, TemplateGenerator};
use crate::schema::wizard::SchemaWizard;

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
    ColumnDef::new("part-number", "PART #", 12),
    ColumnDef::new("revision", "REV", 8),
    ColumnDef::new("title", "TITLE", 30),
    ColumnDef::new("make-buy", "M/B", 6),
    ColumnDef::new("category", "CATEGORY", 12),
    ColumnDef::new("status", "STATUS", 10),
    ColumnDef::new("author", "AUTHOR", 16),
    ColumnDef::new("created", "CREATED", 12),
];

/// Sort field (reuses ListColumn for consistency)
pub type SortField = ListColumn;

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

    // Determine if we need full entity loading (for complex filters or full output)
    let output_format = match global.output {
        OutputFormat::Auto => OutputFormat::Tsv,
        f => f,
    };
    let needs_full_output = matches!(output_format, OutputFormat::Json | OutputFormat::Yaml);
    let needs_complex_filters = args.search.is_some()  // search in description
        || args.long_lead.is_some()  // needs supplier data
        || args.single_source        // needs supplier data
        || args.no_quote             // needs quote data
        || args.high_cost.is_some()  // needs unit_cost
        || args.assembly.is_some(); // needs assembly BOM traversal
    let needs_full_entities = needs_full_output || needs_complex_filters;

    // Pre-load quotes if needed for no_quote filter
    let quotes: Vec<crate::entities::quote::Quote> = if args.no_quote {
        load_all_quotes(&project)
    } else {
        Vec::new()
    };

    // Fast path: use cache directly for simple list outputs
    if !needs_full_entities {
        let cache = EntityCache::open(&project)?;

        // Convert filters to cache-compatible format
        let status_filter = crate::cli::entity_cmd::status_filter_to_str(args.status);

        let make_buy_filter = match args.make_buy {
            MakeBuyFilter::Make => Some("make"),
            MakeBuyFilter::Buy => Some("buy"),
            MakeBuyFilter::All => None,
        };

        let category_filter = match args.category {
            CategoryFilter::Mechanical => Some("mechanical"),
            CategoryFilter::Electrical => Some("electrical"),
            CategoryFilter::Software => Some("software"),
            CategoryFilter::Fastener => Some("fastener"),
            CategoryFilter::Consumable => Some("consumable"),
            CategoryFilter::All => None,
        };

        // Query cache with basic filters
        let mut cached_cmps = cache.list_components(
            status_filter,
            make_buy_filter,
            category_filter,
            args.author.as_deref(),
            None, // No search
            None, // No limit yet
        );

        // Apply post-filters
        cached_cmps.retain(|c| {
            args.recent.is_none_or(|days| {
                let cutoff = chrono::Utc::now() - chrono::Duration::days(days as i64);
                c.created >= cutoff
            })
        });

        // Handle count-only mode
        if args.count {
            println!("{}", cached_cmps.len());
            return Ok(());
        }

        if cached_cmps.is_empty() {
            println!("No components found.");
            return Ok(());
        }

        // Sort
        match args.sort {
            ListColumn::Id => cached_cmps.sort_by(|a, b| a.id.cmp(&b.id)),
            ListColumn::PartNumber => cached_cmps.sort_by(|a, b| a.part_number.cmp(&b.part_number)),
            ListColumn::Revision => cached_cmps.sort_by(|a, b| a.revision.cmp(&b.revision)),
            ListColumn::Title => cached_cmps.sort_by(|a, b| a.title.cmp(&b.title)),
            ListColumn::MakeBuy => cached_cmps.sort_by(|a, b| a.make_buy.cmp(&b.make_buy)),
            ListColumn::Category => cached_cmps.sort_by(|a, b| a.category.cmp(&b.category)),
            ListColumn::Status => cached_cmps.sort_by(|a, b| a.status.cmp(&b.status)),
            ListColumn::Author => cached_cmps.sort_by(|a, b| a.author.cmp(&b.author)),
            ListColumn::Created => cached_cmps.sort_by(|a, b| a.created.cmp(&b.created)),
        }

        if args.reverse {
            cached_cmps.reverse();
        }

        if let Some(limit) = args.limit {
            cached_cmps.truncate(limit);
        }

        // Update short ID index
        let mut short_ids = ShortIdIndex::load(&project);
        short_ids.ensure_all(cached_cmps.iter().map(|c| c.id.clone()));
        super::utils::save_short_ids(&mut short_ids, &project);

        // Output from cached data
        return output_cached_components(&cached_cmps, &short_ids, &args, output_format);
    }

    // Slow path: full entity loading
    let cmp_dir = project.root().join("bom/components");

    if !cmp_dir.exists() {
        if args.count {
            println!("0");
        } else {
            println!("No components found.");
        }
        return Ok(());
    }

    // Load and parse all components
    let mut components: Vec<Component> = Vec::new();

    for entry in fs::read_dir(&cmp_dir).into_diagnostic()? {
        let entry = entry.into_diagnostic()?;
        let path = entry.path();

        if path.extension().is_some_and(|e| e == "yaml") {
            let content = fs::read_to_string(&path).into_diagnostic()?;
            if let Ok(cmp) = serde_yml::from_str::<Component>(&content) {
                components.push(cmp);
            }
        }
    }

    // Apply filters
    let components: Vec<Component> = components
        .into_iter()
        .filter(|c| match args.make_buy {
            MakeBuyFilter::Make => c.make_buy == MakeBuy::Make,
            MakeBuyFilter::Buy => c.make_buy == MakeBuy::Buy,
            MakeBuyFilter::All => true,
        })
        .filter(|c| match args.category {
            CategoryFilter::Mechanical => c.category == ComponentCategory::Mechanical,
            CategoryFilter::Electrical => c.category == ComponentCategory::Electrical,
            CategoryFilter::Software => c.category == ComponentCategory::Software,
            CategoryFilter::Fastener => c.category == ComponentCategory::Fastener,
            CategoryFilter::Consumable => c.category == ComponentCategory::Consumable,
            CategoryFilter::All => true,
        })
        .filter(|c| crate::cli::entity_cmd::status_enum_matches_filter(&c.status, args.status))
        .filter(|c| {
            if let Some(ref search) = args.search {
                let search_lower = search.to_lowercase();
                c.part_number.to_lowercase().contains(&search_lower)
                    || c.title.to_lowercase().contains(&search_lower)
                    || c.description
                        .as_ref()
                        .is_some_and(|d| d.to_lowercase().contains(&search_lower))
            } else {
                true
            }
        })
        .filter(|c| {
            args.author
                .as_ref()
                .is_none_or(|author| c.author.to_lowercase().contains(&author.to_lowercase()))
        })
        .filter(|c| {
            args.recent.is_none_or(|days| {
                let cutoff = chrono::Utc::now() - chrono::Duration::days(days as i64);
                c.created >= cutoff
            })
        })
        // Long lead time filter - check if any supplier has lead_time_days > threshold
        .filter(|c| {
            args.long_lead.is_none_or(|threshold| {
                c.suppliers
                    .iter()
                    .any(|s| s.lead_time_days.is_some_and(|days| days > threshold))
            })
        })
        // Single source filter - exactly one supplier
        .filter(|c| {
            if args.single_source {
                c.suppliers.len() == 1
            } else {
                true
            }
        })
        // No quote filter - component not referenced by any quote
        .filter(|c| {
            if args.no_quote {
                let cid_str = c.id.to_string();
                !quotes
                    .iter()
                    .any(|q| q.component.as_ref() == Some(&cid_str))
            } else {
                true
            }
        })
        // High cost filter
        .filter(|c| {
            args.high_cost
                .is_none_or(|threshold| c.unit_cost.is_some_and(|cost| cost > threshold))
        })
        .collect();

    // Filter by parent assembly (components in BOM only)
    let components = if let Some(ref parent_asm) = args.assembly {
        let short_ids_tmp = ShortIdIndex::load(&project);
        let parent_id = short_ids_tmp
            .resolve(parent_asm)
            .unwrap_or_else(|| parent_asm.clone());

        // Load all assemblies for recursive BOM traversal
        let assemblies = load_all_assemblies(&project);
        let assembly_map: std::collections::HashMap<String, &Assembly> =
            assemblies.iter().map(|a| (a.id.to_string(), a)).collect();

        // Find the parent assembly
        if let Some(parent) = assembly_map.get(&parent_id) {
            // Collect all component IDs in the BOM recursively
            let mut bom_component_ids: HashSet<String> = HashSet::new();
            let mut visited: HashSet<String> = HashSet::new();
            visited.insert(parent_id.clone());

            collect_bom_component_ids(parent, &assembly_map, &mut bom_component_ids, &mut visited);

            // Filter to only components in the BOM
            components
                .into_iter()
                .filter(|c| bom_component_ids.contains(&c.id.to_string()))
                .collect()
        } else {
            eprintln!(
                "Warning: Assembly '{}' not found, showing all components",
                parent_asm
            );
            components
        }
    } else {
        components
    };

    // Sort
    let mut components = components;
    match args.sort {
        ListColumn::Id => components.sort_by(|a, b| a.id.to_string().cmp(&b.id.to_string())),
        ListColumn::PartNumber => components.sort_by(|a, b| a.part_number.cmp(&b.part_number)),
        ListColumn::Revision => components.sort_by(|a, b| a.revision.cmp(&b.revision)),
        ListColumn::Title => components.sort_by(|a, b| a.title.cmp(&b.title)),
        ListColumn::MakeBuy => {
            components.sort_by(|a, b| format!("{:?}", a.make_buy).cmp(&format!("{:?}", b.make_buy)))
        }
        ListColumn::Category => {
            components.sort_by(|a, b| format!("{:?}", a.category).cmp(&format!("{:?}", b.category)))
        }
        ListColumn::Status => {
            components.sort_by(|a, b| format!("{:?}", a.status).cmp(&format!("{:?}", b.status)))
        }
        ListColumn::Author => components.sort_by(|a, b| a.author.cmp(&b.author)),
        ListColumn::Created => components.sort_by(|a, b| a.created.cmp(&b.created)),
    }

    if args.reverse {
        components.reverse();
    }

    // Apply limit
    if let Some(limit) = args.limit {
        components.truncate(limit);
    }

    // Count only
    if args.count {
        println!("{}", components.len());
        return Ok(());
    }

    // No results
    if components.is_empty() {
        println!("No components found.");
        return Ok(());
    }

    // Update short ID index
    let mut short_ids = ShortIdIndex::load(&project);
    short_ids.ensure_all(components.iter().map(|c| c.id.to_string()));
    super::utils::save_short_ids(&mut short_ids, &project);

    // Output based on format
    let format = match global.output {
        OutputFormat::Auto => OutputFormat::Tsv,
        f => f,
    };

    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&components).into_diagnostic()?;
            println!("{}", json);
        }
        OutputFormat::Yaml => {
            let yaml = serde_yml::to_string(&components).into_diagnostic()?;
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
            let rows: Vec<TableRow> = components
                .iter()
                .map(|cmp| component_to_row(cmp, &short_ids))
                .collect();

            // Configure table
            let config = if let Some(width) = args.wrap {
                TableConfig::with_wrap(width)
            } else {
                TableConfig::default()
            };

            let formatter =
                TableFormatter::new(CMP_COLUMNS, "component", "CMP").with_config(config);
            formatter.output(rows, format, &visible);
        }
        OutputFormat::Auto | OutputFormat::Path => unreachable!(),
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
    cmps: &[crate::core::CachedComponent],
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
    cmp: &crate::core::CachedComponent,
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
    let config = Config::load();

    let part_number: String;
    let title: String;
    let make_buy: String;
    let category: String;
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
            .map(String::from)
            .unwrap_or_else(|| "buy".to_string());

        category = result
            .get_string("category")
            .map(String::from)
            .unwrap_or_else(|| "mechanical".to_string());

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
        make_buy = args.make_buy.to_string();
        category = args.category.to_string();
        revision = args.revision.clone();
        material = args.material.clone();
        description = None;
        mass_kg = None;
        unit_cost = None;
    }

    // Generate ID
    let id = EntityId::new(EntityPrefix::Cmp);

    // Generate template
    let generator = TemplateGenerator::new().map_err(|e| miette::miette!("{}", e))?;
    let ctx = TemplateContext::new(id.clone(), config.author())
        .with_title(&title)
        .with_part_number(&part_number)
        .with_make_buy(&make_buy)
        .with_component_category(&category);

    let ctx = if let Some(ref rev) = revision {
        ctx.with_part_revision(rev)
    } else {
        ctx
    };

    // Use material from wizard or args
    let ctx = if let Some(ref mat) = material {
        ctx.with_material(mat)
    } else {
        ctx
    };

    let yaml_content = generator
        .generate_component(&ctx)
        .map_err(|e| miette::miette!("{}", e))?;

    // Parse template and apply wizard values (more robust than string replacement)
    let mut component: Component = serde_yml::from_str(&yaml_content).into_diagnostic()?;
    if args.interactive {
        if let Some(ref desc) = description {
            if !desc.is_empty() {
                component.description = Some(desc.clone());
            }
        }
        component.mass_kg = mass_kg;
        component.unit_cost = unit_cost;
    }
    let yaml_content = serde_yml::to_string(&component).into_diagnostic()?;

    // Write file
    let output_dir = project.root().join("bom/components");
    if !output_dir.exists() {
        fs::create_dir_all(&output_dir).into_diagnostic()?;
    }

    let file_path = output_dir.join(format!("{}.tdt.yaml", id));
    fs::write(&file_path, &yaml_content).into_diagnostic()?;

    // Add to short ID index
    let mut short_ids = ShortIdIndex::load(&project);
    let short_id = short_ids.add(id.to_string());
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
        style(&part_number).yellow(),
        style(&title).white()
    );
    crate::cli::entity_cmd::output_new_entity(
        &id,
        &file_path,
        short_id.clone(),
        ENTITY_CONFIG.name,
        &title,
        Some(&extra_info),
        &added_links,
        global,
    );

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

    // Resolve ID from argument or stdin
    let id = resolve_id_arg(&args.id).map_err(|e| miette::miette!("{}", e))?;

    // Resolve short ID if needed
    let short_ids = ShortIdIndex::load(&project);
    let resolved_id = short_ids.resolve(&id).unwrap_or_else(|| id.clone());

    // Find the component file
    let cmp_dir = project.root().join("bom/components");
    let mut found_path = None;

    if cmp_dir.exists() {
        for entry in fs::read_dir(&cmp_dir).into_diagnostic()? {
            let entry = entry.into_diagnostic()?;
            let path = entry.path();

            if path.extension().is_some_and(|e| e == "yaml") {
                let filename = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                if filename.contains(&resolved_id) || filename.starts_with(&resolved_id) {
                    found_path = Some(path);
                    break;
                }
            }
        }
    }

    let path = found_path.ok_or_else(|| miette::miette!("No component found matching '{}'", id))?;

    // Read and parse component
    let content = fs::read_to_string(&path).into_diagnostic()?;
    let cmp: Component = serde_yml::from_str(&content).into_diagnostic()?;

    match global.output {
        OutputFormat::Yaml => {
            print!("{}", content);
        }
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&cmp).into_diagnostic()?;
            println!("{}", json);
        }
        OutputFormat::Id | OutputFormat::ShortId => {
            if global.output == OutputFormat::ShortId {
                let short_ids = ShortIdIndex::load(&project);
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
                    crate::entities::component::MakeBuy::Make => style("MAKE").green(),
                    crate::entities::component::MakeBuy::Buy => style("BUY").blue(),
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

/// Load all quotes from the project
fn load_all_quotes(project: &Project) -> Vec<crate::entities::quote::Quote> {
    let mut quotes = Vec::new();

    let quotes_dir = project.root().join("bom/quotes");
    if quotes_dir.exists() {
        for entry in walkdir::WalkDir::new(&quotes_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        {
            if let Ok(quote) =
                crate::yaml::parse_yaml_file::<crate::entities::quote::Quote>(entry.path())
            {
                quotes.push(quote);
            }
        }
    }

    quotes
}

fn run_set_quote(args: SetQuoteArgs) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let short_ids = ShortIdIndex::load(&project);

    // Resolve component ID
    let cmp_id = short_ids
        .resolve(&args.component)
        .unwrap_or_else(|| args.component.clone());

    // Resolve quote ID
    let quote_id = short_ids
        .resolve(&args.quote)
        .unwrap_or_else(|| args.quote.clone());

    // Find and load the quote to validate it exists and is for this component
    let quotes = load_all_quotes(&project);
    let quote = quotes
        .iter()
        .find(|q| q.id.to_string() == quote_id || q.id.to_string().starts_with(&quote_id))
        .ok_or_else(|| miette::miette!("Quote '{}' not found", args.quote))?;

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

    // Find and load the component
    let cmp_dir = project.root().join("bom/components");
    let mut found_path = None;
    let mut component: Option<Component> = None;

    if cmp_dir.exists() {
        for entry in fs::read_dir(&cmp_dir).into_diagnostic()? {
            let entry = entry.into_diagnostic()?;
            let path = entry.path();

            if path.extension().is_some_and(|e| e == "yaml") {
                let filename = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                if filename.contains(&cmp_id) || filename.starts_with(&cmp_id) {
                    let content = fs::read_to_string(&path).into_diagnostic()?;
                    if let Ok(cmp) = serde_yml::from_str::<Component>(&content) {
                        component = Some(cmp);
                        found_path = Some(path);
                        break;
                    }
                }
            }
        }
    }

    let mut component =
        component.ok_or_else(|| miette::miette!("Component '{}' not found", args.component))?;
    let path = found_path.unwrap();

    // Update the selected_quote field
    let old_quote = component.selected_quote.clone();
    component.selected_quote = Some(quote.id.to_string());

    // Save the updated component
    let yaml = serde_yml::to_string(&component).into_diagnostic()?;
    fs::write(&path, yaml).into_diagnostic()?;

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

    Ok(())
}

fn run_clear_quote(args: ClearQuoteArgs) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let short_ids = ShortIdIndex::load(&project);

    // Resolve component ID
    let cmp_id = short_ids
        .resolve(&args.component)
        .unwrap_or_else(|| args.component.clone());

    // Find and load the component
    let cmp_dir = project.root().join("bom/components");
    let mut found_path = None;
    let mut component: Option<Component> = None;

    if cmp_dir.exists() {
        for entry in fs::read_dir(&cmp_dir).into_diagnostic()? {
            let entry = entry.into_diagnostic()?;
            let path = entry.path();

            if path.extension().is_some_and(|e| e == "yaml") {
                let filename = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                if filename.contains(&cmp_id) || filename.starts_with(&cmp_id) {
                    let content = fs::read_to_string(&path).into_diagnostic()?;
                    if let Ok(cmp) = serde_yml::from_str::<Component>(&content) {
                        component = Some(cmp);
                        found_path = Some(path);
                        break;
                    }
                }
            }
        }
    }

    let mut component =
        component.ok_or_else(|| miette::miette!("Component '{}' not found", args.component))?;
    let path = found_path.unwrap();

    let cmp_display = short_ids
        .get_short_id(&component.id.to_string())
        .unwrap_or_else(|| args.component.clone());

    if component.selected_quote.is_none() {
        println!(
            "{} {} has no selected quote",
            style("•").dim(),
            style(&cmp_display).cyan()
        );
        return Ok(());
    }

    let old_quote = component.selected_quote.take();

    // Save the updated component
    let yaml = serde_yml::to_string(&component).into_diagnostic()?;
    fs::write(&path, yaml).into_diagnostic()?;

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

    Ok(())
}

/// Load all assemblies from the project
fn load_all_assemblies(project: &Project) -> Vec<Assembly> {
    let mut assemblies = Vec::new();
    let dir = project.root().join("bom/assemblies");

    if dir.exists() {
        for entry in walkdir::WalkDir::new(&dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        {
            if let Ok(asm) = crate::yaml::parse_yaml_file::<Assembly>(entry.path()) {
                assemblies.push(asm);
            }
        }
    }

    assemblies
}

/// Recursively collect all component IDs from an assembly's BOM
fn collect_bom_component_ids(
    assembly: &Assembly,
    assembly_map: &std::collections::HashMap<String, &Assembly>,
    component_ids: &mut HashSet<String>,
    visited: &mut HashSet<String>,
) {
    for item in &assembly.bom {
        let item_id = &item.component_id;

        // Check if this is an assembly (ASM-*) or component (CMP-*)
        if item_id.starts_with("ASM-") {
            // It's a sub-assembly - recurse into it
            if !visited.contains(item_id) {
                visited.insert(item_id.clone());
                if let Some(sub_asm) = assembly_map.get(item_id) {
                    collect_bom_component_ids(sub_asm, assembly_map, component_ids, visited);
                }
            }
        } else if item_id.starts_with("CMP-") {
            // It's a component - add it
            component_ids.insert(item_id.clone());
        }
    }

    // Also check the subassemblies field
    for sub_id in &assembly.subassemblies {
        if !visited.contains(sub_id) {
            visited.insert(sub_id.clone());
            if let Some(sub_asm) = assembly_map.get(sub_id) {
                collect_bom_component_ids(sub_asm, assembly_map, component_ids, visited);
            }
        }
    }
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
    use crate::entities::process::Process;

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
            miette::miette!("Component {} has no manufacturing routing configured", args.cmp)
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
                    miette::miette!(
                        "Process {} not found in routing",
                        args.proc_or_position
                    )
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

    Ok(())
}

fn run_routing_list(args: RoutingListArgs) -> Result<()> {
    use crate::entities::process::Process;

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
            let short = short_ids.get_short_id(proc_id).unwrap_or_else(|| proc_id.clone());
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
        println!("   (Replaced {} previous step{})", old_len, if old_len == 1 { "" } else { "s" });
    }

    Ok(())
}
