//! `tdt config` command - Configuration management
//!
//! Provides commands to view and modify TDT configuration.

use clap::Subcommand;
use console::style;
use miette::{IntoDiagnostic, Result};
use std::fs;
use std::path::PathBuf;

use crate::cli::GlobalOpts;
use crate::core::project::Project;
use crate::core::Config;

#[derive(Subcommand, Debug)]
pub enum ConfigCommands {
    /// Show current configuration values
    Show(ShowArgs),

    /// Set a configuration value
    Set(SetArgs),

    /// Unset (remove) a configuration value
    Unset(UnsetArgs),

    /// Show paths to configuration files
    Path(PathArgs),

    /// List all available configuration keys
    Keys,
}

#[derive(clap::Args, Debug)]
pub struct ShowArgs {
    /// Show only this key's value
    pub key: Option<String>,

    /// Show only project-level config
    #[arg(long = "project-only")]
    pub project_only: bool,

    /// Show only global (user) config
    #[arg(long = "global-only")]
    pub global_only: bool,
}

#[derive(clap::Args, Debug)]
pub struct SetArgs {
    /// Configuration key (e.g., author, editor)
    pub key: String,

    /// Value to set
    pub value: String,

    /// Set in global (user) config instead of project config
    #[arg(long, short = 'g')]
    pub global: bool,
}

#[derive(clap::Args, Debug)]
pub struct UnsetArgs {
    /// Configuration key to remove
    pub key: String,

    /// Remove from global (user) config instead of project config
    #[arg(long, short = 'g')]
    pub global: bool,
}

#[derive(clap::Args, Debug)]
pub struct PathArgs {
    /// Show only project config path
    #[arg(long = "project-only")]
    pub project_only: bool,

    /// Show only global config path
    #[arg(long = "global-only")]
    pub global_only: bool,
}

/// Valid configuration keys
const VALID_KEYS: &[(&str, &str)] = &[
    ("author", "Default author for new entities"),
    ("editor", "Editor command for `tdt edit`"),
    ("pager", "Pager command for long output"),
    (
        "default_format",
        "Default output format (yaml, json, tsv, etc.)",
    ),
    // Workflow configuration
    ("workflow.enabled", "Enable workflow commands (true/false)"),
    (
        "workflow.provider",
        "Git provider: github, gitlab, or none",
    ),
    (
        "workflow.auto_commit",
        "Auto-commit on status changes (true/false)",
    ),
    (
        "workflow.auto_merge",
        "Merge PR automatically on approval (true/false)",
    ),
    ("workflow.base_branch", "Target branch for PRs (e.g., main)"),
    // Default approval requirements
    (
        "workflow.default_approvals.min_approvals",
        "Default minimum approvals required (number)",
    ),
    (
        "workflow.default_approvals.require_unique_approvers",
        "Require different approvers (true/false)",
    ),
    (
        "workflow.default_approvals.require_signature",
        "Require GPG-signed approvals for Part 11 (true/false)",
    ),
    // Per-entity approval requirements (examples)
    (
        "workflow.approvals.RISK.min_approvals",
        "Min approvals for RISK entities",
    ),
    (
        "workflow.approvals.RISK.require_signature",
        "Require GPG signature for RISK approvals",
    ),
    (
        "workflow.approvals.REQ.min_approvals",
        "Min approvals for REQ entities",
    ),
];

/// Run a config subcommand
pub fn run(cmd: ConfigCommands, _global: &GlobalOpts) -> Result<()> {
    match cmd {
        ConfigCommands::Show(args) => run_show(args),
        ConfigCommands::Set(args) => run_set(args),
        ConfigCommands::Unset(args) => run_unset(args),
        ConfigCommands::Path(args) => run_path(args),
        ConfigCommands::Keys => run_keys(),
    }
}

