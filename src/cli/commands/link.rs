//! `pdt link` command - Manage links between entities

use console::style;
use miette::{IntoDiagnostic, Result};
use std::fs;

use crate::core::identity::EntityId;
use crate::core::project::Project;
use crate::entities::requirement::Requirement;

#[derive(clap::Subcommand, Debug)]
pub enum LinkCommands {
    /// Add a link between two entities
    Add(AddLinkArgs),

    /// Remove a link between two entities
    Remove(RemoveLinkArgs),

    /// Show all links for an entity
    Show(ShowLinksArgs),

    /// Find broken links (references to non-existent entities)
    Check(CheckLinksArgs),
}

#[derive(clap::Args, Debug)]
pub struct AddLinkArgs {
    /// Source entity ID (or partial ID)
    pub source: String,

    /// Link type (satisfied_by, verified_by, etc.)
    #[arg(long, short = 't')]
    pub link_type: String,

    /// Target entity ID (or partial ID)
    pub target: String,

    /// Add reciprocal link (target -> source)
    #[arg(long)]
    pub reciprocal: bool,
}

#[derive(clap::Args, Debug)]
pub struct RemoveLinkArgs {
    /// Source entity ID (or partial ID)
    pub source: String,

    /// Link type (satisfied_by, verified_by, etc.)
    #[arg(long, short = 't')]
    pub link_type: String,

    /// Target entity ID (or partial ID)
    pub target: String,

    /// Remove reciprocal link too
    #[arg(long)]
    pub reciprocal: bool,
}

#[derive(clap::Args, Debug)]
pub struct ShowLinksArgs {
    /// Entity ID (or partial ID)
    pub id: String,

    /// Show outgoing links only
    #[arg(long)]
    pub outgoing: bool,

    /// Show incoming links only
    #[arg(long)]
    pub incoming: bool,
}

#[derive(clap::Args, Debug)]
pub struct CheckLinksArgs {
    /// Fix broken links by removing them
    #[arg(long)]
    pub fix: bool,
}

pub fn run(cmd: LinkCommands) -> Result<()> {
    match cmd {
        LinkCommands::Add(args) => run_add(args),
        LinkCommands::Remove(args) => run_remove(args),
        LinkCommands::Show(args) => run_show(args),
        LinkCommands::Check(args) => run_check(args),
    }
}

fn run_add(args: AddLinkArgs) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;

    // Find source entity
    let (source_req, source_path) = find_requirement(&project, &args.source)?;

    // Validate target exists (if it's a known prefix)
    let target_id = resolve_entity_id(&project, &args.target)?;

    // Parse link type
    let link_type = args.link_type.to_lowercase();

    // Read the current file content
    let content = fs::read_to_string(&source_path).into_diagnostic()?;

    // Add the link to the appropriate array
    let updated_content = add_link_to_yaml(&content, &link_type, &target_id.to_string())?;

    // Write back
    fs::write(&source_path, &updated_content).into_diagnostic()?;

    println!(
        "{} Added link: {} --[{}]--> {}",
        style("✓").green(),
        format_short_id(&source_req.id),
        style(&link_type).cyan(),
        format_short_id(&target_id)
    );

    if args.reciprocal {
        // TODO: Add reciprocal link
        println!(
            "{} Reciprocal linking not yet implemented",
            style("!").yellow()
        );
    }

    Ok(())
}

fn run_remove(args: RemoveLinkArgs) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;

    // Find source entity
    let (source_req, source_path) = find_requirement(&project, &args.source)?;

    // Parse target ID
    let target_id = resolve_entity_id(&project, &args.target)?;

    // Parse link type
    let link_type = args.link_type.to_lowercase();

    // Read the current file content
    let content = fs::read_to_string(&source_path).into_diagnostic()?;

    // Remove the link from the appropriate array
    let updated_content = remove_link_from_yaml(&content, &link_type, &target_id.to_string())?;

    // Write back
    fs::write(&source_path, &updated_content).into_diagnostic()?;

    println!(
        "{} Removed link: {} --[{}]--> {}",
        style("✓").green(),
        format_short_id(&source_req.id),
        style(&link_type).cyan(),
        format_short_id(&target_id)
    );

    Ok(())
}

