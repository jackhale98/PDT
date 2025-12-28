//! `tdt tol` command - Stackup/tolerance analysis management

use chrono::{Duration, Utc};
use clap::{Subcommand, ValueEnum};
use console::style;
use miette::{IntoDiagnostic, Result};
use std::fs;

use crate::cli::filters::StatusFilter;
use crate::cli::helpers::{format_short_id, smart_round, truncate_str};
use crate::cli::table::{CellValue, ColumnDef, TableConfig, TableFormatter, TableRow};
use crate::cli::{GlobalOpts, OutputFormat};
use crate::core::cache::EntityCache;
use crate::core::entity::Entity;
use crate::core::identity::{EntityId, EntityPrefix};
use crate::core::project::Project;
use crate::core::shortid::ShortIdIndex;
use crate::core::Config;
use crate::entities::feature::Feature;
use crate::entities::stackup::{Contributor, Direction, Disposition, FeatureRef, Stackup};
use crate::schema::template::{TemplateContext, TemplateGenerator};
use crate::schema::wizard::SchemaWizard;

#[derive(Subcommand, Debug)]
pub enum TolCommands {
    /// List stackups with filtering
    List(ListArgs),

    /// Create a new stackup
    New(NewArgs),

    /// Show a stackup's details (includes analysis results)
    Show(ShowArgs),

    /// Edit a stackup in your editor
    Edit(EditArgs),

    /// Delete a stackup
    Delete(DeleteArgs),

    /// Archive a stackup (soft delete)
    Archive(ArchiveArgs),

    /// Run/recalculate analysis (worst-case, RSS, Monte Carlo)
    Analyze(AnalyzeArgs),

    /// Add feature(s) as contributors to a stackup
    /// Use +FEAT@N for positive direction, ~FEAT@N for negative
    Add(AddArgs),

    /// Remove contributor(s) from a stackup by feature ID
    #[command(name = "rm")]
    Remove(RemoveArgs),
}

/// Disposition filter
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum DispositionFilter {
    UnderReview,
    Approved,
    Rejected,
    All,
}

/// Analysis result filter
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ResultFilter {
    Pass,
    Marginal,
    Fail,
    All,
}

/// List column selection
#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
pub enum ListColumn {
    Id,
    Title,
    Result,
    Cpk,
    Yield,
    Disposition,
    Status,
    Critical,
    Author,
    Created,
}

impl std::fmt::Display for ListColumn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ListColumn::Id => write!(f, "id"),
            ListColumn::Title => write!(f, "title"),
            ListColumn::Result => write!(f, "result"),
            ListColumn::Cpk => write!(f, "cpk"),
            ListColumn::Yield => write!(f, "yield"),
            ListColumn::Disposition => write!(f, "disposition"),
            ListColumn::Status => write!(f, "status"),
            ListColumn::Critical => write!(f, "critical"),
            ListColumn::Author => write!(f, "author"),
            ListColumn::Created => write!(f, "created"),
        }
    }
}

/// Column definitions for stackup list output
const TOL_COLUMNS: &[ColumnDef] = &[
    ColumnDef::new("id", "ID", 30),
    ColumnDef::new("title", "TITLE", 30),
    ColumnDef::new("result", "RESULT", 10),
    ColumnDef::new("cpk", "CPK", 8),
    ColumnDef::new("yield", "YIELD", 10),
    ColumnDef::new("disposition", "DISPOSITION", 14),
    ColumnDef::new("status", "STATUS", 10),
    ColumnDef::new("critical", "CRIT", 5),
    ColumnDef::new("author", "AUTHOR", 20),
    ColumnDef::new("created", "CREATED", 16),
];

#[derive(clap::Args, Debug)]
pub struct ListArgs {
    /// Filter by disposition
    #[arg(long, short = 'd', default_value = "all")]
    pub disposition: DispositionFilter,

    /// Filter by status
    #[arg(long, short = 's', default_value = "all")]
    pub status: StatusFilter,

    /// Filter by worst-case result
    #[arg(long, short = 'r')]
    pub result: Option<ResultFilter>,

    /// Search in title
    #[arg(long)]
    pub search: Option<String>,

    /// Show only critical stackups
    #[arg(long)]
    pub critical: bool,

    /// Filter by author
    #[arg(long, short = 'a')]
    pub author: Option<String>,

    /// Show only stackups created in the last N days
    #[arg(long)]
    pub recent: Option<u32>,

    /// Columns to display
    #[arg(long, value_delimiter = ',', default_values_t = vec![ListColumn::Title, ListColumn::Result, ListColumn::Cpk, ListColumn::Yield, ListColumn::Status])]
    pub columns: Vec<ListColumn>,

    /// Show full ID column (hidden by default since SHORT is always shown)
    #[arg(long)]
    pub show_id: bool,

    /// Sort by column
    #[arg(long)]
    pub sort: Option<ListColumn>,

    /// Reverse sort order
    #[arg(long)]
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
    /// Stackup title
    #[arg(long, short = 't')]
    pub title: Option<String>,

    /// Target dimension name
    #[arg(long)]
    pub target_name: Option<String>,

    /// Target nominal value
    #[arg(long)]
    pub target_nominal: Option<f64>,

    /// Target upper specification limit
    #[arg(long)]
    pub target_upper: Option<f64>,

    /// Target lower specification limit
    #[arg(long)]
    pub target_lower: Option<f64>,

    /// Mark as critical dimension
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
    /// Stackup ID or short ID (TOL@N)
    pub id: String,
}

#[derive(clap::Args, Debug)]
pub struct EditArgs {
    /// Stackup ID or short ID (TOL@N)
    pub id: String,
}

#[derive(clap::Args, Debug)]
pub struct DeleteArgs {
    /// Stackup ID or short ID (TOL@N)
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
    /// Stackup ID or short ID (TOL@N)
    pub id: String,

    /// Force archive even if other entities reference this one
    #[arg(long)]
    pub force: bool,

    /// Suppress output
    #[arg(long, short = 'q')]
    pub quiet: bool,
}

/// Directories where stackups are stored
const STACKUP_DIRS: &[&str] = &["tolerances/stackups"];

/// Entity configuration for stackup commands
const ENTITY_CONFIG: crate::cli::EntityConfig = crate::cli::EntityConfig {
    prefix: EntityPrefix::Tol,
    dirs: STACKUP_DIRS,
    name: "stackup",
    name_plural: "stackups",
};

#[derive(clap::Args, Debug)]
pub struct AnalyzeArgs {
    /// Stackup ID or short ID (TOL@N) - omit when using --all
    pub id: Option<String>,