fn run_show(args: ShowArgs) -> Result<()> {
    let config = Config::load();

    // If a specific key is requested, show just that value
    if let Some(key) = &args.key {
        let value = get_config_value(&config, key);
        if let Some(v) = value {
            println!("{}", v);
        } else {
            return Err(miette::miette!("Key '{}' is not set", key));
        }
        return Ok(());
    }

    // Show all config values
    if args.project_only && args.global_only {
        return Err(miette::miette!(
            "Cannot specify both --project-only and --global-only"
        ));
    }

    if args.project_only {
        show_project_config()?;
    } else if args.global_only {
        show_global_config()?;
    } else {
        // Show merged/effective config
        println!("{}", style("Effective Configuration").bold().underlined());
        println!();

        print_config_value("author", config.author.as_deref());
        print_config_value("editor", config.editor.as_deref());
        print_config_value("pager", config.pager.as_deref());
        print_config_value("default_format", config.default_format.as_deref());

        // Show source info
        println!();
        println!("{}", style("Config Sources (in priority order):").dim());
        println!("  1. Environment variables (TDT_AUTHOR, TDT_EDITOR)");
        println!("  2. Project config (.tdt/config.yaml)");
        println!("  3. Global config (~/.config/tdt/config.yaml)");
    }

    Ok(())
}

fn run_set(args: SetArgs) -> Result<()> {
    let config_path = if args.global {
        get_global_config_path()?
    } else {
        get_project_config_path()?
    };

    // Load existing config or create new
    let mut config_map: serde_yml::Value = if config_path.exists() {
        let content = fs::read_to_string(&config_path).into_diagnostic()?;
        let parsed: serde_yml::Value =
            serde_yml::from_str(&content).unwrap_or(serde_yml::Value::Mapping(Default::default()));
        // If the file was empty or null, use an empty mapping
        if parsed.is_null() {
            serde_yml::Value::Mapping(Default::default())
        } else {
            parsed
        }
    } else {
        serde_yml::Value::Mapping(Default::default())
    };

    // Set the value (handle nested keys like "aliases.r")
    set_nested_value(&mut config_map, &args.key, &args.value)?;

    // Ensure parent directory exists
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent).into_diagnostic()?;
    }

    // Write back
    let yaml = serde_yml::to_string(&config_map).into_diagnostic()?;
    fs::write(&config_path, yaml).into_diagnostic()?;

    let scope = if args.global { "global" } else { "project" };
    println!(
        "{} Set {} {} = {} in {} config",
        style("✓").green(),
        style(&args.key).cyan(),
        style("→").dim(),
        style(&args.value).yellow(),
        scope
    );

    Ok(())
}

fn run_unset(args: UnsetArgs) -> Result<()> {
    let config_path = if args.global {
        get_global_config_path()?
    } else {
        get_project_config_path()?
    };

    if !config_path.exists() {
        return Err(miette::miette!(
            "Config file does not exist: {}",
            config_path.display()
        ));
    }

    let content = fs::read_to_string(&config_path).into_diagnostic()?;
    let mut config_map: serde_yml::Value =
        serde_yml::from_str(&content).unwrap_or(serde_yml::Value::Mapping(Default::default()));

    // Remove the value (handle nested keys like "aliases.r")
    let removed = unset_nested_value(&mut config_map, &args.key)?;

    if !removed {
        return Err(miette::miette!("Key '{}' not found in config", args.key));
    }

    let yaml = serde_yml::to_string(&config_map).into_diagnostic()?;
    fs::write(&config_path, yaml).into_diagnostic()?;

    let scope = if args.global { "global" } else { "project" };
    println!(
        "{} Removed {} from {} config",
        style("✓").green(),
        style(&args.key).cyan(),
        scope
    );

    Ok(())
}

fn run_path(args: PathArgs) -> Result<()> {
    if args.project_only && args.global_only {
        return Err(miette::miette!(
            "Cannot specify both --project-only and --global-only"
        ));
    }

    if args.project_only {
        let path = get_project_config_path()?;
        println!("{}", path.display());
    } else if args.global_only {
        let path = get_global_config_path()?;
        println!("{}", path.display());
    } else {
        let global_path = get_global_config_path()?;
        let project_path = get_project_config_path();

        println!("{}", style("Configuration file paths:").bold());
        println!();
        println!("  {} {}", style("Global:").cyan(), global_path.display());
        if global_path.exists() {
            println!("         {}", style("(exists)").green());
        } else {
            println!("         {}", style("(not created)").dim());
        }

        if let Ok(path) = project_path {
            println!();
            println!("  {} {}", style("Project:").cyan(), path.display());
            if path.exists() {
                println!("          {}", style("(exists)").green());
            } else {
                println!("          {}", style("(not created)").dim());
            }
        } else {
            println!();
            println!(
                "  {} {}",
                style("Project:").cyan(),
                style("(not in a TDT project)").dim()
            );
        }
    }

    Ok(())
}

