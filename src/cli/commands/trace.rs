//! `tdt trace` command - Traceability matrix and queries

use console::style;
use miette::{IntoDiagnostic, Result};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use crate::cli::helpers::{escape_csv, format_short_id, format_short_id_str, truncate_str};
use crate::cli::{GlobalOpts, OutputFormat};
use crate::core::cache::EntityCache;
use crate::core::identity::EntityPrefix;
use crate::core::project::Project;
use crate::core::shortid::ShortIdIndex;
use crate::entities::requirement::Requirement;
use crate::entities::test::Test;

/// A generic entity with extracted link information
#[derive(Debug, Clone)]
struct GenericEntity {
    id: String,
    title: String,
    prefix: EntityPrefix,
    outgoing_links: Vec<(String, String)>, // (link_type, target_id)
}

#[derive(clap::Subcommand, Debug)]
pub enum TraceCommands {
    /// Show traceability matrix
    Matrix(MatrixArgs),

    /// Trace from a specific entity (show what depends on it)
    From(FromArgs),

    /// Trace to a specific entity (show what it depends on)
    To(ToArgs),

    /// Find orphaned requirements (no incoming or outgoing links)
    Orphans(OrphansArgs),

    /// Coverage report - requirements with/without verification
    Coverage(CoverageArgs),
}

#[derive(clap::Args, Debug)]
pub struct MatrixArgs {
    /// Filter by source entity type
    #[arg(long)]
    pub source_type: Option<String>,

    /// Filter by target entity type
    #[arg(long)]
    pub target_type: Option<String>,

    /// Output format: table, csv, dot (graphviz)
    #[arg(long, short = 'o', default_value = "table")]
    pub output: String,

    /// Show full IDs instead of short aliases (e.g., REQ@1, TEST@2)
    #[arg(long)]
    pub ids: bool,

    /// Show Requirements Verification Matrix (requirements as source, what verifies them)
    #[arg(long)]
    pub rvm: bool,
}

#[derive(clap::Args, Debug)]
pub struct FromArgs {
    /// Entity ID to trace from
    pub id: String,

    /// Maximum depth to trace (default: unlimited)
    #[arg(long, short = 'd')]
    pub depth: Option<usize>,

    /// Show full IDs instead of short aliases
    #[arg(long)]
    pub ids: bool,
}

#[derive(clap::Args, Debug)]
pub struct ToArgs {
    /// Entity ID to trace to
    pub id: String,

    /// Maximum depth to trace (default: unlimited)
    #[arg(long, short = 'd')]
    pub depth: Option<usize>,

    /// Show full IDs instead of short aliases
    #[arg(long)]
    pub ids: bool,
}

#[derive(clap::Args, Debug)]
pub struct OrphansArgs {
    /// Only show entities without outgoing links
    #[arg(long)]
    pub no_outgoing: bool,

    /// Only show entities without incoming links
    #[arg(long)]
    pub no_incoming: bool,

    /// Filter by entity type (e.g., REQ, PROC, CTRL, NCR)
    #[arg(long, short = 't')]
    pub entity_type: Option<String>,
}

#[derive(clap::Args, Debug)]
pub struct CoverageArgs {
    /// Filter by requirement type (input/output)
    #[arg(long, short = 't')]
    pub r#type: Option<String>,

    /// Show only uncovered requirements
    #[arg(long)]
    pub uncovered: bool,
}

pub fn run(cmd: TraceCommands, global: &GlobalOpts) -> Result<()> {
    match cmd {
        TraceCommands::Matrix(args) => run_matrix(args, global),
        TraceCommands::From(args) => run_from(args),
        TraceCommands::To(args) => run_to(args),
        TraceCommands::Orphans(args) => run_orphans(args, global),
        TraceCommands::Coverage(args) => run_coverage(args, global),
    }
}

