//! Suspect link tracking for change impact analysis
//!
//! When an entity is modified (revision change or status regression),
//! its incoming links become "suspect" until reviewed and cleared.
//! This helps teams understand the impact of changes on traceability.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;

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

/// Check if an entity file has any suspect links
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
                        // Check if it's an extended link with suspect=true
                        if let Some(link_map) = link.as_mapping() {
                            if let Some(suspect) =
                                link_map.get(serde_yml::Value::String("suspect".to_string()))
                            {
                                if suspect.as_bool() == Some(true) {
                                    return Ok(true);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(false)
}

/// Get all suspect links from an entity file
pub fn get_suspect_links(
    file_path: &Path,
) -> Result<Vec<(String, String, SuspectReason)>, SuspectError> {
    let contents = std::fs::read_to_string(file_path)?;

    let doc: serde_yml::Value =
        serde_yml::from_str(&contents).map_err(|e| SuspectError::YamlError {
            message: e.to_string(),
        })?;

    let mut suspect_links = Vec::new();

    // Check the links section
    if let Some(links) = doc.get("links") {
        if let Some(links_map) = links.as_mapping() {
            for (link_type, link_values) in links_map {
                let link_type_str = link_type.as_str().unwrap_or("unknown").to_string();
                if let Some(seq) = link_values.as_sequence() {
                    for link in seq {
                        // Check if it's an extended link with suspect=true
                        if let Some(link_map) = link.as_mapping() {
                            let is_suspect = link_map
                                .get(serde_yml::Value::String("suspect".to_string()))
                                .and_then(|v| v.as_bool())
                                .unwrap_or(false);

                            if is_suspect {
                                let target_id = link_map
                                    .get(serde_yml::Value::String("id".to_string()))
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string();

                                let reason = link_map
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

                                suspect_links.push((link_type_str.clone(), target_id, reason));
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(suspect_links)
}

/// Mark a specific link as suspect in an entity file
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

    if let Some(links) = doc.get_mut("links") {
        if let Some(links_map) = links.as_mapping_mut() {
            let link_type_key = serde_yml::Value::String(link_type.to_string());
            if let Some(link_values) = links_map.get_mut(&link_type_key) {
                if let Some(seq) = link_values.as_sequence_mut() {
                    for link in seq.iter_mut() {
                        // Handle both simple and extended links
                        let matches = match link {
                            serde_yml::Value::String(id) => id == target_id,
                            serde_yml::Value::Mapping(map) => map
                                .get(serde_yml::Value::String("id".to_string()))
                                .and_then(|v| v.as_str())
                                .map(|id| id == target_id)
                                .unwrap_or(false),
                            _ => false,
                        };

                        if matches {
                            // Convert to extended format
                            let mut new_link = serde_yml::Mapping::new();
                            new_link.insert(
                                serde_yml::Value::String("id".to_string()),
                                serde_yml::Value::String(target_id.to_string()),
                            );
                            new_link.insert(
                                serde_yml::Value::String("suspect".to_string()),
                                serde_yml::Value::Bool(true),
                            );
                            new_link.insert(
                                serde_yml::Value::String("suspect_reason".to_string()),
                                serde_yml::Value::String(match reason {
                                    SuspectReason::RevisionChanged => {
                                        "revision_changed".to_string()
                                    }
                                    SuspectReason::StatusRegressed => {
                                        "status_regressed".to_string()
                                    }
                                    SuspectReason::ManuallyMarked => "manually_marked".to_string(),
                                    SuspectReason::ContentModified => {
                                        "content_modified".to_string()
                                    }
                                }),
                            );
                            new_link.insert(
                                serde_yml::Value::String("suspect_since".to_string()),
                                serde_yml::Value::String(Utc::now().to_rfc3339()),
                            );
                            *link = serde_yml::Value::Mapping(new_link);
                            break;
                        }
                    }
                }
            }
        }
    }

    let new_contents = serde_yml::to_string(&doc).map_err(|e| SuspectError::YamlError {
        message: e.to_string(),
    })?;

    std::fs::write(file_path, new_contents)?;
    Ok(())
}

/// Clear suspect status for a specific link
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

    if let Some(links) = doc.get_mut("links") {
        if let Some(links_map) = links.as_mapping_mut() {
            let link_type_key = serde_yml::Value::String(link_type.to_string());
            if let Some(link_values) = links_map.get_mut(&link_type_key) {
                if let Some(seq) = link_values.as_sequence_mut() {
                    for link in seq.iter_mut() {
                        if let serde_yml::Value::Mapping(map) = link {
                            let id_matches = map
                                .get(serde_yml::Value::String("id".to_string()))
                                .and_then(|v| v.as_str())
                                .map(|id| id == target_id)
                                .unwrap_or(false);

                            if id_matches {
                                // Clear suspect status
                                map.remove(serde_yml::Value::String("suspect".to_string()));
                                map.remove(serde_yml::Value::String("suspect_reason".to_string()));
                                map.remove(serde_yml::Value::String("suspect_since".to_string()));

                                // Add verified revision if provided
                                if let Some(rev) = verified_revision {
                                    map.insert(
                                        serde_yml::Value::String("verified_revision".to_string()),
                                        serde_yml::Value::Number(rev.into()),
                                    );
                                }
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    let new_contents = serde_yml::to_string(&doc).map_err(|e| SuspectError::YamlError {
        message: e.to_string(),
    })?;

    std::fs::write(file_path, new_contents)?;
    Ok(())
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
}
