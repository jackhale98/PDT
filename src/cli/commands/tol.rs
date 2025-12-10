//! `pdt tol` command - Stackup/tolerance analysis management

use clap::{Subcommand, ValueEnum};
use console::style;
use miette::{IntoDiagnostic, Result};
use std::fs;

use crate::cli::{GlobalOpts, OutputFormat};
use crate::core::entity::Entity;
use crate::core::identity::{EntityId, EntityPrefix};
use crate::core::project::Project;
use crate::core::shortid::ShortIdIndex;
use crate::core::Config;
use crate::entities::feature::Feature;
use crate::entities::stackup::{Contributor, Direction, Disposition, Stackup};
use crate::schema::template::{TemplateContext, TemplateGenerator};

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

/// Analysis result filter
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ResultFilter {
    Pass,
    Marginal,
    Fail,
    All,
}

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

    /// Limit number of results
    #[arg(long, short = 'n')]
    pub limit: Option<usize>,

    /// Show only count
    #[arg(long)]
    pub count: bool,

    /// Output format
    #[arg(long, short = 'o', default_value = "auto")]
    pub format: OutputFormat,
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
}

#[derive(clap::Args, Debug)]
pub struct ShowArgs {
    /// Stackup ID or short ID (TOL@N)
    pub id: String,

    /// Output format
    #[arg(long, short = 'o', default_value = "yaml")]
    pub format: OutputFormat,
}

#[derive(clap::Args, Debug)]
pub struct EditArgs {
    /// Stackup ID or short ID (TOL@N)
    pub id: String,
}

#[derive(clap::Args, Debug)]
pub struct AnalyzeArgs {
    /// Stackup ID or short ID (TOL@N)
    pub id: String,

    /// Number of Monte Carlo iterations (default: 10000)
    #[arg(long, default_value = "10000")]
    pub iterations: u32,