fn run_show(args: ShowLinksArgs) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;

    // Find entity
    let (req, _path) = find_requirement(&project, &args.id)?;

    println!("{}", style("─".repeat(60)).dim());
    println!(
        "Links for {} - {}",
        style(&req.id.to_string()).cyan(),
        style(&req.title).yellow()
    );
    println!("{}", style("─".repeat(60)).dim());

    if !args.incoming {
        // Show outgoing links
        println!();
        println!("{}", style("Outgoing Links:").bold());

        let satisfied_by = &req.links.satisfied_by;
        let verified_by = &req.links.verified_by;

        if satisfied_by.is_empty() && verified_by.is_empty() {
            println!("  {}", style("(none)").dim());
        } else {
            if !satisfied_by.is_empty() {
                println!("  {}:", style("satisfied_by").cyan());
                for id in satisfied_by {
                    println!("    → {}", format_short_id(id));
                }
            }
            if !verified_by.is_empty() {
                println!("  {}:", style("verified_by").cyan());
                for id in verified_by {
                    println!("    → {}", format_short_id(id));
                }
            }
        }
    }

    if !args.outgoing {
        // Show incoming links (requires scanning all entities)
        println!();
        println!("{}", style("Incoming Links:").bold());

        let incoming = find_incoming_links(&project, &req.id)?;
        if incoming.is_empty() {
            println!("  {}", style("(none)").dim());
        } else {
            for (source_id, link_type) in incoming {
                println!("  {} ← {} ({})", format_short_id(&req.id), format_short_id(&source_id), link_type);
            }
        }
    }

    Ok(())
}

fn run_check(args: CheckLinksArgs) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;

    println!(
        "{} Checking links...\n",
        style("→").blue()
    );

    let mut broken_count = 0;
    let mut checked_count = 0;

    // Collect all entity IDs first
    let all_ids = collect_all_entity_ids(&project)?;

    // Check all requirements in inputs directory
    let inputs_dir = project.root().join("requirements/inputs");
    if inputs_dir.exists() {
        for entry in walkdir::WalkDir::new(&inputs_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().to_string_lossy().ends_with(".pdt.yaml"))
        {
            if let Ok(req) = crate::yaml::parse_yaml_file::<Requirement>(entry.path()) {
                // Check satisfied_by links
                for target_id in &req.links.satisfied_by {
                    checked_count += 1;
                    let target_str = target_id.to_string();
                    if !entity_exists(&all_ids, &target_str) {
                        broken_count += 1;
                        println!(
                            "{} {} → {} (satisfied_by) - {}",
                            style("✗").red(),
                            format_short_id(&req.id),
                            format_short_id(target_id),
                            style("target not found").red()
                        );

                        if args.fix {
                            println!("  {} Would remove broken link", style("fix:").yellow());
                        }
                    }
                }

                // Check verified_by links
                for target_id in &req.links.verified_by {
                    checked_count += 1;
                    let target_str = target_id.to_string();
                    if !entity_exists(&all_ids, &target_str) {
                        broken_count += 1;
                        println!(
                            "{} {} → {} (verified_by) - {}",
                            style("✗").red(),
                            format_short_id(&req.id),
                            format_short_id(target_id),
                            style("target not found").red()
                        );

                        if args.fix {
                            println!("  {} Would remove broken link", style("fix:").yellow());
                        }
                    }
                }
            }
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
                for target_id in &req.links.satisfied_by {
                    checked_count += 1;
                    let target_str = target_id.to_string();
                    if !entity_exists(&all_ids, &target_str) {
                        broken_count += 1;
                        println!(
                            "{} {} → {} (satisfied_by) - {}",
                            style("✗").red(),
                            format_short_id(&req.id),
                            format_short_id(target_id),
                            style("target not found").red()
                        );
                    }
                }

                for target_id in &req.links.verified_by {
                    checked_count += 1;
                    let target_str = target_id.to_string();
                    if !entity_exists(&all_ids, &target_str) {
                        broken_count += 1;
                        println!(
                            "{} {} → {} (verified_by) - {}",
                            style("✗").red(),
                            format_short_id(&req.id),
                            format_short_id(target_id),
                            style("target not found").red()
                        );
                    }
                }
            }
        }
    }

    println!();
    println!("{}", style("─".repeat(60)).dim());
    println!(
        "Checked {} link(s), found {} broken",
        style(checked_count).cyan(),
        if broken_count > 0 {
            style(broken_count).red()
        } else {
            style(broken_count).green()
        }
    );

    if broken_count > 0 {
        Err(miette::miette!(
            "{} broken link(s) found",
            broken_count
        ))
    } else {
        println!(
            "{} All links are valid!",
            style("✓").green().bold()
        );
        Ok(())
    }
}

