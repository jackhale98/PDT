//! Short ID system for easier entity selection
//!
//! Provides prefixed aliases that map to full entity IDs.
//! Format: `REQ@1`, `RISK@2`, `TEST@3`
//!
//! IMPORTANT: Short IDs are user-local and stored in the SQLite cache.
//! They should NEVER appear in entity YAML files - only full ULIDs.
//!
//! Aliases are stable - once assigned, an entity keeps its alias.
//! New aliases are only added when new entities are created.

use std::collections::HashMap;
use std::fs;

use crate::core::cache::EntityCache;
use crate::core::identity::EntityId;
use crate::core::project::Project;

/// Legacy index file location (for migration)
const LEGACY_INDEX_FILE: &str = ".tdt/shortids.json";

/// A mapping of prefixed short IDs to full entity IDs
///
/// This is kept for backward compatibility with existing code and tests.
/// For new code, prefer using `EntityCache` directly.
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct ShortIdIndex {
    /// Maps "PREFIX@N" to full entity ID string (e.g., "REQ@1" -> "REQ-01ABC...")
    entries: HashMap<String, String>,
    /// Next available short ID per prefix
    next_ids: HashMap<String, u32>,
    /// Reverse lookup (not persisted, rebuilt on load)
    #[serde(skip)]
    reverse: HashMap<String, String>,
}

