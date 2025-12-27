//! `tdt link` command - Manage links between entities

use console::style;
use miette::{IntoDiagnostic, Result};
use std::fs;
use std::path::PathBuf;

use crate::cli::helpers::format_short_id;
use crate::core::cache::EntityCache;
use crate::core::identity::{EntityId, EntityPrefix};
use crate::core::project::Project;
use crate::core::shortid::ShortIdIndex;
use crate::core::suspect::{
    clear_link_suspect, get_suspect_links, mark_link_suspect, SuspectReason,
};

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

    /// Manage suspect links (links that need review due to changes)
    Suspect(SuspectCommands),
}

#[derive(clap::Args, Debug)]
pub struct SuspectCommands {
    #[command(subcommand)]
    pub command: SuspectSubcommand,
}

#[derive(clap::Subcommand, Debug)]
pub enum SuspectSubcommand {
    /// List all suspect links in the project
    List(SuspectListArgs),

    /// Review suspect links for a specific entity
    Review(SuspectReviewArgs),

    /// Clear suspect status for a link (after review)
    Clear(SuspectClearArgs),

    /// Manually mark a link as suspect
    Mark(SuspectMarkArgs),
}

#[derive(clap::Args, Debug)]
pub struct SuspectListArgs {
    /// Filter by source entity type
    #[arg(long, short = 't')]
    pub entity_type: Option<String>,

    /// Show count only
    #[arg(long)]
    pub count: bool,
}

#[derive(clap::Args, Debug)]
pub struct SuspectReviewArgs {
    /// Entity ID to review
    pub id: String,
}

#[derive(clap::Args, Debug)]
pub struct SuspectClearArgs {
    /// Source entity ID
    pub source: String,

    /// Target entity ID (optional - clear all if not specified)
    pub target: Option<String>,

    /// Link type to clear
    #[arg(long = "link-type", short = 't')]
    pub link_type: Option<String>,

    /// Mark as verified at this revision
    #[arg(long)]
    pub verified_revision: Option<u32>,
}

#[derive(clap::Args, Debug)]
pub struct SuspectMarkArgs {
    /// Source entity ID
    pub source: String,

    /// Target entity ID
    pub target: String,

    /// Link type
    #[arg(long = "link-type", short = 't')]
    pub link_type: Option<String>,

    /// Reason for marking as suspect
    #[arg(long, short = 'r', default_value = "manually_marked")]
    pub reason: String,
}

#[derive(clap::Args, Debug)]
#[command(after_help = "\
LINK TYPES:
  Requirements (REQ):
    satisfied_by    Entity that satisfies this requirement (→ requirements)
    verified_by     TEST or CTRL that verifies this requirement (→ verifies)
    derives_from    Parent REQ this derives from (→ derived_by)
    allocated_to    FEAT this requirement is allocated to (→ allocated_from)
    risks           RISKs associated with this requirement (→ requirement)

  Tests (TEST):
    verifies        REQ that this test verifies (→ verified_by)
    component       CMP being tested (→ tests)
    assembly        ASM being tested (→ tests)

  Results (RSLT):
    component       CMP that was tested (→ tests)
    assembly        ASM that was tested (→ tests)
    ncrs            NCRs created from failures (→ from_result)

  Risks (RISK):
    requirement     REQ this risk is associated with (→ risks)
    component       CMP primarily affected (single-value)
    assembly        ASM primarily affected (single-value)
    process         PROC associated with this risk (→ risks)
    affects         Additional entities affected (→ risks)
    mitigated_by    Design output that mitigates this risk
    verified_by     TEST that verifies mitigation
    controls        CTRL items that address this risk (→ risks)

  Components (CMP) / Assemblies (ASM):
    requirements    REQs this component satisfies (→ satisfied_by)
    processes       PROCs used to manufacture this (→ produces)
    tests           TESTs for this component (→ component)
    risks           RISKs affecting this component
    used_in         ASMs using this component
    replaces        CMP this replaces (→ replaced_by)
    replaced_by     CMP that replaces this (→ replaces)

  Processes (PROC):
    requirements    REQs this process implements
    produces        CMPs/ASMs produced (→ processes)
    risks           RISKs affecting this process

  Controls (CTRL):
    verifies        REQ that this control verifies (→ verified_by)
    component       CMP being controlled
    risks           RISKs this control mitigates

  Work Instructions (WORK):
    component       CMP this instruction is for
    assembly        ASM this instruction is for
    risks           RISKs addressed by following this

  NCRs (NCR):
    component       CMP affected
    supplier        SUP related to this NCR
    process         PROC related to this NCR
    from_result     RSLT that created this NCR (→ ncrs)
    capa            CAPA opened for this NCR (→ ncrs)

  CAPAs (CAPA):
    ncrs                 Source NCRs for this CAPA
    component            CMP this CAPA addresses
    supplier             SUP this CAPA is for
    risks                RISKs addressed by this CAPA
    processes_modified   PROC modified by this CAPA (→ modified_by_capa)
    controls_added       CTRL added by this CAPA (→ added_by_capa)

  General (all entities):
    related_to           Symmetric link to any related entity

  Reciprocal links are added by default. Use --no-reciprocal to skip.
  Single-value links (component, assembly, requirement, etc.) replace existing values.

