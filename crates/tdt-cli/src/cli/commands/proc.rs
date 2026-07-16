//! `tdt proc` command - Manufacturing process management

use clap::{Subcommand, ValueEnum};
use console::style;
use miette::{IntoDiagnostic, Result};
use std::fs;

use crate::cli::filters::StatusFilter;
use crate::cli::helpers::truncate_str;
use crate::cli::table::{CellValue, ColumnDef, TableConfig, TableFormatter, TableRow};
use crate::cli::{GlobalOpts, OutputFormat};
use tdt_core::core::cache::{CachedEntity, EntityCache};
use tdt_core::core::identity::EntityPrefix;
use tdt_core::core::project::Project;
use tdt_core::core::shortid::ShortIdIndex;
use tdt_core::core::Config;
use tdt_core::entities::process::{Process, ProcessType, SkillLevel};
use tdt_core::schema::wizard::SchemaWizard;
use tdt_core::services::{
    CommonFilter, CreateProcess, ProcessFilter, ProcessService, ProcessSortField, SortDirection,
};

/// Column definitions for process list output
const PROC_COLUMNS: &[ColumnDef] = &[
    ColumnDef::new("id", "ID", 17),
    ColumnDef::new("title", "TITLE", 30),
    ColumnDef::new("process-type", "TYPE", 12),
    ColumnDef::new("operation", "OP #", 10),
    ColumnDef::new("status", "STATUS", 10),
    ColumnDef::new("author", "AUTHOR", 16),
    ColumnDef::new("created", "CREATED", 20),
];

const PROC_LIST_SPEC: crate::cli::entity_cmd::CachedListSpec =
    crate::cli::entity_cmd::CachedListSpec {
        columns: PROC_COLUMNS,
        entity_name: "process",
        entity_prefix: "PROC",
        empty_message: "No processes found.",
    };

/// CLI-friendly process type enum
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CliProcessType {
    Machining,
    Assembly,
    Inspection,
    Test,
    Finishing,
    Packaging,
    Handling,
    #[value(name = "heat_treat")]
    HeatTreat,
    Welding,
    Coating,
}

impl std::fmt::Display for CliProcessType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CliProcessType::Machining => write!(f, "machining"),
            CliProcessType::Assembly => write!(f, "assembly"),
            CliProcessType::Inspection => write!(f, "inspection"),
            CliProcessType::Test => write!(f, "test"),
            CliProcessType::Finishing => write!(f, "finishing"),
            CliProcessType::Packaging => write!(f, "packaging"),
            CliProcessType::Handling => write!(f, "handling"),
            CliProcessType::HeatTreat => write!(f, "heat_treat"),
            CliProcessType::Welding => write!(f, "welding"),
            CliProcessType::Coating => write!(f, "coating"),
        }
    }
}

impl From<CliProcessType> for ProcessType {
    fn from(cli: CliProcessType) -> Self {
        match cli {
            CliProcessType::Machining => ProcessType::Machining,
            CliProcessType::Assembly => ProcessType::Assembly,
            CliProcessType::Inspection => ProcessType::Inspection,
            CliProcessType::Test => ProcessType::Test,
            CliProcessType::Finishing => ProcessType::Finishing,
            CliProcessType::Packaging => ProcessType::Packaging,
            CliProcessType::Handling => ProcessType::Handling,
            CliProcessType::HeatTreat => ProcessType::HeatTreat,
            CliProcessType::Welding => ProcessType::Welding,
            CliProcessType::Coating => ProcessType::Coating,
        }
    }
}

#[derive(Subcommand, Debug)]
pub enum ProcCommands {
    /// List manufacturing processes with filtering
    List(ListArgs),

    /// Create a new manufacturing process
    New(NewArgs),

    /// Show a process's details
    Show(ShowArgs),

    /// Edit a process in your editor
    Edit(EditArgs),

    /// Delete a process
    Delete(DeleteArgs),

    /// Archive a process (soft delete)
    Archive(ArchiveArgs),

    /// Visualize process flow with linked controls
    Flow(FlowArgs),
}

/// Process type filter
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ProcessTypeFilter {
    Machining,
    Assembly,
    Inspection,
    Test,
    Finishing,
    Packaging,
    Handling,
    HeatTreat,
    Welding,
    Coating,
    All,
}

#[derive(clap::Args, Debug)]
pub struct ListArgs {
    /// Filter by process type
    #[arg(long, short = 't', default_value = "all")]
    pub r#type: ProcessTypeFilter,

    /// Filter by status
    #[arg(long, short = 's', default_value = "all")]
    pub status: StatusFilter,

