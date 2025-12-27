//! `tdt work` command - Work instruction management

use clap::{Subcommand, ValueEnum};
use console::style;
use miette::{IntoDiagnostic, Result};
use std::fs;

use crate::cli::commands::utils::format_link_with_title;
use crate::cli::filters::StatusFilter;
use crate::cli::table::{CellValue, ColumnDef, TableConfig, TableFormatter, TableRow};
use crate::cli::{GlobalOpts, OutputFormat};
use crate::core::cache::EntityCache;
use crate::core::identity::{EntityId, EntityPrefix};
use crate::core::project::Project;
use crate::core::shortid::ShortIdIndex;
use crate::core::Config;
use crate::entities::work_instruction::WorkInstruction;
use crate::schema::template::{TemplateContext, TemplateGenerator};
use crate::schema::wizard::SchemaWizard;

#[derive(Subcommand, Debug)]
pub enum WorkCommands {
    /// List work instructions with filtering
    List(ListArgs),

    /// Create a new work instruction
    New(NewArgs),

    /// Show a work instruction's details
    Show(ShowArgs),

    /// Edit a work instruction in your editor
    Edit(EditArgs),

    /// Delete a work instruction
    Delete(DeleteArgs),

    /// Archive a work instruction (soft delete)
    Archive(ArchiveArgs),
}

/// Column to display in list output
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ListColumn {
    Id,
    Title,
    DocNumber,
    Status,
    Author,
    Created,
}

impl std::fmt::Display for ListColumn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ListColumn::Id => write!(f, "id"),
            ListColumn::Title => write!(f, "title"),
            ListColumn::DocNumber => write!(f, "doc-number"),
            ListColumn::Status => write!(f, "status"),
            ListColumn::Author => write!(f, "author"),
            ListColumn::Created => write!(f, "created"),
        }
    }
}

/// Column definitions for work instruction list output
const WORK_COLUMNS: &[ColumnDef] = &[
    ColumnDef::new("id", "ID", 17),
    ColumnDef::new("doc-number", "DOC #", 14),
    ColumnDef::new("title", "TITLE", 30),
    ColumnDef::new("status", "STATUS", 10),
    ColumnDef::new("author", "AUTHOR", 20),
    ColumnDef::new("created", "CREATED", 20),
];

#[derive(clap::Args, Debug)]
pub struct ListArgs {
    /// Filter by status
    #[arg(long, short = 's', default_value = "all")]
    pub status: StatusFilter,

    /// Filter by process ID
    #[arg(long, short = 'p')]
    pub process: Option<String>,

    /// Filter by author
    #[arg(long, short = 'a')]
    pub author: Option<String>,

    /// Show only recent items (last 10)
    #[arg(long)]
    pub recent: bool,

    /// Search in title and description
    #[arg(long)]
    pub search: Option<String>,

    /// Columns to display
    #[arg(long, short = 'c', value_delimiter = ',', default_values_t = vec![
        ListColumn::DocNumber,
        ListColumn::Title,
        ListColumn::Status,
    ])]
    pub columns: Vec<ListColumn>,

    /// Sort by field
    #[arg(long, default_value = "title")]
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
    /// Work instruction title (required)
    #[arg(long, short = 't')]
    pub title: Option<String>,

    /// Document number (e.g., "WI-MACH-015")
    #[arg(long, short = 'd')]
    pub doc_number: Option<String>,

    /// Parent process ID
    #[arg(long, short = 'p')]
    pub process: Option<String>,

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
    /// Work instruction ID or short ID (WORK@N)
    pub id: String,
}

#[derive(clap::Args, Debug)]
pub struct EditArgs {
    /// Work instruction ID or short ID (WORK@N)
    pub id: String,
}

#[derive(clap::Args, Debug)]
pub struct DeleteArgs {
    /// Work instruction ID or short ID (WORK@N)
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
    /// Work instruction ID or short ID (WORK@N)
    pub id: String,

    /// Force archive even if other entities reference this one
    #[arg(long)]
    pub force: bool,

    /// Suppress output
    #[arg(long, short = 'q')]
    pub quiet: bool,
}

