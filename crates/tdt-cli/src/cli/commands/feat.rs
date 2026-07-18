//! `tdt feat` command - Feature management (dimensional features on components)

use clap::{Subcommand, ValueEnum};
use console::style;
use dialoguer::{theme::ColorfulTheme, Input};
use miette::{IntoDiagnostic, Result};

use crate::cli::filters::StatusFilter;
use crate::cli::helpers::truncate_str;
use crate::cli::table::{CellValue, ColumnDef, TableRow};
use crate::cli::{GlobalOpts, OutputFormat};
use tdt_core::core::cache::EntityCache;
use tdt_core::core::identity::EntityPrefix;
use tdt_core::core::project::Project;
use tdt_core::core::shortid::ShortIdIndex;
use tdt_core::core::CachedFeature;
use tdt_core::core::Config;
use tdt_core::entities::feature::{Dimension, DimensionRef, Feature, FeatureType};
use tdt_core::schema::wizard::SchemaWizard;
use tdt_core::services::{
    CommonFilter, CreateFeature, FeatureFilter, FeatureService, FeatureSortField, SortDirection,
    UpdateFeature,
};

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

    /// Compute torsor bounds from GD&T controls
    /// Auto-calculates torsor_bounds from gdt array and geometry_class
    ComputeBounds(ComputeBoundsArgs),

    /// Set a feature's 3D geometry length from another feature's dimension
    SetLength(SetLengthArgs),
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
    ColumnDef::new("component", "COMPONENT", 24),
    ColumnDef::new("status", "STATUS", 10),
    ColumnDef::new("author", "AUTHOR", 14),
    ColumnDef::new("created", "CREATED", 12),
];

const FEAT_LIST_SPEC: crate::cli::entity_cmd::CachedListSpec =
    crate::cli::entity_cmd::CachedListSpec {
        columns: FEAT_COLUMNS,
        entity_name: "feature",
        entity_prefix: "FEAT",
        empty_message: "No features found.",
    };

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
        ListColumn::Title,
        ListColumn::FeatureType,
        ListColumn::Component,
        ListColumn::Status
    ])]
    pub columns: Vec<ListColumn>,

    /// Show full ID column (hidden by default since SHORT is always shown)
    #[arg(long)]
    pub show_id: bool,

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

    /// Only show entities linked to these IDs (use - for stdin pipe)
    #[arg(long, value_delimiter = ',')]
    pub linked_to: Vec<String>,

    /// Filter by link type when using --linked-to (e.g., verified_by, satisfied_by)
    #[arg(long, requires = "linked_to")]
    pub via: Option<String>,
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

#[derive(clap::Args, Debug)]
pub struct ComputeBoundsArgs {
    /// Feature ID or short ID (FEAT@N)
    pub id: String,

    /// Optional actual size for MMC/LMC bonus calculation
    #[arg(long)]
    pub actual_size: Option<f64>,

    /// Update the feature file with computed bounds
    #[arg(long, short = 'u')]
    pub update: bool,

    /// Suppress output (only show errors)
    #[arg(long, short = 'q')]
    pub quiet: bool,
}

#[derive(clap::Args, Debug)]
pub struct SetLengthArgs {
    /// Target feature ID or short ID (FEAT@N)
    pub id: String,

    /// Source dimension reference (FEAT@N:dimension_name or FEAT-xxx:dimension_name)
    #[arg(long)]
    pub from: String,

    /// Suppress output
    #[arg(long, short = 'q')]
    pub quiet: bool,
}

/// Search directories for this entity type — single source of truth
/// lives in `Project::entity_search_directories`.
const FEATURE_DIRS: &[&str] = tdt_core::core::project::Project::entity_search_directories(
    tdt_core::core::identity::EntityPrefix::Feat,
);

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
        FeatCommands::ComputeBounds(args) => run_compute_bounds(args, global),
        FeatCommands::SetLength(args) => run_set_length(args),
    }
}