    /// Show detailed results after analysis
    #[arg(long, short = 'v')]
    pub verbose: bool,
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
pub fn run(cmd: TolCommands, _global: &GlobalOpts) -> Result<()> {
    match cmd {
        TolCommands::List(args) => run_list(args),
        TolCommands::New(args) => run_new(args),
        TolCommands::Show(args) => run_show(args),
        TolCommands::Edit(args) => run_edit(args),
        TolCommands::Analyze(args) => run_analyze(args),
        TolCommands::Add(args) => run_add(args),
        TolCommands::Remove(args) => run_remove(args),
    }
}

fn run_list(args: ListArgs) -> Result<()> {
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

        if path.extension().map_or(false, |e| e == "yaml") {
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
                        .map_or(false, |d| d.to_lowercase().contains(&search_lower))
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
        .collect();

    // Apply limit
    let mut stackups = stackups;
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
    let _ = short_ids.save(&project);

    // Output based on format
    let format = if args.format == OutputFormat::Auto {
        OutputFormat::Tsv
    } else {
        args.format
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
        OutputFormat::Csv => {
            println!("short_id,id,title,target,wc_result,cpk,disposition,status");
            for s in &stackups {
                let short_id = short_ids.get_short_id(&s.id.to_string()).unwrap_or_default();
                let wc_result = s
                    .analysis_results
                    .worst_case
                    .as_ref()
                    .map(|wc| format!("{}", wc.result))
                    .unwrap_or_else(|| "n/a".to_string());
                let cpk = s
                    .analysis_results
                    .rss
                    .as_ref()
                    .map(|rss| format!("{:.2}", rss.cpk))
                    .unwrap_or_else(|| "n/a".to_string());
                println!(
                    "{},{},{},{},{},{},{},{}",
                    short_id,
                    s.id,
                    escape_csv(&s.title),
                    escape_csv(&s.target.name),
                    wc_result,
                    cpk,
                    s.disposition,
                    s.status()
                );
            }
        }
        OutputFormat::Tsv => {
            println!(
                "{:<8} {:<16} {:<22} {:<12} {:<8} {:<8} {:<10}",
                style("SHORT").bold().dim(),
                style("ID").bold(),
                style("TITLE").bold(),
                style("TARGET").bold(),
                style("W/C").bold(),
                style("CPK").bold(),
                style("STATUS").bold()
            );
            println!("{}", "-".repeat(90));

            for s in &stackups {
                let short_id = short_ids.get_short_id(&s.id.to_string()).unwrap_or_default();
                let id_display = format_short_id(&s.id);
                let wc_result = s
                    .analysis_results
                    .worst_case
                    .as_ref()
                    .map(|wc| format!("{}", wc.result))
                    .unwrap_or_else(|| "n/a".to_string());
                let cpk = s
                    .analysis_results
                    .rss
                    .as_ref()
                    .map(|rss| format!("{:.2}", rss.cpk))
                    .unwrap_or_else(|| "n/a".to_string());

                let wc_styled = match wc_result.as_str() {
                    "pass" => style(wc_result).green(),
                    "marginal" => style(wc_result).yellow(),
                    "fail" => style(wc_result).red(),
                    _ => style(wc_result).dim(),
                };

                println!(
                    "{:<8} {:<16} {:<22} {:<12} {:<8} {:<8} {:<10}",
                    style(&short_id).cyan(),
                    id_display,
                    truncate_str(&s.title, 20),
                    truncate_str(&s.target.name, 10),
                    wc_styled,
                    cpk,
                    s.status()
                );
            }

            println!();
            println!(
                "{} stackup(s) found. Use {} to reference by short ID.",
                style(stackups.len()).cyan(),
                style("TOL@N").cyan()
            );
        }
        OutputFormat::Id => {
            for s in &stackups {
                println!("{}", s.id);
            }
        }
        OutputFormat::Md => {
            println!("| Short | ID | Title | Target | W/C | Cpk | Status |");
            println!("|---|---|---|---|---|---|---|");
            for s in &stackups {
                let short_id = short_ids.get_short_id(&s.id.to_string()).unwrap_or_default();
                let wc_result = s
                    .analysis_results
                    .worst_case
                    .as_ref()
                    .map(|wc| format!("{}", wc.result))
                    .unwrap_or_else(|| "n/a".to_string());
                let cpk = s
                    .analysis_results
                    .rss
                    .as_ref()
                    .map(|rss| format!("{:.2}", rss.cpk))
                    .unwrap_or_else(|| "n/a".to_string());
                println!(
                    "| {} | {} | {} | {} | {} | {} | {} |",
                    short_id,
                    format_short_id(&s.id),
                    s.title,
                    s.target.name,
                    wc_result,
                    cpk,
                    s.status()
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
    let target_name: String;
    let target_nominal: f64;
    let target_upper: f64;
    let target_lower: f64;

    if args.interactive
        || args.title.is_none()
        || args.target_nominal.is_none()
        || args.target_upper.is_none()
        || args.target_lower.is_none()
    {
        use dialoguer::{Confirm, Input};

        title = if let Some(t) = args.title {
            t
        } else {
            Input::new()
                .with_prompt("Stackup title")
                .interact_text()
                .into_diagnostic()?
        };

        target_name = if let Some(n) = args.target_name {
            n
        } else {
            Input::new()
                .with_prompt("Target dimension name (e.g., 'Gap', 'Clearance')")
                .interact_text()
                .into_diagnostic()?
        };

        target_nominal = if let Some(n) = args.target_nominal {
            n
        } else {
            Input::new()
                .with_prompt("Target nominal value")
                .interact_text()
                .into_diagnostic()?
        };

        target_upper = if let Some(u) = args.target_upper {
            u
        } else {
            Input::new()
                .with_prompt("Target upper specification limit")
                .interact_text()
                .into_diagnostic()?
        };

        target_lower = if let Some(l) = args.target_lower {
            l
        } else {
            Input::new()
                .with_prompt("Target lower specification limit")
                .interact_text()
                .into_diagnostic()?
        };

        // Ask about critical if not specified
        let _critical = if args.critical {
            true
        } else {
            Confirm::new()
                .with_prompt("Is this a critical dimension?")
                .default(false)
                .interact()
                .into_diagnostic()?
        };
    } else {
        title = args.title.unwrap();
        target_name = args.target_name.unwrap_or_else(|| "Target".to_string());
        target_nominal = args.target_nominal.unwrap();
        target_upper = args.target_upper.unwrap();
        target_lower = args.target_lower.unwrap();
    }

    // Generate ID
    let id = EntityId::new(EntityPrefix::Tol);

    // Generate template
    let generator = TemplateGenerator::new().map_err(|e| miette::miette!("{}", e))?;
    let ctx = TemplateContext::new(id.clone(), config.author())
        .with_title(&title)
        .with_target(&target_name, target_nominal, target_upper, target_lower);

    let yaml_content = generator
        .generate_stackup(&ctx)
        .map_err(|e| miette::miette!("{}", e))?;

    // Write file
    let output_dir = project.root().join("tolerances/stackups");
    if !output_dir.exists() {
        fs::create_dir_all(&output_dir).into_diagnostic()?;
    }

    let file_path = output_dir.join(format!("{}.pdt.yaml", id));
    fs::write(&file_path, &yaml_content).into_diagnostic()?;

    // Add to short ID index
    let mut short_ids = ShortIdIndex::load(&project);
    let short_id = short_ids.add(id.to_string());
    let _ = short_ids.save(&project);

    println!(
        "{} Created stackup {}",
        style("✓").green(),
        style(short_id.unwrap_or_else(|| format_short_id(&id))).cyan()
    );
    println!("   {}", style(file_path.display()).dim());
    println!(
        "   Target: {} = {:.3} (LSL: {:.3}, USL: {:.3})",
        style(&target_name).yellow(),
        target_nominal,
        target_lower,
        target_upper
    );

    // Open in editor if requested
    if args.edit || (!args.no_edit && !args.interactive) {
        let editor = config.editor();
        println!();
        println!("Opening in {}...", style(&editor).yellow());

        std::process::Command::new(&editor)
            .arg(&file_path)
            .status()
            .into_diagnostic()?;
    }

    Ok(())
}

fn run_show(args: ShowArgs) -> Result<()> {
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

            if path.extension().map_or(false, |e| e == "yaml") {
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

    // Read and display
    let content = fs::read_to_string(&path).into_diagnostic()?;

    match args.format {
        OutputFormat::Yaml | OutputFormat::Auto => {
            print!("{}", content);
        }
        OutputFormat::Json => {
            let stackup: Stackup = serde_yml::from_str(&content).into_diagnostic()?;
            let json = serde_json::to_string_pretty(&stackup).into_diagnostic()?;
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

    // Find the stackup file
    let tol_dir = project.root().join("tolerances/stackups");
    let mut found_path = None;

    if tol_dir.exists() {
        for entry in fs::read_dir(&tol_dir).into_diagnostic()? {
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

    let path =
        found_path.ok_or_else(|| miette::miette!("No stackup found matching '{}'", args.id))?;

    let editor = config.editor();
    println!(
        "Opening {} in {}...",
        style(path.display()).cyan(),
        style(&editor).yellow()
    );

    std::process::Command::new(&editor)
        .arg(&path)
        .status()
        .into_diagnostic()?;

    Ok(())
}

fn run_analyze(args: AnalyzeArgs) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;

    // Resolve short ID if needed
    let short_ids = ShortIdIndex::load(&project);
    let resolved_id = short_ids
        .resolve(&args.id)
        .unwrap_or_else(|| args.id.clone());

    // Find and load the stackup
    let tol_dir = project.root().join("tolerances/stackups");
    let mut found_path = None;

    if tol_dir.exists() {
        for entry in fs::read_dir(&tol_dir).into_diagnostic()? {
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

    let path =
        found_path.ok_or_else(|| miette::miette!("No stackup found matching '{}'", args.id))?;

    // Load stackup
    let content = fs::read_to_string(&path).into_diagnostic()?;
    let mut stackup: Stackup = serde_yml::from_str(&content).into_diagnostic()?;

    if stackup.contributors.is_empty() {
        return Err(miette::miette!(
            "Stackup has no contributors. Add contributors before running analysis."
        ));
    }

    // Run analysis
    println!(
        "{} Analyzing stackup {} with {} contributors...",
        style("⚙").cyan(),
        style(&args.id).cyan(),
        stackup.contributors.len()
    );

    stackup.analysis_results.worst_case = Some(stackup.calculate_worst_case());
    stackup.analysis_results.rss = Some(stackup.calculate_rss());
    stackup.analysis_results.monte_carlo = Some(stackup.calculate_monte_carlo(args.iterations));

    // Write back
    let yaml_content = serde_yml::to_string(&stackup).into_diagnostic()?;
    fs::write(&path, &yaml_content).into_diagnostic()?;

    println!(
        "{} Analysis complete for stackup {}",
        style("✓").green(),
        style(&args.id).cyan()
    );

    // Show results summary
    println!();
    println!(
        "   Target: {} = {:.4} (LSL: {:.4}, USL: {:.4})",
        style(&stackup.target.name).yellow(),
        stackup.target.nominal,
        stackup.target.lower_limit,
        stackup.target.upper_limit
    );

    if let Some(ref wc) = stackup.analysis_results.worst_case {
        let result_style = match wc.result {
            crate::entities::stackup::AnalysisResult::Pass => style(format!("{}", wc.result)).green(),
            crate::entities::stackup::AnalysisResult::Marginal => {
                style(format!("{}", wc.result)).yellow()
            }
            crate::entities::stackup::AnalysisResult::Fail => style(format!("{}", wc.result)).red(),
        };

        println!();
        println!("   {} Analysis:", style("Worst-Case").bold());
        println!(
            "     Range: {:.4} to {:.4}",
            wc.min, wc.max
        );
        println!("     Margin: {:.4}", wc.margin);
        println!("     Result: {}", result_style);
    }

    if let Some(ref rss) = stackup.analysis_results.rss {
        println!();
        println!("   {} Analysis:", style("RSS (Statistical)").bold());
        println!("     Mean: {:.4}", rss.mean);
        println!("     ±3σ: {:.4}", rss.sigma_3);
        println!("     Margin: {:.4}", rss.margin);
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
        println!("     Mean: {:.4}", mc.mean);
        println!("     Std Dev: {:.4}", mc.std_dev);
        println!(
            "     Range: {:.4} to {:.4}",
            mc.min, mc.max
        );
        println!(
            "     95% CI: {:.4} to {:.4}",
            mc.percentile_2_5, mc.percentile_97_5
        );
        println!("     Yield: {:.2}%", mc.yield_percent);
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

            if path.extension().map_or(false, |e| e == "yaml") {
                let filename = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                if filename.contains(&resolved_stackup_id) || filename.starts_with(&resolved_stackup_id)
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
        let (direction, feat_id_str) = if feat_ref.starts_with('+') {
            (Direction::Positive, &feat_ref[1..])
        } else if feat_ref.starts_with('~') {
            (Direction::Negative, &feat_ref[1..])
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

                if path.extension().map_or(false, |e| e == "yaml") {
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
                miette::miette!(
                    "Feature {} has no dimensions defined",
                    feature.id
                )
            })?
        };

        // Check if feature is already in stackup
        let already_exists = stackup
            .contributors
            .iter()
            .any(|c| c.feature_id.as_ref() == Some(&feature.id.to_string()));

        if already_exists {
            println!(
                "{} Feature {} already in stackup, skipping",
                style("!").yellow(),
                style(feat_id_str).cyan()
            );
            continue;
        }

        // Create contributor from feature
        // Distribution comes from the feature's dimension, not CLI args
        let contributor = Contributor {
            name: format!("{} - {}", feature.title, dimension.name),
            feature_id: Some(feature.id.to_string()),
            direction,
            nominal: dimension.nominal,
            plus_tol: dimension.plus_tol,
            minus_tol: dimension.minus_tol,
            distribution: dimension.distribution,
            source: if feature.drawing.number.is_empty() {
                None
            } else {
                Some(format!("{} Rev {}", feature.drawing.number, feature.drawing.revision))
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
            id: args.stackup,
            iterations: 10000,
            verbose: false,
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

            if path.extension().map_or(false, |e| e == "yaml") {
                let filename = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                if filename.contains(&resolved_stackup_id) || filename.starts_with(&resolved_stackup_id)
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
            if let Some(ref fid) = c.feature_id {
                !fid.contains(&resolved_feat_id)
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

// Helper functions

fn format_short_id(id: &EntityId) -> String {
    let s = id.to_string();
    if s.len() > 15 {
        format!("{}...", &s[..12])
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
