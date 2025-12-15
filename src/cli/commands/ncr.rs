//! `tdt ncr` command - Non-conformance report management

use clap::{Subcommand, ValueEnum};
use console::style;
use miette::{IntoDiagnostic, Result};
use std::fs;

use crate::cli::helpers::{escape_csv, format_short_id, truncate_str};
use crate::cli::{GlobalOpts, OutputFormat};
use crate::core::cache::{CachedNcr, EntityCache};
use crate::core::identity::{EntityId, EntityPrefix};
use crate::core::project::Project;
use crate::core::shortid::ShortIdIndex;
use crate::core::Config;
use crate::entities::ncr::{
    Disposition, DispositionDecision, Ncr, NcrCategory, NcrSeverity, NcrStatus, NcrType,
};
use crate::schema::template::{TemplateContext, TemplateGenerator};
use crate::schema::wizard::SchemaWizard;

/// CLI-friendly NCR type enum
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CliNcrType {
    Internal,
    Supplier,
    Customer,
}

impl std::fmt::Display for CliNcrType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CliNcrType::Internal => write!(f, "internal"),
            CliNcrType::Supplier => write!(f, "supplier"),
            CliNcrType::Customer => write!(f, "customer"),
        }
    }
}

impl From<CliNcrType> for NcrType {
    fn from(cli: CliNcrType) -> Self {
        match cli {
            CliNcrType::Internal => NcrType::Internal,
            CliNcrType::Supplier => NcrType::Supplier,
            CliNcrType::Customer => NcrType::Customer,
        }
    }
}

/// CLI-friendly NCR severity enum
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CliNcrSeverity {
    Minor,
    Major,
    Critical,
}

impl std::fmt::Display for CliNcrSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CliNcrSeverity::Minor => write!(f, "minor"),
            CliNcrSeverity::Major => write!(f, "major"),
            CliNcrSeverity::Critical => write!(f, "critical"),
        }
    }
}

impl From<CliNcrSeverity> for NcrSeverity {
    fn from(cli: CliNcrSeverity) -> Self {
        match cli {
            CliNcrSeverity::Minor => NcrSeverity::Minor,
            CliNcrSeverity::Major => NcrSeverity::Major,
            CliNcrSeverity::Critical => NcrSeverity::Critical,
        }
    }
}

/// CLI-friendly NCR category enum
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CliNcrCategory {
    Dimensional,
    Cosmetic,
    Material,
    Functional,
    Documentation,
    Process,
    Packaging,
}

impl std::fmt::Display for CliNcrCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CliNcrCategory::Dimensional => write!(f, "dimensional"),
            CliNcrCategory::Cosmetic => write!(f, "cosmetic"),
            CliNcrCategory::Material => write!(f, "material"),
            CliNcrCategory::Functional => write!(f, "functional"),
            CliNcrCategory::Documentation => write!(f, "documentation"),
            CliNcrCategory::Process => write!(f, "process"),
            CliNcrCategory::Packaging => write!(f, "packaging"),
        }
    }
}

impl From<CliNcrCategory> for NcrCategory {
    fn from(cli: CliNcrCategory) -> Self {
        match cli {
            CliNcrCategory::Dimensional => NcrCategory::Dimensional,
            CliNcrCategory::Cosmetic => NcrCategory::Cosmetic,
            CliNcrCategory::Material => NcrCategory::Material,
            CliNcrCategory::Functional => NcrCategory::Functional,
            CliNcrCategory::Documentation => NcrCategory::Documentation,
            CliNcrCategory::Process => NcrCategory::Process,
            CliNcrCategory::Packaging => NcrCategory::Packaging,
        }
    }
}

#[derive(Subcommand, Debug)]
pub enum NcrCommands {
    /// List NCRs with filtering
    List(ListArgs),

    /// Create a new NCR
    New(NewArgs),

    /// Show an NCR's details
    Show(ShowArgs),

    /// Edit an NCR in your editor
    Edit(EditArgs),

    /// Close an NCR with disposition
    Close(CloseArgs),
}

/// NCR type filter
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum NcrTypeFilter {
    Internal,
    Supplier,
    Customer,
    All,
}

/// Severity filter
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum SeverityFilter {
    Minor,
    Major,
    Critical,
    All,
}

/// NCR status filter
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum NcrStatusFilter {
    Open,
    Containment,
    Investigation,
    Disposition,
    Closed,
    All,
}

