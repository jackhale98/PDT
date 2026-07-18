//! `tdt quote` command - Supplier quotation management

use clap::{Subcommand, ValueEnum};
use console::style;
use miette::{IntoDiagnostic, Result};

use crate::cli::commands::utils::format_link_with_title;
use crate::cli::filters::StatusFilter;
use crate::cli::helpers::{format_short_id, truncate_str};
use crate::cli::table::{CellValue, ColumnDef, TableRow};
use crate::cli::{GlobalOpts, OutputFormat};
use tdt_core::core::cache::EntityCache;
use tdt_core::core::identity::{EntityId, EntityPrefix};
use tdt_core::core::project::Project;
use tdt_core::core::shortid::ShortIdIndex;
use tdt_core::core::CachedQuote;
use tdt_core::core::Config;
use tdt_core::entities::quote::{Quote, QuoteStatus};
use tdt_core::schema::wizard::SchemaWizard;
use tdt_core::services::{
    CommonFilter, CreateQuote, QuoteFilter, QuoteService, QuoteSortField, SortDirection,
};

#[derive(Subcommand, Debug)]
pub enum QuoteCommands {
    /// List quotes with filtering
    List(ListArgs),

    /// Create a new quote (requires --component)
    New(NewArgs),

    /// Show a quote's details
    Show(ShowArgs),

    /// Edit a quote in your editor
    Edit(EditArgs),

    /// Delete a quote
    Delete(DeleteArgs),

    /// Archive a quote (soft delete)
    Archive(ArchiveArgs),

    /// Compare quotes for a component
    Compare(CompareArgs),

    /// Get price for a specific quantity (shows price break and total cost)
    Price(PriceArgs),
}

/// Quote status filter
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum QuoteStatusFilter {
    Pending,
    Received,
    Accepted,
    Rejected,
    Expired,
    All,
}

#[derive(clap::Args, Debug)]
pub struct ListArgs {
    /// Filter by quote status
    #[arg(long, short = 'Q', default_value = "all")]
    pub quote_status: QuoteStatusFilter,

    /// Filter by entity status
    #[arg(long, short = 's', default_value = "all")]
    pub status: StatusFilter,

    /// Filter by component
    #[arg(long, short = 'c')]
    pub component: Option<String>,

    /// Filter by assembly
    #[arg(long, short = 'a')]
    pub assembly: Option<String>,

    /// Filter by supplier ID (SUP@N or full ID)
    #[arg(long, short = 'S')]
    pub supplier: Option<String>,

    /// Search in title
    #[arg(long)]
    pub search: Option<String>,

    /// Filter by author (substring match)
    #[arg(long)]
    pub author: Option<String>,

    /// Show quotes created in last N days
    #[arg(long)]
    pub recent: Option<u32>,

    /// Columns to display (can specify multiple)
    #[arg(long, value_delimiter = ',', default_values_t = vec![
        ListColumn::Title,
        ListColumn::Supplier,
        ListColumn::Component,
        ListColumn::Price,
        ListColumn::QuoteStatus
    ])]
    pub columns: Vec<ListColumn>,

    /// Show full ID column (hidden by default since SHORT is always shown)
    #[arg(long)]
    pub show_id: bool,

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

/// Columns to display in list output
#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
pub enum ListColumn {
    Id,
    Title,
    Supplier,
    Component,
    Price,
    QuoteStatus,
    Status,
    Author,
    Created,
}

impl std::fmt::Display for ListColumn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ListColumn::Id => write!(f, "id"),
            ListColumn::Title => write!(f, "title"),
            ListColumn::Supplier => write!(f, "supplier"),
            ListColumn::Component => write!(f, "component"),
            ListColumn::Price => write!(f, "price"),
            ListColumn::QuoteStatus => write!(f, "quote-status"),
            ListColumn::Status => write!(f, "status"),
            ListColumn::Author => write!(f, "author"),
            ListColumn::Created => write!(f, "created"),
        }
    }
}

/// Column definitions for quote list output
const QUOTE_COLUMNS: &[ColumnDef] = &[
    ColumnDef::new("id", "ID", 17),
    ColumnDef::new("title", "TITLE", 20),
    ColumnDef::new("supplier", "SUPPLIER", 15),
    ColumnDef::new("component", "FOR", 12),
    ColumnDef::new("price", "PRICE", 10),
    ColumnDef::new("quote-status", "Q-STATUS", 10),
    ColumnDef::new("status", "STATUS", 10),
    ColumnDef::new("author", "AUTHOR", 14),
    ColumnDef::new("created", "CREATED", 12),
];

const QUOTE_LIST_SPEC: crate::cli::entity_cmd::CachedListSpec =
    crate::cli::entity_cmd::CachedListSpec {
        columns: QUOTE_COLUMNS,
        entity_name: "quote",
        entity_prefix: "QUOT",
        empty_message: "No quotes found.",
    };

#[derive(clap::Args, Debug)]
pub struct NewArgs {
    /// Component ID this quote is for (mutually exclusive with --assembly)
    #[arg(long, short = 'c')]
    pub component: Option<String>,

    /// Assembly ID this quote is for (mutually exclusive with --component)
    #[arg(long, short = 'a')]
    pub assembly: Option<String>,

    /// Supplier ID (SUP@N or full ID) - REQUIRED
    #[arg(long, short = 's')]
    pub supplier: Option<String>,