fn run_matrix(args: MatrixArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;

    // Load all entities generically
    let entities = load_all_entities(&project)?;

    // Load short ID index (default behavior is to show aliases, unless --ids is set)
    let short_ids = if !args.ids || args.rvm {
        let mut idx = ShortIdIndex::load(&project);
        idx.ensure_all(entities.iter().map(|e| e.id.clone()));
        let _ = idx.save(&project);
        Some(idx)
    } else {
        None
    };

    // Handle RVM (Requirements Verification Matrix) mode
    if args.rvm {
        return run_rvm(&entities, short_ids.as_ref(), global);
    }

    // Parse source/target type filters
    let source_filter: Option<EntityPrefix> = args
        .source_type
        .as_ref()
        .and_then(|t| t.to_uppercase().parse().ok());
    let target_filter: Option<EntityPrefix> = args
        .target_type
        .as_ref()
        .and_then(|t| t.to_uppercase().parse().ok());

    // Build a map of entity IDs to prefixes for target filtering
    let id_to_prefix: HashMap<String, EntityPrefix> =
        entities.iter().map(|e| (e.id.clone(), e.prefix)).collect();

    // Determine format - prefer args.output, fallback to global
    let use_dot = args.output == "dot";
    let use_csv = args.output == "csv" || matches!(global.format, OutputFormat::Csv);
    let use_json = matches!(global.format, OutputFormat::Json);

    if use_json {
        // JSON format - structured link data
        #[derive(serde::Serialize)]
        struct Link {
            source_id: String,
            source_type: String,
            source_title: String,
            link_type: String,
            target_id: String,
        }

        let mut links = Vec::new();
        for entity in &entities {
            // Apply source filter
            if let Some(filter) = source_filter {
                if entity.prefix != filter {
                    continue;
                }
            }

            for (link_type, target) in &entity.outgoing_links {
                // Apply target filter
                if let Some(filter) = target_filter {
                    if let Some(target_prefix) = id_to_prefix.get(target) {
                        if *target_prefix != filter {
                            continue;
                        }
                    }
                }

                links.push(Link {
                    source_id: entity.id.clone(),
                    source_type: entity.prefix.to_string(),
                    source_title: entity.title.clone(),
                    link_type: link_type.clone(),
                    target_id: target.clone(),
                });
            }
        }
        let json = serde_json::to_string_pretty(&links).into_diagnostic()?;
        println!("{}", json);
        return Ok(());
    }

    if !use_dot && !use_csv {
        println!("{}", style("Traceability Matrix").bold());
        println!("{}", style("═".repeat(60)).dim());
    }

    if use_dot {
        // GraphViz DOT format
        println!("digraph traceability {{");
        println!("  rankdir=LR;");
        println!("  node [shape=box];");
        println!();

        for entity in &entities {
            // Apply source filter
            if let Some(filter) = source_filter {
                if entity.prefix != filter {
                    continue;
                }
            }

            let short_id = format_short_id_str(&entity.id);
            let label = format!("{}\\n{}", short_id, truncate_str(&entity.title, 20));
            println!("  \"{}\" [label=\"{}\"];", entity.id, label);

            for (link_type, target) in &entity.outgoing_links {
                // Apply target filter
                if let Some(filter) = target_filter {
                    if let Some(target_prefix) = id_to_prefix.get(target) {
                        if *target_prefix != filter {
                            continue;
                        }
                    }
                }
                println!(
                    "  \"{}\" -> \"{}\" [label=\"{}\"];",
                    entity.id, target, link_type
                );
            }
        }
        println!("}}");
    } else if use_csv {
        // CSV format
        println!("source_id,source_type,source_title,link_type,target_id");
        for entity in &entities {
            // Apply source filter
            if let Some(filter) = source_filter {
                if entity.prefix != filter {
                    continue;
                }
            }

            for (link_type, target) in &entity.outgoing_links {
                // Apply target filter
                if let Some(filter) = target_filter {
                    if let Some(target_prefix) = id_to_prefix.get(target) {
                        if *target_prefix != filter {
                            continue;
                        }
                    }
                }
                println!(
                    "{},{},{},{},{}",
                    entity.id,
                    entity.prefix,
                    escape_csv(&entity.title),
                    link_type,
                    target
                );
            }
        }
    } else {
        // Table format
        println!(
            "{:<16} {:<30} {:<14} {:<16}",
            style("SOURCE").bold(),
            style("TITLE").bold(),
            style("LINK TYPE").bold(),
            style("TARGET").bold()
        );
        println!("{}", "-".repeat(76));

        let mut has_links = false;
        for entity in &entities {
            // Apply source filter
            if let Some(filter) = source_filter {
                if entity.prefix != filter {
                    continue;
                }
            }

            // Use alias (REQ@1) if --aliases flag set, otherwise truncated full ID
            let source_display = if let Some(ref idx) = short_ids {
                idx.get_short_id(&entity.id)
                    .unwrap_or_else(|| format_short_id_str(&entity.id))
            } else {
                format_short_id_str(&entity.id)
            };
            let title = truncate_str(&entity.title, 28);

            for (link_type, target) in &entity.outgoing_links {
                // Apply target filter
                if let Some(filter) = target_filter {
                    if let Some(target_prefix) = id_to_prefix.get(target) {
                        if *target_prefix != filter {
                            continue;
                        }
                    }
                }

                has_links = true;
                // Use alias for target too
                let target_display = if let Some(ref idx) = short_ids {
                    idx.get_short_id(target)
                        .unwrap_or_else(|| format_short_id_str(target))
                } else {
                    format_short_id_str(target)
                };
                println!(
                    "{:<16} {:<30} {:<14} {:<16}",
                    source_display,
                    title,
                    style(link_type).cyan(),
                    target_display
                );
            }
        }

        if !has_links {
            println!("  {}", style("No links found in project").dim());
        }
    }

    Ok(())
}