    /// Filter by author
    #[arg(long)]
    pub author: Option<String>,

    /// Show only recent processes (last 30 days)
    #[arg(long)]
    pub recent: bool,

    /// Search in title and description
    #[arg(long)]
    pub search: Option<String>,

    /// Sort by column
    #[arg(long, default_value = "title")]
    pub sort: ListColumn,

    /// Reverse sort order
    #[arg(long, short = 'r')]
    pub reverse: bool,

    /// Columns to display (comma-separated)
    #[arg(long, value_delimiter = ',', default_values_t = vec![
        ListColumn::Title,
        ListColumn::ProcessType,
        ListColumn::Operation,
        ListColumn::Status,
    ])]
    pub columns: Vec<ListColumn>,

    /// Limit number of results
    #[arg(long, short = 'n')]
    pub limit: Option<usize>,

    /// Show only count
    #[arg(long)]
    pub count: bool,

    /// Wrap text at specified width (enables multi-line rows)
    #[arg(long, short = 'w')]
    pub wrap: Option<usize>,

    /// Show full entity ID column (hidden by default, SHORT ID always shown)
    #[arg(long)]
    pub show_id: bool,

    /// Only show entities linked to these IDs (use - for stdin pipe)
    #[arg(long, value_delimiter = ',')]
    pub linked_to: Vec<String>,

    /// Filter by link type when using --linked-to (e.g., verified_by, satisfied_by)
    #[arg(long, requires = "linked_to")]
    pub via: Option<String>,
}

/// Column selection for list output
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ListColumn {
    Id,
    Title,
    ProcessType,
    Operation,
    Status,
    Author,
    Created,
}

impl std::fmt::Display for ListColumn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.key())
    }
}

