//! `tdt ctrl` command - Control plan item management

use clap::{Subcommand, ValueEnum};
use console::style;
use miette::{IntoDiagnostic, Result};

use crate::cli::commands::utils::format_link_with_title;
use crate::cli::filters::StatusFilter;
use crate::cli::table::{CellValue, ColumnDef, TableConfig, TableFormatter, TableRow};
use crate::cli::{GlobalOpts, OutputFormat};
use tdt_core::core::cache::EntityCache;
use tdt_core::core::identity::EntityPrefix;
use tdt_core::core::project::Project;
use tdt_core::core::shortid::ShortIdIndex;
use tdt_core::core::CachedEntity;
use tdt_core::core::Config;
use tdt_core::entities::control::{Characteristic, Control, ControlCategory, ControlType};
use tdt_core::schema::wizard::SchemaWizard;
use tdt_core::services::{
    CommonFilter, ControlFilter, ControlService, ControlSortField, CreateControl, SortDirection,
};

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

    /// Only show entities linked to these IDs (use - for stdin pipe)
    #[arg(long, value_delimiter = ',')]
    pub linked_to: Vec<String>,

    /// Filter by link type when using --linked-to (e.g., verified_by, satisfied_by)
    #[arg(long, requires = "linked_to")]
    pub via: Option<String>,
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

/// Build a ControlFilter from CLI list arguments
fn build_ctrl_filter(args: &ListArgs, short_ids: &ShortIdIndex) -> ControlFilter {
    // Map ControlTypeFilter to ControlType
    let control_type = match args.r#type {
        ControlTypeFilter::Spc => Some(ControlType::Spc),
        ControlTypeFilter::Inspection => Some(ControlType::Inspection),
        ControlTypeFilter::PokaYoke => Some(ControlType::PokaYoke),
        ControlTypeFilter::Visual => Some(ControlType::Visual),
        ControlTypeFilter::FunctionalTest => Some(ControlType::FunctionalTest),
        ControlTypeFilter::Attribute => Some(ControlType::Attribute),
        ControlTypeFilter::All => None,
    };

    // Resolve process ID if provided
    let process = args
        .process
        .as_ref()
        .map(|p| short_ids.resolve(p).unwrap_or_else(|| p.clone()));

    ControlFilter {
        common: CommonFilter {
            status: crate::cli::entity_cmd::status_filter_to_status(args.status).map(|s| vec![s]),
            author: args.author.clone(),
            search: args.search.clone(),
            recent_days: if args.recent { Some(30) } else { None },
            limit: args.limit,
            ..Default::default()
        },
        control_type,
        process,
        critical_only: args.critical,
        ..Default::default()
    }
}

/// Build sort field and direction from CLI arguments
fn build_ctrl_sort(args: &ListArgs) -> (ControlSortField, SortDirection) {
    let field = match args.sort {
        ListColumn::Id => ControlSortField::Id,
        ListColumn::Title => ControlSortField::Title,
        ListColumn::ControlType => ControlSortField::ControlType,
        ListColumn::Status => ControlSortField::Status,
        ListColumn::Author => ControlSortField::Author,
        ListColumn::Created => ControlSortField::Created,
    };

    let direction = if args.reverse {
        SortDirection::Ascending
    } else {
        SortDirection::Descending
    };

    (field, direction)
}

/// Sort cached controls by the specified column
fn sort_cached_controls(entities: &mut [CachedEntity], sort: ListColumn, reverse: bool) {
    entities.sort_by(|a, b| {
        let cmp = match sort {
            ListColumn::Id => a.id.cmp(&b.id),
            ListColumn::Title => a.title.cmp(&b.title),
            ListColumn::ControlType => a.id.cmp(&b.id), // Can't sort by type in cache
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

/// Output controls in the requested format
fn output_controls(
    controls: &[Control],
    short_ids: &mut ShortIdIndex,
    args: &ListArgs,
    format: OutputFormat,
    project: &Project,
) -> Result<()> {
    // Update short ID index
    short_ids.ensure_all(controls.iter().map(|c| c.id.to_string()));
    super::utils::save_short_ids(short_ids, project);

    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&controls).into_diagnostic()?;
            println!("{}", json);
        }
        OutputFormat::Yaml => {
            let yaml = serde_yml::to_string(&controls).into_diagnostic()?;
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
            let rows: Vec<TableRow> = controls
                .iter()
                .map(|ctrl| control_to_row(ctrl, short_ids))
                .collect();

            // Configure table
            let config = if let Some(width) = args.wrap {
                TableConfig::with_wrap(width)
            } else {
                TableConfig::default()
            };

            let formatter =
                TableFormatter::new(CTRL_COLUMNS, "control", "CTRL").with_config(config);
            formatter.output(rows, format, &visible);
        }
        OutputFormat::Auto | OutputFormat::Path => {
            eprintln!("error: -o path is not supported for this view; use -o id to get entity IDs");
            std::process::exit(2);
        }
    }

    Ok(())
}

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
    let cache = EntityCache::open(&project).map_err(|e| miette::miette!("{}", e))?;
    let service = ControlService::new(&project, &cache);
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
        sort_cached_controls(&mut entities, args.sort, args.reverse);

        // Apply limit
        if let Some(limit) = args.limit {
            entities.truncate(limit);
        }

        return output_cached_controls(&entities, &short_ids, &args, format, &project);
    }

    // Full entity loading via service
    let filter = build_ctrl_filter(&args, &short_ids);
    let (sort_field, sort_dir) = build_ctrl_sort(&args);

    let result = service
        .list(&filter, sort_field, sort_dir)
        .map_err(|e| miette::miette!("{}", e))?;
    let mut controls = result.items;

    // Apply linked-to filter
    if let Some(ref ids) = allowed_ids {
        controls.retain(|e| ids.contains(&e.id.to_string()));
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

    output_controls(&controls, &mut short_ids, &args, format, &project)
}

