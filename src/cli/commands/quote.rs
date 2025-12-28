//! `tdt quote` command - Supplier quotation management

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
use crate::core::identity::{EntityId, EntityPrefix};
use crate::core::project::Project;
use crate::core::shortid::ShortIdIndex;
use crate::core::CachedQuote;
use crate::core::Config;
use crate::entities::quote::{Quote, QuoteStatus};
use crate::schema::wizard::SchemaWizard;

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
        ListColumn::Id,
        ListColumn::Title,
        ListColumn::Supplier,
        ListColumn::Component,
        ListColumn::Price,
        ListColumn::QuoteStatus
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

/// Directories where quotes are stored
const QUOTE_DIRS: &[&str] = &["bom/quotes"];

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
    }
}

fn run_list(args: ListArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let short_ids = ShortIdIndex::load(&project);

    // Determine output format
    let format = match global.output {
        OutputFormat::Auto => OutputFormat::Tsv,
        f => f,
    };

    // Resolve supplier filter if provided
    let supplier_filter = args
        .supplier
        .as_ref()
        .map(|s| short_ids.resolve(s).unwrap_or_else(|| s.clone()));

    // Resolve component filter if provided
    let component_filter = args
        .component
        .as_ref()
        .map(|c| short_ids.resolve(c).unwrap_or_else(|| c.clone()));

    // Check if we can use the fast cache path:
    // - No assembly filter (cache doesn't store this)
    // - No recent filter (would need time-based SQL)
    // - Not JSON/YAML output (needs full entity serialization)
    let can_use_cache = args.assembly.is_none()
        && args.recent.is_none()
        && !matches!(format, OutputFormat::Json | OutputFormat::Yaml);

    if can_use_cache {
        if let Ok(cache) = EntityCache::open(&project) {
            let status_filter = crate::cli::entity_cmd::status_filter_to_str(args.status);

            let quote_status_filter = match args.quote_status {
                QuoteStatusFilter::Pending => Some("pending"),
                QuoteStatusFilter::Received => Some("received"),
                QuoteStatusFilter::Accepted => Some("accepted"),
                QuoteStatusFilter::Rejected => Some("rejected"),
                QuoteStatusFilter::Expired => Some("expired"),
                QuoteStatusFilter::All => None,
            };

            let mut quotes = cache.list_quotes(
                status_filter,
                quote_status_filter,
                supplier_filter.as_deref(),
                component_filter.as_deref(),
                args.author.as_deref(),
                args.search.as_deref(),
                None, // We'll apply limit after sorting
            );

            // Sort
            match args.sort {
                ListColumn::Id => quotes.sort_by(|a, b| a.id.cmp(&b.id)),
                ListColumn::Title => quotes.sort_by(|a, b| a.title.cmp(&b.title)),
                ListColumn::Supplier => quotes.sort_by(|a, b| {
                    a.supplier_id
                        .as_deref()
                        .unwrap_or("")
                        .cmp(b.supplier_id.as_deref().unwrap_or(""))
                }),
                ListColumn::Component => quotes.sort_by(|a, b| {
                    a.component_id
                        .as_deref()
                        .unwrap_or("")
                        .cmp(b.component_id.as_deref().unwrap_or(""))
                }),
                ListColumn::Price => quotes.sort_by(|a, b| {
                    let price_a = a.unit_price.unwrap_or(0.0);
                    let price_b = b.unit_price.unwrap_or(0.0);
                    price_a
                        .partial_cmp(&price_b)
                        .unwrap_or(std::cmp::Ordering::Equal)
                }),
                ListColumn::QuoteStatus => quotes.sort_by(|a, b| {
                    a.quote_status
                        .as_deref()
                        .unwrap_or("")
                        .cmp(b.quote_status.as_deref().unwrap_or(""))
                }),
                ListColumn::Status => quotes.sort_by(|a, b| a.status.cmp(&b.status)),
                ListColumn::Author => quotes.sort_by(|a, b| a.author.cmp(&b.author)),
                ListColumn::Created => quotes.sort_by(|a, b| a.created.cmp(&b.created)),
            }

            if args.reverse {
                quotes.reverse();
            }

            if let Some(limit) = args.limit {
                quotes.truncate(limit);
            }

            return output_cached_quotes(&quotes, &short_ids, &args, format);
        }
    }

    // Fall back to full YAML loading
    let quote_dir = project.root().join("bom/quotes");

    if !quote_dir.exists() {
        if args.count {
            println!("0");
        } else {
            println!("No quotes found.");
        }
        return Ok(());
    }

    // Resolve assembly filter if provided
    let assembly_filter = args
        .assembly
        .as_ref()
        .map(|a| short_ids.resolve(a).unwrap_or_else(|| a.clone()));

    // Load and parse all quotes
    let mut quotes: Vec<Quote> = Vec::new();

    for entry in fs::read_dir(&quote_dir).into_diagnostic()? {
        let entry = entry.into_diagnostic()?;
        let path = entry.path();

        if path.extension().is_some_and(|e| e == "yaml") {
            let content = fs::read_to_string(&path).into_diagnostic()?;
            if let Ok(quote) = serde_yml::from_str::<Quote>(&content) {
                quotes.push(quote);
            }
        }
    }

    // Apply filters
    let quotes: Vec<Quote> = quotes
        .into_iter()
        .filter(|q| match args.quote_status {
            QuoteStatusFilter::Pending => q.quote_status == QuoteStatus::Pending,
            QuoteStatusFilter::Received => q.quote_status == QuoteStatus::Received,
            QuoteStatusFilter::Accepted => q.quote_status == QuoteStatus::Accepted,
            QuoteStatusFilter::Rejected => q.quote_status == QuoteStatus::Rejected,
            QuoteStatusFilter::Expired => q.quote_status == QuoteStatus::Expired,
            QuoteStatusFilter::All => true,
        })
        .filter(|q| match args.status {
            StatusFilter::Draft => q.status == crate::core::entity::Status::Draft,
            StatusFilter::Review => q.status == crate::core::entity::Status::Review,
            StatusFilter::Approved => q.status == crate::core::entity::Status::Approved,
            StatusFilter::Released => q.status == crate::core::entity::Status::Released,
            StatusFilter::Obsolete => q.status == crate::core::entity::Status::Obsolete,
            StatusFilter::Active => q.status != crate::core::entity::Status::Obsolete,
            StatusFilter::All => true,
        })
        .filter(|q| {
            if let Some(ref cmp) = component_filter {
                q.component.as_ref().is_some_and(|c| c.contains(cmp))
            } else {
                true
            }
        })
        .filter(|q| {
            if let Some(ref asm) = assembly_filter {
                q.assembly.as_ref().is_some_and(|a| a.contains(asm))
            } else {
                true
            }
        })
        .filter(|q| {
            if let Some(ref sup) = supplier_filter {
                q.supplier.contains(sup)
            } else {
                true
            }
        })
        .filter(|q| {
            if let Some(ref search) = args.search {
                let search_lower = search.to_lowercase();
                q.title.to_lowercase().contains(&search_lower)
                    || q.description
                        .as_ref()
                        .is_some_and(|d| d.to_lowercase().contains(&search_lower))
            } else {
                true
            }
        })
        .filter(|q| {
            args.author
                .as_ref()
                .is_none_or(|author| q.author.to_lowercase().contains(&author.to_lowercase()))
        })
        .filter(|q| {
            args.recent.is_none_or(|days| {
                let cutoff = chrono::Utc::now() - chrono::Duration::days(days as i64);
                q.created >= cutoff
            })
        })
        .collect();

    // Sort
    let mut quotes = quotes;
    match args.sort {
        ListColumn::Id => quotes.sort_by(|a, b| a.id.to_string().cmp(&b.id.to_string())),
        ListColumn::Title => quotes.sort_by(|a, b| a.title.cmp(&b.title)),
        ListColumn::Supplier => quotes.sort_by(|a, b| a.supplier.cmp(&b.supplier)),
        ListColumn::Component => quotes.sort_by(|a, b| a.component.cmp(&b.component)),
        ListColumn::Price => quotes.sort_by(|a, b| {
            let price_a = a.price_for_qty(1).unwrap_or(0.0);
            let price_b = b.price_for_qty(1).unwrap_or(0.0);
            price_a
                .partial_cmp(&price_b)
                .unwrap_or(std::cmp::Ordering::Equal)
        }),
        ListColumn::QuoteStatus => quotes
            .sort_by(|a, b| format!("{:?}", a.quote_status).cmp(&format!("{:?}", b.quote_status))),
        ListColumn::Status => {
            quotes.sort_by(|a, b| format!("{:?}", a.status).cmp(&format!("{:?}", b.status)))
        }
        ListColumn::Author => quotes.sort_by(|a, b| a.author.cmp(&b.author)),
        ListColumn::Created => quotes.sort_by(|a, b| a.created.cmp(&b.created)),
    }

    if args.reverse {
        quotes.reverse();
    }

    // Apply limit
    if let Some(limit) = args.limit {
        quotes.truncate(limit);
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

    // Update short ID index
    let mut short_ids = ShortIdIndex::load(&project);
    short_ids.ensure_all(quotes.iter().map(|q| q.id.to_string()));
    super::utils::save_short_ids(&mut short_ids, &project);

    // Output based on format
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
        OutputFormat::Csv | OutputFormat::Tsv | OutputFormat::Md | OutputFormat::Table | OutputFormat::Dot | OutputFormat::Tree => {
            let columns: Vec<&str> = args
                .columns
                .iter()
                .map(|c| c.to_string().leak() as &str)
                .collect();
            let rows: Vec<TableRow> = quotes.iter().map(|q| quote_to_row(q, &short_ids)).collect();

            let config = TableConfig {
                wrap_width: args.wrap,
                show_summary: true,
            };
            let formatter = TableFormatter::new(QUOTE_COLUMNS, "quote", "QUOT").with_config(config);
            formatter.output(rows, format, &columns);
        }
        OutputFormat::Id | OutputFormat::ShortId => {
            for quote in &quotes {
                if format == OutputFormat::ShortId {
                    let short_id = short_ids
                        .get_short_id(&quote.id.to_string())
                        .unwrap_or_default();
                    println!("{}", short_id);
                } else {
                    println!("{}", quote.id);
                }
            }
        }
        OutputFormat::Auto | OutputFormat::Path => unreachable!(),
    }

    Ok(())
}