EXAMPLES:
  tdt link add REQ@1 TEST@1                   # Auto-infers 'verified_by' (both directions)
  tdt link add TEST@1 CMP@1                   # Links test to component under test
  tdt link add CMP@1 REQ@1                    # Links component to requirement it satisfies
  tdt link add RISK@1 CMP@1                   # Links risk to affected component
  tdt link add NCR@1 CMP@1                    # Links NCR to affected component
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
        LinkCommands::Suspect(args) => run_suspect(args),
    }
}

fn run_suspect(args: SuspectCommands) -> Result<()> {
    match args.command {
        SuspectSubcommand::List(args) => run_suspect_list(args),
        SuspectSubcommand::Review(args) => run_suspect_review(args),
        SuspectSubcommand::Clear(args) => run_suspect_clear(args),
        SuspectSubcommand::Mark(args) => run_suspect_mark(args),
    }
}

fn run_suspect_list(args: SuspectListArgs) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;

    println!(
        "{} Scanning for suspect links...\n",
        style("→").blue()
    );

    let mut total_suspect = 0;
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
            // Apply type filter if specified
            if let Some(ref type_filter) = args.entity_type {
                let filename = entry.file_name().to_string_lossy();
                let prefix = type_filter.to_uppercase();
                if !filename.starts_with(&prefix) {
                    continue;
                }
            }

            if let Ok(suspect_links) = get_suspect_links(entry.path()) {
                if !suspect_links.is_empty() {
                    // Get source entity ID
                    let source_id = fs::read_to_string(entry.path())
                        .ok()
                        .and_then(|content| {
                            serde_yml::from_str::<serde_yml::Value>(&content)
                                .ok()
                                .and_then(|v| v.get("id").and_then(|id| id.as_str()).map(|s| s.to_string()))
                        })
                        .unwrap_or_else(|| "unknown".to_string());

                    if !args.count {
                        for (link_type, target_id, reason) in &suspect_links {
                            println!(
                                "  {} {} --[{}]--> {} ({})",
                                style("!").yellow(),
                                truncate_id(&source_id),
                                style(link_type).cyan(),
                                truncate_id(target_id),
                                style(reason.to_string()).dim()
                            );
                        }
                    }
                    total_suspect += suspect_links.len();
                }
            }
        }
    }

    println!();
    if total_suspect == 0 {
        println!("{} No suspect links found.", style("✓").green());
    } else {
        println!(
            "{} {} suspect link(s) found. Run 'tdt link suspect review <ID>' to review.",
            style("!").yellow(),
            total_suspect
        );
    }

    Ok(())
}

