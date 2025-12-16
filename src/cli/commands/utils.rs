//! Shared utilities for CLI commands

use crate::core::cache::EntityCache;
use crate::core::shortid::ShortIdIndex;

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
