//! `tdt mate` command - Mate management (1:1 feature contacts with fit calculation)

use clap::{Subcommand, ValueEnum};
use console::style;
use miette::{bail, IntoDiagnostic, Result};
use std::fs;

use crate::cli::{GlobalOpts, OutputFormat};
use crate::core::entity::Entity;
use crate::core::identity::{EntityId, EntityPrefix};
use crate::core::project::Project;
use crate::core::shortid::ShortIdIndex;
use crate::core::Config;
use crate::entities::feature::Feature;
use crate::entities::mate::{FitAnalysis, Mate, MateType};
use crate::schema::template::{TemplateContext, TemplateGenerator};
use crate::schema::wizard::SchemaWizard;

#[derive(Subcommand, Debug)]
pub enum MateCommands {
    /// List mates with filtering
    List(ListArgs),

    /// Create a new mate (requires --feature-a and --feature-b)
    New(NewArgs),

    /// Show a mate's details (includes calculated fit)
    Show(ShowArgs),

    /// Edit a mate in your editor
    Edit(EditArgs),

    /// Recalculate fit analysis from current feature dimensions
    Recalc(RecalcArgs),
}

/// Mate type filter
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum TypeFilter {
    ClearanceFit,
    InterferenceFit,
    TransitionFit,
    PlanarContact,
    ThreadEngagement,
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

/// List column selection
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ListColumn {
    Id,
    Title,
    MateType,
    FitResult,
    Match,
    FeatureA,
    FeatureB,
    Status,
    Author,
    Created,
}

impl std::fmt::Display for ListColumn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ListColumn::Id => write!(f, "id"),
            ListColumn::Title => write!(f, "title"),
            ListColumn::MateType => write!(f, "mate-type"),
            ListColumn::FitResult => write!(f, "fit-result"),
            ListColumn::Match => write!(f, "match"),
            ListColumn::FeatureA => write!(f, "feature-a"),
            ListColumn::FeatureB => write!(f, "feature-b"),
            ListColumn::Status => write!(f, "status"),
            ListColumn::Author => write!(f, "author"),
            ListColumn::Created => write!(f, "created"),
        }
    }
}

#[derive(clap::Args, Debug)]
pub struct ListArgs {
    /// Filter by mate type
    #[arg(long, short = 't', default_value = "all")]
    pub mate_type: TypeFilter,

    /// Filter by status
    #[arg(long, short = 's', default_value = "all")]
    pub status: StatusFilter,

    /// Search in title
    #[arg(long)]
    pub search: Option<String>,

    /// Filter by author name (case-insensitive substring match)
    #[arg(long, short = 'a')]
    pub author: Option<String>,

    /// Filter by recent days (e.g., --recent 7 for last 7 days)
    #[arg(long)]
    pub recent: Option<u32>,

    /// Limit number of results
    #[arg(long, short = 'n')]
    pub limit: Option<usize>,

    /// Show only count
    #[arg(long)]
    pub count: bool,

    /// Sort by column
    #[arg(long)]
    pub sort: Option<ListColumn>,

    /// Columns to display
    #[arg(long, value_delimiter = ',', default_values_t = vec![ListColumn::Id, ListColumn::Title, ListColumn::MateType, ListColumn::FitResult, ListColumn::Match, ListColumn::Status])]
    pub columns: Vec<ListColumn>,
}

#[derive(clap::Args, Debug)]
pub struct NewArgs {
    /// Features to mate: FEAT_A FEAT_B (positional args)
    /// Example: tdt mate new FEAT@1 FEAT@2 -t interference
    #[arg(value_name = "FEATURE")]
    pub features: Vec<String>,

    /// First feature ID - alternative to positional arg
    #[arg(long = "feature-a", short = 'a')]
    pub feature_a: Option<String>,

    /// Second feature ID - alternative to positional arg
    #[arg(long = "feature-b", short = 'b')]
    pub feature_b: Option<String>,

    /// Mate type
    #[arg(long, short = 't', default_value = "clearance_fit")]
    pub mate_type: String,

    /// Title/description
    #[arg(long, short = 'T')]
    pub title: Option<String>,

    /// Open in editor after creation
    #[arg(long, short = 'e')]
    pub edit: bool,

    /// Skip opening in editor
    #[arg(long, short = 'n')]
    pub no_edit: bool,

