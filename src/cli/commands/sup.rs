//! `tdt sup` command - Supplier management

use clap::{Subcommand, ValueEnum};
use console::style;
use miette::{IntoDiagnostic, Result};
use std::fs;

use crate::cli::filters::StatusFilter;
use crate::cli::table::{CellValue, ColumnDef, TableConfig, TableFormatter, TableRow};
use crate::cli::{GlobalOpts, OutputFormat};
use crate::core::cache::EntityCache;
use crate::core::identity::{EntityId, EntityPrefix};
use crate::core::project::Project;
use crate::core::shortid::ShortIdIndex;
use crate::core::CachedSupplier;
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

    /// Delete a supplier
    Delete(DeleteArgs),

    /// Archive a supplier (soft delete)
    Archive(ArchiveArgs),
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

/// Column definitions for supplier list output
const SUP_COLUMNS: &[ColumnDef] = &[
    ColumnDef::new("id", "ID", 17),
    ColumnDef::new("name", "NAME", 25),
    ColumnDef::new("short-name", "SHORT NAME", 12),
    ColumnDef::new("status", "STATUS", 10),
    ColumnDef::new("website", "WEBSITE", 25),
    ColumnDef::new("capabilities", "CAPABILITIES", 20),
    ColumnDef::new("author", "AUTHOR", 14),
    ColumnDef::new("created", "CREATED", 12),
];

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

    /// Wrap text at specified width (for mobile-friendly display)
    #[arg(long, short = 'w')]
    pub wrap: Option<usize>,

    /// Show full ID column (hidden by default since SHORT is always shown)
    #[arg(long)]
    pub show_id: bool,
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

#[derive(clap::Args, Debug)]
pub struct DeleteArgs {
    /// Supplier ID or short ID (SUP@N)
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
    /// Supplier ID or short ID (SUP@N)
    pub id: String,

    /// Force archive even if other entities reference this one
    #[arg(long)]
    pub force: bool,

    /// Suppress output
    #[arg(long, short = 'q')]
    pub quiet: bool,
}

/// Directories where suppliers are stored
const SUPPLIER_DIRS: &[&str] = &["bom/suppliers"];

/// Entity configuration for suppliers
const ENTITY_CONFIG: crate::cli::EntityConfig = crate::cli::EntityConfig {
    prefix: EntityPrefix::Sup,
    dirs: SUPPLIER_DIRS,
    name: "supplier",
    name_plural: "suppliers",
};

/// Run a supplier subcommand
pub fn run(cmd: SupCommands, global: &GlobalOpts) -> Result<()> {
    match cmd {
        SupCommands::List(args) => run_list(args, global),
        SupCommands::New(args) => run_new(args, global),
        SupCommands::Show(args) => run_show(args, global),
        SupCommands::Edit(args) => run_edit(args),
        SupCommands::Delete(args) => run_delete(args),
        SupCommands::Archive(args) => run_archive(args),
    }
}

fn run_list(args: ListArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;

    // Open cache (auto-syncs if files changed)
    let cache = EntityCache::open(&project)?;

    // Convert filters to cache-compatible format
    let status_filter = crate::cli::entity_cmd::status_filter_to_str(args.status);

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

    // Apply post-filters that cache doesn't support
    let mut suppliers: Vec<_> = suppliers
        .into_iter()
        .filter(|s| {
            // Active filter: exclude obsolete
            if !crate::cli::entity_cmd::status_enum_matches_filter(&s.status, args.status) {
                return false;
            }
            // Recent filter
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
    super::utils::save_short_ids(&mut short_ids, &project);

    // Output based on format
    let format = match global.output {
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
            let rows: Vec<TableRow> = suppliers
                .iter()
                .map(|sup| cached_supplier_to_row(sup, &short_ids))
                .collect();

            // Configure table
            let config = if let Some(width) = args.wrap {
                TableConfig::with_wrap(width)
            } else {
                TableConfig::default()
            };

            let formatter = TableFormatter::new(SUP_COLUMNS, "supplier", "SUP").with_config(config);
            formatter.output(rows, format, &visible);
        }
        OutputFormat::Auto | OutputFormat::Path => unreachable!(),
    }

    Ok(())
}

/// Convert a cached supplier to a TableRow
fn cached_supplier_to_row(sup: &CachedSupplier, short_ids: &ShortIdIndex) -> TableRow {
    // Format capabilities display
    let caps_display = if sup.capabilities.len() > 2 {
        let first_two: Vec<_> = sup.capabilities.iter().take(2).cloned().collect();
        format!("{}+{}", first_two.join(","), sup.capabilities.len() - 2)
    } else {
        sup.capabilities.join(",")
    };

    TableRow::new(sup.id.clone(), short_ids)
        .cell("id", CellValue::Id(sup.id.clone()))
        .cell("name", CellValue::Text(sup.name.clone()))
        .cell(
            "short-name",
            CellValue::Text(sup.short_name.clone().unwrap_or_else(|| "-".to_string())),
        )
        .cell("status", CellValue::Status(sup.status))
        .cell(
            "website",
            CellValue::Text(sup.website.clone().unwrap_or_else(|| "-".to_string())),
        )
        .cell("capabilities", CellValue::Text(caps_display))
        .cell("author", CellValue::Text(sup.author.clone()))
        .cell("created", CellValue::Date(sup.created))
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
    super::utils::save_short_ids(&mut short_ids, &project);

    // Handle --link flags
    let added_links = crate::cli::entity_cmd::process_link_flags(
        &file_path,
        EntityPrefix::Sup,
        &args.link,
        &short_ids,
    );

    // Output based on format flag
    crate::cli::entity_cmd::output_new_entity(
        &id,
        &file_path,
        short_id.clone(),
        ENTITY_CONFIG.name,
        &name,
        Some(&format!("Name: {}", style(&name).yellow())),
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

    match global.output {
        OutputFormat::Yaml => {
            print!("{}", content);
        }
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&sup).into_diagnostic()?;
            println!("{}", json);
        }
        OutputFormat::Id
        | OutputFormat::ShortId
        | OutputFormat::Table
        | OutputFormat::Dot
        | OutputFormat::Tree => {
            if global.output == OutputFormat::ShortId {
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
    crate::cli::entity_cmd::run_edit_generic(&args.id, &ENTITY_CONFIG)
}

fn run_delete(args: DeleteArgs) -> Result<()> {
    crate::cli::commands::utils::run_delete(&args.id, SUPPLIER_DIRS, args.force, false, args.quiet)
}

fn run_archive(args: ArchiveArgs) -> Result<()> {
    crate::cli::commands::utils::run_delete(&args.id, SUPPLIER_DIRS, args.force, true, args.quiet)
}
