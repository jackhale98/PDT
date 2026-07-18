//! `tdt asm` command - Assembly management

use clap::{Subcommand, ValueEnum};
use console::style;
use miette::{IntoDiagnostic, Result};
use std::collections::HashSet;
use std::fmt;
use std::fs;

use crate::cli::filters::StatusFilter;
use crate::cli::helpers::{escape_csv, truncate_str};
use crate::cli::table::{CellValue, ColumnDef, TableRow};
use crate::cli::{GlobalOpts, OutputFormat};
use tdt_core::core::cache::EntityCache;
use tdt_core::core::entity::Status;
use tdt_core::core::identity::EntityPrefix;
use tdt_core::core::project::Project;
use tdt_core::core::shortid::ShortIdIndex;
use tdt_core::core::Config;
use tdt_core::entities::assembly::{Assembly, BomItem};
use tdt_core::entities::component::Component;
use tdt_core::schema::wizard::SchemaWizard;
use tdt_core::services::{
    AssemblyFilter, AssemblyService, AssemblySortField, CommonFilter, ComponentService,
    CreateAssembly, SortDirection, UpdateAssembly,
};

type BomCostRow = (String, String, u32, f64, f64, String, f64, String);

#[derive(Subcommand, Debug)]
pub enum AsmCommands {
    /// List assemblies with filtering
    List(ListArgs),

    /// Create a new assembly
    New(NewArgs),

    /// Show an assembly's details
    Show(ShowArgs),

    /// Edit an assembly in your editor
    Edit(EditArgs),

    /// Delete an assembly
    Delete(DeleteArgs),

    /// Archive an assembly (soft delete)
    Archive(ArchiveArgs),

    /// Show expanded BOM for an assembly
    Bom(BomArgs),

    /// Add a component to an assembly's BOM
    #[command(name = "add")]
    AddComponent(AddComponentArgs),

    /// Remove a component from an assembly's BOM
    #[command(name = "rm")]
    RemoveComponent(RemoveComponentArgs),

    /// Calculate total cost for an assembly (recursive BOM)
    Cost(CostArgs),

    /// Calculate total mass for an assembly (recursive BOM)
    Mass(MassArgs),

    /// Manage manufacturing routing for assembly
    #[command(subcommand)]
    Routing(RoutingCommands),
}

/// Routing subcommands for managing manufacturing process sequences
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
    /// Assembly ID (ASM-xxx or short ID like ASM@1)
    pub asm: String,

    /// Process ID to add (PROC-xxx or short ID like PROC@1)
    pub proc: String,

    /// Position in routing (0-indexed, default: append)
    #[arg(long)]
    pub position: Option<usize>,
}

#[derive(clap::Args, Debug)]
pub struct RoutingRmArgs {
    /// Assembly ID (ASM-xxx or short ID like ASM@1)
    pub asm: String,

    /// Process ID to remove (PROC-xxx or short ID) or position number
    pub proc_or_position: String,
}

#[derive(clap::Args, Debug)]
pub struct RoutingListArgs {
    /// Assembly ID (ASM-xxx or short ID like ASM@1)
    pub asm: String,

    /// Show full PROC IDs (default shows titles)
    #[arg(long)]
    pub ids: bool,
}

#[derive(clap::Args, Debug)]
pub struct RoutingSetArgs {
    /// Assembly ID (ASM-xxx or short ID like ASM@1)
    pub asm: String,

    /// Ordered list of PROC IDs (full or short IDs)
    pub procs: Vec<String>,
}

/// List column types
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ListColumn {
    Short,
    Id,
    PartNumber,
    Title,
    Status,
    Author,
    Created,
}

impl fmt::Display for ListColumn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ListColumn::Short => write!(f, "short"),
            ListColumn::Id => write!(f, "id"),
            ListColumn::PartNumber => write!(f, "part-number"),
            ListColumn::Title => write!(f, "title"),
            ListColumn::Status => write!(f, "status"),
            ListColumn::Author => write!(f, "author"),
            ListColumn::Created => write!(f, "created"),
        }
    }
}

impl ListColumn {}

/// Column definitions for TableFormatter
const ASM_COLUMNS: &[ColumnDef] = &[
    ColumnDef::new("id", "ID", 17),
    ColumnDef::new("part-number", "PART #", 18),
    ColumnDef::new("title", "TITLE", 30),
    ColumnDef::new("status", "STATUS", 10),
    ColumnDef::new("author", "AUTHOR", 15),
    ColumnDef::new("created", "CREATED", 20),
];

const ASM_LIST_SPEC: crate::cli::entity_cmd::CachedListSpec =
    crate::cli::entity_cmd::CachedListSpec {
        columns: ASM_COLUMNS,
        entity_name: "assembly",
        entity_prefix: "ASM",
        empty_message: "No assemblies found.",
    };

#[derive(clap::Args, Debug)]
pub struct ListArgs {
    /// Filter by status
    #[arg(long, short = 's', default_value = "all")]
    pub status: StatusFilter,

    /// Search in part number and title
    #[arg(long)]
    pub search: Option<String>,

    /// Filter by author
    #[arg(long)]
    pub author: Option<String>,

    /// Filter to sub-assemblies within this assembly (recursive)
    #[arg(long, short = 'A')]
    pub assembly: Option<String>,

    /// Show recent assemblies (limit to 10 most recent)
    #[arg(long)]
    pub recent: bool,

    /// Columns to display
    #[arg(long, value_delimiter = ',', default_values_t = vec![ListColumn::PartNumber, ListColumn::Title, ListColumn::Status])]
    pub columns: Vec<ListColumn>,

    /// Sort by column
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

    /// Wrap output for mobile/narrow terminals (specify width, e.g., -w 60)
    #[arg(short = 'w', long)]
    pub wrap: Option<usize>,

    /// Only show entities linked to these IDs (use - for stdin pipe)
    #[arg(long, value_delimiter = ',')]
    pub linked_to: Vec<String>,

    /// Filter by link type when using --linked-to (e.g., verified_by, satisfied_by)
    #[arg(long, requires = "linked_to")]
    pub via: Option<String>,
}

#[derive(clap::Args, Debug)]
pub struct NewArgs {
    /// Part number (required)
    #[arg(long, short = 'p')]
    pub part_number: Option<String>,

    /// Title/description
    #[arg(long, short = 'T')]
    pub title: Option<String>,

    /// Part revision
    #[arg(long)]
    pub revision: Option<String>,

    /// BOM items as ID:QTY pairs (e.g., --bom "CMP@1:2,CMP@2:1,ASM@1:1")
    #[arg(long, short = 'b', value_delimiter = ',')]
    pub bom: Vec<String>,

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
    /// Assembly ID or short ID (ASM@N)
    pub id: String,
}

#[derive(clap::Args, Debug)]
pub struct EditArgs {
    /// Assembly ID or short ID (ASM@N)
    pub id: String,
}

#[derive(clap::Args, Debug)]
pub struct DeleteArgs {
    /// Assembly ID or short ID (ASM@N)
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
    /// Assembly ID or short ID (ASM@N)
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
const ASSEMBLY_DIRS: &[&str] = tdt_core::core::project::Project::entity_search_directories(
    tdt_core::core::identity::EntityPrefix::Asm,
);

/// Entity configuration for assemblies
const ENTITY_CONFIG: crate::cli::EntityConfig = crate::cli::EntityConfig {
    prefix: EntityPrefix::Asm,
    dirs: ASSEMBLY_DIRS,
    name: "assembly",
    name_plural: "assemblies",
};

#[derive(clap::Args, Debug)]
pub struct BomArgs {
    /// Assembly ID or short ID (ASM@N)
    pub id: String,

    /// Flatten nested assemblies (show all components)
    #[arg(long)]
    pub flat: bool,
}

#[derive(clap::Args, Debug)]
pub struct AddComponentArgs {
    /// Assembly ID or short ID (ASM@N)
    pub assembly: String,

    /// Components as ID:QTY pairs (e.g., CMP@1:2 CMP@2:1) or single ID
    #[arg(value_name = "COMPONENT")]
    pub components: Vec<String>,

    /// Quantity for single component (ignored if using ID:QTY format)
    #[arg(long, short = 'Q', default_value = "1")]
    pub qty: u32,

    /// Reference designators (comma-separated, e.g., "U1,U2,U3") - only for single component
    #[arg(long, short = 'r', value_delimiter = ',')]
    pub refs: Vec<String>,

    /// Notes about this BOM line item - only for single component
    #[arg(long)]
    pub notes: Option<String>,
}

#[derive(clap::Args, Debug)]
pub struct RemoveComponentArgs {
    /// Assembly ID or short ID (ASM@N)
    pub assembly: String,

    /// Component ID or short ID (CMP@N) to remove
    pub component: String,
}

#[derive(clap::Args, Debug)]
pub struct CostArgs {
    /// Assembly ID or short ID (ASM@N)
    pub assembly: String,

    /// Production quantity (for price break lookup)
    /// BOM quantities are multiplied by this to determine purchase quantities
    #[arg(long, default_value = "1")]
    pub qty: u32,

    /// Show breakdown by component
    #[arg(long)]
    pub breakdown: bool,

    /// Flatten nested assemblies (show all individual components instead of subassembly totals)
    #[arg(long)]
    pub flat: bool,

    /// Include NRE/tooling costs amortized over this production run quantity
    /// (e.g., --amortize 1000 spreads tooling cost over 1000 units)
    #[arg(long, short = 'a')]
    pub amortize: Option<u32>,

    /// Exclude NRE/tooling costs from the total
    #[arg(long)]
    pub no_nre: bool,

    /// Warn about expired quotes (enabled by default)
    #[arg(long, default_value = "true")]
    pub warn_expired: bool,
}

#[derive(clap::Args, Debug)]
pub struct MassArgs {
    /// Assembly ID or short ID (ASM@N)
    pub assembly: String,