fn run_suspect_review(args: SuspectReviewArgs) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let entity = find_entity(&project, &args.id)?;

    println!("{}", style("─".repeat(60)).dim());
    println!(
        "Suspect links for {} - {}",
        style(&entity.id.to_string()).cyan(),
        style(&entity.title).yellow()
    );
    println!("{}", style("─".repeat(60)).dim());

    let suspect_links = get_suspect_links(&entity.path).into_diagnostic()?;

    if suspect_links.is_empty() {
        println!("\n{} No suspect links found.", style("✓").green());
    } else {
        println!();
        for (link_type, target_id, reason) in &suspect_links {
            println!(
                "  {} {} → {} ({})",
                style("!").yellow(),
                style(link_type).cyan(),
                truncate_id(target_id),
                style(reason.to_string()).dim()
            );
        }
        println!();
        println!(
            "Run 'tdt link suspect clear {} --to <TARGET>' to clear after review.",
            args.id
        );
    }

    Ok(())
}

fn run_suspect_clear(args: SuspectClearArgs) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let source = find_entity(&project, &args.source)?;

    if let Some(ref target_id) = args.target {
        // Clear specific link
        let link_type = args.link_type.ok_or_else(|| {
            miette::miette!("Link type required when clearing specific target. Use -t <link_type>")
        })?;

        clear_link_suspect(&source.path, &link_type, target_id, args.verified_revision)
            .into_diagnostic()?;

        println!(
            "{} Cleared suspect status: {} --[{}]--> {}",
            style("✓").green(),
            format_short_id(&source.id),
            style(&link_type).cyan(),
            truncate_id(target_id)
        );
    } else {
        // Clear all suspect links for this entity
        let suspect_links = get_suspect_links(&source.path).into_diagnostic()?;
        let mut cleared = 0;

        for (link_type, target_id, _) in suspect_links {
            clear_link_suspect(&source.path, &link_type, &target_id, args.verified_revision)
                .into_diagnostic()?;
            cleared += 1;
        }

        println!(
            "{} Cleared {} suspect link(s) for {}",
            style("✓").green(),
            cleared,
            format_short_id(&source.id)
        );
    }

    Ok(())
}

