//! `tdt capa` command - Corrective/Preventive Action management

use clap::{Subcommand, ValueEnum};
use console::style;
use miette::{IntoDiagnostic, Result};
use std::fs;

use crate::cli::{GlobalOpts, OutputFormat};
use crate::core::identity::{EntityId, EntityPrefix};
use crate::core::project::Project;
use crate::core::shortid::ShortIdIndex;
use crate::core::Config;
use crate::entities::capa::{Capa, CapaStatus, CapaType, SourceType};
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

/// Run a CAPA subcommand
pub fn run(cmd: CapaCommands, global: &GlobalOpts) -> Result<()> {
    match cmd {
        CapaCommands::List(args) => run_list(args, global),
        CapaCommands::New(args) => run_new(args),
        CapaCommands::Show(args) => run_show(args, global),
        CapaCommands::Edit(args) => run_edit(args),
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

    // Load and parse all CAPAs
    let mut capas: Vec<Capa> = Vec::new();

    for entry in fs::read_dir(&capa_dir).into_diagnostic()? {
        let entry = entry.into_diagnostic()?;
        let path = entry.path();

        if path.extension().map_or(false, |e| e == "yaml") {
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
                    .map_or(false, |target| target < today && c.capa_status != CapaStatus::Closed)
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
                        .map_or(false, |d| d.to_lowercase().contains(&search_lower))
                    || c.capa_number
                        .as_ref()
                        .map_or(false, |num| num.to_lowercase().contains(&search_lower))
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
        ListColumn::Status => {
            capas.sort_by(|a, b| format!("{:?}", a.capa_status).cmp(&format!("{:?}", b.capa_status)))
        }
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
    let _ = short_ids.save(&project);

    // Output based on format
    let format = match global.format {
        OutputFormat::Auto => OutputFormat::Tsv,
        f => f,
    };

    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&capas).into_diagnostic()?;
            println!("{}", json);
        }
        OutputFormat::Yaml => {
            let yaml = serde_yml::to_string(&capas).into_diagnostic()?;
            print!("{}", yaml);
        }
        OutputFormat::Csv => {
            println!("short_id,id,title,type,actions,capa_status");
            for capa in &capas {
                let short_id = short_ids.get_short_id(&capa.id.to_string()).unwrap_or_default();
                println!(
                    "{},{},{},{},{},{}",
                    short_id,
                    capa.id,
                    escape_csv(&capa.title),
                    capa.capa_type,
                    capa.actions.len(),
                    capa.capa_status
                );
            }
        }
        OutputFormat::Tsv => {
            // Build header dynamically based on columns
            let mut headers = vec![style("SHORT").bold().dim().to_string()];
            let mut widths = vec![8];

            for col in &args.columns {
                let (header, width) = match col {
                    ListColumn::Id => ("ID", 17),
                    ListColumn::Title => ("TITLE", 30),
                    ListColumn::CapaType => ("TYPE", 12),
                    ListColumn::Status => ("STATUS", 14),
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
            println!("{}", "-".repeat(widths.iter().sum::<usize>() + widths.len() - 1));

            // Print rows
            for capa in &capas {
                let short_id = short_ids.get_short_id(&capa.id.to_string()).unwrap_or_default();

                // Print SHORT column
                print!("{:<8}", style(&short_id).cyan());

                // Print selected columns
                for col in &args.columns {
                    print!(" ");
                    match col {
                        ListColumn::Id => {
                            let id_display = format_short_id(&capa.id);
                            print!("{:<17}", id_display);
                        }
                        ListColumn::Title => {
                            let title_truncated = truncate_str(&capa.title, 28);
                            print!("{:<30}", title_truncated);
                        }
                        ListColumn::CapaType => {
                            print!("{:<12}", capa.capa_type);
                        }
                        ListColumn::Status => {
                            // Check if overdue
                            let is_overdue = capa
                                .timeline
                                .as_ref()
                                .and_then(|t| t.target_date)
                                .map_or(false, |target| target < today && capa.capa_status != CapaStatus::Closed);

                            let status_styled = if is_overdue {
                                style(format!("{} !", capa.capa_status)).red().bold()
                            } else {
                                style(capa.capa_status.to_string()).white()
                            };
                            print!("{:<14}", status_styled);
                        }
                        ListColumn::Author => {
                            let author_truncated = truncate_str(&capa.author, 18);
                            print!("{:<20}", author_truncated);
                        }
                        ListColumn::Created => {
                            let created_str = capa.created.format("%Y-%m-%d %H:%M").to_string();
                            print!("{:<20}", created_str);
                        }
                    }
                }
                println!();
            }

            println!();
            println!(
                "{} CAPA(s) found. Use {} to reference by short ID.",
                style(capas.len()).cyan(),
                style("CAPA@N").cyan()
            );
        }
        OutputFormat::Id => {
            for capa in &capas {
                println!("{}", capa.id);
            }
        }
        OutputFormat::Md => {
            println!("| Short | ID | Title | Type | Actions | Status |");
            println!("|---|---|---|---|---|---|");
            for capa in &capas {
                let short_id = short_ids.get_short_id(&capa.id.to_string()).unwrap_or_default();
                println!(
                    "| {} | {} | {} | {} | {} | {} |",
                    short_id,
                    format_short_id(&capa.id),
                    capa.title,
                    capa.capa_type,
                    capa.actions.len(),
                    capa.capa_status
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
    let capa_type: String;
    let source_type: String;

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
    } else {
        title = args.title.unwrap_or_else(|| "New CAPA".to_string());
        capa_type = args.r#type;
        source_type = args.source;
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

    let yaml_content = generator
        .generate_capa(&ctx)
        .map_err(|e| miette::miette!("{}", e))?;

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
    let _ = short_ids.save(&project);

    println!(
        "{} Created CAPA {}",
        style("✓").green(),
        style(short_id.unwrap_or_else(|| format_short_id(&id))).cyan()
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

            if path.extension().map_or(false, |e| e == "yaml") {
                let filename = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                if filename.contains(&resolved_id) || filename.starts_with(&resolved_id) {
                    found_path = Some(path);
                    break;
                }
            }
        }
    }

    let path = found_path.ok_or_else(|| miette::miette!("No CAPA found matching '{}'", args.id))?;

    // Read and display
    let content = fs::read_to_string(&path).into_diagnostic()?;

    let format = match global.format {
        OutputFormat::Auto => OutputFormat::Yaml,
        f => f,
    };

    match format {
        OutputFormat::Yaml => {
            print!("{}", content);
        }
        OutputFormat::Json => {
            let capa: Capa = serde_yml::from_str(&content).into_diagnostic()?;
            let json = serde_json::to_string_pretty(&capa).into_diagnostic()?;
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

    // Find the CAPA file
    let capa_dir = project.root().join("manufacturing/capas");
    let mut found_path = None;

    if capa_dir.exists() {
        for entry in fs::read_dir(&capa_dir).into_diagnostic()? {
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

    let path = found_path.ok_or_else(|| miette::miette!("No CAPA found matching '{}'", args.id))?;

    println!(
        "Opening {} in {}...",
        style(path.display()).cyan(),
        style(config.editor()).yellow()
    );

    config.run_editor(&path).into_diagnostic()?;

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