    /// Show breakdown by component
    #[arg(long)]
    pub breakdown: bool,
}

/// Parse an ID:QTY pair (e.g., "CMP@1:2" or "CMP-xxx:3")
/// Returns (id, quantity). If no quantity specified, defaults to 1.
fn parse_bom_item(input: &str) -> Result<(String, u32)> {
    if let Some((id, qty_str)) = input.rsplit_once(':') {
        // Check if qty_str is a valid number (not part of an ID like CMP-xxx)
        if let Ok(qty) = qty_str.parse::<u32>() {
            return Ok((id.to_string(), qty));
        }
    }
    // No colon or not a valid quantity, treat whole thing as ID with qty 1
    Ok((input.to_string(), 1))
}

/// Run an assembly subcommand
pub fn run(cmd: AsmCommands, global: &GlobalOpts) -> Result<()> {
    match cmd {
        AsmCommands::List(args) => run_list(args, global),
        AsmCommands::New(args) => run_new(args, global),
        AsmCommands::Show(args) => run_show(args, global),
        AsmCommands::Edit(args) => run_edit(args),
        AsmCommands::Delete(args) => run_delete(args),
        AsmCommands::Archive(args) => run_archive(args),
        AsmCommands::Bom(args) => run_bom(args, global),
        AsmCommands::AddComponent(args) => run_add_component(args),
        AsmCommands::RemoveComponent(args) => run_remove_component(args),
        AsmCommands::Cost(args) => run_cost(args),
        AsmCommands::Mass(args) => run_mass(args),
        AsmCommands::Routing(cmd) => run_routing(cmd),
    }
}

fn run_routing(cmd: RoutingCommands) -> Result<()> {
    match cmd {
        RoutingCommands::Add(args) => run_routing_add(args),
        RoutingCommands::Rm(args) => run_routing_rm(args),
        RoutingCommands::List(args) => run_routing_list(args),
        RoutingCommands::Set(args) => run_routing_set(args),
    }
}

fn run_list(args: ListArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let cache = EntityCache::open(&project).map_err(|e| miette::miette!("{}", e))?;
    let service = AssemblyService::new(&project, &cache);

    // Resolve linked-to filter via cache
    let short_ids = ShortIdIndex::load(&project);
    let allowed_ids = crate::cli::helpers::resolve_linked_to(
        &args.linked_to,
        args.via.as_deref(),
        &short_ids,
        &cache,
    );

    // Build filter and sort from CLI args
    let filter = build_asm_filter(&args);
    let (sort_field, sort_dir) = build_asm_sort(&args);

    // Get assemblies via service
    let result = service
        .list(&filter, sort_field, sort_dir)
        .map_err(|e| miette::miette!("{}", e))?;

    let mut assemblies = result.items;

    // Apply linked-to filter
    if let Some(ref ids) = allowed_ids {
        assemblies.retain(|e| ids.contains(&e.id.to_string()));
    }

    // Filter by parent assembly (sub-assemblies only) - post-filter
    if let Some(ref parent_asm) = args.assembly {
        let short_ids_tmp = ShortIdIndex::load(&project);
        let parent_id = short_ids_tmp
            .resolve(parent_asm)
            .unwrap_or_else(|| parent_asm.clone());

        let assembly_map: std::collections::HashMap<String, &Assembly> =
            assemblies.iter().map(|a| (a.id.to_string(), a)).collect();

        if let Some(parent) = assembly_map.get(&parent_id) {
            let mut sub_asm_ids: HashSet<String> = HashSet::new();
            let mut visited: HashSet<String> = HashSet::new();
            visited.insert(parent_id.clone());
            collect_subassembly_ids(parent, &assembly_map, &mut sub_asm_ids, &mut visited);
            assemblies.retain(|a| sub_asm_ids.contains(&a.id.to_string()));
        } else {
            eprintln!(
                "Warning: Assembly '{}' not found, showing all assemblies",
                parent_asm
            );
        }
    }

    // Apply recent filter (show 10 most recent)
    if args.recent {
        assemblies.sort_by(|a, b| b.created.cmp(&a.created));
        assemblies.truncate(10);
    }

    // Apply limit
    if let Some(limit) = args.limit {
        assemblies.truncate(limit);
    }

    // Handle count-only mode
    if args.count {
        println!("{}", assemblies.len());
        return Ok(());
    }

    if assemblies.is_empty() {
        match global.output {
            OutputFormat::Json => println!("[]"),
            OutputFormat::Yaml => println!("[]"),
            _ => println!("No assemblies found."),
        }
        return Ok(());
    }

    let mut short_ids = ShortIdIndex::load(&project);
    let format = global.list_format();
    let visible: Vec<String> = args.columns.iter().map(|c| c.to_string()).collect();
    crate::cli::entity_cmd::output_entity_list(
        &assemblies,
        &ASM_LIST_SPEC,
        &visible,
        args.wrap,
        args.count,
        format,
        &project,
        &mut short_ids,
        |x| x.id.to_string(),
        asm_to_row,
    )
}

/// Build AssemblyFilter from CLI args
fn build_asm_filter(args: &ListArgs) -> AssemblyFilter {
    let status = match args.status {
        StatusFilter::Draft => Some(vec![Status::Draft]),
        StatusFilter::Review => Some(vec![Status::Review]),
        StatusFilter::Approved => Some(vec![Status::Approved]),
        StatusFilter::Released => Some(vec![Status::Released]),
        StatusFilter::Obsolete => Some(vec![Status::Obsolete]),
        StatusFilter::Active | StatusFilter::All => None,
    };

    AssemblyFilter {
        common: CommonFilter {
            status,
            author: args.author.clone(),
            search: args.search.clone(),
            ..Default::default()
        },
        ..Default::default()
    }
}

/// Build sort field and direction from CLI args
fn build_asm_sort(args: &ListArgs) -> (AssemblySortField, SortDirection) {
    let field = match args.sort {
        ListColumn::Short | ListColumn::Id => AssemblySortField::Id,
        ListColumn::PartNumber => AssemblySortField::PartNumber,
        ListColumn::Title => AssemblySortField::Title,
        ListColumn::Status => AssemblySortField::Status,
        ListColumn::Author => AssemblySortField::Author,
        ListColumn::Created => AssemblySortField::Created,
    };

    let direction = if args.reverse {
        match field {
            AssemblySortField::Created => SortDirection::Ascending,
            _ => SortDirection::Descending,
        }
    } else {
        match field {
            AssemblySortField::Created => SortDirection::Descending,
            _ => SortDirection::Ascending,
        }
    };

    (field, direction)
}

/// Convert an Assembly to a TableRow
fn asm_to_row(asm: &Assembly, short_ids: &ShortIdIndex) -> TableRow {
    TableRow::new(asm.id.to_string(), short_ids)
        .cell("id", CellValue::Id(asm.id.to_string()))
        .cell("part-number", CellValue::Text(asm.part_number.clone()))
        .cell("title", CellValue::Text(asm.title.clone()))
        .cell("status", CellValue::Status(asm.status))
        .cell("author", CellValue::Text(asm.author.clone()))
        .cell("created", CellValue::DateTime(asm.created))
}

fn run_new(args: NewArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let cache = EntityCache::open(&project).map_err(|e| miette::miette!("{}", e))?;
    let config = Config::load();
    let service = AssemblyService::new(&project, &cache);

    let part_number: String;
    let title: String;
    let description: Option<String>;
    let revision: Option<String>;

    // Load short IDs early for BOM item resolution
    let mut short_ids = ShortIdIndex::load(&project);

    if args.interactive {
        // Use schema-driven wizard
        let wizard = SchemaWizard::new();
        let result = wizard.run(EntityPrefix::Asm)?;

        part_number = result
            .get_string("part_number")
            .map(String::from)
            .unwrap_or_else(|| "NEW-ASM".to_string());

        title = result
            .get_string("title")
            .map(String::from)
            .unwrap_or_else(|| "New Assembly".to_string());

        description = result.get_string("description").map(String::from);
        revision = result.get_string("revision").map(String::from);
    } else {
        part_number = args
            .part_number
            .ok_or_else(|| miette::miette!("Part number is required (use --part-number or -p)"))?;
        title = args
            .title
            .ok_or_else(|| miette::miette!("Title is required (use --title or -T)"))?;
        description = None;
        revision = args.revision.clone();
    }

    // Parse BOM items from --bom flags, separating components from subassemblies
    let mut bom_items: Vec<BomItem> = Vec::new();
    let mut subassembly_ids: Vec<String> = Vec::new();

    for item_str in &args.bom {
        if let Ok((id, qty)) = parse_bom_item(item_str) {
            let resolved_id = short_ids.resolve(&id).unwrap_or_else(|| id.clone());

            // Check if this is an assembly (ASM-) or component (CMP-)
            if resolved_id.starts_with("ASM-") {
                // Subassemblies - just store the ID (quantity is ignored for subassemblies)
                subassembly_ids.push(resolved_id);
            } else {
                // Components - store as BOM item with quantity
                bom_items.push(BomItem {
                    component_id: resolved_id,
                    quantity: qty,
                    reference_designators: Vec::new(),
                    notes: None,
                });
            }
        }
    }

    let bom_count = bom_items.len();
    let subasm_count = subassembly_ids.len();

    // Create assembly via service
    let input = CreateAssembly {
        part_number: part_number.clone(),
        title: title.clone(),
        author: config.author(),
        revision,
        description,
        bom: bom_items,
        subassemblies: subassembly_ids,
        tags: Vec::new(),
    };

    let assembly = service
        .create(input)
        .map_err(|e| miette::miette!("{}", e))?;

    // Get file path for the created assembly
    let file_path = project
        .root()
        .join("bom/assemblies")
        .join(format!("{}.tdt.yaml", assembly.id));

    // Add to short ID index
    let short_id = short_ids.add(assembly.id.to_string());
    super::utils::save_short_ids(&mut short_ids, &project);

    // Handle --link flags
    let added_links = crate::cli::entity_cmd::process_link_flags(
        &file_path,
        EntityPrefix::Asm,
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
        &assembly.id,
        &file_path,
        short_id.clone(),
        ENTITY_CONFIG.name,
        &title,
        Some(&extra_info),
        &added_links,
        global,
    );

    // Report BOM items and subassemblies if any were added
    if bom_count > 0 {
        println!(
            "   {} Added {} BOM item{}",
            style("→").dim(),
            style(bom_count).cyan(),
            if bom_count == 1 { "" } else { "s" }
        );
    }
    if subasm_count > 0 {
        println!(
            "   {} Added {} subassembl{}",
            style("→").dim(),
            style(subasm_count).cyan(),
            if subasm_count == 1 { "y" } else { "ies" }
        );
    }

    // Sync cache after creation
    super::utils::sync_cache(&project);

    // Open in editor if requested
    if args.edit || (!args.no_edit && !args.interactive && args.bom.is_empty()) {
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

    // Use AssemblyService to get the assembly (cache-first lookup)
    let service = AssemblyService::new(&project, &cache);
    let asm = service
        .get(&resolved_id)
        .map_err(|e| miette::miette!("{}", e))?
        .ok_or_else(|| miette::miette!("No assembly found matching '{}'", args.id))?;

    match global.output {
        OutputFormat::Yaml => {
            let yaml = serde_yml::to_string(&asm).into_diagnostic()?;
            print!("{}", yaml);
        }
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&asm).into_diagnostic()?;
            println!("{}", json);
        }
        OutputFormat::Id | OutputFormat::ShortId => {
            if global.output == OutputFormat::ShortId {
                let short_id = short_ids
                    .get_short_id(&asm.id.to_string())
                    .unwrap_or_default();
                println!("{}", short_id);
            } else {
                println!("{}", asm.id);
            }
        }
        _ => {
            // Pretty format (default)
            println!("{}", style("─".repeat(60)).dim());
            println!(
                "{}: {}",
                style("ID").bold(),
                style(&asm.id.to_string()).cyan()
            );
            println!("{}: {}", style("Title").bold(), style(&asm.title).yellow());
            if !asm.part_number.is_empty() {
                println!("{}: {}", style("Part Number").bold(), asm.part_number);
            }
            if let Some(ref rev) = asm.revision {
                if !rev.is_empty() {
                    println!("{}: {}", style("Revision").bold(), rev);
                }
            }
            println!("{}: {}", style("Status").bold(), asm.status);
            println!("{}", style("─".repeat(60)).dim());

            // BOM
            if !asm.bom.is_empty() {
                println!();
                println!("{}", style("Bill of Materials:").bold());
                for item in &asm.bom {
                    let cmp_display = short_ids
                        .get_short_id(&item.component_id)
                        .unwrap_or_else(|| item.component_id.clone());
                    println!("  • {} x{}", style(&cmp_display).cyan(), item.quantity);
                }
            }

            // Subassemblies
            if !asm.subassemblies.is_empty() {
                println!();
                println!("{}", style("Subassemblies:").bold());
                for sub in &asm.subassemblies {
                    let sub_display = short_ids.get_short_id(sub).unwrap_or_else(|| sub.clone());
                    println!("  • {}", style(&sub_display).cyan());
                }
            }

            // Documents
            if !asm.documents.is_empty() && asm.documents.iter().any(|d| !d.path.is_empty()) {
                println!();
                println!("{}", style("Documents:").bold());
                for doc in &asm.documents {
                    if !doc.path.is_empty() {
                        println!("  • [{}] {}", doc.doc_type, doc.path);
                    }
                }
            }

            // Tags
            if !asm.tags.is_empty() {
                println!();
                println!("{}: {}", style("Tags").bold(), asm.tags.join(", "));
            }

            // Description
            if let Some(ref desc) = asm.description {
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
                asm.author,
                style("Created").dim(),
                asm.created.format("%Y-%m-%d %H:%M"),
                style("Revision").dim(),
                asm.entity_revision
            );
        }
    }

