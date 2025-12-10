//! Short ID system for easier entity selection
//!
//! Provides session-local aliases that map to full entity IDs.
//! Supports two formats:
//! - Entity-prefixed: `REQ@1`, `RISK@2`, `TEST@3` (cross-entity safe)
//! - Simple: `@1`, `@2` (works within same entity type)
//!
//! These are persisted in .pdt/shortids.json and regenerated when entities are listed.

use std::collections::HashMap;
use std::fs;

use crate::core::identity::EntityId;
use crate::core::project::Project;

/// Index file location within a project
const INDEX_FILE: &str = ".pdt/shortids.json";

/// A mapping of short IDs to full entity IDs
///
/// Supports entity-prefixed aliases (REQ@1, RISK@2) for cross-entity operations
/// and simple aliases (@1, @2) for within-entity-type operations.
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct ShortIdIndex {
    /// Maps "PREFIX@N" to full entity ID string (e.g., "REQ@1" -> "REQ-01ABC...")
    entries: HashMap<String, String>,
    /// Maps full entity ID to prefixed short ID (reverse lookup)
    #[serde(skip)]
    reverse: HashMap<String, String>,
    /// Next available short ID per prefix
    next_ids: HashMap<String, u32>,
    /// Legacy: simple @N to full ID (for backwards compatibility within single list)
    #[serde(skip)]
    simple_entries: HashMap<u32, String>,
    /// Legacy: full ID to simple @N
    #[serde(skip)]
    simple_reverse: HashMap<String, u32>,
    /// Legacy next ID for simple entries
    #[serde(skip)]
    simple_next_id: u32,
}