/// Find a requirement by ID prefix match
fn find_requirement(project: &Project, id_query: &str) -> Result<(Requirement, std::path::PathBuf)> {
    let mut matches: Vec<(Requirement, std::path::PathBuf)> = Vec::new();

    // Search both inputs and outputs
    for subdir in &["inputs", "outputs"] {
        let dir = project.root().join(format!("requirements/{}", subdir));
        if !dir.exists() {
            continue;
        }

        for entry in walkdir::WalkDir::new(&dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().to_string_lossy().ends_with(".pdt.yaml"))
        {
            if let Ok(req) = crate::yaml::parse_yaml_file::<Requirement>(entry.path()) {
                let id_str = req.id.to_string();
                if id_str.starts_with(id_query) || id_str == id_query {
                    matches.push((req, entry.path().to_path_buf()));
                } else if req.title.to_lowercase().contains(&id_query.to_lowercase()) {
                    matches.push((req, entry.path().to_path_buf()));
                }
            }
        }
    }

    match matches.len() {
        0 => Err(miette::miette!(
            "No entity found matching '{}'",
            id_query
        )),
        1 => Ok(matches.remove(0)),
        _ => {
            println!(
                "{} Multiple matches found:",
                style("!").yellow()
            );
            for (req, _path) in &matches {
                println!(
                    "  {} - {}",
                    format_short_id(&req.id),
                    req.title
                );
            }
            Err(miette::miette!(
                "Ambiguous query '{}'. Please be more specific.",
                id_query
            ))
        }
    }
}

/// Resolve an entity ID from a query string
fn resolve_entity_id(project: &Project, query: &str) -> Result<EntityId> {
    // First try to parse as a full ID
    if let Ok(id) = query.parse::<EntityId>() {
        return Ok(id);
    }

    // Try to find by prefix
    let (req, _) = find_requirement(project, query)?;
    Ok(req.id)
}

/// Add a link to a YAML file
fn add_link_to_yaml(content: &str, link_type: &str, target_id: &str) -> Result<String> {
    // Parse YAML
    let mut value: serde_yml::Value = serde_yml::from_str(content).into_diagnostic()?;

    // Navigate to links section
    let links = value
        .get_mut("links")
        .ok_or_else(|| miette::miette!("No 'links' section found in file"))?;

    let link_array = links
        .get_mut(link_type)
        .ok_or_else(|| miette::miette!("Unknown link type: {}", link_type))?;

    // Add the new ID if not already present
    if let Some(arr) = link_array.as_sequence_mut() {
        let new_value = serde_yml::Value::String(target_id.to_string());
        if !arr.contains(&new_value) {
            arr.push(new_value);
        }
    } else {
        return Err(miette::miette!(
            "Link type '{}' is not an array",
            link_type
        ));
    }

    // Serialize back
    serde_yml::to_string(&value).into_diagnostic()
}

/// Remove a link from a YAML file
fn remove_link_from_yaml(content: &str, link_type: &str, target_id: &str) -> Result<String> {
    // Parse YAML
    let mut value: serde_yml::Value = serde_yml::from_str(content).into_diagnostic()?;

    // Navigate to links section
    let links = value
        .get_mut("links")
        .ok_or_else(|| miette::miette!("No 'links' section found in file"))?;

    let link_array = links
        .get_mut(link_type)
        .ok_or_else(|| miette::miette!("Unknown link type: {}", link_type))?;

    // Remove the ID
    if let Some(arr) = link_array.as_sequence_mut() {
        let remove_value = serde_yml::Value::String(target_id.to_string());
        arr.retain(|v| v != &remove_value);
    }

    // Serialize back
    serde_yml::to_string(&value).into_diagnostic()
}

/// Find all incoming links to an entity
fn find_incoming_links(project: &Project, target_id: &EntityId) -> Result<Vec<(EntityId, String)>> {
    let mut incoming = Vec::new();

    // Scan all requirements
    for subdir in &["inputs", "outputs"] {
        let dir = project.root().join(format!("requirements/{}", subdir));
        if !dir.exists() {
            continue;
        }

        for entry in walkdir::WalkDir::new(&dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().to_string_lossy().ends_with(".pdt.yaml"))
        {
            if let Ok(req) = crate::yaml::parse_yaml_file::<Requirement>(entry.path()) {
                for link in &req.links.satisfied_by {
                    if link == target_id {
                        incoming.push((req.id.clone(), "satisfied_by".to_string()));
                    }
                }
                for link in &req.links.verified_by {
                    if link == target_id {
                        incoming.push((req.id.clone(), "verified_by".to_string()));
                    }
                }
            }
        }
    }

    Ok(incoming)
}

/// Collect all entity IDs in the project
fn collect_all_entity_ids(project: &Project) -> Result<Vec<String>> {
    let mut ids = Vec::new();

    // Scan all requirements
    for subdir in &["inputs", "outputs"] {
        let dir = project.root().join(format!("requirements/{}", subdir));
        if !dir.exists() {
            continue;
        }

        for entry in walkdir::WalkDir::new(&dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().to_string_lossy().ends_with(".pdt.yaml"))
        {
            if let Ok(req) = crate::yaml::parse_yaml_file::<Requirement>(entry.path()) {
                ids.push(req.id.to_string());
            }
        }
    }

    Ok(ids)
}

/// Check if an entity exists
fn entity_exists(all_ids: &[String], id: &str) -> bool {
    all_ids.iter().any(|existing| existing == id || existing.starts_with(id))
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
