//! `tdt work` command - Work instruction management

use clap::{Subcommand, ValueEnum};
use console::style;
use miette::{IntoDiagnostic, Result};

use crate::cli::commands::utils::format_link_with_title;
use crate::cli::filters::StatusFilter;
use crate::cli::table::{CellValue, ColumnDef, TableRow};
use crate::cli::{GlobalOpts, OutputFormat};
use tdt_core::core::cache::EntityCache;
use tdt_core::core::identity::EntityPrefix;
use tdt_core::core::project::Project;
use tdt_core::core::shortid::ShortIdIndex;
use tdt_core::core::CachedEntity;
use tdt_core::core::Config;
use tdt_core::entities::work_instruction::WorkInstruction;
use tdt_core::schema::wizard::SchemaWizard;
use tdt_core::services::{
    CommonFilter, CreateWorkInstruction, SortDirection, WorkInstructionFilter,
    WorkInstructionService, WorkInstructionSortField,
};

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

    /// Manage procedure steps
    #[command(subcommand)]
    Step(StepCommands),
}

#[derive(Subcommand, Debug)]
pub enum StepCommands {
    /// Add a procedure step to a work instruction
    Add(StepAddArgs),

    /// Remove a procedure step from a work instruction
    Rm(StepRmArgs),

    /// List procedure steps for a work instruction
    List(StepListArgs),
}

#[derive(clap::Args, Debug)]
pub struct StepAddArgs {
    /// Work instruction ID (WORK@N or WORK-xxx)
    pub id: String,

    /// Action description for this step
    #[arg(long, short = 'a')]
    pub action: String,

    /// Step number (defaults to next available)
    #[arg(long, short = 's')]
    pub step: Option<u32>,

    /// Verification/check point description
    #[arg(long)]
    pub verification: Option<String>,

    /// Caution/warning note
    #[arg(long)]
    pub caution: Option<String>,

    /// Estimated time in minutes
    #[arg(long, short = 't')]
    pub time: Option<f64>,

    /// Mark step as requiring quality approval
    #[arg(long)]
    pub require_approval: bool,

    /// Mark step as a quality hold point
    #[arg(long)]
    pub hold_point: bool,

    /// Data field to collect (key:label format, can be repeated)
    #[arg(long, short = 'd', value_parser = parse_data_field)]
    pub data_field: Vec<(String, String)>,

    /// Equipment required (name, can be repeated)
    #[arg(long, short = 'e')]
    pub equipment: Vec<String>,
}

#[derive(clap::Args, Debug)]
pub struct StepRmArgs {
    /// Work instruction ID (WORK@N or WORK-xxx)
    pub id: String,

    /// Step number to remove
    #[arg(long, short = 's')]
    pub step: u32,
}

#[derive(clap::Args, Debug)]
pub struct StepListArgs {
    /// Work instruction ID (WORK@N or WORK-xxx)
    pub id: String,
}