    Ok(())
}

fn run_edit(args: EditArgs) -> Result<()> {
    crate::cli::entity_cmd::run_edit_generic(&args.id, &ENTITY_CONFIG)
}

fn run_delete(args: DeleteArgs) -> Result<()> {
    crate::cli::commands::utils::run_delete(&args.id, ASSEMBLY_DIRS, args.force, false, args.quiet)
}

fn run_archive(args: ArchiveArgs) -> Result<()> {
    crate::cli::commands::utils::run_delete(&args.id, ASSEMBLY_DIRS, args.force, true, args.quiet)
}

fn run_bom(args: BomArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let cache = EntityCache::open(&project).map_err(|e| miette::miette!("{}", e))?;

    // Resolve short ID if needed
    let short_ids = ShortIdIndex::load(&project);
    let resolved_id = short_ids
        .resolve(&args.id)
        .unwrap_or_else(|| args.id.clone());

    // Use services
    let asm_service = AssemblyService::new(&project, &cache);
    let cmp_service = ComponentService::new(&project, &cache);

    // Load assembly via service
    let assembly = asm_service
        .get_required(&resolved_id)
        .map_err(|_| miette::miette!("No assembly found matching '{}'", args.id))?;

    // Load all components for name lookup
    let all_components = cmp_service.load_all().unwrap_or_default();
    let components: std::collections::HashMap<String, Component> = all_components
        .into_iter()
        .map(|c| (c.id.to_string(), c))
        .collect();

    // Load all assemblies for subassembly lookup (needed for --flat)
    let all_assemblies = asm_service.load_all().unwrap_or_default();
    let assembly_map: std::collections::HashMap<String, &tdt_core::entities::assembly::Assembly> =
        all_assemblies
            .iter()
            .map(|a| (a.id.to_string(), a))
            .collect();

    // Display BOM
    let format = global.list_format();

    println!();
    println!(
        "{} BOM for {} - {}",
        style("Assembly").bold(),
        style(&assembly.part_number).yellow(),
        style(&assembly.title).white()
    );
    if args.flat {
        println!(
            "{}",
            style("(flattened view - all components from subassemblies)").dim()
        );
    }
    println!();

    // Collect BOM items - either flat or hierarchical
    let bom_items: Vec<(BomItem, Option<String>)> = if args.flat {
        // Flatten: collect all components recursively from subassemblies
        let mut flat_items: Vec<(BomItem, Option<String>)> = Vec::new();
        let mut visited: HashSet<String> = HashSet::new();
        visited.insert(assembly.id.to_string());

        fn collect_flat_bom(
            bom: &[BomItem],
            subassemblies: &[String],
            assembly_map: &std::collections::HashMap<
                String,
                &tdt_core::entities::assembly::Assembly,
            >,
            flat_items: &mut Vec<(BomItem, Option<String>)>,
            visited: &mut HashSet<String>,
            parent_asm: Option<&str>,
            qty_multiplier: u32,
        ) {
            // Process direct BOM items
            for item in bom {
                let item_id = item.component_id.to_string();

                if item_id.starts_with("ASM-") {
                    // This is a subassembly in the BOM - recurse into it
                    if !visited.contains(&item_id) {
                        visited.insert(item_id.clone());
                        if let Some(sub_asm) = assembly_map.get(&item_id) {
                            collect_flat_bom(
                                &sub_asm.bom,
                                &sub_asm.subassemblies,
                                assembly_map,
                                flat_items,
                                visited,
                                Some(&sub_asm.title),
                                qty_multiplier * item.quantity,
                            );
                        }
                        visited.remove(&item_id);
                    }
                } else {
                    // Regular component - add to flat list with multiplied quantity
                    flat_items.push((
                        BomItem {
                            component_id: item.component_id.clone(),
                            quantity: item.quantity * qty_multiplier,
                            reference_designators: item.reference_designators.clone(),
                            notes: item.notes.clone(),
                        },
                        parent_asm.map(String::from),
                    ));
                }
            }

            // Process subassemblies field
            for sub_id in subassemblies {
                if !visited.contains(sub_id) {
                    visited.insert(sub_id.clone());
                    if let Some(sub_asm) = assembly_map.get(sub_id) {
                        collect_flat_bom(
                            &sub_asm.bom,
                            &sub_asm.subassemblies,
                            assembly_map,
                            flat_items,
                            visited,
                            Some(&sub_asm.title),
                            qty_multiplier,
                        );
                    }
                    visited.remove(sub_id);
                }
            }
        }

        collect_flat_bom(
            &assembly.bom,
            &assembly.subassemblies,
            &assembly_map,
            &mut flat_items,
            &mut visited,
            None,
            1,
        );

        // Aggregate duplicate components (sum quantities)
        let mut aggregated: std::collections::HashMap<String, (BomItem, Option<String>)> =
            std::collections::HashMap::new();
        for (item, source) in flat_items {
            if let Some((existing, _)) = aggregated.get_mut(&item.component_id) {
                existing.quantity += item.quantity;
                // Merge reference designators
                for r in &item.reference_designators {
                    if !existing.reference_designators.contains(r) {
                        existing.reference_designators.push(r.clone());
                    }
                }
            } else {
                aggregated.insert(item.component_id.clone(), (item, source));
            }
        }

        aggregated.into_values().collect()
    } else {
        // Non-flat: just use direct BOM items
        assembly
            .bom
            .iter()
            .map(|item| (item.clone(), None))
            .collect()
    };

    match format {
        OutputFormat::Tsv
        | OutputFormat::Auto
        | OutputFormat::Table
        | OutputFormat::Dot
        | OutputFormat::Tree => {
            if args.flat {
                println!(
                    "{:<6} {:<15} {:<12} {:<30} {:<15}",
                    style("QTY").bold(),
                    style("COMPONENT ID").bold(),
                    style("PART #").bold(),
                    style("TITLE").bold(),
                    style("FROM ASM").bold()
                );
            } else {
                println!(
                    "{:<6} {:<15} {:<12} {:<30} {:<20}",
                    style("QTY").bold(),
                    style("COMPONENT ID").bold(),
                    style("PART #").bold(),
                    style("TITLE").bold(),
                    style("REFERENCES").bold()
                );
            }
            println!("{}", "-".repeat(85));

            for (item, source_asm) in &bom_items {
                let cmp_info = components.get(&item.component_id);
                let part_number = cmp_info.map(|c| c.part_number.as_str()).unwrap_or("-");
                let title = cmp_info.map(|c| c.title.as_str()).unwrap_or("(not found)");

                // Use short ID if available, otherwise truncate the full ID
                let cmp_display = short_ids
                    .get_short_id(&item.component_id)
                    .unwrap_or_else(|| truncate_str(&item.component_id, 13).to_string());

                if args.flat {
                    let source = source_asm.as_deref().unwrap_or("-");
                    println!(
                        "{:<6} {:<15} {:<12} {:<30} {:<15}",
                        item.quantity,
                        cmp_display,
                        truncate_str(part_number, 10),
                        truncate_str(title, 28),
                        truncate_str(source, 13)
                    );
                } else {
                    let refs = if item.reference_designators.is_empty() {
                        String::new()
                    } else {
                        item.reference_designators.join(", ")
                    };
                    println!(
                        "{:<6} {:<15} {:<12} {:<30} {:<20}",
                        item.quantity,
                        cmp_display,
                        truncate_str(part_number, 10),
                        truncate_str(title, 28),
                        truncate_str(&refs, 18)
                    );
                }
            }

            if !args.flat && !assembly.subassemblies.is_empty() {
                println!();
                println!("{}", style("Sub-assemblies:").bold());
                for sub_id in &assembly.subassemblies {
                    let sub_display = short_ids
                        .get_short_id(sub_id)
                        .unwrap_or_else(|| sub_id.clone());
                    println!("  - {}", sub_display);
                }
            }

            println!();
            if args.flat {
                println!(
                    "{} Total: {} unique components",
                    style("Summary").bold(),
                    bom_items.len()
                );
            } else {
                println!(
                    "{} Total: {} line items, {} total components",
                    style("Summary").bold(),
                    assembly.bom.len(),
                    assembly.total_component_count()
                );
            }
        }
        OutputFormat::Json => {
            if args.flat {
                let items: Vec<&BomItem> = bom_items.iter().map(|(item, _)| item).collect();
                let json = serde_json::to_string_pretty(&items).into_diagnostic()?;
                println!("{}", json);
            } else {
                let json = serde_json::to_string_pretty(&assembly.bom).into_diagnostic()?;
                println!("{}", json);
            }
        }
        OutputFormat::Yaml => {
            if args.flat {
                let items: Vec<&BomItem> = bom_items.iter().map(|(item, _)| item).collect();
                let yaml = serde_yml::to_string(&items).into_diagnostic()?;
                print!("{}", yaml);
            } else {
                let yaml = serde_yml::to_string(&assembly.bom).into_diagnostic()?;
                print!("{}", yaml);
            }
        }
        OutputFormat::Csv => {
            if args.flat {
                println!("quantity,component_id,part_number,title,from_assembly");
                for (item, source_asm) in &bom_items {
                    let cmp_info = components.get(&item.component_id);
                    let part_number = cmp_info.map(|c| c.part_number.as_str()).unwrap_or("");
                    let title = cmp_info.map(|c| c.title.as_str()).unwrap_or("");
                    let source = source_asm.as_deref().unwrap_or("");

                    println!(
                        "{},{},{},{},{}",
                        item.quantity,
                        item.component_id,
                        escape_csv(part_number),
                        escape_csv(title),
                        escape_csv(source)
                    );
                }
            } else {
                println!("quantity,component_id,part_number,title,reference_designators,notes");
                for (item, _) in &bom_items {
                    let cmp_info = components.get(&item.component_id);
                    let part_number = cmp_info.map(|c| c.part_number.as_str()).unwrap_or("");
                    let title = cmp_info.map(|c| c.title.as_str()).unwrap_or("");
                    let refs = item.reference_designators.join(";");
                    let notes = item.notes.as_deref().unwrap_or("");

                    println!(
                        "{},{},{},{},{},{}",
                        item.quantity,
                        item.component_id,
                        escape_csv(part_number),
                        escape_csv(title),
                        escape_csv(&refs),
                        escape_csv(notes)
                    );
                }
            }
        }
        _ => {}
    }

    Ok(())
}

