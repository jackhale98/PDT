//! `tdt ctrl` command - Control plan item management

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
use crate::entities::control::{Control, ControlType};
use crate::schema::template::{TemplateContext, TemplateGenerator};
use crate::schema::wizard::SchemaWizard;

#[derive(Subcommand, Debug)]
pub enum CtrlCommands {
    /// List control plan items with filtering
    List(ListArgs),

    /// Create a new control plan item
    New(NewArgs),

    /// Show a control item's details
    Show(ShowArgs),

    /// Edit a control item in your editor
    Edit(EditArgs),

    /// Delete a control item
    Delete(DeleteArgs),

    /// Archive a control item (soft delete)
    Archive(ArchiveArgs),
}

/// Control type filter
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ControlTypeFilter {
    Spc,
    Inspection,
    PokaYoke,
    Visual,
    FunctionalTest,
    Attribute,
    All,
}

/// Column selection for list output
#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
pub enum ListColumn {
    Id,
    Title,
    ControlType,
    Status,
    Author,
    Created,
}

impl std::fmt::Display for ListColumn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ListColumn::Id => write!(f, "id"),
            ListColumn::Title => write!(f, "title"),
            ListColumn::ControlType => write!(f, "control-type"),
            ListColumn::Status => write!(f, "status"),
            ListColumn::Author => write!(f, "author"),
            ListColumn::Created => write!(f, "created"),
        }
    }
}

/// Column definitions for control list output
const CTRL_COLUMNS: &[ColumnDef] = &[
    ColumnDef::new("id", "ID", 17),
    ColumnDef::new("title", "TITLE", 30),
    ColumnDef::new("control-type", "TYPE", 16),
    ColumnDef::new("status", "STATUS", 10),
    ColumnDef::new("author", "AUTHOR", 20),
    ColumnDef::new("created", "CREATED", 20),
];

#[derive(clap::Args, Debug)]
pub struct ListArgs {
    /// Filter by control type
    #[arg(long, short = 't', default_value = "all")]
    pub r#type: ControlTypeFilter,

    /// Filter by status
    #[arg(long, short = 's', default_value = "all")]
    pub status: StatusFilter,

    /// Filter by process ID
    #[arg(long, short = 'p')]
    pub process: Option<String>,

    /// Filter by author
    #[arg(long, short = 'a')]
    pub author: Option<String>,

    /// Show only critical (CTQ) controls
    #[arg(long)]
    pub critical: bool,

    /// Show only recent controls (last 30 days)
    #[arg(long)]
    pub recent: bool,

    /// Search in title and description
    #[arg(long)]
    pub search: Option<String>,

    /// Columns to display
    #[arg(long, short = 'c', value_delimiter = ',', default_values_t = vec![ListColumn::Title, ListColumn::ControlType, ListColumn::Status])]
    pub columns: Vec<ListColumn>,

    /// Sort by column
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
    /// Control title (required)
    #[arg(long, short = 't')]
    pub title: Option<String>,

    /// Control type
    #[arg(long, short = 'T', default_value = "inspection")]
    pub r#type: String,

    /// Parent process ID (recommended)
    #[arg(long, short = 'p')]
    pub process: Option<String>,

    /// Feature ID being controlled
    #[arg(long)]
    pub feature: Option<String>,

    /// Characteristic name
    #[arg(long, short = 'c')]
    pub characteristic: Option<String>,

    /// Mark as critical (CTQ)
    #[arg(long)]
    pub critical: bool,

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
    /// Control ID or short ID (CTRL@N)
    pub id: String,
}

#[derive(clap::Args, Debug)]
pub struct EditArgs {
    /// Control ID or short ID (CTRL@N)
    pub id: String,
}

#[derive(clap::Args, Debug)]
pub struct DeleteArgs {
    /// Control ID or short ID (CTRL@N)
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
    /// Control ID or short ID (CTRL@N)
    pub id: String,

    /// Force archive even if other entities reference this one
    #[arg(long)]
    pub force: bool,