/// Directories where work instructions are stored
const WORK_INSTRUCTION_DIRS: &[&str] = &["manufacturing/work_instructions"];

/// Entity configuration for work instructions
const ENTITY_CONFIG: crate::cli::EntityConfig = crate::cli::EntityConfig {
    prefix: EntityPrefix::Work,
    dirs: WORK_INSTRUCTION_DIRS,
    name: "work instruction",
    name_plural: "work instructions",
};

/// Run a work instruction subcommand
pub fn run(cmd: WorkCommands, global: &GlobalOpts) -> Result<()> {
    match cmd {
        WorkCommands::List(args) => run_list(args, global),
        WorkCommands::New(args) => run_new(args, global),
        WorkCommands::Show(args) => run_show(args, global),
        WorkCommands::Edit(args) => run_edit(args),
        WorkCommands::Delete(args) => run_delete(args),
        WorkCommands::Archive(args) => run_archive(args),
    }
}

fn run_list(args: ListArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let short_ids = ShortIdIndex::load(&project);

    // Determine output format
    let format = match global.format {
        OutputFormat::Auto => OutputFormat::Tsv,
        f => f,
    };

    // Check if we can use the fast cache path:
    // - No process filter (link-based)
    // - No recent filter
    // - No search filter
    // - Not JSON/YAML output
    let can_use_cache = args.process.is_none()
        && !args.recent
        && args.search.is_none()
        && !matches!(format, OutputFormat::Json | OutputFormat::Yaml);

    if can_use_cache {
        if let Ok(cache) = EntityCache::open(&project) {
            let filter = crate::core::cache::EntityFilter {
                prefix: Some(EntityPrefix::Work),
                status: crate::cli::entity_cmd::status_filter_to_status(args.status),
                author: args.author.clone(),
                search: None,
                limit: None,
                priority: None,
                entity_type: None,
                category: None,
            };

            let mut entities = cache.list_entities(&filter);

            // Sort
            match args.sort {
                ListColumn::Id => entities.sort_by(|a, b| a.id.cmp(&b.id)),
                ListColumn::Title => entities.sort_by(|a, b| a.title.cmp(&b.title)),
                ListColumn::DocNumber => entities.sort_by(|a, b| a.id.cmp(&b.id)), // Not in cache
                ListColumn::Status => entities.sort_by(|a, b| a.status.cmp(&b.status)),
                ListColumn::Author => entities.sort_by(|a, b| a.author.cmp(&b.author)),
                ListColumn::Created => entities.sort_by(|a, b| a.created.cmp(&b.created)),
            }

            if args.reverse {
                entities.reverse();
            }

            if let Some(limit) = args.limit {
                entities.truncate(limit);
            }

            return output_cached_work_instructions(&entities, &short_ids, &args, format);
        }
    }

    // Fall back to full YAML loading
    let work_dir = project.root().join("manufacturing/work_instructions");

    if !work_dir.exists() {
        if args.count {
            println!("0");
        } else {
            println!("No work instructions found.");
        }
        return Ok(());
    }

    // Load and parse all work instructions
    let mut work_instructions: Vec<WorkInstruction> = Vec::new();

    for entry in fs::read_dir(&work_dir).into_diagnostic()? {
        let entry = entry.into_diagnostic()?;
        let path = entry.path();

        if path.extension().is_some_and(|e| e == "yaml") {
            let content = fs::read_to_string(&path).into_diagnostic()?;
            if let Ok(work) = serde_yml::from_str::<WorkInstruction>(&content) {
                work_instructions.push(work);
            }
        }
    }

    // Resolve process filter if provided
    let process_filter = args.process.as_ref().map(|proc_id| {
        short_ids
            .resolve(proc_id)
            .unwrap_or_else(|| proc_id.clone())
    });

    // Apply filters
    let work_instructions: Vec<WorkInstruction> = work_instructions
        .into_iter()
        .filter(|w| crate::cli::entity_cmd::status_enum_matches_filter(&w.status, args.status))
        .filter(|w| {
            if let Some(ref proc_id) = process_filter {
                w.links
                    .process
                    .as_ref()
                    .is_some_and(|p| p.to_string().contains(proc_id))
            } else {
                true
            }
        })
        .filter(|w| {
            if let Some(ref author) = args.author {
                w.author.to_lowercase().contains(&author.to_lowercase())
            } else {
                true
            }
        })
        .filter(|w| {
            if let Some(ref search) = args.search {
                let search_lower = search.to_lowercase();
                w.title.to_lowercase().contains(&search_lower)
                    || w.description
                        .as_ref()
                        .is_some_and(|d| d.to_lowercase().contains(&search_lower))
                    || w.document_number
                        .as_ref()
                        .is_some_and(|d| d.to_lowercase().contains(&search_lower))
            } else {
                true
            }
        })
        .collect();

    // Sort
    let mut work_instructions = work_instructions;
    match args.sort {
        ListColumn::Id => work_instructions.sort_by(|a, b| a.id.to_string().cmp(&b.id.to_string())),
        ListColumn::Title => work_instructions.sort_by(|a, b| a.title.cmp(&b.title)),
        ListColumn::DocNumber => work_instructions.sort_by(|a, b| {
            a.document_number
                .as_deref()
                .unwrap_or("")
                .cmp(b.document_number.as_deref().unwrap_or(""))
        }),
        ListColumn::Status => work_instructions
            .sort_by(|a, b| format!("{:?}", a.status).cmp(&format!("{:?}", b.status))),
        ListColumn::Author => work_instructions.sort_by(|a, b| a.author.cmp(&b.author)),
        ListColumn::Created => work_instructions.sort_by(|a, b| a.created.cmp(&b.created)),
    }

    if args.reverse {
        work_instructions.reverse();
    }

    // Apply recent filter (last 10 by creation date)
    if args.recent {
        work_instructions.sort_by(|a, b| b.created.cmp(&a.created));
        work_instructions.truncate(10);
    }

    // Apply limit
    if let Some(limit) = args.limit {
        work_instructions.truncate(limit);
    }

    // Count only
    if args.count {
        println!("{}", work_instructions.len());
        return Ok(());
    }

    // No results
    if work_instructions.is_empty() {
        println!("No work instructions found.");
        return Ok(());
    }

    // Update short ID index
    let mut short_ids = short_ids;
    short_ids.ensure_all(work_instructions.iter().map(|w| w.id.to_string()));
    super::utils::save_short_ids(&mut short_ids, &project);

    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&work_instructions).into_diagnostic()?;
            println!("{}", json);
        }
        OutputFormat::Yaml => {
            let yaml = serde_yml::to_string(&work_instructions).into_diagnostic()?;
            print!("{}", yaml);
        }
        OutputFormat::Tsv
        | OutputFormat::Csv
        | OutputFormat::Md
        | OutputFormat::Id
        | OutputFormat::ShortId => {
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
            let rows: Vec<TableRow> = work_instructions
                .iter()
                .map(|work| work_instruction_to_row(work, &short_ids))
                .collect();

            // Configure table
            let config = if let Some(width) = args.wrap {
                TableConfig::with_wrap(width)
            } else {
                TableConfig::default()
            };

            let formatter =
                TableFormatter::new(WORK_COLUMNS, "work instruction", "WORK").with_config(config);
            formatter.output(rows, format, &visible);
        }
        OutputFormat::Auto | OutputFormat::Path => unreachable!(),
    }

    Ok(())
}

