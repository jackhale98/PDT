//! `tdt sup` command - Supplier management

use clap::{Subcommand, ValueEnum};
use console::style;
use miette::{IntoDiagnostic, Result};

use crate::cli::filters::StatusFilter;
use crate::cli::table::{CellValue, ColumnDef, TableConfig, TableFormatter, TableRow};
use crate::cli::{GlobalOpts, OutputFormat};
use tdt_core::core::cache::EntityCache;
use tdt_core::core::identity::EntityPrefix;
use tdt_core::core::project::Project;
use tdt_core::core::shortid::ShortIdIndex;
use tdt_core::core::CachedSupplier;
use tdt_core::core::Config;
use tdt_core::entities::supplier::{Capability, Supplier};
use tdt_core::schema::wizard::SchemaWizard;
use tdt_core::services::{
    CommonFilter, CreateSupplier, SortDirection, SupplierFilter, SupplierService, SupplierSortField,
};

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

    /// Only show entities linked to these IDs (use - for stdin pipe)
    #[arg(long, value_delimiter = ',')]
    pub linked_to: Vec<String>,

    /// Filter by link type when using --linked-to (e.g., verified_by, satisfied_by)
    #[arg(long, requires = "linked_to")]
    pub via: Option<String>,
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

/// Convert CapabilityFilter to Capability
fn capability_filter_to_capability(filter: CapabilityFilter) -> Option<Capability> {
    match filter {
        CapabilityFilter::Machining => Some(Capability::Machining),
        CapabilityFilter::SheetMetal => Some(Capability::SheetMetal),
        CapabilityFilter::Casting => Some(Capability::Casting),
        CapabilityFilter::Injection => Some(Capability::Injection),
        CapabilityFilter::Extrusion => Some(Capability::Extrusion),
        CapabilityFilter::Pcb => Some(Capability::Pcb),
        CapabilityFilter::PcbAssembly => Some(Capability::PcbAssembly),
        CapabilityFilter::CableAssembly => Some(Capability::CableAssembly),
        CapabilityFilter::Assembly => Some(Capability::Assembly),
        CapabilityFilter::Testing => Some(Capability::Testing),
        CapabilityFilter::Finishing => Some(Capability::Finishing),
        CapabilityFilter::Packaging => Some(Capability::Packaging),
        CapabilityFilter::All => None,
    }
}

/// Build a SupplierFilter from CLI list arguments
fn build_sup_filter(args: &ListArgs) -> SupplierFilter {
    SupplierFilter {
        common: CommonFilter {
            status: crate::cli::entity_cmd::status_filter_to_status(args.status).map(|s| vec![s]),
            author: args.author.clone(),
            search: args.search.clone(),
            recent_days: args.recent,
            limit: args.limit,
            ..Default::default()
        },
        capability: capability_filter_to_capability(args.capability),
        ..Default::default()
    }
}

/// Build sort field and direction from CLI arguments
fn build_sup_sort(args: &ListArgs) -> (SupplierSortField, SortDirection) {
    let field = match args.sort {
        ListColumn::Id => SupplierSortField::Id,
        ListColumn::Name => SupplierSortField::Name,
        ListColumn::ShortName => SupplierSortField::ShortName,
        ListColumn::Status => SupplierSortField::Status,
        ListColumn::Website => SupplierSortField::Website,
        ListColumn::Capabilities => SupplierSortField::Capabilities,
        ListColumn::Author => SupplierSortField::Author,
        ListColumn::Created => SupplierSortField::Created,
    };

    let direction = if args.reverse {
        SortDirection::Ascending
    } else {
        SortDirection::Descending
    };

    (field, direction)
}