    /// Suppress output
    #[arg(long, short = 'q')]
    pub quiet: bool,
}

/// Directories where controls are stored
const CONTROL_DIRS: &[&str] = &["manufacturing/controls"];

/// Entity configuration for controls
const ENTITY_CONFIG: crate::cli::EntityConfig = crate::cli::EntityConfig {
    prefix: EntityPrefix::Ctrl,
    dirs: CONTROL_DIRS,
    name: "control",
    name_plural: "controls",
};

/// Run a control subcommand
pub fn run(cmd: CtrlCommands, global: &GlobalOpts) -> Result<()> {
    match cmd {
        CtrlCommands::List(args) => run_list(args, global),
        CtrlCommands::New(args) => run_new(args, global),
        CtrlCommands::Show(args) => run_show(args, global),
        CtrlCommands::Edit(args) => run_edit(args),
        CtrlCommands::Delete(args) => run_delete(args),
        CtrlCommands::Archive(args) => run_archive(args),
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
    // - No type filter (control_type not in base cache)
    // - No process filter (link-based)
    // - No critical filter (nested field)
    // - No recent filter
    // - No search filter (searches in nested fields)
    // - Not JSON/YAML output
    let can_use_cache = matches!(args.r#type, ControlTypeFilter::All)
        && args.process.is_none()
        && !args.critical
        && !args.recent
        && args.search.is_none()
        && !matches!(format, OutputFormat::Json | OutputFormat::Yaml);

    if can_use_cache {
        if let Ok(cache) = EntityCache::open(&project) {
            let filter = crate::core::cache::EntityFilter {
                prefix: Some(EntityPrefix::Ctrl),
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
                ListColumn::ControlType => entities.sort_by(|a, b| a.id.cmp(&b.id)), // Can't sort by type in cache
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

            return output_cached_controls(&entities, &short_ids, &args, format);
        }
    }

    // Fall back to full YAML loading
    let ctrl_dir = project.root().join("manufacturing/controls");

    if !ctrl_dir.exists() {
        if args.count {
            println!("0");
        } else {
            println!("No controls found.");
        }
        return Ok(());
    }

    // Load and parse all controls
    let mut controls: Vec<Control> = Vec::new();

    for entry in fs::read_dir(&ctrl_dir).into_diagnostic()? {
        let entry = entry.into_diagnostic()?;
        let path = entry.path();

        if path.extension().is_some_and(|e| e == "yaml") {
            let content = fs::read_to_string(&path).into_diagnostic()?;
            if let Ok(ctrl) = serde_yml::from_str::<Control>(&content) {
                controls.push(ctrl);
            }
        }
    }

    // Resolve process filter if provided
    let process_filter = args.process.as_ref().map(|proc_id| {
        short_ids
            .resolve(proc_id)
            .unwrap_or_else(|| proc_id.clone())
    });

    // Calculate recent cutoff if needed
    let recent_cutoff = if args.recent {
        Some(chrono::Utc::now() - chrono::Duration::days(30))
    } else {
        None
    };

    // Apply filters
    let controls: Vec<Control> = controls
        .into_iter()
        .filter(|c| match args.r#type {
            ControlTypeFilter::Spc => c.control_type == ControlType::Spc,
            ControlTypeFilter::Inspection => c.control_type == ControlType::Inspection,
            ControlTypeFilter::PokaYoke => c.control_type == ControlType::PokaYoke,
            ControlTypeFilter::Visual => c.control_type == ControlType::Visual,
            ControlTypeFilter::FunctionalTest => c.control_type == ControlType::FunctionalTest,
            ControlTypeFilter::Attribute => c.control_type == ControlType::Attribute,
            ControlTypeFilter::All => true,
        })
        .filter(|c| crate::cli::entity_cmd::status_enum_matches_filter(&c.status, args.status))
        .filter(|c| {
            if let Some(ref proc_id) = process_filter {
                c.links
                    .process
                    .as_ref()
                    .is_some_and(|p| p.to_string().contains(proc_id))
            } else {
                true
            }
        })
        .filter(|c| {
            if let Some(ref author_filter) = args.author {
                c.author
                    .to_lowercase()
                    .contains(&author_filter.to_lowercase())
            } else {
                true
            }
        })
        .filter(|c| {
            if args.critical {
                c.characteristic.critical
            } else {
                true
            }
        })
        .filter(|c| {
            if let Some(cutoff) = recent_cutoff {
                c.created >= cutoff
            } else {
                true
            }
        })
        .filter(|c| {
            if let Some(ref search) = args.search {
                let search_lower = search.to_lowercase();
                c.title.to_lowercase().contains(&search_lower)
                    || c.description
                        .as_ref()
                        .is_some_and(|d| d.to_lowercase().contains(&search_lower))
                    || c.characteristic.name.to_lowercase().contains(&search_lower)
            } else {
                true
            }
        })
        .collect();

    // Sort
    let mut controls = controls;
    match args.sort {
        ListColumn::Id => controls.sort_by(|a, b| a.id.to_string().cmp(&b.id.to_string())),
        ListColumn::Title => controls.sort_by(|a, b| a.title.cmp(&b.title)),
        ListColumn::ControlType => controls
            .sort_by(|a, b| format!("{:?}", a.control_type).cmp(&format!("{:?}", b.control_type))),
        ListColumn::Status => {
            controls.sort_by(|a, b| format!("{:?}", a.status).cmp(&format!("{:?}", b.status)))
        }
        ListColumn::Author => controls.sort_by(|a, b| a.author.cmp(&b.author)),
        ListColumn::Created => controls.sort_by(|a, b| a.created.cmp(&b.created)),
    }

    if args.reverse {
        controls.reverse();
    }

    // Apply limit
    if let Some(limit) = args.limit {
        controls.truncate(limit);
    }

    // Count only
    if args.count {
        println!("{}", controls.len());
        return Ok(());
    }

    // No results
    if controls.is_empty() {
        println!("No controls found.");
        return Ok(());
    }

    // Update short ID index
    let mut short_ids = short_ids;
    short_ids.ensure_all(controls.iter().map(|c| c.id.to_string()));
    super::utils::save_short_ids(&mut short_ids, &project);

    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&controls).into_diagnostic()?;
            println!("{}", json);
        }
        OutputFormat::Yaml => {
            let yaml = serde_yml::to_string(&controls).into_diagnostic()?;
            print!("{}", yaml);
        }
        OutputFormat::Tsv | OutputFormat::Csv | OutputFormat::Md | OutputFormat::Id | OutputFormat::ShortId => {
            // Build visible columns list
            let mut visible: Vec<&str> = args.columns.iter().map(|c| c.to_string().leak() as &str).collect();
            if args.show_id && !visible.contains(&"id") {
                visible.insert(0, "id");
            }

            // Convert to TableRows
            let rows: Vec<TableRow> = controls
                .iter()
                .map(|ctrl| control_to_row(ctrl, &short_ids))
                .collect();

            // Configure table
            let config = if let Some(width) = args.wrap {
                TableConfig::with_wrap(width)
            } else {
                TableConfig::default()
            };

            let formatter = TableFormatter::new(CTRL_COLUMNS, "control", "CTRL")
                .with_config(config);
            formatter.output(rows, format, &visible);
        }
        OutputFormat::Auto | OutputFormat::Path => unreachable!(),
    }

    Ok(())
}