    /// Analyze all stackups in the project
    #[arg(long, short = 'A')]
    pub all: bool,

    /// Number of Monte Carlo iterations (default: 10000)
    #[arg(long, default_value = "10000")]
    pub iterations: u32,

    /// Show detailed results after analysis
    #[arg(long, short = 'v')]
    pub verbose: bool,

    /// Show ASCII histogram of Monte Carlo distribution
    #[arg(long, short = 'H')]
    pub histogram: bool,

    /// Output raw Monte Carlo samples as CSV (for external analysis)
    #[arg(long)]
    pub csv: bool,

    /// Number of histogram bins (default: 40)
    #[arg(long, default_value = "40")]
    pub bins: usize,

    /// Only show what would be analyzed (don't run analysis)
    #[arg(long)]
    pub dry_run: bool,

    /// Debug mode - trace calculation steps
    #[arg(long)]
    pub debug: bool,
}

#[derive(clap::Args, Debug)]
pub struct AddArgs {
    /// Stackup ID or short ID (TOL@N)
    pub stackup: String,

    /// Features to add with direction prefix: +FEAT@1 (positive) or ~FEAT@2 (negative)
    /// Use ~ instead of - for negative to avoid conflicts with CLI flags
    /// Examples: +FEAT@1 ~FEAT@2 +FEAT@3
    #[arg(required = true)]
    pub features: Vec<String>,

    /// Dimension name to use from feature (default: first dimension)
    #[arg(long, short = 'd')]
    pub dimension: Option<String>,

    /// Run analysis after adding
    #[arg(long, short = 'a')]
    pub analyze: bool,
}

#[derive(clap::Args, Debug)]
pub struct RemoveArgs {
    /// Stackup ID or short ID (TOL@N)
    pub stackup: String,

    /// Features to remove (by feature ID or short ID)
    /// Examples: FEAT@1 FEAT@2
    #[arg(required = true)]
    pub features: Vec<String>,
}

/// Run a tol subcommand
pub fn run(cmd: TolCommands, global: &GlobalOpts) -> Result<()> {
    match cmd {
        TolCommands::List(args) => run_list(args, global),
        TolCommands::New(args) => run_new(args, global),
        TolCommands::Show(args) => run_show(args, global),
        TolCommands::Edit(args) => run_edit(args),
        TolCommands::Delete(args) => run_delete(args),
        TolCommands::Archive(args) => run_archive(args),
        TolCommands::Analyze(args) => run_analyze(args),
        TolCommands::Add(args) => run_add(args),
        TolCommands::Remove(args) => run_remove(args),
    }
}