impl ListColumn {
    /// Get the column key for use with TableFormatter
    pub const fn key(&self) -> &'static str {
        match self {
            ListColumn::Id => "id",
            ListColumn::Title => "title",
            ListColumn::ProcessType => "process-type",
            ListColumn::Operation => "operation",
            ListColumn::Status => "status",
            ListColumn::Author => "author",
            ListColumn::Created => "created",
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum SortField {
    Title,
    Type,
    Status,
    Created,
}

#[derive(clap::Args, Debug)]
pub struct NewArgs {
    /// Process title (required)
    #[arg(long, short = 't')]
    pub title: Option<String>,

    /// Process type
    #[arg(long, short = 'T', default_value = "machining")]
    pub r#type: CliProcessType,

    /// Operation number (e.g., "OP-010")
    #[arg(long, short = 'n')]
    pub op_number: Option<String>,

    /// Cycle time in minutes
    #[arg(long)]
    pub cycle_time: Option<f64>,

    /// Setup time in minutes
    #[arg(long)]
    pub setup_time: Option<f64>,

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
    /// Process ID or short ID (PROC@N)
    pub id: String,
}

#[derive(clap::Args, Debug)]
pub struct EditArgs {
    /// Process ID or short ID (PROC@N)
    pub id: String,
}

#[derive(clap::Args, Debug)]
pub struct DeleteArgs {
    /// Process ID or short ID (PROC@N)
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
    /// Process ID or short ID (PROC@N)
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
const PROCESS_DIRS: &[&str] = tdt_core::core::project::Project::entity_search_directories(
    tdt_core::core::identity::EntityPrefix::Proc,
);

/// Entity configuration for processes
const ENTITY_CONFIG: crate::cli::EntityConfig = crate::cli::EntityConfig {
    prefix: EntityPrefix::Proc,
    dirs: PROCESS_DIRS,
    name: "process",
    name_plural: "processes",
};

#[derive(clap::Args, Debug)]
pub struct FlowArgs {
    /// Filter by process ID (optional - shows all if omitted)
    #[arg(long, short = 'p')]
    pub process: Option<String>,

    /// Show controls for each process
    #[arg(long, short = 'c')]
    pub controls: bool,

    /// Show work instructions
    #[arg(long, short = 'w')]
    pub work_instructions: bool,
}

/// Run a process subcommand
pub fn run(cmd: ProcCommands, global: &GlobalOpts) -> Result<()> {
    match cmd {
        ProcCommands::List(args) => run_list(args, global),
        ProcCommands::New(args) => run_new(args, global),
        ProcCommands::Show(args) => run_show(args, global),
        ProcCommands::Edit(args) => run_edit(args),
        ProcCommands::Delete(args) => run_delete(args),
        ProcCommands::Archive(args) => run_archive(args),
        ProcCommands::Flow(args) => run_flow(args, global),
    }
}

fn run_list(args: ListArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let cache = EntityCache::open(&project).map_err(|e| miette::miette!("{}", e))?;
    let service = ProcessService::new(&project, &cache);
    let mut short_ids = ShortIdIndex::load(&project);

    // Resolve linked-to filter via cache
    let allowed_ids = crate::cli::helpers::resolve_linked_to(
        &args.linked_to,
        args.via.as_deref(),
        &short_ids,
        &cache,
    );

    let format = match global.output {
        OutputFormat::Auto => OutputFormat::Tsv,
        f => f,
    };

    let filter = build_proc_filter(&args);

    // Fast path: use cache when no domain-specific filters need full entities
    let can_use_cache = matches!(args.r#type, ProcessTypeFilter::All)
        && !args.recent
        && args.search.is_none()
        && !matches!(format, OutputFormat::Json | OutputFormat::Yaml);

    if can_use_cache {
        let mut entities = service.list_cached();
        sort_cached_processes(&mut entities, &args);

        // Apply linked-to filter
        if let Some(ref ids) = allowed_ids {
            entities.retain(|e| ids.contains(&e.id));
        }

        if args.reverse {
            entities.reverse();
        }
        if let Some(limit) = args.limit {
            entities.truncate(limit);
        }

        // Update short ID index
        short_ids.ensure_all(entities.iter().map(|e| e.id.clone()));
        super::utils::save_short_ids(&mut short_ids, &project);

        let mut visible: Vec<String> = args.columns.iter().map(|c| c.key().to_string()).collect();
        if args.show_id && !visible.iter().any(|c| c == "id") {
            visible.insert(0, "id".to_string());
        }
        return crate::cli::entity_cmd::output_cached_list(
            &entities,
            &PROC_LIST_SPEC,
            &visible,
            args.wrap,
            args.count,
            format,
            &project,
            |e| cached_entity_to_row(e, &short_ids),
            |e| e.file_path.as_path(),
        );
    }

    // Full entity loading path
    let (sort_field, sort_dir) = build_proc_sort(&args);
    let result = service
        .list(&filter, sort_field, sort_dir)
        .map_err(|e| miette::miette!("{}", e))?;
    let mut processes = result.items;

    // Apply linked-to filter
    if let Some(ref ids) = allowed_ids {
        processes.retain(|e| ids.contains(&e.id.to_string()));
    }

    // Post-sort for Operation column (not in service sort for cached path)
    if matches!(args.sort, ListColumn::Operation) {
        processes.sort_by(|a, b| {
            a.operation_number
                .as_deref()
                .unwrap_or("")
                .cmp(b.operation_number.as_deref().unwrap_or(""))
        });
    }

    if args.reverse {
        processes.reverse();
    }
    if let Some(limit) = args.limit {
        processes.truncate(limit);
    }

    // Count only
    if args.count {
        println!("{}", processes.len());
        return Ok(());
    }

    output_processes(&processes, &mut short_ids, &args, format, &project)
}

/// Build a ProcessFilter from CLI ListArgs
fn build_proc_filter(args: &ListArgs) -> ProcessFilter {
    let process_type = match args.r#type {
        ProcessTypeFilter::All => None,
        ProcessTypeFilter::Machining => Some(ProcessType::Machining),
        ProcessTypeFilter::Assembly => Some(ProcessType::Assembly),
        ProcessTypeFilter::Inspection => Some(ProcessType::Inspection),
        ProcessTypeFilter::Test => Some(ProcessType::Test),
        ProcessTypeFilter::Finishing => Some(ProcessType::Finishing),
        ProcessTypeFilter::Packaging => Some(ProcessType::Packaging),
        ProcessTypeFilter::Handling => Some(ProcessType::Handling),
        ProcessTypeFilter::HeatTreat => Some(ProcessType::HeatTreat),
        ProcessTypeFilter::Welding => Some(ProcessType::Welding),
        ProcessTypeFilter::Coating => Some(ProcessType::Coating),
    };

    let status = match args.status {
        StatusFilter::All => None,
        StatusFilter::Draft => Some(vec![tdt_core::core::entity::Status::Draft]),
        StatusFilter::Review => Some(vec![tdt_core::core::entity::Status::Review]),
        StatusFilter::Approved => Some(vec![tdt_core::core::entity::Status::Approved]),
        StatusFilter::Released => Some(vec![tdt_core::core::entity::Status::Released]),
        StatusFilter::Obsolete => Some(vec![tdt_core::core::entity::Status::Obsolete]),
        StatusFilter::Active => Some(vec![
            tdt_core::core::entity::Status::Draft,
            tdt_core::core::entity::Status::Review,
            tdt_core::core::entity::Status::Approved,
            tdt_core::core::entity::Status::Released,
        ]),
    };

    let recent_days = if args.recent { Some(30) } else { None };

    ProcessFilter {
        common: CommonFilter {
            status,
            author: args.author.clone(),
            search: args.search.clone(),
            recent_days,
            limit: args.limit,
            ..Default::default()
        },
        process_type,
        ..Default::default()
    }
}

/// Build sort field and direction from CLI args
fn build_proc_sort(args: &ListArgs) -> (ProcessSortField, SortDirection) {
    let field = match args.sort {
        ListColumn::Id => ProcessSortField::Id,
        ListColumn::Title => ProcessSortField::Title,
        ListColumn::ProcessType => ProcessSortField::ProcessType,
        ListColumn::Operation => ProcessSortField::OperationNumber,
        ListColumn::Status => ProcessSortField::Status,
        ListColumn::Author => ProcessSortField::Author,
        ListColumn::Created => ProcessSortField::Created,
    };

    let direction = if args.reverse {
        SortDirection::Ascending
    } else {
        SortDirection::Descending
    };

    (field, direction)
}

/// Sort cached processes based on CLI args
fn sort_cached_processes(entities: &mut [CachedEntity], args: &ListArgs) {
    match args.sort {
        ListColumn::Id => entities.sort_by(|a, b| a.id.cmp(&b.id)),
        ListColumn::Title => entities.sort_by(|a, b| a.title.cmp(&b.title)),
        ListColumn::ProcessType => entities.sort_by(|a, b| {
            a.entity_type
                .as_deref()
                .unwrap_or("")
                .cmp(b.entity_type.as_deref().unwrap_or(""))
        }),
        ListColumn::Operation => {} // Not in cache
        ListColumn::Status => entities.sort_by(|a, b| a.status.cmp(&b.status)),
        ListColumn::Author => entities.sort_by(|a, b| a.author.cmp(&b.author)),
        ListColumn::Created => entities.sort_by(|a, b| a.created.cmp(&b.created)),
    }
}

/// Output full Process entities
fn output_processes(
    processes: &[Process],
    short_ids: &mut ShortIdIndex,
    args: &ListArgs,
    format: OutputFormat,
    project: &Project,
) -> Result<()> {
    if processes.is_empty() {
        println!("No processes found.");
        return Ok(());
    }

    // Update short ID index
    short_ids.ensure_all(processes.iter().map(|p| p.id.to_string()));
    super::utils::save_short_ids(short_ids, project);

    match format {
        OutputFormat::Json => {
            let json =
                serde_json::to_string_pretty(&processes).map_err(|e| miette::miette!("{}", e))?;
            println!("{}", json);
        }
        OutputFormat::Yaml => {
            let yaml = serde_yml::to_string(&processes).map_err(|e| miette::miette!("{}", e))?;
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
            let mut visible_columns: Vec<&str> = args.columns.iter().map(|c| c.key()).collect();
            if args.show_id && !visible_columns.contains(&"id") {
                visible_columns.insert(0, "id");
            }

            let rows: Vec<TableRow> = processes
                .iter()
                .map(|p| process_to_row(p, short_ids))
                .collect();

            let config = match args.wrap {
                Some(width) => TableConfig::with_wrap(width),
                None => TableConfig::default(),
            };

            let formatter =
                TableFormatter::new(PROC_COLUMNS, "process", "PROC").with_config(config);
            formatter.output(rows, format, &visible_columns);
        }
        OutputFormat::Auto | OutputFormat::Path => {
            eprintln!("error: -o path is not supported for this view; use -o id to get entity IDs");
            std::process::exit(2);
        }
    }

    Ok(())
}

/// Convert a Process entity to a TableRow
fn process_to_row(proc: &Process, short_ids: &ShortIdIndex) -> TableRow {
    TableRow::new(proc.id.to_string(), short_ids)
        .cell("id", CellValue::Id(proc.id.to_string()))
        .cell("title", CellValue::Text(proc.title.clone()))
        .cell(
            "process-type",
            CellValue::Type(proc.process_type.to_string()),
        )
        .cell(
            "operation",
            CellValue::Text(proc.operation_number.as_deref().unwrap_or("-").to_string()),
        )
        .cell("status", CellValue::Status(proc.status))
        .cell("author", CellValue::Text(proc.author.clone()))
        .cell("created", CellValue::DateTime(proc.created))
}

/// Convert string to SkillLevel, defaulting to Intermediate
fn parse_skill_level(s: &str) -> SkillLevel {
    match s.to_lowercase().as_str() {
        "entry" => SkillLevel::Entry,
        "intermediate" => SkillLevel::Intermediate,
        "advanced" => SkillLevel::Advanced,
        "expert" => SkillLevel::Expert,
        _ => SkillLevel::default(),
    }
}

/// Convert string to ProcessType, defaulting to Machining
fn parse_process_type(s: &str) -> ProcessType {
    match s.to_lowercase().as_str() {
        "machining" => ProcessType::Machining,
        "assembly" => ProcessType::Assembly,
        "inspection" => ProcessType::Inspection,
        "test" => ProcessType::Test,
        "finishing" => ProcessType::Finishing,
        "packaging" => ProcessType::Packaging,
        "handling" => ProcessType::Handling,
        "heat_treat" => ProcessType::HeatTreat,
        "welding" => ProcessType::Welding,
        "coating" => ProcessType::Coating,
        _ => ProcessType::default(),
    }
}

fn run_new(args: NewArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let cache = EntityCache::open(&project).map_err(|e| miette::miette!("{}", e))?;
    let service = ProcessService::new(&project, &cache);
    let config = Config::load();

    let title: String;
    let process_type: ProcessType;
    let operation_number: Option<String>;
    let description: Option<String>;
    let cycle_time: Option<f64>;
    let setup_time: Option<f64>;
    let operator_skill: SkillLevel;

    if args.interactive {
        let wizard = SchemaWizard::new();
        let result = wizard.run(EntityPrefix::Proc)?;

        title = result
            .get_string("title")
            .map(String::from)
            .unwrap_or_else(|| "New Process".to_string());
        process_type = result
            .get_string("process_type")
            .map(parse_process_type)
            .unwrap_or_default();
        operation_number = result.get_string("operation_number").map(String::from);
        description = result.get_string("description").map(String::from);
        cycle_time = result.get_f64("cycle_time_minutes");
        setup_time = result.get_f64("setup_time_minutes");
        operator_skill = result
            .get_string("operator_skill")
            .map(parse_skill_level)
            .unwrap_or_default();
    } else {
        title = args.title.unwrap_or_else(|| "New Process".to_string());
        process_type = args.r#type.into();
        operation_number = args.op_number.clone();
        description = None;
        cycle_time = args.cycle_time;
        setup_time = args.setup_time;
        operator_skill = SkillLevel::default();
    }

    // Create process using service
    let input = CreateProcess {
        title: title.clone(),
        author: config.author(),
        process_type,
        operation_number,
        description,
        cycle_time_minutes: cycle_time,
        setup_time_minutes: setup_time,
        operator_skill,
        ..Default::default()
    };

    let process = service
        .create(input)
        .map_err(|e| miette::miette!("{}", e))?;

    let file_path = project
        .root()
        .join("manufacturing/processes")
        .join(format!("{}.tdt.yaml", process.id));

    // Add to short ID index
    let mut short_ids = ShortIdIndex::load(&project);
    let short_id = short_ids.add(process.id.to_string());
    super::utils::save_short_ids(&mut short_ids, &project);

    // Handle --link flags
    let added_links = crate::cli::entity_cmd::process_link_flags(
        &file_path,
        EntityPrefix::Proc,
        &args.link,
        &short_ids,
    );

    // Output based on format flag
    crate::cli::entity_cmd::output_new_entity(
        &process.id,
        &file_path,
        short_id.clone(),
        ENTITY_CONFIG.name,
        &process.title,
        Some(&format!(
            "Type: {} | {}",
            style(process.process_type.to_string()).yellow(),
            style(&process.title).white()
        )),
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

    // Use ProcessService to get the process (cache-first lookup)
    let service = ProcessService::new(&project, &cache);
    let proc = service
        .get(&resolved_id)
        .map_err(|e| miette::miette!("{}", e))?
        .ok_or_else(|| miette::miette!("No process found matching '{}'", args.id))?;

    match global.output {
        OutputFormat::Yaml => {
            let yaml = serde_yml::to_string(&proc).into_diagnostic()?;
            print!("{}", yaml);
        }
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&proc).into_diagnostic()?;
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
                    .get_short_id(&proc.id.to_string())
                    .unwrap_or_default();
                println!("{}", short_id);
            } else {
                println!("{}", proc.id);
            }
        }
        _ => {
            // Pretty format (default)
            println!("{}", style("─".repeat(60)).dim());
            println!(
                "{}: {}",
                style("ID").bold(),
                style(&proc.id.to_string()).cyan()
            );
            println!("{}: {}", style("Title").bold(), style(&proc.title).yellow());
            if let Some(ref op) = proc.operation_number {
                println!("{}: {}", style("Operation #").bold(), op);
            }
            println!("{}: {}", style("Process Type").bold(), proc.process_type);
            println!("{}: {}", style("Skill Level").bold(), proc.operator_skill);
            println!("{}: {}", style("Status").bold(), proc.status);
            println!("{}", style("─".repeat(60)).dim());

            // Setup and Cycle Time
            if proc.setup_time_minutes.is_some() || proc.cycle_time_minutes.is_some() {
                println!();
                println!("{}", style("Time Estimates:").bold());
                if let Some(setup) = proc.setup_time_minutes {
                    println!("  Setup: {} min", setup);
                }
                if let Some(cycle) = proc.cycle_time_minutes {
                    println!("  Cycle: {} min", cycle);
                }
            }

            // Equipment
            if !proc.equipment.is_empty() {
                println!();
                println!("{} ({}):", style("Equipment").bold(), proc.equipment.len());
                for equip in &proc.equipment {
                    println!("  • {}", equip.name);
                }
            }

            // Parameters
            if !proc.parameters.is_empty() {
                println!();
                println!(
                    "{} ({}):",
                    style("Parameters").bold(),
                    proc.parameters.len()
                );
                for param in &proc.parameters {
                    print!("  • {}: {}", param.name, param.value);
                    if let Some(ref units) = param.units {
                        print!(" {}", units);
                    }
                    println!();
                }
            }

            // Tags
            if !proc.tags.is_empty() {
                println!();
                println!("{}: {}", style("Tags").bold(), proc.tags.join(", "));
            }

            // Description
            if let Some(ref desc) = proc.description {
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
                proc.author,
                style("Created").dim(),
                proc.created.format("%Y-%m-%d %H:%M"),
                style("Revision").dim(),
                proc.entity_revision
            );
        }
    }

    Ok(())
}

fn run_edit(args: EditArgs) -> Result<()> {
    crate::cli::entity_cmd::run_edit_generic(&args.id, &ENTITY_CONFIG)
}

fn run_delete(args: DeleteArgs) -> Result<()> {
    crate::cli::commands::utils::run_delete(&args.id, PROCESS_DIRS, args.force, false, args.quiet)
}

fn run_archive(args: ArchiveArgs) -> Result<()> {
    crate::cli::commands::utils::run_delete(&args.id, PROCESS_DIRS, args.force, true, args.quiet)
}

/// Convert a CachedEntity to a TableRow for process output
fn cached_entity_to_row(entity: &CachedEntity, short_ids: &ShortIdIndex) -> TableRow {
    TableRow::new(entity.id.clone(), short_ids)
        .cell("id", CellValue::Id(entity.id.clone()))
        .cell("title", CellValue::Text(entity.title.clone()))
        .cell(
            "process-type",
            CellValue::Type(entity.entity_type.as_deref().unwrap_or("").to_string()),
        )
        .cell("operation", CellValue::Empty) // Not in cache
        .cell("status", CellValue::Status(entity.status))
        .cell("author", CellValue::Text(entity.author.clone()))
        .cell("created", CellValue::DateTime(entity.created))
}

fn run_flow(args: FlowArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let proc_dir = project.root().join("manufacturing/processes");

    if !proc_dir.exists() {
        println!("{}", style("No processes found in project.").yellow());
        return Ok(());
    }

    let short_ids = ShortIdIndex::load(&project);

    // Load all processes
    let mut processes: Vec<Process> = Vec::new();

    for entry in fs::read_dir(&proc_dir).into_diagnostic()? {
        let entry = entry.into_diagnostic()?;
        let path = entry.path();

        if path.extension().is_some_and(|e| e == "yaml") {
            let content = fs::read_to_string(&path).into_diagnostic()?;
            if let Ok(proc) = serde_yml::from_str::<Process>(&content) {
                // If filtering by process ID, skip non-matching
                if let Some(ref filter) = args.process {
                    let resolved = short_ids.resolve(filter).unwrap_or_else(|| filter.clone());
                    if !proc.id.to_string().contains(&resolved) {
                        continue;
                    }
                }
                processes.push(proc);
            }
        }
    }

    if processes.is_empty() {
        println!("{}", style("No processes found.").yellow());
        return Ok(());
    }

    // Sort by operation number (natural sort)
    processes.sort_by(|a, b| {
        let op_a = a.operation_number.as_deref().unwrap_or("ZZ-999");
        let op_b = b.operation_number.as_deref().unwrap_or("ZZ-999");
        op_a.cmp(op_b)
    });

    // Load controls and work instructions if requested
    let controls_dir = project.root().join("manufacturing/controls");
    let work_dir = project.root().join("manufacturing/work_instructions");

    let mut controls_by_process: std::collections::HashMap<String, Vec<(String, String)>> =
        std::collections::HashMap::new();
    let mut work_by_process: std::collections::HashMap<String, Vec<(String, String)>> =
        std::collections::HashMap::new();

    if args.controls && controls_dir.exists() {
        for entry in fs::read_dir(&controls_dir).into_diagnostic()?.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "yaml") {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(ctrl) = serde_yml::from_str::<serde_json::Value>(&content) {
                        let ctrl_id = ctrl.get("id").and_then(|v| v.as_str()).unwrap_or("");
                        let ctrl_title = ctrl.get("title").and_then(|v| v.as_str()).unwrap_or("");
                        let proc_id = ctrl
                            .get("links")
                            .and_then(|l| l.get("process"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("");

                        if !proc_id.is_empty() {
                            let short = short_ids
                                .get_short_id(ctrl_id)
                                .unwrap_or_else(|| truncate_str(ctrl_id, 8));
                            controls_by_process
                                .entry(proc_id.to_string())
                                .or_default()
                                .push((short, ctrl_title.to_string()));
                        }
                    }
                }
            }
        }
    }