fn run_suspect_mark(args: SuspectMarkArgs) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let source = find_entity(&project, &args.source)?;
    let target = find_entity(&project, &args.target)?;

    // Determine link type
    let link_type = match args.link_type {
        Some(lt) => lt,
        None => {
            // Try to auto-infer
            infer_link_type(source.id.prefix(), target.id.prefix())
                .ok_or_else(|| miette::miette!("Cannot infer link type. Use -t <link_type>"))?
        }
    };

    let reason = match args.reason.as_str() {
        "revision_changed" => SuspectReason::RevisionChanged,
        "status_regressed" => SuspectReason::StatusRegressed,
        "content_modified" => SuspectReason::ContentModified,
        _ => SuspectReason::ManuallyMarked,
    };

    mark_link_suspect(&source.path, &link_type, &target.id.to_string(), reason).into_diagnostic()?;

    println!(
        "{} Marked as suspect: {} --[{}]--> {}",
        style("✓").green(),
        format_short_id(&source.id),
        style(&link_type).cyan(),
        format_short_id(&target.id)
    );

    Ok(())
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

    // Update cache to reflect new links
    if let Ok(mut cache) = EntityCache::open(&project) {
        if let Err(e) = cache.sync() {
            eprintln!(
                "{} Warning: Failed to update cache: {}",
                console::style("!").yellow(),
                e
            );
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

    // Update cache to reflect removed links
    if let Ok(mut cache) = EntityCache::open(&project) {
        if let Err(e) = cache.sync() {
            eprintln!(
                "{} Warning: Failed to update cache: {}",
                console::style("!").yellow(),
                e
            );
        }
    }

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
    use crate::core::cache::EntityCache;

    // Try cache-based lookup first (O(1) via SQLite)
    if let Ok(cache) = EntityCache::open(project) {
        // Resolve short ID if needed
        let full_id = if id_query.contains('@') {
            cache.resolve_short_id(id_query)
        } else {
            None
        };

        let lookup_id = full_id.as_deref().unwrap_or(id_query);

        // Try exact match via cache
        if let Some(entity) = cache.get_entity(lookup_id) {
            if let Ok(id) = entity.id.parse::<EntityId>() {
                return Ok(EntityInfo {
                    id,
                    title: entity.title,
                    path: entity.file_path,
                });
            }
        }
    }

    // Fallback: filesystem search
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

    // Navigate to links section, creating it if it doesn't exist
    if value.get("links").is_none() {
        value["links"] = serde_yml::Value::Mapping(serde_yml::Mapping::new());
    }

    let links = value
        .get_mut("links")
        .ok_or_else(|| miette::miette!("No 'links' section found in file"))?;

    // Check if link type exists; if not, create it
    let link_value = if let Some(existing) = links.get_mut(link_type) {
        existing
    } else {
        // Determine if this should be an array or single-value link
        let is_array_link = is_array_link_type(link_type);
        let links_map = links
            .as_mapping_mut()
            .ok_or_else(|| miette::miette!("Links section is not a mapping"))?;

        if is_array_link {
            links_map.insert(
                serde_yml::Value::String(link_type.to_string()),
                serde_yml::Value::Sequence(vec![]),
            );
        } else {
            links_map.insert(
                serde_yml::Value::String(link_type.to_string()),
                serde_yml::Value::Null,
            );
        }
        links
            .get_mut(link_type)
            .ok_or_else(|| miette::miette!("Failed to create link type"))?
    };

    // Handle both array links and single-value links
    if let Some(arr) = link_value.as_sequence_mut() {
        // Array link - add to array if not already present
        let new_value = serde_yml::Value::String(target_id.to_string());
        if !arr.contains(&new_value) {
            arr.push(new_value);
        }
    } else if link_value.is_null() || link_value.as_str().is_some() {
        // Single-value link (null or existing string) - replace with new value
        *link_value = serde_yml::Value::String(target_id.to_string());
    } else {
        return Err(miette::miette!(
            "Link type '{}' has unexpected format (not array or single value)",
            link_type
        ));
    }

    // Serialize back
    serde_yml::to_string(&value).into_diagnostic()
}

/// Determine if a link type should be an array (multiple values) or single-value
fn is_array_link_type(link_type: &str) -> bool {
    match link_type {
        // Single-value links (can only have one target)
        "component" | "assembly" | "requirement" | "process" | "parent" | "supplier" | "capa"
        | "from_result" | "control" | "feature" | "test" => false,
        // Everything else is an array (can have multiple targets)
        _ => true,
    }
}

/// Remove a link from a YAML file
fn remove_link_from_yaml(content: &str, link_type: &str, target_id: &str) -> Result<String> {
    // Parse YAML
    let mut value: serde_yml::Value = serde_yml::from_str(content).into_diagnostic()?;

    // Navigate to links section
    let links = value
        .get_mut("links")
        .ok_or_else(|| miette::miette!("No 'links' section found in file"))?;

    let link_value = links
        .get_mut(link_type)
        .ok_or_else(|| miette::miette!("Unknown link type: {}", link_type))?;

    // Handle both array links and single-value links
    if let Some(arr) = link_value.as_sequence_mut() {
        // Array link - remove from array
        let remove_value = serde_yml::Value::String(target_id.to_string());
        arr.retain(|v| v != &remove_value);
    } else if let Some(current) = link_value.as_str() {
        // Single-value link - clear if it matches
        if current == target_id {
            *link_value = serde_yml::Value::Null;
        }
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
// Link inference functions moved to shared module
use crate::core::links::{get_reciprocal_link_type, infer_link_type};

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
        EntityPrefix::Cmp => vec![project.root().join("bom/components")],
        EntityPrefix::Asm => vec![project.root().join("bom/assemblies")],
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
        EntityPrefix::Lot => vec![project.root().join("manufacturing/lots")],
        EntityPrefix::Dev => vec![project.root().join("manufacturing/deviations")],
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
