//! `tdt feat` command - Feature management (dimensional features on components)

use clap::{Subcommand, ValueEnum};
use console::style;
use dialoguer::{theme::ColorfulTheme, Input};
use miette::{IntoDiagnostic, Result};
use std::fs;

use crate::cli::filters::StatusFilter;
use crate::cli::helpers::truncate_str;
use crate::cli::table::{CellValue, ColumnDef, TableConfig, TableFormatter, TableRow};
use crate::cli::{GlobalOpts, OutputFormat};
use crate::core::cache::EntityCache;
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

    /// Delete a feature
    Delete(DeleteArgs),

    /// Archive a feature (soft delete)
    Archive(ArchiveArgs),
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

/// Column definitions for feature list output
const FEAT_COLUMNS: &[ColumnDef] = &[
    ColumnDef::new("id", "ID", 17),
    ColumnDef::new("title", "TITLE", 20),
    ColumnDef::new("description", "DESCRIPTION", 30),
    ColumnDef::new("feature-type", "TYPE", 10),
    ColumnDef::new("component", "COMPONENT", 40),
    ColumnDef::new("status", "STATUS", 10),
    ColumnDef::new("author", "AUTHOR", 14),
    ColumnDef::new("created", "CREATED", 12),
];

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

    /// Wrap text in columns (mobile-friendly output with specified width)
    #[arg(long, short = 'w')]
    pub wrap: Option<usize>,
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

    /// Link to another entity (auto-infers link type)
    #[arg(long, short = 'L')]
    pub link: Vec<String>,
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

#[derive(clap::Args, Debug)]
pub struct DeleteArgs {
    /// Feature ID or short ID (FEAT@N)
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
    /// Feature ID or short ID (FEAT@N)
    pub id: String,

    /// Force archive even if other entities reference this one
    #[arg(long)]
    pub force: bool,

    /// Suppress output
    #[arg(long, short = 'q')]
    pub quiet: bool,
}

/// Directories where features are stored
const FEATURE_DIRS: &[&str] = &["tolerances/features"];

/// Entity configuration for features
const ENTITY_CONFIG: crate::cli::EntityConfig = crate::cli::EntityConfig {
    prefix: EntityPrefix::Feat,
    dirs: FEATURE_DIRS,
    name: "feature",
    name_plural: "features",
};

