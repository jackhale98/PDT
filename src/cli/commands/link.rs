//! `tdt link` command - Manage links between entities

use console::style;
use miette::{IntoDiagnostic, Result};
use std::fs;
use std::path::PathBuf;

use crate::cli::helpers::format_short_id;
use crate::core::identity::{EntityId, EntityPrefix};
use crate::core::project::Project;
use crate::core::shortid::ShortIdIndex;

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
    satisfied_by    REQ that satisfies this requirement (symmetric)
    verified_by     TEST or CTRL that verifies this requirement (→ verifies)
    derives_from    Parent REQ this derives from (→ derived_by)
    allocated_to    FEAT this requirement is allocated to (→ allocated_from)

  Tests (TEST) / Controls (CTRL):
    verifies        REQ that this test/control verifies (→ verified_by)

  Risks (RISK):
    affects         Any entity affected by this risk: FEAT, CMP, ASM, PROC (→ risks)
    mitigated_by    Design output that mitigates this risk
    verified_by     TEST that verifies mitigation
    related_to      Any related entity (symmetric)

  Results (RSLT):
    created_ncr     NCR created from this result (→ from_result)

  NCRs (NCR):
    from_result     RSLT that created this NCR (→ created_ncr)

  CAPAs (CAPA):
    processes_modified   PROC modified by this CAPA (→ modified_by_capa)
    controls_added       CTRL added by this CAPA (→ added_by_capa)
    ncrs                 Source NCRs for this CAPA

  Components (CMP):
    replaces             CMP this replaces (→ replaced_by)
    replaced_by          CMP that replaces this (→ replaces)
    interchangeable_with CMP that is interchangeable (symmetric)
    risks                RISKs affecting this component (→ affects)

  Processes (PROC):
    risks                RISKs affecting this process (→ affects)
    modified_by_capa     CAPA that modified this process (→ processes_modified)

  Features (FEAT):
    allocated_from       REQs allocated to this feature (→ allocated_to)
    risks                RISKs affecting this feature (→ affects)

  General (all entities):
    related_to           Symmetric link to any related entity

  Reciprocal links are added by default. Use --no-reciprocal to skip.

EXAMPLES:
  tdt link add REQ@1 TEST@1                   # Auto-infers 'verified_by' (both directions)
  tdt link add RISK@1 CMP@1                   # Auto-infers 'affects' (both directions)
  tdt link add TEST@1 REQ@1                   # Auto-infers 'verifies' (both directions)
  tdt link add REQ@1 TEST@1 verified_by       # Explicit link type
  tdt link add REQ@1 REQ@2 derives_from       # Requirement decomposition
  tdt link add CAPA@1 PROC@1 --no-reciprocal  # One-way only
")]
pub struct AddLinkArgs {
    /// Source entity ID (or partial ID)
    pub source: String,

    /// Target entity ID (or partial ID)
    pub target: String,

    /// Link type (optional - auto-inferred if not specified)
    ///
    /// If omitted, TDT will infer the most appropriate link type based on
    /// the source and target entity types. For example:
    ///   REQ → TEST  infers  verified_by
    ///   RISK → CMP  infers  affects
    ///   TEST → REQ  infers  verifies
    #[arg(value_name = "LINK_TYPE")]
    pub link_type_pos: Option<String>,

    /// Link type (alternative to positional arg, also optional)
    #[arg(long = "link-type", short = 't')]
    pub link_type_flag: Option<String>,

    /// Add reciprocal link (target -> source) - enabled by default
    #[arg(long, short = 'r', default_value = "true", action = clap::ArgAction::Set)]
    pub reciprocal: bool,

    /// Skip adding reciprocal link
    #[arg(long)]
    pub no_reciprocal: bool,
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

    // Find source entity (works with any entity type)
    let source = find_entity(&project, &args.source)?;

    // Validate target exists
    let target = find_entity(&project, &args.target)?;

