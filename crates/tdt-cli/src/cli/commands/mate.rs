//! `tdt mate` command - Mate management (1:1 feature contacts with fit calculation)

use clap::{Subcommand, ValueEnum};
use console::style;
use miette::{bail, IntoDiagnostic, Result};
use std::fs;

use crate::cli::commands::utils::format_link_with_title;
use crate::cli::filters::StatusFilter;
use crate::cli::helpers::{format_short_id, smart_round, truncate_str};
use crate::cli::table::{CellValue, ColumnDef, TableRow};
use crate::cli::{GlobalOpts, OutputFormat};
use tdt_core::core::cache::EntityCache;
use tdt_core::core::entity::Entity;
use tdt_core::core::identity::{EntityId, EntityPrefix};
use tdt_core::core::loader;
use tdt_core::core::project::Project;
use tdt_core::core::shortid::ShortIdIndex;
use tdt_core::core::Config;
use tdt_core::entities::assembly::Assembly;
use tdt_core::entities::feature::Feature;
use tdt_core::entities::mate::{FitAnalysis, Mate, MateType, StatisticalFit};
use tdt_core::schema::wizard::SchemaWizard;
use tdt_core::services::{
    CommonFilter, CreateMate, FeatureService, MateFilter, MateService, MateSortField, SortDirection,
};

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

