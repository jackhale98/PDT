//! `pdt trace` command - Traceability matrix and queries

use console::style;
use miette::Result;
use std::collections::{HashMap, HashSet};

use crate::core::identity::{EntityId, EntityPrefix};
use crate::core::project::Project;
use crate::entities::requirement::Requirement;

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
}

#[derive(clap::Args, Debug)]
pub struct FromArgs {
    /// Entity ID to trace from
    pub id: String,

    /// Maximum depth to trace (default: unlimited)
    #[arg(long, short = 'd')]
    pub depth: Option<usize>,
}

#[derive(clap::Args, Debug)]
pub struct ToArgs {
    /// Entity ID to trace to
    pub id: String,

    /// Maximum depth to trace (default: unlimited)
    #[arg(long, short = 'd')]
    pub depth: Option<usize>,
}

#[derive(clap::Args, Debug)]
pub struct OrphansArgs {
    /// Only show requirements without outgoing links
    #[arg(long)]
    pub no_outgoing: bool,

    /// Only show requirements without incoming links
    #[arg(long)]
    pub no_incoming: bool,
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

pub fn run(cmd: TraceCommands) -> Result<()> {
    match cmd {
        TraceCommands::Matrix(args) => run_matrix(args),
        TraceCommands::From(args) => run_from(args),
        TraceCommands::To(args) => run_to(args),
        TraceCommands::Orphans(args) => run_orphans(args),
        TraceCommands::Coverage(args) => run_coverage(args),
    }
}

fn run_matrix(args: MatrixArgs) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;

    // Build the traceability data
    let reqs = load_all_requirements(&project)?;

    println!("{}", style("Traceability Matrix").bold());
    println!("{}", style("═".repeat(60)).dim());

    if args.output == "dot" {
        // GraphViz DOT format
        println!("digraph traceability {{");
        println!("  rankdir=LR;");
        println!("  node [shape=box];");
        println!();

        for req in &reqs {
            let short_id = format_short_id(&req.id);
            let label = format!("{}\\n{}", short_id, truncate(&req.title, 20));
            println!("  \"{}\" [label=\"{}\"];", req.id, label);

            for target in &req.links.satisfied_by {
                println!("  \"{}\" -> \"{}\" [label=\"satisfied_by\"];", req.id, target);
            }
            for target in &req.links.verified_by {
                println!("  \"{}\" -> \"{}\" [label=\"verified_by\", style=dashed];", req.id, target);
            }
        }
        println!("}}");
    } else if args.output == "csv" {
        // CSV format
        println!("source_id,source_title,link_type,target_id");
        for req in &reqs {
            for target in &req.links.satisfied_by {
                println!("{},{},satisfied_by,{}", req.id, escape_csv(&req.title), target);
            }
            for target in &req.links.verified_by {
                println!("{},{},verified_by,{}", req.id, escape_csv(&req.title), target);
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
        for req in &reqs {
            let short_id = format_short_id(&req.id);
            let title = truncate(&req.title, 28);

            for target in &req.links.satisfied_by {
                has_links = true;
                let target_short = format_short_id(target);
                println!(
                    "{:<16} {:<30} {:<14} {:<16}",
                    short_id,
                    title,
                    style("satisfied_by").cyan(),
                    target_short
                );
            }
            for target in &req.links.verified_by {
                has_links = true;
                let target_short = format_short_id(target);
                println!(
                    "{:<16} {:<30} {:<14} {:<16}",
                    short_id,
                    title,
                    style("verified_by").yellow(),
                    target_short
                );
            }
        }

        if !has_links {
            println!("  {}", style("No links found in project").dim());
        }
    }

    Ok(())
}

fn run_from(args: FromArgs) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;

    // Find the starting entity
    let reqs = load_all_requirements(&project)?;
    let source = reqs.iter()
        .find(|r| r.id.to_string().starts_with(&args.id) || r.title.to_lowercase().contains(&args.id.to_lowercase()))
        .ok_or_else(|| miette::miette!("Entity '{}' not found", args.id))?;

    println!(
        "{} Tracing from: {} - {}",
        style("→").blue(),
        style(&source.id.to_string()).cyan(),
        source.title
    );
    println!();

    // Build adjacency list for incoming links
    let mut incoming: HashMap<String, Vec<(String, String)>> = HashMap::new();
    for req in &reqs {
        for target in &req.links.satisfied_by {
            incoming
                .entry(target.to_string())
                .or_default()
                .push((req.id.to_string(), "satisfied_by".to_string()));
        }
        for target in &req.links.verified_by {
            incoming
                .entry(target.to_string())
                .or_default()
                .push((req.id.to_string(), "verified_by".to_string()));
        }
    }

    // BFS to find what depends on this entity
    let mut visited = HashSet::new();
    let mut queue: Vec<(String, usize)> = vec![(source.id.to_string(), 0)];
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
            println!("{}← {}", indent, format_id_short(&id));
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

    // Find the target entity
    let reqs = load_all_requirements(&project)?;
    let target = reqs.iter()
        .find(|r| r.id.to_string().starts_with(&args.id) || r.title.to_lowercase().contains(&args.id.to_lowercase()))
        .ok_or_else(|| miette::miette!("Entity '{}' not found", args.id))?;

    println!(
        "{} Tracing to: {} - {}",
        style("→").blue(),
        style(&target.id.to_string()).cyan(),
        target.title
    );
    println!();

    // Build adjacency list for outgoing links
    let mut outgoing: HashMap<String, Vec<(String, String)>> = HashMap::new();
    for req in &reqs {
        let mut links = Vec::new();
        for t in &req.links.satisfied_by {
            links.push((t.to_string(), "satisfied_by".to_string()));
        }
        for t in &req.links.verified_by {
            links.push((t.to_string(), "verified_by".to_string()));
        }
        if !links.is_empty() {
            outgoing.insert(req.id.to_string(), links);
        }
    }

    // BFS to find what this entity depends on
    let mut visited = HashSet::new();
    let mut queue: Vec<(String, usize)> = vec![(target.id.to_string(), 0)];
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
            println!("{}→ {}", indent, format_id_short(&id));
        }

