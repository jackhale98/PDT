//! Project discovery and structure

use std::path::{Path, PathBuf};
use thiserror::Error;

use crate::core::identity::{EntityId, EntityPrefix};

/// Represents a PDT project
#[derive(Debug)]
pub struct Project {
    /// Root directory of the project (parent of .pdt/)
    root: PathBuf,
}

impl Project {
    /// Find project root by walking up from the current directory
    pub fn discover() -> Result<Self, ProjectError> {
        let current = std::env::current_dir()
            .map_err(|e| ProjectError::IoError(e.to_string()))?;
        Self::discover_from(&current)
    }

    /// Find project root by walking up from the given directory
    pub fn discover_from(start: &Path) -> Result<Self, ProjectError> {
        let mut current = start
            .canonicalize()
            .map_err(|e| ProjectError::IoError(e.to_string()))?;

        loop {
            let pdt_dir = current.join(".pdt");
            if pdt_dir.is_dir() {
                return Ok(Self { root: current });
            }

            if !current.pop() {
                return Err(ProjectError::NotFound {
                    searched_from: start.to_path_buf(),
                });
            }
        }
    }

    /// Create a new project structure at the given path
    pub fn init(path: &Path) -> Result<Self, ProjectError> {
        let root = path
            .canonicalize()
            .unwrap_or_else(|_| path.to_path_buf());

        let pdt_dir = root.join(".pdt");
        if pdt_dir.exists() {
            return Err(ProjectError::AlreadyExists(root.clone()));
        }

        // Create .pdt directory structure
        std::fs::create_dir_all(pdt_dir.join("schema"))
            .map_err(|e| ProjectError::IoError(e.to_string()))?;
        std::fs::create_dir_all(pdt_dir.join("templates"))
            .map_err(|e| ProjectError::IoError(e.to_string()))?;

        // Create default config
        let config_path = pdt_dir.join("config.yaml");
        std::fs::write(&config_path, Self::default_config())
            .map_err(|e| ProjectError::IoError(e.to_string()))?;

        // Create entity directories
        Self::create_entity_dirs(&root)?;

        Ok(Self { root })
    }

    /// Force initialization even if .pdt/ exists
    pub fn init_force(path: &Path) -> Result<Self, ProjectError> {
        let root = path
            .canonicalize()
            .unwrap_or_else(|_| path.to_path_buf());

        let pdt_dir = root.join(".pdt");

        // Create .pdt directory structure (overwrite if exists)
        std::fs::create_dir_all(pdt_dir.join("schema"))
            .map_err(|e| ProjectError::IoError(e.to_string()))?;
        std::fs::create_dir_all(pdt_dir.join("templates"))
            .map_err(|e| ProjectError::IoError(e.to_string()))?;

        // Create default config
        let config_path = pdt_dir.join("config.yaml");
        std::fs::write(&config_path, Self::default_config())
            .map_err(|e| ProjectError::IoError(e.to_string()))?;

        // Create entity directories
        Self::create_entity_dirs(&root)?;

        Ok(Self { root })
    }