crate::list_columns! {
    /// List column selection
    pub enum ListColumn {
        Id => "id",
        Title => "title",
        MateType => "mate-type",
        FitResult => "fit-result",
        Match => "match",
        FeatureA => "feature-a",
        FeatureB => "feature-b",
        Status => "status",
        Author => "author",
        Created => "created",
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

const MATE_LIST_SPEC: crate::cli::entity_cmd::CachedListSpec =
    crate::cli::entity_cmd::CachedListSpec {
        columns: MATE_COLUMNS,
        entity_name: "mate",
        entity_prefix: "MATE",
        empty_message: "No mates found.",
    };

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

    /// Only show entities linked to these IDs (use - for stdin pipe)
    #[arg(long, value_delimiter = ',')]
    pub linked_to: Vec<String>,

    /// Filter by link type when using --linked-to (e.g., verified_by, satisfied_by)
    #[arg(long, requires = "linked_to")]
    pub via: Option<String>,

    /// Filter by component (show mates involving this component's features)
    #[arg(long, short = 'c')]
    pub component: Option<String>,

    /// Filter by assembly (show mates involving features of components in this assembly)
    #[arg(long, short = 'A')]
    pub assembly: Option<String>,
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

/// Search directories for this entity type — single source of truth
/// lives in `Project::entity_search_directories`.
const MATE_DIRS: &[&str] = tdt_core::core::project::Project::entity_search_directories(
    tdt_core::core::identity::EntityPrefix::Mate,
);

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

    /// Include statistical (RSS) analysis with interference probability
    /// Future: Will support 3D torsor-based analysis with --analysis-mode 3d
    #[arg(long, short = 's')]
    pub statistical: bool,

    /// Sigma level for statistical analysis (tolerance = sigma_level × σ)
    /// Common values: 6.0 (±3σ, 99.73%), 4.0 (±2σ, 95.4%), 8.0 (±4σ, 99.99%)
    #[arg(long, default_value = "6.0")]
    pub sigma: f64,
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
    let cache = EntityCache::open(&project).map_err(|e| miette::miette!("{}", e))?;
    let service = MateService::new(&project, &cache);
    let mut short_ids = ShortIdIndex::load(&project);

    // Resolve linked-to filter via cache
    let allowed_ids = crate::cli::helpers::resolve_linked_to(
        &args.linked_to,
        args.via.as_deref(),
        &short_ids,
        &cache,
    );

    let format = global.list_format();

    let filter = build_mate_filter(&args);
    let mut mates = service
        .list(&filter)
        .map_err(|e| miette::miette!("{}", e))?;

    // Apply linked-to filter
    if let Some(ref ids) = allowed_ids {
        mates.retain(|e| ids.contains(&e.id.to_string()));
    }

    // Apply component filter
    if let Some(ref cmp_arg) = args.component {
        let cmp_id = short_ids
            .resolve(cmp_arg)
            .unwrap_or_else(|| cmp_arg.clone());
        mates.retain(|m| {
            m.feature_a.component_id.as_deref() == Some(cmp_id.as_str())
                || m.feature_b.component_id.as_deref() == Some(cmp_id.as_str())
        });
    }

    // Apply assembly filter
    if let Some(ref asm_arg) = args.assembly {
        let asm_id = short_ids
            .resolve(asm_arg)
            .unwrap_or_else(|| asm_arg.clone());
        // Load assembly to get its BOM component IDs
        let asm_cmp_ids: std::collections::HashSet<String> = {
            let asm_dir = project.root().join("bom/assemblies");
            if let Ok(Some((_path, asm))) = loader::load_entity::<Assembly>(&asm_dir, &asm_id) {
                asm.bom
                    .iter()
                    .map(|item| item.component_id.clone())
                    .collect()
            } else {
                eprintln!("{} Assembly '{}' not found", style("⚠").yellow(), asm_arg);
                mates.clear();
                std::collections::HashSet::new()
            }
        };
        if !asm_cmp_ids.is_empty() {
            mates.retain(|m| {
                m.feature_a
                    .component_id
                    .as_ref()
                    .is_some_and(|c| asm_cmp_ids.contains(c))
                    || m.feature_b
                        .component_id
                        .as_ref()
                        .is_some_and(|c| asm_cmp_ids.contains(c))
            });
        }
    }
    // Post-sort for columns not in service (Match, FeatureA, FeatureB)
    if let Some(ref sort_col) = args.sort {
        match sort_col {
            ListColumn::Match => mates.sort_by_key(fit_matches_type),
            ListColumn::FeatureA => {
                mates.sort_by(|a, b| a.feature_a.id.to_string().cmp(&b.feature_a.id.to_string()))
            }
            ListColumn::FeatureB => {
                mates.sort_by(|a, b| a.feature_b.id.to_string().cmp(&b.feature_b.id.to_string()))
            }
            _ => {} // Handled by service
        }
    }

    if let Some(limit) = args.limit {
        mates.truncate(limit);
    }

    // Count only
    if args.count {
        println!("{}", mates.len());
        return Ok(());
    }

    {
        let mut visible: Vec<String> = args.columns.iter().map(|c| c.to_string()).collect();
        if args.show_id && !visible.iter().any(|c| c == "id") {
            visible.insert(0, "id".to_string());
        }
        crate::cli::entity_cmd::output_entity_list(
            &mates,
            &MATE_LIST_SPEC,
            &visible,
            args.wrap,
            args.count,
            format,
            &project,
            &mut short_ids,
            |x| x.id.to_string(),
            mate_to_row,
        )
    }
}

/// Build a MateFilter from CLI ListArgs
fn build_mate_filter(args: &ListArgs) -> MateFilter {
    let mate_type = match args.mate_type {
        TypeFilter::All => None,
        TypeFilter::Clearance => Some(MateType::Clearance),
        TypeFilter::Transition => Some(MateType::Transition),
        TypeFilter::Interference => Some(MateType::Interference),
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

    let (sort, sort_direction) = build_mate_sort(args);

    MateFilter {
        common: CommonFilter {
            status,
            author: args.author.clone(),
            search: args.search.clone(),
            recent_days: args.recent,
            limit: args.limit,
            ..Default::default()
        },
        mate_type,
        recent_days: args.recent,
        sort,
        sort_direction,
        ..Default::default()
    }
}

/// Build sort field and direction from CLI args
fn build_mate_sort(args: &ListArgs) -> (MateSortField, SortDirection) {
    let field = args
        .sort
        .as_ref()
        .map(|col| match col {
            ListColumn::Id => MateSortField::Id,
            ListColumn::Title => MateSortField::Title,
            ListColumn::MateType => MateSortField::MateType,
            ListColumn::FitResult => MateSortField::FitResult,
            ListColumn::Status => MateSortField::Status,
            ListColumn::Author => MateSortField::Author,
            ListColumn::Created => MateSortField::Created,
            // Columns handled as post-sort
            ListColumn::Match | ListColumn::FeatureA | ListColumn::FeatureB => {
                MateSortField::Created
            }
        })
        .unwrap_or(MateSortField::Created);

    (field, SortDirection::Descending)
}

fn run_new(args: NewArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let config = Config::load();
    let cache = EntityCache::open(&project).map_err(|e| miette::miette!("{}", e))?;

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
    let feature_a_str = short_ids
        .resolve(&feat_a_input)
        .unwrap_or_else(|| feat_a_input.clone());
    let feature_b_str = short_ids
        .resolve(&feat_b_input)
        .unwrap_or_else(|| feat_b_input.clone());

    // Validate features exist using service
    let feat_service = FeatureService::new(&project, &cache);
    if feat_service
        .get(&feature_a_str)
        .map_err(|e| miette::miette!("{}", e))?
        .is_none()
    {
        return Err(miette::miette!(
            "Feature A '{}' not found. Create it first with: tdt feat new",
            feat_a_input
        ));
    }
    if feat_service
        .get(&feature_b_str)
        .map_err(|e| miette::miette!("{}", e))?
        .is_none()
    {
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

    // Create mate via service
    let service = MateService::new(&project, &cache);

    // Parse feature IDs
    let feature_a_id: EntityId = feature_a_str
        .parse()
        .map_err(|_| miette::miette!("Invalid feature A ID: {}", feature_a_str))?;
    let feature_b_id: EntityId = feature_b_str
        .parse()
        .map_err(|_| miette::miette!("Invalid feature B ID: {}", feature_b_str))?;

    let input = CreateMate {
        title: title.clone(),
        feature_a: feature_a_id,
        feature_b: feature_b_id,
        mate_type,
        description,
        notes,
        tags: Vec::new(),
        author: config.author(),
    };

    let mate = service
        .create(input)
        .map_err(|e| miette::miette!("{}", e))?;

    // Calculate fit analysis
    let _ = service.recalculate(&mate.id.to_string());

    // Reload the mate to get updated fit analysis
    let mate = service
        .get(&mate.id.to_string())
        .map_err(|e| miette::miette!("{}", e))?
        .ok_or_else(|| miette::miette!("Failed to reload mate after creation"))?;

    let id = &mate.id;
    let file_path = project
        .root()
        .join(format!("tolerances/mates/{}.tdt.yaml", id));

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
    match global.output {
        OutputFormat::Id => {
            println!("{}", id);
        }
        OutputFormat::ShortId => {
            println!(
                "{}",
                short_id.clone().unwrap_or_else(|| format_short_id(id))
            );
        }
        OutputFormat::Path => {
            println!("{}", file_path.display());
        }
        _ => {
            println!(
                "{} Created mate {}",
                style("✓").green(),
                style(short_id.clone().unwrap_or_else(|| format_short_id(id))).cyan()
            );
            println!("   {}", style(file_path.display()).dim());
            println!(
                "   {} <-> {} | {}",
                style(truncate_str(&feature_a_str, 13)).yellow(),
                style(truncate_str(&feature_b_str, 13)).yellow(),
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

    // Use MateService to get the mate (cache-first lookup)
    let service = MateService::new(&project, &cache);
    let mate = service
        .get(&resolved_id)
        .map_err(|e| miette::miette!("{}", e))?
        .ok_or_else(|| miette::miette!("No mate found matching '{}'", args.id))?;

    match global.output {
        OutputFormat::Yaml => {
            let yaml = serde_yml::to_string(&mate).into_diagnostic()?;
            print!("{}", yaml);
        }
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&mate).into_diagnostic()?;
            println!("{}", json);
        }
        OutputFormat::Id | OutputFormat::ShortId => {
            if global.output == OutputFormat::ShortId {
                let short_id = short_ids
                    .get_short_id(&mate.id.to_string())
                    .unwrap_or_default();
                println!("{}", short_id);
            } else {
                println!("{}", mate.id);
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
                format_link_with_title(&mate.feature_a.id.to_string(), &short_ids, &cache_opt);
            let feat_b_display =
                format_link_with_title(&mate.feature_b.id.to_string(), &short_ids, &cache_opt);

            // Helper to get component display with part number and title
            let get_component_display =
                |cmp_id: Option<&String>, cmp_name: Option<&String>| -> Option<String> {
                    if let Some(cmp_id) = cmp_id {
                        // Look up component info from cache
                        if let Some(ref c) = cache_opt {
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
                    tdt_core::entities::mate::FitResult::Clearance => style("CLEARANCE").green(),
                    tdt_core::entities::mate::FitResult::Interference => {
                        style("INTERFERENCE").red()
                    }
                    tdt_core::entities::mate::FitResult::Transition => style("TRANSITION").yellow(),
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
    // Route through the service so the stackup-reference check fires before
    // the file is removed (utils::run_delete is a generic file deleter and
    // doesn't know about engineering relationships).
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let cache = EntityCache::open(&project).map_err(|e| miette::miette!("{}", e))?;

    let short_ids = ShortIdIndex::load(&project);
    let resolved_id = short_ids
        .resolve(&args.id)
        .unwrap_or_else(|| args.id.clone());

    let service = MateService::new(&project, &cache);
    service
        .delete(&resolved_id, args.force)
        .map_err(|e| miette::miette!("{}", e))?;

    if !args.quiet {
        println!(
            "{} Deleted mate {}",
            style("✓").green(),
            style(&args.id).cyan()
        );
    }

    crate::cli::commands::utils::sync_cache(&project);
    Ok(())
}

fn run_archive(args: ArchiveArgs) -> Result<()> {
    crate::cli::commands::utils::run_delete(&args.id, MATE_DIRS, args.force, true, args.quiet)
}

fn run_recalc(args: RecalcArgs) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let cache = EntityCache::open(&project).map_err(|e| miette::miette!("{}", e))?;

    // Resolve short ID if needed
    let short_ids = ShortIdIndex::load(&project);
    let resolved_id = short_ids
        .resolve(&args.id)
        .unwrap_or_else(|| args.id.clone());

    // Use service to recalculate
    let service = MateService::new(&project, &cache);
    let result = service
        .recalculate(&resolved_id)
        .map_err(|e| miette::miette!("{}", e))?;

    // Check for errors
    if let Some(ref error) = result.error {
        return Err(miette::miette!("{}", error));
    }

    let mut mate = result.mate;

    // Add statistical analysis if requested (needs features)
    if args.statistical {
        if let Some(ref mut analysis) = mate.fit_analysis {
            // Load features for statistical calculation
            let feat_service = tdt_core::services::FeatureService::new(&project, &cache);
            let feat_a = feat_service
                .get(&mate.feature_a.id.to_string())
                .map_err(|e| miette::miette!("{}", e))?
                .ok_or_else(|| miette::miette!("Could not find feature A"))?;
            let feat_b = feat_service
                .get(&mate.feature_b.id.to_string())
                .map_err(|e| miette::miette!("{}", e))?
                .ok_or_else(|| miette::miette!("Could not find feature B"))?;

            let dim_a = feat_a.primary_dimension();
            let dim_b = feat_b.primary_dimension();
            if let (Some(dim_a), Some(dim_b)) = (dim_a, dim_b) {
                // Determine which is hole/shaft
                let (hole_dim, shaft_dim) = if dim_a.internal && !dim_b.internal {
                    (dim_a, dim_b)
                } else if !dim_a.internal && dim_b.internal {
                    (dim_b, dim_a)
                } else {
                    return Err(miette::miette!(
                        "Statistical analysis requires one internal and one external feature"
                    ));
                };

                // Calculate statistical fit
                match StatisticalFit::calculate(hole_dim, shaft_dim, args.sigma) {
                    Ok(stat) => {
                        analysis.statistical = Some(stat);
                        // Save updated mate with statistical analysis
                        let file_path = project
                            .root()
                            .join(format!("tolerances/mates/{}.tdt.yaml", mate.id));
                        let yaml_content =
                            serde_yml::to_string(&mate).map_err(|e| miette::miette!("{}", e))?;
                        fs::write(&file_path, yaml_content).into_diagnostic()?;
                    }
                    Err(e) => {
                        eprintln!(
                            "{} Could not calculate statistical fit: {}",
                            style("⚠").yellow(),
                            e
                        );
                    }
                }
            }
        }
    }

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
            "   {} Worst-case: {} ({} to {})",
            style("⬩").dim(),
            style(format!("{}", analysis.fit_result)).cyan(),
            min_rounded,
            max_rounded
        );

        // Show statistical analysis if present
        if let Some(ref stat) = analysis.statistical {
            let stat_ref_precision = stat
                .mean_clearance
                .abs()
                .max(stat.sigma_clearance)
                .max(0.001);
            println!(
                "   {} Statistical ({}σ): {} at 3σ",
                style("⬩").dim(),
                args.sigma,
                style(format!("{}", stat.fit_result_3sigma)).cyan()
            );
            println!(
                "     Mean clearance: {}, σ: {}",
                smart_round(stat.mean_clearance, stat_ref_precision),
                smart_round(stat.sigma_clearance, stat_ref_precision)
            );
            println!(
                "     3σ range: {} to {}",
                smart_round(stat.clearance_3sigma_min, stat_ref_precision),
                smart_round(stat.clearance_3sigma_max, stat_ref_precision)
            );
            // Color the interference probability based on risk
            let prob_styled = if stat.probability_interference < 0.01 {
                style(format!("{:.4}%", stat.probability_interference)).green()
            } else if stat.probability_interference < 1.0 {
                style(format!("{:.3}%", stat.probability_interference)).yellow()
            } else {
                style(format!("{:.2}%", stat.probability_interference)).red()
            };
            println!("     P(interference): {}", prob_styled);
        }
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
    use tdt_core::entities::mate::{FitResult, MateType};

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
