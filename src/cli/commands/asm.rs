//! `tdt asm` command - Assembly management

use clap::{Subcommand, ValueEnum};
use console::style;
use miette::{IntoDiagnostic, Result};
use std::collections::HashSet;
use std::fmt;
use std::fs;

use crate::cli::filters::StatusFilter;
use crate::cli::helpers::{escape_csv, truncate_str};
use crate::cli::table::{CellValue, ColumnDef, TableConfig, TableFormatter, TableRow};
use crate::cli::{GlobalOpts, OutputFormat};
use crate::core::identity::{EntityId, EntityPrefix};
use crate::core::project::Project;
use crate::core::shortid::ShortIdIndex;
use crate::core::Config;
use crate::entities::assembly::Assembly;
use crate::entities::component::Component;
use crate::schema::template::{TemplateContext, TemplateGenerator};
use crate::schema::wizard::SchemaWizard;

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

impl ListColumn {
    pub fn as_str(&self) -> &'static str {
        match self {
            ListColumn::Short => "short",
            ListColumn::Id => "id",
            ListColumn::PartNumber => "part-number",
            ListColumn::Title => "title",
            ListColumn::Status => "status",
            ListColumn::Author => "author",
            ListColumn::Created => "created",
        }
    }
}

/// Column definitions for TableFormatter
const ASM_COLUMNS: &[ColumnDef] = &[
    ColumnDef::new("short", "SHORT", 8),
    ColumnDef::new("id", "ID", 17),
    ColumnDef::new("part-number", "PART #", 12),
    ColumnDef::new("title", "TITLE", 30),
    ColumnDef::new("status", "STATUS", 10),
    ColumnDef::new("author", "AUTHOR", 15),
    ColumnDef::new("created", "CREATED", 20),
];

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
    #[arg(long, value_delimiter = ',', default_values_t = vec![ListColumn::Short, ListColumn::PartNumber, ListColumn::Title, ListColumn::Status])]
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

/// Directories where assemblies are stored
const ASSEMBLY_DIRS: &[&str] = &["bom/assemblies"];

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
    }
}