fn run_add_component(args: AddComponentArgs) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let cache = EntityCache::open(&project).map_err(|e| miette::miette!("{}", e))?;
    let short_ids = ShortIdIndex::load(&project);

    if args.components.is_empty() {
        return Err(miette::miette!(
            "At least one component is required.\n\
             Usage: tdt asm add ASM@1 CMP@1:2 CMP@2:1\n\
                    tdt asm add ASM@1 CMP@1 --qty 2"
        ));
    }

    // Use services
    let asm_service = AssemblyService::new(&project, &cache);
    let cmp_service = ComponentService::new(&project, &cache);

    // Resolve assembly ID and load via service
    let asm_id = short_ids
        .resolve(&args.assembly)
        .unwrap_or_else(|| args.assembly.clone());

    let mut assembly = asm_service.get_required(&asm_id).map_err(|_| {
        miette::miette!(
            "Assembly '{}' not found. Create it first with: tdt asm new",
            args.assembly
        )
    })?;

    // Determine if we're in single-component mode (with --qty, --refs, --notes) or multi-component mode
    let single_component_mode = args.components.len() == 1 && !args.components[0].contains(':');

    let mut added_count = 0;
    let mut updated_count = 0;

    for component_arg in &args.components {
        // Parse component:qty or use --qty for single component
        let (component_input, qty) = if single_component_mode {
            (component_arg.clone(), args.qty)
        } else {
            parse_bom_item(component_arg)?
        };

        // Resolve component ID
        let cmp_id = short_ids
            .resolve(&component_input)
            .unwrap_or_else(|| component_input.clone());

        // Load component via service to validate and get info
        let component_info = cmp_service.get(&cmp_id).ok().flatten();

        // Get the full component ID (use resolved or original)
        let full_cmp_id = component_info
            .as_ref()
            .map(|c| c.id.to_string())
            .unwrap_or_else(|| cmp_id.clone());

        // Check if component already exists in BOM
        if let Some(existing) = assembly
            .bom
            .iter_mut()
            .find(|item| item.component_id == full_cmp_id)
        {
            // Update existing entry
            existing.quantity += qty;
            if single_component_mode {
                if !args.refs.is_empty() {
                    existing.reference_designators.extend(args.refs.clone());
                }
                if args.notes.is_some() {
                    existing.notes = args.notes.clone();
                }
            }
            updated_count += 1;

            println!(
                "{} Updated {} (qty now: {})",
                style("✓").green(),
                style(&component_input).cyan(),
                existing.quantity
            );
        } else {
            // Add new BOM item
            let bom_item = BomItem {
                component_id: full_cmp_id.clone(),
                quantity: qty,
                reference_designators: if single_component_mode {
                    args.refs.clone()
                } else {
                    Vec::new()
                },
                notes: if single_component_mode {
                    args.notes.clone()
                } else {
                    None
                },
            };
            assembly.bom.push(bom_item);
            added_count += 1;

            if single_component_mode {
                let cmp_info = component_info.as_ref();
                println!(
                    "{} Added {} to {}",
                    style("✓").green(),
                    style(&component_input).cyan(),
                    style(&args.assembly).yellow()
                );
                if let Some(info) = cmp_info {
                    println!(
                        "   Component: {} | {}",
                        style(&info.part_number).white(),
                        style(&info.title).dim()
                    );
                }
                println!("   Quantity: {}", qty);
                if !args.refs.is_empty() {
                    println!("   References: {}", args.refs.join(", "));
                }
            } else {
                println!(
                    "{} Added {}:{} to BOM",
                    style("✓").green(),
                    style(&component_input).cyan(),
                    qty
                );
            }
        }
    }

    // Save via service update
    let update_input = UpdateAssembly {
        bom: Some(assembly.bom.clone()),
        ..Default::default()
    };
    let assembly = asm_service
        .update(&asm_id, update_input)
        .map_err(|e| miette::miette!("{}", e))?;

    if !single_component_mode {
        println!();
        if added_count > 0 {
            println!(
                "   Added {} new component{}",
                style(added_count).cyan(),
                if added_count == 1 { "" } else { "s" }
            );
        }
        if updated_count > 0 {
            println!(
                "   Updated {} existing component{}",
                style(updated_count).yellow(),
                if updated_count == 1 { "" } else { "s" }
            );
        }
    }

    println!(
        "   BOM now has {} line items ({} total components)",
        assembly.bom.len(),
        assembly.total_component_count()
    );

    // Sync cache after BOM modification
    super::utils::sync_cache(&project);

    Ok(())
}

fn run_remove_component(args: RemoveComponentArgs) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let cache = EntityCache::open(&project).map_err(|e| miette::miette!("{}", e))?;
    let short_ids = ShortIdIndex::load(&project);

    // Resolve IDs
    let asm_id = short_ids
        .resolve(&args.assembly)
        .unwrap_or_else(|| args.assembly.clone());
    let cmp_id = short_ids
        .resolve(&args.component)
        .unwrap_or_else(|| args.component.clone());

    // Use service
    let service = AssemblyService::new(&project, &cache);

    // Try to remove via service - it handles the validation
    let assembly = service.remove_component(&asm_id, &cmp_id).map_err(|_| {
        // Convert service error to user-friendly message
        miette::miette!(
            "Component '{}' not found in assembly '{}' BOM",
            args.component,
            args.assembly
        )
    })?;

    println!(
        "{} Removed {} from {}",
        style("✓").green(),
        style(&args.component).cyan(),
        style(&args.assembly).yellow()
    );
    println!("   BOM now has {} line items", assembly.bom.len());

    // Sync cache after BOM modification
    super::utils::sync_cache(&project);

    Ok(())
}

