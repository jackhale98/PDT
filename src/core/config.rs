//! Configuration management with layered hierarchy

use serde::Deserialize;
use std::path::PathBuf;

use crate::core::Project;

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

    /// Get the path to the global config file
    fn global_config_path() -> Option<PathBuf> {
        directories::ProjectDirs::from("", "", "tdt")
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