/// Convert a WorkInstruction to a TableRow
fn work_instruction_to_row(work: &WorkInstruction, short_ids: &ShortIdIndex) -> TableRow {
    TableRow::new(work.id.to_string(), short_ids)
        .cell("id", CellValue::Id(work.id.to_string()))
        .cell(
            "doc-number",
            CellValue::Text(
                work.document_number
                    .clone()
                    .unwrap_or_else(|| "-".to_string()),
            ),
        )
        .cell("title", CellValue::Text(work.title.clone()))
        .cell("status", CellValue::Status(work.status))
        .cell("author", CellValue::Text(work.author.clone()))
        .cell("created", CellValue::DateTime(work.created))
}

/// Output cached work instructions (fast path - no YAML parsing needed)
fn output_cached_work_instructions(
    entities: &[crate::core::CachedEntity],
    short_ids: &ShortIdIndex,
    args: &ListArgs,
    format: OutputFormat,
) -> Result<()> {
    if entities.is_empty() {
        println!("No work instructions found.");
        return Ok(());
    }

    if args.count {
        println!("{}", entities.len());
        return Ok(());
    }

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
    let rows: Vec<TableRow> = entities
        .iter()
        .map(|entity| cached_entity_to_row(entity, short_ids))
        .collect();

    // Configure table
    let config = if let Some(width) = args.wrap {
        TableConfig::with_wrap(width)
    } else {
        TableConfig::default()
    };

    let formatter =
        TableFormatter::new(WORK_COLUMNS, "work instruction", "WORK").with_config(config);
    formatter.output(rows, format, &visible);

    Ok(())
}