fn run_list(args: ListArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let asm_dir = project.root().join("bom/assemblies");

    if !asm_dir.exists() {
        if args.count {
            println!("0");
        } else {
            println!("No assemblies found.");
        }
        return Ok(());
    }

    // Load and parse all assemblies
    let mut assemblies: Vec<Assembly> = Vec::new();

    for entry in fs::read_dir(&asm_dir).into_diagnostic()? {
        let entry = entry.into_diagnostic()?;
        let path = entry.path();

        if path.extension().is_some_and(|e| e == "yaml") {
            let content = fs::read_to_string(&path).into_diagnostic()?;
            if let Ok(asm) = serde_yml::from_str::<Assembly>(&content) {
                assemblies.push(asm);
            }
        }
    }

    // Apply filters
    let assemblies: Vec<Assembly> = assemblies
        .into_iter()
        .filter(|a| match args.status {
            StatusFilter::Draft => a.status == crate::core::entity::Status::Draft,
            StatusFilter::Review => a.status == crate::core::entity::Status::Review,
            StatusFilter::Approved => a.status == crate::core::entity::Status::Approved,
            StatusFilter::Released => a.status == crate::core::entity::Status::Released,
            StatusFilter::Obsolete => a.status == crate::core::entity::Status::Obsolete,
            StatusFilter::Active => a.status != crate::core::entity::Status::Obsolete,
            StatusFilter::All => true,
        })
        .filter(|a| {
            if let Some(ref search) = args.search {
                let search_lower = search.to_lowercase();
                a.part_number.to_lowercase().contains(&search_lower)
                    || a.title.to_lowercase().contains(&search_lower)
                    || a.description
                        .as_ref()
                        .is_some_and(|d| d.to_lowercase().contains(&search_lower))
            } else {
                true
            }
        })
        .filter(|a| {
            if let Some(ref author) = args.author {
                let author_lower = author.to_lowercase();
                a.author.to_lowercase().contains(&author_lower)
            } else {
                true
            }
        })
        .collect();

    // Filter by parent assembly (sub-assemblies only)
    let assemblies = if let Some(ref parent_asm) = args.assembly {
        let short_ids_tmp = ShortIdIndex::load(&project);
        let parent_id = short_ids_tmp
            .resolve(parent_asm)
            .unwrap_or_else(|| parent_asm.clone());

        // Build assembly map for recursive lookup
        let assembly_map: std::collections::HashMap<String, &Assembly> =
            assemblies.iter().map(|a| (a.id.to_string(), a)).collect();

        // Find the parent assembly
        if let Some(parent) = assembly_map.get(&parent_id) {
            // Collect all sub-assembly IDs recursively
            let mut sub_asm_ids: HashSet<String> = HashSet::new();
            let mut visited: HashSet<String> = HashSet::new();
            visited.insert(parent_id.clone());

            collect_subassembly_ids(parent, &assembly_map, &mut sub_asm_ids, &mut visited);

            // Filter to only those sub-assemblies
            assemblies
                .into_iter()
                .filter(|a| sub_asm_ids.contains(&a.id.to_string()))
                .collect()
        } else {
            eprintln!(
                "Warning: Assembly '{}' not found, showing all assemblies",
                parent_asm
            );
            assemblies
        }
    } else {
        assemblies
    };

    // Sort
    let mut assemblies = assemblies;
    match args.sort {
        ListColumn::Short | ListColumn::Id => {
            assemblies.sort_by(|a, b| a.id.to_string().cmp(&b.id.to_string()))
        }
        ListColumn::PartNumber => assemblies.sort_by(|a, b| a.part_number.cmp(&b.part_number)),
        ListColumn::Title => assemblies.sort_by(|a, b| a.title.cmp(&b.title)),
        ListColumn::Status => {
            assemblies.sort_by(|a, b| format!("{:?}", a.status).cmp(&format!("{:?}", b.status)))
        }
        ListColumn::Author => assemblies.sort_by(|a, b| a.author.cmp(&b.author)),
        ListColumn::Created => assemblies.sort_by(|a, b| a.created.cmp(&b.created)),
    }

    if args.reverse {
        assemblies.reverse();
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

    // Count only
    if args.count {
        println!("{}", assemblies.len());
        return Ok(());
    }

    // No results
    if assemblies.is_empty() {
        println!("No assemblies found.");
        return Ok(());
    }

    // Update short ID index
    let mut short_ids = ShortIdIndex::load(&project);
    short_ids.ensure_all(assemblies.iter().map(|a| a.id.to_string()));
    super::utils::save_short_ids(&mut short_ids, &project);

    // Build rows for table output
    let rows: Vec<TableRow> = assemblies
        .iter()
        .map(|asm| asm_to_row(asm, &short_ids))
        .collect();

    // Map ListColumn to column name strings
    let columns: Vec<&str> = args.columns.iter().map(|c| c.as_str()).collect();

    // Output based on format
    let format = match global.format {
        OutputFormat::Auto => OutputFormat::Tsv,
        f => f,
    };

    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&assemblies).into_diagnostic()?;
            println!("{}", json);
        }
        OutputFormat::Yaml => {
            let yaml = serde_yml::to_string(&assemblies).into_diagnostic()?;
            print!("{}", yaml);
        }
        OutputFormat::Id | OutputFormat::ShortId => {
            for asm in &assemblies {
                if format == OutputFormat::ShortId {
                    let short_id = short_ids
                        .get_short_id(&asm.id.to_string())
                        .unwrap_or_default();
                    println!("{}", short_id);
                } else {
                    println!("{}", asm.id);
                }
            }
        }
        OutputFormat::Tsv | OutputFormat::Csv | OutputFormat::Md => {
            let config = TableConfig {
                wrap_width: args.wrap,
                show_summary: true,
            };
            let formatter = TableFormatter::new(ASM_COLUMNS, "assembly", "ASM").with_config(config);
            formatter.output(rows, format, &columns);
        }
        OutputFormat::Auto | OutputFormat::Path => unreachable!(),
    }

    Ok(())
}