/// Convert a Control to a TableRow
fn control_to_row(ctrl: &Control, short_ids: &ShortIdIndex) -> TableRow {
    TableRow::new(ctrl.id.to_string(), short_ids)
        .cell("id", CellValue::Id(ctrl.id.to_string()))
        .cell("title", CellValue::Text(ctrl.title.clone()))
        .cell(
            "control-type",
            CellValue::Type(ctrl.control_type.to_string()),
        )
        .cell("status", CellValue::Status(ctrl.status))
        .cell("author", CellValue::Text(ctrl.author.clone()))
        .cell("created", CellValue::DateTime(ctrl.created))
}

/// Output cached controls (fast path - no YAML parsing needed)
fn output_cached_controls(
    entities: &[tdt_core::core::CachedEntity],
    short_ids: &ShortIdIndex,
    args: &ListArgs,
    format: OutputFormat,
    project: &Project,
) -> Result<()> {
    // The table formatter below can't render file paths; handle -o path here.
    if format == OutputFormat::Path {
        for e in entities {
            let path = if e.file_path.is_absolute() {
                e.file_path.clone()
            } else {
                project.root().join(&e.file_path)
            };
            println!("{}", path.display());
        }
        return Ok(());
    }

    if entities.is_empty() {
        println!("No controls found.");
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

    let formatter = TableFormatter::new(CTRL_COLUMNS, "control", "CTRL").with_config(config);
    formatter.output(rows, format, &visible);

    Ok(())
}

/// Convert a CachedEntity to a TableRow for controls
fn cached_entity_to_row(
    entity: &tdt_core::core::CachedEntity,
    short_ids: &ShortIdIndex,
) -> TableRow {
    TableRow::new(entity.id.clone(), short_ids)
        .cell("id", CellValue::Id(entity.id.clone()))
        .cell("title", CellValue::Text(entity.title.clone()))
        .cell("control-type", CellValue::Text("-".to_string())) // Not available in cache
        .cell("status", CellValue::Status(entity.status))
        .cell("author", CellValue::Text(entity.author.clone()))
        .cell("created", CellValue::DateTime(entity.created))
}

/// Convert string to ControlCategory
fn parse_control_category(s: &str) -> ControlCategory {
    match s.to_lowercase().as_str() {
        "variable" => ControlCategory::Variable,
        "attribute" => ControlCategory::Attribute,
        _ => ControlCategory::default(),
    }
}

fn run_new(args: NewArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let cache = EntityCache::open(&project).map_err(|e| miette::miette!("{}", e))?;
    let service = ControlService::new(&project, &cache);
    let config = Config::load();

    let title: String;
    let control_type: ControlType;
    let description: Option<String>;
    let control_category: ControlCategory;

    if args.interactive {
        let wizard = SchemaWizard::new();
        let result = wizard.run(EntityPrefix::Ctrl)?;

        title = result
            .get_string("title")
            .map(String::from)
            .unwrap_or_else(|| "New Control".to_string());
        control_type = result
            .get_string("control_type")
            .map(|s| s.parse::<ControlType>().unwrap_or_default())
            .unwrap_or_default();
        description = result.get_string("description").map(String::from);
        control_category = result
            .get_string("control_category")
            .map(parse_control_category)
            .unwrap_or_default();
    } else {
        title = args.title.unwrap_or_else(|| "New Control".to_string());
        control_type = args
            .r#type
            .parse::<ControlType>()
            .map_err(|e| miette::miette!("{}", e))?;
        description = None;
        control_category = ControlCategory::default();
    }

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

    // Build characteristic if name provided
    let characteristic = args.characteristic.as_ref().map(|name| Characteristic {
        name: name.clone(),
        nominal: None,
        upper_limit: None,
        lower_limit: None,
        units: None,
        critical: args.critical,
    });

    // Create control using service
    let input = CreateControl {
        title: title.clone(),
        author: config.author(),
        control_type,
        control_category,
        description,
        characteristic,
        process: process_id,
        feature: feature_id,
        ..Default::default()
    };

    let control = service
        .create(input)
        .map_err(|e| miette::miette!("{}", e))?;

    let file_path = project
        .root()
        .join("manufacturing/controls")
        .join(format!("{}.tdt.yaml", control.id));

    // Add to short ID index
    let mut short_ids = ShortIdIndex::load(&project);
    let short_id = short_ids.add(control.id.to_string());
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
        style(control.control_type.to_string()).yellow(),
        style(&control.title).white(),
        if args.critical {
            format!(" {}", style("[CTQ]").red().bold())
        } else {
            String::new()
        }
    );
    crate::cli::entity_cmd::output_new_entity(
        &control.id,
        &file_path,
        short_id.clone(),
        ENTITY_CONFIG.name,
        &control.title,
        Some(&extra_info),
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

    // Use ControlService to get the control (cache-first lookup)
    let service = ControlService::new(&project, &cache);
    let ctrl = service
        .get(&resolved_id)
        .map_err(|e| miette::miette!("{}", e))?
        .ok_or_else(|| miette::miette!("No control found matching '{}'", args.id))?;

    match global.output {
        OutputFormat::Yaml => {
            let yaml = serde_yml::to_string(&ctrl).into_diagnostic()?;
            print!("{}", yaml);
        }
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&ctrl).into_diagnostic()?;
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