    if args.work_instructions && work_dir.exists() {
        for entry in fs::read_dir(&work_dir).into_diagnostic()?.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "yaml") {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(work) = serde_yml::from_str::<serde_json::Value>(&content) {
                        let work_id = work.get("id").and_then(|v| v.as_str()).unwrap_or("");
                        let work_title = work.get("title").and_then(|v| v.as_str()).unwrap_or("");
                        let proc_id = work
                            .get("links")
                            .and_then(|l| l.get("process"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("");

                        if !proc_id.is_empty() {
                            let short = short_ids
                                .get_short_id(work_id)
                                .unwrap_or_else(|| truncate_str(work_id, 8));
                            work_by_process
                                .entry(proc_id.to_string())
                                .or_default()
                                .push((short, work_title.to_string()));
                        }
                    }
                }
            }
        }
    }

    // JSON/YAML output
    match global.output {
        OutputFormat::Json => {
            let flow: Vec<serde_json::Value> = processes.iter().map(|p| {
                let proc_id = p.id.to_string();
                serde_json::json!({
                    "id": proc_id,
                    "operation_number": p.operation_number,
                    "title": p.title,
                    "process_type": p.process_type.to_string(),
                    "cycle_time_minutes": p.cycle_time_minutes,
                    "setup_time_minutes": p.setup_time_minutes,
                    "equipment": p.equipment.iter().map(|e| e.name.clone()).collect::<Vec<_>>(),
                    "controls": controls_by_process.get(&proc_id).cloned().unwrap_or_default(),
                    "work_instructions": work_by_process.get(&proc_id).cloned().unwrap_or_default(),
                })
            }).collect();
            println!(
                "{}",
                serde_json::to_string_pretty(&flow).unwrap_or_default()
            );
            return Ok(());
        }
        OutputFormat::Yaml => {
            let flow: Vec<serde_json::Value> = processes.iter().map(|p| {
                let proc_id = p.id.to_string();
                serde_json::json!({
                    "id": proc_id,
                    "operation_number": p.operation_number,
                    "title": p.title,
                    "process_type": p.process_type.to_string(),
                    "controls": controls_by_process.get(&proc_id).cloned().unwrap_or_default(),
                    "work_instructions": work_by_process.get(&proc_id).cloned().unwrap_or_default(),
                })
            }).collect();
            println!("{}", serde_yml::to_string(&flow).unwrap_or_default());
            return Ok(());
        }
        _ => {}
    }

    // DOT/Graphviz output
    if matches!(global.output, OutputFormat::Dot) {
        println!("digraph ProcessFlow {{");
        println!("  rankdir=TB;");
        println!("  node [shape=box, style=rounded];");
        println!();

        // Create nodes for each process
        for proc in &processes {
            let proc_id = proc.id.to_string();
            let node_id = proc_id.replace('-', "_");
            let op_num = proc.operation_number.as_deref().unwrap_or("???");

            // Build label with details
            let mut label_parts = vec![format!("[{}] {}", op_num, proc.title)];
            label_parts.push(format!("Type: {}", proc.process_type));
            if let Some(cycle) = proc.cycle_time_minutes {
                if let Some(setup) = proc.setup_time_minutes {
                    label_parts.push(format!("Cycle: {:.0}m | Setup: {:.0}m", cycle, setup));
                } else {
                    label_parts.push(format!("Cycle: {:.0}m", cycle));
                }
            }

            let label = label_parts.join("\\n");

            // Color by process type
            let color = match proc.process_type {
                ProcessType::Machining => "lightblue",
                ProcessType::Assembly => "lightgreen",
                ProcessType::Inspection => "lightyellow",
                ProcessType::Test => "lightyellow",
                ProcessType::Finishing => "lavender",
                ProcessType::Packaging => "lightgray",
                _ => "white",
            };

            println!(
                "  \"{}\" [label=\"{}\" fillcolor={} style=\"rounded,filled\"];",
                node_id, label, color
            );

            // Add control nodes if requested
            if args.controls {
                if let Some(ctrls) = controls_by_process.get(&proc_id) {
                    for (i, (ctrl_short, ctrl_title)) in ctrls.iter().enumerate() {
                        let ctrl_node = format!("{}_ctrl_{}", node_id, i);
                        let ctrl_label =
                            format!("{}\\n{}", ctrl_short, truncate_str(ctrl_title, 25));
                        println!(
                            "  \"{}\" [label=\"{}\" shape=diamond fillcolor=khaki style=filled fontsize=10];",
                            ctrl_node, ctrl_label
                        );
                        println!(
                            "  \"{}\" -> \"{}\" [style=dashed arrowhead=none];",
                            node_id, ctrl_node
                        );
                    }
                }
            }
        }

        println!();

        // Create edges between consecutive processes
        for i in 0..processes.len().saturating_sub(1) {
            let from_id = processes[i].id.to_string().replace('-', "_");
            let to_id = processes[i + 1].id.to_string().replace('-', "_");
            println!("  \"{}\" -> \"{}\";", from_id, to_id);
        }

        println!("}}");
        return Ok(());
    }

    // Human-readable flow diagram
    println!();
    println!("{}", style("Process Flow").bold().cyan());
    println!("{}", style("─".repeat(60)).dim());
    println!();

    for (i, proc) in processes.iter().enumerate() {
        let proc_id = proc.id.to_string();
        let short_id = short_ids
            .get_short_id(&proc_id)
            .unwrap_or_else(|| truncate_str(&proc_id, 8));
        let op_num = proc.operation_number.as_deref().unwrap_or("???");

        // Process header
        println!(
            "[{}] {} ({})",
            style(op_num).cyan().bold(),
            style(&proc.title).bold(),
            style(&short_id).dim()
        );

        // Details line
        let mut details = vec![format!("Type: {}", proc.process_type)];
        if let Some(cycle) = proc.cycle_time_minutes {
            details.push(format!("Cycle: {:.0} min", cycle));
        }
        if let Some(setup) = proc.setup_time_minutes {
            details.push(format!("Setup: {:.0} min", setup));
        }
        println!(
            "  {} {}",
            style("│").dim(),
            style(details.join(" | ")).dim()
        );

        // Equipment
        if !proc.equipment.is_empty() {
            let equip_names: Vec<&str> = proc.equipment.iter().map(|e| e.name.as_str()).collect();
            println!(
                "  {} Equipment: {}",
                style("│").dim(),
                equip_names.join(", ")
            );
        }

        // Controls
        if args.controls {
            if let Some(ctrls) = controls_by_process.get(&proc_id) {
                for (ctrl_short, ctrl_title) in ctrls {
                    println!(
                        "  {} Controls: {} \"{}\"",
                        style("│").dim(),
                        style(ctrl_short).yellow(),
                        truncate_str(ctrl_title, 35)
                    );
                }
            }
        }

        // Work instructions
        if args.work_instructions {
            if let Some(works) = work_by_process.get(&proc_id) {
                for (work_short, work_title) in works {
                    println!(
                        "  {} Work Inst: {} \"{}\"",
                        style("│").dim(),
                        style(work_short).magenta(),
                        truncate_str(work_title, 35)
                    );
                }
            }
        }

        // Arrow to next process (if not last)
        if i < processes.len() - 1 {
            println!("  {}", style("▼").cyan());
        }
    }

    println!();
    println!(
        "{} process{} in flow",
        style(processes.len()).cyan(),
        if processes.len() == 1 { "" } else { "es" }
    );

    Ok(())
}
