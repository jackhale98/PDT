//! Short ID system for easier entity selection
//!
//! Provides prefixed aliases that map to full entity IDs.
//! Format: `REQ@1`, `RISK@2`, `TEST@3` (cross-entity safe)
//!
//! These are persisted in .pdt/shortids.json and regenerated when entities are listed.

use std::collections::HashMap;
use std::fs;

use crate::core::identity::EntityId;
use crate::core::project::Project;

/// Index file location within a project
const INDEX_FILE: &str = ".pdt/shortids.json";

/// A mapping of prefixed short IDs to full entity IDs
///
/// Supports entity-prefixed aliases (REQ@1, RISK@2) for cross-entity operations.
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct ShortIdIndex {
    /// Maps "PREFIX@N" to full entity ID string (e.g., "REQ@1" -> "REQ-01ABC...")
    entries: HashMap<String, String>,
    /// Maps full entity ID to prefixed short ID (reverse lookup)
    #[serde(skip)]
    reverse: HashMap<String, String>,
    /// Next available short ID per prefix
    next_ids: HashMap<String, u32>,
}

impl ShortIdIndex {
    /// Create a new empty index
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            reverse: HashMap::new(),
            next_ids: HashMap::new(),
        }
    }

    /// Load the index from a project, or create empty if not found
    pub fn load(project: &Project) -> Self {
        let path = project.root().join(INDEX_FILE);
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(mut index) = serde_json::from_str::<ShortIdIndex>(&content) {
                    // Rebuild reverse lookup
                    index.reverse = index
                        .entries
                        .iter()
                        .map(|(k, v)| (v.clone(), k.clone()))
                        .collect();
                    return index;
                }
            }
        }
        Self::new()
    }

    /// Save the index to a project
    pub fn save(&self, project: &Project) -> std::io::Result<()> {
        let path = project.root().join(INDEX_FILE);
        let content = serde_json::to_string_pretty(self)?;
        fs::write(path, content)
    }

    /// Clear and rebuild the index with new entity IDs
    ///
    /// Automatically extracts prefix from entity IDs and rebuilds entries for each prefix.
    pub fn rebuild(&mut self, entity_ids: impl IntoIterator<Item = String>) {
        let ids: Vec<String> = entity_ids.into_iter().collect();

        // Find all prefixes and rebuild each
        let prefixes: std::collections::HashSet<String> = ids
            .iter()
            .filter_map(|id| Self::extract_prefix(id).map(String::from))
            .collect();

        for prefix in &prefixes {
            self.rebuild_for_prefix(prefix, ids.iter().filter(|id| id.starts_with(&format!("{}-", prefix))).cloned());
        }
    }

    /// Clear and rebuild the index for a specific entity type
    ///
    /// This resets the numbering for the given prefix.
    pub fn rebuild_for_prefix(&mut self, prefix: &str, entity_ids: impl IntoIterator<Item = String>) {
        // Remove old entries for this prefix
        let old_keys: Vec<String> = self
            .entries
            .keys()
            .filter(|k| k.starts_with(&format!("{}@", prefix)))
            .cloned()
            .collect();

        for key in old_keys {
            if let Some(entity_id) = self.entries.remove(&key) {
                self.reverse.remove(&entity_id);
            }
        }

        // Reset counter for this prefix
        self.next_ids.insert(prefix.to_string(), 1);

        // Add new entities
        for id in entity_ids {
            self.add(id);
        }
    }

    /// Extract the prefix from an entity ID (e.g., "REQ" from "REQ-01ABC...")
    fn extract_prefix(entity_id: &str) -> Option<&str> {
        entity_id.split('-').next()
    }

    /// Add an entity ID and return its short ID (e.g., "REQ@1")
    pub fn add(&mut self, entity_id: String) -> Option<String> {
        // Check if already exists
        if let Some(existing) = self.reverse.get(&entity_id) {
            return Some(existing.clone());
        }

        // Extract prefix and create prefixed short ID
        let prefix = Self::extract_prefix(&entity_id)?;
        let next = self.next_ids.entry(prefix.to_string()).or_insert(1);
        let prefixed_key = format!("{}@{}", prefix, next);
        self.entries.insert(prefixed_key.clone(), entity_id.clone());
        self.reverse.insert(entity_id, prefixed_key.clone());
        *next += 1;

        Some(prefixed_key)
    }

    /// Resolve a short ID reference to a full entity ID
    ///
    /// Accepts:
    /// - `PREFIX@N` format (e.g., `REQ@1`, `RISK@42`)
    /// - Full or partial entity ID (passed through for matching)
    pub fn resolve(&self, reference: &str) -> Option<String> {
        // Check for prefixed format: PREFIX@N (e.g., REQ@1, RISK@2)
        if let Some(at_pos) = reference.find('@') {
            let prefix = &reference[..at_pos];

            // If there's a non-empty uppercase prefix, it's a prefixed reference
            if !prefix.is_empty() && prefix.chars().all(|c| c.is_ascii_uppercase()) {
                return self.entries.get(reference).cloned();
            }
        }

        // Not a short ID reference, return as-is for partial matching
        Some(reference.to_string())
    }

    /// Get the prefixed short ID for a full entity ID (e.g., "REQ@1")
    pub fn get_short_id(&self, entity_id: &str) -> Option<String> {
        self.reverse.get(entity_id).cloned()
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

    /// Get short ID number for an entity (for backward compat with display)
    pub fn get_number(&self, entity_id: &str) -> Option<u32> {
        self.reverse.get(entity_id).and_then(|s| {
            s.split('@').nth(1).and_then(|n| n.parse().ok())
        })
    }

    /// Get all entries as (prefixed_id, full_id) pairs
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.entries.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    /// Number of entries in the index
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the index is empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Merge another index's entries into this one (preserving existing entries)
    pub fn merge(&mut self, other: &ShortIdIndex) {
        for (key, value) in &other.entries {
            if !self.entries.contains_key(key) {
                self.entries.insert(key.clone(), value.clone());
                self.reverse.insert(value.clone(), key.clone());
            }
        }
        // Update next_ids to avoid collisions
        for (prefix, &next) in &other.next_ids {
            let current = self.next_ids.entry(prefix.clone()).or_insert(1);
            if next > *current {
                *current = next;
            }
        }
    }
}

