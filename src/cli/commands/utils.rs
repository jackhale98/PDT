//! Shared utilities for CLI commands

use console::style;
use miette::{IntoDiagnostic, Result};
use std::fs;
use std::path::PathBuf;

use crate::core::cache::EntityCache;
use crate::core::project::Project;
use crate::core::shortid::ShortIdIndex;

/// Save short ID index with warning on failure (instead of silent discard)
pub fn save_short_ids(short_ids: &mut ShortIdIndex, project: &Project) {
    if let Err(e) = short_ids.save(project) {
        eprintln!(
            "{} Warning: Failed to save short ID index: {}",
            style("!").yellow(),
            e
        );
    }
}

/// Format a linked entity ID with its title for display
/// Returns format like "CMP@1 (Housing)" or just "CMP@1" if title lookup fails
pub fn format_link_with_title(
    entity_id: &str,
    short_ids: &ShortIdIndex,
    cache: &Option<EntityCache>,
) -> String {
    // Get short ID display
    let display_id = short_ids
        .get_short_id(entity_id)
        .unwrap_or_else(|| entity_id.to_string());

    // Try to get title from cache
    if let Some(cache) = cache {
        // Resolve short ID to full ID if needed
        let full_id = short_ids
            .resolve(&display_id)
            .unwrap_or_else(|| entity_id.to_string());

        if let Some(entity) = cache.get_entity(&full_id) {
            return format!("{} ({})", display_id, entity.title);
        }
    }

    display_id
}

/// Format multiple linked entity IDs with titles
pub fn format_links_with_titles(
    entity_ids: &[String],
    short_ids: &ShortIdIndex,
    cache: &Option<EntityCache>,
) -> Vec<String> {
    entity_ids
        .iter()
        .map(|id| format_link_with_title(id, short_ids, cache))
        .collect()
}

// =========================================================================
// Delete and Archive Operations
// =========================================================================

/// Delete an entity file
///
/// This function:
/// 1. Checks if the entity has any incoming links (entities that reference it)
/// 2. If force is false and there are incoming links, warns and returns an error
/// 3. Otherwise, deletes the file
///
/// Returns the path of the deleted file on success.
pub fn delete_entity_file(
    project: &Project,
    entity_id: &str,
    entity_dirs: &[&str],
    force: bool,
) -> Result<PathBuf> {
    // Find the entity file
    let file_path = find_entity_file_in_dirs(project, entity_id, entity_dirs)?;

    // Check for incoming links
    if !force {
        let cache = EntityCache::open(project)?;
        let incoming = cache.get_links_to(entity_id);

        if !incoming.is_empty() {
            let linked_ids: Vec<String> = incoming.iter().map(|l| l.source_id.clone()).collect();
            return Err(miette::miette!(
                "Entity '{}' is referenced by {} other entities: {}\nUse --force to delete anyway, or remove the links first.",
                entity_id,
                incoming.len(),
                linked_ids.join(", ")
            ));
        }
    }

    // Delete the file
    fs::remove_file(&file_path).into_diagnostic()?;

    Ok(file_path)
}

/// Archive an entity file by moving it to .tdt/archive/
///
/// This function:
/// 1. Creates the archive directory if needed
/// 2. Moves the entity file to the archive directory
/// 3. Preserves the relative path structure within the archive
///
/// Returns the new archived file path on success.
pub fn archive_entity_file(
    project: &Project,
    entity_id: &str,
    entity_dirs: &[&str],
    force: bool,
) -> Result<PathBuf> {
    // Find the entity file
    let file_path = find_entity_file_in_dirs(project, entity_id, entity_dirs)?;

    // Check for incoming links
    if !force {
        let cache = EntityCache::open(project)?;
        let incoming = cache.get_links_to(entity_id);

        if !incoming.is_empty() {
            let linked_ids: Vec<String> = incoming.iter().map(|l| l.source_id.clone()).collect();
            return Err(miette::miette!(
                "Entity '{}' is referenced by {} other entities: {}\nUse --force to archive anyway, or remove the links first.",
                entity_id,
                incoming.len(),
                linked_ids.join(", ")
            ));
        }
    }

    // Compute archive path preserving directory structure
    let relative_path = file_path.strip_prefix(project.root()).unwrap_or(&file_path);
    let archive_path = project.tdt_dir().join("archive").join(relative_path);

    // Create archive directory
    if let Some(parent) = archive_path.parent() {
        fs::create_dir_all(parent).into_diagnostic()?;
    }

    // Move the file
    fs::rename(&file_path, &archive_path).into_diagnostic()?;

    Ok(archive_path)
}

/// Find an entity file in the given directories
fn find_entity_file_in_dirs(
    project: &Project,
    entity_id: &str,
    entity_dirs: &[&str],
) -> Result<PathBuf> {
    for dir in entity_dirs {
        let dir_path = project.root().join(dir);
        if !dir_path.exists() {
            continue;
        }

        for entry in fs::read_dir(&dir_path).into_diagnostic()? {
            let entry = entry.into_diagnostic()?;
            let path = entry.path();

            if path.extension().is_some_and(|e| e == "yaml") {
                let filename = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                if filename.contains(entity_id) || filename.starts_with(entity_id) {
                    return Ok(path);
                }
            }
        }
    }

    Err(miette::miette!("No entity found matching '{}'", entity_id))
}

/// Common delete command implementation that can be shared across entity types
pub fn run_delete(
    entity_id: &str,
    entity_dirs: &[&str],
    force: bool,
    archive: bool,
    quiet: bool,
) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;

    // Resolve short ID if needed
    let short_ids = ShortIdIndex::load(&project);
    let resolved_id = short_ids
        .resolve(entity_id)
        .unwrap_or_else(|| entity_id.to_string());

    if archive {
        let archive_path = archive_entity_file(&project, &resolved_id, entity_dirs, force)?;
        if !quiet {
            println!(
                "{} Archived entity to {}",
                style("✓").green(),
                style(archive_path.display()).dim()
            );
        }
    } else {
        let deleted_path = delete_entity_file(&project, &resolved_id, entity_dirs, force)?;
        if !quiet {
            println!(
                "{} Deleted {}",
                style("✓").green(),
                style(deleted_path.display()).cyan()
            );
        }
    }

    Ok(())
}
