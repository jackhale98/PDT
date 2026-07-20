//! Suspect link tracking for change impact analysis
//!
//! When an entity is modified (revision change or status regression),
//! its incoming links become "suspect" until reviewed and cleared.
//! This helps teams understand the impact of changes on traceability.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;

use crate::core::cache::EntityCache;
use crate::core::project::Project;

/// Reason why a link became suspect
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SuspectReason {
    /// Target entity revision was incremented
    RevisionChanged,
    /// Target entity status regressed (e.g., approved → draft)
    StatusRegressed,
    /// Link was manually marked as suspect
    ManuallyMarked,
    /// Target entity content was modified
    ContentModified,
}

impl std::fmt::Display for SuspectReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SuspectReason::RevisionChanged => write!(f, "revision changed"),
            SuspectReason::StatusRegressed => write!(f, "status regressed"),
            SuspectReason::ManuallyMarked => write!(f, "manually marked"),
            SuspectReason::ContentModified => write!(f, "content modified"),
        }
    }
}

/// Extended link reference with suspect tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtendedLinkRef {
    /// Target entity ID
    pub id: String,

    /// Cached title of the target entity (written by `tdt link add` so links
    /// stay human-readable in the YAML)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    /// Whether the link is suspect (needs review)
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub suspect: bool,

    /// Reason the link became suspect
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suspect_reason: Option<SuspectReason>,

    /// When the link became suspect
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suspect_since: Option<DateTime<Utc>>,

    /// Target entity revision when link was verified
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verified_revision: Option<u32>,
}

impl ExtendedLinkRef {
    /// Create a new non-suspect link reference
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: None,
            suspect: false,
            suspect_reason: None,
            suspect_since: None,
            verified_revision: None,
        }
    }

    /// Mark this link as suspect
    pub fn mark_suspect(&mut self, reason: SuspectReason) {
        self.suspect = true;
        self.suspect_reason = Some(reason);
        self.suspect_since = Some(Utc::now());
    }

    /// Clear suspect status (after review)
    pub fn clear_suspect(&mut self, verified_revision: Option<u32>) {
        self.suspect = false;
        self.suspect_reason = None;
        self.suspect_since = None;
        self.verified_revision = verified_revision;
    }
}

/// A link reference that can be either simple (string) or extended (with suspect tracking)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum LinkRef {
    /// Simple string reference (e.g., "TEST-01ABC...")
    Simple(String),
    /// Extended reference with suspect tracking
    Extended(ExtendedLinkRef),
}

impl LinkRef {
    /// Get the target entity ID
    pub fn id(&self) -> &str {
        match self {
            LinkRef::Simple(id) => id,
            LinkRef::Extended(ext) => &ext.id,
        }
    }

    /// Check if the link is suspect
    pub fn is_suspect(&self) -> bool {
        match self {
            LinkRef::Simple(_) => false,
            LinkRef::Extended(ext) => ext.suspect,
        }
    }

    /// Get the suspect reason if any
    pub fn suspect_reason(&self) -> Option<&SuspectReason> {
        match self {
            LinkRef::Simple(_) => None,
            LinkRef::Extended(ext) => ext.suspect_reason.as_ref(),
        }
    }

    /// Convert to extended format if needed and mark as suspect
    pub fn mark_suspect(&mut self, reason: SuspectReason) {
        match self {
            LinkRef::Simple(id) => {
                let mut ext = ExtendedLinkRef::new(id.clone());
                ext.mark_suspect(reason);
                *self = LinkRef::Extended(ext);
            }
            LinkRef::Extended(ext) => {
                ext.mark_suspect(reason);
            }
        }
    }

    /// Clear suspect status
    pub fn clear_suspect(&mut self, verified_revision: Option<u32>) {
        if let LinkRef::Extended(ext) = self {
            ext.clear_suspect(verified_revision);
        }
    }
}

impl From<String> for LinkRef {
    fn from(s: String) -> Self {
        LinkRef::Simple(s)
    }
}

impl From<&str> for LinkRef {
    fn from(s: &str) -> Self {
        LinkRef::Simple(s.to_string())
    }
}