/// List column for display and sorting
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ListColumn {
    Id,
    Title,
    NcrType,
    Severity,
    Status,
    Author,
    Created,
}

impl std::fmt::Display for ListColumn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ListColumn::Id => write!(f, "id"),
            ListColumn::Title => write!(f, "title"),
            ListColumn::NcrType => write!(f, "ncr-type"),
            ListColumn::Severity => write!(f, "severity"),
            ListColumn::Status => write!(f, "status"),
            ListColumn::Author => write!(f, "author"),
            ListColumn::Created => write!(f, "created"),
        }
    }
}

#[derive(clap::Args, Debug)]
pub struct ListArgs {
    /// Filter by NCR type
    #[arg(long, short = 't', default_value = "all")]
    pub r#type: NcrTypeFilter,

    /// Filter by severity
    #[arg(long, short = 'S', default_value = "all")]
    pub severity: SeverityFilter,

    /// Filter by NCR status
    #[arg(long, default_value = "all")]
    pub ncr_status: NcrStatusFilter,

    /// Filter by author
    #[arg(long)]
    pub author: Option<String>,

    /// Show only recent NCRs (last 30 days)
    #[arg(long)]
    pub recent: bool,

    /// Search in title and description
    #[arg(long)]
    pub search: Option<String>,

    /// Show only open NCRs (status != closed) - shortcut filter
    #[arg(long)]
    pub open: bool,

    /// Columns to display
    #[arg(long, value_delimiter = ',', default_values_t = vec![
        ListColumn::Id,
        ListColumn::Title,
        ListColumn::NcrType,
        ListColumn::Severity,
        ListColumn::Status
    ])]
    pub columns: Vec<ListColumn>,

    /// Sort by column
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
}

#[derive(clap::Args, Debug)]
pub struct NewArgs {
    /// NCR title (required)
    #[arg(long, short = 't')]
    pub title: Option<String>,

    /// NCR type
    #[arg(long, short = 'T', default_value = "internal")]
    pub r#type: CliNcrType,

    /// Severity level
    #[arg(long, short = 'S', default_value = "minor")]
    pub severity: CliNcrSeverity,

    /// Category
    #[arg(long, short = 'c', default_value = "dimensional")]
    pub category: CliNcrCategory,

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
    /// NCR ID or short ID (NCR@N)
    pub id: String,
}

#[derive(clap::Args, Debug)]
pub struct EditArgs {
    /// NCR ID or short ID (NCR@N)
    pub id: String,
}

/// Disposition decision for CLI
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CliDisposition {
    /// Use the part as-is
    UseAsIs,
    /// Rework the part to spec
    Rework,
    /// Scrap the part
    Scrap,
    /// Return to supplier
    Return,
}

#[derive(clap::Args, Debug)]
pub struct CloseArgs {
    /// NCR ID or short ID (NCR@N)
    pub ncr: String,

    /// Disposition decision
    #[arg(long, short = 'd')]
    pub disposition: CliDisposition,

    /// Disposition justification/rationale
    #[arg(long, short = 'r')]
    pub rationale: Option<String>,

    /// Link to CAPA (create if needed)
    #[arg(long)]
    pub capa: Option<String>,

    /// Skip confirmation prompt
    #[arg(long, short = 'y')]
    pub yes: bool,
}

/// Run an NCR subcommand
pub fn run(cmd: NcrCommands, global: &GlobalOpts) -> Result<()> {
    match cmd {
        NcrCommands::List(args) => run_list(args, global),
        NcrCommands::New(args) => run_new(args),
        NcrCommands::Show(args) => run_show(args, global),
        NcrCommands::Edit(args) => run_edit(args),
        NcrCommands::Close(args) => run_close(args, global),
    }
}

