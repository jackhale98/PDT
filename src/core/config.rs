//! Configuration management with layered hierarchy

use serde::Deserialize;
use std::path::PathBuf;

use crate::core::workflow::WorkflowConfig;
use crate::core::Project;

/// Manufacturing workflow configuration
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ManufacturingConfigSection {
    /// Whether to create git branches for lots
    #[serde(default)]
    pub lot_branch_enabled: bool,

    /// Base branch to create lot branches from
    pub base_branch: Option<String>,

    /// Pattern for lot branch names (e.g., "lot/{lot_number}")
    pub branch_pattern: Option<String>,

    /// Whether to create tags at lot lifecycle events
    #[serde(default = "default_create_tags")]
    pub create_tags: bool,

    /// Whether to sign commits
    #[serde(default)]
    pub sign_commits: bool,
}

fn default_create_tags() -> bool {
    true
}

/// TDT configuration with layered hierarchy
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Default author for new entities
    pub author: Option<String>,

    /// Editor command for `tdt edit`
    pub editor: Option<String>,

    /// Pager command for long output
    pub pager: Option<String>,

    /// Default output format
    pub default_format: Option<String>,

    /// Git workflow configuration (opt-in)
    pub workflow: WorkflowConfig,

    /// Manufacturing workflow configuration
    pub manufacturing: Option<ManufacturingConfigSection>,
}

impl Config {
    /// Load configuration from all sources, merging in priority order
    pub fn load() -> Self {
        let mut config = Config::default();

        // 1. Built-in defaults (already in Default impl)

        // 2. Global user config (~/.config/tdt/config.yaml)
        if let Some(global_path) = Self::global_config_path() {
            if global_path.exists() {
                if let Ok(contents) = std::fs::read_to_string(&global_path) {
                    if let Ok(global) = serde_yml::from_str::<Config>(&contents) {
                        config.merge(global);
                    }
                }
            }
        }

        // 3. Project config (.tdt/config.yaml)
        if let Ok(project) = Project::discover() {
            let project_config_path = project.tdt_dir().join("config.yaml");
            if project_config_path.exists() {
                if let Ok(contents) = std::fs::read_to_string(&project_config_path) {
                    if let Ok(project_config) = serde_yml::from_str::<Config>(&contents) {
                        config.merge(project_config);
                    }
                }
            }
        }

        // 4. Environment variables
        if let Ok(author) = std::env::var("TDT_AUTHOR") {
            config.author = Some(author);
        }
        if let Ok(editor) = std::env::var("TDT_EDITOR") {
            config.editor = Some(editor);
        }

        config
    }

    /// Merge another config into this one (other takes precedence)
    fn merge(&mut self, other: Config) {
        if other.author.is_some() {
            self.author = other.author;
        }
        if other.editor.is_some() {
            self.editor = other.editor;
        }
        if other.pager.is_some() {
            self.pager = other.pager;
        }
        if other.default_format.is_some() {
            self.default_format = other.default_format;
        }
        // Workflow config: merge if the other has it enabled
        if other.workflow.enabled {
            self.workflow = other.workflow;
        }
        // Manufacturing config: merge if present
        if other.manufacturing.is_some() {
            self.manufacturing = other.manufacturing;
        }
    }

    /// Get the path to the global config file (public for config command)
    pub fn global_config_path() -> Option<PathBuf> {
        directories::ProjectDirs::from("", "", "tdt")
            .map(|dirs| dirs.config_dir().join("config.yaml"))
    }

    /// Get the author name, falling back to git config or username
    pub fn author(&self) -> String {
        if let Some(ref author) = self.author {
            return author.clone();
        }

        // Try git config
        if let Ok(output) = std::process::Command::new("git")
            .args(["config", "user.name"])
            .output()
        {
            if output.status.success() {
                let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !name.is_empty() {
                    return name;
                }
            }
        }

        // Fall back to username
        std::env::var("USER")
            .or_else(|_| std::env::var("USERNAME"))
            .unwrap_or_else(|_| "unknown".to_string())
    }

    /// Get the editor command
    pub fn editor(&self) -> String {
        self.editor
            .clone()
            .or_else(|| std::env::var("EDITOR").ok())
            .or_else(|| std::env::var("VISUAL").ok())
            .unwrap_or_else(|| "vi".to_string())
    }