    // Determine link type from positional arg, -t flag, or auto-infer
    let (link_type_input, was_inferred) = match args.link_type_pos.or(args.link_type_flag) {
        Some(lt) => (lt, false),
        None => {
            // Auto-infer link type based on source and target entity types
            match infer_link_type(source.id.prefix(), target.id.prefix()) {
                Some(inferred) => (inferred, true),
                None => {
                    return Err(miette::miette!(
                        "Cannot infer link type for {} → {}. Please specify explicitly:\n  tdt link add {} {} <link_type>\n\nUse 'tdt link add --help' for available link types.",
                        source.id.prefix(),
                        target.id.prefix(),
                        args.source,
                        args.target
                    ));
                }
            }
        }
    };

    // Parse link type
    let link_type = link_type_input.to_lowercase();

    // Read the current file content
    let content = fs::read_to_string(&source.path).into_diagnostic()?;

    // Add the link to the appropriate array
    let updated_content = add_link_to_yaml(&content, &link_type, &target.id.to_string())?;

    // Write back
    fs::write(&source.path, &updated_content).into_diagnostic()?;

    if was_inferred {
        println!(
            "{} Added link: {} --[{}]--> {} {}",
            style("✓").green(),
            format_short_id(&source.id),
            style(&link_type).cyan(),
            format_short_id(&target.id),
            style("(auto-inferred)").dim()
        );
    } else {
        println!(
            "{} Added link: {} --[{}]--> {}",
            style("✓").green(),
            format_short_id(&source.id),
            style(&link_type).cyan(),
            format_short_id(&target.id)
        );
    }