fn run_list(args: ListArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let cache = EntityCache::open(&project).map_err(|e| miette::miette!("{}", e))?;
    let service = FeatureService::new(&project, &cache);
    let mut short_ids = ShortIdIndex::load(&project);

    // Resolve linked-to filter via cache
    let allowed_ids = crate::cli::helpers::resolve_linked_to(
        &args.linked_to,
        args.via.as_deref(),
        &short_ids,
        &cache,
    );

    let format = match global.output {
        OutputFormat::Auto => OutputFormat::Tsv,
        f => f,
    };

    // Resolve component filter if provided
    let component_filter = args
        .component
        .as_ref()
        .map(|c| short_ids.resolve(c).unwrap_or_else(|| c.clone()));

    let filter = build_feat_filter(&args, component_filter.as_deref());

    // Check if we can use the fast cache path
    let can_use_cache =
        args.recent.is_none() && !matches!(format, OutputFormat::Json | OutputFormat::Yaml);

    // Build component lookup map for displaying part numbers and titles
    let component_info: std::collections::HashMap<String, (String, String)> = cache
        .list_components(None, None, None, None, None, None)
        .into_iter()
        .map(|c| {
            let pn = c.part_number.unwrap_or_default();
            (c.id, (pn, c.title))
        })
        .collect();

    if can_use_cache {
        let mut features = service
            .list_cached(&filter)
            .map_err(|e| miette::miette!("{}", e))?;
        sort_cached_features(&mut features, &args);

        // Apply linked-to filter
        if let Some(ref ids) = allowed_ids {
            features.retain(|e| ids.contains(&e.id));
        }

        if args.reverse {
            features.reverse();
        }
        if let Some(limit) = args.limit {
            features.truncate(limit);
        }

        let mut visible: Vec<String> = if args.show_id {
            vec!["id".to_string()]
        } else {
            vec![]
        };
        visible.extend(args.columns.iter().map(|c| c.to_string()));
        return crate::cli::entity_cmd::output_cached_list(
            &features,
            &FEAT_LIST_SPEC,
            &visible,
            args.wrap,
            args.count,
            format,
            &project,
            |f| cached_feat_to_row(f, &short_ids, &component_info),
            |f| f.file_path.as_path(),
        );
    }

    // Full entity loading path
    let mut features = service
        .list(&filter)
        .map_err(|e| miette::miette!("{}", e))?;

    // Apply linked-to filter
    if let Some(ref ids) = allowed_ids {
        features.retain(|e| ids.contains(&e.id.to_string()));
    }

    // Post-sort for Description column (not in service sort)
    if matches!(args.sort, ListColumn::Description) {
        features.sort_by(|a, b| {
            a.description
                .as_deref()
                .unwrap_or("")
                .cmp(b.description.as_deref().unwrap_or(""))
        });
    }

    if args.reverse {
        features.reverse();
    }
    if let Some(limit) = args.limit {
        features.truncate(limit);
    }

    // Count only
    if args.count {
        println!("{}", features.len());
        return Ok(());
    }

    let mut visible: Vec<String> = if args.show_id {
        vec!["id".to_string()]
    } else {
        vec![]
    };
    visible.extend(args.columns.iter().map(|c| c.to_string()));
    crate::cli::entity_cmd::output_entity_list(
        &features,
        &FEAT_LIST_SPEC,
        &visible,
        args.wrap,
        args.count,
        format,
        &project,
        &mut short_ids,
        |x| x.id.to_string(),
        |x, sids| feat_to_row(x, sids, &component_info),
    )
}

/// Build a FeatureFilter from CLI ListArgs
fn build_feat_filter(args: &ListArgs, component: Option<&str>) -> FeatureFilter {
    let feature_type = match args.feature_type {
        TypeFilter::All => None,
        TypeFilter::Internal => Some(FeatureType::Internal),
        TypeFilter::External => Some(FeatureType::External),
    };

    let status = match args.status {
        StatusFilter::All => None,
        StatusFilter::Draft => Some(vec![tdt_core::core::entity::Status::Draft]),
        StatusFilter::Review => Some(vec![tdt_core::core::entity::Status::Review]),
        StatusFilter::Approved => Some(vec![tdt_core::core::entity::Status::Approved]),
        StatusFilter::Released => Some(vec![tdt_core::core::entity::Status::Released]),
        StatusFilter::Obsolete => Some(vec![tdt_core::core::entity::Status::Obsolete]),
        StatusFilter::Active => Some(vec![
            tdt_core::core::entity::Status::Draft,
            tdt_core::core::entity::Status::Review,
            tdt_core::core::entity::Status::Approved,
            tdt_core::core::entity::Status::Released,
        ]),
    };

    let (sort, sort_direction) = build_feat_sort(args);

    FeatureFilter {
        common: CommonFilter {
            status,
            author: args.author.clone(),
            search: args.search.clone(),
            recent_days: args.recent,
            limit: args.limit,
            ..Default::default()
        },
        feature_type,
        component: component.map(String::from),
        sort,
        sort_direction,
        ..Default::default()
    }
}