    /// Quote title
    #[arg(long, short = 'T')]
    pub title: Option<String>,

    /// Unit price (for qty 1, or use --breaks for multiple price breaks)
    #[arg(long, short = 'p')]
    pub price: Option<f64>,

    /// Price breaks as QTY:PRICE:LEAD_TIME triplets (e.g., --breaks "100:5.00:14,500:4.50:10,1000:4.00:7")
    #[arg(long, short = 'B', value_delimiter = ',')]
    pub breaks: Vec<String>,

    /// Minimum order quantity
    #[arg(long)]
    pub moq: Option<u32>,

    /// Lead time in days (for single price, or use --breaks)
    #[arg(long, short = 'l')]
    pub lead_time: Option<u32>,

    /// Tooling cost
    #[arg(long, short = 't')]
    pub tooling: Option<f64>,

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
    /// Quote ID or short ID (QUOT@N)
    pub id: String,
}

#[derive(clap::Args, Debug)]
pub struct EditArgs {
    /// Quote ID or short ID (QUOT@N)
    pub id: String,
}

#[derive(clap::Args, Debug)]
pub struct DeleteArgs {
    /// Quote ID or short ID (QUOT@N)
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
    /// Quote ID or short ID (QUOT@N)
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
const QUOTE_DIRS: &[&str] = tdt_core::core::project::Project::entity_search_directories(
    tdt_core::core::identity::EntityPrefix::Quot,
);

/// Entity configuration for quotes
const ENTITY_CONFIG: crate::cli::EntityConfig = crate::cli::EntityConfig {
    prefix: EntityPrefix::Quot,
    dirs: QUOTE_DIRS,
    name: "quote",
    name_plural: "quotes",
};

#[derive(clap::Args, Debug)]
pub struct CompareArgs {
    /// Component or Assembly ID to compare quotes for
    pub item: String,

    /// Quantity to compare prices at (default: 1)
    #[arg(long, short = 'Q', default_value = "1")]
    pub qty: u32,

    /// Include NRE/tooling costs amortized over this production run
    /// (e.g., --amortize 1000 spreads tooling cost over 1000 units)
    #[arg(long, short = 'a')]
    pub amortize: Option<u32>,

    /// Exclude NRE/tooling costs from comparison
    #[arg(long)]
    pub no_nre: bool,
}

#[derive(clap::Args, Debug)]
pub struct PriceArgs {
    /// Quote ID or short ID (QUOT@N)
    pub id: String,

    /// Quantity to get price for
    #[arg(long, short = 'Q', default_value = "1")]
    pub qty: u32,

    /// Include NRE/tooling costs amortized over this production run
    /// (e.g., --amortize 1000 spreads tooling cost over 1000 units)
    #[arg(long, short = 'a')]
    pub amortize: Option<u32>,