fn parse_data_field(s: &str) -> std::result::Result<(String, String), String> {
    let parts: Vec<&str> = s.splitn(2, ':').collect();
    if parts.len() != 2 {
        return Err(
            "Data field must be in key:label format (e.g., diameter:\"Hole Diameter (mm)\")"
                .to_string(),
        );
    }
    Ok((parts[0].to_string(), parts[1].to_string()))
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

const WORK_LIST_SPEC: crate::cli::entity_cmd::CachedListSpec =
    crate::cli::entity_cmd::CachedListSpec {
        columns: WORK_COLUMNS,
        entity_name: "work instruction",
        entity_prefix: "WORK",
        empty_message: "No work instructions found.",
    };

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

    /// Only show entities linked to these IDs (use - for stdin pipe)
    #[arg(long, value_delimiter = ',')]
    pub linked_to: Vec<String>,

    /// Filter by link type when using --linked-to (e.g., verified_by, satisfied_by)
    #[arg(long, requires = "linked_to")]
    pub via: Option<String>,
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

/// Search directories for this entity type — single source of truth
/// lives in `Project::entity_search_directories`.
const WORK_INSTRUCTION_DIRS: &[&str] = tdt_core::core::project::Project::entity_search_directories(
    tdt_core::core::identity::EntityPrefix::Work,
);

/// Entity configuration for work instructions
const ENTITY_CONFIG: crate::cli::EntityConfig = crate::cli::EntityConfig {
    prefix: EntityPrefix::Work,
    dirs: WORK_INSTRUCTION_DIRS,
    name: "work instruction",
    name_plural: "work instructions",
};

/// Build a WorkInstructionFilter from CLI list arguments
fn build_work_filter(args: &ListArgs, short_ids: &ShortIdIndex) -> WorkInstructionFilter {
    // Resolve process ID if provided
    let process = args
        .process
        .as_ref()
        .map(|p| short_ids.resolve(p).unwrap_or_else(|| p.clone()));

    WorkInstructionFilter {
        common: CommonFilter {
            status: crate::cli::entity_cmd::status_filter_to_status(args.status).map(|s| vec![s]),
            author: args.author.clone(),
            search: args.search.clone(),
            limit: if args.recent { Some(10) } else { args.limit },
            ..Default::default()
        },
        process,
        ..Default::default()
    }
}

/// Build sort field and direction from CLI arguments
fn build_work_sort(args: &ListArgs) -> (WorkInstructionSortField, SortDirection) {
    let field = match args.sort {
        ListColumn::Id => WorkInstructionSortField::Id,
        ListColumn::Title => WorkInstructionSortField::Title,
        ListColumn::DocNumber => WorkInstructionSortField::DocumentNumber,
        ListColumn::Status => WorkInstructionSortField::Status,
        ListColumn::Author => WorkInstructionSortField::Author,
        ListColumn::Created => WorkInstructionSortField::Created,
    };

    let direction = if args.reverse || args.recent {
        // Recent sorts newest first (descending)
        SortDirection::Ascending
    } else {
        SortDirection::Descending
    };

    // If recent, always sort by created
    let field = if args.recent {
        WorkInstructionSortField::Created
    } else {
        field
    };

    (field, direction)
}

/// Sort cached work instructions by the specified column
fn sort_cached_work_instructions(entities: &mut [CachedEntity], sort: ListColumn, reverse: bool) {
    entities.sort_by(|a, b| {
        let cmp = match sort {
            ListColumn::Id => a.id.cmp(&b.id),
            ListColumn::Title => a.title.cmp(&b.title),
            ListColumn::DocNumber => a.id.cmp(&b.id), // Not in cache
            ListColumn::Status => a.status.cmp(&b.status),
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

/// Run a work instruction subcommand
pub fn run(cmd: WorkCommands, global: &GlobalOpts) -> Result<()> {
    match cmd {
        WorkCommands::List(args) => run_list(args, global),
        WorkCommands::New(args) => run_new(args, global),
        WorkCommands::Show(args) => run_show(args, global),
        WorkCommands::Edit(args) => run_edit(args),
        WorkCommands::Delete(args) => run_delete(args),
        WorkCommands::Archive(args) => run_archive(args),
        WorkCommands::Step(cmd) => match cmd {
            StepCommands::Add(args) => run_step_add(args),
            StepCommands::Rm(args) => run_step_rm(args),
            StepCommands::List(args) => run_step_list(args),
        },
    }
}

fn run_list(args: ListArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let cache = EntityCache::open(&project).map_err(|e| miette::miette!("{}", e))?;
    let service = WorkInstructionService::new(&project, &cache);
    let mut short_ids = ShortIdIndex::load(&project);

    // Resolve linked-to filter via cache
    let allowed_ids = crate::cli::helpers::resolve_linked_to(
        &args.linked_to,
        args.via.as_deref(),
        &short_ids,
        &cache,
    );

    // Determine output format
    let format = global.list_format();

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
        // Fast path: use cached entities
        let mut entities = service.list_cached();

        // Apply status filter manually on cached entities
        if let Some(status) = crate::cli::entity_cmd::status_filter_to_status(args.status) {
            entities.retain(|e| e.status == status);
        }

        // Apply author filter
        if let Some(ref author_filter) = args.author {
            let author_lower = author_filter.to_lowercase();
            entities.retain(|e| e.author.to_lowercase().contains(&author_lower));
        }

        // Apply linked-to filter
        if let Some(ref ids) = allowed_ids {
            entities.retain(|e| ids.contains(&e.id));
        }

        // Sort
        sort_cached_work_instructions(&mut entities, args.sort, args.reverse);

        // Apply limit
        if let Some(limit) = args.limit {
            entities.truncate(limit);
        }

        let mut visible: Vec<String> = args.columns.iter().map(|c| c.to_string()).collect();
        if args.show_id && !visible.iter().any(|c| c == "id") {
            visible.insert(0, "id".to_string());
        }
        return crate::cli::entity_cmd::output_cached_list(
            &entities,
            &WORK_LIST_SPEC,
            &visible,
            args.wrap,
            args.count,
            format,
            &project,
            |e| cached_entity_to_row(e, &short_ids),
            |e| e.file_path.as_path(),
        );
    }

    // Full entity loading via service
    let filter = build_work_filter(&args, &short_ids);
    let (sort_field, sort_dir) = build_work_sort(&args);

    let result = service
        .list(&filter, sort_field, sort_dir)
        .map_err(|e| miette::miette!("{}", e))?;
    let mut instructions = result.items;

    // Apply linked-to filter
    if let Some(ref ids) = allowed_ids {
        instructions.retain(|e| ids.contains(&e.id.to_string()));
    }

    // Count only
    if args.count {
        println!("{}", instructions.len());
        return Ok(());
    }

    // No results
    if instructions.is_empty() {
        println!("No work instructions found.");
        return Ok(());
    }

    {
        let mut visible: Vec<String> = args.columns.iter().map(|c| c.to_string()).collect();
        if args.show_id && !visible.iter().any(|c| c == "id") {
            visible.insert(0, "id".to_string());
        }
        crate::cli::entity_cmd::output_entity_list(
            &instructions,
            &WORK_LIST_SPEC,
            &visible,
            args.wrap,
            args.count,
            format,
            &project,
            &mut short_ids,
            |x| x.id.to_string(),
            work_instruction_to_row,
        )
    }
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

/// Convert a CachedEntity to a TableRow for work instructions
fn cached_entity_to_row(
    entity: &tdt_core::core::CachedEntity,
    short_ids: &ShortIdIndex,
) -> TableRow {
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
    let cache = EntityCache::open(&project).map_err(|e| miette::miette!("{}", e))?;
    let service = WorkInstructionService::new(&project, &cache);
    let config = Config::load();

    let title: String;
    let document_number: Option<String>;
    let description: Option<String>;
    let revision: Option<String>;
    let estimated_duration: Option<f64>;

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
        description = None;
        revision = None;
        estimated_duration = None;
    }

    // Resolve linked IDs if provided
    let short_ids = ShortIdIndex::load(&project);
    let process_id = args
        .process
        .as_ref()
        .map(|p| short_ids.resolve(p).unwrap_or_else(|| p.clone()));

    // Create work instruction using service
    let input = CreateWorkInstruction {
        title: title.clone(),
        author: config.author(),
        document_number,
        revision,
        description,
        process: process_id,
        estimated_duration_minutes: estimated_duration,
        ..Default::default()
    };

    let work = service
        .create(input)
        .map_err(|e| miette::miette!("{}", e))?;

    let file_path = project
        .root()
        .join("manufacturing/work_instructions")
        .join(format!("{}.tdt.yaml", work.id));

    // Add to short ID index
    let mut short_ids = ShortIdIndex::load(&project);
    let short_id = short_ids.add(work.id.to_string());
    super::utils::save_short_ids(&mut short_ids, &project);

    // Handle --link flags
    let added_links = crate::cli::entity_cmd::process_link_flags(
        &file_path,
        EntityPrefix::Work,
        &args.link,
        &short_ids,
    );

    // Output based on format flag
    let extra_info = if let Some(ref doc_num) = work.document_number {
        format!(
            "{}\n   Doc: {}",
            style(&work.title).white(),
            style(doc_num).yellow()
        )
    } else {
        format!("{}", style(&work.title).white())
    };
    crate::cli::entity_cmd::output_new_entity(
        &work.id,
        &file_path,
        short_id.clone(),
        ENTITY_CONFIG.name,
        &work.title,
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
    let cache = EntityCache::open(&project).map_err(|e| miette::miette!("{}", e))?;

    // Resolve short ID if needed
    let short_ids = ShortIdIndex::load(&project);
    let resolved_id = short_ids
        .resolve(&args.id)
        .unwrap_or_else(|| args.id.clone());

    // Use WorkInstructionService to get the work instruction (cache-first lookup)
    let service = WorkInstructionService::new(&project, &cache);
    let work = service
        .get(&resolved_id)
        .map_err(|e| miette::miette!("{}", e))?
        .ok_or_else(|| miette::miette!("No work instruction found matching '{}'", args.id))?;

    match global.output {
        OutputFormat::Yaml => {
            let yaml = serde_yml::to_string(&work).into_diagnostic()?;
            print!("{}", yaml);
        }
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&work).into_diagnostic()?;
            println!("{}", json);
        }
        OutputFormat::Id
        | OutputFormat::ShortId
        | OutputFormat::Table
        | OutputFormat::Dot
        | OutputFormat::Tree => {
            if global.output == OutputFormat::ShortId {
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

fn run_step_add(args: StepAddArgs) -> Result<()> {
    use tdt_core::entities::work_instruction::{
        ProcedureStep, StepApprovalRequirement, StepDataField,
    };

    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let cache = EntityCache::open(&project).map_err(|e| miette::miette!("{}", e))?;
    let short_ids = ShortIdIndex::load(&project);
    let resolved_id = short_ids
        .resolve(&args.id)
        .unwrap_or_else(|| args.id.clone());

    let service = WorkInstructionService::new(&project, &cache);
    let mut wi = service
        .get(&resolved_id)
        .map_err(|e| miette::miette!("{}", e))?
        .ok_or_else(|| miette::miette!("No work instruction found matching '{}'", args.id))?;

    let display_id = short_ids
        .get_short_id(&wi.id.to_string())
        .unwrap_or_else(|| args.id.clone());

    // Determine step number
    let step_num = args
        .step
        .unwrap_or_else(|| wi.procedure.iter().map(|s| s.step).max().unwrap_or(0) + 1);

    // Check for duplicate step number
    if wi.procedure.iter().any(|s| s.step == step_num) {
        return Err(miette::miette!(
            "Step {} already exists in {}. Use a different step number or remove it first.",
            step_num,
            display_id
        ));
    }

    // Build the procedure step
    let mut proc_step = ProcedureStep::new(step_num, args.action.clone());

    if let Some(ref v) = args.verification {
        proc_step.verification = Some(v.clone());
    }
    if let Some(ref c) = args.caution {
        proc_step.caution = Some(c.clone());
    }
    if let Some(t) = args.time {
        proc_step.estimated_time_minutes = Some(t);
    }
    if args.require_approval || args.hold_point {
        proc_step.approval = Some(StepApprovalRequirement {
            requires_signoff: args.require_approval || args.hold_point,
            quality_hold_point: args.hold_point,
            ..Default::default()
        });
    }
    for (key, label) in &args.data_field {
        proc_step.data_fields.push(StepDataField {
            key: key.clone(),
            label: label.clone(),
            ..Default::default()
        });
    }
    proc_step.equipment = args.equipment.clone();

    // Add step and sort
    wi.procedure.push(proc_step);
    wi.procedure.sort_by_key(|s| s.step);
    wi.entity_revision += 1;

    // Save
    let file_path = project
        .root()
        .join("manufacturing/work_instructions")
        .join(format!("{}.tdt.yaml", wi.id));
    let yaml = serde_yml::to_string(&wi).into_diagnostic()?;
    std::fs::write(&file_path, yaml).into_diagnostic()?;

    println!(
        "{} Added step {} to {}",
        style("✓").green(),
        style(step_num).yellow(),
        style(&display_id).cyan()
    );
    println!("   Action: {}", args.action);
    if let Some(ref v) = args.verification {
        println!("   Verification: {}", v);
    }
    if args.hold_point {
        println!("   {} Quality hold point", style("⚠").yellow());
    } else if args.require_approval {
        println!("   {} Requires approval", style("⏳").yellow());
    }
    println!("   {} now has {} step(s)", display_id, wi.procedure.len());

    super::utils::sync_cache(&project);
    Ok(())
}

fn run_step_rm(args: StepRmArgs) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let cache = EntityCache::open(&project).map_err(|e| miette::miette!("{}", e))?;
    let short_ids = ShortIdIndex::load(&project);
    let resolved_id = short_ids
        .resolve(&args.id)
        .unwrap_or_else(|| args.id.clone());

    let service = WorkInstructionService::new(&project, &cache);
    let mut wi = service
        .get(&resolved_id)
        .map_err(|e| miette::miette!("{}", e))?
        .ok_or_else(|| miette::miette!("No work instruction found matching '{}'", args.id))?;

    let display_id = short_ids
        .get_short_id(&wi.id.to_string())
        .unwrap_or_else(|| args.id.clone());

    let before_len = wi.procedure.len();
    wi.procedure.retain(|s| s.step != args.step);

    if wi.procedure.len() == before_len {
        return Err(miette::miette!(
            "Step {} not found in {}",
            args.step,
            display_id
        ));
    }

    wi.entity_revision += 1;

    let file_path = project
        .root()
        .join("manufacturing/work_instructions")
        .join(format!("{}.tdt.yaml", wi.id));
    let yaml = serde_yml::to_string(&wi).into_diagnostic()?;
    std::fs::write(&file_path, yaml).into_diagnostic()?;

    println!(
        "{} Removed step {} from {}",
        style("✓").green(),
        style(args.step).yellow(),
        style(&display_id).cyan()
    );
    println!("   {} now has {} step(s)", display_id, wi.procedure.len());

    super::utils::sync_cache(&project);
    Ok(())
}

fn run_step_list(args: StepListArgs) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let cache = EntityCache::open(&project).map_err(|e| miette::miette!("{}", e))?;
    let short_ids = ShortIdIndex::load(&project);
    let resolved_id = short_ids
        .resolve(&args.id)
        .unwrap_or_else(|| args.id.clone());

    let service = WorkInstructionService::new(&project, &cache);
    let wi = service
        .get(&resolved_id)
        .map_err(|e| miette::miette!("{}", e))?
        .ok_or_else(|| miette::miette!("No work instruction found matching '{}'", args.id))?;

    let display_id = short_ids
        .get_short_id(&wi.id.to_string())
        .unwrap_or_else(|| args.id.clone());

    if wi.procedure.is_empty() {
        println!(
            "{} has no procedure steps defined.",
            style(&display_id).cyan()
        );
        println!(
            "Add steps with: tdt work step add {} --action \"...\"",
            display_id
        );
        return Ok(());
    }

    println!(
        "{} - {} ({} steps)",
        style(&display_id).cyan(),
        style(&wi.title).bold(),
        wi.procedure.len()
    );
    println!("{}", style("─".repeat(60)).dim());

    for step in &wi.procedure {
        let mut flags = Vec::new();
        if step.is_hold_point() {
            flags.push(style("HOLD").yellow().bold().to_string());
        } else if step.requires_approval() {
            flags.push(style("APPROVAL").yellow().to_string());
        }
        if !step.data_fields.is_empty() {
            flags.push(format!("{}x data", step.data_fields.len()));
        }
        if !step.equipment.is_empty() {
            flags.push(format!("{}x equip", step.equipment.len()));
        }

        let flag_str = if flags.is_empty() {
            String::new()
        } else {
            format!(" [{}]", flags.join(", "))
        };

        println!(
            "  {}. {}{}",
            style(step.step).cyan(),
            step.action,
            style(&flag_str).dim()
        );
        if let Some(ref v) = step.verification {
            println!("     {} Verify: {}", style("✓").dim(), v);
        }
        if let Some(ref c) = step.caution {
            println!("     {} Caution: {}", style("⚠").yellow(), c);
        }
        if let Some(t) = step.estimated_time_minutes {
            println!("     {} Est. {:.0} min", style("⏱").dim(), t);
        }
    }

    Ok(())
}