    fn default_config() -> &'static str {
        r#"# PDT Project Configuration
# See https://pdt.dev/docs/config for all options

# Default author for new entities (can be overridden by global config)
# author: ""

# Editor to use for `pdt edit` commands (default: $EDITOR)
# editor: ""

# Default output format (auto, yaml, tsv, json, csv, md, id)
# default_format: auto
"#
    }

    fn create_entity_dirs(root: &Path) -> Result<(), ProjectError> {
        let dirs = [
            "requirements/inputs",
            "requirements/outputs",
            "risks/design",
            "risks/process",
            "bom/assemblies",
            "bom/components",
            "bom/quotes",
            "tolerances/features",
            "tolerances/mates",
            "tolerances/stackups",
            "verification/protocols",
            "verification/results",
            "validation/protocols",
            "validation/results",
            "manufacturing/processes",
            "manufacturing/controls",
        ];

        for dir in dirs {
            std::fs::create_dir_all(root.join(dir))
                .map_err(|e| ProjectError::IoError(e.to_string()))?;
        }

        Ok(())
    }

    /// Get the project root directory
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Get the .pdt configuration directory
    pub fn pdt_dir(&self) -> PathBuf {
        self.root.join(".pdt")
    }

    /// Get the path for a new entity file
    pub fn entity_path(&self, prefix: EntityPrefix, id: &EntityId) -> PathBuf {
        let subdir = Self::entity_directory(prefix);
        self.root
            .join(subdir)
            .join(format!("{}.pdt.yaml", id))
    }

    /// Get the directory for a given entity prefix
    pub fn entity_directory(prefix: EntityPrefix) -> &'static str {
        match prefix {
            EntityPrefix::Req => "requirements/inputs", // Default to inputs
            EntityPrefix::Risk => "risks/design",       // Default to design
            EntityPrefix::Test => "verification/protocols",
            EntityPrefix::Rslt => "verification/results",
            EntityPrefix::Tol => "tolerances/stackups",
            EntityPrefix::Mate => "tolerances/mates",
            EntityPrefix::Asm => "bom/assemblies",
            EntityPrefix::Cmp => "bom/components",
            EntityPrefix::Feat => "tolerances/features",
            EntityPrefix::Proc => "manufacturing/processes",
            EntityPrefix::Ctrl => "manufacturing/controls",
            EntityPrefix::Quot => "bom/quotes",
            EntityPrefix::Act => "actions",
        }
    }

    /// Get the directory for requirements of a specific type
    pub fn requirement_directory(&self, req_type: &str) -> PathBuf {
        match req_type {
            "input" | "inputs" => self.root.join("requirements/inputs"),
            "output" | "outputs" => self.root.join("requirements/outputs"),
            _ => self.root.join("requirements/inputs"),
        }
    }

    /// Get the directory for risks of a specific type
    pub fn risk_directory(&self, risk_type: &str) -> PathBuf {
        match risk_type {
            "design" => self.root.join("risks/design"),
            "process" => self.root.join("risks/process"),
            _ => self.root.join("risks/design"),
        }
    }

    /// Get the directory for tests of a specific type (verification or validation)
    pub fn test_directory(&self, test_type: &str) -> PathBuf {
        match test_type {
            "verification" => self.root.join("verification/protocols"),
            "validation" => self.root.join("validation/protocols"),
            _ => self.root.join("verification/protocols"),
        }
    }

    /// Get the directory for test results of a specific type
    pub fn result_directory(&self, test_type: &str) -> PathBuf {
        match test_type {
            "verification" => self.root.join("verification/results"),
            "validation" => self.root.join("validation/results"),
            _ => self.root.join("verification/results"),
        }
    }

    /// Iterate all entity files of a given prefix type
    pub fn iter_entity_files(&self, prefix: EntityPrefix) -> impl Iterator<Item = PathBuf> {
        let dir = self.root.join(Self::entity_directory(prefix));
        walkdir::WalkDir::new(dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| {
                e.path()
                    .to_string_lossy()
                    .ends_with(".pdt.yaml")
            })
            .map(|e| e.path().to_path_buf())
    }
}

/// Errors that can occur during project operations
#[derive(Debug, Error)]
pub enum ProjectError {
    #[error("not a PDT project (searched from {searched_from:?}). Run 'pdt init' to create one.")]
    NotFound { searched_from: PathBuf },

    #[error("PDT project already exists at {0:?}")]
    AlreadyExists(PathBuf),

    #[error("IO error: {0}")]
    IoError(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_project_init_creates_structure() {
        let tmp = tempdir().unwrap();
        let project = Project::init(tmp.path()).unwrap();

        assert!(project.pdt_dir().exists());
        assert!(project.pdt_dir().join("config.yaml").exists());
        assert!(project.pdt_dir().join("schema").is_dir());
        assert!(project.pdt_dir().join("templates").is_dir());
        assert!(project.root().join("requirements/inputs").is_dir());
        assert!(project.root().join("requirements/outputs").is_dir());
        assert!(project.root().join("risks/design").is_dir());
    }

    #[test]
    fn test_project_init_fails_if_exists() {
        let tmp = tempdir().unwrap();
        Project::init(tmp.path()).unwrap();

        let err = Project::init(tmp.path()).unwrap_err();
        assert!(matches!(err, ProjectError::AlreadyExists(_)));
    }

    #[test]
    fn test_project_discover_finds_pdt_dir() {
        let tmp = tempdir().unwrap();
        Project::init(tmp.path()).unwrap();

        // Create a subdirectory
        let subdir = tmp.path().join("some/nested/dir");
        std::fs::create_dir_all(&subdir).unwrap();

        // Discover from subdirectory should find root
        let project = Project::discover_from(&subdir).unwrap();
        assert_eq!(
            project.root().canonicalize().unwrap(),
            tmp.path().canonicalize().unwrap()
        );
    }

    #[test]
    fn test_project_discover_fails_without_pdt_dir() {
        let tmp = tempdir().unwrap();
        let err = Project::discover_from(tmp.path()).unwrap_err();
        assert!(matches!(err, ProjectError::NotFound { .. }));
    }
}
