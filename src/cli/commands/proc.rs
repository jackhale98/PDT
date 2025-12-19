//! `tdt proc` command - Manufacturing process management

use clap::{Subcommand, ValueEnum};
use console::style;
use miette::{IntoDiagnostic, Result};
use std::fs;

use crate::cli::helpers::{escape_csv, format_short_id, truncate_str};
use crate::cli::{GlobalOpts, OutputFormat};
use crate::core::cache::{CachedEntity, EntityCache, EntityFilter};
use crate::core::identity::{EntityId, EntityPrefix};
use crate::core::project::Project;
use crate::core::shortid::ShortIdIndex;
use crate::core::links::add_inferred_link;
use crate::core::Config;
use crate::entities::process::{Process, ProcessType};
use crate::schema::template::{TemplateContext, TemplateGenerator};
use crate::schema::wizard::SchemaWizard;

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

/// Status filter
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum StatusFilter {
    Draft,
    Review,
    Approved,
    Released,
    Obsolete,
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
        ListColumn::Id,
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
        match self {
            ListColumn::Id => write!(f, "id"),
            ListColumn::Title => write!(f, "title"),
            ListColumn::ProcessType => write!(f, "process-type"),
            ListColumn::Operation => write!(f, "operation"),
            ListColumn::Status => write!(f, "status"),
            ListColumn::Author => write!(f, "author"),
            ListColumn::Created => write!(f, "created"),
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
        ProcCommands::Flow(args) => run_flow(args, global),
    }
}

