//! `tdt capa` command - Corrective/Preventive Action management

use clap::{Subcommand, ValueEnum};
use console::style;
use miette::{IntoDiagnostic, Result};
use std::fs;

use crate::cli::commands::utils::format_link_with_title;
use crate::cli::helpers::format_short_id;
use crate::cli::table::{CellValue, ColumnDef, TableConfig, TableFormatter, TableRow};
use crate::cli::{GlobalOpts, OutputFormat};
use crate::core::cache::{CachedCapa, EntityCache};
use crate::core::identity::{EntityId, EntityPrefix};
use crate::core::project::Project;
use crate::core::shortid::ShortIdIndex;
use crate::core::Config;
use crate::entities::capa::{
    Capa, CapaStatus, CapaType, Effectiveness, EffectivenessResult, SourceType,
};
use crate::schema::template::{TemplateContext, TemplateGenerator};
use crate::schema::wizard::SchemaWizard;

#[derive(Subcommand, Debug)]
pub enum CapaCommands {
    /// List CAPAs with filtering
    List(ListArgs),

    /// Create a new CAPA
    New(NewArgs),

    /// Show a CAPA's details
    Show(ShowArgs),

    /// Edit a CAPA in your editor
    Edit(EditArgs),

    /// Delete a CAPA
    Delete(DeleteArgs),

    /// Archive a CAPA (soft delete)
    Archive(ArchiveArgs),

    /// Record effectiveness verification
    Verify(VerifyArgs),
}

/// CAPA type filter
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CapaTypeFilter {
    Corrective,
    Preventive,
    All,
}

/// CAPA status filter
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CapaStatusFilter {
    Initiation,
    Investigation,
    Implementation,
    Verification,
    Closed,
    All,
}

/// List column selection
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ListColumn {
    Id,
    Title,
    CapaType,
    Status,
    Author,
    Created,
}

impl std::fmt::Display for ListColumn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ListColumn::Id => write!(f, "id"),
            ListColumn::Title => write!(f, "title"),
            ListColumn::CapaType => write!(f, "capa-type"),
            ListColumn::Status => write!(f, "status"),
            ListColumn::Author => write!(f, "author"),
            ListColumn::Created => write!(f, "created"),
        }
    }
}

/// Column definitions for CAPA list output
const CAPA_COLUMNS: &[ColumnDef] = &[
    ColumnDef::new("id", "ID", 17),
    ColumnDef::new("title", "TITLE", 30),
    ColumnDef::new("capa-type", "TYPE", 12),
    ColumnDef::new("status", "STATUS", 14),
    ColumnDef::new("author", "AUTHOR", 20),
    ColumnDef::new("created", "CREATED", 20),
];

#[derive(clap::Args, Debug)]
pub struct ListArgs {
    /// Filter by CAPA type
    #[arg(long, short = 't', default_value = "all")]
    pub r#type: CapaTypeFilter,

    /// Filter by CAPA status
    #[arg(long, default_value = "all")]
    pub capa_status: CapaStatusFilter,

    /// Show only overdue CAPAs
    #[arg(long)]
    pub overdue: bool,

    /// Show only open CAPAs (status != closed) - shortcut filter
    #[arg(long)]
    pub open: bool,

    /// Search in title and problem statement
    #[arg(long)]
    pub search: Option<String>,

    /// Filter by author
    #[arg(long)]
    pub author: Option<String>,

    /// Show only recent CAPAs (last 30 days)
    #[arg(long)]
    pub recent: bool,

    /// Columns to display
    #[arg(long, value_delimiter = ',', default_values_t = vec![
        ListColumn::Id,
        ListColumn::Title,
        ListColumn::CapaType,
        ListColumn::Status,
    ])]
    pub columns: Vec<ListColumn>,

    /// Sort by field
    #[arg(long, default_value = "created")]
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

    /// Wrap text in columns (mobile-friendly output with specified width)
    #[arg(long, short = 'w')]
    pub wrap: Option<usize>,
}