/// Output cached quotes (fast path - no YAML parsing needed)
fn output_cached_quotes(
    quotes: &[CachedQuote],
    short_ids: &ShortIdIndex,
    args: &ListArgs,
    format: OutputFormat,
) -> Result<()> {
    if quotes.is_empty() {
        println!("No quotes found.");
        return Ok(());
    }

    if args.count {
        println!("{}", quotes.len());
        return Ok(());
    }

    match format {
        OutputFormat::Csv | OutputFormat::Tsv | OutputFormat::Md | OutputFormat::Table | OutputFormat::Dot | OutputFormat::Tree => {
            let columns: Vec<&str> = args
                .columns
                .iter()
                .map(|c| c.to_string().leak() as &str)
                .collect();
            let rows: Vec<TableRow> = quotes
                .iter()
                .map(|q| cached_quote_to_row(q, short_ids))
                .collect();

            let config = TableConfig {
                wrap_width: args.wrap,
                show_summary: true,
            };
            let formatter = TableFormatter::new(QUOTE_COLUMNS, "quote", "QUOT").with_config(config);
            formatter.output(rows, format, &columns);
        }
        OutputFormat::Id | OutputFormat::ShortId => {
            for quote in quotes {
                if format == OutputFormat::ShortId {
                    let short_id = short_ids.get_short_id(&quote.id).unwrap_or_default();
                    println!("{}", short_id);
                } else {
                    println!("{}", quote.id);
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

    // Validate referenced item exists
    if let Some(ref cmp) = component {
        let cmp_dir = project.root().join("bom/components");
        let mut found = false;
        if cmp_dir.exists() {
            for entry in fs::read_dir(&cmp_dir).into_diagnostic()? {
                let entry = entry.into_diagnostic()?;
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "yaml") {
                    let filename = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                    if filename.contains(cmp) {
                        found = true;
                        break;
                    }
                }
            }
        }
        if !found {
            println!(
                "{} Warning: Component '{}' not found. Create it first with: tdt cmp new",
                style("!").yellow(),
                cmp
            );
        }
    }

    if let Some(ref asm) = assembly {
        let asm_dir = project.root().join("bom/assemblies");
        let mut found = false;
        if asm_dir.exists() {
            for entry in fs::read_dir(&asm_dir).into_diagnostic()? {
                let entry = entry.into_diagnostic()?;
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "yaml") {
                    let filename = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                    if filename.contains(asm) {
                        found = true;
                        break;
                    }
                }
            }
        }
        if !found {
            println!(
                "{} Warning: Assembly '{}' not found. Create it first with: tdt asm new",
                style("!").yellow(),
                asm
            );
        }
    }

    // Generate ID
    let id = EntityId::new(EntityPrefix::Quot);

    // Create quote directly instead of using template
    let mut quote = if let Some(ref cmp) = component {
        Quote::new_for_component(&supplier, cmp, &title, config.author())
    } else if let Some(ref asm) = assembly {
        Quote::new_for_assembly(&supplier, asm, &title, config.author())
    } else {
        unreachable!("Either component or assembly must be set")
    };

    // Override the ID to use the one we generated
    quote.id = id.clone();

    // Add price breaks if provided
    if !args.breaks.is_empty() {
        // Multiple price breaks via --breaks
        for break_str in &args.breaks {
            let (qty, price, lead_time) = parse_price_break(break_str)?;
            quote.add_price_break(qty, price, lead_time);
        }
    } else if let Some(price) = args.price {
        // Single price via --price
        quote.add_price_break(1, price, args.lead_time);
    }

    if let Some(moq) = args.moq {
        quote.moq = Some(moq);
    }
    if let Some(lead_time) = args.lead_time {
        quote.lead_time_days = Some(lead_time);
    }
    if let Some(tooling) = args.tooling {
        quote.tooling_cost = Some(tooling);
    }

    let yaml_content = serde_yml::to_string(&quote).into_diagnostic()?;

    // Write file
    let output_dir = project.root().join("bom/quotes");
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

    // Resolve short ID if needed
    let short_ids = ShortIdIndex::load(&project);
    let resolved_id = short_ids
        .resolve(&args.id)
        .unwrap_or_else(|| args.id.clone());

    // Find the quote file
    let quote_dir = project.root().join("bom/quotes");
    let mut found_path = None;

    if quote_dir.exists() {
        for entry in fs::read_dir(&quote_dir).into_diagnostic()? {
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

    let path =
        found_path.ok_or_else(|| miette::miette!("No quote found matching '{}'", args.id))?;

    // Read and parse quote
    let content = fs::read_to_string(&path).into_diagnostic()?;
    let quote: Quote = serde_yml::from_str(&content).into_diagnostic()?;

    match global.output {
        OutputFormat::Yaml => {
            print!("{}", content);
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

fn run_compare(args: CompareArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let quote_dir = project.root().join("bom/quotes");

    if !quote_dir.exists() {
        println!("No quotes found.");
        return Ok(());
    }

    // Resolve the item ID (could be component or assembly)
    let short_ids = ShortIdIndex::load(&project);
    let item = short_ids
        .resolve(&args.item)
        .unwrap_or_else(|| args.item.clone());

    // Load quotes for this item (component or assembly)
    let mut quotes: Vec<Quote> = Vec::new();

    for entry in fs::read_dir(&quote_dir).into_diagnostic()? {
        let entry = entry.into_diagnostic()?;
        let path = entry.path();

        if path.extension().is_some_and(|e| e == "yaml") {
            let content = fs::read_to_string(&path).into_diagnostic()?;
            if let Ok(quote) = serde_yml::from_str::<Quote>(&content) {
                // Check if quote matches either component or assembly
                let matches = quote.component.as_ref().is_some_and(|c| c.contains(&item))
                    || quote.assembly.as_ref().is_some_and(|a| a.contains(&item));
                if matches {
                    quotes.push(quote);
                }
            }
        }
    }

    if quotes.is_empty() {
        println!("No quotes found for '{}'", args.item);
        return Ok(());
    }

    // Sort by unit price (lowest first)
    quotes.sort_by(|a, b| {
        let price_a = a.price_for_qty(1).unwrap_or(f64::MAX);
        let price_b = b.price_for_qty(1).unwrap_or(f64::MAX);
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
                "Comparing {} quotes for {}",
                style(quotes.len()).cyan(),
                style(&args.item).yellow()
            );
            println!();
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
            println!("{}", "-".repeat(100));

            for (i, quote) in quotes.iter().enumerate() {
                let short_id = short_ids
                    .get_short_id(&quote.id.to_string())
                    .unwrap_or_default();
                let title_truncated = truncate_str(&quote.title, 18);
                let supplier_short = short_ids
                    .get_short_id(&quote.supplier)
                    .unwrap_or_else(|| truncate_str(&quote.supplier, 13).to_string());
                let unit_price = quote
                    .price_for_qty(1)
                    .map_or("-".to_string(), |p| format!("{:.2}", p));
                let moq = quote.moq.map_or("-".to_string(), |m| m.to_string());
                let lead_time = quote
                    .lead_time_days
                    .map_or("-".to_string(), |d| format!("{}d", d));
                let tooling = quote
                    .tooling_cost
                    .map_or("-".to_string(), |t| format!("{:.0}", t));

                let price_style = if i == 0 {
                    style(unit_price).green()
                } else {
                    style(unit_price).white()
                };

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

            if let Some(lowest) = quotes.first() {
                let supplier_display = short_ids
                    .get_short_id(&lowest.supplier)
                    .unwrap_or_else(|| lowest.supplier.clone());
                println!();
                println!(
                    "{} Lowest price: {} from {}",
                    style("★").yellow(),
                    style(format!("{:.2}", lowest.price_for_qty(1).unwrap_or(0.0))).green(),
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
