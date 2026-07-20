//! Base service utilities for shared CRUD operations
//!
//! This module provides a `ServiceBase` struct with common functionality
//! used by all entity services, reducing code duplication.
//!
//! # Example
//!
//! ```ignore
//! use tdt_core::services::base::ServiceBase;
//!
//! let base = ServiceBase::new(&project, &cache);
//! let entity: Option<MyEntity> = base.get("MY-123", &my_dir)?;
//! ```

use std::fs;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::core::cache::EntityCache;
use crate::core::entity::Entity;
use crate::core::loader;
use crate::core::project::Project;

use super::common::{ServiceError, ServiceResult};

/// Base service providing shared CRUD utilities
///
/// ServiceBase holds references to the project and cache, providing
/// common operations that all entity services need.
pub struct ServiceBase<'a> {
    project: &'a Project,
    cache: &'a EntityCache,
}

impl<'a> ServiceBase<'a> {
    /// Create a new ServiceBase
    pub fn new(project: &'a Project, cache: &'a EntityCache) -> Self {
        Self { project, cache }
    }

    /// Get reference to the project
    pub fn project(&self) -> &Project {
        self.project
    }

    /// Get reference to the cache
    pub fn cache(&self) -> &EntityCache {
        self.cache
    }

    /// Resolve a potentially relative path to an absolute path
    ///
    /// The cache stores relative paths, so this helper converts them
    /// to absolute paths by joining with the project root.
    pub fn resolve_path(&self, path: &Path) -> PathBuf {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.project.root().join(path)
        }
    }

    /// Get an entity by ID from the cache or filesystem
    ///
    /// This method first checks the cache for the entity's file path,
    /// then loads the full entity from disk. If not found in cache,
    /// falls back to scanning the directory.
    ///
    /// # Arguments
    /// * `id` - The entity ID to look up
    /// * `dir` - The directory to search in (used for fallback)
    /// * `expected_prefix` - Expected entity prefix for cache validation
    pub fn get<T>(&self, id: &str, dir: &Path, expected_prefix: &str) -> ServiceResult<Option<T>>
    where
        T: Entity + DeserializeOwned,
    {
        // Try cache first for fast lookup
        if let Some(entity) = self.cache.get_entity(id) {
            if entity.prefix == expected_prefix {
                let full_path = self.resolve_path(&entity.file_path);
                if let Ok(item) = crate::yaml::parse_yaml_file::<T>(&full_path) {
                    return Ok(Some(item));
                }
            }
        }

        // Fallback to filesystem search if not in cache
        if let Some((_, item)) = loader::load_entity::<T>(dir, id)? {
            return Ok(Some(item));
        }

        Ok(None)
    }

    /// Get an entity by ID, returning an error if not found
    pub fn get_required<T>(&self, id: &str, dir: &Path, expected_prefix: &str) -> ServiceResult<T>
    where
        T: Entity + DeserializeOwned,
    {
        self.get(id, dir, expected_prefix)?
            .ok_or_else(|| ServiceError::NotFound(id.to_string()))
    }

    /// Find an entity and its file path
    ///
    /// Searches for an entity in the given directory and returns
    /// both the file path and the loaded entity.
    pub fn find_entity<T>(&self, id: &str, dir: &Path) -> ServiceResult<(PathBuf, T)>
    where
        T: Entity + DeserializeOwned,
    {
        if let Some((path, entity)) = loader::load_entity::<T>(dir, id)? {
            return Ok((path, entity));
        }

        Err(ServiceError::NotFound(id.to_string()))
    }

    /// Find an entity in multiple directories
    ///
    /// Searches through multiple directories in order and returns
    /// the first match found.
    pub fn find_entity_in_dirs<T>(&self, id: &str, dirs: &[PathBuf]) -> ServiceResult<(PathBuf, T)>
    where
        T: Entity + DeserializeOwned,
    {
        for dir in dirs {
            if let Some((path, entity)) = loader::load_entity::<T>(dir, id)? {
                return Ok((path, entity));
            }
        }

        Err(ServiceError::NotFound(id.to_string()))
    }

    /// Save an entity to a file
    ///
    /// Writes the entity as YAML to the specified path, ensuring
    /// the parent directory exists.
    ///
    /// When `prefix` is `Some("REQ")` (or any entity prefix), this is a
    /// **creation** save: the YAML is enhanced with inline enum hints,
    /// TODO placeholders, and commented-out optional fields.
    ///
    /// When `prefix` is `None`, this is an **update** save: the entity is
    /// written as plain YAML without any template modifications, preserving
    /// user data exactly as-is.
    pub fn save<T>(&self, entity: &T, path: &Path, prefix: Option<&str>) -> ServiceResult<()>
    where
        T: Serialize,
    {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Serialize to YAML
        let yaml = serde_yml::to_string(entity).map_err(|e| ServiceError::Yaml(e.to_string()))?;

        // Convert long text fields to block scalars for readability and clean git diffs
        let yaml = crate::yaml::template::to_block_scalars(&yaml);

        // For creation saves, enhance with template comments; for updates, write as-is
        let yaml = match prefix {
            Some(p) => crate::yaml::template::enhance_new_entity_yaml(&yaml, p),
            None => yaml,
        };

        fs::write(path, yaml)?;

        Ok(())
    }

    /// Delete an entity file
    ///
    /// If `force` is false, checks for references before deleting.
    /// Currently reference checking uses the cache link table.
    pub fn delete(&self, id: &str, path: &Path, force: bool) -> ServiceResult<()> {
        // Check for references unless force is true
        if !force {
            let links_to = self.cache.get_links_to(id);
            if !links_to.is_empty() {
                return Err(ServiceError::HasReferences);
            }
        }

        // Delete the file
        fs::remove_file(path)?;

        Ok(())
    }

    /// Load all entities from a directory
    pub fn load_all<T>(&self, dir: &Path) -> ServiceResult<Vec<T>>
    where
        T: Entity + DeserializeOwned,
    {
        if !dir.exists() {
            return Ok(Vec::new());
        }

        Ok(loader::load_all(dir)?)
    }

    /// Load all entities from multiple directories
    pub fn load_all_from_dirs<T>(&self, dirs: &[PathBuf]) -> ServiceResult<Vec<T>>
    where
        T: Entity + DeserializeOwned,
    {
        let mut all = Vec::new();
        for dir in dirs {
            if dir.exists() {
                let loaded: Vec<T> = loader::load_all(dir)?;
                all.extend(loaded);
            }
        }
        Ok(all)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::entity::Status;
    use crate::core::identity::{EntityId, EntityPrefix};
    use crate::entities::requirement::{Level, Links, Requirement, RequirementType};
    use chrono::Utc;
    use std::fs;
    use tempfile::TempDir;

    fn setup_test_project() -> (TempDir, Project, EntityCache) {
        let tmp = TempDir::new().unwrap();

        // Create project structure
        fs::create_dir_all(tmp.path().join(".tdt")).unwrap();
        fs::create_dir_all(tmp.path().join("requirements/inputs")).unwrap();
        fs::create_dir_all(tmp.path().join("requirements/outputs")).unwrap();

        // Create config file
        fs::write(tmp.path().join(".tdt/config.yaml"), "author: Test Author\n").unwrap();

        let project = Project::discover_from(tmp.path()).unwrap();
        let cache = EntityCache::open(&project).unwrap();

        (tmp, project, cache)
    }

    fn create_test_requirement(id: &str, title: &str) -> Requirement {
        Requirement {
            id: id.parse().unwrap(),
            req_type: RequirementType::Input,
            level: Level::System,
            title: title.to_string(),
            text: "Test text".to_string(),
            author: "Test".to_string(),
            priority: crate::core::entity::Priority::Medium,
            status: Status::Draft,
            category: None,
            tags: vec![],
            rationale: None,
            acceptance_criteria: vec![],
            source: None,
            links: Links::default(),
            created: Utc::now(),
            revision: 1,
        }
    }

    #[test]
    fn test_resolve_path_absolute() {
        let (_tmp, project, cache) = setup_test_project();
        let base = ServiceBase::new(&project, &cache);

        let abs_path = PathBuf::from("/absolute/path/to/file.yaml");
        let result = base.resolve_path(&abs_path);
        assert_eq!(result, abs_path);
    }

    #[test]
    fn test_resolve_path_relative() {
        let (_tmp, project, cache) = setup_test_project();
        let base = ServiceBase::new(&project, &cache);

        let rel_path = PathBuf::from("requirements/inputs/REQ-123.tdt.yaml");
        let result = base.resolve_path(&rel_path);
        assert_eq!(result, project.root().join(&rel_path));
    }

    #[test]
    fn test_save_and_get() {
        let (tmp, project, cache) = setup_test_project();
        let base = ServiceBase::new(&project, &cache);

        // Create a requirement
        let id = EntityId::new(EntityPrefix::Req);
        let req = Requirement {
            id: id.clone(),
            req_type: RequirementType::Input,
            level: Level::System,
            title: "Test Requirement".to_string(),
            text: "Test text".to_string(),
            author: "Test".to_string(),
            priority: crate::core::entity::Priority::Medium,
            status: Status::Draft,
            category: None,
            tags: vec![],
            rationale: None,
            acceptance_criteria: vec![],
            source: None,
            links: Links::default(),
            created: Utc::now(),
            revision: 1,
        };

        // Save it
        let dir = tmp.path().join("requirements/inputs");
        let path = dir.join(format!("{}.tdt.yaml", id));
        base.save(&req, &path, None).unwrap();

        // Verify file exists
        assert!(path.exists());

        // Load it back (via filesystem, not cache since cache wasn't synced)
        let loaded: Option<Requirement> = base.get(&id.to_string(), &dir, "REQ").unwrap();
        assert!(loaded.is_some());
        let loaded = loaded.unwrap();
        assert_eq!(loaded.title, "Test Requirement");
    }

    #[test]
    fn test_find_entity_not_found() {
        let (tmp, project, cache) = setup_test_project();
        let base = ServiceBase::new(&project, &cache);

        let dir = tmp.path().join("requirements/inputs");
        let result: Result<(PathBuf, Requirement), _> = base.find_entity("REQ-NONEXISTENT", &dir);

        assert!(result.is_err());
        match result.unwrap_err() {
            ServiceError::NotFound(id) => assert_eq!(id, "REQ-NONEXISTENT"),
            _ => panic!("Expected NotFound error"),
        }
    }

    #[test]
    fn test_delete_with_no_references() {
        let (tmp, project, cache) = setup_test_project();
        let base = ServiceBase::new(&project, &cache);

        // Create and save a requirement
        let id = EntityId::new(EntityPrefix::Req);
        let req = create_test_requirement(&id.to_string(), "To Delete");

        let dir = tmp.path().join("requirements/inputs");
        let path = dir.join(format!("{}.tdt.yaml", id));
        base.save(&req, &path, None).unwrap();

        // Delete it
        base.delete(&id.to_string(), &path, false).unwrap();

        // Verify file is gone
        assert!(!path.exists());
    }

    #[test]
    fn test_load_all_empty_dir() {
        let (_tmp, project, cache) = setup_test_project();
        let base = ServiceBase::new(&project, &cache);

        let dir = project.root().join("requirements/inputs");
        let loaded: Vec<Requirement> = base.load_all(&dir).unwrap();
        assert!(loaded.is_empty());
    }

    #[test]
    fn test_load_all_with_entities() {
        let (tmp, project, cache) = setup_test_project();
        let base = ServiceBase::new(&project, &cache);

        let dir = tmp.path().join("requirements/inputs");

        // Create a few requirements
        for i in 1..=3 {
            let id = EntityId::new(EntityPrefix::Req);
            let req = create_test_requirement(&id.to_string(), &format!("Req {}", i));
            let path = dir.join(format!("{}.tdt.yaml", id));
            base.save(&req, &path, None).unwrap();
        }

        // Load all
        let loaded: Vec<Requirement> = base.load_all(&dir).unwrap();
        assert_eq!(loaded.len(), 3);
    }

    #[test]
    fn test_find_entity_in_dirs() {
        let (tmp, project, cache) = setup_test_project();
        let base = ServiceBase::new(&project, &cache);

        let inputs_dir = tmp.path().join("requirements/inputs");
        let outputs_dir = tmp.path().join("requirements/outputs");

        // Create a requirement in outputs
        let id = EntityId::new(EntityPrefix::Req);
        let req = create_test_requirement(&id.to_string(), "Output Req");
        let path = outputs_dir.join(format!("{}.tdt.yaml", id));
        base.save(&req, &path, None).unwrap();

        // Find it by searching both directories
        let dirs = vec![inputs_dir, outputs_dir];
        let (found_path, found): (PathBuf, Requirement) =
            base.find_entity_in_dirs(&id.to_string(), &dirs).unwrap();

        assert_eq!(found_path, path);
        assert_eq!(found.title, "Output Req");
    }
}