#[derive(clap::Args, Debug)]
pub struct NewArgs {
    /// CAPA title (required)
    #[arg(long, short = 't')]
    pub title: Option<String>,

    /// CAPA type
    #[arg(long, short = 'T', default_value = "corrective")]
    pub r#type: String,

    /// Source NCR ID (for corrective actions)
    #[arg(long)]
    pub ncr: Option<String>,

    /// Source type (ncr, audit, customer_complaint, trend_analysis, risk)
    #[arg(long, short = 's', default_value = "ncr")]
    pub source: String,

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
    /// CAPA ID or short ID (CAPA@N)
    pub id: String,
}

#[derive(clap::Args, Debug)]
pub struct EditArgs {
    /// CAPA ID or short ID (CAPA@N)
    pub id: String,
}

#[derive(clap::Args, Debug)]
pub struct DeleteArgs {
    /// CAPA ID or short ID (CAPA@N)
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
    /// CAPA ID or short ID (CAPA@N)
    pub id: String,

    /// Force archive even if other entities reference this one
    #[arg(long)]
    pub force: bool,

    /// Suppress output
    #[arg(long, short = 'q')]
    pub quiet: bool,
}

/// Directories where CAPAs are stored
const CAPA_DIRS: &[&str] = &["manufacturing/capas"];

/// Entity configuration for CAPAs
const ENTITY_CONFIG: crate::cli::EntityConfig = crate::cli::EntityConfig {
    prefix: EntityPrefix::Capa,
    dirs: CAPA_DIRS,
    name: "CAPA",
    name_plural: "CAPAs",
};

/// Verification result CLI option
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum VerifyResult {
    Effective,
    Partial,
    Ineffective,
}

#[derive(clap::Args, Debug)]
pub struct VerifyArgs {
    /// CAPA ID or short ID (CAPA@N)
    pub capa: String,

    /// Verification result
    #[arg(long, short = 'r')]
    pub result: VerifyResult,

    /// Evidence or notes (optional)
    #[arg(long, short = 'e')]
    pub evidence: Option<String>,

    /// Skip confirmation prompt
    #[arg(long, short = 'y')]
    pub yes: bool,
}

/// Run a CAPA subcommand
pub fn run(cmd: CapaCommands, global: &GlobalOpts) -> Result<()> {
    match cmd {
        CapaCommands::List(args) => run_list(args, global),
        CapaCommands::New(args) => run_new(args, global),
        CapaCommands::Show(args) => run_show(args, global),
        CapaCommands::Edit(args) => run_edit(args),
        CapaCommands::Delete(args) => run_delete(args),
        CapaCommands::Archive(args) => run_archive(args),
        CapaCommands::Verify(args) => run_verify(args, global),
    }
}

