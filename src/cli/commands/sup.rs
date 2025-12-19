//! `tdt sup` command - Supplier management

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
use crate::core::links::add_inferred_link;
use crate::core::Config;
use crate::entities::supplier::Supplier;
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

    /// Link to another entity (auto-infers link type)
    #[arg(long, short = 'L')]
    pub link: Vec<String>,
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
        SupCommands::New(args) => run_new(args, global),
        SupCommands::Show(args) => run_show(args, global),
        SupCommands::Edit(args) => run_edit(args),
    }
}

fn run_list(args: ListArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;

    // Open cache (auto-syncs if files changed)
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

    let capability_filter = match args.capability {
        CapabilityFilter::Machining => Some("machining"),
        CapabilityFilter::SheetMetal => Some("sheet_metal"),
        CapabilityFilter::Casting => Some("casting"),
        CapabilityFilter::Injection => Some("injection"),
        CapabilityFilter::Extrusion => Some("extrusion"),
        CapabilityFilter::Pcb => Some("pcb"),
        CapabilityFilter::PcbAssembly => Some("pcb_assembly"),
        CapabilityFilter::CableAssembly => Some("cable_assembly"),
        CapabilityFilter::Assembly => Some("assembly"),
        CapabilityFilter::Testing => Some("testing"),
        CapabilityFilter::Finishing => Some("finishing"),
        CapabilityFilter::Packaging => Some("packaging"),
        CapabilityFilter::All => None,
    };

    // Query from cache with filters (applies indexed SQL queries)
    let suppliers = cache.list_suppliers(
        status_filter,
        capability_filter,
        args.author.as_deref(),
        args.search.as_deref(),
        None, // We'll apply limit after sorting
    );

    // Apply post-filters that cache doesn't support (recent filter)
    let mut suppliers: Vec<_> = suppliers
        .into_iter()
        .filter(|s| {
            args.recent.is_none_or(|days| {
                let cutoff = chrono::Utc::now() - chrono::Duration::days(days as i64);
                s.created >= cutoff
            })
        })
        .collect();

    // Sort
    match args.sort {
        ListColumn::Id => suppliers.sort_by(|a, b| a.id.cmp(&b.id)),
        ListColumn::Name => suppliers.sort_by(|a, b| a.name.cmp(&b.name)),
        ListColumn::ShortName => suppliers.sort_by(|a, b| a.short_name.cmp(&b.short_name)),
        ListColumn::Status => suppliers.sort_by(|a, b| a.status.cmp(&b.status)),
        ListColumn::Website => suppliers.sort_by(|a, b| a.website.cmp(&b.website)),
        ListColumn::Capabilities => {
            suppliers.sort_by(|a, b| a.capabilities.len().cmp(&b.capabilities.len()))
        }
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
    short_ids.ensure_all(suppliers.iter().map(|s| s.id.clone()));
    let _ = short_ids.save(&project);

    // Output based on format
    let format = match global.format {
        OutputFormat::Auto => OutputFormat::Tsv,
        f => f,
    };

    match format {
        OutputFormat::Json | OutputFormat::Yaml => {
            // For full fidelity output, load complete entities from files
            let full_suppliers: Vec<Supplier> = suppliers
                .iter()
                .filter_map(|s| {
                    let full_path = project.root().join(&s.file_path);
                    fs::read_to_string(&full_path)
                        .ok()
                        .and_then(|content| serde_yml::from_str(&content).ok())
                })
                .collect();

            if format == OutputFormat::Json {
                let json = serde_json::to_string_pretty(&full_suppliers).into_diagnostic()?;
                println!("{}", json);
            } else {
                let yaml = serde_yml::to_string(&full_suppliers).into_diagnostic()?;
                print!("{}", yaml);
            }
        }
        OutputFormat::Csv => {
            println!("short_id,id,name,short_name,website,status,capabilities");
            for sup in &suppliers {
                let short_id = short_ids.get_short_id(&sup.id).unwrap_or_default();
                println!(
                    "{},{},{},{},{},{},\"{}\"",
                    short_id,
                    sup.id,
                    escape_csv(&sup.name),
                    sup.short_name.as_deref().unwrap_or(""),
                    sup.website.as_deref().unwrap_or(""),
                    sup.status,
                    sup.capabilities.join(";")
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
                let short_id = short_ids.get_short_id(&sup.id).unwrap_or_default();
                let mut row_parts = vec![format!("{:<8}", style(&short_id).cyan())];

                for col in &args.columns {
                    let value = match col {
                        ListColumn::Id => format!("{:<17}", truncate_str(&sup.id, 15)),
                        ListColumn::Name => format!("{:<25}", truncate_str(&sup.name, 23)),
                        ListColumn::ShortName => format!(
                            "{:<12}",
                            truncate_str(sup.short_name.as_deref().unwrap_or("-"), 10)
                        ),
                        ListColumn::Status => format!("{:<10}", sup.status),
                        ListColumn::Website => format!(
                            "{:<25}",
                            truncate_str(sup.website.as_deref().unwrap_or("-"), 23)
                        ),
                        ListColumn::Capabilities => {
                            let caps: Vec<_> = sup.capabilities.iter().take(2).cloned().collect();
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
        OutputFormat::Id | OutputFormat::ShortId => {
            for sup in &suppliers {
                if format == OutputFormat::ShortId {
                    let short_id = short_ids.get_short_id(&sup.id).unwrap_or_default();
                    println!("{}", short_id);
                } else {
                    println!("{}", sup.id);
                }
            }
        }
        OutputFormat::Md => {
            println!("| Short | ID | Name | Short | Status | Capabilities |");
            println!("|---|---|---|---|---|---|");
            for sup in &suppliers {
                let short_id = short_ids.get_short_id(&sup.id).unwrap_or_default();
                println!(
                    "| {} | {} | {} | {} | {} | {} |",
                    short_id,
                    truncate_str(&sup.id, 15),
                    sup.name,
                    sup.short_name.as_deref().unwrap_or("-"),
                    sup.status,
                    sup.capabilities.join(", ")
                );
            }
        }
        OutputFormat::Auto | OutputFormat::Path => unreachable!(),
    }

    Ok(())
}

fn run_new(args: NewArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let config = Config::load();

    let name: String;
    let short_name: Option<String>;
    let website: Option<String>;
    let payment_terms: Option<String>;
    let notes: Option<String>;

    if args.interactive {
        let wizard = SchemaWizard::new();
        let result = wizard.run(EntityPrefix::Sup)?;

        name = result
            .get_string("name")
            .map(String::from)
            .unwrap_or_else(|| "New Supplier".to_string());

        // Extract additional fields from wizard
        short_name = result.get_string("short_name").map(String::from);
        website = result.get_string("website").map(String::from);
        payment_terms = result.get_string("payment_terms").map(String::from);
        notes = result.get_string("notes").map(String::from);
    } else {
        name = args.name.unwrap_or_else(|| "New Supplier".to_string());
        short_name = args.short_name.clone();
        website = args.website.clone();
        payment_terms = args.payment_terms.clone();
        notes = args.notes.clone();
    }

    // Generate ID
    let id = EntityId::new(EntityPrefix::Sup);

    // Generate template
    let generator = TemplateGenerator::new().map_err(|e| miette::miette!("{}", e))?;
    let ctx = TemplateContext::new(id.clone(), config.author()).with_title(&name);

    let ctx = if let Some(ref short) = short_name {
        ctx.with_short_name(short)
    } else {
        ctx
    };

    let ctx = if let Some(ref w) = website {
        ctx.with_website(w)
    } else {
        ctx
    };

    let ctx = if let Some(ref terms) = payment_terms {
        ctx.with_payment_terms(terms)
    } else {
        ctx
    };

    let ctx = if let Some(ref n) = notes {
        ctx.with_notes(n)
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

    // Handle --link flags
    let mut added_links = Vec::new();
    for link_target in &args.link {
        let resolved_target = short_ids
            .resolve(link_target)
            .unwrap_or_else(|| link_target.clone());

        if let Ok(target_entity_id) = EntityId::parse(&resolved_target) {
            match add_inferred_link(
                &file_path,
                EntityPrefix::Sup,
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
            eprintln!(
                "{} Invalid entity ID: {}",
                style("!").yellow(),
                link_target
            );
        }
    }

    // Output based on format flag
    match global.format {
        OutputFormat::Id => {
            println!("{}", id);
        }
        OutputFormat::ShortId => {
            println!("{}", short_id.clone().unwrap_or_else(|| format_short_id(&id)));
        }
        OutputFormat::Path => {
            println!("{}", file_path.display());
        }
        _ => {
            println!(
                "{} Created supplier {}",
                style("✓").green(),
                style(short_id.clone().unwrap_or_else(|| format_short_id(&id))).cyan()
            );
            println!("   {}", style(file_path.display()).dim());
            println!("   Name: {}", style(&name).yellow());

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
        found_path.ok_or_else(|| miette::miette!("No supplier found matching '{}'", args.id))?;

    // Read and parse supplier
    let content = fs::read_to_string(&path).into_diagnostic()?;
    let sup: Supplier = serde_yml::from_str(&content).into_diagnostic()?;

    match global.format {
        OutputFormat::Yaml => {
            print!("{}", content);
        }
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&sup).into_diagnostic()?;
            println!("{}", json);
        }
        OutputFormat::Id | OutputFormat::ShortId => {
            if global.format == OutputFormat::ShortId {
                let short_ids = ShortIdIndex::load(&project);
                let short_id = short_ids
                    .get_short_id(&sup.id.to_string())
                    .unwrap_or_default();
                println!("{}", short_id);
            } else {
                println!("{}", sup.id);
            }
        }
        _ => {
            // Pretty format (default)
            println!("{}", style("─".repeat(60)).dim());
            println!(
                "{}: {}",
                style("ID").bold(),
                style(&sup.id.to_string()).cyan()
            );
            println!("{}: {}", style("Name").bold(), style(&sup.name).yellow());
            println!("{}: {}", style("Status").bold(), sup.status);
            println!("{}", style("─".repeat(60)).dim());

            // Contact Info
            if !sup.contacts.is_empty() {
                println!();
                println!("{} ({}):", style("Contacts").bold(), sup.contacts.len());
                for contact in &sup.contacts {
                    let primary = if contact.primary { " (primary)" } else { "" };
                    print!("  • {}", contact.name);
                    if let Some(ref role) = contact.role {
                        print!(" - {}", role);
                    }
                    println!("{}", primary);
                    if let Some(ref email) = contact.email {
                        println!("    Email: {}", email);
                    }
                    if let Some(ref phone) = contact.phone {
                        println!("    Phone: {}", phone);
                    }
                }
            }

            // Addresses
            if !sup.addresses.is_empty() {
                println!();
                println!("{} ({}):", style("Addresses").bold(), sup.addresses.len());
                for addr in &sup.addresses {
                    print!("  • {:?}", addr.address_type);
                    if let Some(ref city) = addr.city {
                        print!(": {}", city);
                    }
                    if let Some(ref country) = addr.country {
                        print!(", {}", country);
                    }
                    println!();
                }
            }

            // Capabilities
            if !sup.capabilities.is_empty() {
                println!();
                let cap_strs: Vec<String> =
                    sup.capabilities.iter().map(|c| c.to_string()).collect();
                println!("{}: {}", style("Capabilities").bold(), cap_strs.join(", "));
            }

            // Certifications
            if !sup.certifications.is_empty() {
                println!();
                println!(
                    "{} ({}):",
                    style("Certifications").bold(),
                    sup.certifications.len()
                );
                for cert in &sup.certifications {
                    print!("  • {}", cert.name);
                    if let Some(expiry) = cert.expiry {
                        print!(" (expires: {})", expiry);
                    }
                    println!();
                }
            }

            // Tags
            if !sup.tags.is_empty() {
                println!();
                println!("{}: {}", style("Tags").bold(), sup.tags.join(", "));
            }

            // Notes
            if let Some(ref notes) = sup.notes {
                if !notes.is_empty() && !notes.starts_with('#') {
                    println!();
                    println!("{}", style("Notes:").bold());
                    println!("{}", notes);
                }
            }

            // Footer
            println!("{}", style("─".repeat(60)).dim());
            println!(
                "{}: {} | {}: {} | {}: {}",
                style("Author").dim(),
                sup.author,
                style("Created").dim(),
                sup.created.format("%Y-%m-%d %H:%M"),
                style("Revision").dim(),
                sup.entity_revision
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

    // Find the supplier file
    let sup_dir = project.root().join("bom/suppliers");
    let mut found_path = None;

    if sup_dir.exists() {
        for entry in fs::read_dir(&sup_dir).into_diagnostic()? {
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
        found_path.ok_or_else(|| miette::miette!("No supplier found matching '{}'", args.id))?;

    println!(
        "Opening {} in {}...",
        style(path.display()).cyan(),
        style(config.editor()).yellow()
    );

    config.run_editor(&path).into_diagnostic()?;

    Ok(())
}