impl ShortIdIndex {
    /// Create a new empty index
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            reverse: HashMap::new(),
            next_ids: HashMap::new(),
            simple_entries: HashMap::new(),
            simple_reverse: HashMap::new(),
            simple_next_id: 1,
        }
    }

    /// Load the index from a project, or create empty if not found
    pub fn load(project: &Project) -> Self {
        let path = project.root().join(INDEX_FILE);
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(mut index) = serde_json::from_str::<ShortIdIndex>(&content) {
                    // Rebuild reverse lookup
                    index.reverse = index.entries.iter()
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

    /// Clear and rebuild the index with new entity IDs (for a single entity type list)
    ///
    /// This clears the simple @N mappings but preserves and updates prefixed mappings.
    pub fn rebuild(&mut self, entity_ids: impl IntoIterator<Item = String>) {
        // Clear simple entries (these are session-local for current list)
        self.simple_entries.clear();
        self.simple_reverse.clear();
        self.simple_next_id = 1;

        for id in entity_ids {
            self.add(id);
        }
    }

    /// Extract the prefix from an entity ID (e.g., "REQ" from "REQ-01ABC...")
    fn extract_prefix(entity_id: &str) -> Option<&str> {
        entity_id.split('-').next()
    }

    /// Add an entity ID and return its simple short ID number
    ///
    /// This adds to both the prefixed index (REQ@1) and simple index (@1).
    pub fn add(&mut self, entity_id: String) -> u32 {
        // Handle simple @N (for current list session)
        let simple_id = if let Some(&existing) = self.simple_reverse.get(&entity_id) {
            existing
        } else {
            let id = self.simple_next_id;
            self.simple_next_id += 1;
            self.simple_entries.insert(id, entity_id.clone());
            self.simple_reverse.insert(entity_id.clone(), id);
            id
        };

        // Handle prefixed PREFIX@N (persistent)
        if let Some(prefix) = Self::extract_prefix(&entity_id) {
            if !self.reverse.contains_key(&entity_id) {
                let next = self.next_ids.entry(prefix.to_string()).or_insert(1);
                let prefixed_key = format!("{}@{}", prefix, next);
                self.entries.insert(prefixed_key.clone(), entity_id.clone());
                self.reverse.insert(entity_id, prefixed_key);
                *next += 1;
            }
        }

        simple_id
    }

    /// Resolve a short ID reference to a full entity ID
    ///
    /// Accepts:
    /// - `PREFIX@N` format (e.g., `REQ@1`, `RISK@42`) - uses persistent prefixed index
    /// - `@N` format (e.g., `@1`, `@42`) - uses session-local simple index
    /// - Plain number (e.g., `1`, `42`) - uses session-local simple index
    /// - Full or partial entity ID (passed through)
    pub fn resolve(&self, reference: &str) -> Option<String> {
        // Check for prefixed format: PREFIX@N (e.g., REQ@1, RISK@2)
        if let Some(at_pos) = reference.find('@') {
            let prefix = &reference[..at_pos];
            let num_str = &reference[at_pos + 1..];

            // If there's a non-empty prefix, it's a prefixed reference
            if !prefix.is_empty() && prefix.chars().all(|c| c.is_ascii_uppercase()) {
                // Look up in prefixed entries
                return self.entries.get(reference).cloned();
            }
        }

        // Check if it's a simple short ID reference (@N or just N)
        let num_str = if reference.starts_with('@') {
            &reference[1..]
        } else if reference.chars().all(|c| c.is_ascii_digit()) {
            reference
        } else {
            // Not a short ID, return as-is for partial matching
            return Some(reference.to_string());
        };

        // Parse the number and look up in simple entries
        num_str.parse::<u32>().ok()
            .and_then(|n| self.simple_entries.get(&n).cloned())
            // Fall back to prefixed entries if simple not found
            .or_else(|| {
                // Try to find any prefixed entry ending with @N
                for (key, value) in &self.entries {
                    if key.ends_with(&format!("@{}", num_str)) {
                        return Some(value.clone());
                    }
                }
                None
            })
    }

    /// Get the simple short ID number for a full entity ID
    pub fn get_short_id(&self, entity_id: &str) -> Option<u32> {
        self.simple_reverse.get(entity_id).copied()
    }

    /// Get the prefixed short ID for a full entity ID (e.g., "REQ@1")
    pub fn get_prefixed_short_id(&self, entity_id: &str) -> Option<String> {
        self.reverse.get(entity_id).cloned()
    }

    /// Format an entity ID with its short ID prefix
    pub fn format_with_short_id(&self, entity_id: &EntityId) -> String {
        let id_str = entity_id.to_string();
        if let Some(short_id) = self.simple_reverse.get(&id_str) {
            format!("@{:<3} {}", short_id, id_str)
        } else {
            format!("     {}", id_str)
        }
    }

    /// Format an entity ID with its prefixed short ID (e.g., "REQ@1")
    pub fn format_with_prefixed_short_id(&self, entity_id: &EntityId) -> Option<String> {
        let id_str = entity_id.to_string();
        self.reverse.get(&id_str).cloned()
    }

    /// Get all simple entries as (short_id, full_id) pairs
    pub fn iter(&self) -> impl Iterator<Item = (u32, &str)> {
        self.simple_entries.iter().map(|(k, v)| (*k, v.as_str()))
    }

    /// Get all prefixed entries as (prefixed_id, full_id) pairs
    pub fn iter_prefixed(&self) -> impl Iterator<Item = (&str, &str)> {
        self.entries.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    /// Number of simple entries in the index (current session)
    pub fn len(&self) -> usize {
        self.simple_entries.len()
    }

    /// Total number of prefixed entries (all entity types)
    pub fn prefixed_len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the index is empty
    pub fn is_empty(&self) -> bool {
        self.simple_entries.is_empty()
    }

    /// Merge another index's prefixed entries into this one
    ///
    /// This is useful for combining indexes from different list operations.
    pub fn merge_prefixed(&mut self, other: &ShortIdIndex) {
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

        assert_eq!(short1, 1);
        assert_eq!(short2, 2);

        // Simple @N format works for current session
        assert_eq!(index.resolve("@1"), Some("REQ-01ABC".to_string()));
        assert_eq!(index.resolve("@2"), Some("REQ-02DEF".to_string()));
        assert_eq!(index.resolve("1"), Some("REQ-01ABC".to_string()));
        assert_eq!(index.resolve("@99"), None);
    }

    #[test]
    fn test_prefixed_short_id() {
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

        // Non-numeric references should pass through
        assert_eq!(index.resolve("REQ-01ABC"), Some("REQ-01ABC".to_string()));
        assert_eq!(index.resolve("temperature"), Some("temperature".to_string()));
    }

    #[test]
    fn test_short_id_rebuild() {
        let mut index = ShortIdIndex::new();
        index.add("OLD-001".to_string());
        index.add("OLD-002".to_string());

        assert_eq!(index.len(), 2);

        index.rebuild(vec!["NEW-001".to_string(), "NEW-002".to_string(), "NEW-003".to_string()]);

        // Simple entries get rebuilt
        assert_eq!(index.len(), 3);
        assert_eq!(index.resolve("@1"), Some("NEW-001".to_string()));
        assert_eq!(index.resolve("@3"), Some("NEW-003".to_string()));

        // Prefixed entries accumulate (old ones preserved, new ones added)
        assert!(index.prefixed_len() >= 3);
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
        index.add("REQ-002".to_string());

        assert_eq!(index.get_short_id("REQ-001"), Some(1));
        assert_eq!(index.get_short_id("REQ-002"), Some(2));
        assert_eq!(index.get_short_id("REQ-003"), None);
    }

    #[test]
    fn test_get_prefixed_short_id() {
        let mut index = ShortIdIndex::new();
        index.add("REQ-001".to_string());
        index.add("RISK-001".to_string());

        assert_eq!(index.get_prefixed_short_id("REQ-001"), Some("REQ@1".to_string()));
        assert_eq!(index.get_prefixed_short_id("RISK-001"), Some("RISK@1".to_string()));
        assert_eq!(index.get_prefixed_short_id("TEST-001"), None);
    }

    #[test]
    fn test_cross_entity_linking() {
        let mut index = ShortIdIndex::new();

        // Add entities of different types
        index.add("REQ-01ABCDEF".to_string());
        index.add("RISK-01GHIJKL".to_string());
        index.add("TEST-01MNOPQR".to_string());

        // Can resolve any entity type with prefixed format
        assert_eq!(index.resolve("REQ@1"), Some("REQ-01ABCDEF".to_string()));
        assert_eq!(index.resolve("RISK@1"), Some("RISK-01GHIJKL".to_string()));
        assert_eq!(index.resolve("TEST@1"), Some("TEST-01MNOPQR".to_string()));

        // Simple @N refers to current session order
        assert_eq!(index.resolve("@1"), Some("REQ-01ABCDEF".to_string()));
        assert_eq!(index.resolve("@2"), Some("RISK-01GHIJKL".to_string()));
        assert_eq!(index.resolve("@3"), Some("TEST-01MNOPQR".to_string()));
    }
}
