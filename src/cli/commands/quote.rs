//! `pdt quote` command - Supplier quotation management

use clap::{Subcommand, ValueEnum};
use console::style;
use miette::{IntoDiagnostic, Result};
use std::fs;

use crate::cli::{GlobalOpts, OutputFormat};
use crate::core::identity::{EntityId, EntityPrefix};
use crate::core::project::Project;
use crate::core::shortid::ShortIdIndex;
use crate::core::Config;
use crate::entities::quote::{Quote, QuoteStatus};

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

/// Entity status filter
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum StatusFilter {
    Draft,
    Review,
    Approved,
    Released,
    Obsolete,
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

    /// Sort by field
    #[arg(long, default_value = "created")]
    pub sort: SortField,

    /// Reverse sort order
    #[arg(long, short = 'r')]
    pub reverse: bool,

    /// Limit number of results
    #[arg(long, short = 'n')]
    pub limit: Option<usize>,

    /// Show only count
    #[arg(long)]
    pub count: bool,

    /// Output format
    #[arg(long, short = 'o', default_value = "auto")]
    pub format: OutputFormat,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum SortField {
    Title,
    Supplier,
    Component,
    Status,
    Created,
}

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
    #[arg(long, short = 't')]
    pub title: Option<String>,

    /// Unit price (for qty 1)
    #[arg(long, short = 'p')]
    pub price: Option<f64>,

    /// Minimum order quantity
    #[arg(long)]
    pub moq: Option<u32>,

    /// Lead time in days
    #[arg(long)]
    pub lead_time: Option<u32>,

    /// Tooling cost
    #[arg(long)]
    pub tooling: Option<f64>,

    /// Open in editor after creation
    #[arg(long, short = 'e')]
    pub edit: bool,

    /// Skip opening in editor
    #[arg(long)]
    pub no_edit: bool,

    /// Interactive mode (prompt for fields)
    #[arg(long, short = 'i')]
    pub interactive: bool,
}

#[derive(clap::Args, Debug)]
pub struct ShowArgs {
    /// Quote ID or short ID (QUOT@N)
    pub id: String,

    /// Output format
    #[arg(long, short = 'o', default_value = "yaml")]
    pub format: OutputFormat,
}

#[derive(clap::Args, Debug)]
pub struct EditArgs {
    /// Quote ID or short ID (QUOT@N)
    pub id: String,
}

#[derive(clap::Args, Debug)]
pub struct CompareArgs {
    /// Component or Assembly ID to compare quotes for
    pub item: String,

    /// Output format
    #[arg(long, short = 'o', default_value = "auto")]
    pub format: OutputFormat,
}

/// Run a quote subcommand
pub fn run(cmd: QuoteCommands, _global: &GlobalOpts) -> Result<()> {
    match cmd {
        QuoteCommands::List(args) => run_list(args),
        QuoteCommands::New(args) => run_new(args),
        QuoteCommands::Show(args) => run_show(args),
        QuoteCommands::Edit(args) => run_edit(args),
        QuoteCommands::Compare(args) => run_compare(args),
    }
}