fn run_cost(args: CostArgs) -> Result<()> {
    use tdt_core::entities::quote::Quote;

    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let mut cache = EntityCache::open(&project).map_err(|e| miette::miette!("{}", e))?;
    let short_ids = ShortIdIndex::load(&project);

    // Resolve assembly ID
    let resolved_id = short_ids
        .resolve(&args.assembly)
        .unwrap_or_else(|| args.assembly.clone());

    // Load assembly
    let assembly = find_assembly(&project, &resolved_id)?;

    let production_qty = args.qty;
    let include_nre = !args.no_nre;

    // Sync cache to ensure BOM data is fresh (needed for both flat and hierarchical modes)
    cache.sync().map_err(|e| miette::miette!("{}", e))?;

    // Use optimized cache-based calculation for flat mode
    if args.flat {
        let service = AssemblyService::new(&project, &cache);
        let result = service
            .calculate_cost_cached(&assembly.id.to_string(), production_qty)
            .map_err(|e| miette::miette!("{}", e))?;

        // Load quotes for warning detection
        let quotes = load_all_quotes(&project);
        let quote_map: std::collections::HashMap<String, &Quote> =
            quotes.iter().map(|q| (q.id.to_string(), q)).collect();

        // Build component -> quotes map for unselected warnings
        let mut component_quotes: std::collections::HashMap<String, Vec<&Quote>> =
            std::collections::HashMap::new();
        for quote in &quotes {
            if let Some(ref cmp_id) = quote.component {
                component_quotes
                    .entry(cmp_id.clone())
                    .or_default()
                    .push(quote);
            }
        }

        // Detect warnings
        let mut expired_warnings: Vec<(String, String, String)> = Vec::new();
        let mut unselected_warnings: Vec<(String, String, usize)> = Vec::new();

        for line in &result.component_costs {
            // Check for expired quotes
            if let Some(ref qid) = line.quote_id {
                if let Some(quote) = quote_map.get(qid) {
                    if quote.is_expired() && args.warn_expired {
                        let valid_until =
                            quote.valid_until.map(|d| d.to_string()).unwrap_or_default();
                        if !expired_warnings.iter().any(|(_, t, _)| t == &line.title) {
                            expired_warnings.push((qid.clone(), line.title.clone(), valid_until));
                        }
                    }
                }
            } else {
                // No quote used - check if quotes are available but unselected
                if let Some(cmp_quotes) = component_quotes.get(&line.component_id) {
                    if !cmp_quotes.is_empty()
                        && !unselected_warnings
                            .iter()
                            .any(|(id, _, _)| id == &line.component_id)
                    {
                        unselected_warnings.push((
                            line.component_id.clone(),
                            line.title.clone(),
                            cmp_quotes.len(),
                        ));
                    }
                }
            }
        }

        // Output using cached result
        return output_cached_cost_result(
            &assembly,
            &result,
            &args,
            &short_ids,
            &expired_warnings,
            &unselected_warnings,
            include_nre,
            production_qty,
        );
    }

    // For non-flat (hierarchical) mode, use cache-optimized loading
    // First, collect all needed IDs from cache (fast)
    let (needed_component_ids, needed_assembly_ids) =
        collect_bom_ids_from_cache(&cache, &assembly.id.to_string());

    // Batch-load only the entities we need (instead of loading ALL entities)
    let components = load_components_by_ids(&project, &needed_component_ids);
    let component_map: std::collections::HashMap<String, &Component> =
        components.iter().map(|c| (c.id.to_string(), c)).collect();

    let mut assemblies = load_assemblies_by_ids(&project, &needed_assembly_ids);
    // Add the root assembly if not already included
    assemblies.push(assembly.clone());
    let assembly_map: std::collections::HashMap<String, &Assembly> =
        assemblies.iter().map(|a| (a.id.to_string(), a)).collect();

    // Load all quotes (usually few, and we need them for selected_quote lookups)
    let quotes = load_all_quotes(&project);
    let quote_map: std::collections::HashMap<String, &Quote> =
        quotes.iter().map(|q| (q.id.to_string(), q)).collect();

    // Build a map of component -> quotes for that component (for warning about unselected quotes)
    let mut component_quotes: std::collections::HashMap<String, Vec<&Quote>> =
        std::collections::HashMap::new();
    for quote in &quotes {
        if let Some(ref cmp_id) = quote.component {
            component_quotes
                .entry(cmp_id.clone())
                .or_default()
                .push(quote);
        }
    }

    // Track components with quotes but no selection (for user feedback)
    let mut unselected_quote_warnings: Vec<(String, String, usize)> = Vec::new(); // (id, title, quote_count)

    // Track expired quotes used
    let mut expired_quote_warnings: Vec<(String, String, String)> = Vec::new(); // (quote_id, component_title, valid_until)

    // Track NRE costs from selected quotes
    let mut nre_items: Vec<(String, String, f64)> = Vec::new(); // (quote_id, component_title, nre_amount)

    // Calculate costs recursively
    // breakdown: (id, title, bom_qty, unit_price, line_cost, price_source, nre, source_asm)
    let mut breakdown: Vec<BomCostRow> = Vec::new();
    let mut visited = std::collections::HashSet::new();
    visited.insert(assembly.id.to_string());

    #[allow(clippy::too_many_arguments)]
    fn calculate_bom_cost(
        bom: &[tdt_core::entities::assembly::BomItem],
        subassemblies: &[String],
        component_map: &std::collections::HashMap<String, &Component>,
        assembly_map: &std::collections::HashMap<String, &Assembly>,
        quote_map: &std::collections::HashMap<String, &Quote>,
        component_quotes: &std::collections::HashMap<String, Vec<&Quote>>,
        breakdown: &mut Vec<BomCostRow>,
        unselected_warnings: &mut Vec<(String, String, usize)>,
        expired_warnings: &mut Vec<(String, String, String)>,
        nre_items: &mut Vec<(String, String, f64)>,
        visited: &mut std::collections::HashSet<String>,
        production_qty: u32,
        warn_expired: bool,
        flat: bool,
        qty_multiplier: u32,
        source_asm: &str,
    ) -> f64 {
        let mut total = 0.0;
        for item in bom {
            let item_id = item.component_id.to_string();
            let effective_qty = item.quantity * qty_multiplier;

            if let Some(cmp) = component_map.get(&item_id) {
                // Determine price: selected quote > unit_cost > 0.0
                let purchase_qty = effective_qty * production_qty;
                let (unit_price, price_source, nre, is_expired, valid_until) =
                    get_component_price_with_nre(
                        cmp,
                        quote_map,
                        component_quotes,
                        purchase_qty,
                        unselected_warnings,
                    );

                // Track expired quote warning
                if is_expired && warn_expired {
                    let already_warned = expired_warnings
                        .iter()
                        .any(|(_, title, _)| title == &cmp.title);
                    if !already_warned {
                        expired_warnings.push((
                            cmp.selected_quote.clone().unwrap_or_default(),
                            cmp.title.clone(),
                            valid_until,
                        ));
                    }
                }

                // Track NRE if present
                if nre > 0.0 {
                    let already_tracked = nre_items.iter().any(|(_, title, _)| title == &cmp.title);
                    if !already_tracked {
                        nre_items.push((
                            cmp.selected_quote.clone().unwrap_or_default(),
                            cmp.title.clone(),
                            nre,
                        ));
                    }
                }

                let line_cost = unit_price * effective_qty as f64;
                total += line_cost;
                breakdown.push((
                    item_id,
                    cmp.title.clone(),
                    effective_qty,
                    unit_price,
                    line_cost,
                    price_source,
                    nre,
                    source_asm.to_string(),
                ));
            } else if let Some(sub_asm) = assembly_map.get(&item_id) {
                if !visited.contains(&item_id) {
                    visited.insert(item_id.clone());

                    if flat {
                        // In flat mode, recurse into subassembly and collect individual components
                        let sub_cost = calculate_bom_cost(
                            &sub_asm.bom,
                            &sub_asm.subassemblies,
                            component_map,
                            assembly_map,
                            quote_map,
                            component_quotes,
                            breakdown,
                            unselected_warnings,
                            expired_warnings,
                            nre_items,
                            visited,
                            production_qty,
                            warn_expired,
                            flat,
                            effective_qty,
                            &sub_asm.title,
                        );
                        total += sub_cost;
                    } else {
                        // In hierarchical mode, show subassembly as a single line
                        let sub_cost = calculate_bom_cost(
                            &sub_asm.bom,
                            &sub_asm.subassemblies,
                            component_map,
                            assembly_map,
                            quote_map,
                            component_quotes,
                            breakdown,
                            unselected_warnings,
                            expired_warnings,
                            nre_items,
                            visited,
                            production_qty,
                            warn_expired,
                            flat,
                            1,
                            "",
                        );
                        let line_cost = sub_cost * effective_qty as f64;
                        total += line_cost;
                        breakdown.push((
                            item_id.clone(),
                            sub_asm.title.clone(),
                            effective_qty,
                            sub_cost,
                            line_cost,
                            "sub-asm".to_string(),
                            0.0,
                            "".to_string(),
                        ));
                    }
                    visited.remove(&item_id);
                }
            }
        }

        // Also process items from the subassemblies field
        for sub_id in subassemblies {
            if !visited.contains(sub_id) {
                if let Some(sub_asm) = assembly_map.get(sub_id) {
                    visited.insert(sub_id.clone());

                    if flat {
                        let sub_cost = calculate_bom_cost(
                            &sub_asm.bom,
                            &sub_asm.subassemblies,
                            component_map,
                            assembly_map,
                            quote_map,
                            component_quotes,
                            breakdown,
                            unselected_warnings,
                            expired_warnings,
                            nre_items,
                            visited,
                            production_qty,
                            warn_expired,
                            flat,
                            qty_multiplier,
                            &sub_asm.title,
                        );
                        total += sub_cost;
                    } else {
                        let sub_cost = calculate_bom_cost(
                            &sub_asm.bom,
                            &sub_asm.subassemblies,
                            component_map,
                            assembly_map,
                            quote_map,
                            component_quotes,
                            breakdown,
                            unselected_warnings,
                            expired_warnings,
                            nre_items,
                            visited,
                            production_qty,
                            warn_expired,
                            flat,
                            1,
                            "",
                        );
                        total += sub_cost * qty_multiplier as f64;
                        breakdown.push((
                            sub_id.clone(),
                            sub_asm.title.clone(),
                            qty_multiplier,
                            sub_cost,
                            sub_cost * qty_multiplier as f64,
                            "sub-asm".to_string(),
                            0.0,
                            "".to_string(),
                        ));
                    }
                    visited.remove(sub_id);
                }
            }
        }

        total
    }

    fn get_component_price_with_nre(
        cmp: &Component,
        quote_map: &std::collections::HashMap<String, &Quote>,
        component_quotes: &std::collections::HashMap<String, Vec<&Quote>>,
        purchase_qty: u32,
        unselected_warnings: &mut Vec<(String, String, usize)>,
    ) -> (f64, String, f64, bool, String) {
        // Returns: (unit_price, source, nre_total, is_expired, valid_until)

        // Priority 1: Use selected quote if set
        if let Some(ref quote_id) = cmp.selected_quote {
            if let Some(quote) = quote_map.get(quote_id) {
                if let Some(price) = quote.price_for_qty(purchase_qty) {
                    let nre = quote.total_nre();
                    let is_expired = quote.is_expired();
                    let valid_until = quote.valid_until.map(|d| d.to_string()).unwrap_or_default();
                    return (
                        price,
                        format!("quote@{}", purchase_qty),
                        nre,
                        is_expired,
                        valid_until,
                    );
                }
            }
        }

        // Priority 2: Fall back to manual unit_cost
        if let Some(cost) = cmp.unit_cost {
            // Check if there are quotes available but none selected
            if let Some(quotes) = component_quotes.get(&cmp.id.to_string()) {
                if !quotes.is_empty() {
                    // Only warn once per component
                    let already_warned = unselected_warnings
                        .iter()
                        .any(|(id, _, _)| id == &cmp.id.to_string());
                    if !already_warned {
                        unselected_warnings.push((
                            cmp.id.to_string(),
                            cmp.title.clone(),
                            quotes.len(),
                        ));
                    }
                }
            }
            return (cost, "unit_cost".to_string(), 0.0, false, String::new());
        }

        // Check if there are quotes available but none selected (and no unit_cost)
        if let Some(quotes) = component_quotes.get(&cmp.id.to_string()) {
            if !quotes.is_empty() {
                let already_warned = unselected_warnings
                    .iter()
                    .any(|(id, _, _)| id == &cmp.id.to_string());
                if !already_warned {
                    unselected_warnings.push((cmp.id.to_string(), cmp.title.clone(), quotes.len()));
                }
            }
        }

        (0.0, "none".to_string(), 0.0, false, String::new())
    }

    let total_piece_cost = calculate_bom_cost(
        &assembly.bom,
        &assembly.subassemblies,
        &component_map,
        &assembly_map,
        &quote_map,
        &component_quotes,
        &mut breakdown,
        &mut unselected_quote_warnings,
        &mut expired_quote_warnings,
        &mut nre_items,
        &mut visited,
        production_qty,
        args.warn_expired,
        args.flat,
        1,
        "",
    );

    // Calculate total NRE
    let total_nre: f64 = nre_items.iter().map(|(_, _, nre)| nre).sum();

    // Calculate amortized NRE per unit if requested
    let nre_per_unit = if include_nre {
        args.amortize
            .map(|qty| total_nre / qty as f64)
            .unwrap_or(0.0)
    } else {
        0.0
    };

    // Output
    println!(
        "{} {}",
        style("Assembly:").bold(),
        style(&assembly.title).cyan()
    );
    println!("{} {}", style("Part Number:").bold(), assembly.part_number);
    if args.flat {
        println!(
            "{}",
            style("(flattened view - all components from subassemblies)").dim()
        );
    }
    if production_qty > 1 {
        println!(
            "{} {}",
            style("Production Qty:").bold(),
            style(production_qty).yellow()
        );
    }
    if let Some(amort_qty) = args.amortize {
        if include_nre {
            println!(
                "{} {} units",
                style("NRE Amortization:").bold(),
                style(amort_qty).cyan()
            );
        }
    }
    println!();

    if (args.breakdown || args.flat) && !breakdown.is_empty() {
        let show_nre_col = include_nre && total_nre > 0.0;

        // Print header based on mode
        if args.flat {
            if show_nre_col {
                println!(
                    "{:<10} {:<20} {:<5} {:<10} {:<10} {:<8} {:<10} {:<12}",
                    style("ID").bold(),
                    style("TITLE").bold(),
                    style("QTY").bold(),
                    style("UNIT").bold(),
                    style("LINE").bold(),
                    style("NRE").bold(),
                    style("SOURCE").bold(),
                    style("FROM ASM").bold()
                );
                println!("{}", "-".repeat(95));
            } else {
                println!(
                    "{:<10} {:<22} {:<5} {:<10} {:<10} {:<10} {:<15}",
                    style("ID").bold(),
                    style("TITLE").bold(),
                    style("QTY").bold(),
                    style("UNIT").bold(),
                    style("LINE").bold(),
                    style("SOURCE").bold(),
                    style("FROM ASM").bold()
                );
                println!("{}", "-".repeat(90));
            }
        } else if show_nre_col {
            println!(
                "{:<10} {:<24} {:<5} {:<10} {:<10} {:<10} {:<10}",
                style("ID").bold(),
                style("TITLE").bold(),
                style("QTY").bold(),
                style("UNIT").bold(),
                style("LINE").bold(),
                style("NRE").bold(),
                style("SOURCE").bold()
            );
            println!("{}", "-".repeat(85));
        } else {
            println!(
                "{:<10} {:<26} {:<5} {:<10} {:<10} {:<10}",
                style("ID").bold(),
                style("TITLE").bold(),
                style("QTY").bold(),
                style("UNIT").bold(),
                style("LINE").bold(),
                style("SOURCE").bold()
            );
            println!("{}", "-".repeat(75));
        }

        for (id, title, qty, unit_price, line_cost, source, nre, source_asm) in &breakdown {
            let id_short = short_ids
                .get_short_id(id)
                .unwrap_or_else(|| truncate_str(id, 8));
            let from_asm = if source_asm.is_empty() {
                "-".to_string()
            } else {
                truncate_str(source_asm, 13)
            };

            if *line_cost > 0.0 || *unit_price > 0.0 {
                if args.flat {
                    if show_nre_col {
                        let nre_str = if *nre > 0.0 {
                            format!("${:.0}", nre)
                        } else {
                            "-".to_string()
                        };
                        println!(
                            "{:<10} {:<20} {:<5} ${:<9.2} ${:<9.2} {:<8} {:<10} {:<12}",
                            id_short,
                            truncate_str(title, 18),
                            qty,
                            unit_price,
                            line_cost,
                            nre_str,
                            style(source).dim(),
                            from_asm
                        );
                    } else {
                        println!(
                            "{:<10} {:<22} {:<5} ${:<9.2} ${:<9.2} {:<10} {:<15}",
                            id_short,
                            truncate_str(title, 20),
                            qty,
                            unit_price,
                            line_cost,
                            style(source).dim(),
                            from_asm
                        );
                    }
                } else if show_nre_col {
                    let nre_str = if *nre > 0.0 {
                        format!("${:.0}", nre)
                    } else {
                        "-".to_string()
                    };
                    println!(
                        "{:<10} {:<24} {:<5} ${:<9.2} ${:<9.2} {:<10} {}",
                        id_short,
                        truncate_str(title, 22),
                        qty,
                        unit_price,
                        line_cost,
                        nre_str,
                        style(source).dim()
                    );
                } else {
                    println!(
                        "{:<10} {:<26} {:<5} ${:<9.2} ${:<9.2} {}",
                        id_short,
                        truncate_str(title, 24),
                        qty,
                        unit_price,
                        line_cost,
                        style(source).dim()
                    );
                }
            } else if args.flat {
                if show_nre_col {
                    println!(
                        "{:<10} {:<20} {:<5} {:<10} {:<10} {:<8} {:<10} {:<12}",
                        id_short,
                        truncate_str(title, 18),
                        qty,
                        style("-").dim(),
                        style("-").dim(),
                        style("-").dim(),
                        style(source).dim(),
                        from_asm
                    );
                } else {
                    println!(
                        "{:<10} {:<22} {:<5} {:<10} {:<10} {:<10} {:<15}",
                        id_short,
                        truncate_str(title, 20),
                        qty,
                        style("-").dim(),
                        style("-").dim(),
                        style(source).dim(),
                        from_asm
                    );
                }
            } else if show_nre_col {
                println!(
                    "{:<10} {:<24} {:<5} {:<10} {:<10} {:<10} {}",
                    id_short,
                    truncate_str(title, 22),
                    qty,
                    style("-").dim(),
                    style("-").dim(),
                    style("-").dim(),
                    style(source).dim()
                );
            } else {
                println!(
                    "{:<10} {:<26} {:<5} {:<10} {:<10} {}",
                    id_short,
                    truncate_str(title, 24),
                    qty,
                    style("-").dim(),
                    style("-").dim(),
                    style(source).dim()
                );
            }
        }
        let line_width = if args.flat {
            if show_nre_col {
                95
            } else {
                90
            }
        } else if show_nre_col {
            85
        } else {
            75
        };
        println!("{}", "-".repeat(line_width));
    }

    // Cost summary
    println!("{} ${:.2}", style("Piece Cost:").bold(), total_piece_cost);

    // Calculate total cost (piece cost × production quantity)
    let total_run_cost = total_piece_cost * production_qty as f64;

    if include_nre && total_nre > 0.0 {
        println!("{} ${:.2}", style("Total NRE:").bold(), total_nre);

        // Total cost = (piece cost × qty) + NRE (one-time)
        let total_with_nre = total_run_cost + total_nre;

        if let Some(amort_qty) = args.amortize {
            println!(
                "{} ${:.4} (NRE / {} units)",
                style("NRE per Unit:").bold(),
                nre_per_unit,
                amort_qty
            );
            let effective_unit = total_piece_cost + nre_per_unit;
            println!(
                "{} ${:.4}",
                style("Effective Unit Cost:").green().bold(),
                effective_unit
            );
        }

        // Always show the combined total when there's NRE
        if production_qty > 1 {
            println!(
                "{} ${:.2} ({} units × ${:.2} + ${:.2} NRE)",
                style("Total Cost:").green().bold(),
                total_with_nre,
                production_qty,
                total_piece_cost,
                total_nre
            );
        } else {
            println!(
                "{} ${:.2} (piece + NRE)",
                style("Total Cost:").green().bold(),
                total_with_nre
            );
        }
    } else if production_qty > 1 {
        // Show both piece cost and total run cost when qty > 1
        println!(
            "{} ${:.2} ({} units × ${:.2})",
            style("Total Cost:").green().bold(),
            total_run_cost,
            production_qty,
            total_piece_cost
        );
    } else {
        println!(
            "{} ${:.2}",
            style("Total Cost:").green().bold(),
            total_piece_cost
        );
    }

    // Show warnings about expired quotes
    if !expired_quote_warnings.is_empty() && args.warn_expired {
        println!();
        println!(
            "{} {} quote(s) used in this BOM have expired:",
            style("⚠ Warning:").red().bold(),
            expired_quote_warnings.len()
        );
        for (quote_id, title, valid_until) in &expired_quote_warnings {
            let quote_short = short_ids
                .get_short_id(quote_id)
                .unwrap_or_else(|| truncate_str(quote_id, 10));
            println!(
                "   {} {} (quote {} expired {})",
                style("•").dim(),
                style(truncate_str(title, 30)).cyan(),
                style(&quote_short).yellow(),
                style(valid_until).red()
            );
        }
        println!(
            "   {}",
            style("Consider requesting updated quotes for accurate pricing").dim()
        );
    }

    // Show warnings about components with quotes but no selection
    if !unselected_quote_warnings.is_empty() {
        println!();
        println!(
            "{} Some components have quotes but no selected quote:",
            style("Note:").yellow().bold()
        );
        for (id, title, count) in &unselected_quote_warnings {
            let id_short = short_ids
                .get_short_id(id)
                .unwrap_or_else(|| truncate_str(id, 10));
            println!(
                "   {} {} ({} quote{}) - use: tdt cmp set-quote {} <quote-id>",
                style("•").dim(),
                style(truncate_str(title, 30)).cyan(),
                count,
                if *count == 1 { "" } else { "s" },
                id_short
            );
        }
        println!(
            "   {}",
            style("Run 'tdt quote compare <component>' to see available quotes").dim()
        );
    }

    Ok(())
}