fn run_list(args: ListArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let tol_dir = project.root().join("tolerances/stackups");

    if !tol_dir.exists() {
        if args.count {
            println!("0");
        } else {
            println!("No stackups found.");
        }
        return Ok(());
    }

    // Load and parse all stackups
    let mut stackups: Vec<Stackup> = Vec::new();

    for entry in fs::read_dir(&tol_dir).into_diagnostic()? {
        let entry = entry.into_diagnostic()?;
        let path = entry.path();

        if path.extension().is_some_and(|e| e == "yaml") {
            let content = fs::read_to_string(&path).into_diagnostic()?;
            if let Ok(stackup) = serde_yml::from_str::<Stackup>(&content) {
                stackups.push(stackup);
            }
        }
    }

    // Apply filters
    let stackups: Vec<Stackup> = stackups
        .into_iter()
        .filter(|s| match args.disposition {
            DispositionFilter::UnderReview => s.disposition == Disposition::UnderReview,
            DispositionFilter::Approved => s.disposition == Disposition::Approved,
            DispositionFilter::Rejected => s.disposition == Disposition::Rejected,
            DispositionFilter::All => true,
        })
        .filter(|s| match args.status {
            StatusFilter::Draft => s.status == crate::core::entity::Status::Draft,
            StatusFilter::Review => s.status == crate::core::entity::Status::Review,
            StatusFilter::Approved => s.status == crate::core::entity::Status::Approved,
            StatusFilter::Released => s.status == crate::core::entity::Status::Released,
            StatusFilter::Obsolete => s.status == crate::core::entity::Status::Obsolete,
            StatusFilter::Active => s.status != crate::core::entity::Status::Obsolete,
            StatusFilter::All => true,
        })
        .filter(|s| {
            if let Some(ref result_filter) = args.result {
                if let Some(ref wc) = s.analysis_results.worst_case {
                    match result_filter {
                        ResultFilter::Pass => {
                            wc.result == crate::entities::stackup::AnalysisResult::Pass
                        }
                        ResultFilter::Marginal => {
                            wc.result == crate::entities::stackup::AnalysisResult::Marginal
                        }
                        ResultFilter::Fail => {
                            wc.result == crate::entities::stackup::AnalysisResult::Fail
                        }
                        ResultFilter::All => true,
                    }
                } else {
                    false // No analysis yet
                }
            } else {
                true
            }
        })
        .filter(|s| {
            if let Some(ref search) = args.search {
                let search_lower = search.to_lowercase();
                s.title.to_lowercase().contains(&search_lower)
                    || s.description
                        .as_ref()
                        .is_some_and(|d| d.to_lowercase().contains(&search_lower))
            } else {
                true
            }
        })
        .filter(|s| {
            if args.critical {
                s.target.critical
            } else {
                true
            }
        })
        .filter(|s| {
            if let Some(ref author_filter) = args.author {
                let author_lower = author_filter.to_lowercase();
                s.author.to_lowercase().contains(&author_lower)
            } else {
                true
            }
        })
        .filter(|s| {
            if let Some(days) = args.recent {
                let cutoff = Utc::now() - Duration::days(days as i64);
                s.created >= cutoff
            } else {
                true
            }
        })
        .collect();

    // Apply sorting
    let mut stackups = stackups;
    if let Some(sort_col) = args.sort {
        stackups.sort_by(|a, b| {
            let cmp = match sort_col {
                ListColumn::Id => a.id.to_string().cmp(&b.id.to_string()),
                ListColumn::Title => a.title.cmp(&b.title),
                ListColumn::Disposition => {
                    format!("{}", a.disposition).cmp(&format!("{}", b.disposition))
                }
                ListColumn::Status => a.status().cmp(b.status()),
                ListColumn::Result => {
                    let a_result = a
                        .analysis_results
                        .worst_case
                        .as_ref()
                        .map(|wc| format!("{}", wc.result))
                        .unwrap_or_else(|| "zzz".to_string()); // Sort missing results last
                    let b_result = b
                        .analysis_results
                        .worst_case
                        .as_ref()
                        .map(|wc| format!("{}", wc.result))
                        .unwrap_or_else(|| "zzz".to_string());
                    a_result.cmp(&b_result)
                }
                ListColumn::Cpk => {
                    let a_cpk = a
                        .analysis_results
                        .rss
                        .as_ref()
                        .map(|r| r.cpk)
                        .unwrap_or(-999.0);
                    let b_cpk = b
                        .analysis_results
                        .rss
                        .as_ref()
                        .map(|r| r.cpk)
                        .unwrap_or(-999.0);
                    b_cpk
                        .partial_cmp(&a_cpk)
                        .unwrap_or(std::cmp::Ordering::Equal) // Higher Cpk first
                }
                ListColumn::Yield => {
                    let a_yield = a
                        .analysis_results
                        .monte_carlo
                        .as_ref()
                        .map(|m| m.yield_percent)
                        .unwrap_or(-999.0);
                    let b_yield = b
                        .analysis_results
                        .monte_carlo
                        .as_ref()
                        .map(|m| m.yield_percent)
                        .unwrap_or(-999.0);
                    b_yield
                        .partial_cmp(&a_yield)
                        .unwrap_or(std::cmp::Ordering::Equal) // Higher yield first
                }
                ListColumn::Critical => b.target.critical.cmp(&a.target.critical), // Critical first
                ListColumn::Author => a.author.cmp(&b.author),
                ListColumn::Created => a.created.cmp(&b.created),
            };
            if args.reverse {
                cmp.reverse()
            } else {
                cmp
            }
        });
    }

    // Apply limit
    if let Some(limit) = args.limit {
        stackups.truncate(limit);
    }

    // Count only
    if args.count {
        println!("{}", stackups.len());
        return Ok(());
    }

    // No results
    if stackups.is_empty() {
        println!("No stackups found.");
        return Ok(());
    }

    // Update short ID index
    let mut short_ids = ShortIdIndex::load(&project);
    short_ids.ensure_all(stackups.iter().map(|s| s.id.to_string()));
    super::utils::save_short_ids(&mut short_ids, &project);

    // Output based on format
    let format = match global.output {
        OutputFormat::Auto => OutputFormat::Tsv,
        f => f,
    };

    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&stackups).into_diagnostic()?;
            println!("{}", json);
        }
        OutputFormat::Yaml => {
            let yaml = serde_yml::to_string(&stackups).into_diagnostic()?;
            print!("{}", yaml);
        }
        OutputFormat::Csv | OutputFormat::Tsv | OutputFormat::Md | OutputFormat::Table | OutputFormat::Dot | OutputFormat::Tree => {
            let mut columns: Vec<&str> = args
                .columns
                .iter()
                .map(|c| c.to_string().leak() as &str)
                .collect();
            if args.show_id && !columns.contains(&"id") {
                columns.insert(0, "id");
            }
            let rows: Vec<TableRow> = stackups
                .iter()
                .map(|s| stackup_to_row(s, &short_ids))
                .collect();

            let config = TableConfig {
                wrap_width: args.wrap,
                show_summary: true,
            };
            let formatter = TableFormatter::new(TOL_COLUMNS, "stackup", "TOL").with_config(config);
            formatter.output(rows, format, &columns);
        }
        OutputFormat::Id | OutputFormat::ShortId => {
            for s in &stackups {
                if format == OutputFormat::ShortId {
                    let short_id = short_ids
                        .get_short_id(&s.id.to_string())
                        .unwrap_or_default();
                    println!("{}", short_id);
                } else {
                    println!("{}", s.id);
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
    let target_name: String;
    let target_nominal: f64;
    let target_upper: f64;
    let target_lower: f64;
    let description: Option<String>;

    if args.interactive {
        let wizard = SchemaWizard::new();
        let result = wizard.run(EntityPrefix::Tol)?;

        title = result
            .get_string("title")
            .map(String::from)
            .unwrap_or_else(|| "New Stackup".to_string());
        description = result.get_string("description").map(String::from);

        // Wizard doesn't support nested object fields like "target"
        // So we prompt for these separately using dialoguer
        use dialoguer::{theme::ColorfulTheme, Input};
        let theme = ColorfulTheme::default();

        println!();
        println!("{} Target specification", console::style("◆").cyan());

        target_name = Input::with_theme(&theme)
            .with_prompt("Target name")
            .default("Target".to_string())
            .interact_text()
            .into_diagnostic()?;

        target_nominal = Input::with_theme(&theme)
            .with_prompt("Target nominal value")
            .default(0.0)
            .interact_text()
            .into_diagnostic()?;

        target_upper = Input::with_theme(&theme)
            .with_prompt("Upper specification limit (USL)")
            .default(0.0)
            .interact_text()
            .into_diagnostic()?;

        target_lower = Input::with_theme(&theme)
            .with_prompt("Lower specification limit (LSL)")
            .default(0.0)
            .interact_text()
            .into_diagnostic()?;
    } else {
        title = args.title.unwrap_or_else(|| "New Stackup".to_string());
        target_name = args.target_name.unwrap_or_else(|| "Target".to_string());
        target_nominal = args.target_nominal.unwrap_or(0.0);
        target_upper = args.target_upper.unwrap_or(0.0);
        target_lower = args.target_lower.unwrap_or(0.0);
        description = None;
    }

    // Generate ID
    let id = EntityId::new(EntityPrefix::Tol);

    // Generate template
    let generator = TemplateGenerator::new().map_err(|e| miette::miette!("{}", e))?;
    let ctx = TemplateContext::new(id.clone(), config.author())
        .with_title(&title)
        .with_target(&target_name, target_nominal, target_upper, target_lower);

    let mut yaml_content = generator
        .generate_stackup(&ctx)
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
                    "description: |\n  # Detailed description of this tolerance stackup\n  # Include the tolerance chain being analyzed",
                    &format!("description: |\n{}", indented),
                );
            }
        }
    }

    // Write file
    let output_dir = project.root().join("tolerances/stackups");
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
        EntityPrefix::Tol,
        &args.link,
        &short_ids,
    );

    // Output based on format flag
    match global.output {
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
                "{} Created stackup {}",
                style("✓").green(),
                style(short_id.clone().unwrap_or_else(|| format_short_id(&id))).cyan()
            );
            println!("   {}", style(file_path.display()).dim());
            println!(
                "   Target: {} = {:.3} (LSL: {:.3}, USL: {:.3})",
                style(&target_name).yellow(),
                target_nominal,
                target_lower,
                target_upper
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

    // Find the stackup file
    let tol_dir = project.root().join("tolerances/stackups");
    let mut found_path = None;

    if tol_dir.exists() {
        for entry in fs::read_dir(&tol_dir).into_diagnostic()? {
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
        found_path.ok_or_else(|| miette::miette!("No stackup found matching '{}'", args.id))?;

    // Read and parse stackup
    let content = fs::read_to_string(&path).into_diagnostic()?;
    let stackup: Stackup = serde_yml::from_str(&content).into_diagnostic()?;

    match global.output {
        OutputFormat::Yaml => {
            print!("{}", content);
        }
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&stackup).into_diagnostic()?;
            println!("{}", json);
        }
        OutputFormat::Id | OutputFormat::ShortId => {
            if global.output == OutputFormat::ShortId {
                let short_ids = ShortIdIndex::load(&project);
                let short_id = short_ids
                    .get_short_id(&stackup.id.to_string())
                    .unwrap_or_default();
                println!("{}", short_id);
            } else {
                println!("{}", stackup.id);
            }
        }
        _ => {
            // Pretty format (default)
            println!("{}", style("─".repeat(60)).dim());
            println!(
                "{}: {}",
                style("ID").bold(),
                style(&stackup.id.to_string()).cyan()
            );
            println!(
                "{}: {}",
                style("Title").bold(),
                style(&stackup.title).yellow()
            );
            println!("{}: {}", style("Status").bold(), stackup.status);
            println!("{}: {}", style("Disposition").bold(), stackup.disposition);
            println!("{}", style("─".repeat(60)).dim());

            // Target
            println!();
            println!("{}", style("Target:").bold());
            println!(
                "  {}: {} {} (+{} / -{})",
                style(&stackup.target.name).cyan(),
                stackup.target.nominal,
                stackup.target.units,
                stackup.target.upper_limit - stackup.target.nominal,
                stackup.target.nominal - stackup.target.lower_limit
            );

            // Contributors
            if !stackup.contributors.is_empty() {
                // Load cache for component lookups
                let cache = EntityCache::open(&project).ok();
                let component_info: std::collections::HashMap<String, (String, String)> =
                    if let Some(ref c) = cache {
                        c.list_components(None, None, None, None, None, None)
                            .into_iter()
                            .map(|cmp| {
                                let pn = cmp.part_number.unwrap_or_default();
                                (cmp.id, (pn, cmp.title))
                            })
                            .collect()
                    } else {
                        std::collections::HashMap::new()
                    };

                println!();
                println!(
                    "{} ({}):",
                    style("Contributors").bold(),
                    stackup.contributors.len()
                );
                for c in &stackup.contributors {
                    let dir = if c.direction == crate::entities::stackup::Direction::Positive {
                        "+"
                    } else {
                        "-"
                    };
                    // Use tolerance as reference for precision
                    let ref_precision = c.plus_tol.max(c.minus_tol).max(0.001);
                    let plus_str = smart_round(c.plus_tol, ref_precision);
                    let minus_str = smart_round(c.minus_tol, ref_precision);
                    println!(
                        "  {} {} {} +{}/-{}",
                        dir,
                        style(&c.name).cyan(),
                        c.nominal,
                        plus_str,
                        minus_str
                    );

                    // Show component info if available from feature reference
                    if let Some(ref feat_ref) = c.feature {
                        if let Some(ref cmp_id) = feat_ref.component_id {
                            let cmp_short = short_ids
                                .get_short_id(cmp_id)
                                .unwrap_or_else(|| cmp_id.clone());
                            let display = if let Some((pn, title)) = component_info.get(cmp_id) {
                                if !pn.is_empty() && !title.is_empty() {
                                    format!("{} ({}) {}", cmp_short, pn, title)
                                } else if !pn.is_empty() {
                                    format!("{} ({})", cmp_short, pn)
                                } else if !title.is_empty() {
                                    format!("{} ({})", cmp_short, title)
                                } else {
                                    cmp_short
                                }
                            } else if let Some(ref cmp_name) = feat_ref.component_name {
                                format!("{} ({})", cmp_short, cmp_name)
                            } else {
                                cmp_short
                            };
                            println!("      Component: {}", style(&display).dim());
                        }
                    }
                }
            }

            // Analysis Results
            let results = &stackup.analysis_results;
            if results.worst_case.is_some()
                || results.rss.is_some()
                || results.monte_carlo.is_some()
            {
                // Use target tolerance band as reference for precision
                let ref_precision = (stackup.target.upper_limit - stackup.target.lower_limit)
                    .abs()
                    .max(0.001);

                println!();
                println!("{}", style("Analysis Results:").bold());
                if let Some(ref wc) = results.worst_case {
                    let result_color = match wc.result {
                        crate::entities::stackup::AnalysisResult::Pass => style("PASS").green(),
                        crate::entities::stackup::AnalysisResult::Fail => style("FAIL").red(),
                        crate::entities::stackup::AnalysisResult::Marginal => {
                            style("MARGINAL").yellow()
                        }
                    };
                    let margin_rounded = smart_round(wc.margin, ref_precision);
                    println!(
                        "  Worst Case: {} (margin: {})",
                        result_color, margin_rounded
                    );
                }
                if let Some(ref rss) = results.rss {
                    println!("  RSS: Cpk={:.2}, Yield={:.1}%", rss.cpk, rss.yield_percent);
                }
                if let Some(ref mc) = results.monte_carlo {
                    println!(
                        "  Monte Carlo: {} iter, Yield={:.1}%",
                        mc.iterations, mc.yield_percent
                    );
                }
            }

            // Tags
            if !stackup.tags.is_empty() {
                println!();
                println!("{}: {}", style("Tags").bold(), stackup.tags.join(", "));
            }

            // Footer
            println!("{}", style("─".repeat(60)).dim());
            println!(
                "{}: {} | {}: {} | {}: {}",
                style("Author").dim(),
                stackup.author,
                style("Created").dim(),
                stackup.created.format("%Y-%m-%d %H:%M"),
                style("Revision").dim(),
                stackup.entity_revision
            );
        }
    }

    Ok(())
}

