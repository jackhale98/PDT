//! `tdt asm` command - Assembly management

use clap::{Subcommand, ValueEnum};
use console::style;
use miette::{IntoDiagnostic, Result};
use std::fmt;
use std::fs;

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

    /// Show expanded BOM for an assembly
    Bom(BomArgs),
}

/// List column types
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ListColumn {
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
            ListColumn::Id => write!(f, "id"),
            ListColumn::PartNumber => write!(f, "part-number"),
            ListColumn::Title => write!(f, "title"),
            ListColumn::Status => write!(f, "status"),
            ListColumn::Author => write!(f, "author"),
            ListColumn::Created => write!(f, "created"),
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
    /// Filter by status
    #[arg(long, short = 's', default_value = "all")]
    pub status: StatusFilter,

    /// Search in part number and title
    #[arg(long)]
    pub search: Option<String>,

    /// Filter by author
    #[arg(long)]
    pub author: Option<String>,

    /// Show recent assemblies (limit to 10 most recent)
    #[arg(long)]
    pub recent: bool,

    /// Columns to display
    #[arg(long, value_delimiter = ',', default_values_t = vec![ListColumn::Id, ListColumn::PartNumber, ListColumn::Title, ListColumn::Status])]
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
}

#[derive(clap::Args, Debug)]
pub struct NewArgs {
    /// Part number (required)
    #[arg(long, short = 'p')]
    pub part_number: Option<String>,

    /// Title/description
    #[arg(long, short = 't')]
    pub title: Option<String>,

    /// Part revision
    #[arg(long)]
    pub revision: Option<String>,

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
    /// Assembly ID or short ID (ASM@N)
    pub id: String,
}

#[derive(clap::Args, Debug)]
pub struct EditArgs {
    /// Assembly ID or short ID (ASM@N)
    pub id: String,
}

#[derive(clap::Args, Debug)]
pub struct BomArgs {
    /// Assembly ID or short ID (ASM@N)
    pub id: String,

    /// Flatten nested assemblies (show all components)
    #[arg(long)]
    pub flat: bool,
}