fn run_keys() -> Result<()> {
    println!("{}", style("Available configuration keys:").bold());
    println!();

    for (key, description) in VALID_KEYS {
        println!("  {:<20} {}", style(key).cyan(), style(description).dim());
    }

    println!();
    println!(
        "{}",
        style("Use 'tdt config set <key> <value>' to set a value.").dim()
    );

    Ok(())
}

// Helper functions

fn get_global_config_path() -> Result<PathBuf> {
    directories::ProjectDirs::from("", "", "tdt")
        .map(|dirs| dirs.config_dir().join("config.yaml"))
        .ok_or_else(|| miette::miette!("Could not determine global config directory"))
}

fn get_project_config_path() -> Result<PathBuf> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    Ok(project.tdt_dir().join("config.yaml"))
}

fn get_config_value(config: &Config, key: &str) -> Option<String> {
    match key {
        "author" => config.author.clone(),
        "editor" => config.editor.clone(),
        "pager" => config.pager.clone(),
        "default_format" => config.default_format.clone(),
        _ => None,
    }
}

fn print_config_value(key: &str, value: Option<&str>) {
    if let Some(v) = value {
        println!("  {}: {}", style(key).cyan(), style(v).yellow());
    } else {
        println!("  {}: {}", style(key).cyan(), style("(not set)").dim());
    }
}

fn show_project_config() -> Result<()> {
    let path = get_project_config_path()?;

    println!(
        "{} {}",
        style("Project config:").bold(),
        style(path.display()).dim()
    );
    println!();

    if path.exists() {
        let content = fs::read_to_string(&path).into_diagnostic()?;
        print!("{}", content);
    } else {
        println!("{}", style("(not created)").dim());
    }

    Ok(())
}

fn show_global_config() -> Result<()> {
    let path = get_global_config_path()?;

    println!(
        "{} {}",
        style("Global config:").bold(),
        style(path.display()).dim()
    );
    println!();

    if path.exists() {
        let content = fs::read_to_string(&path).into_diagnostic()?;
        print!("{}", content);
    } else {
        println!("{}", style("(not created)").dim());
    }

    Ok(())
}

fn set_nested_value(root: &mut serde_yml::Value, key: &str, value: &str) -> Result<()> {
    let parts: Vec<&str> = key.split('.').collect();

    let mut current = root;
    for (i, part) in parts.iter().enumerate() {
        if i == parts.len() - 1 {
            // Last part - set the value
            if let serde_yml::Value::Mapping(map) = current {
                map.insert(
                    serde_yml::Value::String(part.to_string()),
                    serde_yml::Value::String(value.to_string()),
                );
            }
        } else {
            // Intermediate part - navigate or create
            if let serde_yml::Value::Mapping(map) = current {
                let key = serde_yml::Value::String(part.to_string());
                if !map.contains_key(&key) {
                    map.insert(key.clone(), serde_yml::Value::Mapping(Default::default()));
                }
                current = map.get_mut(&key).unwrap();
            }
        }
    }

    Ok(())
}

fn unset_nested_value(root: &mut serde_yml::Value, key: &str) -> Result<bool> {
    let parts: Vec<&str> = key.split('.').collect();

    if parts.len() == 1 {
        // Simple key
        if let serde_yml::Value::Mapping(map) = root {
            let key = serde_yml::Value::String(parts[0].to_string());
            return Ok(map.remove(&key).is_some());
        }
    } else {
        // Nested key - navigate to parent and remove
        let mut current = root;
        for part in &parts[..parts.len() - 1] {
            if let serde_yml::Value::Mapping(map) = current {
                let key = serde_yml::Value::String(part.to_string());
                if let Some(next) = map.get_mut(&key) {
                    current = next;
                } else {
                    return Ok(false);
                }
            } else {
                return Ok(false);
            }
        }

        // Remove the final key
        if let serde_yml::Value::Mapping(map) = current {
            let key = serde_yml::Value::String(parts.last().unwrap().to_string());
            return Ok(map.remove(&key).is_some());
        }
    }

    Ok(false)
}