fn run_edit(args: EditArgs) -> Result<()> {
    crate::cli::entity_cmd::run_edit_generic(&args.id, &ENTITY_CONFIG)
}

fn run_delete(args: DeleteArgs) -> Result<()> {
    crate::cli::commands::utils::run_delete(&args.id, STACKUP_DIRS, args.force, false, args.quiet)
}

fn run_archive(args: ArchiveArgs) -> Result<()> {
    crate::cli::commands::utils::run_delete(&args.id, STACKUP_DIRS, args.force, true, args.quiet)
}

fn run_analyze(args: AnalyzeArgs) -> Result<()> {
    // Dispatch to --all or single mode
    if args.all {
        return run_analyze_all(&args);
    }

    let id = args.id.as_ref().ok_or_else(|| {
        miette::miette!("Stackup ID required. Use --all to analyze all stackups.")
    })?;

    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;

    // Resolve short ID if needed
    let short_ids = ShortIdIndex::load(&project);
    let resolved_id = short_ids.resolve(id).unwrap_or_else(|| id.clone());

    // Find and load the stackup
    let tol_dir = project.root().join("tolerances/stackups");
    let mut found_path = None;

    if tol_dir.exists() {
        for entry in fs::read_dir(&tol_dir).into_diagnostic()? {
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

    let path = found_path.ok_or_else(|| miette::miette!("No stackup found matching '{}'", id))?;

    // Load stackup
    let content = fs::read_to_string(&path).into_diagnostic()?;
    let mut stackup: Stackup = serde_yml::from_str(&content).into_diagnostic()?;

    if stackup.contributors.is_empty() {
        return Err(miette::miette!(
            "Stackup has no contributors. Add contributors before running analysis."
        ));
    }

    // Debug mode - trace the RSS calculation step by step
    if args.debug {
        use crate::entities::stackup::Direction;
        println!(
            "{}",
            style("=== DEBUG: RSS Calculation Trace ===")
                .yellow()
                .bold()
        );
        let mut debug_mean = 0.0;
        for (i, contrib) in stackup.contributors.iter().enumerate() {
            let mean_offset = (contrib.plus_tol - contrib.minus_tol) / 2.0;
            let process_mean = contrib.nominal + mean_offset;
            let dir_str = match contrib.direction {
                Direction::Positive => "Positive",
                Direction::Negative => "Negative",
            };
            let contribution = match contrib.direction {
                Direction::Positive => process_mean,
                Direction::Negative => -process_mean,
            };
            debug_mean += contribution;
            println!(
                "  [{}] {} | dir={} | nominal={:.4} | offset={:.4} | process_mean={:.4} | contribution={:+.4} | running_mean={:.4}",
                i + 1,
                contrib.name,
                dir_str,
                contrib.nominal,
                mean_offset,
                process_mean,
                contribution,
                debug_mean
            );
        }
        println!(
            "  {}",
            style(format!("Final RSS mean: {:.4}", debug_mean)).cyan()
        );
        println!("{}", style("=== END DEBUG ===").yellow().bold());
        println!();
    }

    // Run analysis - use with_samples if we need histogram or CSV
    let mc_samples = if args.histogram || args.csv {
        let (mc_result, samples) = stackup.calculate_monte_carlo_with_samples(args.iterations);
        stackup.analysis_results.monte_carlo = Some(mc_result);
        Some(samples)
    } else {
        stackup.analysis_results.monte_carlo = Some(stackup.calculate_monte_carlo(args.iterations));
        None
    };

    stackup.analysis_results.worst_case = Some(stackup.calculate_worst_case());
    stackup.analysis_results.rss = Some(stackup.calculate_rss());

    // CSV output mode - just output samples and exit
    if args.csv {
        if let Some(samples) = mc_samples {
            println!("sample,value,in_spec");
            for (i, value) in samples.iter().enumerate() {
                let in_spec =
                    *value >= stackup.target.lower_limit && *value <= stackup.target.upper_limit;
                println!("{},{:.6},{}", i + 1, value, if in_spec { 1 } else { 0 });
            }
        }
        return Ok(());
    }

    // Write back
    let yaml_content = serde_yml::to_string(&stackup).into_diagnostic()?;
    fs::write(&path, &yaml_content).into_diagnostic()?;

    println!(
        "{} Analyzing stackup {} with {} contributors...",
        style("⚙").cyan(),
        style(id).cyan(),
        stackup.contributors.len()
    );

    println!(
        "{} Analysis complete for stackup {}",
        style("✓").green(),
        style(id).cyan()
    );

    // Use target tolerance band as reference for precision
    let ref_precision = (stackup.target.upper_limit - stackup.target.lower_limit)
        .abs()
        .max(0.001);

    // Show results summary
    println!();
    println!(
        "   Target: {} = {} (LSL: {}, USL: {})",
        style(&stackup.target.name).yellow(),
        smart_round(stackup.target.nominal, ref_precision),
        smart_round(stackup.target.lower_limit, ref_precision),
        smart_round(stackup.target.upper_limit, ref_precision)
    );

    if let Some(ref wc) = stackup.analysis_results.worst_case {
        let result_style = match wc.result {
            crate::entities::stackup::AnalysisResult::Pass => {
                style(format!("{}", wc.result)).green()
            }
            crate::entities::stackup::AnalysisResult::Marginal => {
                style(format!("{}", wc.result)).yellow()
            }
            crate::entities::stackup::AnalysisResult::Fail => style(format!("{}", wc.result)).red(),
        };

        println!();
        println!("   {} Analysis:", style("Worst-Case").bold());
        println!(
            "     Range: {} to {}",
            smart_round(wc.min, ref_precision),
            smart_round(wc.max, ref_precision)
        );
        println!("     Margin: {}", smart_round(wc.margin, ref_precision));
        println!("     Result: {}", result_style);
    }

    if let Some(ref rss) = stackup.analysis_results.rss {
        println!();
        println!("   {} Analysis:", style("RSS (Statistical)").bold());
        println!("     Mean: {}", smart_round(rss.mean, ref_precision));
        println!("     ±3σ: {}", smart_round(rss.sigma_3, ref_precision));
        println!("     Margin: {}", smart_round(rss.margin, ref_precision));
        println!("     Cpk: {:.2}", rss.cpk);
        println!("     Yield: {:.2}%", rss.yield_percent);
    }

    if let Some(ref mc) = stackup.analysis_results.monte_carlo {
        println!();
        println!(
            "   {} ({} iterations):",
            style("Monte Carlo").bold(),
            mc.iterations
        );
        println!("     Mean: {}", smart_round(mc.mean, ref_precision));
        println!("     Std Dev: {}", smart_round(mc.std_dev, ref_precision));
        println!(
            "     Range: {} to {}",
            smart_round(mc.min, ref_precision),
            smart_round(mc.max, ref_precision)
        );
        println!(
            "     95% CI: {} to {}",
            smart_round(mc.percentile_2_5, ref_precision),
            smart_round(mc.percentile_97_5, ref_precision)
        );
        println!("     Yield: {:.2}%", mc.yield_percent);
    }

    // Show histogram if requested
    if args.histogram {
        if let Some(samples) = mc_samples {
            println!();
            print_histogram(
                &samples,
                args.bins,
                stackup.target.lower_limit,
                stackup.target.upper_limit,
            );
        }
    }

    Ok(())
}

/// Print an ASCII histogram of the Monte Carlo samples
fn print_histogram(samples: &[f64], bins: usize, lsl: f64, usl: f64) {
    if samples.is_empty() {
        return;
    }

    let min = samples.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = samples.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

    // Extend range slightly to include spec limits in view
    let range_min = min.min(lsl - (usl - lsl) * 0.1);
    let range_max = max.max(usl + (usl - lsl) * 0.1);
    let range = range_max - range_min;

    if range <= 0.0 {
        return;
    }

    let bin_width = range / bins as f64;

    // Count samples in each bin
    let mut counts: Vec<usize> = vec![0; bins];
    for &sample in samples {
        let bin = ((sample - range_min) / bin_width) as usize;
        let bin = bin.min(bins - 1);
        counts[bin] += 1;
    }

    let max_count = *counts.iter().max().unwrap_or(&1);
    let bar_max_width = 50;

    println!(
        "   {} ({} samples, {} bins):",
        style("Distribution Histogram").bold(),
        samples.len(),
        bins
    );
    println!();

    // Find which bins contain LSL and USL
    let lsl_bin = ((lsl - range_min) / bin_width) as usize;
    let usl_bin = ((usl - range_min) / bin_width) as usize;

    // Print histogram rows
    for (i, &count) in counts.iter().enumerate() {
        let bar_width = (count as f64 / max_count as f64 * bar_max_width as f64) as usize;
        let bin_center = range_min + (i as f64 + 0.5) * bin_width;

        // Determine if this bin is within spec
        let in_spec = bin_center >= lsl && bin_center <= usl;

        // Build the bar
        let bar: String = if in_spec {
            "█".repeat(bar_width)
        } else {
            "░".repeat(bar_width)
        };

        // Mark LSL/USL bins
        let marker = if i == lsl_bin && i == usl_bin {
            " ◄LSL/USL"
        } else if i == lsl_bin {
            " ◄LSL"
        } else if i == usl_bin {
            " ◄USL"
        } else {
            ""
        };

        // Color the bar
        let colored_bar = if in_spec {
            style(bar).green()
        } else {
            style(bar).red()
        };

        println!(
            "   {:>8.3} │{:<width$}│ {:>5}{}",
            bin_center,
            colored_bar,
            count,
            style(marker).cyan(),
            width = bar_max_width
        );
    }

    // Print x-axis summary
    println!("   {:>8} └{}┘", "", "─".repeat(bar_max_width));
    println!(
        "   {} LSL={:.3}  USL={:.3}  (█ in-spec, ░ out-of-spec)",
        style("Legend:").dim(),
        lsl,
        usl
    );
}

/// Analyze all stackups in the project
fn run_analyze_all(args: &AnalyzeArgs) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let tol_dir = project.root().join("tolerances/stackups");

    if !tol_dir.exists() {
        println!("No stackups directory found.");
        return Ok(());
    }

    // Load all stackup files
    let mut stackup_paths: Vec<std::path::PathBuf> = Vec::new();
    for entry in fs::read_dir(&tol_dir).into_diagnostic()? {
        let entry = entry.into_diagnostic()?;
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "yaml") {
            stackup_paths.push(path);
        }
    }

    if stackup_paths.is_empty() {
        println!("No stackups found.");
        return Ok(());
    }

    // Sort by filename for consistent ordering
    stackup_paths.sort();

    let short_ids = ShortIdIndex::load(&project);

    let mut analyzed = 0;
    let mut skipped = 0;
    let mut errors = 0;
    let mut results_summary: Vec<(String, String, Option<String>, Option<f64>, Option<f64>)> =
        Vec::new();

    println!(
        "{} Analyzing {} stackup(s) with {} Monte Carlo iterations...\n",
        style("⚙").cyan(),
        stackup_paths.len(),
        args.iterations
    );

    for path in &stackup_paths {
        // Load stackup
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!(
                    "{} Failed to read {}: {}",
                    style("✗").red(),
                    path.display(),
                    e
                );
                errors += 1;
                continue;
            }
        };

        let mut stackup: Stackup = match serde_yml::from_str(&content) {
            Ok(s) => s,
            Err(e) => {
                eprintln!(
                    "{} Failed to parse {}: {}",
                    style("✗").red(),
                    path.display(),
                    e
                );
                errors += 1;
                continue;
            }
        };

        let short_id = short_ids
            .get_short_id(&stackup.id.to_string())
            .unwrap_or_else(|| format_short_id(&stackup.id));

        // Skip stackups with no contributors
        if stackup.contributors.is_empty() {
            if args.verbose {
                println!(
                    "{} {} - no contributors, skipping",
                    style("⚠").yellow(),
                    style(&short_id).cyan()
                );
            }
            skipped += 1;
            continue;
        }

        if args.dry_run {
            println!(
                "{} {} - {} contributors (would analyze)",
                style("→").blue(),
                style(&short_id).cyan(),
                stackup.contributors.len()
            );
            analyzed += 1;
            continue;
        }

        // Run analysis
        stackup.analysis_results.monte_carlo = Some(stackup.calculate_monte_carlo(args.iterations));
        stackup.analysis_results.worst_case = Some(stackup.calculate_worst_case());
        stackup.analysis_results.rss = Some(stackup.calculate_rss());

        // Write back
        let yaml_content = match serde_yml::to_string(&stackup) {
            Ok(y) => y,
            Err(e) => {
                eprintln!(
                    "{} Failed to serialize {}: {}",
                    style("✗").red(),
                    short_id,
                    e
                );
                errors += 1;
                continue;
            }
        };

        if let Err(e) = fs::write(path, &yaml_content) {
            eprintln!(
                "{} Failed to write {}: {}",
                style("✗").red(),
                path.display(),
                e
            );
            errors += 1;
            continue;
        }

        // Extract summary info
        let wc_result = stackup
            .analysis_results
            .worst_case
            .as_ref()
            .map(|wc| format!("{}", wc.result));
        let cpk = stackup.analysis_results.rss.as_ref().map(|r| r.cpk);
        let mc_yield = stackup
            .analysis_results
            .monte_carlo
            .as_ref()
            .map(|m| m.yield_percent);

        results_summary.push((
            short_id.clone(),
            stackup.title.clone(),
            wc_result.clone(),
            cpk,
            mc_yield,
        ));

        // Brief output for each stackup
        let result_styled = match wc_result.as_deref() {
            Some("pass") => style("pass").green(),
            Some("marginal") => style("marginal").yellow(),
            Some("fail") => style("fail").red(),
            _ => style("-").dim(),
        };

        let cpk_styled = match cpk {
            Some(c) if c >= 1.33 => style(format!("{:.2}", c)).green(),
            Some(c) if c >= 1.0 => style(format!("{:.2}", c)).yellow(),
            Some(c) => style(format!("{:.2}", c)).red(),
            None => style("-".to_string()).dim(),
        };

        println!(
            "{} {} - W/C: {:<8} Cpk: {:<6} Yield: {:.1}%",
            style("✓").green(),
            style(&short_id).cyan(),
            result_styled,
            cpk_styled,
            mc_yield.unwrap_or(0.0)
        );

        analyzed += 1;
    }

    // Summary
    println!();
    if args.dry_run {
        println!(
            "{} {} stackup(s) would be analyzed, {} skipped (no contributors), {} error(s)",
            style("Dry run:").bold(),
            style(analyzed).cyan(),
            skipped,
            errors
        );
    } else {
        println!(
            "{} Analyzed {} stackup(s), {} skipped, {} error(s)",
            style("Done:").bold(),
            style(analyzed).green(),
            skipped,
            errors
        );

        // Show problem stackups if any
        let failing: Vec<_> = results_summary
            .iter()
            .filter(|(_, _, wc, _, _)| wc.as_deref() == Some("fail"))
            .collect();
        let marginal: Vec<_> = results_summary
            .iter()
            .filter(|(_, _, wc, _, _)| wc.as_deref() == Some("marginal"))
            .collect();

        if !failing.is_empty() {
            println!();
            println!(
                "{} {} stackup(s) failing worst-case analysis:",
                style("⚠").red(),
                failing.len()
            );
            for (short_id, title, _, cpk, _) in failing {
                let cpk_str = cpk.map(|c| format!("{:.2}", c)).unwrap_or("-".to_string());
                println!(
                    "   {} {} (Cpk: {})",
                    style(short_id).cyan(),
                    truncate_str(title, 30),
                    cpk_str
                );
            }
        }

        if !marginal.is_empty() {
            println!();
            println!(
                "{} {} stackup(s) marginal:",
                style("!").yellow(),
                marginal.len()
            );
            for (short_id, title, _, cpk, _) in marginal {
                let cpk_str = cpk.map(|c| format!("{:.2}", c)).unwrap_or("-".to_string());
                println!(
                    "   {} {} (Cpk: {})",
                    style(short_id).cyan(),
                    truncate_str(title, 30),
                    cpk_str
                );
            }
        }
    }

    Ok(())
}

