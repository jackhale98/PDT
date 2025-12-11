//! `tdt sup` command - Supplier management

use clap::{Subcommand, ValueEnum};
use console::style;
use miette::{IntoDiagnostic, Result};
use std::fs;

use crate::cli::{GlobalOpts, OutputFormat};
use crate::core::identity::{EntityId, EntityPrefix};
use crate::core::project::Project;
use crate::core::shortid::ShortIdIndex;
use crate::core::Config;
use crate::entities::supplier::{Capability, Supplier};
use crate::schema::template::{TemplateContext, TemplateGenerator};
use crate::schema::wizard::SchemaWizard;

#[derive(Subcommand, Debug)]
pub enum SupCommands {
    /// List suppliers with filtering
    List(ListArgs),

    /// Create a new supplier
    New(NewArgs),

    /// Show a supplier's details
    Show(ShowArgs),

    /// Edit a supplier in your editor
    Edit(EditArgs),
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

/// Capability filter
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CapabilityFilter {
    Machining,
    SheetMetal,
    Casting,
    Injection,
    Extrusion,
    Pcb,
    PcbAssembly,
    CableAssembly,
    Assembly,
    Testing,
    Finishing,
    Packaging,
    All,
}

/// Columns to display in list output
#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
pub enum ListColumn {
    Id,
    Name,
    ShortName,
    Status,
    Website,
    Capabilities,
    Author,
    Created,
}

impl std::fmt::Display for ListColumn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ListColumn::Id => write!(f, "id"),
            ListColumn::Name => write!(f, "name"),
            ListColumn::ShortName => write!(f, "short-name"),
            ListColumn::Status => write!(f, "status"),
            ListColumn::Website => write!(f, "website"),
            ListColumn::Capabilities => write!(f, "capabilities"),
            ListColumn::Author => write!(f, "author"),
            ListColumn::Created => write!(f, "created"),
        }
    }
}

#[derive(clap::Args, Debug)]
pub struct ListArgs {
    /// Filter by status
    #[arg(long, short = 's', default_value = "all")]
    pub status: StatusFilter,

    /// Filter by capability
    #[arg(long, short = 'c', default_value = "all")]
    pub capability: CapabilityFilter,

    /// Search in name and notes
    #[arg(long)]
    pub search: Option<String>,

    /// Filter by author (substring match)
    #[arg(long, short = 'a')]
    pub author: Option<String>,

    /// Show suppliers created in last N days
    #[arg(long)]
    pub recent: Option<u32>,

    /// Columns to display (can specify multiple)
    #[arg(long, value_delimiter = ',', default_values_t = vec![
        ListColumn::Id,
        ListColumn::Name,
        ListColumn::ShortName,
        ListColumn::Status,
        ListColumn::Capabilities
    ])]
    pub columns: Vec<ListColumn>,

    /// Sort by field
    #[arg(long, default_value = "name")]
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

#[derive(clap::Args, Debug)]
pub struct NewArgs {
    /// Supplier name (required)
    #[arg(long, short = 'n')]
    pub name: Option<String>,

    /// Short name for display
    #[arg(long)]
    pub short_name: Option<String>,

    /// Website URL
    #[arg(long, short = 'w')]
    pub website: Option<String>,

    /// Payment terms
    #[arg(long)]
    pub payment_terms: Option<String>,

    /// Notes
    #[arg(long)]
    pub notes: Option<String>,

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
    /// Supplier ID or short ID (SUP@N)
    pub id: String,
}

#[derive(clap::Args, Debug)]
pub struct EditArgs {
    /// Supplier ID or short ID (SUP@N)
    pub id: String,
}

/// Run a supplier subcommand
pub fn run(cmd: SupCommands, global: &GlobalOpts) -> Result<()> {
    match cmd {
        SupCommands::List(args) => run_list(args, global),
        SupCommands::New(args) => run_new(args),
        SupCommands::Show(args) => run_show(args, global),
        SupCommands::Edit(args) => run_edit(args),
    }
}

