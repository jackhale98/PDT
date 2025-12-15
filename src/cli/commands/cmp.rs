//! `tdt cmp` command - Component management

use clap::{Subcommand, ValueEnum};
use console::style;
use miette::{IntoDiagnostic, Result};
use std::fs;

use crate::cli::helpers::{escape_csv, format_short_id, truncate_str};
use crate::cli::{GlobalOpts, OutputFormat};
use crate::core::cache::EntityCache;
use crate::core::identity::{EntityId, EntityPrefix};
use crate::core::project::Project;
use crate::core::shortid::ShortIdIndex;
use crate::core::Config;
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

    /// Set the selected quote for pricing
    SetQuote(SetQuoteArgs),

    /// Clear the selected quote (revert to manual unit_cost)
    ClearQuote(ClearQuoteArgs),

    /// Show component interaction matrix from mates and tolerances
    Matrix(MatrixArgs),
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

/// Status filter
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum StatusFilter {
    Draft,
    Review,
    Approved,
    Released,
    Obsolete,
    /// All statuses
    All,
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

    /// Columns to display (can specify multiple)
    #[arg(long, value_delimiter = ',', default_values_t = vec![
        ListColumn::Id,
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
}

#[derive(clap::Args, Debug)]
pub struct ShowArgs {
    /// Component ID or short ID (CMP@N)
    pub id: String,
}

#[derive(clap::Args, Debug)]
pub struct EditArgs {
    /// Component ID or short ID (CMP@N)
    pub id: String,
}

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

/// Interaction type filter for matrix
#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
pub enum InteractionType {
    /// Show all interactions
    All,
    /// Only mate interactions
    Mate,
    /// Only tolerance stackup interactions
    Tolerance,
}

#[derive(clap::Args, Debug)]
pub struct MatrixArgs {
    /// Filter by interaction type
    #[arg(long, short = 't', default_value = "all")]
    pub interaction_type: InteractionType,

    /// Show only interactions for specific component
    #[arg(long, short = 'c')]
    pub component: Option<String>,

    /// Output as CSV (recommended for large matrices)
    #[arg(long)]
    pub csv: bool,
}

