//! `tdt ctrl` command - Control plan item management

use clap::{Subcommand, ValueEnum};
use console::style;
use miette::{IntoDiagnostic, Result};
use std::fs;

use crate::cli::commands::utils::format_link_with_title;
use crate::cli::helpers::{escape_csv, format_short_id, truncate_str};
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
    #[arg(long, short = 'c', value_delimiter = ',', default_values_t = vec![ListColumn::Id, ListColumn::Title, ListColumn::ControlType, ListColumn::Status])]
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

/// Run a control subcommand
pub fn run(cmd: CtrlCommands, global: &GlobalOpts) -> Result<()> {
    match cmd {
        CtrlCommands::List(args) => run_list(args, global),
        CtrlCommands::New(args) => run_new(args),
        CtrlCommands::Show(args) => run_show(args, global),
        CtrlCommands::Edit(args) => run_edit(args),
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
            let status_filter = match args.status {
                StatusFilter::Draft => Some("draft"),
                StatusFilter::Review => Some("review"),
                StatusFilter::Approved => Some("approved"),
                StatusFilter::Released => Some("released"),
                StatusFilter::Obsolete => Some("obsolete"),
                StatusFilter::All => None,
            };

            let filter = crate::core::cache::EntityFilter {
                prefix: Some(EntityPrefix::Ctrl),
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
    let process_filter = args.process.as_ref().map(|proc_id| short_ids
                .resolve(proc_id)
                .unwrap_or_else(|| proc_id.clone()));

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
        .filter(|c| match args.status {
            StatusFilter::Draft => c.status == crate::core::entity::Status::Draft,
            StatusFilter::Review => c.status == crate::core::entity::Status::Review,
            StatusFilter::Approved => c.status == crate::core::entity::Status::Approved,
            StatusFilter::Released => c.status == crate::core::entity::Status::Released,
            StatusFilter::Obsolete => c.status == crate::core::entity::Status::Obsolete,
            StatusFilter::All => true,
        })
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
    let _ = short_ids.save(&project);

    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&controls).into_diagnostic()?;
            println!("{}", json);
        }
        OutputFormat::Yaml => {
            let yaml = serde_yml::to_string(&controls).into_diagnostic()?;
            print!("{}", yaml);
        }
        OutputFormat::Csv => {
            println!("short_id,id,title,type,characteristic,critical,status");
            for ctrl in &controls {
                let short_id = short_ids
                    .get_short_id(&ctrl.id.to_string())
                    .unwrap_or_default();
                println!(
                    "{},{},{},{},{},{},{}",
                    short_id,
                    ctrl.id,
                    escape_csv(&ctrl.title),
                    ctrl.control_type,
                    escape_csv(&ctrl.characteristic.name),
                    if ctrl.characteristic.critical {
                        "Y"
                    } else {
                        "N"
                    },
                    ctrl.status
                );
            }
        }
        OutputFormat::Tsv => {
            // Build header based on selected columns
            let mut header_parts = Vec::new();
            let mut widths = Vec::new();

            for col in &args.columns {
                let (name, width) = match col {
                    ListColumn::Id => ("SHORT", 20),
                    ListColumn::Title => ("TITLE", 30),
                    ListColumn::ControlType => ("TYPE", 16),
                    ListColumn::Status => ("STATUS", 10),
                    ListColumn::Author => ("AUTHOR", 20),
                    ListColumn::Created => ("CREATED", 20),
                };
                header_parts.push(format!("{:<width$}", style(name).bold(), width = width));
                widths.push(width);
            }

            println!("{}", header_parts.join(" "));
            println!(
                "{}",
                "-".repeat(widths.iter().sum::<usize>() + widths.len() - 1)
            );

            for ctrl in &controls {
                let short_id = short_ids
                    .get_short_id(&ctrl.id.to_string())
                    .unwrap_or_default();
                let mut row_parts = Vec::new();

                for (col, &width) in args.columns.iter().zip(&widths) {
                    let value = match col {
                        ListColumn::Id => {
                            let id_str = if short_id.is_empty() {
                                format_short_id(&ctrl.id)
                            } else {
                                short_id.clone()
                            };
                            style(truncate_str(&id_str, width)).cyan().to_string()
                        }
                        ListColumn::Title => truncate_str(&ctrl.title, width),
                        ListColumn::ControlType => {
                            format!("{}", ctrl.control_type)
                        }
                        ListColumn::Status => {
                            format!("{}", ctrl.status)
                        }
                        ListColumn::Author => truncate_str(&ctrl.author, width),
                        ListColumn::Created => ctrl.created.format("%Y-%m-%d %H:%M").to_string(),
                    };
                    row_parts.push(format!("{:<width$}", value, width = width));
                }

                println!("{}", row_parts.join(" "));
            }

            println!();
            println!(
                "{} control(s) found. Use {} to reference by short ID.",
                style(controls.len()).cyan(),
                style("CTRL@N").cyan()
            );
        }
        OutputFormat::Id => {
            for ctrl in &controls {
                println!("{}", ctrl.id);
            }
        }
        OutputFormat::Md => {
            println!("| Short | ID | Title | Type | Characteristic | CTQ | Status |");
            println!("|---|---|---|---|---|---|---|");
            for ctrl in &controls {
                let short_id = short_ids
                    .get_short_id(&ctrl.id.to_string())
                    .unwrap_or_default();
                let ctq = if ctrl.characteristic.critical {
                    "Yes"
                } else {
                    ""
                };
                println!(
                    "| {} | {} | {} | {} | {} | {} | {} |",
                    short_id,
                    format_short_id(&ctrl.id),
                    ctrl.title,
                    ctrl.control_type,
                    ctrl.characteristic.name,
                    ctq,
                    ctrl.status
                );
            }
        }
        OutputFormat::Auto => unreachable!(),
    }

    Ok(())
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

    match format {
        OutputFormat::Csv => {
            println!("short_id,id,title,status");
            for entity in entities {
                let short_id = short_ids.get_short_id(&entity.id).unwrap_or_default();
                println!(
                    "{},{},{},{}",
                    short_id,
                    entity.id,
                    escape_csv(&entity.title),
                    entity.status
                );
            }
        }
        OutputFormat::Tsv => {
            // Build header based on selected columns
            let mut header_parts = Vec::new();
            let mut widths = Vec::new();

            for col in &args.columns {
                let (name, width) = match col {
                    ListColumn::Id => ("SHORT", 20),
                    ListColumn::Title => ("TITLE", 30),
                    ListColumn::ControlType => ("TYPE", 16),
                    ListColumn::Status => ("STATUS", 10),
                    ListColumn::Author => ("AUTHOR", 20),
                    ListColumn::Created => ("CREATED", 20),
                };
                header_parts.push(format!("{:<width$}", style(name).bold(), width = width));
                widths.push(width);
            }

            println!("{}", header_parts.join(" "));
            println!(
                "{}",
                "-".repeat(widths.iter().sum::<usize>() + widths.len() - 1)
            );

            for entity in entities {
                let short_id = short_ids.get_short_id(&entity.id).unwrap_or_default();
                let mut row_parts = Vec::new();

                for (col, &width) in args.columns.iter().zip(&widths) {
                    let value = match col {
                        ListColumn::Id => {
                            let id_str = if short_id.is_empty() {
                                truncate_str(&entity.id, width)
                            } else {
                                short_id.clone()
                            };
                            style(truncate_str(&id_str, width)).cyan().to_string()
                        }
                        ListColumn::Title => truncate_str(&entity.title, width),
                        ListColumn::ControlType => "-".to_string(), // Not available in cache
                        ListColumn::Status => entity.status.clone(),
                        ListColumn::Author => truncate_str(&entity.author, width),
                        ListColumn::Created => entity.created.format("%Y-%m-%d %H:%M").to_string(),
                    };
                    row_parts.push(format!("{:<width$}", value, width = width));
                }

                println!("{}", row_parts.join(" "));
            }

            println!();
            println!(
                "{} control(s) found. Use {} to reference by short ID.",
                style(entities.len()).cyan(),
                style("CTRL@N").cyan()
            );
        }
        OutputFormat::Id => {
            for entity in entities {
                println!("{}", entity.id);
            }
        }
        OutputFormat::Md => {
            println!("| Short | ID | Title | Status |");
            println!("|---|---|---|---|");
            for entity in entities {
                let short_id = short_ids.get_short_id(&entity.id).unwrap_or_default();
                println!(
                    "| {} | {} | {} | {} |",
                    short_id,
                    truncate_str(&entity.id, 15),
                    entity.title,
                    entity.status
                );
            }
        }
        OutputFormat::Json | OutputFormat::Yaml | OutputFormat::Auto => {
            unreachable!()
        }
    }

    Ok(())
}

fn run_new(args: NewArgs) -> Result<()> {
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
    let _ = short_ids.save(&project);

    println!(
        "{} Created control {}",
        style("✓").green(),
        style(short_id.unwrap_or_else(|| format_short_id(&id))).cyan()
    );
    println!("   {}", style(file_path.display()).dim());
    println!(
        "   Type: {} | {}{}",
        style(&control_type).yellow(),
        style(&title).white(),
        if args.critical {
            format!(" {}", style("[CTQ]").red().bold())
        } else {
            String::new()
        }
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
        OutputFormat::Id => {
            println!("{}", ctrl.id);
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
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let config = Config::load();

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

    println!(
        "Opening {} in {}...",
        style(path.display()).cyan(),
        style(config.editor()).yellow()
    );

    config.run_editor(&path).into_diagnostic()?;

    Ok(())
}