/// Parse a reference that might be a short ID or a full/partial entity ID
pub fn parse_entity_reference(reference: &str, project: &Project) -> String {
    let index = ShortIdIndex::load(project);
    index.resolve(reference).unwrap_or_else(|| reference.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_short_id_add_and_resolve() {
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
    fn test_prefixed_short_id_multiple_types() {
        let mut index = ShortIdIndex::new();

        index.add("REQ-01ABC".to_string());
        index.add("REQ-02DEF".to_string());
        index.add("RISK-01GHI".to_string());

        // Prefixed format should work
        assert_eq!(index.resolve("REQ@1"), Some("REQ-01ABC".to_string()));
        assert_eq!(index.resolve("REQ@2"), Some("REQ-02DEF".to_string()));
        assert_eq!(index.resolve("RISK@1"), Some("RISK-01GHI".to_string()));

        // Prefixed IDs should be independent per entity type
        assert_ne!(index.resolve("REQ@1"), index.resolve("RISK@1"));
    }

    #[test]
    fn test_short_id_passthrough() {
        let index = ShortIdIndex::new();

        // Non-short-ID references should pass through
        assert_eq!(index.resolve("REQ-01ABC"), Some("REQ-01ABC".to_string()));
        assert_eq!(
            index.resolve("temperature"),
            Some("temperature".to_string())
        );
    }

    #[test]
    fn test_short_id_rebuild_for_prefix() {
        let mut index = ShortIdIndex::new();
        index.add("REQ-001".to_string());
        index.add("REQ-002".to_string());
        index.add("RISK-001".to_string());

        assert_eq!(index.len(), 3);

        // Rebuild only REQ entries
        index.rebuild_for_prefix(
            "REQ",
            vec!["REQ-NEW1".to_string(), "REQ-NEW2".to_string()],
        );

        // REQ entries are rebuilt
        assert_eq!(index.resolve("REQ@1"), Some("REQ-NEW1".to_string()));
        assert_eq!(index.resolve("REQ@2"), Some("REQ-NEW2".to_string()));

        // RISK entries unchanged
        assert_eq!(index.resolve("RISK@1"), Some("RISK-001".to_string()));
    }

    #[test]
    fn test_short_id_no_duplicates() {
        let mut index = ShortIdIndex::new();

        let short1 = index.add("REQ-001".to_string());
        let short2 = index.add("REQ-001".to_string()); // Same ID

        assert_eq!(short1, short2);
        assert_eq!(index.len(), 1);
    }

    #[test]
    fn test_get_short_id() {
        let mut index = ShortIdIndex::new();
        index.add("REQ-001".to_string());
        index.add("RISK-001".to_string());

        assert_eq!(
            index.get_short_id("REQ-001"),
            Some("REQ@1".to_string())
        );
        assert_eq!(
            index.get_short_id("RISK-001"),
            Some("RISK@1".to_string())
        );
        assert_eq!(index.get_short_id("TEST-001"), None);
    }

    #[test]
    fn test_cross_entity_linking() {
        let mut index = ShortIdIndex::new();

        // Add entities of different types
        index.add("REQ-01ABCDEF".to_string());
        index.add("RISK-01GHIJKL".to_string());
        index.add("TEST-01MNOPQR".to_string());
        index.add("RSLT-01STUVWX".to_string());

        // Can resolve any entity type with prefixed format
        assert_eq!(index.resolve("REQ@1"), Some("REQ-01ABCDEF".to_string()));
        assert_eq!(index.resolve("RISK@1"), Some("RISK-01GHIJKL".to_string()));
        assert_eq!(index.resolve("TEST@1"), Some("TEST-01MNOPQR".to_string()));
        assert_eq!(index.resolve("RSLT@1"), Some("RSLT-01STUVWX".to_string()));
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
}