/// Build sort field and direction from CLI args
fn build_feat_sort(args: &ListArgs) -> (FeatureSortField, SortDirection) {
    let field = match args.sort {
        ListColumn::Id => FeatureSortField::Id,
        ListColumn::Title => FeatureSortField::Title,
        ListColumn::FeatureType => FeatureSortField::Type,
        ListColumn::Component => FeatureSortField::Component,
        ListColumn::Status => FeatureSortField::Status,
        ListColumn::Author => FeatureSortField::Author,
        ListColumn::Created => FeatureSortField::Created,
        ListColumn::Description => FeatureSortField::Created, // Handled as post-sort
    };

    let direction = if args.reverse {
        SortDirection::Ascending
    } else {
        SortDirection::Descending
    };

    (field, direction)
}

/// Sort cached features based on CLI args
fn sort_cached_features(features: &mut [CachedFeature], args: &ListArgs) {
    match args.sort {
        ListColumn::Id => features.sort_by(|a, b| a.id.cmp(&b.id)),
        ListColumn::Title => features.sort_by(|a, b| a.title.cmp(&b.title)),
        ListColumn::Description => features.sort_by(|a, b| a.id.cmp(&b.id)), // No desc in cache
        ListColumn::FeatureType => features.sort_by(|a, b| a.feature_type.cmp(&b.feature_type)),
        ListColumn::Component => features.sort_by(|a, b| a.component_id.cmp(&b.component_id)),
        ListColumn::Status => features.sort_by(|a, b| a.status.cmp(&b.status)),
        ListColumn::Author => features.sort_by(|a, b| a.author.cmp(&b.author)),
        ListColumn::Created => features.sort_by(|a, b| a.created.cmp(&b.created)),
    }
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
        .cell("status", CellValue::Status(feat.status))
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
    let cache = EntityCache::open(&project).map_err(|e| miette::miette!("{}", e))?;
    let config = Config::load();
    let service = FeatureService::new(&project, &cache);

    // Load short IDs early for component resolution
    let mut short_ids = ShortIdIndex::load(&project);

    // Resolve component ID
    let component_id = short_ids
        .resolve(&args.component)
        .unwrap_or_else(|| args.component.clone());

    // Validate component exists using service
    let cmp_service = tdt_core::services::ComponentService::new(&project, &cache);
    let cmp_exists = cmp_service
        .get(&component_id)
        .map_err(|e| miette::miette!("{}", e))?
        .is_some();
    if !cmp_exists {
        return Err(miette::miette!(
            "Component '{}' not found. Create it first with: tdt cmp new",
            args.component
        ));
    }

    let title: String;
    let feature_type: FeatureType;
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
            .and_then(|s| s.parse().ok())
            .unwrap_or(FeatureType::Internal);

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
        feature_type = args
            .feature_type
            .to_string()
            .parse()
            .unwrap_or(FeatureType::Internal);
    }

    // Build dimensions - add primary dimension for interactive mode
    let is_internal = matches!(feature_type, FeatureType::Internal);
    let dimensions = if args.interactive {
        vec![Dimension {
            name: dimension_name.clone(),
            nominal,
            plus_tol,
            minus_tol,
            units: "mm".to_string(),
            internal: is_internal,
            distribution: tdt_core::entities::stackup::Distribution::Normal,
        }]
    } else {
        // Default dimension for non-interactive
        vec![Dimension {
            name: "diameter".to_string(),
            nominal: 10.0,
            plus_tol: 0.1,
            minus_tol: 0.05,
            units: "mm".to_string(),
            internal: is_internal,
            distribution: tdt_core::entities::stackup::Distribution::Normal,
        }]
    };

    // Create feature via service
    let input = CreateFeature {
        component: component_id.clone(),
        feature_type,
        title: title.clone(),
        description: None,
        dimensions,
        gdt: Vec::new(),
        geometry_class: None,
        datum_label: None,
        tags: Vec::new(),
        status: None,
        author: config.author(),
    };

    let feat = service
        .create(input)
        .map_err(|e| miette::miette!("{}", e))?;

    // Get file path for the created feature
    let file_path = project
        .root()
        .join("tolerances/features")
        .join(format!("{}.tdt.yaml", feat.id));

    // Add to short ID index
    let short_id = short_ids.add(feat.id.to_string());
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
        style(feat.feature_type.to_string()).cyan(),
        style(&title).white()
    );
    crate::cli::entity_cmd::output_new_entity(
        &feat.id,
        &file_path,
        short_id.clone(),
        ENTITY_CONFIG.name,
        &title,
        Some(&extra_info),
        &added_links,
        global,
    );

    // Sync cache after creation
    super::utils::sync_cache(&project);

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
    let cache = EntityCache::open(&project).map_err(|e| miette::miette!("{}", e))?;

    // Resolve short ID if needed
    let short_ids = ShortIdIndex::load(&project);
    let resolved_id = short_ids
        .resolve(&args.id)
        .unwrap_or_else(|| args.id.clone());

    // Use FeatureService to get the feature (cache-first lookup)
    let service = FeatureService::new(&project, &cache);
    let feat = service
        .get(&resolved_id)
        .map_err(|e| miette::miette!("{}", e))?
        .ok_or_else(|| miette::miette!("No feature found matching '{}'", args.id))?;

    match global.output {
        OutputFormat::Yaml => {
            let yaml = serde_yml::to_string(&feat).into_diagnostic()?;
            print!("{}", yaml);
        }
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&feat).into_diagnostic()?;
            println!("{}", json);
        }
        OutputFormat::Id | OutputFormat::ShortId => {
            if global.output == OutputFormat::ShortId {
                let short_id = short_ids
                    .get_short_id(&feat.id.to_string())
                    .unwrap_or_default();
                println!("{}", short_id);
            } else {
                println!("{}", feat.id);
            }
        }
        _ => {
            // Reopen cache for title lookups (format_link_with_title expects Option<EntityCache>)
            let cache_opt = EntityCache::open(&project).ok();

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
            let cmp_display = if let Some(ref cache_ref) = cache_opt {
                // Find component in cache to get part number and title
                let components = cache_ref.list_components(None, None, None, None, None, None);
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

fn run_compute_bounds(args: ComputeBoundsArgs, global: &GlobalOpts) -> Result<()> {
    use tdt_core::core::gdt_torsor::compute_torsor_bounds;

    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let cache = EntityCache::open(&project).map_err(|e| miette::miette!("{}", e))?;
    let short_ids = ShortIdIndex::load(&project);

    // Resolve short ID if needed
    let resolved_id = short_ids
        .resolve(&args.id)
        .unwrap_or_else(|| args.id.clone());

    // Load feature via service
    let service = FeatureService::new(&project, &cache);
    let feat = service
        .get_required(&resolved_id)
        .map_err(|_| miette::miette!("No feature found matching '{}'", args.id))?;

    // Check prerequisites
    if feat.gdt.is_empty() && feat.dimensions.is_empty() {
        return Err(miette::miette!(
            "Feature has no GD&T controls or dimensions to compute bounds from"
        ));
    }

    // Compute bounds
    // Note: Feature lookup not available in this context, use None
    let result =
        compute_torsor_bounds::<fn(&str) -> Option<Feature>>(&feat, args.actual_size, None);

    // Handle output
    let short_id = short_ids
        .get_short_id(&feat.id.to_string())
        .unwrap_or_else(|| "FEAT@?".to_string());

    if !args.quiet {
        match global.output {
            OutputFormat::Json => {
                let json = serde_json::to_string_pretty(&result.bounds).into_diagnostic()?;
                println!("{}", json);
            }
            OutputFormat::Yaml => {
                let yaml = serde_yml::to_string(&result.bounds).into_diagnostic()?;
                print!("{}", yaml);
            }
            _ => {
                println!(
                    "{} Computed torsor bounds for {}",
                    style("✓").green(),
                    style(&short_id).cyan()
                );
                println!();

                // Display bounds
                if let Some([min, max]) = result.bounds.u {
                    println!("  u: [{:.6}, {:.6}]", min, max);
                }
                if let Some([min, max]) = result.bounds.v {
                    println!("  v: [{:.6}, {:.6}]", min, max);
                }
                if let Some([min, max]) = result.bounds.w {
                    println!("  w: [{:.6}, {:.6}]", min, max);
                }
                if let Some([min, max]) = result.bounds.alpha {
                    println!("  α: [{:.6}, {:.6}] rad", min, max);
                }
                if let Some([min, max]) = result.bounds.beta {
                    println!("  β: [{:.6}, {:.6}] rad", min, max);
                }
                if let Some([min, max]) = result.bounds.gamma {
                    println!("  γ: [{:.6}, {:.6}] rad", min, max);
                }

                if result.has_bonus {
                    println!();
                    println!("  {} Includes bonus tolerance (MMC/LMC)", style("ℹ").blue());
                }

                // Show warnings
                for warning in &result.warnings {
                    println!("  {} {}", style("!").yellow(), warning);
                }
            }
        }
    }

    // Update file if requested
    if args.update {
        let update = UpdateFeature {
            torsor_bounds: Some(Some(result.bounds)),
            ..Default::default()
        };
        service
            .update(&resolved_id, update)
            .map_err(|e| miette::miette!("{}", e))?;

        if !args.quiet {
            println!();
            println!(
                "{} Updated torsor_bounds for {}",
                style("✓").green(),
                style(&short_id).cyan()
            );
        }
    } else if !args.quiet && !matches!(global.output, OutputFormat::Json | OutputFormat::Yaml) {
        println!();
        println!(
            "  {} Use {} to save to file",
            style("→").dim(),
            style("--update").yellow()
        );
    }

    Ok(())
}

fn run_set_length(args: SetLengthArgs) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let cache = EntityCache::open(&project).map_err(|e| miette::miette!("{}", e))?;
    let short_ids = ShortIdIndex::load(&project);

    // Parse the source reference (FEAT@N:dimension_name or FEAT-xxx:dimension_name)
    let dim_ref = DimensionRef::parse(&args.from).ok_or_else(|| {
        miette::miette!(
            "Invalid dimension reference '{}'. Expected format: FEAT@N:dimension_name or FEAT-xxx:dimension_name",
            args.from
        )
    })?;

    // Resolve both feature IDs
    let target_id = short_ids
        .resolve(&args.id)
        .unwrap_or_else(|| args.id.clone());
    let source_id = short_ids
        .resolve(&dim_ref.feature_id)
        .unwrap_or_else(|| dim_ref.feature_id.clone());

    // Use service to set length from source dimension
    let service = FeatureService::new(&project, &cache);
    let updated_feat = service
        .set_length_from(&target_id, &source_id, &dim_ref.dimension_name)
        .map_err(|e| miette::miette!("{}", e))?;

    // Get the dimension value for display
    let dimension_value = updated_feat
        .geometry_3d
        .as_ref()
        .and_then(|g| g.length)
        .unwrap_or(0.0);

    if !args.quiet {
        let target_short = short_ids
            .get_short_id(&updated_feat.id.to_string())
            .unwrap_or_else(|| args.id.clone());
        let source_short = short_ids
            .get_short_id(&source_id)
            .unwrap_or_else(|| dim_ref.feature_id.clone());

        println!(
            "{} Set {} geometry length to {} (from {}:{})",
            style("✓").green(),
            style(&target_short).cyan(),
            style(format!("{:.4}", dimension_value)).yellow(),
            style(&source_short).cyan(),
            style(&dim_ref.dimension_name).white()
        );
    }

    Ok(())
}
