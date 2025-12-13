//! `tdt link` command - Manage links between entities

use console::style;
use miette::{IntoDiagnostic, Result};
use std::fs;
use std::path::PathBuf;

use crate::cli::helpers::format_short_id;
use crate::core::identity::{EntityId, EntityPrefix};
use crate::core::project::Project;
use crate::core::shortid::ShortIdIndex;
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
#[command(after_help = "\
LINK TYPES:
  Requirements (REQ):
    satisfied_by    REQ that satisfies this requirement
    verified_by     TEST or CTRL that verifies this requirement
    derives_from    Parent REQ this derives from (→ derived_by)
    allocated_to    FEAT this requirement is allocated to (→ allocated_from)

  Tests (TEST) / Controls (CTRL):
    verifies        REQ that this test/control verifies (→ verified_by)

  Risks (RISK):
    affects_features     FEAT affected by this risk
    affects_components   CMP affected by this risk
    affects_assemblies   ASM affected by this risk
    affects_processes    PROC affected by this risk
    related_to           Any related entity

  Results (RSLT):
    created_ncr     NCR created from this result (→ from_result)

  CAPAs (CAPA):
    processes_modified   PROC modified by this CAPA
    controls_added       CTRL added by this CAPA

  Components (CMP):
    replaces             CMP this replaces (→ replaced_by)
    interchangeable_with CMP that is interchangeable

  General:
    related_to           Symmetric link to any related entity

EXAMPLES:
  tdt link add REQ@1 TEST@1 verified_by -r    # Link requirement to test
  tdt link add REQ@1 REQ@2 derives_from       # Requirement decomposition
  tdt link add RISK@1 CMP@1 affects_components
")]
pub struct AddLinkArgs {
    /// Source entity ID (or partial ID)
    pub source: String,

    /// Target entity ID (or partial ID)
    pub target: String,

    /// Link type (see LINK TYPES below for valid options)
    ///
    /// Example: tdt link add REQ@1 TEST@1 verified_by
    #[arg(value_name = "LINK_TYPE")]
    pub link_type_pos: Option<String>,

    /// Link type (alternative to positional arg)
    #[arg(long = "link-type", short = 't')]
    pub link_type_flag: Option<String>,

    /// Add reciprocal link (target -> source)
    #[arg(long, short = 'r')]
    pub reciprocal: bool,
}

#[derive(clap::Args, Debug)]
pub struct RemoveLinkArgs {
    /// Source entity ID (or partial ID)
    pub source: String,

    /// Target entity ID (or partial ID)
    pub target: String,

    /// Link type (positional or use -t flag): verified_by, mitigates, etc.
    #[arg(value_name = "LINK_TYPE")]
    pub link_type_pos: Option<String>,

    /// Link type (alternative to positional arg)
    #[arg(long = "link-type", short = 't')]
    pub link_type_flag: Option<String>,

    /// Remove reciprocal link too
    #[arg(long, short = 'r')]
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

    // Determine link type from positional arg or -t flag
    let link_type_input = args.link_type_pos
        .or(args.link_type_flag)
        .ok_or_else(|| miette::miette!(
            "Link type required. Usage:\n  tdt link add REQ@1 TEST@1 verified_by\n  tdt link add REQ@1 TEST@1 -t verified_by"
        ))?;

    // Find source entity
    let (source_req, source_path) = find_requirement(&project, &args.source)?;

    // Validate target exists (if it's a known prefix)
    let target_id = resolve_entity_id(&project, &args.target)?;

    // Parse link type
    let link_type = link_type_input.to_lowercase();

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
        // Determine reciprocal link type and add it
        match add_reciprocal_link(&project, &source_req.id, &target_id, &link_type) {
            Ok(Some(recip_type)) => {
                println!(
                    "{} Added reciprocal link: {} --[{}]--> {}",
                    style("✓").green(),
                    format_short_id(&target_id),
                    style(&recip_type).cyan(),
                    format_short_id(&source_req.id)
                );
            }
            Ok(None) => {
                println!(
                    "{} No reciprocal link type defined for '{}' on target entity",
                    style("!").yellow(),
                    link_type
                );
            }
            Err(e) => {
                println!(
                    "{} Failed to add reciprocal link: {}",
                    style("!").yellow(),
                    e
                );
            }
        }
    }

    Ok(())
}

