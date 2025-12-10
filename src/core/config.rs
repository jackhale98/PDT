//! Configuration management with layered hierarchy

use serde::Deserialize;
use std::path::PathBuf;

use crate::core::Project;

/// PDT configuration with layered hierarchy
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Default author for new entities
    pub author: Option<String>,

    /// Editor command for `pdt edit`
    pub editor: Option<String>,

    /// Pager command for long output
    pub pager: Option<String>,

    /// Default output format
    pub default_format: Option<String>,
}

impl Config {
    /// Load configuration from all sources, merging in priority order
    pub fn load() -> Self {
        let mut config = Config::default();

        // 1. Built-in defaults (already in Default impl)

        // 2. Global user config (~/.config/pdt/config.yaml)
        if let Some(global_path) = Self::global_config_path() {
            if global_path.exists() {
                if let Ok(contents) = std::fs::read_to_string(&global_path) {
                    if let Ok(global) = serde_yml::from_str::<Config>(&contents) {
                        config.merge(global);
                    }
                }
            }
        }

        // 3. Project config (.pdt/config.yaml)
        if let Ok(project) = Project::discover() {
            let project_config_path = project.pdt_dir().join("config.yaml");
            if project_config_path.exists() {
                if let Ok(contents) = std::fs::read_to_string(&project_config_path) {
                    if let Ok(project_config) = serde_yml::from_str::<Config>(&contents) {
                        config.merge(project_config);
                    }
                }
            }
        }

        // 4. Environment variables
        if let Ok(author) = std::env::var("PDT_AUTHOR") {
            config.author = Some(author);
        }
        if let Ok(editor) = std::env::var("PDT_EDITOR") {
            config.editor = Some(editor);
        }

        config
    }

    /// Get the path to the global config file
    fn global_config_path() -> Option<PathBuf> {
        directories::ProjectDirs::from("", "", "pdt")
            .map(|dirs| dirs.config_dir().join("config.yaml"))
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
}