/// Output cost result from cached calculation (flat mode)
fn output_cached_cost_result(
    assembly: &Assembly,
    result: &tdt_core::services::BomCostResultDetailed,
    args: &CostArgs,
    short_ids: &ShortIdIndex,
    expired_warnings: &[(String, String, String)],
    unselected_warnings: &[(String, String, usize)],
    include_nre: bool,
    production_qty: u32,
) -> Result<()> {
    let total_piece_cost = result.total_unit_cost;
    let total_nre = result.total_nre_cost;

    // Output header
    println!(
        "{} {}",
        style("Assembly:").bold(),
        style(&assembly.title).cyan()
    );
    println!("{} {}", style("Part Number:").bold(), assembly.part_number);
    println!(
        "{}",
        style("(flattened view - all components from subassemblies)").dim()
    );
    if production_qty > 1 {
        println!(
            "{} {}",
            style("Production Qty:").bold(),
            style(production_qty).yellow()
        );
    }
    if let Some(amort_qty) = args.amortize {
        if include_nre {
            println!(
                "{} {} units",
                style("NRE Amortization:").bold(),
                style(amort_qty).cyan()
            );
        }
    }
    println!();

    // Print breakdown table
    if (args.breakdown || args.flat) && !result.component_costs.is_empty() {
        let show_nre_col = include_nre && total_nre > 0.0;

        // Print header
        if show_nre_col {
            println!(
                "{:<10} {:<20} {:<5} {:<10} {:<10} {:<8} {:<10}",
                style("ID").bold(),
                style("TITLE").bold(),
                style("QTY").bold(),
                style("UNIT").bold(),
                style("LINE").bold(),
                style("NRE").bold(),
                style("SOURCE").bold()
            );
            println!("{}", "-".repeat(80));
        } else {
            println!(
                "{:<10} {:<22} {:<5} {:<10} {:<10} {:<10}",
                style("ID").bold(),
                style("TITLE").bold(),
                style("QTY").bold(),
                style("UNIT").bold(),
                style("LINE").bold(),
                style("SOURCE").bold()
            );
            println!("{}", "-".repeat(70));
        }

        for line in &result.component_costs {
            let id_short = short_ids
                .get_short_id(&line.component_id)
                .unwrap_or_else(|| truncate_str(&line.component_id, 8));

            let source = if line.quote_id.is_some() {
                format!("quote@{}", line.price_break_tier.unwrap_or(1))
            } else if line.unit_price.is_some() {
                "unit_cost".to_string()
            } else {
                "none".to_string()
            };

            if let Some(ext_price) = line.extended_price {
                if show_nre_col {
                    let nre_str = if line.nre_contribution > 0.0 {
                        format!("${:.0}", line.nre_contribution)
                    } else {
                        "-".to_string()
                    };
                    println!(
                        "{:<10} {:<20} {:<5} ${:<9.2} ${:<9.2} {:<8} {}",
                        id_short,
                        truncate_str(&line.title, 18),
                        line.effective_qty,
                        line.unit_price.unwrap_or(0.0),
                        ext_price,
                        nre_str,
                        style(&source).dim()
                    );
                } else {
                    println!(
                        "{:<10} {:<22} {:<5} ${:<9.2} ${:<9.2} {}",
                        id_short,
                        truncate_str(&line.title, 20),
                        line.effective_qty,
                        line.unit_price.unwrap_or(0.0),
                        ext_price,
                        style(&source).dim()
                    );
                }
            } else if show_nre_col {
                println!(
                    "{:<10} {:<20} {:<5} {:<10} {:<10} {:<8} {}",
                    id_short,
                    truncate_str(&line.title, 18),
                    line.effective_qty,
                    style("-").dim(),
                    style("-").dim(),
                    style("-").dim(),
                    style(&source).dim()
                );
            } else {
                println!(
                    "{:<10} {:<22} {:<5} {:<10} {:<10} {}",
                    id_short,
                    truncate_str(&line.title, 20),
                    line.effective_qty,
                    style("-").dim(),
                    style("-").dim(),
                    style(&source).dim()
                );
            }
        }

        let line_width = if show_nre_col { 80 } else { 70 };
        println!("{}", "-".repeat(line_width));
    }

    // Cost summary
    println!("{} ${:.2}", style("Piece Cost:").bold(), total_piece_cost);

    // Calculate total cost (piece cost × production quantity)
    let total_run_cost = total_piece_cost * production_qty as f64;

    if include_nre && total_nre > 0.0 {
        println!("{} ${:.2}", style("Total NRE:").bold(), total_nre);

        // Total cost = (piece cost × qty) + NRE (one-time)
        let total_with_nre = total_run_cost + total_nre;

        if let Some(amort_qty) = args.amortize {
            let nre_per_unit = total_nre / amort_qty as f64;
            println!(
                "{} ${:.4} (NRE / {} units)",
                style("NRE per Unit:").bold(),
                nre_per_unit,
                amort_qty
            );
            let effective_unit = total_piece_cost + nre_per_unit;
            println!(
                "{} ${:.4}",
                style("Effective Unit Cost:").green().bold(),
                effective_unit
            );
        }

        // Always show the combined total when there's NRE
        if production_qty > 1 {
            println!(
                "{} ${:.2} ({} units × ${:.2} + ${:.2} NRE)",
                style("Total Cost:").green().bold(),
                total_with_nre,
                production_qty,
                total_piece_cost,
                total_nre
            );
        } else {
            println!(
                "{} ${:.2} (piece + NRE)",
                style("Total Cost:").green().bold(),
                total_with_nre
            );
        }
    } else if production_qty > 1 {
        // Show both piece cost and total run cost when qty > 1
        println!(
            "{} ${:.2} ({} units × ${:.2})",
            style("Total Cost:").green().bold(),
            total_run_cost,
            production_qty,
            total_piece_cost
        );
    } else {
        println!(
            "{} ${:.2}",
            style("Total Cost:").green().bold(),
            total_piece_cost
        );
    }

    // Show warnings about expired quotes
    if !expired_warnings.is_empty() {
        println!();
        println!(
            "{} {} quote(s) used in this BOM have expired:",
            style("⚠ Warning:").red().bold(),
            expired_warnings.len()
        );
        for (quote_id, title, valid_until) in expired_warnings {
            let quote_short = short_ids
                .get_short_id(quote_id)
                .unwrap_or_else(|| truncate_str(quote_id, 10));
            println!(
                "   {} {} (quote {} expired {})",
                style("•").dim(),
                style(truncate_str(title, 30)).cyan(),
                style(&quote_short).yellow(),
                style(valid_until).red()
            );
        }
        println!(
            "   {}",
            style("Consider requesting updated quotes for accurate pricing").dim()
        );
    }

    // Show warnings about components with quotes but no selection
    if !unselected_warnings.is_empty() {
        println!();
        println!(
            "{} Some components have quotes but no selected quote:",
            style("Note:").yellow().bold()
        );
        for (id, title, count) in unselected_warnings {
            let id_short = short_ids
                .get_short_id(id)
                .unwrap_or_else(|| truncate_str(id, 10));
            println!(
                "   {} {} ({} quote{}) - use: tdt cmp set-quote {} <quote-id>",
                style("•").dim(),
                style(truncate_str(title, 30)).cyan(),
                count,
                if *count == 1 { "" } else { "s" },
                id_short
            );
        }
        println!(
            "   {}",
            style("Run 'tdt quote compare <component>' to see available quotes").dim()
        );
    }

    Ok(())
}

