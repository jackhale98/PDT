//! `tdt feat` command - Feature management (dimensional features on components)

use clap::{Subcommand, ValueEnum};
use console::style;
use miette::{IntoDiagnostic, Result};
use std::fs;

use dialoguer::{theme::ColorfulTheme, Input};

use crate::cli::helpers::{escape_csv, format_short_id, truncate_str};
use crate::cli::{GlobalOpts, OutputFormat};
use crate::core::cache::EntityCache;
use crate::core::entity::Entity;
use crate::core::identity::{EntityId, EntityPrefix};
use crate::core::project::Project;
use crate::core::shortid::ShortIdIndex;
use crate::core::CachedFeature;
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

/// Feature type filter for list command
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum TypeFilter {
    Internal,
    External,
    All,
}

/// CLI-friendly feature type enum
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CliFeatureType {
    Internal,
    External,
}

impl std::fmt::Display for CliFeatureType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CliFeatureType::Internal => write!(f, "internal"),
            CliFeatureType::External => write!(f, "external"),
        }
    }
}

impl From<CliFeatureType> for FeatureType {
    fn from(cli: CliFeatureType) -> Self {
        match cli {
            CliFeatureType::Internal => FeatureType::Internal,
            CliFeatureType::External => FeatureType::External,
        }
    }
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

    /// Feature type (internal = hole/pocket, external = shaft/boss)
    #[arg(long, short = 't', default_value = "internal")]
    pub feature_type: CliFeatureType,

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
    let short_ids = ShortIdIndex::load(&project);

    // Determine output format
    let format = match global.format {
        OutputFormat::Auto => OutputFormat::Tsv,
        f => f,
    };

    // Resolve component filter if provided
    let component_filter = args
        .component
        .as_ref()
        .map(|c| short_ids.resolve(c).unwrap_or_else(|| c.clone()));

    // Check if we can use the fast cache path:
    // - No recent filter (would need time-based SQL)
    // - Not JSON/YAML output (needs full entity serialization)
    let can_use_cache =
        args.recent.is_none() && !matches!(format, OutputFormat::Json | OutputFormat::Yaml);

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

            let type_filter = match args.feature_type {
                TypeFilter::Internal => Some("internal"),
                TypeFilter::External => Some("external"),
                TypeFilter::All => None,
            };

            let mut features = cache.list_features(
                status_filter,
                type_filter,
                component_filter.as_deref(),
                args.author.as_deref(),
                args.search.as_deref(),
                None, // We'll apply limit after sorting
            );

            // Sort
            match args.sort {
                ListColumn::Id => features.sort_by(|a, b| a.id.cmp(&b.id)),
                ListColumn::Title => features.sort_by(|a, b| a.title.cmp(&b.title)),
                ListColumn::Description => features.sort_by(|a, b| a.id.cmp(&b.id)), // No desc in cache
                ListColumn::FeatureType => {
                    features.sort_by(|a, b| a.feature_type.cmp(&b.feature_type))
                }
                ListColumn::Component => {
                    features.sort_by(|a, b| a.component_id.cmp(&b.component_id))
                }
                ListColumn::Status => features.sort_by(|a, b| a.status.cmp(&b.status)),
                ListColumn::Author => features.sort_by(|a, b| a.author.cmp(&b.author)),
                ListColumn::Created => features.sort_by(|a, b| a.created.cmp(&b.created)),
            }

            if args.reverse {
                features.reverse();
            }

            if let Some(limit) = args.limit {
                features.truncate(limit);
            }

            // Build component lookup map for displaying part numbers and titles
            let component_info: std::collections::HashMap<String, (String, String)> = cache
                .list_components(None, None, None, None, None, None)
                .into_iter()
                .map(|c| {
                    let pn = c.part_number.unwrap_or_default();
                    (c.id, (pn, c.title))
                })
                .collect();