/// Run a component subcommand
pub fn run(cmd: CmpCommands, global: &GlobalOpts) -> Result<()> {
    match cmd {
        CmpCommands::List(args) => run_list(args, global),
        CmpCommands::New(args) => run_new(args),
        CmpCommands::Show(args) => run_show(args, global),
        CmpCommands::Edit(args) => run_edit(args),
        CmpCommands::SetQuote(args) => run_set_quote(args),
        CmpCommands::ClearQuote(args) => run_clear_quote(args),
        CmpCommands::Matrix(args) => run_matrix(args, global),
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
    let needs_complex_filters = args.search.is_some()  // search in description
        || args.long_lead.is_some()  // needs supplier data
        || args.single_source        // needs supplier data
        || args.no_quote             // needs quote data
        || args.high_cost.is_some(); // needs unit_cost
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
        let status_filter = match args.status {
            StatusFilter::Draft => Some("draft"),
            StatusFilter::Review => Some("review"),
            StatusFilter::Approved => Some("approved"),
            StatusFilter::Released => Some("released"),
            StatusFilter::Obsolete => Some("obsolete"),
            StatusFilter::All => None,
        };

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
            args.recent.map_or(true, |days| {
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
        let _ = short_ids.save(&project);

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

        if path.extension().map_or(false, |e| e == "yaml") {
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
        .filter(|c| match args.status {
            StatusFilter::Draft => c.status == crate::core::entity::Status::Draft,
            StatusFilter::Review => c.status == crate::core::entity::Status::Review,
            StatusFilter::Approved => c.status == crate::core::entity::Status::Approved,
            StatusFilter::Released => c.status == crate::core::entity::Status::Released,
            StatusFilter::Obsolete => c.status == crate::core::entity::Status::Obsolete,
            StatusFilter::All => true,
        })
        .filter(|c| {
            if let Some(ref search) = args.search {
                let search_lower = search.to_lowercase();
                c.part_number.to_lowercase().contains(&search_lower)
                    || c.title.to_lowercase().contains(&search_lower)
                    || c.description
                        .as_ref()
                        .map_or(false, |d| d.to_lowercase().contains(&search_lower))
            } else {
                true
            }
        })
        .filter(|c| {
            args.author.as_ref().map_or(true, |author| {
                c.author.to_lowercase().contains(&author.to_lowercase())
            })
        })
        .filter(|c| {
            args.recent.map_or(true, |days| {
                let cutoff = chrono::Utc::now() - chrono::Duration::days(days as i64);
                c.created >= cutoff
            })
        })
        // Long lead time filter - check if any supplier has lead_time_days > threshold
        .filter(|c| {
            args.long_lead.map_or(true, |threshold| {
                c.suppliers
                    .iter()
                    .any(|s| s.lead_time_days.map_or(false, |days| days > threshold))
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
                    .any(|q| q.component.as_ref().map_or(false, |qc| qc == &cid_str))
            } else {
                true
            }
        })
        // High cost filter
        .filter(|c| {
            args.high_cost.map_or(true, |threshold| {
                c.unit_cost.map_or(false, |cost| cost > threshold)
            })
        })
        .collect();

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
    let _ = short_ids.save(&project);

    // Output based on format
    let format = match global.format {
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
        OutputFormat::Csv => {
            println!("short_id,id,part_number,revision,title,make_buy,category,status");
            for cmp in &components {
                let short_id = short_ids
                    .get_short_id(&cmp.id.to_string())
                    .unwrap_or_default();
                println!(
                    "{},{},{},{},{},{},{},{}",
                    short_id,
                    cmp.id,
                    cmp.part_number,
                    cmp.revision.as_deref().unwrap_or(""),
                    escape_csv(&cmp.title),
                    cmp.make_buy,
                    cmp.category,
                    cmp.status
                );
            }
        }
        OutputFormat::Tsv => {
            // Build header based on selected columns
            let mut header_parts = vec![format!("{:<8}", style("SHORT").bold().dim())];
            for col in &args.columns {
                let header = match col {
                    ListColumn::Id => format!("{:<17}", style("ID").bold()),
                    ListColumn::PartNumber => format!("{:<12}", style("PART #").bold()),
                    ListColumn::Revision => format!("{:<8}", style("REV").bold()),
                    ListColumn::Title => format!("{:<30}", style("TITLE").bold()),
                    ListColumn::MakeBuy => format!("{:<6}", style("M/B").bold()),
                    ListColumn::Category => format!("{:<12}", style("CATEGORY").bold()),
                    ListColumn::Status => format!("{:<10}", style("STATUS").bold()),
                    ListColumn::Author => format!("{:<16}", style("AUTHOR").bold()),
                    ListColumn::Created => format!("{:<12}", style("CREATED").bold()),
                };
                header_parts.push(header);
            }
            println!("{}", header_parts.join(" "));
            println!("{}", "-".repeat(100));

            for cmp in &components {
                let short_id = short_ids
                    .get_short_id(&cmp.id.to_string())
                    .unwrap_or_default();
                let mut row_parts = vec![format!("{:<8}", style(&short_id).cyan())];

                for col in &args.columns {
                    let value = match col {
                        ListColumn::Id => format!("{:<17}", format_short_id(&cmp.id)),
                        ListColumn::PartNumber => {
                            format!("{:<12}", truncate_str(&cmp.part_number, 10))
                        }
                        ListColumn::Revision => {
                            format!("{:<8}", cmp.revision.as_deref().unwrap_or("-"))
                        }
                        ListColumn::Title => format!("{:<30}", truncate_str(&cmp.title, 28)),
                        ListColumn::MakeBuy => format!(
                            "{:<6}",
                            match cmp.make_buy {
                                MakeBuy::Make => "make",
                                MakeBuy::Buy => "buy",
                            }
                        ),
                        ListColumn::Category => format!("{:<12}", cmp.category),
                        ListColumn::Status => format!("{:<10}", cmp.status),
                        ListColumn::Author => format!("{:<16}", truncate_str(&cmp.author, 14)),
                        ListColumn::Created => format!("{:<12}", cmp.created.format("%Y-%m-%d")),
                    };
                    row_parts.push(value);
                }
                println!("{}", row_parts.join(" "));
            }

            println!();
            println!(
                "{} component(s) found. Use {} to reference by short ID.",
                style(components.len()).cyan(),
                style("CMP@N").cyan()
            );
        }
        OutputFormat::Id => {
            for cmp in &components {
                println!("{}", cmp.id);
            }
        }
        OutputFormat::Md => {
            println!("| Short | ID | Part # | Title | M/B | Category | Status |");
            println!("|---|---|---|---|---|---|---|");
            for cmp in &components {
                let short_id = short_ids
                    .get_short_id(&cmp.id.to_string())
                    .unwrap_or_default();
                println!(
                    "| {} | {} | {} | {} | {} | {} | {} |",
                    short_id,
                    format_short_id(&cmp.id),
                    cmp.part_number,
                    cmp.title,
                    cmp.make_buy,
                    cmp.category,
                    cmp.status
                );
            }
        }
        OutputFormat::Auto => unreachable!(),
    }

    Ok(())
}

/// Output components from cached data (fast path - no YAML parsing)
fn output_cached_components(
    cmps: &[crate::core::CachedComponent],
    short_ids: &ShortIdIndex,
    args: &ListArgs,
    format: OutputFormat,
) -> Result<()> {
    match format {
        OutputFormat::Csv => {
            println!("short_id,id,part_number,revision,title,make_buy,category,status");
            for cmp in cmps {
                let short_id = short_ids.get_short_id(&cmp.id).unwrap_or_default();
                println!(
                    "{},{},{},{},{},{},{},{}",
                    short_id,
                    cmp.id,
                    cmp.part_number.as_deref().unwrap_or(""),
                    cmp.revision.as_deref().unwrap_or(""),
                    escape_csv(&cmp.title),
                    cmp.make_buy.as_deref().unwrap_or("buy"),
                    cmp.category.as_deref().unwrap_or(""),
                    cmp.status
                );
            }
        }
        OutputFormat::Tsv | OutputFormat::Auto => {
            let mut header_parts = vec![format!("{:<8}", style("SHORT").bold().dim())];
            for col in &args.columns {
                let header = match col {
                    ListColumn::Id => format!("{:<17}", style("ID").bold()),
                    ListColumn::PartNumber => format!("{:<12}", style("PART #").bold()),
                    ListColumn::Revision => format!("{:<8}", style("REV").bold()),
                    ListColumn::Title => format!("{:<30}", style("TITLE").bold()),
                    ListColumn::MakeBuy => format!("{:<6}", style("M/B").bold()),
                    ListColumn::Category => format!("{:<12}", style("CATEGORY").bold()),
                    ListColumn::Status => format!("{:<10}", style("STATUS").bold()),
                    ListColumn::Author => format!("{:<16}", style("AUTHOR").bold()),
                    ListColumn::Created => format!("{:<12}", style("CREATED").bold()),
                };
                header_parts.push(header);
            }
            println!("{}", header_parts.join(" "));
            println!("{}", "-".repeat(100));

            for cmp in cmps {
                let short_id = short_ids.get_short_id(&cmp.id).unwrap_or_default();
                let mut row_parts = vec![format!("{:<8}", style(&short_id).cyan())];

                for col in &args.columns {
                    let value = match col {
                        ListColumn::Id => format!("{:<17}", truncate_str(&cmp.id, 15)),
                        ListColumn::PartNumber => format!(
                            "{:<12}",
                            truncate_str(cmp.part_number.as_deref().unwrap_or(""), 10)
                        ),
                        ListColumn::Revision => {
                            format!("{:<8}", cmp.revision.as_deref().unwrap_or("-"))
                        }
                        ListColumn::Title => format!("{:<30}", truncate_str(&cmp.title, 28)),
                        ListColumn::MakeBuy => {
                            format!("{:<6}", cmp.make_buy.as_deref().unwrap_or("buy"))
                        }
                        ListColumn::Category => {
                            format!("{:<12}", cmp.category.as_deref().unwrap_or(""))
                        }
                        ListColumn::Status => format!("{:<10}", cmp.status),
                        ListColumn::Author => format!("{:<16}", truncate_str(&cmp.author, 14)),
                        ListColumn::Created => format!("{:<12}", cmp.created.format("%Y-%m-%d")),
                    };
                    row_parts.push(value);
                }
                println!("{}", row_parts.join(" "));
            }

            println!();
            println!(
                "{} component(s) found. Use {} to reference by short ID.",
                style(cmps.len()).cyan(),
                style("CMP@N").cyan()
            );
        }
        OutputFormat::Id => {
            for cmp in cmps {
                println!("{}", cmp.id);
            }
        }
        OutputFormat::Md => {
            println!("| Short | ID | Part # | Title | M/B | Category | Status |");
            println!("|---|---|---|---|---|---|---|");
            for cmp in cmps {
                let short_id = short_ids.get_short_id(&cmp.id).unwrap_or_default();
                println!(
                    "| {} | {} | {} | {} | {} | {} | {} |",
                    short_id,
                    truncate_str(&cmp.id, 15),
                    cmp.part_number.as_deref().unwrap_or(""),
                    cmp.title,
                    cmp.make_buy.as_deref().unwrap_or("buy"),
                    cmp.category.as_deref().unwrap_or(""),
                    cmp.status
                );
            }
        }
        OutputFormat::Json | OutputFormat::Yaml => unreachable!(),
    }
    Ok(())
}

fn run_new(args: NewArgs) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let config = Config::load();

    let part_number: String;
    let title: String;
    let make_buy: String;
    let category: String;

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
    } else {
        part_number = args
            .part_number
            .ok_or_else(|| miette::miette!("Part number is required (use --part-number or -p)"))?;
        title = args
            .title
            .ok_or_else(|| miette::miette!("Title is required (use --title or -t)"))?;
        make_buy = args.make_buy.to_string();
        category = args.category.to_string();
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

    let ctx = if let Some(ref rev) = args.revision {
        ctx.with_part_revision(rev)
    } else {
        ctx
    };

    let ctx = if let Some(ref mat) = args.material {
        ctx.with_material(mat)
    } else {
        ctx
    };

    let yaml_content = generator
        .generate_component(&ctx)
        .map_err(|e| miette::miette!("{}", e))?;

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
    let _ = short_ids.save(&project);

    println!(
        "{} Created component {}",
        style("✓").green(),
        style(short_id.unwrap_or_else(|| format_short_id(&id))).cyan()
    );
    println!("   {}", style(file_path.display()).dim());
    println!(
        "   Part: {} | {}",
        style(&part_number).yellow(),
        style(&title).white()
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

    // Resolve short ID if needed
    let short_ids = ShortIdIndex::load(&project);
    let resolved_id = short_ids
        .resolve(&args.id)
        .unwrap_or_else(|| args.id.clone());

    // Find the component file
    let cmp_dir = project.root().join("bom/components");
    let mut found_path = None;

    if cmp_dir.exists() {
        for entry in fs::read_dir(&cmp_dir).into_diagnostic()? {
            let entry = entry.into_diagnostic()?;
            let path = entry.path();

            if path.extension().map_or(false, |e| e == "yaml") {
                let filename = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                if filename.contains(&resolved_id) || filename.starts_with(&resolved_id) {
                    found_path = Some(path);
                    break;
                }
            }
        }
    }

    let path =
        found_path.ok_or_else(|| miette::miette!("No component found matching '{}'", args.id))?;

    // Read and parse component
    let content = fs::read_to_string(&path).into_diagnostic()?;
    let cmp: Component = serde_yml::from_str(&content).into_diagnostic()?;

    match global.format {
        OutputFormat::Yaml => {
            print!("{}", content);
        }
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&cmp).into_diagnostic()?;
            println!("{}", json);
        }
        OutputFormat::Id => {
            println!("{}", cmp.id);
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

            // Description
            if let Some(ref desc) = cmp.description {
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
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let config = Config::load();

    // Resolve short ID if needed
    let short_ids = ShortIdIndex::load(&project);
    let resolved_id = short_ids
        .resolve(&args.id)
        .unwrap_or_else(|| args.id.clone());

    // Find the component file
    let cmp_dir = project.root().join("bom/components");
    let mut found_path = None;

    if cmp_dir.exists() {
        for entry in fs::read_dir(&cmp_dir).into_diagnostic()? {
            let entry = entry.into_diagnostic()?;
            let path = entry.path();

            if path.extension().map_or(false, |e| e == "yaml") {
                let filename = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                if filename.contains(&resolved_id) || filename.starts_with(&resolved_id) {
                    found_path = Some(path);
                    break;
                }
            }
        }
    }

    let path =
        found_path.ok_or_else(|| miette::miette!("No component found matching '{}'", args.id))?;

    println!(
        "Opening {} in {}...",
        style(path.display()).cyan(),
        style(config.editor()).yellow()
    );

    config.run_editor(&path).into_diagnostic()?;

    Ok(())
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

            if path.extension().map_or(false, |e| e == "yaml") {
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

            if path.extension().map_or(false, |e| e == "yaml") {
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

/// Component interaction record
#[derive(Debug, Clone)]
struct ComponentInteraction {
    component_a: String,
    component_a_name: String,
    component_b: String,
    component_b_name: String,
    interaction_type: String,
    source_id: String,
    source_name: String,
}

fn run_matrix(args: MatrixArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let short_ids = ShortIdIndex::load(&project);

    // Resolve component filter if provided
    let component_filter = args
        .component
        .as_ref()
        .map(|c| short_ids.resolve(c).unwrap_or_else(|| c.clone()));

    let mut interactions: Vec<ComponentInteraction> = Vec::new();

    // Build feature-to-component lookup from feature files
    let feature_dir = project.root().join("tolerances/features");
    let mut feature_to_component: std::collections::HashMap<String, (String, String)> =
        std::collections::HashMap::new();

    if feature_dir.exists() {
        for entry in fs::read_dir(&feature_dir).into_diagnostic()?.flatten() {
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "yaml") {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(feat) = serde_yml::from_str::<serde_json::Value>(&content) {
                        let feat_id = feat.get("id").and_then(|v| v.as_str()).unwrap_or("");
                        let comp_id = feat.get("component").and_then(|v| v.as_str()).unwrap_or("");
                        let comp_name = feat.get("title").and_then(|v| v.as_str()).unwrap_or("");

                        if !feat_id.is_empty() && !comp_id.is_empty() {
                            feature_to_component.insert(
                                feat_id.to_string(),
                                (comp_id.to_string(), comp_name.to_string()),
                            );
                        }
                    }
                }
            }
        }
    }

    // Load component names for display
    let mut component_names: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    let cmp_dir = project.root().join("bom/components");
    if cmp_dir.exists() {
        for entry in fs::read_dir(&cmp_dir).into_diagnostic()?.flatten() {
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "yaml") {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(cmp) = serde_yml::from_str::<serde_json::Value>(&content) {
                        let cmp_id = cmp.get("id").and_then(|v| v.as_str()).unwrap_or("");
                        let cmp_title = cmp.get("title").and_then(|v| v.as_str()).unwrap_or("");
                        if !cmp_id.is_empty() {
                            component_names.insert(cmp_id.to_string(), cmp_title.to_string());
                        }
                    }
                }
            }
        }
    }

    // Load mates to find component interactions
    if args.interaction_type == InteractionType::All
        || args.interaction_type == InteractionType::Mate
    {
        let mate_dir = project.root().join("tolerances/mates");
        if mate_dir.exists() {
            for entry in fs::read_dir(&mate_dir).into_diagnostic()?.flatten() {
                let path = entry.path();
                if path.extension().map_or(false, |e| e == "yaml") {
                    if let Ok(content) = fs::read_to_string(&path) {
                        if let Ok(mate) = serde_yml::from_str::<serde_json::Value>(&content) {
                            let mate_id = mate.get("id").and_then(|v| v.as_str()).unwrap_or("");
                            let mate_title =
                                mate.get("title").and_then(|v| v.as_str()).unwrap_or("");

                            // Get component IDs from mate - handle three formats:
                            // 1. Simple: feature_a: "FEAT-xxx" (string)
                            // 2. Object with component: feature_a: { id: "...", component_id: "CMP-xxx", ... }
                            // 3. Object without component: feature_a: { id: "FEAT-xxx" } (need lookup)
                            let (comp_a, comp_a_name) = if let Some(feat_a) = mate.get("feature_a")
                            {
                                if let Some(feat_id) = feat_a.as_str() {
                                    // Simple format - look up from feature file
                                    let (cid, _) = feature_to_component
                                        .get(feat_id)
                                        .cloned()
                                        .unwrap_or_default();
                                    let cname =
                                        component_names.get(&cid).cloned().unwrap_or_default();
                                    (cid, cname)
                                } else if let Some(comp_id) =
                                    feat_a.get("component_id").and_then(|v| v.as_str())
                                {
                                    // Object format with component_id
                                    let cname = feat_a
                                        .get("component_name")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("")
                                        .to_string();
                                    (comp_id.to_string(), cname)
                                } else if let Some(feat_id) =
                                    feat_a.get("id").and_then(|v| v.as_str())
                                {
                                    // Object format with only id - look up from feature file
                                    let (cid, _) = feature_to_component
                                        .get(feat_id)
                                        .cloned()
                                        .unwrap_or_default();
                                    let cname =
                                        component_names.get(&cid).cloned().unwrap_or_default();
                                    (cid, cname)
                                } else {
                                    (String::new(), String::new())
                                }
                            } else {
                                (String::new(), String::new())
                            };

                            let (comp_b, comp_b_name) = if let Some(feat_b) = mate.get("feature_b")
                            {
                                if let Some(feat_id) = feat_b.as_str() {
                                    // Simple format - look up from feature file
                                    let (cid, _) = feature_to_component
                                        .get(feat_id)
                                        .cloned()
                                        .unwrap_or_default();
                                    let cname =
                                        component_names.get(&cid).cloned().unwrap_or_default();
                                    (cid, cname)
                                } else if let Some(comp_id) =
                                    feat_b.get("component_id").and_then(|v| v.as_str())
                                {
                                    // Object format with component_id
                                    let cname = feat_b
                                        .get("component_name")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("")
                                        .to_string();
                                    (comp_id.to_string(), cname)
                                } else if let Some(feat_id) =
                                    feat_b.get("id").and_then(|v| v.as_str())
                                {
                                    // Object format with only id - look up from feature file
                                    let (cid, _) = feature_to_component
                                        .get(feat_id)
                                        .cloned()
                                        .unwrap_or_default();
                                    let cname =
                                        component_names.get(&cid).cloned().unwrap_or_default();
                                    (cid, cname)
                                } else {
                                    (String::new(), String::new())
                                }
                            } else {
                                (String::new(), String::new())
                            };

                            if !comp_a.is_empty() && !comp_b.is_empty() && comp_a != comp_b {
                                // Apply component filter
                                if let Some(ref filter) = component_filter {
                                    if !comp_a.contains(filter) && !comp_b.contains(filter) {
                                        continue;
                                    }
                                }

                                interactions.push(ComponentInteraction {
                                    component_a: comp_a,
                                    component_a_name: comp_a_name,
                                    component_b: comp_b,
                                    component_b_name: comp_b_name,
                                    interaction_type: "mate".to_string(),
                                    source_id: mate_id.to_string(),
                                    source_name: mate_title.to_string(),
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    // Load tolerance stackups to find component interactions
    if args.interaction_type == InteractionType::All
        || args.interaction_type == InteractionType::Tolerance
    {
        let stackup_dir = project.root().join("tolerances/stackups");
        if stackup_dir.exists() {
            for entry in fs::read_dir(&stackup_dir).into_diagnostic()?.flatten() {
                let path = entry.path();
                if path.extension().map_or(false, |e| e == "yaml") {
                    if let Ok(content) = fs::read_to_string(&path) {
                        if let Ok(stackup) = serde_yml::from_str::<serde_json::Value>(&content) {
                            let stackup_id =
                                stackup.get("id").and_then(|v| v.as_str()).unwrap_or("");
                            let stackup_title =
                                stackup.get("title").and_then(|v| v.as_str()).unwrap_or("");

                            // Collect all unique components in the stackup
                            // Handle multiple formats:
                            // 1. feature_id: "FEAT-xxx" (simple, need lookup)
                            // 2. feature: { id: "...", component_id: "CMP-xxx", ... } (object with component)
                            // 3. feature: { id: "FEAT-xxx" } (object without component, need lookup)
                            let mut stackup_components: Vec<(String, String)> = Vec::new();

                            if let Some(contributors) =
                                stackup.get("contributors").and_then(|c| c.as_array())
                            {
                                for contrib in contributors {
                                    let (comp_id, comp_name) = if let Some(feat_id) =
                                        contrib.get("feature_id").and_then(|v| v.as_str())
                                    {
                                        // Simple feature_id format - look up from feature file
                                        if let Some((cid, _)) = feature_to_component.get(feat_id) {
                                            let cname = component_names
                                                .get(cid)
                                                .cloned()
                                                .unwrap_or_default();
                                            (cid.clone(), cname)
                                        } else {
                                            continue;
                                        }
                                    } else if let Some(feature) = contrib.get("feature") {
                                        // Nested feature object
                                        if let Some(cid) =
                                            feature.get("component_id").and_then(|v| v.as_str())
                                        {
                                            // Has component_id directly
                                            let cname = feature
                                                .get("component_name")
                                                .and_then(|v| v.as_str())
                                                .unwrap_or("")
                                                .to_string();
                                            (cid.to_string(), cname)
                                        } else if let Some(feat_id) =
                                            feature.get("id").and_then(|v| v.as_str())
                                        {
                                            // Only has feature id - look up component
                                            if let Some((cid, _)) =
                                                feature_to_component.get(feat_id)
                                            {
                                                let cname = component_names
                                                    .get(cid)
                                                    .cloned()
                                                    .unwrap_or_default();
                                                (cid.clone(), cname)
                                            } else {
                                                continue;
                                            }
                                        } else {
                                            continue;
                                        }
                                    } else {
                                        continue;
                                    };

                                    if !comp_id.is_empty()
                                        && !stackup_components.iter().any(|(id, _)| id == &comp_id)
                                    {
                                        stackup_components.push((comp_id, comp_name));
                                    }
                                }
                            }

                            // Create interactions for all pairs in the stackup
                            for i in 0..stackup_components.len() {
                                for j in (i + 1)..stackup_components.len() {
                                    let (ref comp_a, ref comp_a_name) = stackup_components[i];
                                    let (ref comp_b, ref comp_b_name) = stackup_components[j];

                                    // Apply component filter
                                    if let Some(ref filter) = component_filter {
                                        if !comp_a.contains(filter) && !comp_b.contains(filter) {
                                            continue;
                                        }
                                    }

                                    interactions.push(ComponentInteraction {
                                        component_a: comp_a.clone(),
                                        component_a_name: comp_a_name.clone(),
                                        component_b: comp_b.clone(),
                                        component_b_name: comp_b_name.clone(),
                                        interaction_type: "tolerance".to_string(),
                                        source_id: stackup_id.to_string(),
                                        source_name: stackup_title.to_string(),
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    if interactions.is_empty() {
        println!("{}", style("No component interactions found.").yellow());
        if args.interaction_type == InteractionType::Mate {
            println!("Tip: Create mates with `tdt mate new` to track component interfaces.");
        } else if args.interaction_type == InteractionType::Tolerance {
            println!(
                "Tip: Create tolerance stackups with `tdt tol new` to track dimensional chains."
            );
        }
        return Ok(());
    }

    // Collect unique components for matrix
    let mut components: std::collections::BTreeSet<(String, String)> =
        std::collections::BTreeSet::new();
    for interaction in &interactions {
        components.insert((
            interaction.component_a.clone(),
            interaction.component_a_name.clone(),
        ));
        components.insert((
            interaction.component_b.clone(),
            interaction.component_b_name.clone(),
        ));
    }
    let components: Vec<(String, String)> = components.into_iter().collect();

    // Build interaction matrix: [row_idx][col_idx] -> Vec<interaction_type>
    let mut matrix: Vec<Vec<Vec<&str>>> =
        vec![vec![Vec::new(); components.len()]; components.len()];

    for interaction in &interactions {
        let row = components
            .iter()
            .position(|(id, _)| id == &interaction.component_a);
        let col = components
            .iter()
            .position(|(id, _)| id == &interaction.component_b);

        if let (Some(r), Some(c)) = (row, col) {
            let itype = if interaction.interaction_type == "mate" {
                "M"
            } else {
                "T"
            };
            if !matrix[r][c].contains(&itype) {
                matrix[r][c].push(itype);
            }
            if !matrix[c][r].contains(&itype) {
                matrix[c][r].push(itype);
            }
        }
    }

    // Output based on format
    if args.csv || global.format == OutputFormat::Csv {
        // CSV output (recommended for large matrices)
        // Header row: empty, then component names
        print!("\"Component\"");
        for (id, name) in &components {
            let short = short_ids
                .get_short_id(id)
                .unwrap_or_else(|| truncate_str(id, 8));
            print!(",\"{}\"", if name.is_empty() { &short } else { name });
        }
        println!();

        // Data rows
        for (row_idx, (row_id, row_name)) in components.iter().enumerate() {
            let short = short_ids
                .get_short_id(row_id)
                .unwrap_or_else(|| truncate_str(row_id, 8));
            print!(
                "\"{}\"",
                if row_name.is_empty() {
                    &short
                } else {
                    row_name
                }
            );

            for col_idx in 0..components.len() {
                let cell = &matrix[row_idx][col_idx];
                if cell.is_empty() {
                    print!(",\"\"");
                } else {
                    print!(",\"{}\"", cell.join("+"));
                }
            }
            println!();
        }

        // Print legend at bottom
        println!();
        println!("# Legend: M=Mate, T=Tolerance stackup");
        return Ok(());
    }

    match global.format {
        OutputFormat::Json => {
            let json_output = serde_json::json!({
                "components": components.iter().map(|(id, name)| {
                    let short = short_ids.get_short_id(id).unwrap_or_else(|| truncate_str(id, 8));
                    serde_json::json!({
                        "id": id,
                        "short_id": short,
                        "name": name
                    })
                }).collect::<Vec<_>>(),
                "interactions": interactions.iter().map(|i| {
                    serde_json::json!({
                        "component_a": i.component_a,
                        "component_b": i.component_b,
                        "type": i.interaction_type,
                        "source_id": i.source_id,
                        "source_name": i.source_name
                    })
                }).collect::<Vec<_>>(),
                "total_interactions": interactions.len(),
                "total_components": components.len()
            });
            println!(
                "{}",
                serde_json::to_string_pretty(&json_output).unwrap_or_default()
            );
            return Ok(());
        }
        OutputFormat::Yaml => {
            let yaml_output = serde_json::json!({
                "components": components.len(),
                "interactions": interactions.iter().map(|i| {
                    serde_json::json!({
                        "a": i.component_a,
                        "b": i.component_b,
                        "type": i.interaction_type,
                        "source": i.source_id
                    })
                }).collect::<Vec<_>>()
            });
            println!("{}", serde_yml::to_string(&yaml_output).unwrap_or_default());
            return Ok(());
        }
        _ => {}
    }

    // Human-readable matrix output
    println!();
    println!("{}", style("Component Interaction Matrix").bold().cyan());
    println!(
        "{}",
        style(format!(
            "{} components, {} interactions",
            components.len(),
            interactions.len()
        ))
        .dim()
    );
    println!();

    // For large matrices, suggest CSV
    if components.len() > 10 {
        println!(
            "{}",
            style("Tip: For large matrices, use --csv for better readability").dim()
        );
        println!();
    }

    // Calculate column width
    let max_name_len = components
        .iter()
        .map(|(id, name)| {
            if name.is_empty() {
                short_ids
                    .get_short_id(id)
                    .unwrap_or_else(|| truncate_str(id, 8))
                    .len()
            } else {
                name.len().min(12)
            }
        })
        .max()
        .unwrap_or(8);

    let cell_width = 4;

    // Header row
    print!("{:width$} ", "", width = max_name_len);
    for (idx, _) in components.iter().enumerate() {
        print!("{:^width$}", idx + 1, width = cell_width);
    }
    println!();

    // Separator
    print!("{:width$} ", "", width = max_name_len);
    println!("{}", "─".repeat(components.len() * cell_width));

    // Data rows
    for (row_idx, (row_id, row_name)) in components.iter().enumerate() {
        let short = short_ids
            .get_short_id(row_id)
            .unwrap_or_else(|| truncate_str(row_id, 8));
        let display_name = if row_name.is_empty() {
            truncate_str(&short, max_name_len)
        } else {
            truncate_str(row_name, max_name_len)
        };

        print!("{:<width$} ", display_name, width = max_name_len);

        for col_idx in 0..components.len() {
            if row_idx == col_idx {
                print!("{:^width$}", style("·").dim(), width = cell_width);
            } else {
                let cell = &matrix[row_idx][col_idx];
                if cell.is_empty() {
                    print!("{:^width$}", "-", width = cell_width);
                } else {
                    let symbol = cell.join("");
                    let styled = if cell.contains(&"M") && cell.contains(&"T") {
                        style(symbol).magenta().bold()
                    } else if cell.contains(&"M") {
                        style(symbol).cyan()
                    } else {
                        style(symbol).yellow()
                    };
                    print!("{:^width$}", styled, width = cell_width);
                }
            }
        }
        println!();
    }

    // Legend and component index
    println!();
    println!("{}", style("Legend:").bold());
    println!("  {} = Mate interaction", style("M").cyan());
    println!("  {} = Tolerance stackup", style("T").yellow());
    println!(
        "  {} = Both mate and tolerance",
        style("MT").magenta().bold()
    );

    println!();
    println!("{}", style("Components:").bold());
    for (idx, (id, name)) in components.iter().enumerate() {
        let short = short_ids
            .get_short_id(id)
            .unwrap_or_else(|| truncate_str(id, 8));
        println!(
            "  {:>2}. {} {}",
            idx + 1,
            style(&short).cyan(),
            if name.is_empty() { "" } else { name }
        );
    }

    Ok(())
}
