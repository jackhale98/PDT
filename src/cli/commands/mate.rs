//! `tdt mate` command - Mate management (1:1 feature contacts with fit calculation)

use clap::{Subcommand, ValueEnum};
use console::style;
use miette::{bail, IntoDiagnostic, Result};
use std::fs;

use crate::cli::commands::utils::format_link_with_title;
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

    /// Delete a mate
    Delete(DeleteArgs),

    /// Archive a mate (soft delete)
    Archive(ArchiveArgs),

    /// Recalculate fit analysis from current feature dimensions
    Recalc(RecalcArgs),

    /// Recalculate all mates (refresh cached data and fit analysis)
    RecalcAll(RecalcAllArgs),
}

/// Mate type for CLI
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CliMateType {
    /// Clearance fit - guaranteed gap between parts
    Clearance,
    /// Transition fit - may be clearance or interference
    Transition,
    /// Interference fit - press fit
    Interference,
}

impl From<CliMateType> for MateType {
    fn from(cli: CliMateType) -> Self {
        match cli {
            CliMateType::Clearance => MateType::Clearance,
            CliMateType::Transition => MateType::Transition,
            CliMateType::Interference => MateType::Interference,
        }
    }
}

/// Mate type filter
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum TypeFilter {
    Clearance,
    Transition,
    Interference,
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

/// Column definitions for mate list output
const MATE_COLUMNS: &[ColumnDef] = &[
    ColumnDef::new("id", "ID", 30),
    ColumnDef::new("title", "TITLE", 30),
    ColumnDef::new("mate-type", "TYPE", 16),
    ColumnDef::new("fit-result", "FIT", 12),
    ColumnDef::new("match", "OK", 4),
    ColumnDef::new("feature-a", "FEATURE A", 25),
    ColumnDef::new("feature-b", "FEATURE B", 25),
    ColumnDef::new("status", "STATUS", 10),
    ColumnDef::new("author", "AUTHOR", 20),
    ColumnDef::new("created", "CREATED", 16),
];

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
    #[arg(long, value_delimiter = ',', default_values_t = vec![ListColumn::Title, ListColumn::MateType, ListColumn::FitResult, ListColumn::Match, ListColumn::Status])]
    pub columns: Vec<ListColumn>,

    /// Show full ID column (hidden by default since SHORT is always shown)
    #[arg(long)]
    pub show_id: bool,

    /// Wrap text in columns (mobile-friendly output with specified width)
    #[arg(long, short = 'w')]
    pub wrap: Option<usize>,
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
    #[arg(long, short = 't', value_enum, default_value = "clearance")]
    pub mate_type: CliMateType,

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

    /// Link to another entity (auto-infers link type)
    #[arg(long, short = 'L')]
    pub link: Vec<String>,
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
pub struct DeleteArgs {
    /// Mate ID or short ID (MATE@N)
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
    /// Mate ID or short ID (MATE@N)
    pub id: String,

    /// Force archive even if other entities reference this one
    #[arg(long)]
    pub force: bool,

    /// Suppress output
    #[arg(long, short = 'q')]
    pub quiet: bool,
}

/// Directories where mates are stored
const MATE_DIRS: &[&str] = &["tolerances/mates"];

/// Entity configuration for mates
const ENTITY_CONFIG: crate::cli::EntityConfig = crate::cli::EntityConfig {
    prefix: EntityPrefix::Mate,
    dirs: MATE_DIRS,
    name: "mate",
    name_plural: "mates",
};

#[derive(clap::Args, Debug)]
pub struct RecalcArgs {
    /// Mate ID or short ID (MATE@N)
    pub id: String,
}

#[derive(clap::Args, Debug)]
pub struct RecalcAllArgs {
    /// Only show what would be updated (don't modify files)
    #[arg(long)]
    pub dry_run: bool,
}

/// Run a mate subcommand
pub fn run(cmd: MateCommands, global: &GlobalOpts) -> Result<()> {
    match cmd {
        MateCommands::List(args) => run_list(args, global),
        MateCommands::New(args) => run_new(args, global),
        MateCommands::Show(args) => run_show(args, global),
        MateCommands::Edit(args) => run_edit(args),
        MateCommands::Delete(args) => run_delete(args),
        MateCommands::Archive(args) => run_archive(args),
        MateCommands::Recalc(args) => run_recalc(args),
        MateCommands::RecalcAll(args) => run_recalc_all(args),
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

        if path.extension().is_some_and(|e| e == "yaml") {
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
            TypeFilter::Clearance => m.mate_type == MateType::Clearance,
            TypeFilter::Transition => m.mate_type == MateType::Transition,
            TypeFilter::Interference => m.mate_type == MateType::Interference,
            TypeFilter::All => true,
        })
        .filter(|m| match args.status {
            StatusFilter::Draft => m.status == crate::core::entity::Status::Draft,
            StatusFilter::Review => m.status == crate::core::entity::Status::Review,
            StatusFilter::Approved => m.status == crate::core::entity::Status::Approved,
            StatusFilter::Released => m.status == crate::core::entity::Status::Released,
            StatusFilter::Obsolete => m.status == crate::core::entity::Status::Obsolete,
            StatusFilter::Active => m.status != crate::core::entity::Status::Obsolete,
            StatusFilter::All => true,
        })
        .filter(|m| {
            if let Some(ref search) = args.search {
                let search_lower = search.to_lowercase();
                m.title.to_lowercase().contains(&search_lower)
                    || m.description
                        .as_ref()
                        .is_some_and(|d| d.to_lowercase().contains(&search_lower))
            } else {
                true
            }
        })
        .filter(|m| {
            args.author
                .as_ref()
                .is_none_or(|a| m.author.to_lowercase().contains(&a.to_lowercase()))
        })
        .filter(|m| {
            args.recent.is_none_or(|days| {
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
            ListColumn::MateType => {
                mates.sort_by(|a, b| format!("{}", a.mate_type).cmp(&format!("{}", b.mate_type)))
            }
            ListColumn::FitResult => mates.sort_by(|a, b| {
                let fit_a = a
                    .fit_analysis
                    .as_ref()
                    .map(|f| format!("{}", f.fit_result))
                    .unwrap_or_default();
                let fit_b = b
                    .fit_analysis
                    .as_ref()
                    .map(|f| format!("{}", f.fit_result))
                    .unwrap_or_default();
                fit_a.cmp(&fit_b)
            }),
            ListColumn::Match => mates.sort_by(|a, b| {
                let match_a = fit_matches_type(a);
                let match_b = fit_matches_type(b);
                match_a.cmp(&match_b)
            }),
            ListColumn::FeatureA => {
                mates.sort_by(|a, b| a.feature_a.id.to_string().cmp(&b.feature_a.id.to_string()))
            }
            ListColumn::FeatureB => {
                mates.sort_by(|a, b| a.feature_b.id.to_string().cmp(&b.feature_b.id.to_string()))
            }
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
    super::utils::save_short_ids(&mut short_ids, &project);

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
        OutputFormat::Csv | OutputFormat::Tsv | OutputFormat::Md => {
            let mut columns: Vec<&str> = args
                .columns
                .iter()
                .map(|c| c.to_string().leak() as &str)
                .collect();
            if args.show_id && !columns.contains(&"id") {
                columns.insert(0, "id");
            }
            let rows: Vec<TableRow> = mates.iter().map(|m| mate_to_row(m, &short_ids)).collect();

            let config = TableConfig {
                wrap_width: args.wrap,
                show_summary: true,
            };
            let formatter = TableFormatter::new(MATE_COLUMNS, "mate", "MATE").with_config(config);
            formatter.output(rows, format, &columns);
        }
        OutputFormat::Id | OutputFormat::ShortId => {
            for mate in &mates {
                if format == OutputFormat::ShortId {
                    let short_id = short_ids
                        .get_short_id(&mate.id.to_string())
                        .unwrap_or_default();
                    println!("{}", short_id);
                } else {
                    println!("{}", mate.id);
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
    let feature_a = short_ids
        .resolve(&feat_a_input)
        .unwrap_or_else(|| feat_a_input.clone());
    let feature_b = short_ids
        .resolve(&feat_b_input)
        .unwrap_or_else(|| feat_b_input.clone());

    // Validate features exist and load them for fit calculation
    let feat_dir = project.root().join("tolerances/features");
    let mut feat_a: Option<Feature> = None;
    let mut feat_b: Option<Feature> = None;

    if feat_dir.exists() {
        for entry in fs::read_dir(&feat_dir).into_diagnostic()? {
            let entry = entry.into_diagnostic()?;
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "yaml") {
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
    let mate_type: MateType;
    let description: Option<String>;
    let notes: Option<String>;

    if args.interactive {
        let wizard = SchemaWizard::new();
        let result = wizard.run(EntityPrefix::Mate)?;

        title = result
            .get_string("title")
            .map(String::from)
            .unwrap_or_else(|| "New Mate".to_string());
        mate_type = result
            .get_string("mate_type")
            .and_then(|s| s.parse().ok())
            .unwrap_or(MateType::Clearance);
        description = result.get_string("description").map(String::from);
        notes = result.get_string("notes").map(String::from);
    } else {
        title = args.title.unwrap_or_else(|| "New Mate".to_string());
        mate_type = args.mate_type.into();
        description = None;
        notes = None;
    }

    // Generate ID
    let id = EntityId::new(EntityPrefix::Mate);

    // Generate template
    let generator = TemplateGenerator::new().map_err(|e| miette::miette!("{}", e))?;
    let ctx = TemplateContext::new(id.clone(), config.author())
        .with_title(&title)
        .with_feature_a(&feature_a)
        .with_feature_b(&feature_b)
        .with_mate_type(mate_type.to_string());

    let yaml_content = generator
        .generate_mate(&ctx)
        .map_err(|e| miette::miette!("{}", e))?;

    // Try to calculate fit if both features have dimensions
    let fit_analysis = calculate_fit_from_features(&feat_a.unwrap(), &feat_b.unwrap());

    // Parse and update with fit analysis and wizard values
    let mut mate: Mate = serde_yml::from_str(&yaml_content).into_diagnostic()?;
    mate.fit_analysis = fit_analysis;
    if args.interactive {
        if let Some(ref desc) = description {
            if !desc.is_empty() {
                mate.description = Some(desc.clone());
            }
        }
        if let Some(ref n) = notes {
            if !n.is_empty() {
                mate.notes = Some(n.clone());
            }
        }
    }
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
    super::utils::save_short_ids(&mut short_ids, &project);

    // Handle --link flags
    let added_links = crate::cli::entity_cmd::process_link_flags(
        &file_path,
        EntityPrefix::Mate,
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
                "{} Created mate {}",
                style("✓").green(),
                style(short_id.clone().unwrap_or_else(|| format_short_id(&id))).cyan()
            );
            println!("   {}", style(file_path.display()).dim());
            println!(
                "   {} <-> {} | {}",
                style(truncate_str(&feature_a, 13)).yellow(),
                style(truncate_str(&feature_b, 13)).yellow(),
                style(&title).white()
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

            // Show fit analysis if calculated
            if let Some(ref analysis) = mate.fit_analysis {
                // Use the clearance magnitude to determine precision for display
                let ref_precision = analysis
                    .worst_case_min_clearance
                    .abs()
                    .max(analysis.worst_case_max_clearance.abs())
                    .max(0.001); // Minimum precision for tiny values
                let min_rounded = smart_round(analysis.worst_case_min_clearance, ref_precision);
                let max_rounded = smart_round(analysis.worst_case_max_clearance, ref_precision);

                println!();
                println!("   Fit Analysis:");
                println!(
                    "     Result: {} ({} to {})",
                    style(format!("{}", analysis.fit_result)).cyan(),
                    min_rounded,
                    max_rounded
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

    // Find the mate file
    let mate_dir = project.root().join("tolerances/mates");
    let mut found_path = None;

    if mate_dir.exists() {
        for entry in fs::read_dir(&mate_dir).into_diagnostic()? {
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

    let path = found_path.ok_or_else(|| miette::miette!("No mate found matching '{}'", args.id))?;

    // Read and parse mate
    let content = fs::read_to_string(&path).into_diagnostic()?;
    let mate: Mate = serde_yml::from_str(&content).into_diagnostic()?;

    match global.format {
        OutputFormat::Yaml => {
            print!("{}", content);
        }
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&mate).into_diagnostic()?;
            println!("{}", json);
        }
        OutputFormat::Id | OutputFormat::ShortId => {
            if global.format == OutputFormat::ShortId {
                let sid_index = ShortIdIndex::load(&project);
                let short_id = sid_index
                    .get_short_id(&mate.id.to_string())
                    .unwrap_or_default();
                println!("{}", short_id);
            } else {
                println!("{}", mate.id);
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
                style(&mate.id.to_string()).cyan()
            );
            println!("{}: {}", style("Title").bold(), style(&mate.title).yellow());
            println!("{}: {}", style("Type").bold(), mate.mate_type);
            println!("{}: {}", style("Status").bold(), mate.status);
            println!("{}", style("─".repeat(60)).dim());

            // Features
            println!();
            println!("{}", style("Mating Features:").bold());
            let feat_a_display =
                format_link_with_title(&mate.feature_a.id.to_string(), &short_ids, &cache);
            let feat_b_display =
                format_link_with_title(&mate.feature_b.id.to_string(), &short_ids, &cache);

            // Helper to get component display with part number and title
            let get_component_display =
                |cmp_id: Option<&String>, cmp_name: Option<&String>| -> Option<String> {
                    if let Some(cmp_id) = cmp_id {
                        // Look up component info from cache
                        if let Some(ref c) = cache {
                            let components = c.list_components(None, None, None, None, None, None);
                            if let Some(cmp) = components.iter().find(|c| &c.id == cmp_id) {
                                let short = short_ids
                                    .get_short_id(cmp_id)
                                    .unwrap_or_else(|| cmp_id.clone());
                                match (&cmp.part_number, cmp.title.as_str()) {
                                    (Some(pn), title) if !pn.is_empty() && !title.is_empty() => {
                                        return Some(format!("{} ({}) {}", short, pn, title));
                                    }
                                    (Some(pn), _) if !pn.is_empty() => {
                                        return Some(format!("{} ({})", short, pn));
                                    }
                                    (_, title) if !title.is_empty() => {
                                        return Some(format!("{} ({})", short, title));
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                    // Fall back to component_name if available
                    cmp_name.cloned()
                };

            println!("  Feature A: {}", style(&feat_a_display).cyan());
            if let Some(display) = get_component_display(
                mate.feature_a.component_id.as_ref(),
                mate.feature_a.component_name.as_ref(),
            ) {
                println!("             Component: {}", style(&display).dim());
            }

            println!("  Feature B: {}", style(&feat_b_display).cyan());
            if let Some(display) = get_component_display(
                mate.feature_b.component_id.as_ref(),
                mate.feature_b.component_name.as_ref(),
            ) {
                println!("             Component: {}", style(&display).dim());
            }

            // Fit Analysis - compute fresh from features for accurate display
            let computed_fit = compute_mate_fit(&project, &mate);
            let display_fit = computed_fit.as_ref().or(mate.fit_analysis.as_ref());

            if let Some(fit) = display_fit {
                // Use the clearance magnitude to determine precision for display
                let ref_precision = fit
                    .worst_case_min_clearance
                    .abs()
                    .max(fit.worst_case_max_clearance.abs())
                    .max(0.001);
                let min_rounded = smart_round(fit.worst_case_min_clearance, ref_precision);
                let max_rounded = smart_round(fit.worst_case_max_clearance, ref_precision);

                println!();
                println!("{}", style("Fit Analysis:").bold());
                let fit_color = match fit.fit_result {
                    crate::entities::mate::FitResult::Clearance => style("CLEARANCE").green(),
                    crate::entities::mate::FitResult::Interference => style("INTERFERENCE").red(),
                    crate::entities::mate::FitResult::Transition => style("TRANSITION").yellow(),
                };
                println!("  {}: {}", style("Fit Type").dim(), fit_color);
                println!("  {}: {} mm", style("Min Clearance").dim(), min_rounded);
                println!("  {}: {} mm", style("Max Clearance").dim(), max_rounded);

                // Warn if stored fit differs from computed fit
                if let (Some(stored), Some(computed)) = (&mate.fit_analysis, &computed_fit) {
                    if stored.fit_result != computed.fit_result
                        || (stored.worst_case_min_clearance - computed.worst_case_min_clearance)
                            .abs()
                            > 0.0001
                        || (stored.worst_case_max_clearance - computed.worst_case_max_clearance)
                            .abs()
                            > 0.0001
                    {
                        println!(
                            "  {}",
                            style(
                                "⚠ Stored fit differs from computed - run 'mate recalc' to update"
                            )
                            .yellow()
                        );
                    }
                }
            }

            // Tags
            if !mate.tags.is_empty() {
                println!();
                println!("{}: {}", style("Tags").bold(), mate.tags.join(", "));
            }

            // Description
            if let Some(ref desc) = mate.description {
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
                mate.author,
                style("Created").dim(),
                mate.created.format("%Y-%m-%d %H:%M"),
                style("Revision").dim(),
                mate.entity_revision
            );
        }
    }

    Ok(())
}

fn run_edit(args: EditArgs) -> Result<()> {
    crate::cli::entity_cmd::run_edit_generic(&args.id, &ENTITY_CONFIG)
}

fn run_delete(args: DeleteArgs) -> Result<()> {
    crate::cli::commands::utils::run_delete(&args.id, MATE_DIRS, args.force, false, args.quiet)
}

fn run_archive(args: ArchiveArgs) -> Result<()> {
    crate::cli::commands::utils::run_delete(&args.id, MATE_DIRS, args.force, true, args.quiet)
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

            if path.extension().is_some_and(|e| e == "yaml") {
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
            if feat_path.extension().is_some_and(|e| e == "yaml") {
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
        return Err(miette::miette!(
            "Could not find both features to calculate fit"
        ));
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
        // Use the clearance magnitude to determine precision for display
        let ref_precision = analysis
            .worst_case_min_clearance
            .abs()
            .max(analysis.worst_case_max_clearance.abs())
            .max(0.001);
        let min_rounded = smart_round(analysis.worst_case_min_clearance, ref_precision);
        let max_rounded = smart_round(analysis.worst_case_max_clearance, ref_precision);

        println!(
            "   Result: {} ({} to {})",
            style(format!("{}", analysis.fit_result)).cyan(),
            min_rounded,
            max_rounded
        );
    } else {
        println!("   Could not calculate fit (features may not have dimensions)");
    }

    Ok(())
}

fn run_recalc_all(args: RecalcAllArgs) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let mate_dir = project.root().join("tolerances/mates");
    let feat_dir = project.root().join("tolerances/features");

    if !mate_dir.exists() {
        println!("No mates directory found.");
        return Ok(());
    }

    // Load all features into a map for quick lookup
    let mut features: std::collections::HashMap<String, Feature> = std::collections::HashMap::new();
    if feat_dir.exists() {
        for entry in fs::read_dir(&feat_dir).into_diagnostic()? {
            let entry = entry.into_diagnostic()?;
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "yaml") {
                let content = fs::read_to_string(&path).into_diagnostic()?;
                if let Ok(feat) = serde_yml::from_str::<Feature>(&content) {
                    features.insert(feat.id.to_string(), feat);
                }
            }
        }
    }

    // Load all components for cached data
    let cmp_dir = project.root().join("bom/components");
    let mut components: std::collections::HashMap<String, (String, String)> =
        std::collections::HashMap::new(); // id -> (id, title)
    if cmp_dir.exists() {
        for entry in fs::read_dir(&cmp_dir).into_diagnostic()? {
            let entry = entry.into_diagnostic()?;
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "yaml") {
                let content = fs::read_to_string(&path).into_diagnostic()?;
                if let Ok(value) = serde_yml::from_str::<serde_yml::Value>(&content) {
                    if let (Some(id), Some(title)) = (
                        value.get("id").and_then(|v| v.as_str()),
                        value.get("title").and_then(|v| v.as_str()),
                    ) {
                        components.insert(id.to_string(), (id.to_string(), title.to_string()));
                    }
                }
            }
        }
    }

    // Process all mates
    let mut updated = 0;
    let mut skipped = 0;
    let mut errors = 0;

    let short_ids = ShortIdIndex::load(&project);

    for entry in fs::read_dir(&mate_dir).into_diagnostic()? {
        let entry = entry.into_diagnostic()?;
        let path = entry.path();

        if path.extension().is_none_or(|e| e != "yaml") {
            continue;
        }

        let content = fs::read_to_string(&path).into_diagnostic()?;
        let mut mate: Mate = match serde_yml::from_str(&content) {
            Ok(m) => m,
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
            .get_short_id(&mate.id.to_string())
            .unwrap_or_else(|| format_short_id(&mate.id));

        // Look up features
        let feat_a_id = mate.feature_a.id.to_string();
        let feat_b_id = mate.feature_b.id.to_string();

        let feat_a = features.get(&feat_a_id);
        let feat_b = features.get(&feat_b_id);

        if feat_a.is_none() || feat_b.is_none() {
            if args.dry_run {
                println!(
                    "{} {} - missing feature(s)",
                    style("⚠").yellow(),
                    style(&short_id).cyan()
                );
            }
            skipped += 1;
            continue;
        }

        let feat_a = feat_a.unwrap();
        let feat_b = feat_b.unwrap();

        // Update cached feature data
        let mut changed = false;

        // Update feature_a cached data
        if mate.feature_a.name.as_ref() != Some(&feat_a.title) {
            mate.feature_a.name = Some(feat_a.title.clone());
            changed = true;
        }
        let cmp_a_id = &feat_a.component;
        if mate.feature_a.component_id.as_ref() != Some(cmp_a_id) {
            mate.feature_a.component_id = Some(cmp_a_id.clone());
            changed = true;
        }
        if let Some((_, cmp_title)) = components.get(cmp_a_id) {
            if mate.feature_a.component_name.as_ref() != Some(cmp_title) {
                mate.feature_a.component_name = Some(cmp_title.clone());
                changed = true;
            }
        }

        // Update feature_b cached data
        if mate.feature_b.name.as_ref() != Some(&feat_b.title) {
            mate.feature_b.name = Some(feat_b.title.clone());
            changed = true;
        }
        let cmp_b_id = &feat_b.component;
        if mate.feature_b.component_id.as_ref() != Some(cmp_b_id) {
            mate.feature_b.component_id = Some(cmp_b_id.clone());
            changed = true;
        }
        if let Some((_, cmp_title)) = components.get(cmp_b_id) {
            if mate.feature_b.component_name.as_ref() != Some(cmp_title) {
                mate.feature_b.component_name = Some(cmp_title.clone());
                changed = true;
            }
        }

        // Recalculate fit analysis
        let new_fit = calculate_fit_from_features(feat_a, feat_b);
        if mate.fit_analysis != new_fit {
            mate.fit_analysis = new_fit;
            changed = true;
        }

        if changed {
            if args.dry_run {
                println!(
                    "{} {} - would update ({} <-> {})",
                    style("→").blue(),
                    style(&short_id).cyan(),
                    feat_a.title,
                    feat_b.title
                );
            } else {
                // Write back
                let yaml_content = serde_yml::to_string(&mate).into_diagnostic()?;
                fs::write(&path, &yaml_content).into_diagnostic()?;
                println!(
                    "{} {} - updated ({} <-> {})",
                    style("✓").green(),
                    style(&short_id).cyan(),
                    feat_a.title,
                    feat_b.title
                );
            }
            updated += 1;
        } else {
            skipped += 1;
        }
    }

    println!();
    if args.dry_run {
        println!(
            "{} {} mate(s) would be updated, {} already current, {} error(s)",
            style("Dry run:").bold(),
            style(updated).cyan(),
            skipped,
            errors
        );
    } else {
        println!(
            "{} Updated {} mate(s), {} already current, {} error(s)",
            style("Done:").bold(),
            style(updated).green(),
            skipped,
            errors
        );
    }

    Ok(())
}

/// Calculate fit from two feature's primary dimensions
/// Auto-detects which feature is hole vs shaft based on the `internal` field
fn calculate_fit_from_features(feat_a: &Feature, feat_b: &Feature) -> Option<FitAnalysis> {
    let dim_a = feat_a.primary_dimension()?;
    let dim_b = feat_b.primary_dimension()?;

    // Use from_dimensions which auto-detects hole/shaft based on internal field
    FitAnalysis::from_dimensions(dim_a, dim_b).ok()
}

/// Load a feature by ID from the project
fn load_feature(project: &Project, feature_id: &str) -> Option<Feature> {
    let feat_dir = project.root().join("tolerances/features");
    if !feat_dir.exists() {
        return None;
    }

    for entry in fs::read_dir(&feat_dir).ok()? {
        let entry = entry.ok()?;
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "yaml") {
            let filename = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            if filename.contains(feature_id) {
                let content = fs::read_to_string(&path).ok()?;
                if let Ok(feat) = serde_yml::from_str::<Feature>(&content) {
                    if feat.id.to_string() == feature_id {
                        return Some(feat);
                    }
                }
            }
        }
    }
    None
}

/// Compute fresh fit_analysis from a mate's linked features.
/// Returns None if features can't be loaded or have no primary dimensions.
fn compute_mate_fit(project: &Project, mate: &Mate) -> Option<FitAnalysis> {
    let feat_a = load_feature(project, &mate.feature_a.id.to_string())?;
    let feat_b = load_feature(project, &mate.feature_b.id.to_string())?;
    calculate_fit_from_features(&feat_a, &feat_b)
}

/// Result of checking if fit_result matches mate_type
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum FitMatch {
    Match,    // Fit result matches intended mate type
    Mismatch, // Fit result doesn't match intended type (warning)
    Unknown,  // No fit analysis available
}

/// Check if a mate's calculated fit_result matches its intended mate_type
fn fit_matches_type(mate: &Mate) -> FitMatch {
    use crate::entities::mate::{FitResult, MateType};

    let Some(ref analysis) = mate.fit_analysis else {
        return FitMatch::Unknown;
    };

    match (mate.mate_type, analysis.fit_result) {
        // Clearance fit should result in clearance
        (MateType::Clearance, FitResult::Clearance) => FitMatch::Match,
        // Interference fit should result in interference
        (MateType::Interference, FitResult::Interference) => FitMatch::Match,
        // Transition fit can be any result
        (MateType::Transition, _) => FitMatch::Match,
        // Any other combination is a mismatch
        _ => FitMatch::Mismatch,
    }
}

/// Convert a Mate entity to a TableRow
fn mate_to_row(mate: &Mate, short_ids: &ShortIdIndex) -> TableRow {
    let fit_result = mate
        .fit_analysis
        .as_ref()
        .map(|a| a.fit_result.to_string())
        .unwrap_or_else(|| "n/a".to_string());

    let fit_match = match fit_matches_type(mate) {
        FitMatch::Match => "match",
        FitMatch::Mismatch => "mismatch",
        FitMatch::Unknown => "unknown",
    };

    let feature_a_display = mate
        .feature_a
        .name
        .clone()
        .or_else(|| short_ids.get_short_id(&mate.feature_a.id.to_string()))
        .unwrap_or_else(|| mate.feature_a.id.to_string());

    let feature_b_display = mate
        .feature_b
        .name
        .clone()
        .or_else(|| short_ids.get_short_id(&mate.feature_b.id.to_string()))
        .unwrap_or_else(|| mate.feature_b.id.to_string());

    TableRow::new(mate.id.to_string(), short_ids)
        .cell("id", CellValue::Id(mate.id.to_string()))
        .cell("title", CellValue::Text(mate.title.clone()))
        .cell("mate-type", CellValue::Type(mate.mate_type.to_string()))
        .cell("fit-result", CellValue::FitResult(fit_result))
        .cell("match", CellValue::FitMatch(fit_match.to_string()))
        .cell("feature-a", CellValue::Text(feature_a_display))
        .cell("feature-b", CellValue::Text(feature_b_display))
        .cell("status", CellValue::Type(mate.status().to_string()))
        .cell("author", CellValue::Text(mate.author.clone()))
        .cell("created", CellValue::DateTime(mate.created))
}