/// Convert a Control to a TableRow
fn control_to_row(ctrl: &Control, short_ids: &ShortIdIndex) -> TableRow {
    TableRow::new(ctrl.id.to_string(), short_ids)
        .cell("id", CellValue::Id(ctrl.id.to_string()))
        .cell("title", CellValue::Text(ctrl.title.clone()))
        .cell("control-type", CellValue::Type(ctrl.control_type.to_string()))
        .cell("status", CellValue::Status(ctrl.status))
        .cell("author", CellValue::Text(ctrl.author.clone()))
        .cell("created", CellValue::DateTime(ctrl.created))
}

/// Output cached controls (fast path - no YAML parsing needed)
fn output_cached_controls(
    entities: &[crate::core::CachedEntity],
    short_ids: &ShortIdIndex,
    args: &ListArgs,
    format: OutputFormat,
) -> Result<()> {
    if entities.is_empty() {
        println!("No controls found.");
        return Ok(());
    }

    if args.count {
        println!("{}", entities.len());
        return Ok(());
    }

    // Build visible columns list
    let mut visible: Vec<&str> = args.columns.iter().map(|c| c.to_string().leak() as &str).collect();
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

    let formatter = TableFormatter::new(CTRL_COLUMNS, "control", "CTRL")
        .with_config(config);
    formatter.output(rows, format, &visible);

    Ok(())
}

