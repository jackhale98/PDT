//! `tdt mate` command - Mate management (1:1 feature contacts with fit calculation)

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
    FeatureA,
    FeatureB,
    FitType,
    Status,
    Author,
    Created,
}

impl std::fmt::Display for ListColumn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ListColumn::Id => write!(f, "id"),
            ListColumn::Title => write!(f, "title"),
            ListColumn::FeatureA => write!(f, "feature-a"),
            ListColumn::FeatureB => write!(f, "feature-b"),
            ListColumn::FitType => write!(f, "fit-type"),
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
    #[arg(long, value_delimiter = ',', default_values_t = vec![ListColumn::Id, ListColumn::Title, ListColumn::FeatureA, ListColumn::FeatureB, ListColumn::FitType, ListColumn::Status])]
    pub columns: Vec<ListColumn>,
}

#[derive(clap::Args, Debug)]
pub struct NewArgs {
    /// First feature ID (REQUIRED) - FEAT@N or full ID (typically hole)
    #[arg(long, short = 'a', required = true)]
    pub feature_a: String,

    /// Second feature ID (REQUIRED) - FEAT@N or full ID (typically shaft)
    #[arg(long, short = 'b', required = true)]
    pub feature_b: String,

    /// Mate type
    #[arg(long, short = 't', default_value = "clearance_fit")]
    pub mate_type: String,

    /// Title/description
    #[arg(long)]
    pub title: Option<String>,

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
            ListColumn::FeatureA => mates.sort_by(|a, b| a.feature_a.cmp(&b.feature_a)),
            ListColumn::FeatureB => mates.sort_by(|a, b| a.feature_b.cmp(&b.feature_b)),
            ListColumn::FitType => mates.sort_by(|a, b| {
                let fit_a = a.fit_analysis.as_ref().map(|f| format!("{}", f.fit_result)).unwrap_or_default();
                let fit_b = b.fit_analysis.as_ref().map(|f| format!("{}", f.fit_result)).unwrap_or_default();
                fit_a.cmp(&fit_b)
            }),
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
            // Build header
            let mut header_parts = Vec::new();
            for col in &args.columns {
                let header = match col {
                    ListColumn::Id => "ID",
                    ListColumn::Title => "TITLE",
                    ListColumn::FeatureA => "FEATURE A",
                    ListColumn::FeatureB => "FEATURE B",
                    ListColumn::FitType => "FIT TYPE",
                    ListColumn::Status => "STATUS",
                    ListColumn::Author => "AUTHOR",
                    ListColumn::Created => "CREATED",
                };
                header_parts.push(format!("{:<20}", style(header).bold()));
            }
            println!("{}", header_parts.join(" "));
            println!("{}", "-".repeat(20 * args.columns.len() + args.columns.len() - 1));

            // Build rows
            for mate in &mates {
                let short_id = short_ids.get_short_id(&mate.id.to_string()).unwrap_or_default();
                let mut row_parts = Vec::new();

                for col in &args.columns {
                    let value = match col {
                        ListColumn::Id => {
                            let id_str = if !short_id.is_empty() {
                                short_id.clone()
                            } else {
                                format_short_id(&mate.id)
                            };
                            format!("{:<20}", style(&id_str).cyan())
                        }
                        ListColumn::Title => {
                            format!("{:<20}", truncate_str(&mate.title, 18))
                        }
                        ListColumn::FeatureA => {
                            format!("{:<20}", truncate_str(&mate.feature_a, 18))
                        }
                        ListColumn::FeatureB => {
                            format!("{:<20}", truncate_str(&mate.feature_b, 18))
                        }
                        ListColumn::FitType => {
                            let fit_result = mate.fit_analysis.as_ref()
                                .map(|a| format!("{}", a.fit_result))
                                .unwrap_or_else(|| "n/a".to_string());
                            format!("{:<20}", fit_result)
                        }
                        ListColumn::Status => {
                            format!("{:<20}", mate.status())
                        }
                        ListColumn::Author => {
                            format!("{:<20}", truncate_str(&mate.author, 18))
                        }
                        ListColumn::Created => {
                            format!("{:<20}", mate.created.format("%Y-%m-%d"))
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

    // Resolve feature IDs
    let short_ids = ShortIdIndex::load(&project);
    let feature_a = short_ids.resolve(&args.feature_a).unwrap_or_else(|| args.feature_a.clone());
    let feature_b = short_ids.resolve(&args.feature_b).unwrap_or_else(|| args.feature_b.clone());

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
            args.feature_a
        ));
    }
    if feat_b.is_none() {
        return Err(miette::miette!(
            "Feature B '{}' not found. Create it first with: tdt feat new",
            args.feature_b
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
        for entry in fs::read_dir(&feat_dir).into_diagnostic()? {
            let entry = entry.into_diagnostic()?;
            let feat_path = entry.path();
            if feat_path.extension().map_or(false, |e| e == "yaml") {
                let filename = feat_path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                if filename.contains(&mate.feature_a) {
                    let content = fs::read_to_string(&feat_path).into_diagnostic()?;
                    if let Ok(feat) = serde_yml::from_str::<Feature>(&content) {
                        feat_a = Some(feat);
                    }
                }
                if filename.contains(&mate.feature_b) {
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