/// Run an assembly subcommand
pub fn run(cmd: AsmCommands, global: &GlobalOpts) -> Result<()> {
    match cmd {
        AsmCommands::List(args) => run_list(args, global),
        AsmCommands::New(args) => run_new(args),
        AsmCommands::Show(args) => run_show(args, global),
        AsmCommands::Edit(args) => run_edit(args),
        AsmCommands::Bom(args) => run_bom(args, global),
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

        if path.extension().map_or(false, |e| e == "yaml") {
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
            StatusFilter::All => true,
        })
        .filter(|a| {
            if let Some(ref search) = args.search {
                let search_lower = search.to_lowercase();
                a.part_number.to_lowercase().contains(&search_lower)
                    || a.title.to_lowercase().contains(&search_lower)
                    || a.description
                        .as_ref()
                        .map_or(false, |d| d.to_lowercase().contains(&search_lower))
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

    // Sort
    let mut assemblies = assemblies;
    match args.sort {
        ListColumn::Id => assemblies.sort_by(|a, b| a.id.to_string().cmp(&b.id.to_string())),
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
    let _ = short_ids.save(&project);

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
        OutputFormat::Csv => {
            println!("short_id,id,part_number,revision,title,bom_items,status");
            for asm in &assemblies {
                let short_id = short_ids.get_short_id(&asm.id.to_string()).unwrap_or_default();
                println!(
                    "{},{},{},{},{},{},{}",
                    short_id,
                    asm.id,
                    asm.part_number,
                    asm.revision.as_deref().unwrap_or(""),
                    escape_csv(&asm.title),
                    asm.bom.len(),
                    asm.status
                );
            }
        }
        OutputFormat::Tsv => {
            // Build dynamic header based on columns
            let mut header_parts = Vec::new();
            let mut header_widths = Vec::new();

            for col in &args.columns {
                match col {
                    ListColumn::Id => {
                        header_parts.push(style("ID").bold().to_string());
                        header_widths.push(17);
                    }
                    ListColumn::PartNumber => {
                        header_parts.push(style("PART #").bold().to_string());
                        header_widths.push(12);
                    }
                    ListColumn::Title => {
                        header_parts.push(style("TITLE").bold().to_string());
                        header_widths.push(30);
                    }
                    ListColumn::Status => {
                        header_parts.push(style("STATUS").bold().to_string());
                        header_widths.push(10);
                    }
                    ListColumn::Author => {
                        header_parts.push(style("AUTHOR").bold().to_string());
                        header_widths.push(15);
                    }
                    ListColumn::Created => {
                        header_parts.push(style("CREATED").bold().to_string());
                        header_widths.push(20);
                    }
                }
            }

            // Print header
            for (i, part) in header_parts.iter().enumerate() {
                if i > 0 {
                    print!(" ");
                }
                print!("{:<width$}", part, width = header_widths[i]);
            }
            println!();
            println!("{}", "-".repeat(header_widths.iter().sum::<usize>() + args.columns.len() - 1));

            // Print rows
            for asm in &assemblies {
                for (i, col) in args.columns.iter().enumerate() {
                    if i > 0 {
                        print!(" ");
                    }
                    let width = header_widths[i];
                    match col {
                        ListColumn::Id => {
                            print!("{:<width$}", format_short_id(&asm.id), width = width);
                        }
                        ListColumn::PartNumber => {
                            print!("{:<width$}", truncate_str(&asm.part_number, width - 2), width = width);
                        }
                        ListColumn::Title => {
                            print!("{:<width$}", truncate_str(&asm.title, width - 2), width = width);
                        }
                        ListColumn::Status => {
                            print!("{:<width$}", asm.status, width = width);
                        }
                        ListColumn::Author => {
                            print!("{:<width$}", truncate_str(&asm.author, width - 2), width = width);
                        }
                        ListColumn::Created => {
                            print!("{:<width$}", asm.created.format("%Y-%m-%d %H:%M"), width = width);
                        }
                    }
                }
                println!();
            }

            println!();
            println!(
                "{} assembly(s) found.",
                style(assemblies.len()).cyan()
            );
        }
        OutputFormat::Id => {
            for asm in &assemblies {
                println!("{}", asm.id);
            }
        }
        OutputFormat::Md => {
            println!("| Short | ID | Part # | Title | BOM Items | Status |");
            println!("|---|---|---|---|---|---|");
            for asm in &assemblies {
                let short_id = short_ids.get_short_id(&asm.id.to_string()).unwrap_or_default();
                println!(
                    "| {} | {} | {} | {} | {} | {} |",
                    short_id,
                    format_short_id(&asm.id),
                    asm.part_number,
                    asm.title,
                    asm.bom.len(),
                    asm.status
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
    } else {
        part_number = args
            .part_number
            .ok_or_else(|| miette::miette!("Part number is required (use --part-number or -p)"))?;
        title = args
            .title
            .ok_or_else(|| miette::miette!("Title is required (use --title or -t)"))?;
    }

    // Generate ID
    let id = EntityId::new(EntityPrefix::Asm);

    // Generate template
    let generator = TemplateGenerator::new().map_err(|e| miette::miette!("{}", e))?;
    let ctx = TemplateContext::new(id.clone(), config.author())
        .with_title(&title)
        .with_part_number(&part_number);

    let ctx = if let Some(ref rev) = args.revision {
        ctx.with_part_revision(rev)
    } else {
        ctx
    };

    let yaml_content = generator
        .generate_assembly(&ctx)
        .map_err(|e| miette::miette!("{}", e))?;

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
    let _ = short_ids.save(&project);

    println!(
        "{} Created assembly {}",
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

    // Find the assembly file
    let asm_dir = project.root().join("bom/assemblies");
    let mut found_path = None;

    if asm_dir.exists() {
        for entry in fs::read_dir(&asm_dir).into_diagnostic()? {
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

    let path = found_path.ok_or_else(|| miette::miette!("No assembly found matching '{}'", args.id))?;

    // Read and display
    let content = fs::read_to_string(&path).into_diagnostic()?;

    match global.format {
        OutputFormat::Yaml | OutputFormat::Auto => {
            print!("{}", content);
        }
        OutputFormat::Json => {
            let asm: Assembly = serde_yml::from_str(&content).into_diagnostic()?;
            let json = serde_json::to_string_pretty(&asm).into_diagnostic()?;
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

    // Find the assembly file
    let asm_dir = project.root().join("bom/assemblies");
    let mut found_path = None;

    if asm_dir.exists() {
        for entry in fs::read_dir(&asm_dir).into_diagnostic()? {
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

    let path = found_path.ok_or_else(|| miette::miette!("No assembly found matching '{}'", args.id))?;

    println!("Opening {} in {}...", style(path.display()).cyan(), style(config.editor()).yellow());

    config.run_editor(&path).into_diagnostic()?;

    Ok(())
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

            if path.extension().map_or(false, |e| e == "yaml") {
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

    let assembly = found_asm.ok_or_else(|| miette::miette!("No assembly found matching '{}'", args.id))?;

    // Load component index for resolving names
    let cmp_dir = project.root().join("bom/components");
    let mut components: std::collections::HashMap<String, Component> = std::collections::HashMap::new();

    if cmp_dir.exists() {
        for entry in fs::read_dir(&cmp_dir).into_diagnostic()? {
            let entry = entry.into_diagnostic()?;
            let path = entry.path();

            if path.extension().map_or(false, |e| e == "yaml") {
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