/// Run a feature subcommand
pub fn run(cmd: FeatCommands, global: &GlobalOpts) -> Result<()> {
    match cmd {
        FeatCommands::List(args) => run_list(args, global),
        FeatCommands::New(args) => run_new(args, global),
        FeatCommands::Show(args) => run_show(args, global),
        FeatCommands::Edit(args) => run_edit(args),
        FeatCommands::Delete(args) => run_delete(args),
        FeatCommands::Archive(args) => run_archive(args),
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
                StatusFilter::Active | StatusFilter::All => None,
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

        if path.extension().is_some_and(|e| e == "yaml") {
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
            StatusFilter::Active => f.status != crate::core::entity::Status::Obsolete,
            StatusFilter::All => true,
        })
        .filter(|f| {
            if let Some(ref search) = args.search {
                let search_lower = search.to_lowercase();
                f.title.to_lowercase().contains(&search_lower)
                    || f.description
                        .as_ref()
                        .is_some_and(|d| d.to_lowercase().contains(&search_lower))
            } else {
                true
            }
        })
        .filter(|f| {
            args.author
                .as_ref()
                .is_none_or(|author| f.author.to_lowercase().contains(&author.to_lowercase()))
        })
        .filter(|f| {
            args.recent.is_none_or(|days| {
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
    super::utils::save_short_ids(&mut short_ids, &project);

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
        OutputFormat::Csv | OutputFormat::Tsv | OutputFormat::Md => {
            let columns: Vec<&str> = args
                .columns
                .iter()
                .map(|c| c.to_string().leak() as &str)
                .collect();
            let rows: Vec<TableRow> = features
                .iter()
                .map(|f| feat_to_row(f, &short_ids, &component_info))
                .collect();

            let config = TableConfig {
                wrap_width: args.wrap,
                show_summary: true,
            };
            let formatter =
                TableFormatter::new(FEAT_COLUMNS, "feature", "FEAT").with_config(config);
            formatter.output(rows, format, &columns);
        }
        OutputFormat::Id | OutputFormat::ShortId => {
            for feat in &features {
                if format == OutputFormat::ShortId {
                    let short_id = short_ids
                        .get_short_id(&feat.id.to_string())
                        .unwrap_or_default();
                    println!("{}", short_id);
                } else {
                    println!("{}", feat.id);
                }
            }
        }
        OutputFormat::Auto | OutputFormat::Path => unreachable!(),
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
        OutputFormat::Csv | OutputFormat::Tsv | OutputFormat::Md => {
            let columns: Vec<&str> = args
                .columns
                .iter()
                .map(|c| c.to_string().leak() as &str)
                .collect();
            let rows: Vec<TableRow> = features
                .iter()
                .map(|f| cached_feat_to_row(f, short_ids, component_info))
                .collect();

            let config = TableConfig {
                wrap_width: args.wrap,
                show_summary: true,
            };
            let formatter =
                TableFormatter::new(FEAT_COLUMNS, "feature", "FEAT").with_config(config);
            formatter.output(rows, format, &columns);
        }
        OutputFormat::Id | OutputFormat::ShortId => {
            for feat in features {
                if format == OutputFormat::ShortId {
                    let short_id = short_ids.get_short_id(&feat.id).unwrap_or_default();
                    println!("{}", short_id);
                } else {
                    println!("{}", feat.id);
                }
            }
        }
        OutputFormat::Json | OutputFormat::Yaml | OutputFormat::Auto | OutputFormat::Path => {
            // Should never reach here
            unreachable!()
        }
    }

    Ok(())
}

/// Format component display string from component_info map
fn format_component_display(
    component_id: &str,
    short_ids: &ShortIdIndex,
    component_info: &std::collections::HashMap<String, (String, String)>,
) -> String {
    let cmp_alias = short_ids
        .get_short_id(component_id)
        .unwrap_or_else(|| "?".to_string());
    let (part_number, cmp_title) = component_info
        .get(component_id)
        .map(|(pn, t)| (pn.as_str(), t.as_str()))
        .unwrap_or(("", ""));

    if !part_number.is_empty() {
        format!("{} ({}) {}", cmp_alias, part_number, cmp_title)
    } else if !cmp_title.is_empty() {
        format!("{} {}", cmp_alias, cmp_title)
    } else {
        cmp_alias
    }
}

/// Convert a full Feature entity to a TableRow
fn feat_to_row(
    feat: &Feature,
    short_ids: &ShortIdIndex,
    component_info: &std::collections::HashMap<String, (String, String)>,
) -> TableRow {
    let component_display = format_component_display(&feat.component, short_ids, component_info);

    TableRow::new(feat.id.to_string(), short_ids)
        .cell("id", CellValue::Id(feat.id.to_string()))
        .cell("title", CellValue::Text(feat.title.clone()))
        .cell(
            "description",
            CellValue::Text(feat.description.clone().unwrap_or_default()),
        )
        .cell(
            "feature-type",
            CellValue::Type(feat.feature_type.to_string()),
        )
        .cell("component", CellValue::Text(component_display))
        .cell("status", CellValue::Status(feat.status.clone()))
        .cell("author", CellValue::Text(feat.author.clone()))
        .cell("created", CellValue::DateTime(feat.created))
}

/// Convert a cached feature to a TableRow
fn cached_feat_to_row(
    feat: &CachedFeature,
    short_ids: &ShortIdIndex,
    component_info: &std::collections::HashMap<String, (String, String)>,
) -> TableRow {
    let component_display = format_component_display(&feat.component_id, short_ids, component_info);

    TableRow::new(feat.id.clone(), short_ids)
        .cell("id", CellValue::Id(feat.id.clone()))
        .cell("title", CellValue::Text(feat.title.clone()))
        .cell("description", CellValue::Text(String::new())) // No desc in cache
        .cell("feature-type", CellValue::Type(feat.feature_type.clone()))
        .cell("component", CellValue::Text(component_display))
        .cell("status", CellValue::Type(feat.status.to_string()))
        .cell("author", CellValue::Text(feat.author.clone()))
        .cell("created", CellValue::DateTime(feat.created))
}

fn run_new(args: NewArgs, global: &GlobalOpts) -> Result<()> {
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
            if path.extension().is_some_and(|e| e == "yaml") {
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
    super::utils::save_short_ids(&mut short_ids, &project);

    // Handle --link flags
    let added_links = crate::cli::entity_cmd::process_link_flags(
        &file_path,
        EntityPrefix::Feat,
        &args.link,
        &short_ids,
    );

    // Output based on format flag
    let extra_info = format!(
        "Parent: {} | Type: {} | {}",
        style(truncate_str(&component_id, 13)).yellow(),
        style(&feature_type).cyan(),
        style(&title).white()
    );
    crate::cli::entity_cmd::output_new_entity(
        &id,
        &file_path,
        short_id.clone(),
        ENTITY_CONFIG.name,
        &title,
        Some(&extra_info),
        &added_links,
        global,
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
        OutputFormat::Id | OutputFormat::ShortId => {
            if global.format == OutputFormat::ShortId {
                let sid_index = ShortIdIndex::load(&project);
                let short_id = sid_index
                    .get_short_id(&feat.id.to_string())
                    .unwrap_or_default();
                println!("{}", short_id);
            } else {
                println!("{}", feat.id);
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
    crate::cli::entity_cmd::run_edit_generic(&args.id, &ENTITY_CONFIG)
}

fn run_delete(args: DeleteArgs) -> Result<()> {
    crate::cli::commands::utils::run_delete(&args.id, FEATURE_DIRS, args.force, false, args.quiet)
}

fn run_archive(args: ArchiveArgs) -> Result<()> {
    crate::cli::commands::utils::run_delete(&args.id, FEATURE_DIRS, args.force, true, args.quiet)
}
