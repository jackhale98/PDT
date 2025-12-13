//! `tdt cmp` command - Component management

use clap::{Subcommand, ValueEnum};
use console::style;
use miette::{IntoDiagnostic, Result};
use std::fs;

use crate::cli::{GlobalOpts, OutputFormat};
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
}

/// Make/buy filter
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum MakeBuyFilter {
    Make,
    Buy,
    All,
}

/// Category filter
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CategoryFilter {
    Mechanical,
    Electrical,
    Software,
    Fastener,
    Consumable,
    All,
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
    pub make_buy: String,

    /// Component category
    #[arg(long, short = 'c', default_value = "mechanical")]
    pub category: String,

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

/// Run a component subcommand
pub fn run(cmd: CmpCommands, global: &GlobalOpts) -> Result<()> {
    match cmd {
        CmpCommands::List(args) => run_list(args, global),
        CmpCommands::New(args) => run_new(args),
        CmpCommands::Show(args) => run_show(args, global),
        CmpCommands::Edit(args) => run_edit(args),
    }
}

fn run_list(args: ListArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let cmp_dir = project.root().join("bom/components");

    if !cmp_dir.exists() {
        if args.count {
            println!("0");
        } else {
            println!("No components found.");
        }
        return Ok(());
    }

    // Pre-load quotes if needed for no_quote filter
    let quotes: Vec<crate::entities::quote::Quote> = if args.no_quote {
        load_all_quotes(&project)
    } else {
        Vec::new()
    };

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
                c.suppliers.iter().any(|s| {
                    s.lead_time_days.map_or(false, |days| days > threshold)
                })
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
                !quotes.iter().any(|q| {
                    q.component.as_ref().map_or(false, |qc| qc == &cid_str)
                })
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
        ListColumn::MakeBuy => components.sort_by(|a, b| {
            format!("{:?}", a.make_buy).cmp(&format!("{:?}", b.make_buy))
        }),
        ListColumn::Category => components.sort_by(|a, b| {
            format!("{:?}", a.category).cmp(&format!("{:?}", b.category))
        }),
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
                let short_id = short_ids.get_short_id(&cmp.id.to_string()).unwrap_or_default();
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
                let short_id = short_ids.get_short_id(&cmp.id.to_string()).unwrap_or_default();
                let mut row_parts = vec![format!("{:<8}", style(&short_id).cyan())];

                for col in &args.columns {
                    let value = match col {
                        ListColumn::Id => format!("{:<17}", format_short_id(&cmp.id)),
                        ListColumn::PartNumber => format!("{:<12}", truncate_str(&cmp.part_number, 10)),
                        ListColumn::Revision => format!("{:<8}", cmp.revision.as_deref().unwrap_or("-")),
                        ListColumn::Title => format!("{:<30}", truncate_str(&cmp.title, 28)),
                        ListColumn::MakeBuy => format!("{:<6}", match cmp.make_buy {
                            MakeBuy::Make => "make",
                            MakeBuy::Buy => "buy",
                        }),
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
                let short_id = short_ids.get_short_id(&cmp.id.to_string()).unwrap_or_default();
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
        make_buy = args.make_buy;
        category = args.category;
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

    let path = found_path.ok_or_else(|| miette::miette!("No component found matching '{}'", args.id))?;

    // Read and display
    let content = fs::read_to_string(&path).into_diagnostic()?;

    match global.format {
        OutputFormat::Yaml | OutputFormat::Auto => {
            print!("{}", content);
        }
        OutputFormat::Json => {
            let cmp: Component = serde_yml::from_str(&content).into_diagnostic()?;
            let json = serde_json::to_string_pretty(&cmp).into_diagnostic()?;
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

    let path = found_path.ok_or_else(|| miette::miette!("No component found matching '{}'", args.id))?;

    println!("Opening {} in {}...", style(path.display()).cyan(), style(config.editor()).yellow());

    config.run_editor(&path).into_diagnostic()?;

    Ok(())
}

// Helper functions

fn format_short_id(id: &EntityId) -> String {
    let s = id.to_string();
    if s.len() > 16 {
        format!("{}...", &s[..13])
    } else {
        s
    }
}

fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

fn escape_csv(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// Load all quotes from the project
fn load_all_quotes(project: &Project) -> Vec<crate::entities::quote::Quote> {
    let mut quotes = Vec::new();

    let quotes_dir = project.root().join("procurement/quotes");
    if quotes_dir.exists() {
        for entry in walkdir::WalkDir::new(&quotes_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        {
            if let Ok(quote) = crate::yaml::parse_yaml_file::<crate::entities::quote::Quote>(entry.path()) {
                quotes.push(quote);
            }
        }
    }

    quotes
}