    /// Interactive mode (prompt for fields)
    #[arg(long, short = 'i')]
    pub interactive: bool,
}

#[derive(clap::Args, Debug)]
pub struct ShowArgs {
    /// Mate ID or short ID (MATE@N)
    pub id: String,
}

#[derive(clap::Args, Debug)]
pub struct EditArgs {
    /// Mate ID or short ID (MATE@N)
    pub id: String,
}

#[derive(clap::Args, Debug)]
pub struct RecalcArgs {
    /// Mate ID or short ID (MATE@N)
    pub id: String,
}

/// Run a mate subcommand
pub fn run(cmd: MateCommands, global: &GlobalOpts) -> Result<()> {
    match cmd {
        MateCommands::List(args) => run_list(args, global),
        MateCommands::New(args) => run_new(args),
        MateCommands::Show(args) => run_show(args, global),
        MateCommands::Edit(args) => run_edit(args),
        MateCommands::Recalc(args) => run_recalc(args),
    }
}

fn run_list(args: ListArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let mate_dir = project.root().join("tolerances/mates");

    if !mate_dir.exists() {
        if args.count {
            println!("0");
        } else {
            println!("No mates found.");
        }
        return Ok(());
    }

    // Load and parse all mates
    let mut mates: Vec<Mate> = Vec::new();

    for entry in fs::read_dir(&mate_dir).into_diagnostic()? {
        let entry = entry.into_diagnostic()?;
        let path = entry.path();

        if path.extension().map_or(false, |e| e == "yaml") {
            let content = fs::read_to_string(&path).into_diagnostic()?;
            if let Ok(mate) = serde_yml::from_str::<Mate>(&content) {
                mates.push(mate);
            }
        }
    }

    // Apply filters
    let mates: Vec<Mate> = mates
        .into_iter()
        .filter(|m| match args.mate_type {
            TypeFilter::ClearanceFit => m.mate_type == MateType::ClearanceFit,
            TypeFilter::InterferenceFit => m.mate_type == MateType::InterferenceFit,
            TypeFilter::TransitionFit => m.mate_type == MateType::TransitionFit,
            TypeFilter::PlanarContact => m.mate_type == MateType::PlanarContact,
            TypeFilter::ThreadEngagement => m.mate_type == MateType::ThreadEngagement,
            TypeFilter::All => true,
        })
        .filter(|m| match args.status {
            StatusFilter::Draft => m.status == crate::core::entity::Status::Draft,
            StatusFilter::Review => m.status == crate::core::entity::Status::Review,
            StatusFilter::Approved => m.status == crate::core::entity::Status::Approved,
            StatusFilter::Released => m.status == crate::core::entity::Status::Released,
            StatusFilter::Obsolete => m.status == crate::core::entity::Status::Obsolete,
            StatusFilter::All => true,
        })
        .filter(|m| {
            if let Some(ref search) = args.search {
                let search_lower = search.to_lowercase();
                m.title.to_lowercase().contains(&search_lower)
                    || m.description
                        .as_ref()
                        .map_or(false, |d| d.to_lowercase().contains(&search_lower))
            } else {
                true
            }
        })
        .filter(|m| {
            args.author
                .as_ref()
                .map_or(true, |a| m.author.to_lowercase().contains(&a.to_lowercase()))
        })
        .filter(|m| {
            args.recent.map_or(true, |days| {
                let cutoff = chrono::Utc::now() - chrono::Duration::days(days as i64);
                m.created >= cutoff
            })
        })
        .collect();

    // Apply sorting
    let mut mates = mates;
    if let Some(sort_column) = args.sort {
        match sort_column {
            ListColumn::Id => mates.sort_by(|a, b| a.id.to_string().cmp(&b.id.to_string())),
            ListColumn::Title => mates.sort_by(|a, b| a.title.cmp(&b.title)),
            ListColumn::MateType => mates.sort_by(|a, b| format!("{}", a.mate_type).cmp(&format!("{}", b.mate_type))),
            ListColumn::FitResult => mates.sort_by(|a, b| {
                let fit_a = a.fit_analysis.as_ref().map(|f| format!("{}", f.fit_result)).unwrap_or_default();
                let fit_b = b.fit_analysis.as_ref().map(|f| format!("{}", f.fit_result)).unwrap_or_default();
                fit_a.cmp(&fit_b)
            }),
            ListColumn::Match => mates.sort_by(|a, b| {
                let match_a = fit_matches_type(a);
                let match_b = fit_matches_type(b);
                match_a.cmp(&match_b)
            }),
            ListColumn::FeatureA => mates.sort_by(|a, b| a.feature_a.id.to_string().cmp(&b.feature_a.id.to_string())),
            ListColumn::FeatureB => mates.sort_by(|a, b| a.feature_b.id.to_string().cmp(&b.feature_b.id.to_string())),
            ListColumn::Status => mates.sort_by(|a, b| a.status().cmp(b.status())),
            ListColumn::Author => mates.sort_by(|a, b| a.author.cmp(&b.author)),
            ListColumn::Created => mates.sort_by(|a, b| a.created.cmp(&b.created)),
        }
    }

    // Apply limit
    if let Some(limit) = args.limit {
        mates.truncate(limit);
    }

    // Count only
    if args.count {
        println!("{}", mates.len());
        return Ok(());
    }

    // No results
    if mates.is_empty() {
        println!("No mates found.");
        return Ok(());
    }

    // Update short ID index
    let mut short_ids = ShortIdIndex::load(&project);
    short_ids.ensure_all(mates.iter().map(|m| m.id.to_string()));
    let _ = short_ids.save(&project);

    // Output based on format
    let format = match global.format {
        OutputFormat::Auto => OutputFormat::Tsv,
        f => f,
    };

    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&mates).into_diagnostic()?;
            println!("{}", json);
        }
        OutputFormat::Yaml => {
            let yaml = serde_yml::to_string(&mates).into_diagnostic()?;
            print!("{}", yaml);
        }
        OutputFormat::Csv => {
            println!("short_id,id,title,mate_type,fit_result,status");
            for mate in &mates {
                let short_id = short_ids.get_short_id(&mate.id.to_string()).unwrap_or_default();
                let fit_result = mate.fit_analysis.as_ref()
                    .map(|a| format!("{}", a.fit_result))
                    .unwrap_or_else(|| "n/a".to_string());
                println!(
                    "{},{},{},{},{},{}",
                    short_id,
                    mate.id,
                    escape_csv(&mate.title),
                    mate.mate_type,
                    fit_result,
                    mate.status()
                );
            }
        }
        OutputFormat::Tsv => {
            // Build header with variable widths
            let mut header_parts = Vec::new();
            let mut widths = Vec::new();
            for col in &args.columns {
                let (header, width) = match col {
                    ListColumn::Id => ("SHORT", 8),
                    ListColumn::Title => ("TITLE", 20),
                    ListColumn::MateType => ("TYPE", 16),
                    ListColumn::FitResult => ("FIT", 12),
                    ListColumn::Match => ("OK", 4),
                    ListColumn::FeatureA => ("FEATURE A", 16),
                    ListColumn::FeatureB => ("FEATURE B", 16),
                    ListColumn::Status => ("STATUS", 10),
                    ListColumn::Author => ("AUTHOR", 15),
                    ListColumn::Created => ("CREATED", 12),
                };
                header_parts.push(format!("{:<width$}", style(header).bold(), width = width));
                widths.push(width);
            }
            println!("{}", header_parts.join(" "));
            println!("{}", "-".repeat(widths.iter().sum::<usize>() + widths.len() - 1));

            // Build rows
            for mate in &mates {
                let short_id = short_ids.get_short_id(&mate.id.to_string()).unwrap_or_default();
                let mut row_parts = Vec::new();

                for (i, col) in args.columns.iter().enumerate() {
                    let width = widths[i];
                    let value = match col {
                        ListColumn::Id => {
                            let id_str = if !short_id.is_empty() {
                                short_id.clone()
                            } else {
                                format_short_id(&mate.id)
                            };
                            format!("{:<width$}", style(&id_str).cyan(), width = width)
                        }
                        ListColumn::Title => {
                            format!("{:<width$}", truncate_str(&mate.title, width.saturating_sub(2)), width = width)
                        }
                        ListColumn::MateType => {
                            let type_str = format!("{}", mate.mate_type);
                            format!("{:<width$}", truncate_str(&type_str, width.saturating_sub(2)), width = width)
                        }
                        ListColumn::FitResult => {
                            let fit_result = mate.fit_analysis.as_ref()
                                .map(|a| format!("{}", a.fit_result))
                                .unwrap_or_else(|| "n/a".to_string());
                            let styled = match fit_result.as_str() {
                                "clearance" => style(fit_result).green(),
                                "interference" => style(fit_result).yellow(),
                                "transition" => style(fit_result).magenta(),
                                _ => style(fit_result).dim(),
                            };
                            format!("{:<width$}", styled, width = width)
                        }
                        ListColumn::Match => {
                            let styled = match fit_matches_type(mate) {
                                FitMatch::Match => style("✓").green(),
                                FitMatch::Mismatch => style("⚠").yellow(),
                                FitMatch::Unknown => style("-").dim(),
                            };
                            format!("{:<width$}", styled, width = width)
                        }
                        ListColumn::FeatureA => {
                            let display = mate.feature_a.name.clone()
                                .or_else(|| short_ids.get_short_id(&mate.feature_a.id.to_string()))
                                .unwrap_or_else(|| mate.feature_a.id.to_string());
                            format!("{:<width$}", truncate_str(&display, width.saturating_sub(2)), width = width)
                        }
                        ListColumn::FeatureB => {
                            let display = mate.feature_b.name.clone()
                                .or_else(|| short_ids.get_short_id(&mate.feature_b.id.to_string()))
                                .unwrap_or_else(|| mate.feature_b.id.to_string());
                            format!("{:<width$}", truncate_str(&display, width.saturating_sub(2)), width = width)
                        }
                        ListColumn::Status => {
                            format!("{:<width$}", mate.status(), width = width)
                        }
                        ListColumn::Author => {
                            format!("{:<width$}", truncate_str(&mate.author, width.saturating_sub(2)), width = width)
                        }
                        ListColumn::Created => {
                            format!("{:<width$}", mate.created.format("%Y-%m-%d"), width = width)
                        }
                    };
                    row_parts.push(value);
                }
                println!("{}", row_parts.join(" "));
            }

            println!();
            println!(
                "{} mate(s) found. Use {} to reference by short ID.",
                style(mates.len()).cyan(),
                style("MATE@N").cyan()
            );
        }
        OutputFormat::Id => {
            for mate in &mates {
                println!("{}", mate.id);
            }
        }
        OutputFormat::Md => {
            println!("| Short | ID | Title | Type | Fit | Status |");
            println!("|---|---|---|---|---|---|");
            for mate in &mates {
                let short_id = short_ids.get_short_id(&mate.id.to_string()).unwrap_or_default();
                let fit_result = mate.fit_analysis.as_ref()
                    .map(|a| format!("{}", a.fit_result))
                    .unwrap_or_else(|| "n/a".to_string());
                println!(
                    "| {} | {} | {} | {} | {} | {} |",
                    short_id,
                    format_short_id(&mate.id),
                    mate.title,
                    mate.mate_type,
                    fit_result,
                    mate.status()
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

    // Determine feature IDs from positional args or -a/-b flags
    let (feat_a_input, feat_b_input) = if args.features.len() >= 2 {
        // Positional args: tdt mate new FEAT@1 FEAT@2
        (args.features[0].clone(), args.features[1].clone())
    } else if let (Some(a), Some(b)) = (&args.feature_a, &args.feature_b) {
        // Flag args: tdt mate new -a FEAT@1 -b FEAT@2
        (a.clone(), b.clone())
    } else {
        bail!("Two features required. Usage:\n  tdt mate new FEAT@1 FEAT@2 -t interference\n  tdt mate new -a FEAT@1 -b FEAT@2 -t interference");
    };

    // Resolve feature IDs
    let short_ids = ShortIdIndex::load(&project);
    let feature_a = short_ids.resolve(&feat_a_input).unwrap_or_else(|| feat_a_input.clone());
    let feature_b = short_ids.resolve(&feat_b_input).unwrap_or_else(|| feat_b_input.clone());

    // Validate features exist and load them for fit calculation
    let feat_dir = project.root().join("tolerances/features");
    let mut feat_a: Option<Feature> = None;
    let mut feat_b: Option<Feature> = None;

    if feat_dir.exists() {
        for entry in fs::read_dir(&feat_dir).into_diagnostic()? {
            let entry = entry.into_diagnostic()?;
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "yaml") {
                let filename = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                if filename.contains(&feature_a) {
                    let content = fs::read_to_string(&path).into_diagnostic()?;
                    if let Ok(feat) = serde_yml::from_str::<Feature>(&content) {
                        feat_a = Some(feat);
                    }
                }
                if filename.contains(&feature_b) {
                    let content = fs::read_to_string(&path).into_diagnostic()?;
                    if let Ok(feat) = serde_yml::from_str::<Feature>(&content) {
                        feat_b = Some(feat);
                    }
                }
            }
        }
    }

    if feat_a.is_none() {
        return Err(miette::miette!(
            "Feature A '{}' not found. Create it first with: tdt feat new",
            feat_a_input
        ));
    }
    if feat_b.is_none() {
        return Err(miette::miette!(
            "Feature B '{}' not found. Create it first with: tdt feat new",
            feat_b_input
        ));
    }

    let title: String;
    let mate_type: String;

    if args.interactive {
        let wizard = SchemaWizard::new();
        let result = wizard.run(EntityPrefix::Mate)?;

        title = result
            .get_string("title")
            .map(String::from)
            .unwrap_or_else(|| "New Mate".to_string());
        mate_type = result
            .get_string("mate_type")
            .map(String::from)
            .unwrap_or_else(|| "clearance_fit".to_string());
    } else {
        title = args.title.unwrap_or_else(|| "New Mate".to_string());
        mate_type = args.mate_type;
    }

    // Generate ID
    let id = EntityId::new(EntityPrefix::Mate);

    // Generate template
    let generator = TemplateGenerator::new().map_err(|e| miette::miette!("{}", e))?;
    let ctx = TemplateContext::new(id.clone(), config.author())
        .with_title(&title)
        .with_feature_a(&feature_a)
        .with_feature_b(&feature_b)
        .with_mate_type(&mate_type);

    let yaml_content = generator
        .generate_mate(&ctx)
        .map_err(|e| miette::miette!("{}", e))?;

    // Try to calculate fit if both features have dimensions
    let fit_analysis = calculate_fit_from_features(&feat_a.unwrap(), &feat_b.unwrap());

    // Parse and update with fit analysis
    let mut mate: Mate = serde_yml::from_str(&yaml_content).into_diagnostic()?;
    mate.fit_analysis = fit_analysis;
    let yaml_content = serde_yml::to_string(&mate).into_diagnostic()?;

    // Write file
    let output_dir = project.root().join("tolerances/mates");
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
        "{} Created mate {}",
        style("✓").green(),
        style(short_id.unwrap_or_else(|| format_short_id(&id))).cyan()
    );
    println!("   {}", style(file_path.display()).dim());
    println!(
        "   {} <-> {} | {}",
        style(truncate_str(&feature_a, 13)).yellow(),
        style(truncate_str(&feature_b, 13)).yellow(),
        style(&title).white()
    );

    // Show fit analysis if calculated
    if let Some(ref analysis) = mate.fit_analysis {
        println!();
        println!("   Fit Analysis:");
        println!(
            "     Result: {} ({:.4} to {:.4})",
            style(format!("{}", analysis.fit_result)).cyan(),
            analysis.worst_case_min_clearance,
            analysis.worst_case_max_clearance
        );
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

    // Find the mate file
    let mate_dir = project.root().join("tolerances/mates");
    let mut found_path = None;

    if mate_dir.exists() {
        for entry in fs::read_dir(&mate_dir).into_diagnostic()? {
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

    let path = found_path.ok_or_else(|| miette::miette!("No mate found matching '{}'", args.id))?;

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
            let mate: Mate = serde_yml::from_str(&content).into_diagnostic()?;
            let json = serde_json::to_string_pretty(&mate).into_diagnostic()?;
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

    // Find the mate file
    let mate_dir = project.root().join("tolerances/mates");
    let mut found_path = None;

    if mate_dir.exists() {
        for entry in fs::read_dir(&mate_dir).into_diagnostic()? {
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

    let path = found_path.ok_or_else(|| miette::miette!("No mate found matching '{}'", args.id))?;

    println!("Opening {} in {}...", style(path.display()).cyan(), style(config.editor()).yellow());

    config.run_editor(&path).into_diagnostic()?;

    Ok(())
}

fn run_recalc(args: RecalcArgs) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;

    // Resolve short ID if needed
    let short_ids = ShortIdIndex::load(&project);
    let resolved_id = short_ids
        .resolve(&args.id)
        .unwrap_or_else(|| args.id.clone());

    // Find and load the mate
    let mate_dir = project.root().join("tolerances/mates");
    let mut found_path = None;

    if mate_dir.exists() {
        for entry in fs::read_dir(&mate_dir).into_diagnostic()? {
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

    let path = found_path.ok_or_else(|| miette::miette!("No mate found matching '{}'", args.id))?;

    // Load mate
    let content = fs::read_to_string(&path).into_diagnostic()?;
    let mut mate: Mate = serde_yml::from_str(&content).into_diagnostic()?;

    // Load features
    let feat_dir = project.root().join("tolerances/features");
    let mut feat_a: Option<Feature> = None;
    let mut feat_b: Option<Feature> = None;

    if feat_dir.exists() {
        let feat_a_id = mate.feature_a.id.to_string();
        let feat_b_id = mate.feature_b.id.to_string();
        for entry in fs::read_dir(&feat_dir).into_diagnostic()? {
            let entry = entry.into_diagnostic()?;
            let feat_path = entry.path();
            if feat_path.extension().map_or(false, |e| e == "yaml") {
                let filename = feat_path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                if filename.contains(&feat_a_id) {
                    let content = fs::read_to_string(&feat_path).into_diagnostic()?;
                    if let Ok(feat) = serde_yml::from_str::<Feature>(&content) {
                        feat_a = Some(feat);
                    }
                }
                if filename.contains(&feat_b_id) {
                    let content = fs::read_to_string(&feat_path).into_diagnostic()?;
                    if let Ok(feat) = serde_yml::from_str::<Feature>(&content) {
                        feat_b = Some(feat);
                    }
                }
            }
        }
    }

    if feat_a.is_none() || feat_b.is_none() {
        return Err(miette::miette!("Could not find both features to calculate fit"));
    }

    // Calculate fit
    let fit_analysis = calculate_fit_from_features(&feat_a.unwrap(), &feat_b.unwrap());
    mate.fit_analysis = fit_analysis;

    // Write back
    let yaml_content = serde_yml::to_string(&mate).into_diagnostic()?;
    fs::write(&path, &yaml_content).into_diagnostic()?;

    println!(
        "{} Recalculated fit for mate {}",
        style("✓").green(),
        style(&args.id).cyan()
    );

    if let Some(ref analysis) = mate.fit_analysis {
        println!(
            "   Result: {} ({:.4} to {:.4})",
            style(format!("{}", analysis.fit_result)).cyan(),
            analysis.worst_case_min_clearance,
            analysis.worst_case_max_clearance
        );
    } else {
        println!("   Could not calculate fit (features may not have dimensions)");
    }

    Ok(())
}

/// Calculate fit from two feature's primary dimensions
fn calculate_fit_from_features(feat_a: &Feature, feat_b: &Feature) -> Option<FitAnalysis> {
    let dim_a = feat_a.primary_dimension()?;
    let dim_b = feat_b.primary_dimension()?;

    Some(FitAnalysis::calculate(
        (dim_a.nominal, dim_a.plus_tol, dim_a.minus_tol),
        (dim_b.nominal, dim_b.plus_tol, dim_b.minus_tol),
    ))
}

/// Result of checking if fit_result matches mate_type
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum FitMatch {
    Match,     // Fit result matches intended mate type
    Mismatch,  // Fit result doesn't match intended type (warning)
    Unknown,   // No fit analysis available
}

/// Check if a mate's calculated fit_result matches its intended mate_type
fn fit_matches_type(mate: &Mate) -> FitMatch {
    use crate::entities::mate::{FitResult, MateType};

    let Some(ref analysis) = mate.fit_analysis else {
        return FitMatch::Unknown;
    };

    match (mate.mate_type, analysis.fit_result) {
        // Clearance fit should result in clearance
        (MateType::ClearanceFit, FitResult::Clearance) => FitMatch::Match,
        // Interference fit should result in interference
        (MateType::InterferenceFit, FitResult::Interference) => FitMatch::Match,
        // Transition fit can be any result
        (MateType::TransitionFit, _) => FitMatch::Match,
        // Planar and thread don't have fit analysis expectations
        (MateType::PlanarContact, _) => FitMatch::Match,
        (MateType::ThreadEngagement, _) => FitMatch::Match,
        // Any other combination is a mismatch
        _ => FitMatch::Mismatch,
    }
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