fn run_list(args: ListArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let proc_dir = project.root().join("manufacturing/processes");

    if !proc_dir.exists() {
        if args.count {
            println!("0");
        } else {
            println!("No processes found.");
        }
        return Ok(());
    }

    let format = match global.format {
        OutputFormat::Auto => OutputFormat::Tsv,
        f => f,
    };

    // Fast path: use cache when no domain-specific filters
    let can_use_cache = matches!(args.r#type, ProcessTypeFilter::All)
        && !args.recent
        && args.search.is_none()
        && !matches!(format, OutputFormat::Json | OutputFormat::Yaml);

    if can_use_cache {
        if let Ok(cache) = EntityCache::open(&project) {
            let status_filter = match args.status {
                StatusFilter::Draft => Some("draft"),
                StatusFilter::Review => Some("review"),
                StatusFilter::Approved => Some("approved"),
                StatusFilter::Released => Some("released"),
                StatusFilter::Obsolete => Some("obsolete"),
                StatusFilter::All => None,
            };

            let filter = EntityFilter {
                prefix: Some(EntityPrefix::Proc),
                status: status_filter.map(|s| s.to_string()),
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

            if args.reverse {
                entities.reverse();
            }

            if let Some(limit) = args.limit {
                entities.truncate(limit);
            }

            // Update short ID index
            let mut short_ids = ShortIdIndex::load(&project);
            short_ids.ensure_all(entities.iter().map(|e| e.id.clone()));
            let _ = short_ids.save(&project);

            return output_cached_processes(&entities, &args, &short_ids, format);
        }
    }

    // Slow path: load from files
    let mut processes: Vec<Process> = Vec::new();

    for entry in fs::read_dir(&proc_dir).into_diagnostic()? {
        let entry = entry.into_diagnostic()?;
        let path = entry.path();

        if path.extension().is_some_and(|e| e == "yaml") {
            let content = fs::read_to_string(&path).into_diagnostic()?;
            if let Ok(proc) = serde_yml::from_str::<Process>(&content) {
                processes.push(proc);
            }
        }
    }

    // Apply filters
    let processes: Vec<Process> = processes
        .into_iter()
        .filter(|p| match args.r#type {
            ProcessTypeFilter::Machining => p.process_type == ProcessType::Machining,
            ProcessTypeFilter::Assembly => p.process_type == ProcessType::Assembly,
            ProcessTypeFilter::Inspection => p.process_type == ProcessType::Inspection,
            ProcessTypeFilter::Test => p.process_type == ProcessType::Test,
            ProcessTypeFilter::Finishing => p.process_type == ProcessType::Finishing,
            ProcessTypeFilter::Packaging => p.process_type == ProcessType::Packaging,
            ProcessTypeFilter::Handling => p.process_type == ProcessType::Handling,
            ProcessTypeFilter::HeatTreat => p.process_type == ProcessType::HeatTreat,
            ProcessTypeFilter::Welding => p.process_type == ProcessType::Welding,
            ProcessTypeFilter::Coating => p.process_type == ProcessType::Coating,
            ProcessTypeFilter::All => true,
        })
        .filter(|p| match args.status {
            StatusFilter::Draft => p.status == crate::core::entity::Status::Draft,
            StatusFilter::Review => p.status == crate::core::entity::Status::Review,
            StatusFilter::Approved => p.status == crate::core::entity::Status::Approved,
            StatusFilter::Released => p.status == crate::core::entity::Status::Released,
            StatusFilter::Obsolete => p.status == crate::core::entity::Status::Obsolete,
            StatusFilter::All => true,
        })
        .filter(|p| {
            if let Some(ref author) = args.author {
                let author_lower = author.to_lowercase();
                p.author.to_lowercase().contains(&author_lower)
            } else {
                true
            }
        })
        .filter(|p| {
            if args.recent {
                let now = chrono::Utc::now();
                let thirty_days_ago = now - chrono::Duration::days(30);
                p.created >= thirty_days_ago
            } else {
                true
            }
        })
        .filter(|p| {
            if let Some(ref search) = args.search {
                let search_lower = search.to_lowercase();
                p.title.to_lowercase().contains(&search_lower)
                    || p.description
                        .as_ref()
                        .is_some_and(|d| d.to_lowercase().contains(&search_lower))
            } else {
                true
            }
        })
        .collect();

    // Sort
    let mut processes = processes;
    match args.sort {
        ListColumn::Id => processes.sort_by(|a, b| a.id.to_string().cmp(&b.id.to_string())),
        ListColumn::Title => processes.sort_by(|a, b| a.title.cmp(&b.title)),
        ListColumn::ProcessType => processes
            .sort_by(|a, b| format!("{:?}", a.process_type).cmp(&format!("{:?}", b.process_type))),
        ListColumn::Operation => processes.sort_by(|a, b| {
            a.operation_number
                .as_deref()
                .unwrap_or("")
                .cmp(b.operation_number.as_deref().unwrap_or(""))
        }),
        ListColumn::Status => {
            processes.sort_by(|a, b| format!("{:?}", a.status).cmp(&format!("{:?}", b.status)))
        }
        ListColumn::Author => processes.sort_by(|a, b| a.author.cmp(&b.author)),
        ListColumn::Created => processes.sort_by(|a, b| a.created.cmp(&b.created)),
    }

    if args.reverse {
        processes.reverse();
    }

    // Apply limit
    if let Some(limit) = args.limit {
        processes.truncate(limit);
    }

    // Count only
    if args.count {
        println!("{}", processes.len());
        return Ok(());
    }

    // No results
    if processes.is_empty() {
        println!("No processes found.");
        return Ok(());
    }

    // Update short ID index
    let mut short_ids = ShortIdIndex::load(&project);
    short_ids.ensure_all(processes.iter().map(|p| p.id.to_string()));
    let _ = short_ids.save(&project);

    // Output based on format
    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&processes).into_diagnostic()?;
            println!("{}", json);
        }
        OutputFormat::Yaml => {
            let yaml = serde_yml::to_string(&processes).into_diagnostic()?;
            print!("{}", yaml);
        }
        OutputFormat::Csv => {
            println!("short_id,id,title,type,op_number,cycle_time,status");
            for proc in &processes {
                let short_id = short_ids
                    .get_short_id(&proc.id.to_string())
                    .unwrap_or_default();
                println!(
                    "{},{},{},{},{},{},{}",
                    short_id,
                    proc.id,
                    escape_csv(&proc.title),
                    proc.process_type,
                    proc.operation_number.as_deref().unwrap_or(""),
                    proc.cycle_time_minutes
                        .map_or(String::new(), |t| t.to_string()),
                    proc.status
                );
            }
        }
        OutputFormat::Tsv => {
            // Build header
            let mut headers = Vec::new();
            let mut widths = Vec::new();

            // Always show SHORT first
            headers.push(style("SHORT").bold().dim().to_string());
            widths.push(8);

            for col in &args.columns {
                let (header, width) = match col {
                    ListColumn::Id => ("ID", 17),
                    ListColumn::Title => ("TITLE", 30),
                    ListColumn::ProcessType => ("TYPE", 12),
                    ListColumn::Operation => ("OP #", 8),
                    ListColumn::Status => ("STATUS", 10),
                    ListColumn::Author => ("AUTHOR", 20),
                    ListColumn::Created => ("CREATED", 20),
                };
                headers.push(style(header).bold().to_string());
                widths.push(width);
            }

            // Print header
            for (i, header) in headers.iter().enumerate() {
                if i > 0 {
                    print!(" ");
                }
                print!("{:<width$}", header, width = widths[i]);
            }
            println!();
            println!(
                "{}",
                "-".repeat(widths.iter().sum::<usize>() + widths.len() - 1)
            );

            // Print rows
            for proc in &processes {
                let short_id = short_ids
                    .get_short_id(&proc.id.to_string())
                    .unwrap_or_default();

                // Always show SHORT first
                print!("{:<8} ", style(&short_id).cyan());

                for col in &args.columns {
                    let value = match col {
                        ListColumn::Id => format_short_id(&proc.id),
                        ListColumn::Title => truncate_str(&proc.title, 28),
                        ListColumn::ProcessType => proc.process_type.to_string(),
                        ListColumn::Operation => {
                            proc.operation_number.as_deref().unwrap_or("-").to_string()
                        }
                        ListColumn::Status => proc.status.to_string(),
                        ListColumn::Author => truncate_str(&proc.author, 18),
                        ListColumn::Created => proc.created.format("%Y-%m-%d %H:%M").to_string(),
                    };

                    let width = match col {
                        ListColumn::Id => 17,
                        ListColumn::Title => 30,
                        ListColumn::ProcessType => 12,
                        ListColumn::Operation => 8,
                        ListColumn::Status => 10,
                        ListColumn::Author => 20,
                        ListColumn::Created => 20,
                    };

                    print!("{:<width$} ", value, width = width);
                }
                println!();
            }

            println!();
            println!(
                "{} process(es) found. Use {} to reference by short ID.",
                style(processes.len()).cyan(),
                style("PROC@N").cyan()
            );
        }
        OutputFormat::Id | OutputFormat::ShortId => {
            for proc in &processes {
                if format == OutputFormat::ShortId {
                    let short_id = short_ids
                        .get_short_id(&proc.id.to_string())
                        .unwrap_or_default();
                    println!("{}", short_id);
                } else {
                    println!("{}", proc.id);
                }
            }
        }
        OutputFormat::Md => {
            println!("| Short | ID | Title | Type | Op # | Cycle | Status |");
            println!("|---|---|---|---|---|---|---|");
            for proc in &processes {
                let short_id = short_ids
                    .get_short_id(&proc.id.to_string())
                    .unwrap_or_default();
                let cycle_str = proc
                    .cycle_time_minutes
                    .map_or("-".to_string(), |t| format!("{:.1}m", t));
                println!(
                    "| {} | {} | {} | {} | {} | {} | {} |",
                    short_id,
                    format_short_id(&proc.id),
                    proc.title,
                    proc.process_type,
                    proc.operation_number.as_deref().unwrap_or("-"),
                    cycle_str,
                    proc.status
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

    let title: String;
    let process_type: String;
    let operation_number: Option<String>;
    let description: Option<String>;

    if args.interactive {
        let wizard = SchemaWizard::new();
        let result = wizard.run(EntityPrefix::Proc)?;

        title = result
            .get_string("title")
            .map(String::from)
            .unwrap_or_else(|| "New Process".to_string());
        process_type = result
            .get_string("process_type")
            .map(String::from)
            .unwrap_or_else(|| "machining".to_string());
        operation_number = result.get_string("operation_number").map(String::from);
        description = result.get_string("description").map(String::from);
    } else {
        title = args.title.unwrap_or_else(|| "New Process".to_string());
        process_type = args.r#type.to_string();
        operation_number = args.op_number.clone();
        description = None;
    }

    // Generate ID
    let id = EntityId::new(EntityPrefix::Proc);

    // Generate template
    let generator = TemplateGenerator::new().map_err(|e| miette::miette!("{}", e))?;
    let mut ctx = TemplateContext::new(id.clone(), config.author())
        .with_title(&title)
        .with_process_type(&process_type);

    if let Some(ref op) = operation_number {
        ctx = ctx.with_operation_number(op);
    }
    if let Some(cycle) = args.cycle_time {
        ctx = ctx.with_cycle_time(cycle);
    }
    if let Some(setup) = args.setup_time {
        ctx = ctx.with_setup_time(setup);
    }

    let mut yaml_content = generator
        .generate_process(&ctx)
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
                    "description: |\n  # Detailed description of this manufacturing process\n  # Include key steps and requirements",
                    &format!("description: |\n{}", indented),
                );
            }
        }
    }

    // Write file
    let output_dir = project.root().join("manufacturing/processes");
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
                EntityPrefix::Proc,
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
                "{} Created process {}",
                style("✓").green(),
                style(short_id.clone().unwrap_or_else(|| format_short_id(&id))).cyan()
            );
            println!("   {}", style(file_path.display()).dim());
            println!(
                "   Type: {} | {}",
                style(&process_type).yellow(),
                style(&title).white()
            );

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

    // Find the process file
    let proc_dir = project.root().join("manufacturing/processes");
    let mut found_path = None;

    if proc_dir.exists() {
        for entry in fs::read_dir(&proc_dir).into_diagnostic()? {
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
        found_path.ok_or_else(|| miette::miette!("No process found matching '{}'", args.id))?;

    // Read and parse process
    let content = fs::read_to_string(&path).into_diagnostic()?;
    let proc: Process = serde_yml::from_str(&content).into_diagnostic()?;

    match global.format {
        OutputFormat::Yaml => {
            print!("{}", content);
        }
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&proc).into_diagnostic()?;
            println!("{}", json);
        }
        OutputFormat::Id | OutputFormat::ShortId => {
            if global.format == OutputFormat::ShortId {
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
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let config = Config::load();

    // Resolve short ID if needed
    let short_ids = ShortIdIndex::load(&project);
    let resolved_id = short_ids
        .resolve(&args.id)
        .unwrap_or_else(|| args.id.clone());

    // Find the process file
    let proc_dir = project.root().join("manufacturing/processes");
    let mut found_path = None;

    if proc_dir.exists() {
        for entry in fs::read_dir(&proc_dir).into_diagnostic()? {
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
        found_path.ok_or_else(|| miette::miette!("No process found matching '{}'", args.id))?;

    println!(
        "Opening {} in {}...",
        style(path.display()).cyan(),
        style(config.editor()).yellow()
    );

    config.run_editor(&path).into_diagnostic()?;

    Ok(())
}

/// Output cached processes in the requested format
fn output_cached_processes(
    entities: &[CachedEntity],
    args: &ListArgs,
    short_ids: &ShortIdIndex,
    format: OutputFormat,
) -> Result<()> {
    // Count only
    if args.count {
        println!("{}", entities.len());
        return Ok(());
    }

    // No results
    if entities.is_empty() {
        println!("No processes found.");
        return Ok(());
    }

    match format {
        OutputFormat::Csv => {
            println!("short_id,id,title,type,status");
            for entity in entities {
                let short_id = short_ids.get_short_id(&entity.id).unwrap_or_default();
                println!(
                    "{},{},{},{},{}",
                    short_id,
                    entity.id,
                    escape_csv(&entity.title),
                    entity.entity_type.as_deref().unwrap_or(""),
                    entity.status
                );
            }
        }
        OutputFormat::Tsv => {
            // Build header
            let mut headers = vec![];
            let mut widths = vec![];

            for col in &args.columns {
                let (header, width) = match col {
                    ListColumn::Id => ("ID", 17),
                    ListColumn::Title => ("TITLE", 30),
                    ListColumn::ProcessType => ("TYPE", 12),
                    ListColumn::Operation => ("OPERATION", 10),
                    ListColumn::Status => ("STATUS", 10),
                    ListColumn::Author => ("AUTHOR", 16),
                    ListColumn::Created => ("CREATED", 20),
                };
                headers.push((header, *col));
                widths.push(width);
            }

            // Print header
            print!("{:<8} ", style("SHORT").bold().dim());
            for (i, (header, _)) in headers.iter().enumerate() {
                print!("{:<width$} ", style(header).bold(), width = widths[i]);
            }
            println!();
            println!(
                "{}",
                "-".repeat(8 + widths.iter().sum::<usize>() + widths.len())
            );

            // Print rows
            for entity in entities {
                let short_id = short_ids.get_short_id(&entity.id).unwrap_or_default();
                print!("{:<8} ", style(&short_id).cyan());

                for (i, (_, col)) in headers.iter().enumerate() {
                    let cell = match col {
                        ListColumn::Id => truncate_str(&entity.id, widths[i] - 2),
                        ListColumn::Title => truncate_str(&entity.title, widths[i] - 2),
                        ListColumn::ProcessType => {
                            entity.entity_type.as_deref().unwrap_or("").to_string()
                        }
                        ListColumn::Operation => "-".to_string(), // Not in cache
                        ListColumn::Status => entity.status.clone(),
                        ListColumn::Author => truncate_str(&entity.author, widths[i] - 2),
                        ListColumn::Created => entity.created.format("%Y-%m-%d %H:%M").to_string(),
                    };
                    print!("{:<width$} ", cell, width = widths[i]);
                }
                println!();
            }

            println!();
            println!(
                "{} process(es) found. Use {} to reference by short ID.",
                style(entities.len()).cyan(),
                style("PROC@N").cyan()
            );
        }
        OutputFormat::Id | OutputFormat::ShortId => {
            for entity in entities {
                if format == OutputFormat::ShortId {
                    let short_id = short_ids.get_short_id(&entity.id).unwrap_or_default();
                    println!("{}", short_id);
                } else {
                    println!("{}", entity.id);
                }
            }
        }
        OutputFormat::Md => {
            println!("| Short | ID | Title | Type | Status |");
            println!("|---|---|---|---|---|");
            for entity in entities {
                let short_id = short_ids.get_short_id(&entity.id).unwrap_or_default();
                println!(
                    "| {} | {} | {} | {} | {} |",
                    short_id,
                    truncate_str(&entity.id, 16),
                    entity.title,
                    entity.entity_type.as_deref().unwrap_or(""),
                    entity.status
                );
            }
        }
        OutputFormat::Json | OutputFormat::Yaml | OutputFormat::Auto | OutputFormat::Path => {
            // Should not reach here - cache bypassed for these formats
            unreachable!();
        }
    }

    Ok(())
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
                        let proc_id = ctrl.get("process").and_then(|v| v.as_str()).unwrap_or("");

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
                        let proc_id = work.get("process").and_then(|v| v.as_str()).unwrap_or("");

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
    match global.format {
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