fn run_list(args: ListArgs) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let quote_dir = project.root().join("bom/quotes");

    if !quote_dir.exists() {
        if args.count {
            println!("0");
        } else {
            println!("No quotes found.");
        }
        return Ok(());
    }

    // Load short IDs for resolving component references
    let short_ids = ShortIdIndex::load(&project);

    // Resolve component filter if provided
    let component_filter = args.component.as_ref().map(|c| {
        short_ids.resolve(c).unwrap_or_else(|| c.clone())
    });

    // Resolve assembly filter if provided
    let assembly_filter = args.assembly.as_ref().map(|a| {
        short_ids.resolve(a).unwrap_or_else(|| a.clone())
    });

    // Resolve supplier filter if provided
    let supplier_filter = args.supplier.as_ref().map(|s| {
        short_ids.resolve(s).unwrap_or_else(|| s.clone())
    });

    // Load and parse all quotes
    let mut quotes: Vec<Quote> = Vec::new();

    for entry in fs::read_dir(&quote_dir).into_diagnostic()? {
        let entry = entry.into_diagnostic()?;
        let path = entry.path();

        if path.extension().map_or(false, |e| e == "yaml") {
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
            StatusFilter::All => true,
        })
        .filter(|q| {
            if let Some(ref cmp) = component_filter {
                q.component.as_ref().map_or(false, |c| c.contains(cmp))
            } else {
                true
            }
        })
        .filter(|q| {
            if let Some(ref asm) = assembly_filter {
                q.assembly.as_ref().map_or(false, |a| a.contains(asm))
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
                        .map_or(false, |d| d.to_lowercase().contains(&search_lower))
            } else {
                true
            }
        })
        .collect();

    // Sort
    let mut quotes = quotes;
    match args.sort {
        SortField::Title => quotes.sort_by(|a, b| a.title.cmp(&b.title)),
        SortField::Supplier => quotes.sort_by(|a, b| a.supplier.cmp(&b.supplier)),
        SortField::Component => quotes.sort_by(|a, b| a.component.cmp(&b.component)),
        SortField::Status => {
            quotes.sort_by(|a, b| {
                format!("{:?}", a.quote_status).cmp(&format!("{:?}", b.quote_status))
            })
        }
        SortField::Created => quotes.sort_by(|a, b| a.created.cmp(&b.created)),
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
    let _ = short_ids.save(&project);

    // Output based on format
    let format = if args.format == OutputFormat::Auto {
        OutputFormat::Tsv
    } else {
        args.format
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
        OutputFormat::Csv => {
            println!("short_id,id,title,supplier,linked_item,unit_price,lead_time,quote_status,status");
            for quote in &quotes {
                let short_id = short_ids.get_short_id(&quote.id.to_string()).unwrap_or_default();
                let unit_price = quote.price_for_qty(1).map_or("".to_string(), |p| format!("{:.2}", p));
                let lead_time = quote.lead_time_days.map_or("".to_string(), |d| d.to_string());
                let linked_item = quote.linked_item().unwrap_or("-");
                let supplier_short = short_ids
                    .get_short_id(&quote.supplier)
                    .unwrap_or_else(|| quote.supplier.clone());
                println!(
                    "{},{},{},{},{},{},{},{},{}",
                    short_id,
                    quote.id,
                    escape_csv(&quote.title),
                    supplier_short,
                    linked_item,
                    unit_price,
                    lead_time,
                    quote.quote_status,
                    quote.status
                );
            }
        }
        OutputFormat::Tsv => {
            println!(
                "{:<8} {:<17} {:<20} {:<15} {:<12} {:<10} {:<10} {:<10}",
                style("SHORT").bold().dim(),
                style("ID").bold(),
                style("TITLE").bold(),
                style("SUPPLIER").bold(),
                style("FOR").bold(),
                style("PRICE").bold(),
                style("LEAD").bold(),
                style("Q-STATUS").bold()
            );
            println!("{}", "-".repeat(110));

            for quote in &quotes {
                let short_id = short_ids
                    .get_short_id(&quote.id.to_string())
                    .unwrap_or_default();
                let id_display = format_short_id(&quote.id);
                let title_truncated = truncate_str(&quote.title, 18);
                let supplier_short = short_ids
                    .get_short_id(&quote.supplier)
                    .unwrap_or_else(|| truncate_str(&quote.supplier, 13).to_string());
                let linked_item = quote.linked_item().unwrap_or("-");
                let item_short = short_ids.get_short_id(linked_item).unwrap_or_else(|| {
                    truncate_str(linked_item, 10).to_string()
                });
                let unit_price = quote
                    .price_for_qty(1)
                    .map_or("-".to_string(), |p| format!("{:.2}", p));
                let lead_time = quote
                    .lead_time_days
                    .map_or("-".to_string(), |d| format!("{}d", d));

                println!(
                    "{:<8} {:<17} {:<20} {:<15} {:<12} {:<10} {:<10} {:<10}",
                    style(&short_id).cyan(),
                    id_display,
                    title_truncated,
                    supplier_short,
                    item_short,
                    unit_price,
                    lead_time,
                    quote.quote_status
                );
            }

            println!();
            println!(
                "{} quote(s) found. Use {} to reference by short ID.",
                style(quotes.len()).cyan(),
                style("QUOT@N").cyan()
            );
        }
        OutputFormat::Id => {
            for quote in &quotes {
                println!("{}", quote.id);
            }
        }
        OutputFormat::Md => {
            println!("| Short | ID | Title | Supplier | For | Price | Lead | Q-Status |");
            println!("|---|---|---|---|---|---|---|---|");
            for quote in &quotes {
                let short_id = short_ids
                    .get_short_id(&quote.id.to_string())
                    .unwrap_or_default();
                let unit_price = quote
                    .price_for_qty(1)
                    .map_or("-".to_string(), |p| format!("{:.2}", p));
                let lead_time = quote
                    .lead_time_days
                    .map_or("-".to_string(), |d| format!("{}d", d));
                let linked_item = quote.linked_item().unwrap_or("-");
                let supplier_short = short_ids
                    .get_short_id(&quote.supplier)
                    .unwrap_or_else(|| quote.supplier.clone());
                println!(
                    "| {} | {} | {} | {} | {} | {} | {} | {} |",
                    short_id,
                    format_short_id(&quote.id),
                    quote.title,
                    supplier_short,
                    linked_item,
                    unit_price,
                    lead_time,
                    quote.quote_status
                );
            }
        }
        OutputFormat::Auto => unreachable!(),
    }

    Ok(())
}