    /// Show all price breaks for this quote
    #[arg(long, short = 'A')]
    pub all: bool,
}

/// Parse a price break triplet (QTY:PRICE:LEAD_TIME)
/// Returns (min_qty, unit_price, lead_time_days)
fn parse_price_break(input: &str) -> Result<(u32, f64, Option<u32>)> {
    let parts: Vec<&str> = input.split(':').collect();

    if parts.len() < 2 || parts.len() > 3 {
        return Err(miette::miette!(
            "Invalid price break format '{}'. Expected QTY:PRICE or QTY:PRICE:LEAD_TIME",
            input
        ));
    }

    let qty: u32 = parts[0]
        .parse()
        .map_err(|_| miette::miette!("Invalid quantity '{}' in price break", parts[0]))?;

    let price: f64 = parts[1]
        .parse()
        .map_err(|_| miette::miette!("Invalid price '{}' in price break", parts[1]))?;

    let lead_time = if parts.len() == 3 {
        Some(
            parts[2]
                .parse()
                .map_err(|_| miette::miette!("Invalid lead time '{}' in price break", parts[2]))?,
        )
    } else {
        None
    };

    Ok((qty, price, lead_time))
}

/// Convert QuoteStatusFilter to QuoteStatus
fn quote_status_filter_to_quote_status(filter: QuoteStatusFilter) -> Option<QuoteStatus> {
    match filter {
        QuoteStatusFilter::Pending => Some(QuoteStatus::Pending),
        QuoteStatusFilter::Received => Some(QuoteStatus::Received),
        QuoteStatusFilter::Accepted => Some(QuoteStatus::Accepted),
        QuoteStatusFilter::Rejected => Some(QuoteStatus::Rejected),
        QuoteStatusFilter::Expired => Some(QuoteStatus::Expired),
        QuoteStatusFilter::All => None,
    }
}

/// Build a QuoteFilter from CLI list arguments
fn build_quote_filter(args: &ListArgs, short_ids: &ShortIdIndex) -> QuoteFilter {
    // Resolve supplier ID if provided
    let supplier = args
        .supplier
        .as_ref()
        .map(|s| short_ids.resolve(s).unwrap_or_else(|| s.clone()));

    // Resolve component ID if provided
    let component = args
        .component
        .as_ref()
        .map(|c| short_ids.resolve(c).unwrap_or_else(|| c.clone()));

    // Resolve assembly ID if provided
    let assembly = args
        .assembly
        .as_ref()
        .map(|a| short_ids.resolve(a).unwrap_or_else(|| a.clone()));

    QuoteFilter {
        common: CommonFilter {
            status: crate::cli::entity_cmd::status_filter_to_status(args.status).map(|s| vec![s]),
            author: args.author.clone(),
            search: args.search.clone(),
            recent_days: args.recent,
            limit: args.limit,
            ..Default::default()
        },
        quote_status: quote_status_filter_to_quote_status(args.quote_status),
        supplier,
        component,
        assembly,
        ..Default::default()
    }
}

/// Build sort field and direction from CLI arguments
fn build_quote_sort(args: &ListArgs) -> (QuoteSortField, SortDirection) {
    let field = match args.sort {
        ListColumn::Id => QuoteSortField::Id,
        ListColumn::Title => QuoteSortField::Title,
        ListColumn::Supplier => QuoteSortField::Supplier,
        ListColumn::Component => QuoteSortField::Component,
        ListColumn::Price => QuoteSortField::Price,
        ListColumn::QuoteStatus => QuoteSortField::QuoteStatus,
        ListColumn::Status => QuoteSortField::Status,
        ListColumn::Author => QuoteSortField::Author,
        ListColumn::Created => QuoteSortField::Created,
    };

    let direction = if args.reverse {
        SortDirection::Ascending
    } else {
        SortDirection::Descending
    };

    (field, direction)
}

/// Sort cached quotes by the specified column
fn sort_cached_quotes(entities: &mut [CachedQuote], sort: ListColumn, reverse: bool) {
    entities.sort_by(|a, b| {
        let cmp = match sort {
            ListColumn::Id => a.id.cmp(&b.id),
            ListColumn::Title => a.title.cmp(&b.title),
            ListColumn::Supplier => a.supplier_id.cmp(&b.supplier_id),
            ListColumn::Component => a.component_id.cmp(&b.component_id),
            ListColumn::Price => {
                let price_a = a.unit_price.unwrap_or(0.0);
                let price_b = b.unit_price.unwrap_or(0.0);
                price_a
                    .partial_cmp(&price_b)
                    .unwrap_or(std::cmp::Ordering::Equal)
            }
            ListColumn::QuoteStatus => a.quote_status.cmp(&b.quote_status),
            ListColumn::Status => a.status.cmp(&b.status),
            ListColumn::Author => a.author.cmp(&b.author),
            ListColumn::Created => a.created.cmp(&b.created),
        };
        if reverse {
            cmp.reverse()
        } else {
            cmp
        }
    });
}

/// Run a quote subcommand
pub fn run(cmd: QuoteCommands, global: &GlobalOpts) -> Result<()> {
    match cmd {
        QuoteCommands::List(args) => run_list(args, global),
        QuoteCommands::New(args) => run_new(args, global),
        QuoteCommands::Show(args) => run_show(args, global),
        QuoteCommands::Edit(args) => run_edit(args),
        QuoteCommands::Delete(args) => run_delete(args),
        QuoteCommands::Archive(args) => run_archive(args),
        QuoteCommands::Compare(args) => run_compare(args, global),
        QuoteCommands::Price(args) => run_price(args, global),
    }
}

fn run_list(args: ListArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let cache = EntityCache::open(&project).map_err(|e| miette::miette!("{}", e))?;
    let service = QuoteService::new(&project, &cache);
    let mut short_ids = ShortIdIndex::load(&project);

    // Resolve linked-to filter via cache
    let allowed_ids = crate::cli::helpers::resolve_linked_to(
        &args.linked_to,
        args.via.as_deref(),
        &short_ids,
        &cache,
    );

    // Determine output format
    let format = match global.output {
        OutputFormat::Auto => OutputFormat::Tsv,
        f => f,
    };

    // Check if we can use the fast cache path:
    // - No assembly filter (cache doesn't store this)
    // - No recent filter (would need time-based comparison)
    // - Not JSON/YAML output (needs full entity serialization)
    let can_use_cache = args.assembly.is_none()
        && args.recent.is_none()
        && !matches!(format, OutputFormat::Json | OutputFormat::Yaml);

    if can_use_cache {
        // Fast path: use cached entities via service
        let mut quotes = service.list_cached();

        // Apply status filter
        if let Some(status) = crate::cli::entity_cmd::status_filter_to_status(args.status) {
            quotes.retain(|q| q.status == status);
        }

        // Apply quote status filter
        if let Some(qs) = quote_status_filter_to_quote_status(args.quote_status) {
            let qs_str = qs.to_string();
            quotes.retain(|q| q.quote_status.as_ref().is_some_and(|s| s == &qs_str));
        }

        // Apply supplier filter
        if let Some(ref sup) = args.supplier {
            let resolved_sup = short_ids.resolve(sup).unwrap_or_else(|| sup.clone());
            quotes.retain(|q| {
                q.supplier_id
                    .as_ref()
                    .is_some_and(|s| s.contains(&resolved_sup))
            });
        }

        // Apply component filter
        if let Some(ref cmp) = args.component {
            let resolved_cmp = short_ids.resolve(cmp).unwrap_or_else(|| cmp.clone());
            quotes.retain(|q| {
                q.component_id
                    .as_ref()
                    .is_some_and(|c| c.contains(&resolved_cmp))
            });
        }

        // Apply author filter
        if let Some(ref author_filter) = args.author {
            let author_lower = author_filter.to_lowercase();
            quotes.retain(|q| q.author.to_lowercase().contains(&author_lower));
        }

        // Apply search filter
        if let Some(ref search) = args.search {
            let search_lower = search.to_lowercase();
            quotes.retain(|q| q.title.to_lowercase().contains(&search_lower));
        }

        // Apply linked-to filter
        if let Some(ref ids) = allowed_ids {
            quotes.retain(|e| ids.contains(&e.id));
        }

        // Sort
        sort_cached_quotes(&mut quotes, args.sort, args.reverse);

        // Apply limit
        if let Some(limit) = args.limit {
            quotes.truncate(limit);
        }

        let mut visible: Vec<String> = if args.show_id {
            vec!["id".to_string()]
        } else {
            vec![]
        };
        visible.extend(args.columns.iter().map(|c| c.to_string()));
        return crate::cli::entity_cmd::output_cached_list(
            &quotes,
            &QUOTE_LIST_SPEC,
            &visible,
            args.wrap,
            args.count,
            format,
            &project,
            |q| cached_quote_to_row(q, &short_ids),
            |q| q.file_path.as_path(),
        );
    }

    // Full entity loading via service
    let filter = build_quote_filter(&args, &short_ids);
    let (sort_field, sort_dir) = build_quote_sort(&args);

    let result = service
        .list(&filter, sort_field, sort_dir)
        .map_err(|e| miette::miette!("{}", e))?;
    let mut quotes = result.items;

    // Apply linked-to filter
    if let Some(ref ids) = allowed_ids {
        quotes.retain(|e| ids.contains(&e.id.to_string()));
    }

    // Count only
    if args.count {
        println!("{}", quotes.len());
        return Ok(());
    }

    // No results
    if quotes.is_empty() {
        println!("No quotes found.");
        return Ok(());
    }

    {
        let mut visible: Vec<String> = args.columns.iter().map(|c| c.to_string()).collect();
        if args.show_id && !visible.iter().any(|c| c == "id") {
            visible.insert(0, "id".to_string());
        }
        crate::cli::entity_cmd::output_entity_list(
            &quotes,
            &QUOTE_LIST_SPEC,
            &visible,
            args.wrap,
            args.count,
            format,
            &project,
            &mut short_ids,
            |x| x.id.to_string(),
            quote_to_row,
        )
    }
}

/// Convert a full Quote entity to a TableRow
fn quote_to_row(quote: &Quote, short_ids: &ShortIdIndex) -> TableRow {
    let supplier_display = short_ids
        .get_short_id(&quote.supplier)
        .unwrap_or_else(|| quote.supplier.clone());

    let linked_item = quote.linked_item().unwrap_or("-");
    let component_display = short_ids
        .get_short_id(linked_item)
        .unwrap_or_else(|| linked_item.to_string());

    let unit_price = quote
        .price_for_qty(1)
        .map_or("-".to_string(), |p| format!("{:.2}", p));

    TableRow::new(quote.id.to_string(), short_ids)
        .cell("id", CellValue::Id(quote.id.to_string()))
        .cell("title", CellValue::Text(quote.title.clone()))
        .cell("supplier", CellValue::Text(supplier_display))
        .cell("component", CellValue::Text(component_display))
        .cell("price", CellValue::Text(unit_price))
        .cell(
            "quote-status",
            CellValue::Type(quote.quote_status.to_string()),
        )
        .cell("status", CellValue::Type(quote.status.to_string()))
        .cell("author", CellValue::Text(quote.author.clone()))
        .cell("created", CellValue::DateTime(quote.created))
}

/// Convert a cached quote to a TableRow
fn cached_quote_to_row(quote: &CachedQuote, short_ids: &ShortIdIndex) -> TableRow {
    let supplier_display = quote
        .supplier_id
        .as_ref()
        .map(|s| short_ids.get_short_id(s).unwrap_or_else(|| s.clone()))
        .unwrap_or_else(|| "-".to_string());

    let linked_item = quote.component_id.as_deref().unwrap_or("-");
    let component_display = short_ids
        .get_short_id(linked_item)
        .unwrap_or_else(|| linked_item.to_string());

    let unit_price = quote
        .unit_price
        .map_or("-".to_string(), |p| format!("{:.2}", p));

    TableRow::new(quote.id.clone(), short_ids)
        .cell("id", CellValue::Id(quote.id.clone()))
        .cell("title", CellValue::Text(quote.title.clone()))
        .cell("supplier", CellValue::Text(supplier_display))
        .cell("component", CellValue::Text(component_display))
        .cell("price", CellValue::Text(unit_price))
        .cell(
            "quote-status",
            CellValue::Type(quote.quote_status.clone().unwrap_or_default()),
        )
        .cell("status", CellValue::Type(quote.status.to_string()))
        .cell("author", CellValue::Text(quote.author.clone()))
        .cell("created", CellValue::DateTime(quote.created))
}

fn run_new(args: NewArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let config = Config::load();
    let cache = EntityCache::open(&project).map_err(|e| miette::miette!("{}", e))?;

    // Resolve IDs
    let short_ids = ShortIdIndex::load(&project);

    let component: Option<String>;
    let assembly: Option<String>;
    let supplier: String;
    let title: String;

    // Check for mutually exclusive options
    if args.component.is_some() && args.assembly.is_some() {
        return Err(miette::miette!(
            "Cannot specify both --component and --assembly. Use one or the other."
        ));
    }

    if args.interactive {
        let wizard = SchemaWizard::new();
        let result = wizard.run(EntityPrefix::Quot)?;

        title = result
            .get_string("title")
            .map(String::from)
            .unwrap_or_else(|| "New Quote".to_string());

        // Get supplier from wizard result
        let supplier_input = result
            .get_string("supplier")
            .map(String::from)
            .unwrap_or_default();
        supplier = short_ids.resolve(&supplier_input).unwrap_or(supplier_input);

        // Get component or assembly from wizard result
        let comp_input = result.get_string("component").map(String::from);
        let asm_input = result.get_string("assembly").map(String::from);

        if let Some(cmp) = comp_input {
            if !cmp.is_empty() {
                component = Some(short_ids.resolve(&cmp).unwrap_or(cmp));
                assembly = None;
            } else {
                component = None;
                assembly = asm_input.map(|a| short_ids.resolve(&a).unwrap_or(a));
            }
        } else {
            component = None;
            assembly = asm_input.map(|a| short_ids.resolve(&a).unwrap_or(a));
        }
    } else {
        // At least one of component or assembly must be provided
        if args.component.is_none() && args.assembly.is_none() {
            return Err(miette::miette!(
                "Either --component or --assembly is required"
            ));
        }

        component = args.component.map(|c| short_ids.resolve(&c).unwrap_or(c));
        assembly = args.assembly.map(|a| short_ids.resolve(&a).unwrap_or(a));

        let supplier_input = args
            .supplier
            .ok_or_else(|| miette::miette!("Supplier is required (use --supplier or -s)"))?;
        supplier = short_ids.resolve(&supplier_input).unwrap_or(supplier_input);

        title = args.title.unwrap_or_else(|| "Quote".to_string());
    }

    // Validate referenced item exists using services
    let cmp_service = tdt_core::services::ComponentService::new(&project, &cache);
    let asm_service = tdt_core::services::AssemblyService::new(&project, &cache);

    if let Some(ref cmp) = component {
        if cmp_service
            .get(cmp)
            .map_err(|e| miette::miette!("{}", e))?
            .is_none()
        {
            println!(
                "{} Warning: Component '{}' not found. Create it first with: tdt cmp new",
                style("!").yellow(),
                cmp
            );
        }
    }

    if let Some(ref asm) = assembly {
        if asm_service
            .get(asm)
            .map_err(|e| miette::miette!("{}", e))?
            .is_none()
        {
            println!(
                "{} Warning: Assembly '{}' not found. Create it first with: tdt asm new",
                style("!").yellow(),
                asm
            );
        }
    }

    // Create quote via service
    let service = QuoteService::new(&project, &cache);
    let input = CreateQuote {
        title: title.clone(),
        author: config.author(),
        supplier: supplier.clone(),
        component: component.clone(),
        assembly: assembly.clone(),
        quote_ref: None,
        description: None,
        currency: Default::default(),
        moq: args.moq,
        lead_time_days: args.lead_time,
        tooling_cost: args.tooling,
        tags: Vec::new(),
    };

    let quote = service
        .create(input)
        .map_err(|e| miette::miette!("{}", e))?;
    let id = quote.id.clone();

    // Add price breaks if provided
    if !args.breaks.is_empty() {
        for break_str in &args.breaks {
            let (qty, price, lead_time) = parse_price_break(break_str)?;
            service
                .add_price_break(&id.to_string(), qty, price, lead_time)
                .map_err(|e| miette::miette!("{}", e))?;
        }
    } else if let Some(price) = args.price {
        service
            .add_price_break(&id.to_string(), 1, price, args.lead_time)
            .map_err(|e| miette::miette!("{}", e))?;
    }

    let file_path = project.root().join(format!("bom/quotes/{}.tdt.yaml", id));

    // Add to short ID index
    let mut short_ids = ShortIdIndex::load(&project);
    let short_id = short_ids.add(id.to_string());
    super::utils::save_short_ids(&mut short_ids, &project);

    // Handle --link flags
    let added_links = crate::cli::entity_cmd::process_link_flags(
        &file_path,
        EntityPrefix::Quot,
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
                "{} Created quote {}",
                style("✓").green(),
                style(short_id.clone().unwrap_or_else(|| format_short_id(&id))).cyan()
            );
            println!("   {}", style(file_path.display()).dim());

            let linked_item = component.as_ref().or(assembly.as_ref()).unwrap();
            let item_type = if component.is_some() {
                "Component"
            } else {
                "Assembly"
            };
            println!(
                "   Supplier: {} | {}: {}",
                style(&supplier).yellow(),
                item_type,
                style(linked_item).dim()
            );

            // Show price info
            if !args.breaks.is_empty() {
                println!(
                    "   {} Price break{}:",
                    style(args.breaks.len()).cyan(),
                    if args.breaks.len() == 1 { "" } else { "s" }
                );
                for break_str in &args.breaks {
                    if let Ok((qty, price, lead)) = parse_price_break(break_str) {
                        let lead_str = lead.map(|l| format!(" ({}d)", l)).unwrap_or_default();
                        println!(
                            "     {} @ ${:.2}{}",
                            style(format!("{}+", qty)).white(),
                            price,
                            style(lead_str).dim()
                        );
                    }
                }
            } else if let Some(price) = args.price {
                println!(
                    "   Price: ${:.2} | Lead: {}d",
                    style(format!("{:.2}", price)).green(),
                    style(args.lead_time.unwrap_or(0)).white()
                );
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

    // Open in editor if requested
    if args.edit
        || (!args.no_edit && !args.interactive && args.breaks.is_empty() && args.price.is_none())
    {
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

    // Use QuoteService to get the quote (cache-first lookup)
    let service = QuoteService::new(&project, &cache);
    let quote = service
        .get(&resolved_id)
        .map_err(|e| miette::miette!("{}", e))?
        .ok_or_else(|| miette::miette!("No quote found matching '{}'", args.id))?;

    match global.output {
        OutputFormat::Yaml => {
            let yaml = serde_yml::to_string(&quote).into_diagnostic()?;
            print!("{}", yaml);
        }
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&quote).into_diagnostic()?;
            println!("{}", json);
        }
        OutputFormat::Id | OutputFormat::ShortId => {
            if global.output == OutputFormat::ShortId {
                let sid_index = ShortIdIndex::load(&project);
                let short_id = sid_index
                    .get_short_id(&quote.id.to_string())
                    .unwrap_or_default();
                println!("{}", short_id);
            } else {
                println!("{}", quote.id);
            }
        }
        _ => {
            // Load cache for title lookups
            let cache = EntityCache::open(&project).ok();

            // Pretty format (default)
            println!("{}", style("─".repeat(60)).dim());
            println!(
                "{}: {}",
                style("ID").bold(),
                style(&quote.id.to_string()).cyan()
            );
            println!(
                "{}: {}",
                style("Title").bold(),
                style(&quote.title).yellow()
            );
            if let Some(ref cmp) = quote.component {
                let cmp_display = format_link_with_title(cmp, &short_ids, &cache);
                println!(
                    "{}: {}",
                    style("Component").bold(),
                    style(&cmp_display).cyan()
                );
            }
            if let Some(ref asm) = quote.assembly {
                let asm_display = format_link_with_title(asm, &short_ids, &cache);
                println!(
                    "{}: {}",
                    style("Assembly").bold(),
                    style(&asm_display).cyan()
                );
            }
            let sup_display = format_link_with_title(&quote.supplier, &short_ids, &cache);
            println!(
                "{}: {}",
                style("Supplier").bold(),
                style(&sup_display).cyan()
            );
            println!("{}: {}", style("Status").bold(), quote.status);
            println!("{}", style("─".repeat(60)).dim());

            // Price Breaks
            if !quote.price_breaks.is_empty() {
                println!();
                println!("{}", style("Price Breaks:").bold());
                for pb in &quote.price_breaks {
                    print!("  Qty {}: ${:.2}", pb.min_qty, pb.unit_price);
                    if let Some(lead) = pb.lead_time_days {
                        print!(" ({} day lead)", lead);
                    }
                    println!();
                }
            }

            // Quote Details
            if let Some(ref qn) = quote.quote_ref {
                println!();
                println!("{}: {}", style("Quote Ref").bold(), qn);
            }
            if let Some(ref date) = quote.quote_date {
                println!("{}: {}", style("Quote Date").bold(), date);
            }
            if let Some(ref valid) = quote.valid_until {
                println!("{}: {}", style("Valid Until").bold(), valid);
            }

            // Tags
            if !quote.tags.is_empty() {
                println!();
                println!("{}: {}", style("Tags").bold(), quote.tags.join(", "));
            }

            // Description
            if let Some(ref desc) = quote.description {
                if !desc.is_empty() && !desc.starts_with('#') {
                    println!();
                    println!("{}", style("Description:").bold());
                    println!("{}", desc);
                }
            }

            // Footer
            println!("{}", style("─".repeat(60)).dim());
            println!(
                "{}: {} | {}: {} | {}: {}",
                style("Author").dim(),
                quote.author,
                style("Created").dim(),
                quote.created.format("%Y-%m-%d %H:%M"),
                style("Revision").dim(),
                quote.entity_revision
            );
        }
    }

    Ok(())
}

fn run_edit(args: EditArgs) -> Result<()> {
    crate::cli::entity_cmd::run_edit_generic(&args.id, &ENTITY_CONFIG)
}

fn run_delete(args: DeleteArgs) -> Result<()> {
    crate::cli::commands::utils::run_delete(&args.id, QUOTE_DIRS, args.force, false, args.quiet)
}

fn run_archive(args: ArchiveArgs) -> Result<()> {
    crate::cli::commands::utils::run_delete(&args.id, QUOTE_DIRS, args.force, true, args.quiet)
}

/// Calculate effective unit price including amortized NRE/tooling costs
fn effective_unit_price(quote: &Quote, qty: u32, amortize: Option<u32>, include_nre: bool) -> f64 {
    let base_price = quote.price_for_qty(qty).unwrap_or(0.0);

    if !include_nre {
        return base_price;
    }

    // Calculate NRE per unit
    let nre_per_unit = if let Some(amort_qty) = amortize {
        let total_nre = quote.total_nre();
        total_nre / amort_qty as f64
    } else {
        0.0
    };

    base_price + nre_per_unit
}

fn run_compare(args: CompareArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let cache = EntityCache::open(&project).map_err(|e| miette::miette!("{}", e))?;

    // Resolve the item ID (could be component or assembly)
    let short_ids = ShortIdIndex::load(&project);
    let item = short_ids
        .resolve(&args.item)
        .unwrap_or_else(|| args.item.clone());

    let qty = args.qty;
    let include_nre = !args.no_nre;

    // Use service to get quotes for this item (try component first, then assembly)
    let service = QuoteService::new(&project, &cache);
    let mut quotes = service.get_by_component(&item).unwrap_or_default();
    if quotes.is_empty() {
        quotes = service.get_by_assembly(&item).unwrap_or_default();
    }

    if quotes.is_empty() {
        println!("No quotes found for '{}'", args.item);
        return Ok(());
    }

    // Sort by effective unit price at the specified quantity (lowest first)
    quotes.sort_by(|a, b| {
        let price_a = effective_unit_price(a, qty, args.amortize, include_nre);
        let price_b = effective_unit_price(b, qty, args.amortize, include_nre);
        price_a
            .partial_cmp(&price_b)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Update short ID index
    let mut short_ids = ShortIdIndex::load(&project);
    short_ids.ensure_all(quotes.iter().map(|q| q.id.to_string()));
    super::utils::save_short_ids(&mut short_ids, &project);

    // Output comparison
    let format = match global.output {
        OutputFormat::Auto => OutputFormat::Tsv,
        f => f,
    };

    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&quotes).into_diagnostic()?;
            println!("{}", json);
        }
        OutputFormat::Yaml => {
            let yaml = serde_yml::to_string(&quotes).into_diagnostic()?;
            print!("{}", yaml);
        }
        OutputFormat::Tsv => {
            println!(
                "Comparing {} quotes for {} at qty {}",
                style(quotes.len()).cyan(),
                style(&args.item).yellow(),
                style(qty).white()
            );
            if let Some(amort) = args.amortize {
                println!(
                    "   {} NRE/tooling amortized over {} units",
                    style("→").dim(),
                    style(amort).cyan()
                );
            }
            println!();

            // Adjust header based on whether we're showing NRE
            let show_nre_col = include_nre && args.amortize.is_some();
            if show_nre_col {
                println!(
                    "{:<8} {:<18} {:<12} {:<10} {:<10} {:<8} {:<8} {:<10} {:<10}",
                    style("SHORT").bold().dim(),
                    style("TITLE").bold(),
                    style("SUPPLIER").bold(),
                    style("UNIT").bold(),
                    style("EFF.UNIT").bold(),
                    style("MOQ").bold(),
                    style("LEAD").bold(),
                    style("TOOLING").bold(),
                    style("STATUS").bold()
                );
            } else {
                println!(
                    "{:<8} {:<20} {:<15} {:<10} {:<8} {:<10} {:<10} {:<10}",
                    style("SHORT").bold().dim(),
                    style("TITLE").bold(),
                    style("SUPPLIER").bold(),
                    style("PRICE").bold(),
                    style("MOQ").bold(),
                    style("LEAD").bold(),
                    style("TOOLING").bold(),
                    style("STATUS").bold()
                );
            }
            println!("{}", "-".repeat(if show_nre_col { 110 } else { 100 }));

            for (i, quote) in quotes.iter().enumerate() {
                let short_id = short_ids
                    .get_short_id(&quote.id.to_string())
                    .unwrap_or_default();
                let title_truncated =
                    truncate_str(&quote.title, if show_nre_col { 16 } else { 18 });
                let supplier_short = short_ids.get_short_id(&quote.supplier).unwrap_or_else(|| {
                    truncate_str(&quote.supplier, if show_nre_col { 10 } else { 13 }).to_string()
                });
                let base_price = quote.price_for_qty(qty);
                let unit_price_str = base_price.map_or("-".to_string(), |p| format!("{:.2}", p));
                let eff_price = effective_unit_price(quote, qty, args.amortize, include_nre);
                let eff_price_str = format!("{:.2}", eff_price);
                let moq = quote.moq.map_or("-".to_string(), |m| m.to_string());
                let lead_time = quote
                    .lead_time_for_qty(qty)
                    .map_or("-".to_string(), |d| format!("{}d", d));
                let tooling = quote
                    .tooling_cost
                    .map_or("-".to_string(), |t| format!("{:.0}", t));

                let price_style = if i == 0 {
                    style(if show_nre_col {
                        eff_price_str.clone()
                    } else {
                        unit_price_str.clone()
                    })
                    .green()
                } else {
                    style(if show_nre_col {
                        eff_price_str.clone()
                    } else {
                        unit_price_str.clone()
                    })
                    .white()
                };

                if show_nre_col {
                    println!(
                        "{:<8} {:<18} {:<12} {:<10} {:<10} {:<8} {:<8} {:<10} {:<10}",
                        style(&short_id).cyan(),
                        title_truncated,
                        supplier_short,
                        unit_price_str,
                        price_style,
                        moq,
                        lead_time,
                        tooling,
                        quote.quote_status
                    );
                } else {
                    println!(
                        "{:<8} {:<20} {:<15} {:<10} {:<8} {:<10} {:<10} {:<10}",
                        style(&short_id).cyan(),
                        title_truncated,
                        supplier_short,
                        price_style,
                        moq,
                        lead_time,
                        tooling,
                        quote.quote_status
                    );
                }
            }

            if let Some(lowest) = quotes.first() {
                let supplier_display = short_ids
                    .get_short_id(&lowest.supplier)
                    .unwrap_or_else(|| lowest.supplier.clone());
                let best_price = effective_unit_price(lowest, qty, args.amortize, include_nre);
                println!();
                println!(
                    "{} Lowest price at qty {}: ${:.2} from {}",
                    style("★").yellow(),
                    qty,
                    style(format!("{:.2}", best_price)).green(),
                    style(&supplier_display).cyan()
                );
            }
        }
        _ => {
            let yaml = serde_yml::to_string(&quotes).into_diagnostic()?;
            print!("{}", yaml);
        }
    }