impl ShortIdIndex {
    /// Create a new empty index
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            next_ids: HashMap::new(),
            reverse: HashMap::new(),
        }
    }

    /// Load the index from the cache, migrating from JSON if needed
    ///
    /// This now uses the SQLite cache as the primary storage.
    /// If a legacy shortids.json exists, it will be migrated and deleted.
    pub fn load(project: &Project) -> Self {
        // Try to open/create the cache
        let cache = match EntityCache::open(project) {
            Ok(c) => c,
            Err(_) => return Self::new(),
        };

        // Build in-memory index from cache
        let mut index = Self::new();

        // Query all short IDs from cache
        if let Ok(rows) = cache.query_raw("SELECT short_id, entity_id FROM short_ids") {
            for row in rows {
                if row.len() >= 2 {
                    let short_id = &row[0];
                    let entity_id = &row[1];
                    index.entries.insert(short_id.clone(), entity_id.clone());
                    index.reverse.insert(entity_id.clone(), short_id.clone());

                    // Update next_ids
                    if let Some(at_pos) = short_id.find('@') {
                        let prefix = &short_id[..at_pos];
                        if let Ok(num) = short_id[at_pos + 1..].parse::<u32>() {
                            let next = index.next_ids.entry(prefix.to_string()).or_insert(1);
                            if num >= *next {
                                *next = num + 1;
                            }
                        }
                    }
                }
            }
        }

        // Check for and migrate legacy JSON file
        let legacy_path = project.root().join(LEGACY_INDEX_FILE);
        if legacy_path.exists() {
            if let Ok(content) = fs::read_to_string(&legacy_path) {
                if let Ok(legacy_index) = serde_json::from_str::<ShortIdIndex>(&content) {
                    // Merge legacy entries (cache takes precedence for conflicts)
                    for (short_id, entity_id) in legacy_index.entries {
                        if !index.entries.contains_key(&short_id) {
                            index.entries.insert(short_id.clone(), entity_id.clone());
                            index.reverse.insert(entity_id, short_id);
                        }
                    }
                }
            }
            // Delete legacy file after migration
            let _ = fs::remove_file(&legacy_path);
        }

        index
    }

    /// Save the index - now delegates to cache
    ///
    /// Note: Short IDs are automatically saved to the SQLite cache.
    /// This method is kept for backward compatibility but is mostly a no-op.
    pub fn save(&self, project: &Project) -> std::io::Result<()> {
        // Open cache and ensure all our entries are saved
        if let Ok(mut cache) = EntityCache::open(project) {
            for entity_id in self.entries.values() {
                let _ = cache.ensure_short_id(entity_id);
            }
        }
        Ok(())
    }

    /// Extract the prefix from an entity ID (e.g., "REQ" from "REQ-01ABC...")
    fn extract_prefix(entity_id: &str) -> Option<&str> {
        entity_id.split('-').next()
    }

    /// Add an entity ID if not already present, returns its short ID
    pub fn add(&mut self, entity_id: String) -> Option<String> {
        // Already exists? Return existing alias
        if let Some(existing) = self.reverse.get(&entity_id) {
            return Some(existing.clone());
        }

        // Extract prefix and create new alias
        let prefix = Self::extract_prefix(&entity_id)?;
        let next = self.next_ids.entry(prefix.to_string()).or_insert(1);
        let short_id = format!("{}@{}", prefix, next);

        self.entries.insert(short_id.clone(), entity_id.clone());
        self.reverse.insert(entity_id, short_id.clone());
        *next += 1;

        Some(short_id)
    }

    /// Ensure all entity IDs have aliases (adds missing ones, keeps existing)
    pub fn ensure_all(&mut self, entity_ids: impl IntoIterator<Item = String>) {
        for id in entity_ids {
            self.add(id);
        }
    }

    /// Resolve a short ID reference to a full entity ID
    ///
    /// Accepts:
    /// - `PREFIX@N` format (e.g., `REQ@1`, `req@1`, `Req@1`)
    /// - Full or partial entity ID (passed through)
    pub fn resolve(&self, reference: &str) -> Option<String> {
        // Check for prefixed format: PREFIX@N (case-insensitive)
        if let Some(at_pos) = reference.find('@') {
            let prefix = &reference[..at_pos];
            if !prefix.is_empty() && prefix.chars().all(|c| c.is_ascii_alphabetic()) {
                // Normalize to uppercase for lookup
                let normalized = format!(
                    "{}@{}",
                    prefix.to_ascii_uppercase(),
                    &reference[at_pos + 1..]
                );
                return self.entries.get(&normalized).cloned();
            }
        }

        // Not a short ID, pass through for partial matching
        Some(reference.to_string())
    }

    /// Get the short ID for a full entity ID
    pub fn get_short_id(&self, entity_id: &str) -> Option<String> {
        self.reverse.get(entity_id).cloned()
    }

    /// Get the numeric part of a short ID (for display)
    pub fn get_number(&self, entity_id: &str) -> Option<u32> {
        self.reverse
            .get(entity_id)
            .and_then(|s| s.split('@').nth(1).and_then(|n| n.parse().ok()))
    }

    /// Format an entity ID with its short ID prefix for display
    pub fn format_with_short_id(&self, entity_id: &EntityId) -> String {
        let id_str = entity_id.to_string();
        if let Some(short_id) = self.reverse.get(&id_str) {
            format!("{:<8} {}", short_id, id_str)
        } else {
            format!("{:8} {}", "", id_str)
        }
    }

    /// Number of entries
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Parse a reference that might be a short ID or a full/partial entity ID
///
/// This is the primary entry point for resolving entity references.
/// It uses the SQLite cache for short ID resolution.
pub fn parse_entity_reference(reference: &str, project: &Project) -> String {
    // Check for short ID format (PREFIX@N)
    if reference.contains('@') {
        // Use cache directly for better performance
        if let Ok(cache) = EntityCache::open(project) {
            if let Some(entity_id) = cache.resolve_short_id(reference) {
                return entity_id;
            }
        }
    }

    // Not a short ID or not found - return as-is for partial matching downstream
    reference.to_string()
}

/// Get the short ID for an entity, assigning one if needed
///
/// This is useful when displaying entities - ensures they have a short ID.
pub fn get_or_create_short_id(entity_id: &str, project: &Project) -> Option<String> {
    let mut cache = EntityCache::open(project).ok()?;
    cache.ensure_short_id(entity_id).ok()
}