fn run_new(args: NewArgs) -> Result<()> {
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

    if args.interactive
        || (args.component.is_none() && args.assembly.is_none() && args.supplier.is_none())
    {
        use dialoguer::{Input, Select};

        // Ask which type of item this quote is for
        let item_types = vec!["Component (CMP)", "Assembly (ASM)"];
        let selection = Select::new()
            .with_prompt("What is this quote for?")
            .items(&item_types)
            .default(0)
            .interact()
            .into_diagnostic()?;

        if selection == 0 {
            let cmp_input: String = Input::new()
                .with_prompt("Component ID (CMP@N or full ID)")
                .interact_text()
                .into_diagnostic()?;
            component = Some(short_ids.resolve(&cmp_input).unwrap_or(cmp_input));
            assembly = None;
        } else {
            let asm_input: String = Input::new()
                .with_prompt("Assembly ID (ASM@N or full ID)")
                .interact_text()
                .into_diagnostic()?;
            assembly = Some(short_ids.resolve(&asm_input).unwrap_or(asm_input));
            component = None;
        }

        let supplier_input: String = Input::new()
            .with_prompt("Supplier ID (SUP@N or full ID)")
            .interact_text()
            .into_diagnostic()?;
        supplier = short_ids.resolve(&supplier_input).unwrap_or(supplier_input);

        title = Input::new()
            .with_prompt("Quote title")
            .interact_text()
            .into_diagnostic()?;
    } else {
        // At least one of component or assembly must be provided
        if args.component.is_none() && args.assembly.is_none() {
            return Err(miette::miette!(
                "Either --component or --assembly is required"
            ));
        }

        component = args
            .component
            .map(|c| short_ids.resolve(&c).unwrap_or(c));
        assembly = args
            .assembly
            .map(|a| short_ids.resolve(&a).unwrap_or(a));

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
                if path.extension().map_or(false, |e| e == "yaml") {
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
                "{} Warning: Component '{}' not found. Create it first with: pdt cmp new",
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
                if path.extension().map_or(false, |e| e == "yaml") {
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
                "{} Warning: Assembly '{}' not found. Create it first with: pdt asm new",
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

    // Add price break if provided
    if let Some(price) = args.price {
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

    let file_path = output_dir.join(format!("{}.pdt.yaml", id));
    fs::write(&file_path, &yaml_content).into_diagnostic()?;

    // Add to short ID index
    let mut short_ids = ShortIdIndex::load(&project);
    let short_id = short_ids.add(id.to_string());
    let _ = short_ids.save(&project);

    println!(
        "{} Created quote {}",
        style("✓").green(),
        style(short_id.unwrap_or_else(|| format_short_id(&id))).cyan()
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

    if let Some(price) = args.price {
        println!(
            "   Price: {} | Lead: {}d",
            style(format!("{:.2}", price)).green(),
            style(args.lead_time.unwrap_or(0)).white()
        );
    }

    // Open in editor if requested
    if args.edit || (!args.no_edit && !args.interactive) {
        let editor = config.editor();
        println!();
        println!("Opening in {}...", style(&editor).yellow());

        std::process::Command::new(&editor)
            .arg(&file_path)
            .status()
            .into_diagnostic()?;
    }

    Ok(())
}

fn run_show(args: ShowArgs) -> Result<()> {
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

            if path.extension().map_or(false, |e| e == "yaml") {
                let filename = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                if filename.contains(&resolved_id) || filename.starts_with(&resolved_id) {
                    found_path = Some(path);
                    break;
                }
            }
        }
    }

    let path = found_path.ok_or_else(|| miette::miette!("No quote found matching '{}'", args.id))?;

    // Read and display
    let content = fs::read_to_string(&path).into_diagnostic()?;

    match args.format {
        OutputFormat::Yaml | OutputFormat::Auto => {
            print!("{}", content);
        }
        OutputFormat::Json => {
            let quote: Quote = serde_yml::from_str(&content).into_diagnostic()?;
            let json = serde_json::to_string_pretty(&quote).into_diagnostic()?;
            println!("{}", json);
        }
        _ => {
            print!("{}", content);
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

    // Find the quote file
    let quote_dir = project.root().join("bom/quotes");
    let mut found_path = None;

    if quote_dir.exists() {
        for entry in fs::read_dir(&quote_dir).into_diagnostic()? {
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

    let path = found_path.ok_or_else(|| miette::miette!("No quote found matching '{}'", args.id))?;

    // Open in editor
    let editor = config.editor();
    std::process::Command::new(&editor)
        .arg(&path)
        .status()
        .into_diagnostic()?;

    Ok(())
}

fn run_compare(args: CompareArgs) -> Result<()> {
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

        if path.extension().map_or(false, |e| e == "yaml") {
            let content = fs::read_to_string(&path).into_diagnostic()?;
            if let Ok(quote) = serde_yml::from_str::<Quote>(&content) {
                // Check if quote matches either component or assembly
                let matches = quote
                    .component
                    .as_ref()
                    .map_or(false, |c| c.contains(&item))
                    || quote
                        .assembly
                        .as_ref()
                        .map_or(false, |a| a.contains(&item));
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
    let _ = short_ids.save(&project);

    // Output comparison
    let format = if args.format == OutputFormat::Auto {
        OutputFormat::Tsv
    } else {
        args.format
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
        OutputFormat::Tsv | OutputFormat::Auto => {
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

// Helper functions

fn format_short_id(id: &EntityId) -> String {
    let id_str = id.to_string();
    if id_str.len() > 13 {
        format!("{}...", &id_str[..13])
    } else {
        id_str
    }
}

fn truncate_str(s: &str, max_len: usize) -> &str {
    if s.len() > max_len {
        &s[..max_len]
    } else {
        s
    }
}

fn escape_csv(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}
