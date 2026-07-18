//! `tdt where-used` command - Find where an entity is referenced

use console::style;
use miette::Result;
use std::collections::HashMap;

use crate::cli::helpers::format_short_id_str;
use crate::cli::{GlobalOpts, OutputFormat};
use tdt_core::core::cache::EntityCache;
use tdt_core::core::project::Project;
use tdt_core::core::shortid::ShortIdIndex;

#[derive(clap::Args, Debug)]
pub struct WhereUsedArgs {
    /// Entity ID or short ID to search for (e.g., CMP@1, FEAT@3, REQ@5)
    pub id: String,

    /// Show only direct references (not transitive)
    #[arg(long)]
    pub direct_only: bool,
}

pub fn run(args: WhereUsedArgs, global: &GlobalOpts) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let cache = EntityCache::open(&project).map_err(|e| miette::miette!("{}", e))?;
    let short_ids = ShortIdIndex::load(&project);

    // Resolve the ID
    let resolved_id = short_ids
        .resolve(&args.id)
        .unwrap_or_else(|| args.id.clone());

    // Determine entity type from prefix
    let entity_type = get_entity_type(&resolved_id);

    println!(
        "{} {}",
        style("Searching for references to:").bold(),
        style(&resolved_id).cyan()
    );
    println!("{}: {}\n", style("Entity type").dim(), entity_type);

    let mut found_refs: Vec<(String, String, String)> = Vec::new(); // (ref_id, ref_type, relationship)

    // Use cache to find all link-based references
    // This includes:
    // - Standard entity links (verifies, mitigates, etc.)
    // - BOM containment links
    // - Mate feature references (feature_a, feature_b)
    // - Stackup contributor feature references (contributor[N])
    find_link_references(&cache, &resolved_id, &mut found_refs);

    // Search for supplier/component usage in quotes
    if resolved_id.starts_with("SUP-") || resolved_id.starts_with("CMP-") {
        find_quote_references(&cache, &resolved_id, &mut found_refs);
    }

    // Output results
    if found_refs.is_empty() {
        println!("{}", style("No references found.").yellow());
    } else {
        let format = global.list_format();

        match format {
            OutputFormat::Json => {
                let refs: Vec<HashMap<&str, &str>> = found_refs
                    .iter()
                    .map(|(id, typ, rel)| {
                        let mut map = HashMap::new();
                        map.insert("id", id.as_str());
                        map.insert("type", typ.as_str());
                        map.insert("relationship", rel.as_str());
                        map
                    })
                    .collect();
                println!(
                    "{}",
                    serde_json::to_string_pretty(&refs).unwrap_or_default()
                );
            }
            OutputFormat::Csv => {
                println!("ref_id,ref_type,relationship");
                for (ref_id, ref_type, rel) in &found_refs {
                    println!("{},{},{}", ref_id, ref_type, rel);
                }
            }
            _ => {
                println!(
                    "{:<12} {:<20} {}",
                    style("REF ID").bold(),
                    style("TYPE").bold(),
                    style("RELATIONSHIP").bold()
                );
                println!("{}", "-".repeat(60));
                for (ref_id, ref_type, rel) in &found_refs {
                    let ref_short = short_ids
                        .get_short_id(ref_id)
                        .unwrap_or_else(|| format_short_id_str(ref_id));
                    println!("{:<12} {:<20} {}", style(&ref_short).cyan(), ref_type, rel);
                }
                println!();
                println!("{} reference(s) found.", style(found_refs.len()).cyan());
            }
        }
    }

    Ok(())
}

/// Get entity type from ID prefix
fn get_entity_type(id: &str) -> &'static str {
    if id.starts_with("CMP-") {
        "component"
    } else if id.starts_with("ASM-") {
        "assembly"
    } else if id.starts_with("FEAT-") {
        "feature"
    } else if id.starts_with("REQ-") {
        "requirement"
    } else if id.starts_with("TEST-") {
        "test"
    } else if id.starts_with("RISK-") {
        "risk"
    } else if id.starts_with("PROC-") {
        "process"
    } else if id.starts_with("SUP-") {
        "supplier"
    } else if id.starts_with("QUOTE-") {
        "quote"
    } else if id.starts_with("NCR-") {
        "ncr"
    } else if id.starts_with("CAPA-") {
        "capa"
    } else if id.starts_with("RSLT-") {
        "result"
    } else if id.starts_with("TOL-") {
        "stackup"
    } else if id.starts_with("MATE-") {
        "mate"
    } else {
        "unknown"
    }
}

/// Get entity type from ID prefix for display
fn get_entity_type_from_id(id: &str) -> &'static str {
    get_entity_type(id)
}

/// Find all link-based references using the cache
fn find_link_references(
    cache: &EntityCache,
    target_id: &str,
    found_refs: &mut Vec<(String, String, String)>,
) {
    // Get all entities that link TO this entity
    let links = cache.get_links_to(target_id);

    for link in links {
        // Avoid self-references
        if link.source_id == target_id {
            continue;
        }

        // Avoid duplicates
        if found_refs.iter().any(|(id, _, _)| id == &link.source_id) {
            continue;
        }

        let entity_type = get_entity_type_from_id(&link.source_id);
        found_refs.push((
            link.source_id.clone(),
            entity_type.to_string(),
            link.link_type.clone(),
        ));
    }
}

/// Find quote references using cache (for supplier and component lookups)
fn find_quote_references(
    cache: &EntityCache,
    target_id: &str,
    found_refs: &mut Vec<(String, String, String)>,
) {
    let quotes = cache.list_quotes(None, None, None, None, None, None, None);

    for quote in quotes {
        // Check if this quote references the target as supplier
        if quote.supplier_id.as_ref().is_some_and(|s| s == target_id)
            && !found_refs.iter().any(|(id, _, _)| id == &quote.id)
        {
            found_refs.push((
                quote.id.clone(),
                "quote".to_string(),
                "supplier".to_string(),
            ));
        }

        // Check if this quote references the target as component
        if quote.component_id.as_ref().is_some_and(|c| c == target_id)
            && !found_refs.iter().any(|(id, _, _)| id == &quote.id)
        {
            found_refs.push((
                quote.id.clone(),
                "quote".to_string(),
                "component".to_string(),
            ));
        }
    }
}