/// Convert a CachedEntity to a TableRow for controls
fn cached_entity_to_row(entity: &crate::core::CachedEntity, short_ids: &ShortIdIndex) -> TableRow {
    TableRow::new(entity.id.clone(), short_ids)
        .cell("id", CellValue::Id(entity.id.clone()))
        .cell("title", CellValue::Text(entity.title.clone()))
        .cell("control-type", CellValue::Text("-".to_string())) // Not available in cache
        .cell("status", CellValue::Status(entity.status))
        .cell("author", CellValue::Text(entity.author.clone()))
        .cell("created", CellValue::DateTime(entity.created))
}

fn run_new(args: NewArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let config = Config::load();

    let title: String;
    let control_type: String;
    let description: Option<String>;

    if args.interactive {
        let wizard = SchemaWizard::new();
        let result = wizard.run(EntityPrefix::Ctrl)?;

        title = result
            .get_string("title")
            .map(String::from)
            .unwrap_or_else(|| "New Control".to_string());
        control_type = result
            .get_string("control_type")
            .map(String::from)
            .unwrap_or_else(|| "inspection".to_string());
        description = result.get_string("description").map(String::from);
    } else {
        title = args.title.unwrap_or_else(|| "New Control".to_string());
        control_type = args.r#type;
        description = None;
    }

    // Validate control type
    control_type
        .parse::<ControlType>()
        .map_err(|e| miette::miette!("{}", e))?;

    // Generate ID
    let id = EntityId::new(EntityPrefix::Ctrl);

    // Resolve linked IDs if provided
    let short_ids = ShortIdIndex::load(&project);
    let process_id = args
        .process
        .as_ref()
        .map(|p| short_ids.resolve(p).unwrap_or_else(|| p.clone()));
    let feature_id = args
        .feature
        .as_ref()
        .map(|f| short_ids.resolve(f).unwrap_or_else(|| f.clone()));

    // Generate template
    let generator = TemplateGenerator::new().map_err(|e| miette::miette!("{}", e))?;
    let mut ctx = TemplateContext::new(id.clone(), config.author())
        .with_title(&title)
        .with_control_type(&control_type)
        .with_critical(args.critical);

    if let Some(ref proc_id) = process_id {
        ctx = ctx.with_process_id(proc_id);
    }
    if let Some(ref feat_id) = feature_id {
        ctx = ctx.with_feature_id(feat_id);
    }
    if let Some(ref char_name) = args.characteristic {
        ctx = ctx.with_characteristic_name(char_name);
    }

    let mut yaml_content = generator
        .generate_control(&ctx)
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
                    "description: |\n  # Detailed description of this control plan item\n  # Include what is being controlled and why",
                    &format!("description: |\n{}", indented),
                );
            }
        }
    }

    // Write file
    let output_dir = project.root().join("manufacturing/controls");
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
        EntityPrefix::Ctrl,
        &args.link,
        &short_ids,
    );

    // Output based on format flag
    let extra_info = format!(
        "Type: {} | {}{}",
        style(&control_type).yellow(),
        style(&title).white(),
        if args.critical {
            format!(" {}", style("[CTQ]").red().bold())
        } else {
            String::new()
        }
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

    // Find the control file
    let ctrl_dir = project.root().join("manufacturing/controls");
    let mut found_path = None;

    if ctrl_dir.exists() {
        for entry in fs::read_dir(&ctrl_dir).into_diagnostic()? {
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
        found_path.ok_or_else(|| miette::miette!("No control found matching '{}'", args.id))?;

    // Read and parse control
    let content = fs::read_to_string(&path).into_diagnostic()?;
    let ctrl: Control = serde_yml::from_str(&content).into_diagnostic()?;

    match global.format {
        OutputFormat::Yaml => {
            print!("{}", content);
        }
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&ctrl).into_diagnostic()?;
            println!("{}", json);
        }
        OutputFormat::Id | OutputFormat::ShortId => {
            if global.format == OutputFormat::ShortId {
                let short_ids = ShortIdIndex::load(&project);
                let short_id = short_ids
                    .get_short_id(&ctrl.id.to_string())
                    .unwrap_or_default();
                println!("{}", short_id);
            } else {
                println!("{}", ctrl.id);
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
                style(&ctrl.id.to_string()).cyan()
            );
            println!("{}: {}", style("Title").bold(), style(&ctrl.title).yellow());
            println!("{}: {}", style("Control Type").bold(), ctrl.control_type);
            if let Some(ref proc_id) = ctrl.links.process {
                let proc_display = format_link_with_title(&proc_id.to_string(), &short_ids, &cache);
                println!(
                    "{}: {}",
                    style("Process").bold(),
                    style(&proc_display).cyan()
                );
            }
            if let Some(ref feat_id) = ctrl.links.feature {
                let feat_display = format_link_with_title(&feat_id.to_string(), &short_ids, &cache);
                println!(
                    "{}: {}",
                    style("Feature").bold(),
                    style(&feat_display).cyan()
                );
            }
            println!("{}: {}", style("Status").bold(), ctrl.status);
            println!("{}", style("─".repeat(60)).dim());

            // Sampling info
            if let Some(ref sampling) = ctrl.sampling {
                println!();
                println!("{}", style("Sampling:").bold());
                println!("  Type: {:?}", sampling.sampling_type);
                if let Some(ref freq) = sampling.frequency {
                    println!("  Frequency: {}", freq);
                }
                if let Some(size) = sampling.sample_size {
                    println!("  Sample Size: {}", size);
                }
            }

            // Measurement info
            if let Some(ref meas) = ctrl.measurement {
                println!();
                println!("{}", style("Measurement:").bold());
                if let Some(ref method) = meas.method {
                    println!("  Method: {}", method);
                }
                if let Some(ref equip) = meas.equipment {
                    println!("  Equipment: {}", equip);
                }
            }

            // Characteristic
            if !ctrl.characteristic.name.is_empty() {
                println!();
                println!("{}", style("Characteristic:").bold());
                println!("  Name: {}", ctrl.characteristic.name);
                if let Some(nom) = ctrl.characteristic.nominal {
                    print!("  Nominal: {}", nom);
                    if let Some(ref units) = ctrl.characteristic.units {
                        print!(" {}", units);
                    }
                    println!();
                }
            }

            // Tags
            if !ctrl.tags.is_empty() {
                println!();
                println!("{}: {}", style("Tags").bold(), ctrl.tags.join(", "));
            }

            // Description
            if let Some(ref desc) = ctrl.description {
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
                ctrl.author,
                style("Created").dim(),
                ctrl.created.format("%Y-%m-%d %H:%M"),
                style("Revision").dim(),
                ctrl.entity_revision
            );
        }
    }

    Ok(())
}

fn run_edit(args: EditArgs) -> Result<()> {
    crate::cli::entity_cmd::run_edit_generic(&args.id, &ENTITY_CONFIG)
}

fn run_delete(args: DeleteArgs) -> Result<()> {
    crate::cli::commands::utils::run_delete(&args.id, CONTROL_DIRS, args.force, false, args.quiet)
}

fn run_archive(args: ArchiveArgs) -> Result<()> {
    crate::cli::commands::utils::run_delete(&args.id, CONTROL_DIRS, args.force, true, args.quiet)
}
