//! `tdt feat` command - Feature management (dimensional features on components)

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
use crate::entities::feature::{Feature, FeatureType};
use crate::schema::template::{TemplateContext, TemplateGenerator};
use crate::schema::wizard::SchemaWizard;

#[derive(Subcommand, Debug)]
pub enum FeatCommands {
    /// List features with filtering
    List(ListArgs),

    /// Create a new feature (requires --component)
    New(NewArgs),

    /// Show a feature's details
    Show(ShowArgs),

    /// Edit a feature in your editor
    Edit(EditArgs),
}

/// Feature type filter
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum TypeFilter {
    Hole,
    Shaft,
    PlanarSurface,
    Slot,
    Thread,
    Counterbore,
    Countersink,
    Boss,
    Pocket,
    Edge,
    Other,
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

/// Columns to display in list output
#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
pub enum ListColumn {
    Id,
    Title,
    Description,
    FeatureType,
    Component,
    Status,
    Author,
    Created,
}

impl std::fmt::Display for ListColumn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ListColumn::Id => write!(f, "id"),
            ListColumn::Title => write!(f, "title"),
            ListColumn::Description => write!(f, "description"),
            ListColumn::FeatureType => write!(f, "feature-type"),
            ListColumn::Component => write!(f, "component"),
            ListColumn::Status => write!(f, "status"),
            ListColumn::Author => write!(f, "author"),
            ListColumn::Created => write!(f, "created"),
        }
    }
}

#[derive(clap::Args, Debug)]
pub struct ListArgs {
    /// Filter by parent component (CMP@N or full ID)
    #[arg(long, short = 'c')]
    pub component: Option<String>,

    /// Filter by feature type
    #[arg(long, short = 't', default_value = "all")]
    pub feature_type: TypeFilter,

    /// Filter by status
    #[arg(long, short = 's', default_value = "all")]
    pub status: StatusFilter,

    /// Search in title
    #[arg(long)]
    pub search: Option<String>,

    /// Filter by author (substring match)
    #[arg(long, short = 'a')]
    pub author: Option<String>,

    /// Show features created in last N days
    #[arg(long)]
    pub recent: Option<u32>,

    /// Columns to display (can specify multiple)
    #[arg(long, value_delimiter = ',', default_values_t = vec![
        ListColumn::Id,
        ListColumn::Title,
        ListColumn::FeatureType,
        ListColumn::Component,
        ListColumn::Status
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
    /// Parent component ID (REQUIRED) - CMP@N or full ID
    #[arg(long, short = 'c', required = true)]
    pub component: String,

    /// Feature type
    #[arg(long, short = 't', default_value = "hole")]
    pub feature_type: String,

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
    /// Feature ID or short ID (FEAT@N)
    pub id: String,
}

#[derive(clap::Args, Debug)]
pub struct EditArgs {
    /// Feature ID or short ID (FEAT@N)
    pub id: String,
}

/// Run a feature subcommand
pub fn run(cmd: FeatCommands, global: &GlobalOpts) -> Result<()> {
    match cmd {
        FeatCommands::List(args) => run_list(args, global),
        FeatCommands::New(args) => run_new(args),
        FeatCommands::Show(args) => run_show(args, global),
        FeatCommands::Edit(args) => run_edit(args),
    }
}