impl From<crate::core::identity::EntityId> for LinkRef {
    fn from(id: crate::core::identity::EntityId) -> Self {
        LinkRef::Simple(id.to_string())
    }
}

impl From<&crate::core::identity::EntityId> for LinkRef {
    fn from(id: &crate::core::identity::EntityId) -> Self {
        LinkRef::Simple(id.to_string())
    }
}

// Links are equal when they point at the same entity — metadata (title,
// suspect state) is bookkeeping, not identity. This keeps `contains`-style
// call sites working across the Simple/Extended representations.
impl PartialEq for LinkRef {
    fn eq(&self, other: &Self) -> bool {
        self.id() == other.id()
    }
}

impl Eq for LinkRef {}

impl PartialEq<crate::core::identity::EntityId> for LinkRef {
    fn eq(&self, other: &crate::core::identity::EntityId) -> bool {
        // EntityId's Display is its canonical string form
        *self.id() == other.to_string()
    }
}

impl PartialEq<str> for LinkRef {
    fn eq(&self, other: &str) -> bool {
        self.id() == other
    }
}

impl PartialEq<String> for LinkRef {
    fn eq(&self, other: &String) -> bool {
        self.id() == other
    }
}

impl PartialEq<&str> for LinkRef {
    fn eq(&self, other: &&str) -> bool {
        self.id() == *other
    }
}

impl std::str::FromStr for LinkRef {
    type Err = <crate::core::identity::EntityId as std::str::FromStr>::Err;

    /// Parse a bare ID string into a simple link, validating it as an
    /// EntityId first (same validation the previous `EntityId` fields had).
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let id: crate::core::identity::EntityId = s.parse()?;
        Ok(LinkRef::from(id))
    }
}

impl std::fmt::Display for LinkRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.id())
    }
}

/// Errors related to suspect link operations
#[derive(Debug, Error)]
pub enum SuspectError {
    #[error("Entity not found: {0}")]
    EntityNotFound(String),

    #[error("Link not found: {from} → {to}")]
    LinkNotFound { from: String, to: String },