fn run_list(args: ListArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let ncr_dir = project.root().join("manufacturing/ncrs");

    if !ncr_dir.exists() {
        if args.count {
            println!("0");
        } else {
            println!("No NCRs found.");
        }
        return Ok(());
    }

    let format = match global.format {
        OutputFormat::Auto => OutputFormat::Tsv,
        f => f,
    };

    // Fast path: use cache when possible
    let can_use_cache = !args.recent
        && args.search.is_none()
        && !args.open
        && !matches!(format, OutputFormat::Json | OutputFormat::Yaml);

    if can_use_cache {
        if let Ok(cache) = EntityCache::open(&project) {
            // Build filters for cache query
            let ncr_type_filter = match args.r#type {
                NcrTypeFilter::Internal => Some("internal"),
                NcrTypeFilter::Supplier => Some("supplier"),
                NcrTypeFilter::Customer => Some("customer"),
                NcrTypeFilter::All => None,
            };

            let severity_filter = match args.severity {
                SeverityFilter::Minor => Some("minor"),
                SeverityFilter::Major => Some("major"),
                SeverityFilter::Critical => Some("critical"),
                SeverityFilter::All => None,
            };

            let ncr_status_filter = match args.ncr_status {
                NcrStatusFilter::Open => Some("open"),
                NcrStatusFilter::Containment => Some("containment"),
                NcrStatusFilter::Investigation => Some("investigation"),
                NcrStatusFilter::Disposition => Some("disposition"),
                NcrStatusFilter::Closed => Some("closed"),
                NcrStatusFilter::All => None,
            };

            let mut ncrs = cache.list_ncrs(
                None, // entity status (draft/active/etc)
                ncr_type_filter,
                severity_filter,
                ncr_status_filter,
                None, // category
                args.author.as_deref(),
                None, // limit - apply after sorting
            );

            // Sort
            match args.sort {
                ListColumn::Id => ncrs.sort_by(|a, b| a.id.cmp(&b.id)),
                ListColumn::Title => ncrs.sort_by(|a, b| a.title.cmp(&b.title)),
                ListColumn::NcrType => ncrs.sort_by(|a, b| {
                    a.ncr_type
                        .as_deref()
                        .unwrap_or("")
                        .cmp(b.ncr_type.as_deref().unwrap_or(""))
                }),
                ListColumn::Severity => ncrs.sort_by(|a, b| {
                    a.severity
                        .as_deref()
                        .unwrap_or("")
                        .cmp(b.severity.as_deref().unwrap_or(""))
                }),
                ListColumn::Status => ncrs.sort_by(|a, b| {
                    a.ncr_status
                        .as_deref()
                        .unwrap_or("")
                        .cmp(b.ncr_status.as_deref().unwrap_or(""))
                }),
                ListColumn::Author => ncrs.sort_by(|a, b| a.author.cmp(&b.author)),
                ListColumn::Created => ncrs.sort_by(|a, b| a.created.cmp(&b.created)),
            }

            if args.reverse {
                ncrs.reverse();
            }

            if let Some(limit) = args.limit {
                ncrs.truncate(limit);
            }

            // Update short ID index
            let mut short_ids = ShortIdIndex::load(&project);
            short_ids.ensure_all(ncrs.iter().map(|n| n.id.clone()));
            let _ = short_ids.save(&project);

            return output_cached_ncrs(&ncrs, &args, &short_ids, format);
        }
    }

    // Slow path: load from files
    let mut ncrs: Vec<Ncr> = Vec::new();

    for entry in fs::read_dir(&ncr_dir).into_diagnostic()? {
        let entry = entry.into_diagnostic()?;
        let path = entry.path();

        if path.extension().map_or(false, |e| e == "yaml") {
            let content = fs::read_to_string(&path).into_diagnostic()?;
            if let Ok(ncr) = serde_yml::from_str::<Ncr>(&content) {
                ncrs.push(ncr);
            }
        }
    }

    // Apply filters
    let ncrs: Vec<Ncr> = ncrs
        .into_iter()
        .filter(|n| match args.r#type {
            NcrTypeFilter::Internal => n.ncr_type == NcrType::Internal,
            NcrTypeFilter::Supplier => n.ncr_type == NcrType::Supplier,
            NcrTypeFilter::Customer => n.ncr_type == NcrType::Customer,
            NcrTypeFilter::All => true,
        })
        .filter(|n| match args.severity {
            SeverityFilter::Minor => n.severity == NcrSeverity::Minor,
            SeverityFilter::Major => n.severity == NcrSeverity::Major,
            SeverityFilter::Critical => n.severity == NcrSeverity::Critical,
            SeverityFilter::All => true,
        })
        .filter(|n| match args.ncr_status {
            NcrStatusFilter::Open => n.ncr_status == NcrStatus::Open,
            NcrStatusFilter::Containment => n.ncr_status == NcrStatus::Containment,
            NcrStatusFilter::Investigation => n.ncr_status == NcrStatus::Investigation,
            NcrStatusFilter::Disposition => n.ncr_status == NcrStatus::Disposition,
            NcrStatusFilter::Closed => n.ncr_status == NcrStatus::Closed,
            NcrStatusFilter::All => true,
        })
        .filter(|n| {
            if let Some(ref author) = args.author {
                n.author.to_lowercase().contains(&author.to_lowercase())
            } else {
                true
            }
        })
        .filter(|n| {
            if args.recent {
                let thirty_days_ago = chrono::Utc::now() - chrono::Duration::days(30);
                n.created >= thirty_days_ago
            } else {
                true
            }
        })
        .filter(|n| {
            if let Some(ref search) = args.search {
                let search_lower = search.to_lowercase();
                n.title.to_lowercase().contains(&search_lower)
                    || n.description
                        .as_ref()
                        .map_or(false, |d| d.to_lowercase().contains(&search_lower))
                    || n.ncr_number
                        .as_ref()
                        .map_or(false, |num| num.to_lowercase().contains(&search_lower))
            } else {
                true
            }
        })
        // Open filter - show NCRs not closed
        .filter(|n| {
            if args.open {
                n.ncr_status != NcrStatus::Closed
            } else {
                true
            }
        })
        .collect();

    // Sort
    let mut ncrs = ncrs;
    match args.sort {
        ListColumn::Id => ncrs.sort_by(|a, b| a.id.to_string().cmp(&b.id.to_string())),
        ListColumn::Title => ncrs.sort_by(|a, b| a.title.cmp(&b.title)),
        ListColumn::NcrType => {
            ncrs.sort_by(|a, b| format!("{:?}", a.ncr_type).cmp(&format!("{:?}", b.ncr_type)))
        }
        ListColumn::Severity => {
            ncrs.sort_by(|a, b| format!("{:?}", a.severity).cmp(&format!("{:?}", b.severity)))
        }
        ListColumn::Status => {
            ncrs.sort_by(|a, b| format!("{:?}", a.ncr_status).cmp(&format!("{:?}", b.ncr_status)))
        }
        ListColumn::Author => ncrs.sort_by(|a, b| a.author.cmp(&b.author)),
        ListColumn::Created => ncrs.sort_by(|a, b| a.created.cmp(&b.created)),
    }

    if args.reverse {
        ncrs.reverse();
    }

    // Apply limit
    if let Some(limit) = args.limit {
        ncrs.truncate(limit);
    }

    // Count only
    if args.count {
        println!("{}", ncrs.len());
        return Ok(());
    }

    // No results
    if ncrs.is_empty() {
        println!("No NCRs found.");
        return Ok(());
    }

    // Update short ID index
    let mut short_ids = ShortIdIndex::load(&project);
    short_ids.ensure_all(ncrs.iter().map(|n| n.id.to_string()));
    let _ = short_ids.save(&project);

    // Output based on format
    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&ncrs).into_diagnostic()?;
            println!("{}", json);
        }
        OutputFormat::Yaml => {
            let yaml = serde_yml::to_string(&ncrs).into_diagnostic()?;
            print!("{}", yaml);
        }
        OutputFormat::Csv => {
            println!("short_id,id,title,type,severity,category,ncr_status");
            for ncr in &ncrs {
                let short_id = short_ids
                    .get_short_id(&ncr.id.to_string())
                    .unwrap_or_default();
                println!(
                    "{},{},{},{},{},{},{}",
                    short_id,
                    ncr.id,
                    escape_csv(&ncr.title),
                    ncr.ncr_type,
                    ncr.severity,
                    ncr.category,
                    ncr.ncr_status
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
                    ListColumn::Title => ("TITLE", 26),
                    ListColumn::NcrType => ("TYPE", 10),
                    ListColumn::Severity => ("SEVERITY", 10),
                    ListColumn::Status => ("STATUS", 12),
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
                "-".repeat(8 + widths.iter().sum::<usize>() + widths.len() * 1)
            );

            // Print rows
            for ncr in &ncrs {
                let short_id = short_ids
                    .get_short_id(&ncr.id.to_string())
                    .unwrap_or_default();
                print!("{:<8} ", style(&short_id).cyan());

                for (i, (_, col)) in headers.iter().enumerate() {
                    let cell = match col {
                        ListColumn::Id => format_short_id(&ncr.id),
                        ListColumn::Title => truncate_str(&ncr.title, widths[i] - 2),
                        ListColumn::NcrType => ncr.ncr_type.to_string(),
                        ListColumn::Severity => {
                            let severity_styled = match ncr.severity {
                                NcrSeverity::Critical => {
                                    style(ncr.severity.to_string()).red().bold()
                                }
                                NcrSeverity::Major => style(ncr.severity.to_string()).yellow(),
                                NcrSeverity::Minor => style(ncr.severity.to_string()).white(),
                            };
                            print!("{:<width$} ", severity_styled, width = widths[i]);
                            continue;
                        }
                        ListColumn::Status => ncr.ncr_status.to_string(),
                        ListColumn::Author => truncate_str(&ncr.author, widths[i] - 2),
                        ListColumn::Created => ncr.created.format("%Y-%m-%d %H:%M").to_string(),
                    };
                    print!("{:<width$} ", cell, width = widths[i]);
                }
                println!();
            }

            println!();
            println!(
                "{} NCR(s) found. Use {} to reference by short ID.",
                style(ncrs.len()).cyan(),
                style("NCR@N").cyan()
            );
        }
        OutputFormat::Id => {
            for ncr in &ncrs {
                println!("{}", ncr.id);
            }
        }
        OutputFormat::Md => {
            println!("| Short | ID | Title | Type | Severity | Category | Status |");
            println!("|---|---|---|---|---|---|---|");
            for ncr in &ncrs {
                let short_id = short_ids
                    .get_short_id(&ncr.id.to_string())
                    .unwrap_or_default();
                println!(
                    "| {} | {} | {} | {} | {} | {} | {} |",
                    short_id,
                    format_short_id(&ncr.id),
                    ncr.title,
                    ncr.ncr_type,
                    ncr.severity,
                    ncr.category,
                    ncr.ncr_status
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

    let title: String;
    let ncr_type: String;
    let severity: String;
    let category: String;

    if args.interactive {
        let wizard = SchemaWizard::new();
        let result = wizard.run(EntityPrefix::Ncr)?;

        title = result
            .get_string("title")
            .map(String::from)
            .unwrap_or_else(|| "New NCR".to_string());
        ncr_type = result
            .get_string("ncr_type")
            .map(String::from)
            .unwrap_or_else(|| "internal".to_string());
        severity = result
            .get_string("severity")
            .map(String::from)
            .unwrap_or_else(|| "minor".to_string());
        category = result
            .get_string("category")
            .map(String::from)
            .unwrap_or_else(|| "dimensional".to_string());
    } else {
        title = args.title.unwrap_or_else(|| "New NCR".to_string());
        ncr_type = args.r#type.to_string();
        severity = args.severity.to_string();
        category = args.category.to_string();
    }

    // Generate ID
    let id = EntityId::new(EntityPrefix::Ncr);

    // Generate template
    let generator = TemplateGenerator::new().map_err(|e| miette::miette!("{}", e))?;
    let ctx = TemplateContext::new(id.clone(), config.author())
        .with_title(&title)
        .with_ncr_type(&ncr_type)
        .with_ncr_severity(&severity)
        .with_ncr_category(&category);

    let yaml_content = generator
        .generate_ncr(&ctx)
        .map_err(|e| miette::miette!("{}", e))?;

    // Write file
    let output_dir = project.root().join("manufacturing/ncrs");
    if !output_dir.exists() {
        fs::create_dir_all(&output_dir).into_diagnostic()?;
    }

    let file_path = output_dir.join(format!("{}.tdt.yaml", id));
    fs::write(&file_path, &yaml_content).into_diagnostic()?;

    // Add to short ID index
    let mut short_ids = ShortIdIndex::load(&project);
    let short_id = short_ids.add(id.to_string());
    let _ = short_ids.save(&project);

    let severity_styled = match severity.as_str() {
        "critical" => style(&severity).red().bold(),
        "major" => style(&severity).yellow(),
        _ => style(&severity).white(),
    };

    println!(
        "{} Created NCR {}",
        style("✓").green(),
        style(short_id.unwrap_or_else(|| format_short_id(&id))).cyan()
    );
    println!("   {}", style(file_path.display()).dim());
    println!(
        "   {} | {} | {}",
        style(&ncr_type).yellow(),
        severity_styled,
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

    // Find the NCR file
    let ncr_dir = project.root().join("manufacturing/ncrs");
    let mut found_path = None;

    if ncr_dir.exists() {
        for entry in fs::read_dir(&ncr_dir).into_diagnostic()? {
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

    let path = found_path.ok_or_else(|| miette::miette!("No NCR found matching '{}'", args.id))?;

    // Read and parse NCR
    let content = fs::read_to_string(&path).into_diagnostic()?;
    let ncr: Ncr = serde_yml::from_str(&content).into_diagnostic()?;

    match global.format {
        OutputFormat::Yaml => {
            print!("{}", content);
        }
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&ncr).into_diagnostic()?;
            println!("{}", json);
        }
        OutputFormat::Id => {
            println!("{}", ncr.id);
        }
        _ => {
            // Pretty format (default)
            println!("{}", style("─".repeat(60)).dim());
            println!(
                "{}: {}",
                style("ID").bold(),
                style(&ncr.id.to_string()).cyan()
            );
            println!("{}: {}", style("Title").bold(), style(&ncr.title).yellow());
            println!("{}: {}", style("NCR Type").bold(), ncr.ncr_type);
            let severity_style = match ncr.severity {
                crate::entities::ncr::NcrSeverity::Critical => {
                    style(ncr.severity.to_string()).red().bold()
                }
                crate::entities::ncr::NcrSeverity::Major => style(ncr.severity.to_string()).red(),
                crate::entities::ncr::NcrSeverity::Minor => {
                    style(ncr.severity.to_string()).yellow()
                }
            };
            println!("{}: {}", style("Severity").bold(), severity_style);
            println!("{}: {}", style("NCR Status").bold(), ncr.ncr_status);
            if let Some(ref disp) = ncr.disposition {
                if let Some(decision) = disp.decision {
                    println!("{}: {}", style("Disposition").bold(), decision);
                }
            }
            println!("{}", style("─".repeat(60)).dim());

            // Description
            if let Some(ref desc) = ncr.description {
                if !desc.is_empty() && !desc.starts_with('#') {
                    println!();
                    println!("{}", style("Description:").bold());
                    println!("{}", desc);
                }
            }

            // Detection info
            if let Some(ref det) = ncr.detection {
                println!();
                println!("{}", style("Detection:").bold());
                println!("  Found at: {:?}", det.found_at);
                if let Some(ref by) = det.found_by {
                    println!("  Found by: {}", by);
                }
            }

            // Affected Items
            if let Some(ref items) = ncr.affected_items {
                println!();
                println!("{}", style("Affected Items:").bold());
                if let Some(ref pn) = items.part_number {
                    println!("  Part Number: {}", pn);
                }
                if let Some(ref lot) = items.lot_number {
                    println!("  Lot: {}", lot);
                }
                if let Some(qty) = items.quantity_affected {
                    println!("  Quantity: {}", qty);
                }
            }

            // Containment
            if !ncr.containment.is_empty() {
                println!();
                println!(
                    "{} ({}):",
                    style("Containment Actions").bold(),
                    ncr.containment.len()
                );
                for action in &ncr.containment {
                    println!("  • {} [{:?}]", action.action, action.status);
                }
            }

            // Tags
            if !ncr.tags.is_empty() {
                println!();
                println!("{}: {}", style("Tags").bold(), ncr.tags.join(", "));
            }

            // Footer
            println!("{}", style("─".repeat(60)).dim());
            println!(
                "{}: {} | {}: {} | {}: {}",
                style("Author").dim(),
                ncr.author,
                style("Created").dim(),
                ncr.created.format("%Y-%m-%d %H:%M"),
                style("Revision").dim(),
                ncr.entity_revision
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

    // Find the NCR file
    let ncr_dir = project.root().join("manufacturing/ncrs");
    let mut found_path = None;

    if ncr_dir.exists() {
        for entry in fs::read_dir(&ncr_dir).into_diagnostic()? {
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

    let path = found_path.ok_or_else(|| miette::miette!("No NCR found matching '{}'", args.id))?;

    println!(
        "Opening {} in {}...",
        style(path.display()).cyan(),
        style(config.editor()).yellow()
    );

    config.run_editor(&path).into_diagnostic()?;

    Ok(())
}

/// Output cached NCRs in the requested format
fn output_cached_ncrs(
    ncrs: &[CachedNcr],
    args: &ListArgs,
    short_ids: &ShortIdIndex,
    format: OutputFormat,
) -> Result<()> {
    // Count only
    if args.count {
        println!("{}", ncrs.len());
        return Ok(());
    }

    // No results
    if ncrs.is_empty() {
        println!("No NCRs found.");
        return Ok(());
    }

    match format {
        OutputFormat::Csv => {
            println!("short_id,id,title,type,severity,category,ncr_status");
            for ncr in ncrs {
                let short_id = short_ids.get_short_id(&ncr.id).unwrap_or_default();
                println!(
                    "{},{},{},{},{},{},{}",
                    short_id,
                    ncr.id,
                    escape_csv(&ncr.title),
                    ncr.ncr_type.as_deref().unwrap_or(""),
                    ncr.severity.as_deref().unwrap_or(""),
                    ncr.category.as_deref().unwrap_or(""),
                    ncr.ncr_status.as_deref().unwrap_or("")
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
                    ListColumn::Title => ("TITLE", 26),
                    ListColumn::NcrType => ("TYPE", 10),
                    ListColumn::Severity => ("SEVERITY", 10),
                    ListColumn::Status => ("STATUS", 12),
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
                "-".repeat(8 + widths.iter().sum::<usize>() + widths.len() * 1)
            );

            // Print rows
            for ncr in ncrs {
                let short_id = short_ids.get_short_id(&ncr.id).unwrap_or_default();
                print!("{:<8} ", style(&short_id).cyan());

                for (i, (_, col)) in headers.iter().enumerate() {
                    let cell = match col {
                        ListColumn::Id => truncate_str(&ncr.id, widths[i] - 2),
                        ListColumn::Title => truncate_str(&ncr.title, widths[i] - 2),
                        ListColumn::NcrType => ncr.ncr_type.as_deref().unwrap_or("").to_string(),
                        ListColumn::Severity => {
                            let severity = ncr.severity.as_deref().unwrap_or("");
                            let severity_styled = match severity {
                                "critical" => style(severity.to_string()).red().bold(),
                                "major" => style(severity.to_string()).yellow(),
                                _ => style(severity.to_string()).white(),
                            };
                            print!("{:<width$} ", severity_styled, width = widths[i]);
                            continue;
                        }
                        ListColumn::Status => ncr.ncr_status.as_deref().unwrap_or("").to_string(),
                        ListColumn::Author => truncate_str(&ncr.author, widths[i] - 2),
                        ListColumn::Created => ncr.created.format("%Y-%m-%d %H:%M").to_string(),
                    };
                    print!("{:<width$} ", cell, width = widths[i]);
                }
                println!();
            }

            println!();
            println!(
                "{} NCR(s) found. Use {} to reference by short ID.",
                style(ncrs.len()).cyan(),
                style("NCR@N").cyan()
            );
        }
        OutputFormat::Id => {
            for ncr in ncrs {
                println!("{}", ncr.id);
            }
        }
        OutputFormat::Md => {
            println!("| Short | ID | Title | Type | Severity | Category | Status |");
            println!("|---|---|---|---|---|---|---|");
            for ncr in ncrs {
                let short_id = short_ids.get_short_id(&ncr.id).unwrap_or_default();
                println!(
                    "| {} | {} | {} | {} | {} | {} | {} |",
                    short_id,
                    truncate_str(&ncr.id, 16),
                    ncr.title,
                    ncr.ncr_type.as_deref().unwrap_or(""),
                    ncr.severity.as_deref().unwrap_or(""),
                    ncr.category.as_deref().unwrap_or(""),
                    ncr.ncr_status.as_deref().unwrap_or("")
                );
            }
        }
        OutputFormat::Json | OutputFormat::Yaml | OutputFormat::Auto => {
            // Should not reach here - cache bypassed for these formats
            unreachable!();
        }
    }

    Ok(())
}

fn run_close(args: CloseArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let config = Config::load();

    // Resolve short ID if needed
    let short_ids = ShortIdIndex::load(&project);
    let resolved_id = short_ids
        .resolve(&args.ncr)
        .unwrap_or_else(|| args.ncr.clone());

    // Find the NCR file
    let ncr_dir = project.root().join("manufacturing/ncrs");
    let mut found_path = None;

    if ncr_dir.exists() {
        for entry in fs::read_dir(&ncr_dir).into_diagnostic()? {
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

    let path = found_path.ok_or_else(|| miette::miette!("No NCR found matching '{}'", args.ncr))?;

    // Read and parse NCR
    let content = fs::read_to_string(&path).into_diagnostic()?;
    let mut ncr: Ncr = serde_yml::from_str(&content).into_diagnostic()?;

    // Get display ID for user messages
    let display_id = short_ids
        .get_short_id(&ncr.id.to_string())
        .unwrap_or_else(|| format_short_id(&ncr.id));

    // Validate status allows closing
    if ncr.ncr_status == NcrStatus::Closed {
        return Err(miette::miette!("NCR {} is already closed", display_id));
    }

    // Convert CLI disposition to entity enum
    let disposition_decision = match args.disposition {
        CliDisposition::UseAsIs => DispositionDecision::UseAsIs,
        CliDisposition::Rework => DispositionDecision::Rework,
        CliDisposition::Scrap => DispositionDecision::Scrap,
        CliDisposition::Return => DispositionDecision::ReturnToSupplier,
    };

    // Resolve CAPA link if provided
    let capa_ref = args
        .capa
        .as_ref()
        .map(|c| short_ids.resolve(c).unwrap_or_else(|| c.clone()));

    // Show current state and confirmation
    if !args.yes {
        println!();
        println!("{}", style("Closing NCR").bold().cyan());
        println!("{}", style("─".repeat(50)).dim());
        println!("NCR: {} \"{}\"", style(&display_id).cyan(), &ncr.title);
        println!("Current Status: {}", ncr.ncr_status);
        println!("Severity: {}", ncr.severity);
        println!();
        println!(
            "Disposition: {}",
            style(format!("{:?}", args.disposition)).yellow()
        );
        if let Some(ref rationale) = args.rationale {
            println!("Rationale: {}", rationale);
        }
        if let Some(ref capa) = capa_ref {
            let capa_display = short_ids.get_short_id(capa).unwrap_or_else(|| capa.clone());
            println!("Linked CAPA: {}", style(&capa_display).cyan());
        }
        println!();

        // Simple confirmation
        print!("Confirm close? [y/N] ");
        std::io::Write::flush(&mut std::io::stdout()).into_diagnostic()?;
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).into_diagnostic()?;
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Cancelled.");
            return Ok(());
        }
    }

    // Update disposition
    let today = chrono::Local::now().date_naive();
    ncr.disposition = Some(Disposition {
        decision: Some(disposition_decision),
        decision_date: Some(today),
        decision_by: Some(config.author().to_string()),
        justification: args.rationale.clone(),
        mrb_required: false,
    });

    // Update status
    ncr.ncr_status = NcrStatus::Closed;

    // Add CAPA link if provided
    if let Some(ref capa_id) = capa_ref {
        if let Ok(entity_id) = capa_id.parse::<EntityId>() {
            ncr.links.capa = Some(entity_id);
        }
    }

    // Increment revision
    ncr.entity_revision += 1;

    // Write updated NCR
    let yaml_content = serde_yml::to_string(&ncr).into_diagnostic()?;
    fs::write(&path, &yaml_content).into_diagnostic()?;

    // Output based on format
    match global.format {
        OutputFormat::Json => {
            let result = serde_json::json!({
                "id": ncr.id.to_string(),
                "short_id": display_id,
                "ncr_status": "closed",
                "disposition": disposition_decision.to_string(),
                "capa": capa_ref,
            });
            println!(
                "{}",
                serde_json::to_string_pretty(&result).unwrap_or_default()
            );
        }
        OutputFormat::Yaml => {
            let result = serde_json::json!({
                "id": ncr.id.to_string(),
                "ncr_status": "closed",
                "disposition": disposition_decision.to_string(),
            });
            println!("{}", serde_yml::to_string(&result).unwrap_or_default());
        }
        _ => {
            println!();
            println!(
                "{} NCR {} closed",
                style("✓").green(),
                style(&display_id).cyan()
            );
            println!(
                "  Disposition: {}",
                style(format!("{:?}", args.disposition)).yellow()
            );
            if let Some(ref capa_id) = capa_ref {
                let capa_display = short_ids
                    .get_short_id(capa_id)
                    .unwrap_or_else(|| capa_id.clone());
                println!("  Linked CAPA: {}", style(&capa_display).cyan());
            }
        }
    }

    Ok(())
}