/// Get the short ID for an entity without creating one
pub fn get_short_id(entity_id: &str, project: &Project) -> Option<String> {
    let cache = EntityCache::open(project).ok()?;
    cache.get_short_id(entity_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_and_resolve() {
        let mut index = ShortIdIndex::new();

        let short1 = index.add("REQ-01ABC".to_string());
        let short2 = index.add("REQ-02DEF".to_string());

        assert_eq!(short1, Some("REQ@1".to_string()));
        assert_eq!(short2, Some("REQ@2".to_string()));

        assert_eq!(index.resolve("REQ@1"), Some("REQ-01ABC".to_string()));
        assert_eq!(index.resolve("REQ@2"), Some("REQ-02DEF".to_string()));
        assert_eq!(index.resolve("REQ@99"), None);
    }

    #[test]
    fn test_multiple_entity_types() {
        let mut index = ShortIdIndex::new();

        index.add("REQ-01ABC".to_string());
        index.add("RISK-01GHI".to_string());
        index.add("REQ-02DEF".to_string());

        assert_eq!(index.resolve("REQ@1"), Some("REQ-01ABC".to_string()));
        assert_eq!(index.resolve("REQ@2"), Some("REQ-02DEF".to_string()));
        assert_eq!(index.resolve("RISK@1"), Some("RISK-01GHI".to_string()));
    }

    #[test]
    fn test_passthrough() {
        let index = ShortIdIndex::new();

        // Non-short-ID references pass through
        assert_eq!(index.resolve("REQ-01ABC"), Some("REQ-01ABC".to_string()));
        assert_eq!(
            index.resolve("temperature"),
            Some("temperature".to_string())
        );
    }

    #[test]
    fn test_no_duplicates() {
        let mut index = ShortIdIndex::new();

        let short1 = index.add("REQ-001".to_string());
        let short2 = index.add("REQ-001".to_string()); // Same ID

        assert_eq!(short1, short2);
        assert_eq!(index.len(), 1);
    }

    #[test]
    fn test_stable_aliases() {
        let mut index = ShortIdIndex::new();

        // Add some entities
        index.add("REQ-001".to_string());
        index.add("REQ-002".to_string());

        // ensure_all with same + new entities keeps existing aliases
        index.ensure_all(vec![
            "REQ-002".to_string(), // existing
            "REQ-001".to_string(), // existing
            "REQ-003".to_string(), // new
        ]);

        // Original aliases unchanged
        assert_eq!(index.resolve("REQ@1"), Some("REQ-001".to_string()));
        assert_eq!(index.resolve("REQ@2"), Some("REQ-002".to_string()));
        // New one gets next number
        assert_eq!(index.resolve("REQ@3"), Some("REQ-003".to_string()));
    }

    #[test]
    fn test_get_number() {
        let mut index = ShortIdIndex::new();
        index.add("REQ-001".to_string());
        index.add("REQ-002".to_string());

        assert_eq!(index.get_number("REQ-001"), Some(1));
        assert_eq!(index.get_number("REQ-002"), Some(2));
        assert_eq!(index.get_number("REQ-003"), None);
    }

    #[test]
    fn test_cross_entity_types() {
        let mut index = ShortIdIndex::new();

        index.add("REQ-01ABCDEF".to_string());
        index.add("RISK-01GHIJKL".to_string());
        index.add("TEST-01MNOPQR".to_string());
        index.add("RSLT-01STUVWX".to_string());

        assert_eq!(index.resolve("REQ@1"), Some("REQ-01ABCDEF".to_string()));
        assert_eq!(index.resolve("RISK@1"), Some("RISK-01GHIJKL".to_string()));
        assert_eq!(index.resolve("TEST@1"), Some("TEST-01MNOPQR".to_string()));
        assert_eq!(index.resolve("RSLT@1"), Some("RSLT-01STUVWX".to_string()));
    }

    #[test]
    fn test_case_insensitive_resolve() {
        let mut index = ShortIdIndex::new();

        index.add("REQ-01ABCDEF".to_string());
        index.add("RISK-01GHIJKL".to_string());

        // All case variants should resolve
        assert_eq!(index.resolve("REQ@1"), Some("REQ-01ABCDEF".to_string()));
        assert_eq!(index.resolve("req@1"), Some("REQ-01ABCDEF".to_string()));
        assert_eq!(index.resolve("Req@1"), Some("REQ-01ABCDEF".to_string()));
        assert_eq!(index.resolve("rEq@1"), Some("REQ-01ABCDEF".to_string()));

        assert_eq!(index.resolve("RISK@1"), Some("RISK-01GHIJKL".to_string()));
        assert_eq!(index.resolve("risk@1"), Some("RISK-01GHIJKL".to_string()));
        assert_eq!(index.resolve("Risk@1"), Some("RISK-01GHIJKL".to_string()));
    }
}