        if let Some(deps) = outgoing.get(&id) {
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

fn run_orphans(args: OrphansArgs) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let reqs = load_all_requirements(&project)?;

    // Build incoming links map
    let mut has_incoming: HashSet<String> = HashSet::new();
    for req in &reqs {
        for target in &req.links.satisfied_by {
            has_incoming.insert(target.to_string());
        }
        for target in &req.links.verified_by {
            has_incoming.insert(target.to_string());
        }
    }

    println!("{}", style("Orphaned Requirements").bold());
    println!("{}", style("─".repeat(60)).dim());

    let mut orphan_count = 0;

    for req in &reqs {
        let id_str = req.id.to_string();
        let has_outgoing = !req.links.satisfied_by.is_empty() || !req.links.verified_by.is_empty();
        let has_inc = has_incoming.contains(&id_str);

        let is_orphan = if args.no_outgoing && args.no_incoming {
            !has_outgoing && !has_inc
        } else if args.no_outgoing {
            !has_outgoing
        } else if args.no_incoming {
            !has_inc
        } else {
            !has_outgoing && !has_inc
        };

        if is_orphan {
            orphan_count += 1;
            let reason = if !has_outgoing && !has_inc {
                "no links"
            } else if !has_outgoing {
                "no outgoing"
            } else {
                "no incoming"
            };

            println!(
                "{} {} - {} ({})",
                style("○").yellow(),
                format_short_id(&req.id),
                truncate(&req.title, 40),
                style(reason).dim()
            );
        }
    }

    println!();
    if orphan_count == 0 {
        println!(
            "{} No orphaned requirements found!",
            style("✓").green().bold()
        );
    } else {
        println!(
            "Found {} orphaned requirement(s)",
            style(orphan_count).yellow()
        );
    }

    Ok(())
}

fn run_coverage(args: CoverageArgs) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let reqs = load_all_requirements(&project)?;

    // Filter by type if specified
    let filtered: Vec<&Requirement> = reqs.iter()
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
        let has_verification = !req.links.verified_by.is_empty();
        if has_verification {
            covered += 1;
        } else {
            uncovered_list.push(req);
        }
    }

    let coverage_pct = if total > 0 {
        (covered as f64 / total as f64) * 100.0
    } else {
        100.0
    };

    println!("{}", style("Verification Coverage Report").bold());
    println!("{}", style("═".repeat(60)).dim());
    println!();
    println!(
        "Total requirements:     {}",
        style(total).cyan()
    );
    println!(
        "With verification:      {}",
        style(covered).green()
    );
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
        "Coverage: {:.1}%",
        if coverage_pct >= 100.0 {
            style(format!("{:.1}", coverage_pct)).green().bold()
        } else if coverage_pct >= 80.0 {
            style(format!("{:.1}", coverage_pct)).yellow()
        } else {
            style(format!("{:.1}", coverage_pct)).red()
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
                truncate(&req.title, 45)
            );
        }
    } else if !uncovered_list.is_empty() {
        println!();
        println!(
            "Use {} to see the full list",
            style("pdt trace coverage --uncovered").yellow()
        );
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
            .filter(|e| e.path().to_string_lossy().ends_with(".pdt.yaml"))
        {
            if let Ok(req) = crate::yaml::parse_yaml_file::<Requirement>(entry.path()) {
                reqs.push(req);
            }
        }
    }

    Ok(reqs)
}

/// Format an entity ID for short display
fn format_short_id(id: &EntityId) -> String {
    let full = id.to_string();
    if full.len() > 12 {
        format!("{}...", &full[..12])
    } else {
        full
    }
}

/// Format a string ID for short display
fn format_id_short(id: &str) -> String {
    if id.len() > 16 {
        format!("{}...", &id[..13])
    } else {
        id.to_string()
    }
}

/// Truncate a string
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

/// Escape a string for CSV
fn escape_csv(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}