    Ok(())
}

fn run_price(args: PriceArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let cache = EntityCache::open(&project).map_err(|e| miette::miette!("{}", e))?;

    // Resolve short ID if needed
    let short_ids = ShortIdIndex::load(&project);
    let resolved_id = short_ids
        .resolve(&args.id)
        .unwrap_or_else(|| args.id.clone());

    // Load quote via service
    let service = QuoteService::new(&project, &cache);
    let quote = service
        .get_required(&resolved_id)
        .map_err(|_| miette::miette!("No quote found matching '{}'", args.id))?;

    let qty = args.qty;

    match global.output {
        OutputFormat::Json => {
            #[derive(serde::Serialize)]
            struct PriceResult {
                quote_id: String,
                quantity: u32,
                unit_price: Option<f64>,
                extended_price: Option<f64>,
                nre_total: f64,
                nre_per_unit: Option<f64>,
                effective_unit_price: Option<f64>,
                lead_time_days: Option<u32>,
            }

            let unit_price = quote.price_for_qty(qty);
            let nre_total = quote.total_nre();
            let nre_per_unit = args.amortize.map(|a| nre_total / a as f64);
            let effective = unit_price.map(|p| p + nre_per_unit.unwrap_or(0.0));

            let result = PriceResult {
                quote_id: quote.id.to_string(),
                quantity: qty,
                unit_price,
                extended_price: unit_price.map(|p| p * qty as f64),
                nre_total,
                nre_per_unit,
                effective_unit_price: effective,
                lead_time_days: quote.lead_time_for_qty(qty),
            };

            let json = serde_json::to_string_pretty(&result).into_diagnostic()?;
            println!("{}", json);
        }
        _ => {
            let quote_short = short_ids
                .get_short_id(&quote.id.to_string())
                .unwrap_or_else(|| quote.id.to_string());

            println!("{} {}", style("Quote:").bold(), style(&quote_short).cyan());
            println!("{} {}", style("Title:").bold(), &quote.title);

            // Show expiration warning if applicable
            if quote.is_expired() {
                println!(
                    "{} {}",
                    style("⚠").red(),
                    style("This quote has expired!").red().bold()
                );
            }

            println!();

            if args.all {
                // Show all price breaks
                println!("{}", style("Price Breaks:").bold());
                println!(
                    "{:<10} {:<12} {:<12} {:<10}",
                    style("MIN QTY").dim(),
                    style("UNIT PRICE").dim(),
                    style("LEAD TIME").dim(),
                    style("APPLIES").dim()
                );
                println!("{}", "-".repeat(50));

                for pb in &quote.price_breaks {
                    let applies = if pb.min_qty <= qty { "✓" } else { "" };
                    let lead = pb
                        .lead_time_days
                        .map_or("-".to_string(), |d| format!("{}d", d));
                    println!(
                        "{:<10} ${:<11.2} {:<12} {}",
                        pb.min_qty,
                        pb.unit_price,
                        lead,
                        style(applies).green()
                    );
                }
                println!();
            }

            // Show price for specified quantity
            println!("{} {}", style("Quantity:").bold(), style(qty).yellow());

            if let Some(unit_price) = quote.price_for_qty(qty) {
                println!(
                    "{} ${}",
                    style("Unit Price:").bold(),
                    style(format!("{:.2}", unit_price)).green()
                );
                println!(
                    "{} ${:.2}",
                    style("Extended:").bold(),
                    unit_price * qty as f64
                );

                if let Some(lead) = quote.lead_time_for_qty(qty) {
                    println!("{} {} days", style("Lead Time:").bold(), lead);
                }

                // Show NRE breakdown if applicable
                let nre_total = quote.total_nre();
                if nre_total > 0.0 {
                    println!();
                    println!("{}", style("NRE/Tooling Costs:").bold());
                    if let Some(tooling) = quote.tooling_cost {
                        println!("  Tooling: ${:.2}", tooling);
                    }
                    for nre in &quote.nre_costs {
                        println!(
                            "  {}: ${:.2}{}",
                            nre.description,
                            nre.cost,
                            if nre.one_time { " (one-time)" } else { "" }
                        );
                    }
                    println!("  {} ${:.2}", style("Total NRE:").bold(), nre_total);

                    if let Some(amort_qty) = args.amortize {
                        let nre_per_unit = nre_total / amort_qty as f64;
                        let effective_price = unit_price + nre_per_unit;
                        println!();
                        println!(
                            "{} {} units",
                            style("Amortization:").bold(),
                            style(amort_qty).cyan()
                        );
                        println!("  NRE per unit: ${:.4}", nre_per_unit);
                        println!(
                            "  {} ${:.4}",
                            style("Effective Unit Price:").bold().green(),
                            effective_price
                        );
                    }
                }
            } else {
                println!("{} No price available for qty {}", style("!").yellow(), qty);
                if let Some(moq) = quote.moq {
                    println!("   Minimum order quantity is {}", style(moq).cyan());
                }
            }
        }
    }

    Ok(())
}