    if args.reciprocal && !args.no_reciprocal {
        // Determine reciprocal link type and add it
        match add_reciprocal_link(&project, &source.id, &target.id, &link_type) {
            Ok(Some(recip_type)) => {
                println!(
                    "{} Added reciprocal link: {} --[{}]--> {}",
                    style("✓").green(),
                    format_short_id(&target.id),
                    style(&recip_type).cyan(),
                    format_short_id(&source.id)
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

    // Find source entity (works with any entity type)
    let source = find_entity(&project, &args.source)?;

    // Find target entity
    let target = find_entity(&project, &args.target)?;

    // Parse link type
    let link_type = link_type_input.to_lowercase();

    // Read the current file content
    let content = fs::read_to_string(&source.path).into_diagnostic()?;

    // Remove the link from the appropriate array
    let updated_content = remove_link_from_yaml(&content, &link_type, &target.id.to_string())?;

    // Write back
    fs::write(&source.path, &updated_content).into_diagnostic()?;

    println!(
        "{} Removed link: {} --[{}]--> {}",
        style("✓").green(),
        format_short_id(&source.id),
        style(&link_type).cyan(),
        format_short_id(&target.id)
    );

    Ok(())
}

fn run_show(args: ShowLinksArgs) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;

    // Find entity (works with any type)
    let entity = find_entity(&project, &args.id)?;

    println!("{}", style("─".repeat(60)).dim());
    println!(
        "Links for {} - {}",
        style(&entity.id.to_string()).cyan(),
        style(&entity.title).yellow()
    );
    println!("{}", style("─".repeat(60)).dim());

    if !args.incoming {
        // Show outgoing links by reading YAML directly
        println!();
        println!("{}", style("Outgoing Links:").bold());

        let content = fs::read_to_string(&entity.path).into_diagnostic()?;
        let value: serde_yml::Value = serde_yml::from_str(&content).into_diagnostic()?;

        let mut found_links = false;
        if let Some(links) = value.get("links") {
            if let Some(links_map) = links.as_mapping() {
                for (key, val) in links_map {
                    if let Some(key_str) = key.as_str() {
                        // Handle array links
                        if let Some(arr) = val.as_sequence() {
                            if !arr.is_empty() {
                                found_links = true;
                                println!("  {}:", style(key_str).cyan());
                                for item in arr {
                                    if let Some(id_str) = item.as_str() {
                                        println!("    → {}", truncate_id(id_str));
                                    }
                                }
                            }
                        }
                        // Handle single-value links (Option<EntityId>)
                        else if let Some(id_str) = val.as_str() {
                            found_links = true;
                            println!("  {}:", style(key_str).cyan());
                            println!("    → {}", truncate_id(id_str));
                        }
                    }
                }
            }
        }

        if !found_links {
            println!("  {}", style("(none)").dim());
        }
    }

    if !args.outgoing {
        // Show incoming links (requires scanning all entities)
        println!();
        println!("{}", style("Incoming Links:").bold());

        let incoming = find_incoming_links(&project, &entity.id)?;
        if incoming.is_empty() {
            println!("  {}", style("(none)").dim());
        } else {
            for (source_id, link_type) in incoming {
                println!(
                    "  {} ← {} ({})",
                    format_short_id(&entity.id),
                    format_short_id(&source_id),
                    link_type
                );
            }
        }
    }

    Ok(())
}

/// Truncate an ID string for display
fn truncate_id(id: &str) -> String {
    if id.len() > 16 {
        format!("{}...", &id[..13])
    } else {
        id.to_string()
    }
}

fn run_check(args: CheckLinksArgs) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;

    println!(
        "{} Checking links across all entity types...\n",
        style("→").blue()
    );

    let mut broken_count = 0;
    let mut checked_count = 0;

    // Collect all entity IDs first
    let all_ids = collect_all_entity_ids(&project)?;

    // Get all directories to scan
    let all_dirs = get_search_dirs_for_query(&project, "");

    for dir in all_dirs {
        if !dir.exists() {
            continue;
        }

        for entry in walkdir::WalkDir::new(&dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| {
                let path_str = e.path().to_string_lossy();
                path_str.ends_with(".tdt.yaml") || path_str.ends_with(".yaml")
            })
        {
            if let Ok(content) = fs::read_to_string(entry.path()) {
                if let Ok(value) = serde_yml::from_str::<serde_yml::Value>(&content) {
                    // Get source entity ID
                    let source_id = match value.get("id").and_then(|v| v.as_str()) {
                        Some(id_str) => id_str.to_string(),
                        None => continue,
                    };

                    // Check all links in the links section
                    if let Some(links) = value.get("links") {
                        if let Some(links_map) = links.as_mapping() {
                            for (key, val) in links_map {
                                if let Some(link_type) = key.as_str() {
                                    // Check array links
                                    if let Some(arr) = val.as_sequence() {
                                        for item in arr {
                                            if let Some(target_str) = item.as_str() {
                                                checked_count += 1;
                                                if !entity_exists(&all_ids, target_str) {
                                                    broken_count += 1;
                                                    println!(
                                                        "{} {} → {} ({}) - {}",
                                                        style("✗").red(),
                                                        truncate_id(&source_id),
                                                        truncate_id(target_str),
                                                        link_type,
                                                        style("target not found").red()
                                                    );

                                                    if args.fix {
                                                        println!(
                                                            "  {} Would remove broken link",
                                                            style("fix:").yellow()
                                                        );
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    // Check single-value links
                                    else if let Some(target_str) = val.as_str() {
                                        checked_count += 1;
                                        if !entity_exists(&all_ids, target_str) {
                                            broken_count += 1;
                                            println!(
                                                "{} {} → {} ({}) - {}",
                                                style("✗").red(),
                                                truncate_id(&source_id),
                                                truncate_id(target_str),
                                                link_type,
                                                style("target not found").red()
                                            );

                                            if args.fix {
                                                println!(
                                                    "  {} Would remove broken link",
                                                    style("fix:").yellow()
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        }
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
        Err(miette::miette!("{} broken link(s) found", broken_count))
    } else {
        println!("{} All links are valid!", style("✓").green().bold());
        Ok(())
    }
}

/// Generic entity info extracted from YAML
struct EntityInfo {
    id: EntityId,
    title: String,
    path: PathBuf,
}

/// Find any entity by ID prefix match or short ID
/// Works with all entity types (REQ, RISK, TEST, CMP, etc.)
fn find_entity(project: &Project, id_query: &str) -> Result<EntityInfo> {
    // Resolve short ID (e.g., REQ@1) to full ID
    let short_ids = ShortIdIndex::load(project);
    let resolved_query = short_ids
        .resolve(id_query)
        .unwrap_or_else(|| id_query.to_string());

    // Determine which directories to search based on prefix
    let search_dirs = get_search_dirs_for_query(project, &resolved_query);

    let mut matches: Vec<EntityInfo> = Vec::new();

    for dir in search_dirs {
        if !dir.exists() {
            continue;
        }

        for entry in walkdir::WalkDir::new(&dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| {
                let path_str = e.path().to_string_lossy();
                path_str.ends_with(".tdt.yaml") || path_str.ends_with(".yaml")
            })
        {
            // Parse as generic YAML to extract id and title
            if let Ok(content) = fs::read_to_string(entry.path()) {
                if let Ok(value) = serde_yml::from_str::<serde_yml::Value>(&content) {
                    if let Some(id_str) = value.get("id").and_then(|v| v.as_str()) {
                        if id_str.starts_with(&resolved_query) || id_str == resolved_query {
                            if let Ok(id) = id_str.parse::<EntityId>() {
                                let title = value
                                    .get("title")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("(untitled)")
                                    .to_string();
                                matches.push(EntityInfo {
                                    id,
                                    title,
                                    path: entry.path().to_path_buf(),
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    match matches.len() {
        0 => Err(miette::miette!("No entity found matching '{}'", id_query)),
        1 => Ok(matches.remove(0)),
        _ => {
            println!("{} Multiple matches found:", style("!").yellow());
            for info in &matches {
                println!("  {} - {}", format_short_id(&info.id), info.title);
            }
            Err(miette::miette!(
                "Ambiguous query '{}'. Please be more specific.",
                id_query
            ))
        }
    }
}

/// Get search directories based on query prefix
fn get_search_dirs_for_query(project: &Project, query: &str) -> Vec<PathBuf> {
    let root = project.root();

    // Try to determine prefix from query
    let prefix = query.split('-').next().unwrap_or("");

    match prefix.to_uppercase().as_str() {
        "REQ" => vec![
            root.join("requirements/inputs"),
            root.join("requirements/outputs"),
        ],
        "RISK" => vec![root.join("design/risks")],
        "TEST" => vec![
            root.join("verification/protocols"),
            root.join("validation/protocols"),
        ],
        "RSLT" => vec![
            root.join("verification/results"),
            root.join("validation/results"),
        ],
        "CMP" => vec![root.join("bom/components")],
        "ASM" => vec![root.join("bom/assemblies")],
        "FEAT" => vec![root.join("tolerances/features")],
        "MATE" => vec![root.join("tolerances/mates")],
        "TOL" => vec![root.join("tolerances/stackups")],
        "QUOT" => vec![root.join("sourcing/quotes")],
        "SUP" => vec![root.join("sourcing/suppliers")],
        "PROC" => vec![root.join("manufacturing/processes")],
        "CTRL" => vec![root.join("manufacturing/controls")],
        "WORK" => vec![root.join("manufacturing/work_instructions")],
        "NCR" => vec![root.join("manufacturing/ncrs")],
        "CAPA" => vec![root.join("manufacturing/capas")],
        "ACT" => vec![root.join("manufacturing/actions")],
        _ => {
            // Search all directories if prefix is unknown
            vec![
                root.join("requirements/inputs"),
                root.join("requirements/outputs"),
                root.join("design/risks"),
                root.join("verification/protocols"),
                root.join("validation/protocols"),
                root.join("bom/components"),
                root.join("bom/assemblies"),
                root.join("tolerances/features"),
                root.join("tolerances/mates"),
                root.join("tolerances/stackups"),
                root.join("sourcing/quotes"),
                root.join("sourcing/suppliers"),
                root.join("manufacturing/processes"),
                root.join("manufacturing/controls"),
                root.join("manufacturing/work_instructions"),
                root.join("manufacturing/ncrs"),
                root.join("manufacturing/capas"),
            ]
        }
    }
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
        return Err(miette::miette!("Link type '{}' is not an array", link_type));
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

/// Find all incoming links to an entity (scans all entity types)
fn find_incoming_links(project: &Project, target_id: &EntityId) -> Result<Vec<(EntityId, String)>> {
    let mut incoming = Vec::new();
    let target_str = target_id.to_string();

    // Get all directories to scan
    let all_dirs = get_search_dirs_for_query(project, "");

    for dir in all_dirs {
        if !dir.exists() {
            continue;
        }

        for entry in walkdir::WalkDir::new(&dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| {
                let path_str = e.path().to_string_lossy();
                path_str.ends_with(".tdt.yaml") || path_str.ends_with(".yaml")
            })
        {
            if let Ok(content) = fs::read_to_string(entry.path()) {
                if let Ok(value) = serde_yml::from_str::<serde_yml::Value>(&content) {
                    // Get source entity ID
                    let source_id = match value.get("id").and_then(|v| v.as_str()) {
                        Some(id_str) => match id_str.parse::<EntityId>() {
                            Ok(id) => id,
                            Err(_) => continue,
                        },
                        None => continue,
                    };

                    // Skip if this is the target entity itself
                    if source_id == *target_id {
                        continue;
                    }

                    // Check all link arrays
                    if let Some(links) = value.get("links") {
                        if let Some(links_map) = links.as_mapping() {
                            for (key, val) in links_map {
                                if let Some(link_type) = key.as_str() {
                                    // Check array links
                                    if let Some(arr) = val.as_sequence() {
                                        for item in arr {
                                            if let Some(link_str) = item.as_str() {
                                                if link_str == target_str {
                                                    incoming.push((
                                                        source_id.clone(),
                                                        link_type.to_string(),
                                                    ));
                                                }
                                            }
                                        }
                                    }
                                    // Check single-value links
                                    else if let Some(link_str) = val.as_str() {
                                        if link_str == target_str {
                                            incoming
                                                .push((source_id.clone(), link_type.to_string()));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(incoming)
}

/// Collect all entity IDs in the project (scans all entity types)
fn collect_all_entity_ids(project: &Project) -> Result<Vec<String>> {
    let mut ids = Vec::new();

    // Get all directories to scan
    let all_dirs = get_search_dirs_for_query(project, "");

    for dir in all_dirs {
        if !dir.exists() {
            continue;
        }

        for entry in walkdir::WalkDir::new(&dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| {
                let path_str = e.path().to_string_lossy();
                path_str.ends_with(".tdt.yaml") || path_str.ends_with(".yaml")
            })
        {
            if let Ok(content) = fs::read_to_string(entry.path()) {
                if let Ok(value) = serde_yml::from_str::<serde_yml::Value>(&content) {
                    if let Some(id_str) = value.get("id").and_then(|v| v.as_str()) {
                        ids.push(id_str.to_string());
                    }
                }
            }
        }
    }

    Ok(ids)
}

/// Check if an entity exists
fn entity_exists(all_ids: &[String], id: &str) -> bool {
    all_ids
        .iter()
        .any(|existing| existing == id || existing.starts_with(id))
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

/// Infer the most appropriate link type based on source and target entity types
///
/// This enables users to run `tdt link add REQ@1 TEST@1` without specifying the link type,
/// as the system will automatically determine that REQ → TEST should use "verified_by".
fn infer_link_type(source_prefix: EntityPrefix, target_prefix: EntityPrefix) -> Option<String> {
    match (source_prefix, target_prefix) {
        // Requirements linking to verification entities
        (EntityPrefix::Req, EntityPrefix::Test) => Some("verified_by".to_string()),
        (EntityPrefix::Req, EntityPrefix::Ctrl) => Some("verified_by".to_string()),

        // Requirements linking to other requirements (decomposition)
        (EntityPrefix::Req, EntityPrefix::Req) => Some("derives_from".to_string()),

        // Requirements allocated to features
        (EntityPrefix::Req, EntityPrefix::Feat) => Some("allocated_to".to_string()),

        // Requirements linking to risks
        (EntityPrefix::Req, EntityPrefix::Risk) => Some("related_to".to_string()),

        // Verification entities linking to requirements
        (EntityPrefix::Test, EntityPrefix::Req) => Some("verifies".to_string()),
        (EntityPrefix::Ctrl, EntityPrefix::Req) => Some("verifies".to_string()),

        // Risks affecting entities
        (EntityPrefix::Risk, EntityPrefix::Feat) => Some("affects".to_string()),
        (EntityPrefix::Risk, EntityPrefix::Cmp) => Some("affects".to_string()),
        (EntityPrefix::Risk, EntityPrefix::Asm) => Some("affects".to_string()),
        (EntityPrefix::Risk, EntityPrefix::Proc) => Some("affects".to_string()),
        (EntityPrefix::Risk, EntityPrefix::Test) => Some("verified_by".to_string()),
        (EntityPrefix::Risk, EntityPrefix::Req) => Some("related_to".to_string()),

        // Entities referencing risks
        (EntityPrefix::Feat, EntityPrefix::Risk) => Some("risks".to_string()),
        (EntityPrefix::Cmp, EntityPrefix::Risk) => Some("risks".to_string()),
        (EntityPrefix::Asm, EntityPrefix::Risk) => Some("risks".to_string()),
        (EntityPrefix::Proc, EntityPrefix::Risk) => Some("risks".to_string()),

        // Features to requirements (allocation back-link)
        (EntityPrefix::Feat, EntityPrefix::Req) => Some("allocated_from".to_string()),

        // Results and NCRs
        (EntityPrefix::Rslt, EntityPrefix::Ncr) => Some("created_ncr".to_string()),
        (EntityPrefix::Ncr, EntityPrefix::Rslt) => Some("from_result".to_string()),
        (EntityPrefix::Ncr, EntityPrefix::Capa) => Some("capa".to_string()),

        // CAPAs
        (EntityPrefix::Capa, EntityPrefix::Ncr) => Some("ncrs".to_string()),
        (EntityPrefix::Capa, EntityPrefix::Proc) => Some("processes_modified".to_string()),
        (EntityPrefix::Capa, EntityPrefix::Ctrl) => Some("controls_added".to_string()),

        // Process/Control back-links to CAPA
        (EntityPrefix::Proc, EntityPrefix::Capa) => Some("modified_by_capa".to_string()),
        (EntityPrefix::Ctrl, EntityPrefix::Capa) => Some("added_by_capa".to_string()),

        // Component supersession (default to replaces)
        (EntityPrefix::Cmp, EntityPrefix::Cmp) => Some("replaces".to_string()),

        // Default to related_to for same-type entities (except CMP)
        (a, b) if a == b => Some("related_to".to_string()),

        // No inference available for other combinations
        _ => None,
    }
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

        // RISK.affects -> target.risks (simplified from affects_*)
        ("affects", EntityPrefix::Feat) => Some("risks".to_string()),
        ("affects", EntityPrefix::Cmp) => Some("risks".to_string()),
        ("affects", EntityPrefix::Asm) => Some("risks".to_string()),
        ("affects", EntityPrefix::Proc) => Some("risks".to_string()),
        ("risks", EntityPrefix::Risk) => Some("affects".to_string()),

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
