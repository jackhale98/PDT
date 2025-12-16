//! Entity loading utilities
//!
//! This module provides generic utilities for loading entities from the
//! filesystem, reducing boilerplate in command implementations.

use miette::{IntoDiagnostic, Result};
use serde::de::DeserializeOwned;
use std::fs;
use std::path::{Path, PathBuf};

/// Load all entities of type T from a directory
///
/// Scans the directory for .yaml files and deserializes them.
/// Files that fail to parse are silently skipped.
pub fn load_all<T: DeserializeOwned>(dir: &Path) -> Result<Vec<T>> {
    let mut entities = Vec::new();

    if !dir.exists() {
        return Ok(entities);
    }

    for entry in fs::read_dir(dir).into_diagnostic()? {
        let entry = entry.into_diagnostic()?;
        let path = entry.path();

        if path.extension().is_some_and(|e| e == "yaml") {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(entity) = serde_yml::from_str::<T>(&content) {
                    entities.push(entity);
                }
            }
        }
    }

    Ok(entities)
}

/// Find an entity file by ID (supports partial matching)
///
/// Searches for a file whose stem contains the given ID.
/// Returns the first match found.
pub fn find_entity_file(dir: &Path, id: &str) -> Option<PathBuf> {
    if !dir.exists() {
        return None;
    }

    for entry in fs::read_dir(dir).ok()? {
        let entry = entry.ok()?;
        let path = entry.path();

        if path.extension().is_some_and(|e| e == "yaml") {
            let filename = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            if filename.contains(id) || filename.starts_with(id) {
                return Some(path);
            }
        }
    }

    None
}

/// Load a single entity by ID
///
/// Searches for an entity file matching the ID and deserializes it.
/// Returns the path and entity if found.
pub fn load_entity<T: DeserializeOwned>(dir: &Path, id: &str) -> Result<Option<(PathBuf, T)>> {
    if let Some(path) = find_entity_file(dir, id) {
        let content = fs::read_to_string(&path).into_diagnostic()?;
        let entity: T = serde_yml::from_str(&content).into_diagnostic()?;
        return Ok(Some((path, entity)));
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_load_all_empty_dir() {
        let dir = tempdir().unwrap();
        let result: Result<Vec<serde_json::Value>> = load_all(dir.path());
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_load_all_nonexistent_dir() {
        let result: Result<Vec<serde_json::Value>> = load_all(Path::new("/nonexistent/path"));
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_find_entity_file_nonexistent() {
        let result = find_entity_file(Path::new("/nonexistent/path"), "TEST-123");
        assert!(result.is_none());
    }

    #[test]
    fn test_find_entity_file() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("REQ-01J123456789ABCDEF.yaml");
        fs::write(&file_path, "id: REQ-01J123456789ABCDEF").unwrap();

        let result = find_entity_file(dir.path(), "REQ-01J123456789ABCDEF");
        assert!(result.is_some());
        assert_eq!(result.unwrap(), file_path);
    }
}