fn run_add(args: AddArgs) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;

    // Resolve stackup short ID
    let short_ids = ShortIdIndex::load(&project);
    let resolved_stackup_id = short_ids
        .resolve(&args.stackup)
        .unwrap_or_else(|| args.stackup.clone());

    // Find and load the stackup
    let tol_dir = project.root().join("tolerances/stackups");
    let mut found_path = None;

    if tol_dir.exists() {
        for entry in fs::read_dir(&tol_dir).into_diagnostic()? {
            let entry = entry.into_diagnostic()?;
            let path = entry.path();

            if path.extension().is_some_and(|e| e == "yaml") {
                let filename = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                if filename.contains(&resolved_stackup_id)
                    || filename.starts_with(&resolved_stackup_id)
                {
                    found_path = Some(path);
                    break;
                }
            }
        }
    }

    let stackup_path = found_path
        .ok_or_else(|| miette::miette!("No stackup found matching '{}'", args.stackup))?;

    // Load stackup
    let content = fs::read_to_string(&stackup_path).into_diagnostic()?;
    let mut stackup: Stackup = serde_yml::from_str(&content).into_diagnostic()?;

    // Parse and process each feature reference
    let feat_dir = project.root().join("tolerances/features");
    let mut added_count = 0;

    for feat_ref in &args.features {
        // Parse direction prefix (+/~)
        // Using ~ instead of - to avoid conflicts with CLI flags
        let (direction, feat_id_str) = if let Some(stripped) = feat_ref.strip_prefix('+') {
            (Direction::Positive, stripped)
        } else if let Some(stripped) = feat_ref.strip_prefix('~') {
            (Direction::Negative, stripped)
        } else {
            // Default to positive if no prefix
            (Direction::Positive, feat_ref.as_str())
        };

        // Resolve feature short ID
        let resolved_feat_id = short_ids
            .resolve(feat_id_str)
            .unwrap_or_else(|| feat_id_str.to_string());

        // Find and load the feature
        let mut feature_path = None;
        if feat_dir.exists() {
            for entry in fs::read_dir(&feat_dir).into_diagnostic()? {
                let entry = entry.into_diagnostic()?;
                let path = entry.path();

                if path.extension().is_some_and(|e| e == "yaml") {
                    let filename = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                    if filename.contains(&resolved_feat_id)
                        || filename.starts_with(&resolved_feat_id)
                    {
                        feature_path = Some(path);
                        break;
                    }
                }
            }
        }

        let feat_path = feature_path
            .ok_or_else(|| miette::miette!("No feature found matching '{}'", feat_id_str))?;

        let feat_content = fs::read_to_string(&feat_path).into_diagnostic()?;
        let feature: Feature = serde_yml::from_str(&feat_content).into_diagnostic()?;

        // Get dimension from feature
        let dimension = if let Some(ref dim_name) = args.dimension {
            feature
                .dimensions
                .iter()
                .find(|d| d.name.to_lowercase() == dim_name.to_lowercase())
                .ok_or_else(|| {
                    miette::miette!(
                        "Dimension '{}' not found in feature {}. Available: {}",
                        dim_name,
                        feature.id,
                        feature
                            .dimensions
                            .iter()
                            .map(|d| d.name.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                })?
        } else {
            feature.dimensions.first().ok_or_else(|| {
                miette::miette!("Feature {} has no dimensions defined", feature.id)
            })?
        };

        // Check if feature is already in stackup
        let already_exists = stackup
            .contributors
            .iter()
            .any(|c| c.feature.as_ref().map(|f| &f.id) == Some(&feature.id));

        if already_exists {
            println!(
                "{} Feature {} already in stackup, skipping",
                style("!").yellow(),
                style(feat_id_str).cyan()
            );
            continue;
        }

        // Try to load component to get component_name for cached info
        let component_name = {
            let cmp_dir = project.root().join("bom/components");
            let mut name = None;
            if cmp_dir.exists() {
                for entry in fs::read_dir(&cmp_dir).into_diagnostic()? {
                    let entry = entry.into_diagnostic()?;
                    let path = entry.path();
                    if path.extension().is_some_and(|e| e == "yaml") {
                        let filename = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                        if filename.contains(&feature.component) {
                            if let Ok(content) = fs::read_to_string(&path) {
                                if let Ok(cmp) = serde_yml::from_str::<
                                    crate::entities::component::Component,
                                >(&content)
                                {
                                    name = Some(cmp.title);
                                    break;
                                }
                            }
                        }
                    }
                }
            }
            name
        };

        // Create contributor from feature with cached info
        // Distribution comes from the feature's dimension, not CLI args
        let contributor = Contributor {
            name: format!("{} - {}", feature.title, dimension.name),
            feature: Some(FeatureRef::with_cache(
                feature.id.clone(),
                Some(feature.title.clone()),
                Some(feature.component.clone()),
                component_name,
            )),
            direction,
            nominal: dimension.nominal,
            plus_tol: dimension.plus_tol,
            minus_tol: dimension.minus_tol,
            distribution: dimension.distribution,
            source: if feature.drawing.number.is_empty() {
                None
            } else {
                Some(format!(
                    "{} Rev {}",
                    feature.drawing.number, feature.drawing.revision
                ))
            },
        };

        let dir_symbol = match direction {
            Direction::Positive => "+",
            Direction::Negative => "-",
        };

        println!(
            "{} Added {} ({}{:.3} +{:.3}/-{:.3})",
            style("✓").green(),
            style(&contributor.name).cyan(),
            dir_symbol,
            contributor.nominal,
            contributor.plus_tol,
            contributor.minus_tol
        );

        stackup.contributors.push(contributor);
        added_count += 1;
    }

    if added_count == 0 {
        println!("No features added.");
        return Ok(());
    }

    // Write updated stackup
    let yaml_content = serde_yml::to_string(&stackup).into_diagnostic()?;
    fs::write(&stackup_path, &yaml_content).into_diagnostic()?;

    println!();
    println!(
        "{} Added {} contributor(s) to stackup {}",
        style("✓").green(),
        added_count,
        style(&args.stackup).cyan()
    );

    // Optionally run analysis
    if args.analyze {
        println!();
        run_analyze(AnalyzeArgs {
            id: Some(args.stackup),
            all: false,
            iterations: 10000,
            verbose: false,
            histogram: false,
            csv: false,
            bins: 40,
            dry_run: false,
            debug: false,
        })?;
    }

    Ok(())
}

fn run_remove(args: RemoveArgs) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;

    // Resolve stackup short ID
    let short_ids = ShortIdIndex::load(&project);
    let resolved_stackup_id = short_ids
        .resolve(&args.stackup)
        .unwrap_or_else(|| args.stackup.clone());

    // Find and load the stackup
    let tol_dir = project.root().join("tolerances/stackups");
    let mut found_path = None;

    if tol_dir.exists() {
        for entry in fs::read_dir(&tol_dir).into_diagnostic()? {
            let entry = entry.into_diagnostic()?;
            let path = entry.path();

            if path.extension().is_some_and(|e| e == "yaml") {
                let filename = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                if filename.contains(&resolved_stackup_id)
                    || filename.starts_with(&resolved_stackup_id)
                {
                    found_path = Some(path);
                    break;
                }
            }
        }
    }

    let stackup_path = found_path
        .ok_or_else(|| miette::miette!("No stackup found matching '{}'", args.stackup))?;

    // Load stackup
    let content = fs::read_to_string(&stackup_path).into_diagnostic()?;
    let mut stackup: Stackup = serde_yml::from_str(&content).into_diagnostic()?;

    let original_count = stackup.contributors.len();

    // Resolve each feature ID and remove matching contributors
    for feat_ref in &args.features {
        let resolved_feat_id = short_ids
            .resolve(feat_ref)
            .unwrap_or_else(|| feat_ref.to_string());

        let before_len = stackup.contributors.len();
        stackup.contributors.retain(|c| {
            if let Some(ref feat) = c.feature {
                !feat.id.to_string().contains(&resolved_feat_id)
            } else {
                true
            }
        });

        if stackup.contributors.len() < before_len {
            println!(
                "{} Removed contributor for feature {}",
                style("✓").green(),
                style(feat_ref).cyan()
            );
        } else {
            println!(
                "{} No contributor found for feature {}",
                style("!").yellow(),
                style(feat_ref).cyan()
            );
        }
    }

    let removed_count = original_count - stackup.contributors.len();

    if removed_count == 0 {
        println!("No contributors removed.");
        return Ok(());
    }

    // Write updated stackup
    let yaml_content = serde_yml::to_string(&stackup).into_diagnostic()?;
    fs::write(&stackup_path, &yaml_content).into_diagnostic()?;

    println!();
    println!(
        "{} Removed {} contributor(s) from stackup {}",
        style("✓").green(),
        removed_count,
        style(&args.stackup).cyan()
    );

    Ok(())
}