/// Sort cached suppliers by the specified column
fn sort_cached_suppliers(entities: &mut [CachedSupplier], sort: ListColumn, reverse: bool) {
    entities.sort_by(|a, b| {
        let cmp = match sort {
            ListColumn::Id => a.id.cmp(&b.id),
            ListColumn::Name => a.name.cmp(&b.name),
            ListColumn::ShortName => a.short_name.cmp(&b.short_name),
            ListColumn::Status => a.status.cmp(&b.status),
            ListColumn::Website => a.website.cmp(&b.website),
            ListColumn::Capabilities => a.capabilities.len().cmp(&b.capabilities.len()),
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

/// Output suppliers in the requested format
fn output_suppliers(
    suppliers: &[Supplier],
    short_ids: &mut ShortIdIndex,
    args: &ListArgs,
    format: OutputFormat,
    project: &Project,
) -> Result<()> {
    // Update short ID index
    short_ids.ensure_all(suppliers.iter().map(|s| s.id.to_string()));
    super::utils::save_short_ids(short_ids, project);

    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&suppliers).into_diagnostic()?;
            println!("{}", json);
        }
        OutputFormat::Yaml => {
            let yaml = serde_yml::to_string(&suppliers).into_diagnostic()?;
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
            let rows: Vec<TableRow> = suppliers
                .iter()
                .map(|sup| supplier_to_row(sup, short_ids))
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
        OutputFormat::Auto | OutputFormat::Path => {
            eprintln!("error: -o path is not supported for this view; use -o id to get entity IDs");
            std::process::exit(2);
        }
    }

    Ok(())
}

/// Convert a Supplier to a TableRow
fn supplier_to_row(sup: &Supplier, short_ids: &ShortIdIndex) -> TableRow {
    // Format capabilities display
    let caps_display = if sup.capabilities.len() > 2 {
        let first_two: Vec<_> = sup
            .capabilities
            .iter()
            .take(2)
            .map(|c| c.to_string())
            .collect();
        format!("{}+{}", first_two.join(","), sup.capabilities.len() - 2)
    } else {
        sup.capabilities
            .iter()
            .map(|c| c.to_string())
            .collect::<Vec<_>>()
            .join(",")
    };

    TableRow::new(sup.id.to_string(), short_ids)
        .cell("id", CellValue::Id(sup.id.to_string()))
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
    let cache = EntityCache::open(&project).map_err(|e| miette::miette!("{}", e))?;
    let service = SupplierService::new(&project, &cache);
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
    // - No recent filter (requires date comparison)
    // - Not JSON/YAML output (need full entities)
    let can_use_cache =
        args.recent.is_none() && !matches!(format, OutputFormat::Json | OutputFormat::Yaml);

    if can_use_cache {
        // Fast path: use cached entities via service
        let mut suppliers = service.list_cached();

        // Apply status filter
        if let Some(status) = crate::cli::entity_cmd::status_filter_to_status(args.status) {
            suppliers.retain(|s| s.status == status);
        }

        // Apply capability filter
        if let Some(cap) = capability_filter_to_capability(args.capability) {
            let cap_str = cap.to_string();
            suppliers.retain(|s| s.capabilities.contains(&cap_str));
        }

        // Apply author filter
        if let Some(ref author_filter) = args.author {
            let author_lower = author_filter.to_lowercase();
            suppliers.retain(|s| s.author.to_lowercase().contains(&author_lower));
        }

        // Apply search filter
        if let Some(ref search) = args.search {
            let search_lower = search.to_lowercase();
            suppliers.retain(|s| s.name.to_lowercase().contains(&search_lower));
        }

        // Apply linked-to filter
        if let Some(ref ids) = allowed_ids {
            suppliers.retain(|e| ids.contains(&e.id));
        }

        // Sort
        sort_cached_suppliers(&mut suppliers, args.sort, args.reverse);

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
        short_ids.ensure_all(suppliers.iter().map(|s| s.id.clone()));
        super::utils::save_short_ids(&mut short_ids, &project);

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

        return Ok(());
    }

    // Full entity loading via service
    let filter = build_sup_filter(&args);
    let (sort_field, sort_dir) = build_sup_sort(&args);

    let result = service
        .list(&filter, sort_field, sort_dir)
        .map_err(|e| miette::miette!("{}", e))?;
    let mut suppliers = result.items;

    // Apply linked-to filter
    if let Some(ref ids) = allowed_ids {
        suppliers.retain(|e| ids.contains(&e.id.to_string()));
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

    output_suppliers(&suppliers, &mut short_ids, &args, format, &project)
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
    let cache = EntityCache::open(&project).map_err(|e| miette::miette!("{}", e))?;
    let service = SupplierService::new(&project, &cache);
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

    // Create supplier using service
    let input = CreateSupplier {
        name: name.clone(),
        author: config.author(),
        short_name,
        website,
        payment_terms,
        notes,
        ..Default::default()
    };

    let supplier = service
        .create(input)
        .map_err(|e| miette::miette!("{}", e))?;

    // Get file path for link processing and editor
    let file_path = project
        .root()
        .join("bom/suppliers")
        .join(format!("{}.tdt.yaml", supplier.id));

    // Add to short ID index
    let mut short_ids = ShortIdIndex::load(&project);
    let short_id = short_ids.add(supplier.id.to_string());
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
        &supplier.id,
        &file_path,
        short_id.clone(),
        ENTITY_CONFIG.name,
        &name,
        Some(&format!("Name: {}", style(&name).yellow())),
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

    // Resolve short ID if needed
    let short_ids = ShortIdIndex::load(&project);
    let resolved_id = short_ids
        .resolve(&args.id)
        .unwrap_or_else(|| args.id.clone());

    // Use SupplierService to get the supplier (cache-first lookup)
    let service = SupplierService::new(&project, &cache);
    let sup = service
        .get(&resolved_id)
        .map_err(|e| miette::miette!("{}", e))?
        .ok_or_else(|| miette::miette!("No supplier found matching '{}'", args.id))?;

    match global.output {
        OutputFormat::Yaml => {
            let yaml = serde_yml::to_string(&sup).into_diagnostic()?;
            print!("{}", yaml);
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