fn run_list(args: ListArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let capa_dir = project.root().join("manufacturing/capas");

    if !capa_dir.exists() {
        if args.count {
            println!("0");
        } else {
            println!("No CAPAs found.");
        }
        return Ok(());
    }

    let format = match global.format {
        OutputFormat::Auto => OutputFormat::Tsv,
        f => f,
    };

    // Fast path: use cache when possible
    let can_use_cache = !args.overdue
        && !args.open
        && args.search.is_none()
        && !args.recent
        && !matches!(format, OutputFormat::Json | OutputFormat::Yaml);

    if can_use_cache {
        if let Ok(cache) = EntityCache::open(&project) {
            // Build filters for cache query
            let capa_type_filter = match args.r#type {
                CapaTypeFilter::Corrective => Some("corrective"),
                CapaTypeFilter::Preventive => Some("preventive"),
                CapaTypeFilter::All => None,
            };

            let capa_status_filter = match args.capa_status {
                CapaStatusFilter::Initiation => Some("initiation"),
                CapaStatusFilter::Investigation => Some("investigation"),
                CapaStatusFilter::Implementation => Some("implementation"),
                CapaStatusFilter::Verification => Some("verification"),
                CapaStatusFilter::Closed => Some("closed"),
                CapaStatusFilter::All => None,
            };

            let mut capas = cache.list_capas(
                None, // entity status (draft/active/etc)
                capa_type_filter,
                capa_status_filter,
                args.author.as_deref(),
                None, // limit - apply after sorting
            );

            // Sort
            match args.sort {
                ListColumn::Id => capas.sort_by(|a, b| a.id.cmp(&b.id)),
                ListColumn::Title => capas.sort_by(|a, b| a.title.cmp(&b.title)),
                ListColumn::CapaType => capas.sort_by(|a, b| {
                    a.capa_type
                        .as_deref()
                        .unwrap_or("")
                        .cmp(b.capa_type.as_deref().unwrap_or(""))
                }),
                ListColumn::Status => capas.sort_by(|a, b| {
                    a.capa_status
                        .as_deref()
                        .unwrap_or("")
                        .cmp(b.capa_status.as_deref().unwrap_or(""))
                }),
                ListColumn::Author => capas.sort_by(|a, b| a.author.cmp(&b.author)),
                ListColumn::Created => capas.sort_by(|a, b| a.created.cmp(&b.created)),
            }

            if args.reverse {
                capas.reverse();
            }

            if let Some(limit) = args.limit {
                capas.truncate(limit);
            }

            // Update short ID index
            let mut short_ids = ShortIdIndex::load(&project);
            short_ids.ensure_all(capas.iter().map(|c| c.id.clone()));
            super::utils::save_short_ids(&mut short_ids, &project);

            return output_cached_capas(&capas, &args, &short_ids, format);
        }
    }

    // Slow path: load from files
    let mut capas: Vec<Capa> = Vec::new();

    for entry in fs::read_dir(&capa_dir).into_diagnostic()? {
        let entry = entry.into_diagnostic()?;
        let path = entry.path();

        if path.extension().is_some_and(|e| e == "yaml") {
            let content = fs::read_to_string(&path).into_diagnostic()?;
            if let Ok(capa) = serde_yml::from_str::<Capa>(&content) {
                capas.push(capa);
            }
        }
    }

    let today = chrono::Local::now().date_naive();
    let thirty_days_ago = chrono::Utc::now() - chrono::Duration::days(30);

    // Apply filters
    let capas: Vec<Capa> = capas
        .into_iter()
        .filter(|c| match args.r#type {
            CapaTypeFilter::Corrective => c.capa_type == CapaType::Corrective,
            CapaTypeFilter::Preventive => c.capa_type == CapaType::Preventive,
            CapaTypeFilter::All => true,
        })
        .filter(|c| match args.capa_status {
            CapaStatusFilter::Initiation => c.capa_status == CapaStatus::Initiation,
            CapaStatusFilter::Investigation => c.capa_status == CapaStatus::Investigation,
            CapaStatusFilter::Implementation => c.capa_status == CapaStatus::Implementation,
            CapaStatusFilter::Verification => c.capa_status == CapaStatus::Verification,
            CapaStatusFilter::Closed => c.capa_status == CapaStatus::Closed,
            CapaStatusFilter::All => true,
        })
        .filter(|c| {
            if args.overdue {
                c.timeline
                    .as_ref()
                    .and_then(|t| t.target_date)
                    .is_some_and(|target| target < today && c.capa_status != CapaStatus::Closed)
            } else {
                true
            }
        })
        // Open filter - show CAPAs not closed
        .filter(|c| {
            if args.open {
                c.capa_status != CapaStatus::Closed
            } else {
                true
            }
        })
        .filter(|c| {
            if let Some(ref search) = args.search {
                let search_lower = search.to_lowercase();
                c.title.to_lowercase().contains(&search_lower)
                    || c.problem_statement
                        .as_ref()
                        .is_some_and(|d| d.to_lowercase().contains(&search_lower))
                    || c.capa_number
                        .as_ref()
                        .is_some_and(|num| num.to_lowercase().contains(&search_lower))
            } else {
                true
            }
        })
        .filter(|c| {
            if let Some(ref author) = args.author {
                c.author.to_lowercase().contains(&author.to_lowercase())
            } else {
                true
            }
        })
        .filter(|c| {
            if args.recent {
                c.created >= thirty_days_ago
            } else {
                true
            }
        })
        .collect();

    // Sort
    let mut capas = capas;
    match args.sort {
        ListColumn::Id => capas.sort_by(|a, b| a.id.to_string().cmp(&b.id.to_string())),
        ListColumn::Title => capas.sort_by(|a, b| a.title.cmp(&b.title)),
        ListColumn::CapaType => {
            capas.sort_by(|a, b| format!("{:?}", a.capa_type).cmp(&format!("{:?}", b.capa_type)))
        }
        ListColumn::Status => capas
            .sort_by(|a, b| format!("{:?}", a.capa_status).cmp(&format!("{:?}", b.capa_status))),
        ListColumn::Author => capas.sort_by(|a, b| a.author.cmp(&b.author)),
        ListColumn::Created => capas.sort_by(|a, b| a.created.cmp(&b.created)),
    }

    if args.reverse {
        capas.reverse();
    }

    // Apply limit
    if let Some(limit) = args.limit {
        capas.truncate(limit);
    }

    // Count only
    if args.count {
        println!("{}", capas.len());
        return Ok(());
    }

    // No results
    if capas.is_empty() {
        println!("No CAPAs found.");
        return Ok(());
    }

    // Update short ID index
    let mut short_ids = ShortIdIndex::load(&project);
    short_ids.ensure_all(capas.iter().map(|c| c.id.to_string()));
    super::utils::save_short_ids(&mut short_ids, &project);

    // Output based on format
    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&capas).into_diagnostic()?;
            println!("{}", json);
        }
        OutputFormat::Yaml => {
            let yaml = serde_yml::to_string(&capas).into_diagnostic()?;
            print!("{}", yaml);
        }
        OutputFormat::Csv | OutputFormat::Tsv | OutputFormat::Md => {
            // Build column list from args
            let columns: Vec<&str> = args
                .columns
                .iter()
                .map(|c| c.to_string().leak() as &str)
                .collect();

            // Build rows
            let rows: Vec<TableRow> = capas.iter().map(|c| capa_to_row(c, &short_ids)).collect();

            let config = TableConfig {
                wrap_width: args.wrap,
                show_summary: true,
            };
            let formatter = TableFormatter::new(CAPA_COLUMNS, "CAPA", "CAPA").with_config(config);
            formatter.output(rows, format, &columns);
        }
        OutputFormat::Id | OutputFormat::ShortId => {
            for capa in &capas {
                if format == OutputFormat::ShortId {
                    let short_id = short_ids
                        .get_short_id(&capa.id.to_string())
                        .unwrap_or_default();
                    println!("{}", short_id);
                } else {
                    println!("{}", capa.id);
                }
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
    let capa_type: String;
    let source_type: String;
    let problem_statement: Option<String>;

    if args.interactive {
        let wizard = SchemaWizard::new();
        let result = wizard.run(EntityPrefix::Capa)?;

        title = result
            .get_string("title")
            .map(String::from)
            .unwrap_or_else(|| "New CAPA".to_string());
        capa_type = result
            .get_string("capa_type")
            .map(String::from)
            .unwrap_or_else(|| "corrective".to_string());
        source_type = result
            .get_string("source.type")
            .map(String::from)
            .unwrap_or_else(|| "ncr".to_string());
        problem_statement = result.get_string("problem_statement").map(String::from);
    } else {
        title = args.title.unwrap_or_else(|| "New CAPA".to_string());
        capa_type = args.r#type;
        source_type = args.source;
        problem_statement = None;
    }

    // Validate enums
    capa_type
        .parse::<CapaType>()
        .map_err(|e| miette::miette!("{}", e))?;
    source_type
        .parse::<SourceType>()
        .map_err(|e| miette::miette!("{}", e))?;

    // Generate ID
    let id = EntityId::new(EntityPrefix::Capa);

    // Resolve NCR reference if provided
    let short_ids = ShortIdIndex::load(&project);
    let ncr_ref = args
        .ncr
        .as_ref()
        .map(|n| short_ids.resolve(n).unwrap_or_else(|| n.clone()));

    // Generate template
    let generator = TemplateGenerator::new().map_err(|e| miette::miette!("{}", e))?;
    let mut ctx = TemplateContext::new(id.clone(), config.author())
        .with_title(&title)
        .with_capa_type(&capa_type)
        .with_source_type(&source_type);

    if let Some(ref ncr_id) = ncr_ref {
        ctx = ctx.with_source_ref(ncr_id);
    }

    let mut yaml_content = generator
        .generate_capa(&ctx)
        .map_err(|e| miette::miette!("{}", e))?;

    // Apply wizard values via string replacement (for interactive mode)
    if args.interactive {
        if let Some(ref problem) = problem_statement {
            if !problem.is_empty() {
                let indented = problem
                    .lines()
                    .map(|line| format!("  {}", line))
                    .collect::<Vec<_>>()
                    .join("\n");
                yaml_content = yaml_content.replace(
                    "problem_statement: |\n  # Describe the problem being addressed\n  # Include scope and impact",
                    &format!("problem_statement: |\n{}", indented),
                );
            }
        }
    }

    // Write file
    let output_dir = project.root().join("manufacturing/capas");
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
        EntityPrefix::Capa,
        &args.link,
        &short_ids,
    );

    // Output based on format flag
    match global.format {
        OutputFormat::Id => {
            println!("{}", id);
        }
        OutputFormat::ShortId => {
            println!(
                "{}",
                short_id.clone().unwrap_or_else(|| format_short_id(&id))
            );
        }
        OutputFormat::Path => {
            println!("{}", file_path.display());
        }
        _ => {
            println!(
                "{} Created CAPA {}",
                style("✓").green(),
                style(short_id.clone().unwrap_or_else(|| format_short_id(&id))).cyan()
            );
            println!("   {}", style(file_path.display()).dim());
            println!(
                "   {} | {}",
                style(&capa_type).yellow(),
                style(&title).white()
            );
            if let Some(ref ncr_id) = ncr_ref {
                println!("   Source: {}", style(ncr_id).cyan());
            }

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

    // Find the CAPA file
    let capa_dir = project.root().join("manufacturing/capas");
    let mut found_path = None;

    if capa_dir.exists() {
        for entry in fs::read_dir(&capa_dir).into_diagnostic()? {
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

    let path = found_path.ok_or_else(|| miette::miette!("No CAPA found matching '{}'", args.id))?;

    // Read and parse CAPA
    let content = fs::read_to_string(&path).into_diagnostic()?;
    let capa: Capa = serde_yml::from_str(&content).into_diagnostic()?;

    match global.format {
        OutputFormat::Yaml => {
            print!("{}", content);
        }
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&capa).into_diagnostic()?;
            println!("{}", json);
        }
        OutputFormat::Id | OutputFormat::ShortId => {
            if global.format == OutputFormat::ShortId {
                let short_ids = ShortIdIndex::load(&project);
                let short_id = short_ids
                    .get_short_id(&capa.id.to_string())
                    .unwrap_or_default();
                println!("{}", short_id);
            } else {
                println!("{}", capa.id);
            }
        }
        _ => {
            // Pretty format (default)
            println!("{}", style("─".repeat(60)).dim());
            println!(
                "{}: {}",
                style("ID").bold(),
                style(&capa.id.to_string()).cyan()
            );
            println!("{}: {}", style("Title").bold(), style(&capa.title).yellow());
            println!("{}: {}", style("CAPA Type").bold(), capa.capa_type);
            println!("{}: {}", style("Status").bold(), capa.capa_status);
            println!("{}", style("─".repeat(60)).dim());

            // Problem Statement
            if let Some(ref ps) = capa.problem_statement {
                if !ps.is_empty() && !ps.starts_with('#') {
                    println!();
                    println!("{}", style("Problem Statement:").bold());
                    println!("{}", ps);
                }
            }

            // Root Cause Analysis
            if let Some(ref rca) = capa.root_cause_analysis {
                if let Some(ref rc) = rca.root_cause {
                    if !rc.is_empty() && !rc.starts_with('#') {
                        println!();
                        println!("{}: {}", style("RCA Method").bold(), rca.method);
                        println!("{}", style("Root Cause:").bold());
                        println!("{}", rc);
                    }
                }
            }

            // Actions
            if !capa.actions.is_empty() {
                println!();
                println!("{} ({}):", style("Actions").bold(), capa.actions.len());
                for action in &capa.actions {
                    let status_style = match action.status {
                        crate::entities::capa::ActionStatus::Completed
                        | crate::entities::capa::ActionStatus::Verified => {
                            style(action.status.to_string()).green()
                        }
                        crate::entities::capa::ActionStatus::InProgress => {
                            style(action.status.to_string()).yellow()
                        }
                        _ => style(action.status.to_string()).dim(),
                    };
                    println!(
                        "  {}. {} [{}]",
                        action.action_number, action.description, status_style
                    );
                }
            }

            // Tags
            if !capa.tags.is_empty() {
                println!();
                println!("{}: {}", style("Tags").bold(), capa.tags.join(", "));
            }

            // Links
            let cache = EntityCache::open(&project).ok();
            let has_links = !capa.links.ncrs.is_empty()
                || !capa.links.risks.is_empty()
                || !capa.links.processes_modified.is_empty()
                || !capa.links.controls_added.is_empty();

            if has_links {
                println!();
                println!("{}", style("Links:").bold());

                if !capa.links.ncrs.is_empty() {
                    println!("  {}:", style("NCRs").dim());
                    for id in &capa.links.ncrs {
                        let display = format_link_with_title(&id.to_string(), &short_ids, &cache);
                        println!("    {}", style(&display).cyan());
                    }
                }

                if !capa.links.risks.is_empty() {
                    println!("  {}:", style("Risks").dim());
                    for id in &capa.links.risks {
                        let display = format_link_with_title(&id.to_string(), &short_ids, &cache);
                        println!("    {}", style(&display).cyan());
                    }
                }

                if !capa.links.processes_modified.is_empty() {
                    println!("  {}:", style("Processes Modified").dim());
                    for id in &capa.links.processes_modified {
                        let display = format_link_with_title(&id.to_string(), &short_ids, &cache);
                        println!("    {}", style(&display).cyan());
                    }
                }

                if !capa.links.controls_added.is_empty() {
                    println!("  {}:", style("Controls Added").dim());
                    for id in &capa.links.controls_added {
                        let display = format_link_with_title(&id.to_string(), &short_ids, &cache);
                        println!("    {}", style(&display).cyan());
                    }
                }
            }

            // Footer
            println!("{}", style("─".repeat(60)).dim());
            println!(
                "{}: {} | {}: {} | {}: {}",
                style("Author").dim(),
                capa.author,
                style("Created").dim(),
                capa.created.format("%Y-%m-%d %H:%M"),
                style("Revision").dim(),
                capa.entity_revision
            );
        }
    }

    Ok(())
}

fn run_edit(args: EditArgs) -> Result<()> {
    crate::cli::entity_cmd::run_edit_generic(&args.id, &ENTITY_CONFIG)
}

fn run_delete(args: DeleteArgs) -> Result<()> {
    crate::cli::commands::utils::run_delete(&args.id, CAPA_DIRS, args.force, false, args.quiet)
}

fn run_archive(args: ArchiveArgs) -> Result<()> {
    crate::cli::commands::utils::run_delete(&args.id, CAPA_DIRS, args.force, true, args.quiet)
}

/// Output cached CAPAs in the requested format
fn output_cached_capas(
    capas: &[CachedCapa],
    args: &ListArgs,
    short_ids: &ShortIdIndex,
    format: OutputFormat,
) -> Result<()> {
    // Count only
    if args.count {
        println!("{}", capas.len());
        return Ok(());
    }

    // No results
    if capas.is_empty() {
        println!("No CAPAs found.");
        return Ok(());
    }

    match format {
        OutputFormat::Csv | OutputFormat::Tsv | OutputFormat::Md => {
            let columns: Vec<&str> = args
                .columns
                .iter()
                .map(|c| c.to_string().leak() as &str)
                .collect();
            let rows: Vec<TableRow> = capas
                .iter()
                .map(|c| cached_capa_to_row(c, short_ids))
                .collect();

            let config = TableConfig {
                wrap_width: args.wrap,
                show_summary: true,
            };
            let formatter = TableFormatter::new(CAPA_COLUMNS, "CAPA", "CAPA").with_config(config);
            formatter.output(rows, format, &columns);
        }
        OutputFormat::Id | OutputFormat::ShortId => {
            for capa in capas {
                if format == OutputFormat::ShortId {
                    let short_id = short_ids.get_short_id(&capa.id).unwrap_or_default();
                    println!("{}", short_id);
                } else {
                    println!("{}", capa.id);
                }
            }
        }
        OutputFormat::Json | OutputFormat::Yaml | OutputFormat::Auto | OutputFormat::Path => {
            // Should not reach here - cache bypassed for these formats
            unreachable!();
        }
    }

    Ok(())
}

/// Convert a full Capa entity to a TableRow
fn capa_to_row(capa: &Capa, short_ids: &ShortIdIndex) -> TableRow {
    TableRow::new(capa.id.to_string(), short_ids)
        .cell("id", CellValue::Id(capa.id.to_string()))
        .cell("title", CellValue::Text(capa.title.clone()))
        .cell("capa-type", CellValue::Type(capa.capa_type.to_string()))
        .cell("status", CellValue::Type(capa.capa_status.to_string()))
        .cell("author", CellValue::Text(capa.author.clone()))
        .cell("created", CellValue::DateTime(capa.created))
}

/// Convert a cached CAPA to a TableRow
fn cached_capa_to_row(capa: &CachedCapa, short_ids: &ShortIdIndex) -> TableRow {
    TableRow::new(capa.id.clone(), short_ids)
        .cell("id", CellValue::Id(capa.id.clone()))
        .cell("title", CellValue::Text(capa.title.clone()))
        .cell(
            "capa-type",
            CellValue::Type(capa.capa_type.clone().unwrap_or_default()),
        )
        .cell(
            "status",
            CellValue::Type(capa.capa_status.clone().unwrap_or_default()),
        )
        .cell("author", CellValue::Text(capa.author.clone()))
        .cell("created", CellValue::DateTime(capa.created))
}

fn run_verify(args: VerifyArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;

    // Resolve short ID if needed
    let short_ids = ShortIdIndex::load(&project);
    let resolved_id = short_ids
        .resolve(&args.capa)
        .unwrap_or_else(|| args.capa.clone());

    // Find the CAPA file
    let capa_dir = project.root().join("manufacturing/capas");
    let mut found_path = None;

    if capa_dir.exists() {
        for entry in fs::read_dir(&capa_dir).into_diagnostic()? {
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
        found_path.ok_or_else(|| miette::miette!("No CAPA found matching '{}'", args.capa))?;

    // Read and parse CAPA
    let content = fs::read_to_string(&path).into_diagnostic()?;
    let mut capa: Capa = serde_yml::from_str(&content).into_diagnostic()?;

    // Get display ID for user messages
    let display_id = short_ids
        .get_short_id(&capa.id.to_string())
        .unwrap_or_else(|| format_short_id(&capa.id));

    // Validate status allows verification
    match capa.capa_status {
        CapaStatus::Closed => {
            return Err(miette::miette!(
                "CAPA {} is already closed and cannot be verified again",
                display_id
            ));
        }
        CapaStatus::Initiation | CapaStatus::Investigation => {
            return Err(miette::miette!(
                "CAPA {} is in {} status. Actions must be implemented before verification.",
                display_id,
                capa.capa_status
            ));
        }
        _ => {} // Implementation or Verification status is OK
    }

    // Convert CLI result to entity enum
    let effectiveness_result = match args.result {
        VerifyResult::Effective => EffectivenessResult::Effective,
        VerifyResult::Partial => EffectivenessResult::PartiallyEffective,
        VerifyResult::Ineffective => EffectivenessResult::Ineffective,
    };

    // Show current state and confirmation
    if !args.yes {
        println!();
        println!("{}", style("Verifying CAPA Effectiveness").bold().cyan());
        println!("{}", style("─".repeat(50)).dim());
        println!("CAPA: {} \"{}\"", style(&display_id).cyan(), &capa.title);
        println!("Current Status: {}", capa.capa_status);
        println!();
        println!(
            "Recording verification result: {}",
            style(format!("{:?}", args.result)).yellow()
        );
        if let Some(ref evidence) = args.evidence {
            println!("Evidence: {}", evidence);
        }

        // Auto-close if effective
        if matches!(args.result, VerifyResult::Effective) {
            println!();
            println!(
                "{}",
                style("Note: CAPA will be closed automatically (result = effective)").dim()
            );
        }
        println!();

        // Simple confirmation
        print!("Continue? [y/N] ");
        std::io::Write::flush(&mut std::io::stdout()).into_diagnostic()?;
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).into_diagnostic()?;
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Cancelled.");
            return Ok(());
        }
    }

    // Update effectiveness
    let today = chrono::Local::now().date_naive();
    capa.effectiveness = Some(Effectiveness {
        verified: true,
        verified_date: Some(today),
        result: Some(effectiveness_result),
        evidence: args.evidence.clone(),
    });

    // Auto-close if effective
    if matches!(args.result, VerifyResult::Effective) {
        capa.capa_status = CapaStatus::Closed;
    } else {
        capa.capa_status = CapaStatus::Verification;
    }

    // Increment revision
    capa.entity_revision += 1;

    // Write updated CAPA
    let yaml_content = serde_yml::to_string(&capa).into_diagnostic()?;
    fs::write(&path, &yaml_content).into_diagnostic()?;

    // Output based on format
    match global.format {
        OutputFormat::Json => {
            let result = serde_json::json!({
                "id": capa.id.to_string(),
                "short_id": display_id,
                "verified": true,
                "result": effectiveness_result.to_string(),
                "status": capa.capa_status.to_string(),
            });
            println!(
                "{}",
                serde_json::to_string_pretty(&result).unwrap_or_default()
            );
        }
        OutputFormat::Yaml => {
            let result = serde_json::json!({
                "id": capa.id.to_string(),
                "verified": true,
                "result": effectiveness_result.to_string(),
                "status": capa.capa_status.to_string(),
            });
            println!("{}", serde_yml::to_string(&result).unwrap_or_default());
        }
        _ => {
            println!();
            println!(
                "{} {} verified as {}",
                style("✓").green(),
                style(&display_id).cyan(),
                style(format!("{:?}", args.result)).yellow()
            );
            if let Some(ref evidence) = args.evidence {
                println!("  Evidence: {}", evidence);
            }
            println!("  Status: {}", style(capa.capa_status.to_string()).white());
        }
    }

    Ok(())
}