fn run_list(args: ListArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let feat_dir = project.root().join("tolerances/features");

    if !feat_dir.exists() {
        if args.count {
            println!("0");
        } else {
            println!("No features found.");
        }
        return Ok(());
    }

    // Resolve component filter if provided
    let short_ids = ShortIdIndex::load(&project);
    let component_filter = args.component.as_ref().map(|c| {
        short_ids.resolve(c).unwrap_or_else(|| c.clone())
    });

    // Load and parse all features
    let mut features: Vec<Feature> = Vec::new();

    for entry in fs::read_dir(&feat_dir).into_diagnostic()? {
        let entry = entry.into_diagnostic()?;
        let path = entry.path();

        if path.extension().map_or(false, |e| e == "yaml") {
            let content = fs::read_to_string(&path).into_diagnostic()?;
            if let Ok(feat) = serde_yml::from_str::<Feature>(&content) {
                features.push(feat);
            }
        }
    }

    // Apply filters
    let features: Vec<Feature> = features
        .into_iter()
        .filter(|f| {
            if let Some(ref cmp_id) = component_filter {
                f.component.contains(cmp_id) || f.component == *cmp_id
            } else {
                true
            }
        })
        .filter(|f| match args.feature_type {
            TypeFilter::Hole => f.feature_type == FeatureType::Hole,
            TypeFilter::Shaft => f.feature_type == FeatureType::Shaft,
            TypeFilter::PlanarSurface => f.feature_type == FeatureType::PlanarSurface,
            TypeFilter::Slot => f.feature_type == FeatureType::Slot,
            TypeFilter::Thread => f.feature_type == FeatureType::Thread,
            TypeFilter::Counterbore => f.feature_type == FeatureType::Counterbore,
            TypeFilter::Countersink => f.feature_type == FeatureType::Countersink,
            TypeFilter::Boss => f.feature_type == FeatureType::Boss,
            TypeFilter::Pocket => f.feature_type == FeatureType::Pocket,
            TypeFilter::Edge => f.feature_type == FeatureType::Edge,
            TypeFilter::Other => f.feature_type == FeatureType::Other,
            TypeFilter::All => true,
        })
        .filter(|f| match args.status {
            StatusFilter::Draft => f.status == crate::core::entity::Status::Draft,
            StatusFilter::Review => f.status == crate::core::entity::Status::Review,
            StatusFilter::Approved => f.status == crate::core::entity::Status::Approved,
            StatusFilter::Released => f.status == crate::core::entity::Status::Released,
            StatusFilter::Obsolete => f.status == crate::core::entity::Status::Obsolete,
            StatusFilter::All => true,
        })
        .filter(|f| {
            if let Some(ref search) = args.search {
                let search_lower = search.to_lowercase();
                f.title.to_lowercase().contains(&search_lower)
                    || f.description
                        .as_ref()
                        .map_or(false, |d| d.to_lowercase().contains(&search_lower))
            } else {
                true
            }
        })
        .filter(|f| {
            args.author.as_ref().map_or(true, |author| {
                f.author.to_lowercase().contains(&author.to_lowercase())
            })
        })
        .filter(|f| {
            args.recent.map_or(true, |days| {
                let cutoff = chrono::Utc::now() - chrono::Duration::days(days as i64);
                f.created >= cutoff
            })
        })
        .collect();

    // Sort
    let mut features = features;
    match args.sort {
        ListColumn::Id => features.sort_by(|a, b| a.id.to_string().cmp(&b.id.to_string())),
        ListColumn::Title => features.sort_by(|a, b| a.title.cmp(&b.title)),
        ListColumn::Description => features.sort_by(|a, b| {
            a.description.as_deref().unwrap_or("").cmp(b.description.as_deref().unwrap_or(""))
        }),
        ListColumn::FeatureType => features.sort_by(|a, b| {
            format!("{:?}", a.feature_type).cmp(&format!("{:?}", b.feature_type))
        }),
        ListColumn::Component => features.sort_by(|a, b| a.component.cmp(&b.component)),
        ListColumn::Status => features.sort_by(|a, b| {
            format!("{:?}", a.status).cmp(&format!("{:?}", b.status))
        }),
        ListColumn::Author => features.sort_by(|a, b| a.author.cmp(&b.author)),
        ListColumn::Created => features.sort_by(|a, b| a.created.cmp(&b.created)),
    }

    if args.reverse {
        features.reverse();
    }

    // Apply limit
    if let Some(limit) = args.limit {
        features.truncate(limit);
    }

    // Count only
    if args.count {
        println!("{}", features.len());
        return Ok(());
    }

    // No results
    if features.is_empty() {
        println!("No features found.");
        return Ok(());
    }

    // Update short ID index
    let mut short_ids = ShortIdIndex::load(&project);
    short_ids.ensure_all(features.iter().map(|f| f.id.to_string()));
    let _ = short_ids.save(&project);

    // Output based on format
    let format = match global.format {
        OutputFormat::Auto => OutputFormat::Tsv,
        f => f,
    };

    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&features).into_diagnostic()?;
            println!("{}", json);
        }
        OutputFormat::Yaml => {
            let yaml = serde_yml::to_string(&features).into_diagnostic()?;
            print!("{}", yaml);
        }
        OutputFormat::Csv => {
            println!("short_id,id,component,feature_type,title,dims,status");
            for feat in &features {
                let short_id = short_ids.get_short_id(&feat.id.to_string()).unwrap_or_default();
                println!(
                    "{},{},{},{},{},{},{}",
                    short_id,
                    feat.id,
                    truncate_str(&feat.component, 13),
                    feat.feature_type,
                    escape_csv(&feat.title),
                    feat.dimensions.len(),
                    feat.status()
                );
            }
        }
        OutputFormat::Tsv => {
            // Build header based on selected columns
            let mut header_parts = vec![format!("{:<8}", style("SHORT").bold().dim())];
            let mut widths = vec![8usize];
            for col in &args.columns {
                let (header, width) = match col {
                    ListColumn::Id => (style("ID").bold().to_string(), 17),
                    ListColumn::Title => (style("TITLE").bold().to_string(), 20),
                    ListColumn::Description => (style("DESCRIPTION").bold().to_string(), 30),
                    ListColumn::FeatureType => (style("TYPE").bold().to_string(), 10),
                    ListColumn::Component => (style("COMPONENT").bold().to_string(), 8),
                    ListColumn::Status => (style("STATUS").bold().to_string(), 10),
                    ListColumn::Author => (style("AUTHOR").bold().to_string(), 14),
                    ListColumn::Created => (style("CREATED").bold().to_string(), 12),
                };
                header_parts.push(format!("{:<width$}", header, width = width));
                widths.push(width);
            }
            println!("{}", header_parts.join(" "));
            println!("{}", "-".repeat(widths.iter().sum::<usize>() + widths.len() - 1));

            for feat in &features {
                let short_id = short_ids.get_short_id(&feat.id.to_string()).unwrap_or_default();
                let mut row_parts = vec![format!("{:<8}", style(&short_id).cyan())];

                for (i, col) in args.columns.iter().enumerate() {
                    let width = widths[i + 1]; // +1 because first width is for SHORT column
                    let value = match col {
                        ListColumn::Id => format!("{:<width$}", format_short_id(&feat.id), width = width),
                        ListColumn::Title => format!("{:<width$}", truncate_str(&feat.title, width - 2), width = width),
                        ListColumn::Description => {
                            let desc = feat.description.as_deref().unwrap_or("-");
                            format!("{:<width$}", truncate_str(desc, width - 2), width = width)
                        }
                        ListColumn::FeatureType => format!("{:<width$}", feat.feature_type, width = width),
                        ListColumn::Component => {
                            // Show component alias (CMP@N) instead of truncated full ID
                            let cmp_alias = short_ids.get_short_id(&feat.component).unwrap_or_else(|| truncate_str(&feat.component, width - 2).to_string());
                            format!("{:<width$}", cmp_alias, width = width)
                        }
                        ListColumn::Status => format!("{:<width$}", feat.status(), width = width),
                        ListColumn::Author => format!("{:<width$}", truncate_str(&feat.author, width - 2), width = width),
                        ListColumn::Created => format!("{:<width$}", feat.created.format("%Y-%m-%d"), width = width),
                    };
                    row_parts.push(value);
                }
                println!("{}", row_parts.join(" "));
            }

            println!();
            println!(
                "{} feature(s) found. Use {} to reference by short ID.",
                style(features.len()).cyan(),
                style("FEAT@N").cyan()
            );
        }
        OutputFormat::Id => {
            for feat in &features {
                println!("{}", feat.id);
            }
        }
        OutputFormat::Md => {
            println!("| Short | ID | Component | Type | Title | Dims | Status |");
            println!("|---|---|---|---|---|---|---|");
            for feat in &features {
                let short_id = short_ids.get_short_id(&feat.id.to_string()).unwrap_or_default();
                println!(
                    "| {} | {} | {} | {} | {} | {} | {} |",
                    short_id,
                    format_short_id(&feat.id),
                    truncate_str(&feat.component, 13),
                    feat.feature_type,
                    feat.title,
                    feat.dimensions.len(),
                    feat.status()
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

    // Resolve component ID
    let short_ids = ShortIdIndex::load(&project);
    let component_id = short_ids.resolve(&args.component).unwrap_or_else(|| args.component.clone());

    // Validate component exists
    let cmp_dir = project.root().join("bom/components");
    let mut component_found = false;
    if cmp_dir.exists() {
        for entry in fs::read_dir(&cmp_dir).into_diagnostic()? {
            let entry = entry.into_diagnostic()?;
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "yaml") {
                let filename = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                if filename.contains(&component_id) {
                    component_found = true;
                    break;
                }
            }
        }
    }

    if !component_found {
        return Err(miette::miette!(
            "Component '{}' not found. Create it first with: tdt cmp new",
            args.component
        ));
    }

    let title: String;
    let feature_type: String;

    if args.interactive {
        // Use schema-driven wizard
        let wizard = SchemaWizard::new();
        let result = wizard.run(EntityPrefix::Feat)?;

        title = result
            .get_string("title")
            .map(String::from)
            .unwrap_or_else(|| "New Feature".to_string());

        feature_type = result
            .get_string("feature_type")
            .map(String::from)
            .unwrap_or_else(|| "hole".to_string());
    } else {
        title = args.title.ok_or_else(|| miette::miette!("Title is required (use --title or -i for interactive)"))?;
        feature_type = args.feature_type;
    }

    // Generate ID
    let id = EntityId::new(EntityPrefix::Feat);

    // Generate template
    let generator = TemplateGenerator::new().map_err(|e| miette::miette!("{}", e))?;
    let ctx = TemplateContext::new(id.clone(), config.author())
        .with_title(&title)
        .with_component_id(&component_id)
        .with_feature_type(&feature_type);

    let yaml_content = generator
        .generate_feature(&ctx)
        .map_err(|e| miette::miette!("{}", e))?;

    // Write file
    let output_dir = project.root().join("tolerances/features");
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
        "{} Created feature {}",
        style("✓").green(),
        style(short_id.unwrap_or_else(|| format_short_id(&id))).cyan()
    );
    println!("   {}", style(file_path.display()).dim());
    println!(
        "   Parent: {} | Type: {} | {}",
        style(truncate_str(&component_id, 13)).yellow(),
        style(&feature_type).cyan(),
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

    // Find the feature file
    let feat_dir = project.root().join("tolerances/features");
    let mut found_path = None;

    if feat_dir.exists() {
        for entry in fs::read_dir(&feat_dir).into_diagnostic()? {
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

    let path = found_path.ok_or_else(|| miette::miette!("No feature found matching '{}'", args.id))?;

    // Read and display
    let content = fs::read_to_string(&path).into_diagnostic()?;

    match global.format {
        OutputFormat::Yaml | OutputFormat::Auto => {
            print!("{}", content);
        }
        OutputFormat::Json => {
            let feat: Feature = serde_yml::from_str(&content).into_diagnostic()?;
            let json = serde_json::to_string_pretty(&feat).into_diagnostic()?;
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

    // Find the feature file
    let feat_dir = project.root().join("tolerances/features");
    let mut found_path = None;

    if feat_dir.exists() {
        for entry in fs::read_dir(&feat_dir).into_diagnostic()? {
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

    let path = found_path.ok_or_else(|| miette::miette!("No feature found matching '{}'", args.id))?;

    println!("Opening {} in {}...", style(path.display()).cyan(), style(config.editor()).yellow());

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