fn run_remove(args: RemoveLinkArgs) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;

    // Determine link type from positional arg or -t flag
    let link_type_input = args.link_type_pos
        .or(args.link_type_flag)
        .ok_or_else(|| miette::miette!(
            "Link type required. Usage:\n  tdt link rm REQ@1 TEST@1 verified_by\n  tdt link rm REQ@1 TEST@1 -t verified_by"
        ))?;

    // Find source entity
    let (source_req, source_path) = find_requirement(&project, &args.source)?;

    // Parse target ID
    let target_id = resolve_entity_id(&project, &args.target)?;

    // Parse link type
    let link_type = link_type_input.to_lowercase();

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
            .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
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
            .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
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

/// Find a requirement by ID prefix match or short ID
fn find_requirement(project: &Project, id_query: &str) -> Result<(Requirement, std::path::PathBuf)> {
    // Resolve short ID (e.g., REQ@1) to full ID
    let short_ids = ShortIdIndex::load(project);
    let resolved_query = short_ids.resolve(id_query).unwrap_or_else(|| id_query.to_string());

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
            .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        {
            if let Ok(req) = crate::yaml::parse_yaml_file::<Requirement>(entry.path()) {
                let id_str = req.id.to_string();
                if id_str.starts_with(&resolved_query) || id_str == resolved_query {
                    matches.push((req, entry.path().to_path_buf()));
                } else if !id_query.contains('@') && req.title.to_lowercase().contains(&resolved_query.to_lowercase()) {
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

/// Resolve an entity ID from a query string (supports short IDs like REQ@1)
fn resolve_entity_id(project: &Project, query: &str) -> Result<EntityId> {
    // Resolve short ID (e.g., REQ@1, RISK@2) to full ID
    let short_ids = ShortIdIndex::load(project);
    let resolved_query = short_ids.resolve(query).unwrap_or_else(|| query.to_string());

    // Try to parse as a full ID
    if let Ok(id) = resolved_query.parse::<EntityId>() {
        return Ok(id);
    }

    // Try to find by prefix
    let (req, _) = find_requirement(project, &resolved_query)?;
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
            .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
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
            .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
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

/// Add a reciprocal link from target back to source
/// Returns Ok(Some(link_type)) if successful, Ok(None) if no reciprocal defined
fn add_reciprocal_link(
    project: &Project,
    source_id: &EntityId,
    target_id: &EntityId,
    link_type: &str,
) -> Result<Option<String>> {
    // Determine the reciprocal link type based on source link type and target entity type
    let target_prefix = target_id.prefix();

    let reciprocal_type = get_reciprocal_link_type(link_type, target_prefix);

    let recip_type = match reciprocal_type {
        Some(t) => t,
        None => return Ok(None),
    };

    // Find the target entity file
    let target_path = find_entity_file(project, target_id)?;

    // Read and update the target file
    let content = fs::read_to_string(&target_path).into_diagnostic()?;
    let updated_content = add_link_to_yaml(&content, &recip_type, &source_id.to_string())?;
    fs::write(&target_path, &updated_content).into_diagnostic()?;

    Ok(Some(recip_type))
}

/// Get the reciprocal link type for a given forward link type and target entity prefix
fn get_reciprocal_link_type(link_type: &str, target_prefix: EntityPrefix) -> Option<String> {
    match (link_type, target_prefix) {
        // REQ.verified_by -> TEST means TEST.verifies -> REQ
        ("verified_by", EntityPrefix::Test) => Some("verifies".to_string()),

        // REQ.verified_by -> CTRL means CTRL.verifies -> REQ
        ("verified_by", EntityPrefix::Ctrl) => Some("verifies".to_string()),

        // REQ.satisfied_by -> REQ means bidirectional satisfaction
        ("satisfied_by", EntityPrefix::Req) => Some("satisfied_by".to_string()),

        // TEST.verifies -> REQ or CTRL.verifies -> REQ means REQ.verified_by
        ("verifies", EntityPrefix::Req) => Some("verified_by".to_string()),

        // related_to is symmetric
        ("related_to", _) => Some("related_to".to_string()),

        // process/control/ncr/capa links
        ("process", EntityPrefix::Proc) => None, // Processes don't link back
        ("controls", EntityPrefix::Ctrl) => Some("process".to_string()),
        ("ncrs", EntityPrefix::Ncr) => None,
        ("capa", EntityPrefix::Capa) => Some("ncrs".to_string()),

        // Requirement decomposition: derives_from <-> derived_by
        ("derives_from", EntityPrefix::Req) => Some("derived_by".to_string()),
        ("derived_by", EntityPrefix::Req) => Some("derives_from".to_string()),

        // Requirement allocation: allocated_to <-> allocated_from
        ("allocated_to", EntityPrefix::Feat) => Some("allocated_from".to_string()),
        ("allocated_from", EntityPrefix::Req) => Some("allocated_to".to_string()),

        // Risk affects: affects_* -> affected_by_risk
        ("affects_features", EntityPrefix::Feat) => Some("affected_by_risk".to_string()),
        ("affects_components", EntityPrefix::Cmp) => Some("affected_by_risk".to_string()),
        ("affects_assemblies", EntityPrefix::Asm) => Some("affected_by_risk".to_string()),
        ("affects_processes", EntityPrefix::Proc) => Some("affected_by_risk".to_string()),
        ("affected_by_risk", EntityPrefix::Risk) => None, // Risk doesn't get reverse links auto-added

        // Result -> NCR: created_ncr <-> from_result
        ("created_ncr", EntityPrefix::Ncr) => Some("from_result".to_string()),
        ("from_result", EntityPrefix::Rslt) => Some("created_ncr".to_string()),

        // CAPA -> Process/Control modifications
        ("processes_modified", EntityPrefix::Proc) => Some("modified_by_capa".to_string()),
        ("modified_by_capa", EntityPrefix::Capa) => Some("processes_modified".to_string()),
        ("controls_added", EntityPrefix::Ctrl) => Some("added_by_capa".to_string()),
        ("added_by_capa", EntityPrefix::Capa) => Some("controls_added".to_string()),

        // Component supersession: replaces <-> replaced_by
        ("replaces", EntityPrefix::Cmp) => Some("replaced_by".to_string()),
        ("replaced_by", EntityPrefix::Cmp) => Some("replaces".to_string()),

        // Component interchangeability is symmetric
        ("interchangeable_with", EntityPrefix::Cmp) => Some("interchangeable_with".to_string()),

        // No reciprocal defined for other cases
        (_, _) => None,
    }
}

/// Find an entity file by its ID
fn find_entity_file(project: &Project, id: &EntityId) -> Result<PathBuf> {
    let prefix = id.prefix();
    let id_str = id.to_string();

    // Determine search directories based on entity prefix
    let search_dirs: Vec<PathBuf> = match prefix {
        EntityPrefix::Req => vec![
            project.root().join("requirements/inputs"),
            project.root().join("requirements/outputs"),
        ],
        EntityPrefix::Risk => vec![project.root().join("design/risks")],
        EntityPrefix::Test => vec![
            project.root().join("verification/protocols"),
            project.root().join("validation/protocols"),
        ],
        EntityPrefix::Rslt => vec![
            project.root().join("verification/results"),
            project.root().join("validation/results"),
        ],
        EntityPrefix::Cmp => vec![project.root().join("design/components")],
        EntityPrefix::Asm => vec![project.root().join("design/assemblies")],
        EntityPrefix::Feat => vec![project.root().join("tolerances/features")],
        EntityPrefix::Mate => vec![project.root().join("tolerances/mates")],
        EntityPrefix::Tol => vec![project.root().join("tolerances/stackups")],
        EntityPrefix::Quot => vec![project.root().join("sourcing/quotes")],
        EntityPrefix::Sup => vec![project.root().join("sourcing/suppliers")],
        EntityPrefix::Proc => vec![project.root().join("manufacturing/processes")],
        EntityPrefix::Ctrl => vec![project.root().join("manufacturing/controls")],
        EntityPrefix::Work => vec![project.root().join("manufacturing/work_instructions")],
        EntityPrefix::Ncr => vec![project.root().join("manufacturing/ncrs")],
        EntityPrefix::Capa => vec![project.root().join("manufacturing/capas")],
        EntityPrefix::Act => vec![project.root().join("manufacturing/actions")],
    };

    for dir in search_dirs {
        if !dir.exists() {
            continue;
        }

        for entry in walkdir::WalkDir::new(&dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().to_string_lossy().ends_with(".tdt.yaml"))
        {
            let filename = entry.file_name().to_string_lossy();
            if filename.contains(&id_str) || filename.starts_with(&id_str) {
                return Ok(entry.path().to_path_buf());
            }
        }
    }

    Err(miette::miette!("Entity file not found for {}", id_str))
}