/// Load all quotes from the project
fn load_all_quotes(project: &Project) -> Vec<tdt_core::entities::quote::Quote> {
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
                tdt_core::yaml::parse_yaml_file::<tdt_core::entities::quote::Quote>(entry.path())
            {
                quotes.push(quote);
            }
        }
    }

    quotes
}

fn run_mass(args: MassArgs) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let short_ids = ShortIdIndex::load(&project);

    // Resolve assembly ID
    let resolved_id = short_ids
        .resolve(&args.assembly)
        .unwrap_or_else(|| args.assembly.clone());

    // Load assembly
    let assembly = find_assembly(&project, &resolved_id)?;

    // Load all components and assemblies for lookup
    let components = load_all_components(&project);
    let component_map: std::collections::HashMap<String, &Component> =
        components.iter().map(|c| (c.id.to_string(), c)).collect();

    let assemblies = load_all_assemblies(&project);
    let assembly_map: std::collections::HashMap<String, &Assembly> =
        assemblies.iter().map(|a| (a.id.to_string(), a)).collect();

    // Calculate mass recursively
    let mut breakdown: Vec<(String, String, u32, f64)> = Vec::new(); // (id, title, qty, mass)
    let mut visited = std::collections::HashSet::new();
    visited.insert(assembly.id.to_string());

    fn calculate_bom_mass(
        bom: &[tdt_core::entities::assembly::BomItem],
        subassemblies: &[String],
        component_map: &std::collections::HashMap<String, &Component>,
        assembly_map: &std::collections::HashMap<String, &Assembly>,
        breakdown: &mut Vec<(String, String, u32, f64)>,
        visited: &mut std::collections::HashSet<String>,
    ) -> f64 {
        let mut total = 0.0;
        for item in bom {
            let item_id = item.component_id.to_string();
            if let Some(cmp) = component_map.get(&item_id) {
                let line_mass = cmp.mass_kg.unwrap_or(0.0) * item.quantity as f64;
                total += line_mass;
                breakdown.push((item_id, cmp.title.clone(), item.quantity, line_mass));
            } else if let Some(sub_asm) = assembly_map.get(&item_id) {
                if !visited.contains(&item_id) {
                    visited.insert(item_id.clone());
                    let sub_mass = calculate_bom_mass(
                        &sub_asm.bom,
                        &sub_asm.subassemblies,
                        component_map,
                        assembly_map,
                        breakdown,
                        visited,
                    );
                    let line_mass = sub_mass * item.quantity as f64;
                    total += line_mass;
                    breakdown.push((
                        item_id.clone(),
                        sub_asm.title.clone(),
                        item.quantity,
                        line_mass,
                    ));
                    visited.remove(&item_id);
                }
            }
        }
        // Process subassemblies field (quantity 1 each)
        for sub_id in subassemblies {
            if let Some(sub_asm) = assembly_map.get(sub_id.as_str()) {
                if !visited.contains(sub_id) {
                    visited.insert(sub_id.clone());
                    let sub_mass = calculate_bom_mass(
                        &sub_asm.bom,
                        &sub_asm.subassemblies,
                        component_map,
                        assembly_map,
                        breakdown,
                        visited,
                    );
                    total += sub_mass;
                    breakdown.push((sub_id.clone(), sub_asm.title.clone(), 1, sub_mass));
                    visited.remove(sub_id);
                }
            }
        }
        total
    }

    let total_mass = calculate_bom_mass(
        &assembly.bom,
        &assembly.subassemblies,
        &component_map,
        &assembly_map,
        &mut breakdown,
        &mut visited,
    );

    // Output
    println!(
        "{} {}",
        style("Assembly:").bold(),
        style(&assembly.title).cyan()
    );
    println!(
        "{} {}\n",
        style("Part Number:").bold(),
        assembly.part_number
    );

    if args.breakdown && !breakdown.is_empty() {
        println!(
            "{:<12} {:<30} {:<6} {:<12}",
            style("ID").bold(),
            style("TITLE").bold(),
            style("QTY").bold(),
            style("MASS (kg)").bold()
        );
        println!("{}", "-".repeat(65));
        for (id, title, qty, mass) in &breakdown {
            let id_short = short_ids
                .get_short_id(id)
                .unwrap_or_else(|| truncate_str(id, 10));
            if *mass > 0.0 {
                println!(
                    "{:<12} {:<30} {:<6} {:.3}",
                    id_short,
                    truncate_str(title, 28),
                    qty,
                    mass
                );
            }
        }
        println!("{}", "-".repeat(65));
    }

    println!(
        "{} {:.3} kg",
        style("Total Mass:").green().bold(),
        total_mass
    );

    Ok(())
}

fn find_assembly(project: &Project, id: &str) -> Result<Assembly> {
    use tdt_core::core::cache::EntityCache;

    // Try cache-based lookup first (O(1) via SQLite)
    if let Ok(cache) = EntityCache::open(project) {
        // Resolve short ID if needed
        let full_id = if id.contains('@') {
            cache.resolve_short_id(id)
        } else {
            None
        };

        let lookup_id = full_id.as_deref().unwrap_or(id);

        // Try exact match via cache
        if let Some(entity) = cache.get_entity(lookup_id) {
            if entity.prefix == "ASM" {
                if let Ok(asm) = tdt_core::yaml::parse_yaml_file::<Assembly>(&entity.file_path) {
                    return Ok(asm);
                }
            }
        }

        // Try prefix match via cache
        if lookup_id.starts_with("ASM-") {
            let filter = tdt_core::core::EntityFilter {
                prefix: Some(tdt_core::core::EntityPrefix::Asm),
                search: Some(lookup_id.to_string()),
                ..Default::default()
            };
            let matches: Vec<_> = cache.list_entities(&filter);
            if matches.len() == 1 {
                if let Ok(asm) = tdt_core::yaml::parse_yaml_file::<Assembly>(&matches[0].file_path)
                {
                    return Ok(asm);
                }
            }
        }
    }

    // Fallback: filesystem search
    let asm_dir = project.root().join("bom/assemblies");

    if asm_dir.exists() {
        for entry in walkdir::WalkDir::new(&asm_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        {
            if let Ok(asm) = tdt_core::yaml::parse_yaml_file::<Assembly>(entry.path()) {
                if asm.id.to_string() == id || asm.id.to_string().starts_with(id) {
                    return Ok(asm);
                }
            }
        }
    }

    Err(miette::miette!("Assembly not found: {}", id))
}

fn load_all_components(project: &Project) -> Vec<Component> {
    let mut components = Vec::new();
    let dir = project.root().join("bom/components");

    if dir.exists() {
        for entry in walkdir::WalkDir::new(&dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        {
            if let Ok(cmp) = tdt_core::yaml::parse_yaml_file::<Component>(entry.path()) {
                components.push(cmp);
            }
        }
    }

    components
}

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
            if let Ok(asm) = tdt_core::yaml::parse_yaml_file::<Assembly>(entry.path()) {
                assemblies.push(asm);
            }
        }
    }

    assemblies
}