            return output_cached_features(&features, &short_ids, &args, format, &component_info);
        }
    }

    // Fall back to full YAML loading
    let feat_dir = project.root().join("tolerances/features");

    if !feat_dir.exists() {
        if args.count {
            println!("0");
        } else {
            println!("No features found.");
        }
        return Ok(());
    }

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
            TypeFilter::Internal => f.feature_type == FeatureType::Internal,
            TypeFilter::External => f.feature_type == FeatureType::External,
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
            a.description
                .as_deref()
                .unwrap_or("")
                .cmp(b.description.as_deref().unwrap_or(""))
        }),
        ListColumn::FeatureType => features
            .sort_by(|a, b| format!("{:?}", a.feature_type).cmp(&format!("{:?}", b.feature_type))),
        ListColumn::Component => features.sort_by(|a, b| a.component.cmp(&b.component)),
        ListColumn::Status => {
            features.sort_by(|a, b| format!("{:?}", a.status).cmp(&format!("{:?}", b.status)))
        }
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
    let mut short_ids = short_ids;
    short_ids.ensure_all(features.iter().map(|f| f.id.to_string()));
    let _ = short_ids.save(&project);

    // Load component info for display
    let component_info: std::collections::HashMap<String, (String, String)> =
        if let Ok(cache) = EntityCache::open(&project) {
            cache
                .list_components(None, None, None, None, None, None)
                .into_iter()
                .map(|c| {
                    let pn = c.part_number.unwrap_or_default();
                    (c.id, (pn, c.title))
                })
                .collect()
        } else {
            std::collections::HashMap::new()
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
            println!(
                "short_id,id,component,part_number,component_title,feature_type,title,dims,status"
            );
            for feat in &features {
                let short_id = short_ids
                    .get_short_id(&feat.id.to_string())
                    .unwrap_or_default();
                let cmp_alias = short_ids.get_short_id(&feat.component).unwrap_or_default();
                let (part_number, cmp_title) = component_info
                    .get(&feat.component)
                    .map(|(pn, t)| (pn.as_str(), t.as_str()))
                    .unwrap_or(("", ""));
                println!(
                    "{},{},{},{},{},{},{},{},{}",
                    short_id,
                    feat.id,
                    cmp_alias,
                    escape_csv(part_number),
                    escape_csv(cmp_title),
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
                    ListColumn::Component => (style("COMPONENT").bold().to_string(), 40),
                    ListColumn::Status => (style("STATUS").bold().to_string(), 10),
                    ListColumn::Author => (style("AUTHOR").bold().to_string(), 14),
                    ListColumn::Created => (style("CREATED").bold().to_string(), 12),
                };
                header_parts.push(format!("{:<width$}", header, width = width));
                widths.push(width);
            }
            println!("{}", header_parts.join(" "));
            println!(
                "{}",
                "-".repeat(widths.iter().sum::<usize>() + widths.len() - 1)
            );

            for feat in &features {
                let short_id = short_ids
                    .get_short_id(&feat.id.to_string())
                    .unwrap_or_default();
                let mut row_parts = vec![format!("{:<8}", style(&short_id).cyan())];

                for (i, col) in args.columns.iter().enumerate() {
                    let width = widths[i + 1]; // +1 because first width is for SHORT column
                    let value = match col {
                        ListColumn::Id => {
                            format!("{:<width$}", format_short_id(&feat.id), width = width)
                        }
                        ListColumn::Title => format!(
                            "{:<width$}",
                            truncate_str(&feat.title, width - 2),
                            width = width
                        ),
                        ListColumn::Description => {
                            let desc = feat.description.as_deref().unwrap_or("-");
                            format!("{:<width$}", truncate_str(desc, width - 2), width = width)
                        }
                        ListColumn::FeatureType => {
                            format!("{:<width$}", feat.feature_type, width = width)
                        }
                        ListColumn::Component => {
                            // Show component alias, part number, and title
                            let cmp_alias = short_ids
                                .get_short_id(&feat.component)
                                .unwrap_or_else(|| "?".to_string());
                            let (part_number, cmp_title) = component_info
                                .get(&feat.component)
                                .map(|(pn, t)| (pn.as_str(), t.as_str()))
                                .unwrap_or(("", ""));
                            let display = if !part_number.is_empty() {
                                format!(
                                    "{} ({}) {}",
                                    cmp_alias,
                                    part_number,
                                    truncate_str(cmp_title, 20)
                                )
                            } else if !cmp_title.is_empty() {
                                format!("{} {}", cmp_alias, truncate_str(cmp_title, 25))
                            } else {
                                cmp_alias
                            };
                            format!(
                                "{:<width$}",
                                truncate_str(&display, width - 2),
                                width = width
                            )
                        }
                        ListColumn::Status => format!("{:<width$}", feat.status(), width = width),
                        ListColumn::Author => format!(
                            "{:<width$}",
                            truncate_str(&feat.author, width - 2),
                            width = width
                        ),
                        ListColumn::Created => {
                            format!("{:<width$}", feat.created.format("%Y-%m-%d"), width = width)
                        }
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
                let short_id = short_ids
                    .get_short_id(&feat.id.to_string())
                    .unwrap_or_default();
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

/// Output cached features (fast path - no YAML parsing needed)
fn output_cached_features(
    features: &[CachedFeature],
    short_ids: &ShortIdIndex,
    args: &ListArgs,
    format: OutputFormat,
    component_info: &std::collections::HashMap<String, (String, String)>,
) -> Result<()> {
    if features.is_empty() {
        println!("No features found.");
        return Ok(());
    }

    if args.count {
        println!("{}", features.len());
        return Ok(());
    }

    match format {
        OutputFormat::Csv => {
            println!("short_id,id,component,part_number,component_title,feature_type,title,status");
            for feat in features {
                let short_id = short_ids.get_short_id(&feat.id).unwrap_or_default();
                let cmp_alias = short_ids
                    .get_short_id(&feat.component_id)
                    .unwrap_or_default();
                let (part_number, cmp_title) = component_info
                    .get(&feat.component_id)
                    .map(|(pn, t)| (pn.as_str(), t.as_str()))
                    .unwrap_or(("", ""));
                println!(
                    "{},{},{},{},{},{},{},{}",
                    short_id,
                    feat.id,
                    cmp_alias,
                    escape_csv(part_number),
                    escape_csv(cmp_title),
                    feat.feature_type,
                    escape_csv(&feat.title),
                    feat.status
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
                    ListColumn::Component => (style("COMPONENT").bold().to_string(), 40),
                    ListColumn::Status => (style("STATUS").bold().to_string(), 10),
                    ListColumn::Author => (style("AUTHOR").bold().to_string(), 14),
                    ListColumn::Created => (style("CREATED").bold().to_string(), 12),
                };
                header_parts.push(format!("{:<width$}", header, width = width));
                widths.push(width);
            }
            println!("{}", header_parts.join(" "));
            println!(
                "{}",
                "-".repeat(widths.iter().sum::<usize>() + widths.len() - 1)
            );

            for feat in features {
                let short_id = short_ids.get_short_id(&feat.id).unwrap_or_default();
                let mut row_parts = vec![format!("{:<8}", style(&short_id).cyan())];

                for (i, col) in args.columns.iter().enumerate() {
                    let width = widths[i + 1];
                    let value = match col {
                        ListColumn::Id => format!(
                            "{:<width$}",
                            truncate_str(&feat.id, width - 2),
                            width = width
                        ),
                        ListColumn::Title => format!(
                            "{:<width$}",
                            truncate_str(&feat.title, width - 2),
                            width = width
                        ),
                        ListColumn::Description => format!("{:<width$}", "-", width = width), // No desc in cache
                        ListColumn::FeatureType => {
                            format!("{:<width$}", feat.feature_type, width = width)
                        }
                        ListColumn::Component => {
                            // Show component alias, part number, and title
                            let cmp_alias = short_ids
                                .get_short_id(&feat.component_id)
                                .unwrap_or_else(|| "?".to_string());
                            let (part_number, cmp_title) = component_info
                                .get(&feat.component_id)
                                .map(|(pn, t)| (pn.as_str(), t.as_str()))
                                .unwrap_or(("", ""));
                            let display = if !part_number.is_empty() {
                                format!(
                                    "{} ({}) {}",
                                    cmp_alias,
                                    part_number,
                                    truncate_str(cmp_title, 20)
                                )
                            } else if !cmp_title.is_empty() {
                                format!("{} {}", cmp_alias, truncate_str(cmp_title, 25))
                            } else {
                                cmp_alias
                            };
                            format!(
                                "{:<width$}",
                                truncate_str(&display, width - 2),
                                width = width
                            )
                        }
                        ListColumn::Status => format!("{:<width$}", feat.status, width = width),
                        ListColumn::Author => format!(
                            "{:<width$}",
                            truncate_str(&feat.author, width - 2),
                            width = width
                        ),
                        ListColumn::Created => {
                            format!("{:<width$}", feat.created.format("%Y-%m-%d"), width = width)
                        }
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
            for feat in features {
                println!("{}", feat.id);
            }
        }
        OutputFormat::Md => {
            println!("| Short | ID | Component | Type | Title | Status |");
            println!("|---|---|---|---|---|---|");
            for feat in features {
                let short_id = short_ids.get_short_id(&feat.id).unwrap_or_default();
                println!(
                    "| {} | {} | {} | {} | {} | {} |",
                    short_id,
                    truncate_str(&feat.id, 15),
                    truncate_str(&feat.component_id, 13),
                    feat.feature_type,
                    feat.title,
                    feat.status
                );
            }
        }
        OutputFormat::Json | OutputFormat::Yaml | OutputFormat::Auto => {
            // Should never reach here
            unreachable!()
        }
    }

    Ok(())
}

fn run_new(args: NewArgs) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let config = Config::load();

    // Resolve component ID
    let short_ids = ShortIdIndex::load(&project);
    let component_id = short_ids
        .resolve(&args.component)
        .unwrap_or_else(|| args.component.clone());

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
    let mut dimension_name = String::from("diameter");
    let mut nominal: f64 = 10.0;
    let mut plus_tol: f64 = 0.1;
    let mut minus_tol: f64 = 0.05;

    if args.interactive {
        // Use schema-driven wizard for title and feature_type
        let wizard = SchemaWizard::new();
        let result = wizard.run(EntityPrefix::Feat)?;

        title = result
            .get_string("title")
            .map(String::from)
            .unwrap_or_else(|| "New Feature".to_string());

        feature_type = result
            .get_string("feature_type")
            .map(String::from)
            .unwrap_or_else(|| "internal".to_string());

        // Custom prompts for primary dimension (wizard can't handle nested objects)
        let theme = ColorfulTheme::default();
        println!();
        println!("{}", style("Primary Dimension:").bold());

        dimension_name = Input::with_theme(&theme)
            .with_prompt("Dimension name (e.g., diameter, width, depth)")
            .default("diameter".to_string())
            .interact_text()
            .into_diagnostic()?;

        let nominal_str: String = Input::with_theme(&theme)
            .with_prompt("Nominal value")
            .default("10.0".to_string())
            .interact_text()
            .into_diagnostic()?;
        nominal = nominal_str.parse().unwrap_or(10.0);

        let plus_str: String = Input::with_theme(&theme)
            .with_prompt("Plus tolerance (+)")
            .default("0.1".to_string())
            .interact_text()
            .into_diagnostic()?;
        plus_tol = plus_str.parse().unwrap_or(0.1);

        let minus_str: String = Input::with_theme(&theme)
            .with_prompt("Minus tolerance (-)")
            .default("0.05".to_string())
            .interact_text()
            .into_diagnostic()?;
        minus_tol = minus_str.parse().unwrap_or(0.05);
    } else {
        title = args.title.ok_or_else(|| {
            miette::miette!("Title is required (use --title or -i for interactive)")
        })?;
        feature_type = args.feature_type.to_string();
    }

    // Generate ID
    let id = EntityId::new(EntityPrefix::Feat);

    // Generate template
    let generator = TemplateGenerator::new().map_err(|e| miette::miette!("{}", e))?;
    let ctx = TemplateContext::new(id.clone(), config.author())
        .with_title(&title)
        .with_component_id(&component_id)
        .with_feature_type(&feature_type);

    let mut yaml_content = generator
        .generate_feature(&ctx)
        .map_err(|e| miette::miette!("{}", e))?;

    // Replace default dimension values with user-provided ones (for interactive mode)
    if args.interactive {
        yaml_content = yaml_content
            .replace(
                "name: \"diameter\"",
                &format!("name: \"{}\"", dimension_name),
            )
            .replace("nominal: 10.0", &format!("nominal: {}", nominal))
            .replace("plus_tol: 0.1", &format!("plus_tol: {}", plus_tol))
            .replace("minus_tol: 0.05", &format!("minus_tol: {}", minus_tol));
    }

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

    let path =
        found_path.ok_or_else(|| miette::miette!("No feature found matching '{}'", args.id))?;

    // Read and parse feature
    let content = fs::read_to_string(&path).into_diagnostic()?;
    let feat: Feature = serde_yml::from_str(&content).into_diagnostic()?;

    match global.format {
        OutputFormat::Yaml => {
            print!("{}", content);
        }
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&feat).into_diagnostic()?;
            println!("{}", json);
        }
        OutputFormat::Id => {
            println!("{}", feat.id);
        }
        _ => {
            // Load cache for title lookups
            let cache = EntityCache::open(&project).ok();

            // Pretty format (default)
            println!("{}", style("─".repeat(60)).dim());
            println!(
                "{}: {}",
                style("ID").bold(),
                style(&feat.id.to_string()).cyan()
            );
            println!("{}: {}", style("Title").bold(), style(&feat.title).yellow());
            println!("{}: {}", style("Type").bold(), feat.feature_type);
            // Look up component info for part number and title
            let cmp_short = short_ids
                .get_short_id(&feat.component)
                .unwrap_or_else(|| feat.component.clone());
            let cmp_display = if let Some(ref cache) = cache {
                // Find component in cache to get part number and title
                let components = cache.list_components(None, None, None, None, None, None);
                if let Some(cmp) = components.iter().find(|c| c.id == feat.component) {
                    match (&cmp.part_number, cmp.title.as_str()) {
                        (Some(pn), title) if !pn.is_empty() => {
                            format!("{} ({}) {}", cmp_short, pn, title)
                        }
                        (_, title) if !title.is_empty() => format!("{} ({})", cmp_short, title),
                        _ => cmp_short,
                    }
                } else {
                    cmp_short
                }
            } else {
                cmp_short
            };
            println!(
                "{}: {}",
                style("Component").bold(),
                style(&cmp_display).cyan()
            );
            println!("{}: {}", style("Status").bold(), feat.status);
            println!("{}", style("─".repeat(60)).dim());

            // Dimensions
            if !feat.dimensions.is_empty() {
                println!();
                println!("{}", style("Dimensions:").bold());
                for dim in &feat.dimensions {
                    let int_ext = if dim.internal { "internal" } else { "external" };
                    println!("  {} ({})", style(&dim.name).cyan(), int_ext);
                    println!("    Nominal: {} {}", dim.nominal, dim.units);
                    println!("    Tolerance: +{} / -{}", dim.plus_tol, dim.minus_tol);
                }
            }

            // GD&T
            if !feat.gdt.is_empty() {
                println!();
                println!("{}", style("GD&T Controls:").bold());
                for gdt in &feat.gdt {
                    println!(
                        "  • {:?} {} {}",
                        gdt.symbol,
                        gdt.value,
                        gdt.datum_refs.join("-")
                    );
                }
            }

            // Tags
            if !feat.tags.is_empty() {
                println!();
                println!("{}: {}", style("Tags").bold(), feat.tags.join(", "));
            }

            // Description
            if let Some(ref desc) = feat.description {
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
                feat.author,
                style("Created").dim(),
                feat.created.format("%Y-%m-%d %H:%M"),
                style("Revision").dim(),
                feat.entity_revision
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

    let path =
        found_path.ok_or_else(|| miette::miette!("No feature found matching '{}'", args.id))?;

    println!(
        "Opening {} in {}...",
        style(path.display()).cyan(),
        style(config.editor()).yellow()
    );

    config.run_editor(&path).into_diagnostic()?;

    Ok(())
}