/// Convert a CachedEntity to a TableRow for work instructions
fn cached_entity_to_row(entity: &crate::core::CachedEntity, short_ids: &ShortIdIndex) -> TableRow {
    TableRow::new(entity.id.clone(), short_ids)
        .cell("id", CellValue::Id(entity.id.clone()))
        .cell("doc-number", CellValue::Text("-".to_string())) // Not in cache
        .cell("title", CellValue::Text(entity.title.clone()))
        .cell("status", CellValue::Status(entity.status))
        .cell("author", CellValue::Text(entity.author.clone()))
        .cell("created", CellValue::DateTime(entity.created))
}

fn run_new(args: NewArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let config = Config::load();

    let title: String;
    let document_number: Option<String>;
    let mut description: Option<String> = None;
    let mut revision: Option<String> = None;
    let mut estimated_duration: Option<f64> = None;

    if args.interactive {
        let wizard = SchemaWizard::new();
        let result = wizard.run(EntityPrefix::Work)?;

        title = result
            .get_string("title")
            .map(String::from)
            .unwrap_or_else(|| "New Work Instruction".to_string());
        description = result.get_string("description").map(String::from);
        document_number = result.get_string("document_number").map(String::from);
        revision = result.get_string("revision").map(String::from);
        estimated_duration = result.get_f64("estimated_duration_minutes");
    } else {
        title = args
            .title
            .unwrap_or_else(|| "New Work Instruction".to_string());
        document_number = args.doc_number.clone();
    }

    // Generate ID
    let id = EntityId::new(EntityPrefix::Work);

    // Resolve linked IDs if provided
    let short_ids = ShortIdIndex::load(&project);
    let process_id = args
        .process
        .as_ref()
        .map(|p| short_ids.resolve(p).unwrap_or_else(|| p.clone()));

    // Generate template
    let generator = TemplateGenerator::new().map_err(|e| miette::miette!("{}", e))?;
    let mut ctx = TemplateContext::new(id.clone(), config.author()).with_title(&title);

    if let Some(ref doc_num) = document_number {
        ctx = ctx.with_document_number(doc_num);
    }
    if let Some(ref proc_id) = process_id {
        ctx = ctx.with_process_id(proc_id);
    }

    let mut yaml_content = generator
        .generate_work_instruction(&ctx)
        .map_err(|e| miette::miette!("{}", e))?;

    // Apply wizard-collected values via string replacement
    if args.interactive {
        if let Some(ref desc) = description {
            if !desc.is_empty() {
                let indented = desc
                    .lines()
                    .map(|line| format!("  {}", line))
                    .collect::<Vec<_>>()
                    .join("\n");
                yaml_content = yaml_content.replace(
                    "description: |\n  # Purpose and scope of this work instruction",
                    &format!("description: |\n{}", indented),
                );
            }
        }
        if let Some(ref rev) = revision {
            yaml_content =
                yaml_content.replace("revision: \"A\"", &format!("revision: \"{}\"", rev));
        }
        if let Some(dur) = estimated_duration {
            yaml_content = yaml_content.replace(
                "estimated_duration_minutes: null",
                &format!("estimated_duration_minutes: {}", dur),
            );
        }
    }

    // Write file
    let output_dir = project.root().join("manufacturing/work_instructions");
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
        EntityPrefix::Work,
        &args.link,
        &short_ids,
    );

    // Output based on format flag
    let extra_info = if let Some(ref doc_num) = args.doc_number {
        format!(
            "{}\n   Doc: {}",
            style(&title).white(),
            style(doc_num).yellow()
        )
    } else {
        format!("{}", style(&title).white())
    };
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

    // Find the work instruction file
    let work_dir = project.root().join("manufacturing/work_instructions");
    let mut found_path = None;

    if work_dir.exists() {
        for entry in fs::read_dir(&work_dir).into_diagnostic()? {
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

    let path = found_path
        .ok_or_else(|| miette::miette!("No work instruction found matching '{}'", args.id))?;

    // Read and parse work instruction
    let content = fs::read_to_string(&path).into_diagnostic()?;
    let work: WorkInstruction = serde_yml::from_str(&content).into_diagnostic()?;

    match global.format {
        OutputFormat::Yaml => {
            print!("{}", content);
        }
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&work).into_diagnostic()?;
            println!("{}", json);
        }
        OutputFormat::Id | OutputFormat::ShortId => {
            if global.format == OutputFormat::ShortId {
                let sid_index = ShortIdIndex::load(&project);
                let short_id = sid_index
                    .get_short_id(&work.id.to_string())
                    .unwrap_or_default();
                println!("{}", short_id);
            } else {
                println!("{}", work.id);
            }
        }
        _ => {
            // Load cache for title lookups
            let cache = EntityCache::open(&project).ok();

            // Pretty format (default)
            println!("{}", style("─".repeat(60)).dim());
            println!(
                "{}: {}",
                style("ID").bold(),
                style(&work.id.to_string()).cyan()
            );
            println!("{}: {}", style("Title").bold(), style(&work.title).yellow());
            if let Some(ref doc) = work.document_number {
                if !doc.is_empty() {
                    println!("{}: {}", style("Document #").bold(), doc);
                }
            }
            if let Some(ref proc_id) = work.links.process {
                let proc_display = format_link_with_title(&proc_id.to_string(), &short_ids, &cache);
                println!(
                    "{}: {}",
                    style("Process").bold(),
                    style(&proc_display).cyan()
                );
            }
            println!("{}: {}", style("Status").bold(), work.status);
            println!("{}", style("─".repeat(60)).dim());

            // Procedure Steps
            if !work.procedure.is_empty() {
                println!();
                println!(
                    "{} ({}):",
                    style("Procedure Steps").bold(),
                    work.procedure.len()
                );
                for step in &work.procedure {
                    print!("  {}. {}", step.step, step.action);
                    if let Some(ref caution) = step.caution {
                        print!(" ⚠ {}", caution);
                    }
                    println!();
                }
            }

            // Tools Required
            if !work.tools_required.is_empty() {
                println!();
                println!(
                    "{} ({}):",
                    style("Tools Required").bold(),
                    work.tools_required.len()
                );
                for tool in &work.tools_required {
                    println!("  • {}", tool.name);
                }
            }

            // Materials Required
            if !work.materials_required.is_empty() {
                println!();
                println!(
                    "{} ({}):",
                    style("Materials Required").bold(),
                    work.materials_required.len()
                );
                for mat in &work.materials_required {
                    println!("  • {}", mat.name);
                }
            }

            // Tags
            if !work.tags.is_empty() {
                println!();
                println!("{}: {}", style("Tags").bold(), work.tags.join(", "));
            }

            // Description
            if let Some(ref desc) = work.description {
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
                work.author,
                style("Created").dim(),
                work.created.format("%Y-%m-%d %H:%M"),
                style("Revision").dim(),
                work.entity_revision
            );
        }
    }

    Ok(())
}

fn run_edit(args: EditArgs) -> Result<()> {
    crate::cli::entity_cmd::run_edit_generic(&args.id, &ENTITY_CONFIG)
}

fn run_delete(args: DeleteArgs) -> Result<()> {
    crate::cli::commands::utils::run_delete(
        &args.id,
        WORK_INSTRUCTION_DIRS,
        args.force,
        false,
        args.quiet,
    )
}

fn run_archive(args: ArchiveArgs) -> Result<()> {
    crate::cli::commands::utils::run_delete(
        &args.id,
        WORK_INSTRUCTION_DIRS,
        args.force,
        true,
        args.quiet,
    )
}