    /// Run the editor on a file, properly handling commands with arguments
    /// (e.g., "emacsclient -nw" or "code --wait")
    pub fn run_editor(
        &self,
        file_path: &std::path::Path,
    ) -> std::io::Result<std::process::ExitStatus> {
        let editor = self.editor();
        let parts: Vec<&str> = editor.split_whitespace().collect();

        if parts.is_empty() {
            return std::process::Command::new("vi").arg(file_path).status();
        }

        let cmd = parts[0];
        let args = &parts[1..];

        std::process::Command::new(cmd)
            .args(args)
            .arg(file_path)
            .status()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = Config::default();
        assert!(config.author.is_none());
        assert!(config.editor.is_none());
        assert!(config.pager.is_none());
        assert!(config.default_format.is_none());
        assert!(!config.workflow.enabled);
    }

    #[test]
    fn test_config_merge_overrides() {
        let mut base = Config {
            author: Some("base_author".to_string()),
            editor: Some("vim".to_string()),
            pager: Some("less".to_string()),
            default_format: Some("yaml".to_string()),
            workflow: WorkflowConfig::default(),
            manufacturing: None,
        };

        let other = Config {
            author: Some("new_author".to_string()),
            editor: None, // Should NOT override
            pager: Some("more".to_string()),
            default_format: None, // Should NOT override
            workflow: WorkflowConfig::default(),
            manufacturing: None,
        };

        base.merge(other);

        assert_eq!(base.author, Some("new_author".to_string()));
        assert_eq!(base.editor, Some("vim".to_string())); // Kept original
        assert_eq!(base.pager, Some("more".to_string()));
        assert_eq!(base.default_format, Some("yaml".to_string())); // Kept original
    }

    #[test]
    fn test_config_merge_empty_base() {
        let mut base = Config::default();

        let other = Config {
            author: Some("author".to_string()),
            editor: Some("emacs".to_string()),
            pager: None,
            default_format: Some("json".to_string()),
            workflow: WorkflowConfig::default(),
            manufacturing: None,
        };

        base.merge(other);

        assert_eq!(base.author, Some("author".to_string()));
        assert_eq!(base.editor, Some("emacs".to_string()));
        assert!(base.pager.is_none());
        assert_eq!(base.default_format, Some("json".to_string()));
    }

    #[test]
    fn test_config_author_explicit() {
        let config = Config {
            author: Some("explicit_author".to_string()),
            ..Default::default()
        };
        assert_eq!(config.author(), "explicit_author");
    }

    #[test]
    fn test_config_author_fallback() {
        // When no explicit author, should fallback to git config or username
        let config = Config::default();
        let author = config.author();
        // Should not be empty - will be git user.name or USER/USERNAME env
        assert!(!author.is_empty());
    }

    #[test]
    fn test_config_editor_explicit() {
        let config = Config {
            editor: Some("nano".to_string()),
            ..Default::default()
        };
        assert_eq!(config.editor(), "nano");
    }

    #[test]
    fn test_config_editor_fallback() {
        // When no explicit editor, should fallback to EDITOR, VISUAL, or vi
        let config = Config::default();
        let editor = config.editor();
        // Should return something (either from env or "vi")
        assert!(!editor.is_empty());
    }

    #[test]
    fn test_config_global_path_exists() {
        // Should return Some path (though file may not exist)
        let path = Config::global_config_path();
        assert!(path.is_some());
        let path = path.unwrap();
        assert!(path.to_string_lossy().contains("tdt"));
        assert!(path.to_string_lossy().contains("config.yaml"));
    }

    #[test]
    fn test_config_deserialize_yaml() {
        let yaml = r#"
author: test_author
editor: code --wait
pager: bat
default_format: json
workflow:
  enabled: false
"#;
        let config: Config = serde_yml::from_str(yaml).unwrap();
        assert_eq!(config.author, Some("test_author".to_string()));
        assert_eq!(config.editor, Some("code --wait".to_string()));
        assert_eq!(config.pager, Some("bat".to_string()));
        assert_eq!(config.default_format, Some("json".to_string()));
        assert!(!config.workflow.enabled);
    }

    #[test]
    fn test_config_deserialize_partial_yaml() {
        let yaml = r#"
author: partial_author
"#;
        let config: Config = serde_yml::from_str(yaml).unwrap();
        assert_eq!(config.author, Some("partial_author".to_string()));
        assert!(config.editor.is_none());
        assert!(config.pager.is_none());
        assert!(config.default_format.is_none());
    }

    #[test]
    fn test_config_deserialize_empty_yaml() {
        let yaml = "{}";
        let config: Config = serde_yml::from_str(yaml).unwrap();
        assert!(config.author.is_none());
        assert!(config.editor.is_none());
    }
}