/// Convert a Stackup entity to a TableRow
fn stackup_to_row(stackup: &Stackup, short_ids: &ShortIdIndex) -> TableRow {
    let wc_result = stackup
        .analysis_results
        .worst_case
        .as_ref()
        .map(|wc| wc.result.to_string())
        .unwrap_or_else(|| "n/a".to_string());

    let cpk = stackup.analysis_results.rss.as_ref().map(|rss| rss.cpk);

    let mc_yield = stackup
        .analysis_results
        .monte_carlo
        .as_ref()
        .map(|mc| mc.yield_percent);

    TableRow::new(stackup.id.to_string(), short_ids)
        .cell("id", CellValue::Id(stackup.id.to_string()))
        .cell("title", CellValue::Text(stackup.title.clone()))
        .cell("result", CellValue::AnalysisResult(wc_result))
        .cell("cpk", CellValue::Cpk(cpk))
        .cell("yield", CellValue::YieldPct(mc_yield))
        .cell(
            "disposition",
            CellValue::Type(stackup.disposition.to_string()),
        )
        .cell("status", CellValue::Type(stackup.status().to_string()))
        .cell("critical", CellValue::Critical(stackup.target.critical))
        .cell("author", CellValue::Text(stackup.author.clone()))
        .cell("created", CellValue::DateTime(stackup.created))
}