fn run_list(args: ListArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let sup_dir = project.root().join("bom/suppliers");

    if !sup_dir.exists() {
        if args.count {
            println!("0");
        } else {
            println!("No suppliers found.");
        }
        return Ok(());
    }

    // Load and parse all suppliers
    let mut suppliers: Vec<Supplier> = Vec::new();

    for entry in fs::read_dir(&sup_dir).into_diagnostic()? {
        let entry = entry.into_diagnostic()?;
        let path = entry.path();

        if path.extension().map_or(false, |e| e == "yaml") {
            let content = fs::read_to_string(&path).into_diagnostic()?;
            if let Ok(sup) = serde_yml::from_str::<Supplier>(&content) {
                suppliers.push(sup);
            }
        }
    }

    // Apply filters
    let suppliers: Vec<Supplier> = suppliers
        .into_iter()
        .filter(|s| match args.status {
            StatusFilter::Draft => s.status == crate::core::entity::Status::Draft,
            StatusFilter::Review => s.status == crate::core::entity::Status::Review,
            StatusFilter::Approved => s.status == crate::core::entity::Status::Approved,
            StatusFilter::Released => s.status == crate::core::entity::Status::Released,
            StatusFilter::Obsolete => s.status == crate::core::entity::Status::Obsolete,
            StatusFilter::All => true,
        })
        .filter(|s| match args.capability {
            CapabilityFilter::Machining => s.capabilities.contains(&Capability::Machining),
            CapabilityFilter::SheetMetal => s.capabilities.contains(&Capability::SheetMetal),
            CapabilityFilter::Casting => s.capabilities.contains(&Capability::Casting),
            CapabilityFilter::Injection => s.capabilities.contains(&Capability::Injection),
            CapabilityFilter::Extrusion => s.capabilities.contains(&Capability::Extrusion),
            CapabilityFilter::Pcb => s.capabilities.contains(&Capability::Pcb),
            CapabilityFilter::PcbAssembly => s.capabilities.contains(&Capability::PcbAssembly),
            CapabilityFilter::CableAssembly => s.capabilities.contains(&Capability::CableAssembly),
            CapabilityFilter::Assembly => s.capabilities.contains(&Capability::Assembly),
            CapabilityFilter::Testing => s.capabilities.contains(&Capability::Testing),
            CapabilityFilter::Finishing => s.capabilities.contains(&Capability::Finishing),
            CapabilityFilter::Packaging => s.capabilities.contains(&Capability::Packaging),
            CapabilityFilter::All => true,
        })
        .filter(|s| {
            if let Some(ref search) = args.search {
                let search_lower = search.to_lowercase();
                s.name.to_lowercase().contains(&search_lower)
                    || s.short_name
                        .as_ref()
                        .map_or(false, |n| n.to_lowercase().contains(&search_lower))
                    || s.notes
                        .as_ref()
                        .map_or(false, |n| n.to_lowercase().contains(&search_lower))
            } else {
                true
            }
        })
        .filter(|s| {
            args.author.as_ref().map_or(true, |author| {
                s.author.to_lowercase().contains(&author.to_lowercase())
            })
        })
        .filter(|s| {
            args.recent.map_or(true, |days| {
                let cutoff = chrono::Utc::now() - chrono::Duration::days(days as i64);
                s.created >= cutoff
            })
        })
        .collect();

    // Sort
    let mut suppliers = suppliers;
    match args.sort {
        ListColumn::Id => suppliers.sort_by(|a, b| a.id.to_string().cmp(&b.id.to_string())),
        ListColumn::Name => suppliers.sort_by(|a, b| a.name.cmp(&b.name)),
        ListColumn::ShortName => suppliers.sort_by(|a, b| a.short_name.cmp(&b.short_name)),
        ListColumn::Status => {
            suppliers.sort_by(|a, b| format!("{:?}", a.status).cmp(&format!("{:?}", b.status)))
        }
        ListColumn::Website => suppliers.sort_by(|a, b| a.website.cmp(&b.website)),
        ListColumn::Capabilities => suppliers.sort_by(|a, b| a.capabilities.len().cmp(&b.capabilities.len())),
        ListColumn::Author => suppliers.sort_by(|a, b| a.author.cmp(&b.author)),
        ListColumn::Created => suppliers.sort_by(|a, b| a.created.cmp(&b.created)),
    }

    if args.reverse {
        suppliers.reverse();
    }

    // Apply limit
    if let Some(limit) = args.limit {
        suppliers.truncate(limit);
    }

    // Count only
    if args.count {
        println!("{}", suppliers.len());
        return Ok(());
    }

    // No results
    if suppliers.is_empty() {
        println!("No suppliers found.");
        return Ok(());
    }

    // Update short ID index
    let mut short_ids = ShortIdIndex::load(&project);
    short_ids.ensure_all(suppliers.iter().map(|s| s.id.to_string()));
    let _ = short_ids.save(&project);

    // Output based on format
    let format = match global.format {
        OutputFormat::Auto => OutputFormat::Tsv,
        f => f,
    };

    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&suppliers).into_diagnostic()?;
            println!("{}", json);
        }
        OutputFormat::Yaml => {
            let yaml = serde_yml::to_string(&suppliers).into_diagnostic()?;
            print!("{}", yaml);
        }
        OutputFormat::Csv => {
            println!("short_id,id,name,short_name,website,status,capabilities");
            for sup in &suppliers {
                let short_id = short_ids.get_short_id(&sup.id.to_string()).unwrap_or_default();
                let caps: Vec<_> = sup.capabilities.iter().map(|c| c.to_string()).collect();
                println!(
                    "{},{},{},{},{},{},\"{}\"",
                    short_id,
                    sup.id,
                    escape_csv(&sup.name),
                    sup.short_name.as_deref().unwrap_or(""),
                    sup.website.as_deref().unwrap_or(""),
                    sup.status,
                    caps.join(";")
                );
            }
        }
        OutputFormat::Tsv => {
            // Build header based on selected columns
            let mut header_parts = vec![format!("{:<8}", style("SHORT").bold().dim())];
            for col in &args.columns {
                let header = match col {
                    ListColumn::Id => format!("{:<17}", style("ID").bold()),
                    ListColumn::Name => format!("{:<25}", style("NAME").bold()),
                    ListColumn::ShortName => format!("{:<12}", style("SHORT").bold()),
                    ListColumn::Status => format!("{:<10}", style("STATUS").bold()),
                    ListColumn::Website => format!("{:<25}", style("WEBSITE").bold()),
                    ListColumn::Capabilities => format!("{:<20}", style("CAPABILITIES").bold()),
                    ListColumn::Author => format!("{:<14}", style("AUTHOR").bold()),
                    ListColumn::Created => format!("{:<12}", style("CREATED").bold()),
                };
                header_parts.push(header);
            }
            println!("{}", header_parts.join(" "));
            println!("{}", "-".repeat(95));

            for sup in &suppliers {
                let short_id = short_ids.get_short_id(&sup.id.to_string()).unwrap_or_default();
                let mut row_parts = vec![format!("{:<8}", style(&short_id).cyan())];

                for col in &args.columns {
                    let value = match col {
                        ListColumn::Id => format!("{:<17}", format_short_id(&sup.id)),
                        ListColumn::Name => format!("{:<25}", truncate_str(&sup.name, 23)),
                        ListColumn::ShortName => format!("{:<12}", truncate_str(sup.short_name.as_deref().unwrap_or("-"), 10)),
                        ListColumn::Status => format!("{:<10}", sup.status),
                        ListColumn::Website => format!("{:<25}", truncate_str(sup.website.as_deref().unwrap_or("-"), 23)),
                        ListColumn::Capabilities => {
                            let caps: Vec<_> = sup.capabilities.iter().take(2).map(|c| c.to_string()).collect();
                            let caps_display = if sup.capabilities.len() > 2 {
                                format!("{}+{}", caps.join(","), sup.capabilities.len() - 2)
                            } else {
                                caps.join(",")
                            };
                            format!("{:<20}", caps_display)
                        }
                        ListColumn::Author => format!("{:<14}", truncate_str(&sup.author, 12)),
                        ListColumn::Created => format!("{:<12}", sup.created.format("%Y-%m-%d")),
                    };
                    row_parts.push(value);
                }
                println!("{}", row_parts.join(" "));
            }

            println!();
            println!(
                "{} supplier(s) found. Use {} to reference by short ID.",
                style(suppliers.len()).cyan(),
                style("SUP@N").cyan()
            );
        }
        OutputFormat::Id => {
            for sup in &suppliers {
                println!("{}", sup.id);
            }
        }
        OutputFormat::Md => {
            println!("| Short | ID | Name | Short | Status | Capabilities |");
            println!("|---|---|---|---|---|---|");
            for sup in &suppliers {
                let short_id = short_ids.get_short_id(&sup.id.to_string()).unwrap_or_default();
                let caps: Vec<_> = sup.capabilities.iter().map(|c| c.to_string()).collect();
                println!(
                    "| {} | {} | {} | {} | {} | {} |",
                    short_id,
                    format_short_id(&sup.id),
                    sup.name,
                    sup.short_name.as_deref().unwrap_or("-"),
                    sup.status,
                    caps.join(", ")
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

    let name: String;

    if args.interactive {
        let wizard = SchemaWizard::new();
        let result = wizard.run(EntityPrefix::Sup)?;

        name = result
            .get_string("name")
            .map(String::from)
            .unwrap_or_else(|| "New Supplier".to_string());
    } else {
        name = args.name.unwrap_or_else(|| "New Supplier".to_string());
    }

    // Generate ID
    let id = EntityId::new(EntityPrefix::Sup);

    // Generate template
    let generator = TemplateGenerator::new().map_err(|e| miette::miette!("{}", e))?;
    let ctx = TemplateContext::new(id.clone(), config.author())
        .with_title(&name);

    let ctx = if let Some(ref short) = args.short_name {
        ctx.with_short_name(short)
    } else {
        ctx
    };

    let ctx = if let Some(ref website) = args.website {
        ctx.with_website(website)
    } else {
        ctx
    };

    let ctx = if let Some(ref terms) = args.payment_terms {
        ctx.with_payment_terms(terms)
    } else {
        ctx
    };

    let ctx = if let Some(ref notes) = args.notes {
        ctx.with_notes(notes)
    } else {
        ctx
    };

    let yaml_content = generator
        .generate_supplier(&ctx)
        .map_err(|e| miette::miette!("{}", e))?;

    // Write file
    let output_dir = project.root().join("bom/suppliers");
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
        "{} Created supplier {}",
        style("✓").green(),
        style(short_id.unwrap_or_else(|| format_short_id(&id))).cyan()
    );
    println!("   {}", style(file_path.display()).dim());
    println!("   Name: {}", style(&name).yellow());

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

    // Find the supplier file
    let sup_dir = project.root().join("bom/suppliers");
    let mut found_path = None;

    if sup_dir.exists() {
        for entry in fs::read_dir(&sup_dir).into_diagnostic()? {
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

    let path = found_path.ok_or_else(|| miette::miette!("No supplier found matching '{}'", args.id))?;

    // Read and display
    let content = fs::read_to_string(&path).into_diagnostic()?;

    let format = match global.format {
        OutputFormat::Auto => OutputFormat::Yaml,
        f => f,
    };

    match format {
        OutputFormat::Yaml => {
            print!("{}", content);
        }
        OutputFormat::Json => {
            let sup: Supplier = serde_yml::from_str(&content).into_diagnostic()?;
            let json = serde_json::to_string_pretty(&sup).into_diagnostic()?;
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

    // Find the supplier file
    let sup_dir = project.root().join("bom/suppliers");
    let mut found_path = None;

    if sup_dir.exists() {
        for entry in fs::read_dir(&sup_dir).into_diagnostic()? {
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

    let path = found_path.ok_or_else(|| miette::miette!("No supplier found matching '{}'", args.id))?;

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