    #[error("Failed to parse YAML: {message}")]
    YamlError { message: String },

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

/// Summary of suspect links in a project
#[derive(Debug, Default)]
pub struct SuspectSummary {
    /// Total number of suspect links
    pub total_suspect: usize,
    /// Suspect links by reason
    pub by_reason: std::collections::HashMap<String, usize>,
    /// Entities with suspect incoming links
    pub affected_entities: Vec<String>,
}

/// Check if a single link entry has suspect=true
fn entry_is_suspect(entry: &serde_yml::Value) -> bool {
    entry
        .as_mapping()
        .and_then(|m| m.get(serde_yml::Value::String("suspect".to_string())))
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

/// Check if an entity file has any suspect links.
///
/// Handles both array links and single-value links.
pub fn has_suspect_links(file_path: &Path) -> Result<bool, SuspectError> {
    let contents = std::fs::read_to_string(file_path)?;

    let doc: serde_yml::Value =
        serde_yml::from_str(&contents).map_err(|e| SuspectError::YamlError {
            message: e.to_string(),
        })?;

    // Check the links section
    if let Some(links) = doc.get("links") {
        if let Some(links_map) = links.as_mapping() {
            for (_, link_values) in links_map {
                if let Some(seq) = link_values.as_sequence() {
                    for link in seq {
                        if entry_is_suspect(link) {
                            return Ok(true);
                        }
                    }
                } else if entry_is_suspect(link_values) {
                    return Ok(true);
                }
            }
        }
    }

    Ok(false)
}

/// Extract (target_id, reason) from a single suspect link entry.
fn extract_suspect_info(entry: &serde_yml::Value) -> Option<(String, SuspectReason)> {
    let map = entry.as_mapping()?;
    let is_suspect = map
        .get(serde_yml::Value::String("suspect".to_string()))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if !is_suspect {
        return None;
    }

    let target_id = map
        .get(serde_yml::Value::String("id".to_string()))
        .and_then(|v| v.as_str())?
        .to_string();

    let reason = map
        .get(serde_yml::Value::String("suspect_reason".to_string()))
        .and_then(|v| v.as_str())
        .map(|s| match s {
            "revision_changed" => SuspectReason::RevisionChanged,
            "status_regressed" => SuspectReason::StatusRegressed,
            "manually_marked" => SuspectReason::ManuallyMarked,
            "content_modified" => SuspectReason::ContentModified,
            _ => SuspectReason::ManuallyMarked,
        })
        .unwrap_or(SuspectReason::ManuallyMarked);

    Some((target_id, reason))
}

/// Get all suspect links from an entity file.
///
/// Handles both array links and single-value links. Returns
/// `(link_type, target_id, reason)` for each suspect link.
pub fn get_suspect_links(
    file_path: &Path,
) -> Result<Vec<(String, String, SuspectReason)>, SuspectError> {
    let contents = std::fs::read_to_string(file_path)?;

    let doc: serde_yml::Value =
        serde_yml::from_str(&contents).map_err(|e| SuspectError::YamlError {
            message: e.to_string(),
        })?;

    let mut suspect_links = Vec::new();

    if let Some(links) = doc.get("links") {
        if let Some(links_map) = links.as_mapping() {
            for (link_type, link_values) in links_map {
                let link_type_str = link_type.as_str().unwrap_or("unknown").to_string();
                if let Some(seq) = link_values.as_sequence() {
                    for link in seq {
                        if let Some((id, reason)) = extract_suspect_info(link) {
                            suspect_links.push((link_type_str.clone(), id, reason));
                        }
                    }
                } else if let Some((id, reason)) = extract_suspect_info(link_values) {
                    suspect_links.push((link_type_str.clone(), id, reason));
                }
            }
        }
    }

    Ok(suspect_links)
}

/// Apply suspect status to a single link entry, preserving any existing fields
/// (title, verified_revision, etc.). Handles bare strings, mappings, and Null
/// values. Returns true if the entry was modified.
fn apply_suspect_to_entry(
    entry: &mut serde_yml::Value,
    target_id: &str,
    reason: &SuspectReason,
) -> bool {
    // Check if this entry refers to target_id
    let entry_id = match entry {
        serde_yml::Value::String(id) => Some(id.clone()),
        serde_yml::Value::Mapping(map) => map
            .get(serde_yml::Value::String("id".to_string()))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        _ => None,
    };

    if entry_id.as_deref() != Some(target_id) {
        return false;
    }

    // Convert bare strings to mappings (preserving id)
    if matches!(entry, serde_yml::Value::String(_)) {
        let mut map = serde_yml::Mapping::new();
        map.insert(
            serde_yml::Value::String("id".to_string()),
            serde_yml::Value::String(target_id.to_string()),
        );
        *entry = serde_yml::Value::Mapping(map);
    }

    // Now mutate the mapping in place, preserving existing fields
    if let serde_yml::Value::Mapping(map) = entry {
        map.insert(
            serde_yml::Value::String("suspect".to_string()),
            serde_yml::Value::Bool(true),
        );
        map.insert(
            serde_yml::Value::String("suspect_reason".to_string()),
            serde_yml::Value::String(suspect_reason_to_str(reason).to_string()),
        );
        map.insert(
            serde_yml::Value::String("suspect_since".to_string()),
            serde_yml::Value::String(Utc::now().to_rfc3339()),
        );
        return true;
    }

    false
}

fn suspect_reason_to_str(reason: &SuspectReason) -> &'static str {
    match reason {
        SuspectReason::RevisionChanged => "revision_changed",
        SuspectReason::StatusRegressed => "status_regressed",
        SuspectReason::ManuallyMarked => "manually_marked",
        SuspectReason::ContentModified => "content_modified",
    }
}

/// Mark a specific link as suspect in an entity file.
///
/// Preserves existing fields like `title` and `verified_revision`. Handles
/// both array links and single-value links.
pub fn mark_link_suspect(
    file_path: &Path,
    link_type: &str,
    target_id: &str,
    reason: SuspectReason,
) -> Result<(), SuspectError> {
    let contents = std::fs::read_to_string(file_path)?;

    let mut doc: serde_yml::Value =
        serde_yml::from_str(&contents).map_err(|e| SuspectError::YamlError {
            message: e.to_string(),
        })?;

    let mut modified = false;
    if let Some(links) = doc.get_mut("links") {
        if let Some(links_map) = links.as_mapping_mut() {
            let link_type_key = serde_yml::Value::String(link_type.to_string());
            if let Some(link_values) = links_map.get_mut(&link_type_key) {
                modified = apply_suspect_to_value(link_values, target_id, &reason);
            }
            // The cache normalizes some YAML field names to canonical link
            // types (components/children → contains, parent → contained_in,
            // created_ncr → ncrs, ...). If the aliased name found nothing,
            // scan every links field for the target so those links still get
            // flagged.
            if !modified {
                for (_, link_values) in links_map.iter_mut() {
                    if apply_suspect_to_value(link_values, target_id, &reason) {
                        modified = true;
                        break;
                    }
                }
            }
        }
    }

    if !modified {
        return Err(SuspectError::LinkNotFound {
            from: file_path.display().to_string(),
            to: target_id.to_string(),
        });
    }

    let new_contents = serde_yml::to_string(&doc).map_err(|e| SuspectError::YamlError {
        message: e.to_string(),
    })?;

    std::fs::write(file_path, new_contents)?;
    Ok(())
}

/// Result of marking a dependent's link as suspect.
#[derive(Debug, Clone)]
pub struct MarkedDependent {
    /// The source entity that has the now-suspect link
    pub source_id: String,
    /// The link type field on the source
    pub link_type: String,
    /// The path to the source entity file
    pub source_path: PathBuf,
    /// The reason the link was marked suspect
    pub reason: SuspectReason,
}

/// Mark all incoming links to `target_id` as suspect across the project.
///
/// Queries the cache for entities that link TO `target_id` and marks each
/// link as suspect with the given reason. Returns a list of all marks made.
///
/// This is the core change-impact-analysis primitive: when an entity is
/// modified, downstream artifacts that reference it should be flagged for
/// review.
pub fn mark_dependents_suspect(
    project: &Project,
    cache: &EntityCache,
    target_id: &str,
    reason: SuspectReason,
) -> Result<Vec<MarkedDependent>, SuspectError> {
    let incoming = cache.get_links_to(target_id);
    let mut marked = Vec::new();

    for link in incoming {
        // Look up the source entity to get its file path
        let source_entity = match cache.get_entity(&link.source_id) {
            Some(e) => e,
            None => continue,
        };

        // Resolve relative path
        let source_path = if source_entity.file_path.is_absolute() {
            source_entity.file_path.clone()
        } else {
            project.root().join(&source_entity.file_path)
        };

        if !source_path.exists() {
            continue;
        }

        // Mark the link suspect; ignore LinkNotFound (cache may be stale)
        match mark_link_suspect(&source_path, &link.link_type, target_id, reason.clone()) {
            Ok(()) => {
                marked.push(MarkedDependent {
                    source_id: link.source_id.clone(),
                    link_type: link.link_type.clone(),
                    source_path,
                    reason: reason.clone(),
                });
            }
            Err(SuspectError::LinkNotFound { .. }) => {
                // Cache says the link exists but the YAML doesn't - skip silently
            }
            Err(e) => return Err(e),
        }
    }

    Ok(marked)
}

/// Apply suspect status to a link value that may be a sequence of entries or
/// a single entry. Returns true if any entry matched and was modified.
fn apply_suspect_to_value(
    link_values: &mut serde_yml::Value,
    target_id: &str,
    reason: &SuspectReason,
) -> bool {
    if let Some(seq) = link_values.as_sequence_mut() {
        for link in seq.iter_mut() {
            if apply_suspect_to_entry(link, target_id, reason) {
                return true;
            }
        }
        false
    } else {
        apply_suspect_to_entry(link_values, target_id, reason)
    }
}

/// Clear suspect status from a single link entry, optionally recording the
/// reviewed revision. Preserves existing fields like `title`. Returns true if
/// the entry was modified.
fn apply_clear_to_entry(
    entry: &mut serde_yml::Value,
    target_id: &str,
    verified_revision: Option<u32>,
) -> bool {
    let map = match entry {
        serde_yml::Value::Mapping(m) => m,
        _ => return false,
    };

    let id_matches = map
        .get(serde_yml::Value::String("id".to_string()))
        .and_then(|v| v.as_str())
        .map(|id| id == target_id)
        .unwrap_or(false);

    if !id_matches {
        return false;
    }

    map.remove(serde_yml::Value::String("suspect".to_string()));
    map.remove(serde_yml::Value::String("suspect_reason".to_string()));
    map.remove(serde_yml::Value::String("suspect_since".to_string()));

    if let Some(rev) = verified_revision {
        map.insert(
            serde_yml::Value::String("verified_revision".to_string()),
            serde_yml::Value::Number(rev.into()),
        );
    }

    true
}

/// Clear suspect status for a specific link.
///
/// Handles both array links and single-value links. Preserves all other
/// fields on the link entry (title, etc.).
pub fn clear_link_suspect(
    file_path: &Path,
    link_type: &str,
    target_id: &str,
    verified_revision: Option<u32>,
) -> Result<(), SuspectError> {
    let contents = std::fs::read_to_string(file_path)?;

    let mut doc: serde_yml::Value =
        serde_yml::from_str(&contents).map_err(|e| SuspectError::YamlError {
            message: e.to_string(),
        })?;

    let mut modified = false;
    if let Some(links) = doc.get_mut("links") {
        if let Some(links_map) = links.as_mapping_mut() {
            let link_type_key = serde_yml::Value::String(link_type.to_string());
            if let Some(link_values) = links_map.get_mut(&link_type_key) {
                modified = apply_clear_to_value(link_values, target_id, verified_revision);
            }
            // Alias fallback, mirroring mark_link_suspect: the caller may pass
            // a cache-normalized link type that doesn't match the YAML field.
            if !modified {
                for (_, link_values) in links_map.iter_mut() {
                    if apply_clear_to_value(link_values, target_id, verified_revision) {
                        modified = true;
                        break;
                    }
                }
            }
        }
    }

    // Nothing matched: report it rather than pretending success (and don't
    // rewrite/renormalize a file we didn't change).
    if !modified {
        return Err(SuspectError::LinkNotFound {
            from: file_path.display().to_string(),
            to: target_id.to_string(),
        });
    }

    let new_contents = serde_yml::to_string(&doc).map_err(|e| SuspectError::YamlError {
        message: e.to_string(),
    })?;

    std::fs::write(file_path, new_contents)?;
    Ok(())
}

/// Clear suspect status on a link value that may be a sequence of entries or
/// a single entry. Returns true if any entry matched and was modified.
fn apply_clear_to_value(
    link_values: &mut serde_yml::Value,
    target_id: &str,
    verified_revision: Option<u32>,
) -> bool {
    if let Some(seq) = link_values.as_sequence_mut() {
        for link in seq.iter_mut() {
            if apply_clear_to_entry(link, target_id, verified_revision) {
                return true;
            }
        }
        false
    } else {
        apply_clear_to_entry(link_values, target_id, verified_revision)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_link_ref_simple() {
        let link: LinkRef = "TEST-01ABC".into();
        assert_eq!(link.id(), "TEST-01ABC");
        assert!(!link.is_suspect());
    }

    #[test]
    fn test_link_ref_extended() {
        let mut ext = ExtendedLinkRef::new("TEST-01ABC");
        ext.mark_suspect(SuspectReason::RevisionChanged);

        let link = LinkRef::Extended(ext);
        assert_eq!(link.id(), "TEST-01ABC");
        assert!(link.is_suspect());
        assert_eq!(link.suspect_reason(), Some(&SuspectReason::RevisionChanged));
    }

    #[test]
    fn test_mark_simple_link_suspect() {
        let mut link: LinkRef = "TEST-01ABC".into();
        link.mark_suspect(SuspectReason::StatusRegressed);

        assert!(link.is_suspect());
        assert_eq!(link.suspect_reason(), Some(&SuspectReason::StatusRegressed));
    }

    #[test]
    fn test_clear_suspect() {
        let mut ext = ExtendedLinkRef::new("TEST-01ABC");
        ext.mark_suspect(SuspectReason::RevisionChanged);
        ext.clear_suspect(Some(2));

        assert!(!ext.suspect);
        assert!(ext.suspect_reason.is_none());
        assert_eq!(ext.verified_revision, Some(2));
    }

    #[test]
    fn test_has_suspect_links() {
        let tmp = tempdir().unwrap();
        let file = tmp.path().join("test.yaml");

        std::fs::write(
            &file,
            r#"id: REQ-TEST
title: Test Requirement
links:
  verified_by:
    - id: TEST-01ABC
      suspect: true
      suspect_reason: revision_changed
"#,
        )
        .unwrap();

        assert!(has_suspect_links(&file).unwrap());
    }

    #[test]
    fn test_has_no_suspect_links() {
        let tmp = tempdir().unwrap();
        let file = tmp.path().join("test.yaml");

        std::fs::write(
            &file,
            r#"id: REQ-TEST
title: Test Requirement
links:
  verified_by:
    - TEST-01ABC
    - TEST-02DEF
"#,
        )
        .unwrap();

        assert!(!has_suspect_links(&file).unwrap());
    }

    #[test]
    fn test_get_suspect_links() {
        let tmp = tempdir().unwrap();
        let file = tmp.path().join("test.yaml");

        std::fs::write(
            &file,
            r#"id: REQ-TEST
title: Test Requirement
links:
  verified_by:
    - TEST-01ABC
    - id: TEST-02DEF
      suspect: true
      suspect_reason: revision_changed
"#,
        )
        .unwrap();

        let suspect = get_suspect_links(&file).unwrap();
        assert_eq!(suspect.len(), 1);
        assert_eq!(suspect[0].0, "verified_by");
        assert_eq!(suspect[0].1, "TEST-02DEF");
        assert_eq!(suspect[0].2, SuspectReason::RevisionChanged);
    }

    #[test]
    fn test_mark_and_clear_suspect() {
        let tmp = tempdir().unwrap();
        let file = tmp.path().join("test.yaml");

        std::fs::write(
            &file,
            r#"id: REQ-TEST
title: Test Requirement
links:
  verified_by:
    - TEST-01ABC
"#,
        )
        .unwrap();

        // Mark as suspect
        mark_link_suspect(
            &file,
            "verified_by",
            "TEST-01ABC",
            SuspectReason::RevisionChanged,
        )
        .unwrap();
        assert!(has_suspect_links(&file).unwrap());

        // Clear suspect
        clear_link_suspect(&file, "verified_by", "TEST-01ABC", Some(2)).unwrap();
        assert!(!has_suspect_links(&file).unwrap());

        // Verify the verified_revision was set
        let contents = std::fs::read_to_string(&file).unwrap();
        assert!(contents.contains("verified_revision: 2"));
    }

    #[test]
    fn test_suspect_reason_display() {
        assert_eq!(
            SuspectReason::RevisionChanged.to_string(),
            "revision changed"
        );
        assert_eq!(
            SuspectReason::StatusRegressed.to_string(),
            "status regressed"
        );
        assert_eq!(SuspectReason::ManuallyMarked.to_string(), "manually marked");
        assert_eq!(
            SuspectReason::ContentModified.to_string(),
            "content modified"
        );
    }

    #[test]
    fn test_link_ref_serde_roundtrip() {
        // Simple link
        let simple: LinkRef = "TEST-01ABC".into();
        let yaml = serde_yml::to_string(&simple).unwrap();
        assert!(yaml.trim() == "TEST-01ABC");

        // Extended link
        let mut ext = ExtendedLinkRef::new("TEST-01ABC");
        ext.mark_suspect(SuspectReason::RevisionChanged);
        let extended = LinkRef::Extended(ext);
        let yaml = serde_yml::to_string(&extended).unwrap();
        assert!(yaml.contains("suspect: true"));
    }

    #[test]
    fn test_mark_suspect_preserves_title() {
        let tmp = tempdir().unwrap();
        let file = tmp.path().join("test.yaml");

        std::fs::write(
            &file,
            r#"id: REQ-TEST
title: Test Requirement
links:
  verified_by:
    - id: TEST-01ABC
      title: Important Test
"#,
        )
        .unwrap();

        mark_link_suspect(
            &file,
            "verified_by",
            "TEST-01ABC",
            SuspectReason::RevisionChanged,
        )
        .unwrap();

        let contents = std::fs::read_to_string(&file).unwrap();
        assert!(contents.contains("title: Important Test"));
        assert!(contents.contains("suspect: true"));
        assert!(contents.contains("revision_changed"));
    }

    #[test]
    fn test_mark_suspect_preserves_verified_revision() {
        let tmp = tempdir().unwrap();
        let file = tmp.path().join("test.yaml");

        std::fs::write(
            &file,
            r#"id: REQ-TEST
title: Test Requirement
links:
  verified_by:
    - id: TEST-01ABC
      title: Important Test
      verified_revision: 5
"#,
        )
        .unwrap();

        mark_link_suspect(
            &file,
            "verified_by",
            "TEST-01ABC",
            SuspectReason::RevisionChanged,
        )
        .unwrap();

        let contents = std::fs::read_to_string(&file).unwrap();
        assert!(contents.contains("title: Important Test"));
        assert!(contents.contains("verified_revision: 5"));
        assert!(contents.contains("suspect: true"));
    }

    #[test]
    fn test_mark_and_clear_single_value_link() {
        let tmp = tempdir().unwrap();
        let file = tmp.path().join("risk.yaml");

        // Single-value link (component) on a Risk entity
        std::fs::write(
            &file,
            r#"id: RISK-TEST
title: Test Risk
links:
  component:
    id: CMP-01ABC
    title: Housing
"#,
        )
        .unwrap();

        // Mark as suspect
        mark_link_suspect(
            &file,
            "component",
            "CMP-01ABC",
            SuspectReason::ContentModified,
        )
        .unwrap();

        assert!(has_suspect_links(&file).unwrap());
        let contents = std::fs::read_to_string(&file).unwrap();
        assert!(contents.contains("title: Housing"));
        assert!(contents.contains("suspect: true"));

        // Clear suspect
        clear_link_suspect(&file, "component", "CMP-01ABC", Some(3)).unwrap();
        assert!(!has_suspect_links(&file).unwrap());

        let contents = std::fs::read_to_string(&file).unwrap();
        assert!(contents.contains("title: Housing"));
        assert!(contents.contains("verified_revision: 3"));
    }

    #[test]
    fn test_clear_suspect_preserves_title() {
        let tmp = tempdir().unwrap();
        let file = tmp.path().join("test.yaml");

        std::fs::write(
            &file,
            r#"id: REQ-TEST
title: Test Requirement
links:
  verified_by:
    - id: TEST-01ABC
      title: Important Test
      suspect: true
      suspect_reason: revision_changed
      suspect_since: 2025-01-01T00:00:00Z
"#,
        )
        .unwrap();

        clear_link_suspect(&file, "verified_by", "TEST-01ABC", Some(7)).unwrap();

        let contents = std::fs::read_to_string(&file).unwrap();
        assert!(contents.contains("title: Important Test"));
        assert!(contents.contains("verified_revision: 7"));
        assert!(!contents.contains("suspect:"));
    }

    #[test]
    fn test_get_suspect_links_single_value() {
        let tmp = tempdir().unwrap();
        let file = tmp.path().join("risk.yaml");

        std::fs::write(
            &file,
            r#"id: RISK-TEST
title: Test Risk
links:
  component:
    id: CMP-01ABC
    suspect: true
    suspect_reason: revision_changed
"#,
        )
        .unwrap();

        let suspect = get_suspect_links(&file).unwrap();
        assert_eq!(suspect.len(), 1);
        assert_eq!(suspect[0].0, "component");
        assert_eq!(suspect[0].1, "CMP-01ABC");
        assert_eq!(suspect[0].2, SuspectReason::RevisionChanged);
    }

    #[test]
    fn test_mark_suspect_returns_error_when_link_not_found() {
        let tmp = tempdir().unwrap();
        let file = tmp.path().join("test.yaml");

        std::fs::write(
            &file,
            r#"id: REQ-TEST
title: Test Requirement
links:
  verified_by:
    - TEST-01ABC
"#,
        )
        .unwrap();

        let result = mark_link_suspect(
            &file,
            "verified_by",
            "TEST-NONEXISTENT",
            SuspectReason::RevisionChanged,
        );
        assert!(matches!(result, Err(SuspectError::LinkNotFound { .. })));
    }
}