/// Requirements Verification Matrix - shows requirements as source with what verifies them
fn run_rvm(
    entities: &[GenericEntity],
    short_ids: Option<&ShortIdIndex>,
    _global: &GlobalOpts,
) -> Result<()> {
    // Build reverse lookup: target_id -> Vec<(source_id, link_type)>
    // This shows what entities point TO each entity
    let mut incoming_links: HashMap<String, Vec<(String, String)>> = HashMap::new();
    for entity in entities {
        for (link_type, target) in &entity.outgoing_links {
            incoming_links
                .entry(target.clone())
                .or_default()
                .push((entity.id.clone(), link_type.clone()));
        }
    }

    // Get all requirements
    let requirements: Vec<&GenericEntity> = entities
        .iter()
        .filter(|e| e.prefix == EntityPrefix::Req)
        .collect();

    if requirements.is_empty() {
        println!("{}", style("No requirements found in project").dim());
        return Ok(());
    }

    println!("{}", style("Requirements Verification Matrix").bold());
    println!("{}", "═".repeat(76));
    println!(
        "{:<12} {:<32} {:<12} {:<16}",
        style("REQ").bold(),
        style("TITLE").bold(),
        style("STATUS").bold(),
        style("VERIFIED BY").bold()
    );
    println!("{}", "-".repeat(76));

    let mut verified_count = 0;
    let mut total_count = 0;

    for req in &requirements {
        total_count += 1;

        let req_display = if let Some(idx) = short_ids {
            idx.get_short_id(&req.id)
                .unwrap_or_else(|| format_short_id_str(&req.id))
        } else {
            format_short_id_str(&req.id)
        };

        let title = truncate_str(&req.title, 30);

        // Find all entities that verify this requirement
        let verifiers: Vec<String> = incoming_links
            .get(&req.id)
            .map(|links| {
                links
                    .iter()
                    .filter(|(_, link_type)| link_type == "verifies")
                    .map(|(source_id, _)| {
                        if let Some(idx) = short_ids {
                            idx.get_short_id(source_id)
                                .unwrap_or_else(|| format_short_id_str(source_id))
                        } else {
                            format_short_id_str(source_id)
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();

        let (status, verifier_str) = if verifiers.is_empty() {
            (style("unverified").yellow(), style("-").dim().to_string())
        } else {
            verified_count += 1;
            (style("verified").green(), verifiers.join(", "))
        };

        println!(
            "{:<12} {:<32} {:<12} {}",
            style(&req_display).cyan(),
            title,
            status,
            verifier_str
        );
    }

    println!("{}", "-".repeat(76));
    let coverage_pct = if total_count > 0 {
        (verified_count as f64 / total_count as f64 * 100.0) as u32
    } else {
        0
    };
    println!(
        "Coverage: {} ({}/{})",
        style(format!("{}%", coverage_pct)).cyan(),
        verified_count,
        total_count
    );

    Ok(())
}

fn run_from(args: FromArgs) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;

    // Load all entities first
    let entities = load_all_entities(&project)?;

    // Load short ID index (default behavior is to show aliases, unless --ids is set)
    let mut short_ids = ShortIdIndex::load(&project);
    if !args.ids {
        short_ids.ensure_all(entities.iter().map(|e| e.id.clone()));
        let _ = short_ids.save(&project);
    }

    let resolved_id = short_ids
        .resolve(&args.id)
        .unwrap_or_else(|| args.id.clone());

    // Find the starting entity
    let source = entities
        .iter()
        .find(|e| {
            e.id.starts_with(&resolved_id)
                || e.title.to_lowercase().contains(&resolved_id.to_lowercase())
        })
        .ok_or_else(|| miette::miette!("Entity '{}' not found", args.id))?;

    // Display source with alias unless --ids is set
    let source_display = if !args.ids {
        short_ids
            .get_short_id(&source.id)
            .unwrap_or_else(|| source.id.clone())
    } else {
        source.id.clone()
    };

    println!(
        "{} Tracing from: {} - {}",
        style("→").blue(),
        style(&source_display).cyan(),
        source.title
    );
    println!();

    // Build ID to title map for display
    let id_to_title: HashMap<String, String> = entities
        .iter()
        .map(|e| (e.id.clone(), e.title.clone()))
        .collect();

    // Build adjacency list for incoming links (what points TO each entity)
    let mut incoming: HashMap<String, Vec<(String, String)>> = HashMap::new();
    for entity in &entities {
        for (link_type, target) in &entity.outgoing_links {
            incoming
                .entry(target.clone())
                .or_default()
                .push((entity.id.clone(), link_type.clone()));
        }
    }

    // BFS to find what depends on this entity
    let mut visited = HashSet::new();
    let mut queue: Vec<(String, usize)> = vec![(source.id.clone(), 0)];
    let max_depth = args.depth.unwrap_or(usize::MAX);

    println!("{}", style("Entities that depend on this:").bold());

    while let Some((id, depth)) = queue.pop() {
        if depth > max_depth {
            continue;
        }
        if visited.contains(&id) {
            continue;
        }
        visited.insert(id.clone());

        if depth > 0 {
            let indent = "  ".repeat(depth);
            let id_display = if !args.ids {
                short_ids
                    .get_short_id(&id)
                    .unwrap_or_else(|| format_short_id_str(&id))
            } else {
                id.clone()
            };
            let title = id_to_title
                .get(&id)
                .map(|t| truncate_str(t, 40))
                .unwrap_or_default();
            println!("{}← {} - {}", indent, style(&id_display).cyan(), title);
        }

        if let Some(deps) = incoming.get(&id) {
            for (dep_id, _link_type) in deps {
                if !visited.contains(dep_id) {
                    queue.push((dep_id.clone(), depth + 1));
                }
            }
        }
    }

    if visited.len() == 1 {
        println!("  {}", style("(none)").dim());
    }

    Ok(())
}

fn run_to(args: ToArgs) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;

    // Load all entities first
    let entities = load_all_entities(&project)?;

    // Load short ID index (default behavior is to show aliases, unless --ids is set)
    let mut short_ids = ShortIdIndex::load(&project);
    if !args.ids {
        short_ids.ensure_all(entities.iter().map(|e| e.id.clone()));
        let _ = short_ids.save(&project);
    }

    let resolved_id = short_ids
        .resolve(&args.id)
        .unwrap_or_else(|| args.id.clone());

    // Find the target entity
    let target = entities
        .iter()
        .find(|e| {
            e.id.starts_with(&resolved_id)
                || e.title.to_lowercase().contains(&resolved_id.to_lowercase())
        })
        .ok_or_else(|| miette::miette!("Entity '{}' not found", args.id))?;

    // Display target with alias unless --ids is set
    let target_display = if !args.ids {
        short_ids
            .get_short_id(&target.id)
            .unwrap_or_else(|| target.id.clone())
    } else {
        target.id.clone()
    };

    println!(
        "{} Tracing to: {} - {}",
        style("→").blue(),
        style(&target_display).cyan(),
        target.title
    );
    println!();

    // Build ID to title map for display
    let id_to_title: HashMap<String, String> = entities
        .iter()
        .map(|e| (e.id.clone(), e.title.clone()))
        .collect();

    // Build adjacency list for outgoing links
    let mut outgoing: HashMap<String, Vec<(String, String)>> = HashMap::new();
    for entity in &entities {
        if !entity.outgoing_links.is_empty() {
            outgoing.insert(entity.id.clone(), entity.outgoing_links.clone());
        }
    }

    // BFS to find what this entity depends on
    let mut visited = HashSet::new();
    let mut queue: Vec<(String, usize)> = vec![(target.id.clone(), 0)];
    let max_depth = args.depth.unwrap_or(usize::MAX);

    println!("{}", style("Dependencies:").bold());

    while let Some((id, depth)) = queue.pop() {
        if depth > max_depth {
            continue;
        }
        if visited.contains(&id) {
            continue;
        }
        visited.insert(id.clone());

        if depth > 0 {
            let indent = "  ".repeat(depth);
            let id_display = if !args.ids {
                short_ids
                    .get_short_id(&id)
                    .unwrap_or_else(|| format_short_id_str(&id))
            } else {
                id.clone()
            };
            let title = id_to_title
                .get(&id)
                .map(|t| truncate_str(t, 40))
                .unwrap_or_default();
            println!("{}→ {} - {}", indent, style(&id_display).cyan(), title);
        }

        if let Some(deps) = outgoing.get(&id) {
            for (_, dep_id) in deps {
                if !visited.contains(dep_id) {
                    queue.push((dep_id.clone(), depth + 1));
                }
            }
        }
    }

    if visited.len() == 1 {
        println!("  {}", style("(none)").dim());
    }

    Ok(())
}

fn run_orphans(args: OrphansArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;

    // Use cache for fast orphan detection (single SQL query instead of full filesystem scan)
    let cache = EntityCache::open(&project)?;

    // Parse entity type filter if provided
    let type_filter: Option<&str> = args.entity_type.as_ref().map(|t| {
        // Convert to uppercase prefix format
        match t.to_uppercase().as_str() {
            "REQ" => "REQ",
            "RISK" => "RISK",
            "TEST" => "TEST",
            "RSLT" => "RSLT",
            "CMP" => "CMP",
            "ASM" => "ASM",
            "QUOTE" => "QUOTE",
            "SUP" => "SUP",
            "PROC" => "PROC",
            "CTRL" => "CTRL",
            "WORK" => "WORK",
            "NCR" => "NCR",
            "CAPA" => "CAPA",
            "FEAT" => "FEAT",
            "MATE" => "MATE",
            "TOL" => "TOL",
            _ => t.as_str(),
        }
    });

    // Use cache's find_orphans for entities with NO links (either direction)
    // For more specific filters (no_outgoing only, no_incoming only), we need custom logic
    let orphans = if args.no_outgoing && !args.no_incoming {
        // Entities with no outgoing links
        let all_entities = cache.list_entities(&Default::default());
        all_entities
            .into_iter()
            .filter(|e| {
                if let Some(filter) = type_filter {
                    if e.prefix != filter {
                        return false;
                    }
                }
                cache.get_links_from(&e.id).is_empty()
            })
            .map(|e| (e, "no outgoing"))
            .collect::<Vec<_>>()
    } else if args.no_incoming && !args.no_outgoing {
        // Entities with no incoming links
        let all_entities = cache.list_entities(&Default::default());
        all_entities
            .into_iter()
            .filter(|e| {
                if let Some(filter) = type_filter {
                    if e.prefix != filter {
                        return false;
                    }
                }
                cache.get_links_to(&e.id).is_empty()
            })
            .map(|e| (e, "no incoming"))
            .collect::<Vec<_>>()
    } else {
        // Default: entities with NO links at all (most common case)
        // This uses an optimized SQL query in the cache
        cache
            .find_orphans(type_filter)
            .into_iter()
            .map(|e| (e, "no links"))
            .collect::<Vec<_>>()
    };

    // Output based on format
    match global.format {
        OutputFormat::Json => {
            #[derive(serde::Serialize)]
            struct OrphanInfo {
                id: String,
                entity_type: String,
                title: String,
                reason: String,
            }
            let data: Vec<_> = orphans
                .iter()
                .map(|(entity, reason)| OrphanInfo {
                    id: entity.id.clone(),
                    entity_type: entity.prefix.clone(),
                    title: entity.title.clone(),
                    reason: reason.to_string(),
                })
                .collect();
            let json = serde_json::to_string_pretty(&data).into_diagnostic()?;
            println!("{}", json);
        }
        OutputFormat::Id => {
            for (entity, _) in &orphans {
                println!("{}", entity.id);
            }
        }
        _ => {
            let type_label = type_filter.map(|t| format!("{} ", t)).unwrap_or_default();
            println!(
                "{}",
                style(format!("Orphaned {}Entities", type_label)).bold()
            );
            println!("{}", style("─".repeat(60)).dim());

            for (entity, reason) in &orphans {
                println!(
                    "{} {} - {} ({})",
                    style("○").yellow(),
                    format_short_id_str(&entity.id),
                    truncate_str(&entity.title, 40),
                    style(reason).dim()
                );
            }

            println!();
            if orphans.is_empty() {
                println!("{} No orphaned entities found!", style("✓").green().bold());
            } else {
                println!(
                    "Found {} orphaned entity(ies)",
                    style(orphans.len()).yellow()
                );
            }
        }
    }

    Ok(())
}

fn run_coverage(args: CoverageArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let reqs = load_all_requirements(&project)?;
    let tests = load_all_tests(&project);

    // Build set of requirement IDs that are verified by tests (via test.links.verifies)
    let mut verified_by_tests: HashSet<String> = HashSet::new();
    for test in &tests {
        for req_id in &test.links.verifies {
            verified_by_tests.insert(req_id.to_string());
        }
    }

    // Filter by type if specified
    let filtered: Vec<&Requirement> = reqs
        .iter()
        .filter(|r| {
            if let Some(ref t) = args.r#type {
                r.req_type.to_string().to_lowercase() == t.to_lowercase()
            } else {
                true
            }
        })
        .collect();

    let total = filtered.len();
    let mut covered = 0;
    let mut uncovered_list = Vec::new();

    for req in &filtered {
        // Check both: req.links.verified_by AND tests that verify this req
        let has_verification =
            !req.links.verified_by.is_empty() || verified_by_tests.contains(&req.id.to_string());
        if has_verification {
            covered += 1;
        } else {
            uncovered_list.push(*req);
        }
    }

    let coverage_pct = if total > 0 {
        (covered as f64 / total as f64) * 100.0
    } else {
        100.0
    };

    // Output based on format
    match global.format {
        OutputFormat::Json => {
            #[derive(serde::Serialize)]
            struct CoverageReport {
                total: usize,
                covered: usize,
                uncovered: usize,
                coverage_percent: f64,
                uncovered_ids: Vec<String>,
            }
            let report = CoverageReport {
                total,
                covered,
                uncovered: uncovered_list.len(),
                coverage_percent: coverage_pct,
                uncovered_ids: uncovered_list.iter().map(|r| r.id.to_string()).collect(),
            };
            let json = serde_json::to_string_pretty(&report).into_diagnostic()?;
            println!("{}", json);
        }
        OutputFormat::Id => {
            // Just output uncovered IDs
            for req in &uncovered_list {
                println!("{}", req.id);
            }
        }
        _ => {
            println!("{}", style("Verification Coverage Report").bold());
            println!("{}", style("═".repeat(60)).dim());
            println!();
            println!("Total requirements:     {}", style(total).cyan());
            println!("With verification:      {}", style(covered).green());
            println!(
                "Without verification:   {}",
                if uncovered_list.is_empty() {
                    style(uncovered_list.len()).green()
                } else {
                    style(uncovered_list.len()).red()
                }
            );
            println!();
            println!(
                "Coverage: {}",
                if coverage_pct >= 100.0 {
                    style(format!("{:.1}%", coverage_pct)).green().bold()
                } else if coverage_pct >= 80.0 {
                    style(format!("{:.1}%", coverage_pct)).yellow()
                } else {
                    style(format!("{:.1}%", coverage_pct)).red()
                }
            );

            if !uncovered_list.is_empty() && (args.uncovered || uncovered_list.len() <= 10) {
                println!();
                println!("{}", style("Uncovered Requirements:").bold());
                println!("{}", style("─".repeat(60)).dim());

                for req in &uncovered_list {
                    println!(
                        "  {} {} - {}",
                        style("○").red(),
                        format_short_id(&req.id),
                        truncate_str(&req.title, 45)
                    );
                }
            } else if !uncovered_list.is_empty() {
                println!();
                println!(
                    "Use {} to see the full list",
                    style("tdt trace coverage --uncovered").yellow()
                );
            }
        }
    }

    Ok(())
}

/// Load all requirements from the project
fn load_all_requirements(project: &Project) -> Result<Vec<Requirement>> {
    let mut reqs = Vec::new();

    for path in project.iter_entity_files(EntityPrefix::Req) {
        if let Ok(req) = crate::yaml::parse_yaml_file::<Requirement>(&path) {
            reqs.push(req);
        }
    }

    // Also check outputs directory
    let outputs_dir = project.root().join("requirements/outputs");
    if outputs_dir.exists() {
        for entry in walkdir::WalkDir::new(&outputs_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        {
            if let Ok(req) = crate::yaml::parse_yaml_file::<Requirement>(entry.path()) {
                reqs.push(req);
            }
        }
    }

    Ok(reqs)
}

/// Load all test protocols from the project
fn load_all_tests(project: &Project) -> Vec<Test> {
    let mut tests = Vec::new();

    for subdir in &["verification/protocols", "validation/protocols"] {
        let dir = project.root().join(subdir);
        if dir.exists() {
            for entry in walkdir::WalkDir::new(&dir)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
                .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
            {
                if let Ok(test) = crate::yaml::parse_yaml_file::<Test>(entry.path()) {
                    tests.push(test);
                }
            }
        }
    }

    tests
}

/// Load all entities from the project (generic version)
fn load_all_entities(project: &Project) -> Result<Vec<GenericEntity>> {
    let mut entities = Vec::new();

    // Iterate over all entity types
    for prefix in EntityPrefix::all() {
        for path in project.iter_entity_files(*prefix) {
            if let Ok(entity) = load_generic_entity(&path, *prefix) {
                entities.push(entity);
            }
        }
    }

    // Also check additional directories that may not be covered by iter_entity_files
    let additional_dirs = [
        ("requirements/outputs", EntityPrefix::Req),
        ("verification/results", EntityPrefix::Rslt),
        ("validation/results", EntityPrefix::Rslt),
        ("validation/protocols", EntityPrefix::Test),
    ];

    for (dir, prefix) in additional_dirs {
        let full_path = project.root().join(dir);
        if full_path.exists() {
            for entry in walkdir::WalkDir::new(&full_path)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
                .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
            {
                if let Ok(entity) = load_generic_entity(&entry.path().to_path_buf(), prefix) {
                    // Avoid duplicates
                    if !entities.iter().any(|e| e.id == entity.id) {
                        entities.push(entity);
                    }
                }
            }
        }
    }

    Ok(entities)
}

/// Load a single entity generically by parsing YAML
fn load_generic_entity(path: &PathBuf, prefix: EntityPrefix) -> Result<GenericEntity> {
    let content = std::fs::read_to_string(path).into_diagnostic()?;
    let value: serde_yml::Value = serde_yml::from_str(&content).into_diagnostic()?;

    let id = value
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| miette::miette!("Missing id in {:?}", path))?
        .to_string();

    let title = value
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let mut outgoing_links = Vec::new();

    // Extract links from the 'links' field
    if let Some(links) = value.get("links") {
        if let Some(links_map) = links.as_mapping() {
            for (key, value) in links_map {
                if let Some(link_type) = key.as_str() {
                    // Handle array of links
                    if let Some(arr) = value.as_sequence() {
                        for item in arr {
                            if let Some(target) = item.as_str() {
                                outgoing_links.push((link_type.to_string(), target.to_string()));
                            }
                        }
                    }
                    // Handle single link
                    else if let Some(target) = value.as_str() {
                        outgoing_links.push((link_type.to_string(), target.to_string()));
                    }
                }
            }
        }
    }

    // Also extract top-level reference fields that act as links
    // These are fields that contain entity IDs but aren't in the links section
    let reference_fields = [
        "supplier",  // Quote -> Supplier
        "component", // Quote -> Component, NCR -> Component
        "assembly",  // Quote -> Assembly
        "process",   // Control -> Process, WorkInstruction -> Process, NCR -> Process
        "feature",   // Control -> Feature
        "control",   // NCR -> Control
        "capa",      // NCR -> CAPA
    ];

    for field in reference_fields {
        if let Some(val) = value.get(field) {
            if let Some(target) = val.as_str() {
                // Only add if it looks like an entity ID (contains a prefix pattern)
                if target.contains('-') && target.len() > 4 {
                    outgoing_links.push((field.to_string(), target.to_string()));
                }
            }
        }
    }

    // Handle TOL entity contributors -> feature references
    if prefix == EntityPrefix::Tol {
        if let Some(contributors) = value.get("contributors").and_then(|v| v.as_sequence()) {
            for contrib in contributors {
                // Each contributor may have a feature reference with id
                if let Some(feature_ref) = contrib.get("feature") {
                    if let Some(feat_id) = feature_ref.get("id").and_then(|v| v.as_str()) {
                        outgoing_links.push(("uses_feature".to_string(), feat_id.to_string()));
                    }
                }
            }
        }
    }

    // Handle MATE entity feature references (feature_a, feature_b)
    if prefix == EntityPrefix::Mate {
        for field in ["feature_a", "feature_b"] {
            if let Some(feat_ref) = value.get(field) {
                if let Some(feat_id) = feat_ref.get("id").and_then(|v| v.as_str()) {
                    outgoing_links.push(("mates".to_string(), feat_id.to_string()));
                }
            }
        }
    }

    // Handle FEAT entity component reference
    if prefix == EntityPrefix::Feat {
        if let Some(component) = value.get("component").and_then(|v| v.as_str()) {
            if component.contains('-') && component.len() > 4 {
                outgoing_links.push(("belongs_to".to_string(), component.to_string()));
            }
        }
    }

    Ok(GenericEntity {
        id,
        title,
        prefix,
        outgoing_links,
    })
}