/// Convert an Assembly to a TableRow
fn asm_to_row(asm: &Assembly, short_ids: &ShortIdIndex) -> TableRow {
    TableRow::new(asm.id.to_string(), short_ids)
        .cell("short", CellValue::ShortId(asm.id.to_string()))
        .cell("id", CellValue::Id(asm.id.to_string()))
        .cell("part-number", CellValue::Text(asm.part_number.clone()))
        .cell("title", CellValue::Text(asm.title.clone()))
        .cell("status", CellValue::Status(asm.status))
        .cell("author", CellValue::Text(asm.author.clone()))
        .cell("created", CellValue::DateTime(asm.created))
}

fn run_new(args: NewArgs, global: &GlobalOpts) -> Result<()> {
    use crate::entities::assembly::BomItem;

    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let config = Config::load();

    let part_number: String;
    let title: String;
    let description: Option<String>;
    let revision: Option<String>;

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

    // Generate ID
    let id = EntityId::new(EntityPrefix::Asm);

    // Generate template
    let generator = TemplateGenerator::new().map_err(|e| miette::miette!("{}", e))?;
    let ctx = TemplateContext::new(id.clone(), config.author())
        .with_title(&title)
        .with_part_number(&part_number);

    let ctx = if let Some(ref rev) = revision {
        ctx.with_part_revision(rev)
    } else {
        ctx
    };

    let mut yaml_content = generator
        .generate_assembly(&ctx)
        .map_err(|e| miette::miette!("{}", e))?;

    // Apply wizard description via string replacement (for interactive mode)
    if args.interactive {
        if let Some(ref desc) = description {
            if !desc.is_empty() {
                let indented = desc
                    .lines()
                    .map(|line| format!("  {}", line))
                    .collect::<Vec<_>>()
                    .join("\n");
                yaml_content = yaml_content.replace(
                    "description: |\n  # Detailed description of this assembly\n  # Include key specifications and assembly requirements",
                    &format!("description: |\n{}", indented),
                );
            }
        }
    }

    // Write file
    let output_dir = project.root().join("bom/assemblies");
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
        &id,
        &file_path,
        short_id.clone(),
        ENTITY_CONFIG.name,
        &title,
        Some(&extra_info),
        &added_links,
        global,
    );

    // Handle BOM items if provided
    if !args.bom.is_empty() {
        // Load the assembly we just created
        let content = fs::read_to_string(&file_path).into_diagnostic()?;
        let mut assembly: Assembly = serde_yml::from_str(&content).into_diagnostic()?;

        let mut added_count = 0;
        for item_str in &args.bom {
            let (component_id, qty) = parse_bom_item(item_str)?;

            // Resolve short ID
            let resolved_id = short_ids
                .resolve(&component_id)
                .unwrap_or_else(|| component_id.clone());

            // Add to BOM
            assembly.bom.push(BomItem {
                component_id: resolved_id.clone(),
                quantity: qty,
                reference_designators: Vec::new(),
                notes: None,
            });
            added_count += 1;
        }

        // Save the updated assembly
        let updated_yaml = serde_yml::to_string(&assembly).into_diagnostic()?;
        fs::write(&file_path, updated_yaml).into_diagnostic()?;

        println!(
            "   {} Added {} BOM item{}",
            style("→").dim(),
            style(added_count).cyan(),
            if added_count == 1 { "" } else { "s" }
        );
    }

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

    // Resolve short ID if needed
    let short_ids = ShortIdIndex::load(&project);
    let resolved_id = short_ids
        .resolve(&args.id)
        .unwrap_or_else(|| args.id.clone());

    // Find the assembly file
    let asm_dir = project.root().join("bom/assemblies");
    let mut found_path = None;

    if asm_dir.exists() {
        for entry in fs::read_dir(&asm_dir).into_diagnostic()? {
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
        found_path.ok_or_else(|| miette::miette!("No assembly found matching '{}'", args.id))?;

    // Read and parse assembly
    let content = fs::read_to_string(&path).into_diagnostic()?;
    let asm: Assembly = serde_yml::from_str(&content).into_diagnostic()?;

    match global.format {
        OutputFormat::Yaml => {
            print!("{}", content);
        }
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&asm).into_diagnostic()?;
            println!("{}", json);
        }
        OutputFormat::Id | OutputFormat::ShortId => {
            if global.format == OutputFormat::ShortId {
                let short_ids = ShortIdIndex::load(&project);
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

    // Resolve short ID if needed
    let short_ids = ShortIdIndex::load(&project);
    let resolved_id = short_ids
        .resolve(&args.id)
        .unwrap_or_else(|| args.id.clone());

    // Find and load the assembly
    let asm_dir = project.root().join("bom/assemblies");
    let mut found_asm = None;

    if asm_dir.exists() {
        for entry in fs::read_dir(&asm_dir).into_diagnostic()? {
            let entry = entry.into_diagnostic()?;
            let path = entry.path();

            if path.extension().is_some_and(|e| e == "yaml") {
                let filename = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                if filename.contains(&resolved_id) || filename.starts_with(&resolved_id) {
                    let content = fs::read_to_string(&path).into_diagnostic()?;
                    if let Ok(asm) = serde_yml::from_str::<Assembly>(&content) {
                        found_asm = Some(asm);
                        break;
                    }
                }
            }
        }
    }

    let assembly =
        found_asm.ok_or_else(|| miette::miette!("No assembly found matching '{}'", args.id))?;

    // Load component index for resolving names
    let cmp_dir = project.root().join("bom/components");
    let mut components: std::collections::HashMap<String, Component> =
        std::collections::HashMap::new();

    if cmp_dir.exists() {
        for entry in fs::read_dir(&cmp_dir).into_diagnostic()? {
            let entry = entry.into_diagnostic()?;
            let path = entry.path();

            if path.extension().is_some_and(|e| e == "yaml") {
                let content = fs::read_to_string(&path).into_diagnostic()?;
                if let Ok(cmp) = serde_yml::from_str::<Component>(&content) {
                    components.insert(cmp.id.to_string(), cmp);
                }
            }
        }
    }

    // Display BOM
    let format = match global.format {
        OutputFormat::Auto => OutputFormat::Tsv,
        f => f,
    };

    println!();
    println!(
        "{} BOM for {} - {}",
        style("Assembly").bold(),
        style(&assembly.part_number).yellow(),
        style(&assembly.title).white()
    );
    println!();

    match format {
        OutputFormat::Tsv | OutputFormat::Auto => {
            println!(
                "{:<6} {:<15} {:<12} {:<30} {:<20}",
                style("QTY").bold(),
                style("COMPONENT ID").bold(),
                style("PART #").bold(),
                style("TITLE").bold(),
                style("REFERENCES").bold()
            );
            println!("{}", "-".repeat(85));

            for item in &assembly.bom {
                let cmp_info = components.get(&item.component_id);
                let part_number = cmp_info.map(|c| c.part_number.as_str()).unwrap_or("-");
                let title = cmp_info.map(|c| c.title.as_str()).unwrap_or("(not found)");
                let refs = if item.reference_designators.is_empty() {
                    String::new()
                } else {
                    item.reference_designators.join(", ")
                };

                println!(
                    "{:<6} {:<15} {:<12} {:<30} {:<20}",
                    item.quantity,
                    truncate_str(&item.component_id, 13),
                    truncate_str(part_number, 10),
                    truncate_str(title, 28),
                    truncate_str(&refs, 18)
                );
            }

            if !assembly.subassemblies.is_empty() {
                println!();
                println!("{}", style("Sub-assemblies:").bold());
                for sub_id in &assembly.subassemblies {
                    println!("  - {}", sub_id);
                }
            }

            println!();
            println!(
                "{} Total: {} line items, {} total components",
                style("Summary").bold(),
                assembly.bom.len(),
                assembly.total_component_count()
            );
        }
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&assembly.bom).into_diagnostic()?;
            println!("{}", json);
        }
        OutputFormat::Yaml => {
            let yaml = serde_yml::to_string(&assembly.bom).into_diagnostic()?;
            print!("{}", yaml);
        }
        OutputFormat::Csv => {
            println!("quantity,component_id,part_number,title,reference_designators,notes");
            for item in &assembly.bom {
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
        _ => {}
    }

    Ok(())
}

fn run_add_component(args: AddComponentArgs) -> Result<()> {
    use crate::entities::assembly::BomItem;

    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let short_ids = ShortIdIndex::load(&project);

    if args.components.is_empty() {
        return Err(miette::miette!(
            "At least one component is required.\n\
             Usage: tdt asm add ASM@1 CMP@1:2 CMP@2:1\n\
                    tdt asm add ASM@1 CMP@1 --qty 2"
        ));
    }

    // Resolve assembly ID
    let asm_id = short_ids
        .resolve(&args.assembly)
        .unwrap_or_else(|| args.assembly.clone());

    // Find and load the assembly
    let asm_dir = project.root().join("bom/assemblies");
    let mut found_path = None;
    let mut assembly: Option<Assembly> = None;

    if asm_dir.exists() {
        for entry in fs::read_dir(&asm_dir).into_diagnostic()? {
            let entry = entry.into_diagnostic()?;
            let path = entry.path();

            if path.extension().is_some_and(|e| e == "yaml") {
                let filename = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                if filename.contains(&asm_id) || filename.starts_with(&asm_id) {
                    let content = fs::read_to_string(&path).into_diagnostic()?;
                    if let Ok(asm) = serde_yml::from_str::<Assembly>(&content) {
                        assembly = Some(asm);
                        found_path = Some(path);
                        break;
                    }
                }
            }
        }
    }

    let mut assembly = assembly.ok_or_else(|| {
        miette::miette!(
            "Assembly '{}' not found. Create it first with: tdt asm new",
            args.assembly
        )
    })?;
    let path = found_path.unwrap();

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

        // Try to validate component exists and get info
        let cmp_dir = project.root().join("bom/components");
        let mut component_info: Option<Component> = None;

        if cmp_dir.exists() {
            for entry in fs::read_dir(&cmp_dir).into_diagnostic()? {
                let entry = entry.into_diagnostic()?;
                let entry_path = entry.path();

                if entry_path.extension().is_some_and(|e| e == "yaml") {
                    let filename = entry_path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("");
                    if filename.contains(&cmp_id) || filename.starts_with(&cmp_id) {
                        let content = fs::read_to_string(&entry_path).into_diagnostic()?;
                        if let Ok(cmp) = serde_yml::from_str::<Component>(&content) {
                            component_info = Some(cmp);
                            break;
                        }
                    }
                }
            }
        }

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

    // Save the updated assembly
    let yaml = serde_yml::to_string(&assembly).into_diagnostic()?;
    fs::write(&path, yaml).into_diagnostic()?;

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

    Ok(())
}

fn run_remove_component(args: RemoveComponentArgs) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let short_ids = ShortIdIndex::load(&project);

    // Resolve assembly ID
    let asm_id = short_ids
        .resolve(&args.assembly)
        .unwrap_or_else(|| args.assembly.clone());

    // Resolve component ID
    let cmp_id = short_ids
        .resolve(&args.component)
        .unwrap_or_else(|| args.component.clone());

    // Find and load the assembly
    let asm_dir = project.root().join("bom/assemblies");
    let mut found_path = None;
    let mut assembly: Option<Assembly> = None;

    if asm_dir.exists() {
        for entry in fs::read_dir(&asm_dir).into_diagnostic()? {
            let entry = entry.into_diagnostic()?;
            let path = entry.path();

            if path.extension().is_some_and(|e| e == "yaml") {
                let filename = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                if filename.contains(&asm_id) || filename.starts_with(&asm_id) {
                    let content = fs::read_to_string(&path).into_diagnostic()?;
                    if let Ok(asm) = serde_yml::from_str::<Assembly>(&content) {
                        assembly = Some(asm);
                        found_path = Some(path);
                        break;
                    }
                }
            }
        }
    }

    let mut assembly =
        assembly.ok_or_else(|| miette::miette!("Assembly '{}' not found", args.assembly))?;
    let path = found_path.unwrap();

    // Find and remove the component
    let original_len = assembly.bom.len();
    assembly
        .bom
        .retain(|item| !item.component_id.contains(&cmp_id));

    if assembly.bom.len() == original_len {
        return Err(miette::miette!(
            "Component '{}' not found in assembly '{}' BOM",
            args.component,
            args.assembly
        ));
    }

    // Save the updated assembly
    let yaml = serde_yml::to_string(&assembly).into_diagnostic()?;
    fs::write(&path, yaml).into_diagnostic()?;

    println!(
        "{} Removed {} from {}",
        style("✓").green(),
        style(&args.component).cyan(),
        style(&args.assembly).yellow()
    );
    println!("   BOM now has {} line items", assembly.bom.len());

    Ok(())
}

fn run_cost(args: CostArgs) -> Result<()> {
    use crate::entities::quote::Quote;

    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let short_ids = ShortIdIndex::load(&project);

    // Resolve assembly ID
    let resolved_id = short_ids
        .resolve(&args.assembly)
        .unwrap_or_else(|| args.assembly.clone());

    // Load assembly
    let assembly = find_assembly(&project, &resolved_id)?;

    // Load all components, assemblies, and quotes for lookup
    let components = load_all_components(&project);
    let component_map: std::collections::HashMap<String, &Component> =
        components.iter().map(|c| (c.id.to_string(), c)).collect();

    let assemblies = load_all_assemblies(&project);
    let assembly_map: std::collections::HashMap<String, &Assembly> =
        assemblies.iter().map(|a| (a.id.to_string(), a)).collect();

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

    let production_qty = args.qty;

    // Track components with quotes but no selection (for user feedback)
    let mut unselected_quote_warnings: Vec<(String, String, usize)> = Vec::new(); // (id, title, quote_count)

    // Calculate costs recursively
    // breakdown: (id, title, bom_qty, unit_price, line_cost, price_source)
    let mut breakdown: Vec<(String, String, u32, f64, f64, String)> = Vec::new();
    let mut visited = std::collections::HashSet::new();
    visited.insert(assembly.id.to_string());

    fn calculate_bom_cost(
        bom: &[crate::entities::assembly::BomItem],
        component_map: &std::collections::HashMap<String, &Component>,
        assembly_map: &std::collections::HashMap<String, &Assembly>,
        quote_map: &std::collections::HashMap<String, &Quote>,
        component_quotes: &std::collections::HashMap<String, Vec<&Quote>>,
        breakdown: &mut Vec<(String, String, u32, f64, f64, String)>,
        unselected_warnings: &mut Vec<(String, String, usize)>,
        visited: &mut std::collections::HashSet<String>,
        production_qty: u32,
    ) -> f64 {
        let mut total = 0.0;
        for item in bom {
            let item_id = item.component_id.to_string();
            if let Some(cmp) = component_map.get(&item_id) {
                // Determine price: selected quote > unit_cost > 0.0
                let purchase_qty = item.quantity * production_qty;
                let (unit_price, price_source) = get_component_price(
                    cmp,
                    quote_map,
                    component_quotes,
                    purchase_qty,
                    unselected_warnings,
                );

                let line_cost = unit_price * item.quantity as f64;
                total += line_cost;
                breakdown.push((
                    item_id,
                    cmp.title.clone(),
                    item.quantity,
                    unit_price,
                    line_cost,
                    price_source,
                ));
            } else if let Some(sub_asm) = assembly_map.get(&item_id) {
                if !visited.contains(&item_id) {
                    visited.insert(item_id.clone());
                    let sub_cost = calculate_bom_cost(
                        &sub_asm.bom,
                        component_map,
                        assembly_map,
                        quote_map,
                        component_quotes,
                        breakdown,
                        unselected_warnings,
                        visited,
                        production_qty,
                    );
                    let line_cost = sub_cost * item.quantity as f64;
                    total += line_cost;
                    breakdown.push((
                        item_id.clone(),
                        sub_asm.title.clone(),
                        item.quantity,
                        sub_cost,
                        line_cost,
                        "sub-asm".to_string(),
                    ));
                    visited.remove(&item_id);
                }
            }
        }
        total
    }

    fn get_component_price(
        cmp: &Component,
        quote_map: &std::collections::HashMap<String, &Quote>,
        component_quotes: &std::collections::HashMap<String, Vec<&Quote>>,
        purchase_qty: u32,
        unselected_warnings: &mut Vec<(String, String, usize)>,
    ) -> (f64, String) {
        // Priority 1: Use selected quote if set
        if let Some(ref quote_id) = cmp.selected_quote {
            if let Some(quote) = quote_map.get(quote_id) {
                if let Some(price) = quote.price_for_qty(purchase_qty) {
                    return (price, format!("quote@{}", purchase_qty));
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
            return (cost, "unit_cost".to_string());
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

        (0.0, "none".to_string())
    }

    let total_cost = calculate_bom_cost(
        &assembly.bom,
        &component_map,
        &assembly_map,
        &quote_map,
        &component_quotes,
        &mut breakdown,
        &mut unselected_quote_warnings,
        &mut visited,
        production_qty,
    );

    // Output
    println!(
        "{} {}",
        style("Assembly:").bold(),
        style(&assembly.title).cyan()
    );
    println!("{} {}", style("Part Number:").bold(), assembly.part_number);
    if production_qty > 1 {
        println!(
            "{} {}",
            style("Production Qty:").bold(),
            style(production_qty).yellow()
        );
    }
    println!();

    if args.breakdown && !breakdown.is_empty() {
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
        for (id, title, qty, unit_price, line_cost, source) in &breakdown {
            let id_short = short_ids
                .get_short_id(id)
                .unwrap_or_else(|| truncate_str(id, 8));
            if *line_cost > 0.0 || *unit_price > 0.0 {
                println!(
                    "{:<10} {:<26} {:<5} ${:<9.2} ${:<9.2} {}",
                    id_short,
                    truncate_str(title, 24),
                    qty,
                    unit_price,
                    line_cost,
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
        println!("{}", "-".repeat(75));
    }

    println!("{} ${:.2}", style("Total Cost:").green().bold(), total_cost);

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
        bom: &[crate::entities::assembly::BomItem],
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
        total
    }

    let total_mass = calculate_bom_mass(
        &assembly.bom,
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
    use crate::core::cache::EntityCache;

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
                if let Ok(asm) = crate::yaml::parse_yaml_file::<Assembly>(&entity.file_path) {
                    return Ok(asm);
                }
            }
        }

        // Try prefix match via cache
        if lookup_id.starts_with("ASM-") {
            let filter = crate::core::EntityFilter {
                prefix: Some(crate::core::EntityPrefix::Asm),
                search: Some(lookup_id.to_string()),
                ..Default::default()
            };
            let matches: Vec<_> = cache.list_entities(&filter);
            if matches.len() == 1 {
                if let Ok(asm) = crate::yaml::parse_yaml_file::<Assembly>(&matches[0].file_path) {
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
            if let Ok(asm) = crate::yaml::parse_yaml_file::<Assembly>(entry.path()) {
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
            if let Ok(cmp) = crate::yaml::parse_yaml_file::<Component>(entry.path()) {
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
            if let Ok(asm) = crate::yaml::parse_yaml_file::<Assembly>(entry.path()) {
                assemblies.push(asm);
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