/// Collect all component and assembly IDs needed for a BOM hierarchy using cache
/// This is faster than loading all entities from filesystem
fn collect_bom_ids_from_cache(
    cache: &EntityCache,
    assembly_id: &str,
) -> (HashSet<String>, HashSet<String>) {
    let mut component_ids = HashSet::new();
    let mut assembly_ids = HashSet::new();
    let mut visited = HashSet::new();
    visited.insert(assembly_id.to_string());

    fn collect_recursive(
        cache: &EntityCache,
        asm_id: &str,
        component_ids: &mut HashSet<String>,
        assembly_ids: &mut HashSet<String>,
        visited: &mut HashSet<String>,
    ) {
        // Get BOM items (components)
        let bom_items = cache.get_bom_items(asm_id);
        for item in bom_items {
            component_ids.insert(item.component_id.clone());
        }

        // Get subassembly items
        let subasm_items = cache.get_subassembly_items(asm_id);
        for item in subasm_items {
            if !visited.contains(&item.assembly_id) {
                visited.insert(item.assembly_id.clone());
                assembly_ids.insert(item.assembly_id.clone());
                collect_recursive(
                    cache,
                    &item.assembly_id,
                    component_ids,
                    assembly_ids,
                    visited,
                );
            }
        }
    }

    collect_recursive(
        cache,
        assembly_id,
        &mut component_ids,
        &mut assembly_ids,
        &mut visited,
    );
    (component_ids, assembly_ids)
}

/// Load specific components by ID (faster than loading all)
fn load_components_by_ids(project: &Project, ids: &HashSet<String>) -> Vec<Component> {
    if ids.is_empty() {
        return Vec::new();
    }

    let mut components = Vec::new();
    let dir = project.root().join("bom/components");

    if dir.exists() {
        for entry in walkdir::WalkDir::new(&dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        {
            if let Ok(cmp) = tdt_core::yaml::parse_yaml_file::<Component>(entry.path()) {
                if ids.contains(&cmp.id.to_string()) {
                    components.push(cmp);
                    if components.len() == ids.len() {
                        break; // Found all we need
                    }
                }
            }
        }
    }

    components
}

/// Load specific assemblies by ID (faster than loading all)
fn load_assemblies_by_ids(project: &Project, ids: &HashSet<String>) -> Vec<Assembly> {
    if ids.is_empty() {
        return Vec::new();
    }

    let mut assemblies = Vec::new();
    let dir = project.root().join("bom/assemblies");

    if dir.exists() {
        for entry in walkdir::WalkDir::new(&dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        {
            if let Ok(asm) = tdt_core::yaml::parse_yaml_file::<Assembly>(entry.path()) {
                if ids.contains(&asm.id.to_string()) {
                    assemblies.push(asm);
                    if assemblies.len() == ids.len() {
                        break; // Found all we need
                    }
                }
            }
        }
    }

    assemblies
}

/// Recursively collect all sub-assembly IDs from a parent assembly's BOM
fn collect_subassembly_ids(
    assembly: &Assembly,
    assembly_map: &std::collections::HashMap<String, &Assembly>,
    sub_asm_ids: &mut HashSet<String>,
    visited: &mut HashSet<String>,
) {
    for item in &assembly.bom {
        // Check if this BOM item is an assembly (ASM-* prefix)
        if item.component_id.starts_with("ASM-") {
            let asm_id = item.component_id.clone();
            if !visited.contains(&asm_id) {
                sub_asm_ids.insert(asm_id.clone());
                visited.insert(asm_id.clone());

                // Recurse into sub-assembly
                if let Some(sub_asm) = assembly_map.get(&asm_id) {
                    collect_subassembly_ids(sub_asm, assembly_map, sub_asm_ids, visited);
                }
            }
        }
    }

    // Also check the subassemblies field
    for sub_id in &assembly.subassemblies {
        if !visited.contains(sub_id) {
            sub_asm_ids.insert(sub_id.clone());
            visited.insert(sub_id.clone());

            if let Some(sub_asm) = assembly_map.get(sub_id) {
                collect_subassembly_ids(sub_asm, assembly_map, sub_asm_ids, visited);
            }
        }
    }
}

// ==================== Routing Subcommands ====================

fn run_routing_add(args: RoutingAddArgs) -> Result<()> {
    use tdt_core::entities::assembly::ManufacturingConfig;
    use tdt_core::entities::process::Process;

    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let short_ids = ShortIdIndex::load(&project);

    // Resolve assembly ID
    let asm_id = short_ids
        .resolve(&args.asm)
        .unwrap_or_else(|| args.asm.clone());

    // Resolve process ID (short ID -> full ID for storage)
    let proc_id = short_ids
        .resolve(&args.proc)
        .unwrap_or_else(|| args.proc.clone());

    // Find and load the assembly
    let (mut assembly, path) = find_assembly_file(&project, &asm_id)?;

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
    if assembly.manufacturing.is_none() {
        assembly.manufacturing = Some(ManufacturingConfig::default());
    }

    // Add at position or append
    let (position, new_len) = {
        let mfg = assembly.manufacturing.as_mut().unwrap();

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
    let yaml = serde_yml::to_string(&assembly).into_diagnostic()?;
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

    // Resolve assembly ID
    let asm_id = short_ids
        .resolve(&args.asm)
        .unwrap_or_else(|| args.asm.clone());

    // Find and load the assembly
    let (mut assembly, path) = find_assembly_file(&project, &asm_id)?;

    // Remove from routing and capture results
    let (removed, new_len) = {
        let mfg = assembly.manufacturing.as_mut().ok_or_else(|| {
            miette::miette!(
                "Assembly {} has no manufacturing routing configured",
                args.asm
            )
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
                    miette::miette!("Process {} not found in routing", args.proc_or_position)
                })?;
            mfg.routing.remove(pos)
        };
        (removed, mfg.routing.len())
    };

    // Save
    let yaml = serde_yml::to_string(&assembly).into_diagnostic()?;
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
    use tdt_core::entities::process::Process;

    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let short_ids = ShortIdIndex::load(&project);

    // Resolve assembly ID
    let asm_id = short_ids
        .resolve(&args.asm)
        .unwrap_or_else(|| args.asm.clone());

    // Find and load the assembly
    let (assembly, _path) = find_assembly_file(&project, &asm_id)?;

    let mfg = assembly.manufacturing.as_ref();
    let routing = mfg.map(|m| m.routing.as_slice()).unwrap_or(&[]);

    if routing.is_empty() {
        println!("No routing configured for assembly {}", args.asm);
        return Ok(());
    }

    // Load process titles if not using --ids
    let mut proc_map: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    if !args.ids {
        let proc_dir = project.root().join("manufacturing/processes");
        if proc_dir.exists() {
            for entry in fs::read_dir(&proc_dir).into_diagnostic()? {
                let entry = entry.into_diagnostic()?;
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "yaml") {
                    if let Ok(content) = fs::read_to_string(&path) {
                        if let Ok(proc) = serde_yml::from_str::<Process>(&content) {
                            proc_map.insert(proc.id.to_string(), proc.title.clone());
                        }
                    }
                }
            }
        }
    }

    println!(
        "{} Routing for {} ({})",
        style("Manufacturing").bold(),
        style(&assembly.part_number).yellow(),
        style(&assembly.title).dim()
    );
    println!();

    for (idx, proc_id) in routing.iter().enumerate() {
        let display = if args.ids {
            proc_id.clone()
        } else {
            let title = proc_map
                .get(proc_id)
                .cloned()
                .unwrap_or_else(|| "(unknown)".to_string());
            let short = short_ids
                .get_short_id(proc_id)
                .unwrap_or_else(|| truncate_str(proc_id, 12).to_string());
            format!("{} ({})", title, short)
        };

        println!("  {}. {}", style(idx + 1).cyan(), display);
    }

    Ok(())
}

fn run_routing_set(args: RoutingSetArgs) -> Result<()> {
    use tdt_core::entities::assembly::ManufacturingConfig;

    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let short_ids = ShortIdIndex::load(&project);

    // Resolve assembly ID
    let asm_id = short_ids
        .resolve(&args.asm)
        .unwrap_or_else(|| args.asm.clone());

    // Find and load the assembly
    let (mut assembly, path) = find_assembly_file(&project, &asm_id)?;

    // Resolve all process IDs (short IDs -> full IDs for storage)
    let resolved_procs: Vec<String> = args
        .procs
        .iter()
        .map(|p| short_ids.resolve(p).unwrap_or_else(|| p.clone()))
        .collect();

    // Initialize or update manufacturing config
    if assembly.manufacturing.is_none() {
        assembly.manufacturing = Some(ManufacturingConfig::default());
    }

    let mfg = assembly.manufacturing.as_mut().unwrap();
    mfg.routing = resolved_procs.clone();

    // Save
    let yaml = serde_yml::to_string(&assembly).into_diagnostic()?;
    fs::write(&path, yaml).into_diagnostic()?;

    println!(
        "{} Set routing with {} step{}",
        style("✓").green(),
        style(resolved_procs.len()).cyan(),
        if resolved_procs.len() == 1 { "" } else { "s" }
    );

    for (idx, proc_id) in resolved_procs.iter().enumerate() {
        let short = short_ids
            .get_short_id(proc_id)
            .unwrap_or_else(|| truncate_str(proc_id, 12).to_string());
        println!("  {}. {}", style(idx + 1).dim(), short);
    }

    Ok(())
}

/// Find an assembly file and return both the parsed Assembly and file path
fn find_assembly_file(project: &Project, id: &str) -> Result<(Assembly, std::path::PathBuf)> {
    let asm_dir = project.root().join("bom/assemblies");

    if asm_dir.exists() {
        for entry in fs::read_dir(&asm_dir).into_diagnostic()? {
            let entry = entry.into_diagnostic()?;
            let path = entry.path();

            if path.extension().is_some_and(|e| e == "yaml") {
                let filename = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                if filename.contains(id) || filename.starts_with(id) {
                    let content = fs::read_to_string(&path).into_diagnostic()?;
                    if let Ok(asm) = serde_yml::from_str::<Assembly>(&content) {
                        if asm.id.to_string() == id || asm.id.to_string().starts_with(id) {
                            return Ok((asm, path));
                        }
                    }
                }
            }
        }
    }

    Err(miette::miette!("Assembly not found: {}", id))
}
