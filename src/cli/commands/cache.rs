//! `tdt cache` command - Manage the entity cache
//!
//! The cache is a local SQLite database that stores:
//! - Short ID mappings (PREFIX@N -> full entity ID)
//! - Entity metadata for fast lookups
//! - Feature dimensions for tolerance analysis
//!
//! The cache is user-local (gitignored) and auto-rebuilds from entity files.

use clap::Subcommand;
use console::style;
use miette::Result;

use crate::cli::args::OutputFormat;
use crate::core::cache::EntityCache;
use crate::core::project::Project;

#[derive(Subcommand, Debug)]
pub enum CacheCommands {
    /// Rebuild the cache from scratch
    Rebuild,

    /// Sync cache with filesystem changes (incremental)
    Sync,

    /// Show cache statistics
    Status,

    /// Execute SQL query against cache (read-only)
    Query {
        /// SQL query to execute
        sql: String,

        /// Output format
        #[arg(short, long, default_value = "tsv")]
        format: OutputFormat,
    },

    /// Clear the cache completely
    Clear,
}

pub fn run(cmd: CacheCommands) -> Result<()> {
    match cmd {
        CacheCommands::Rebuild => run_rebuild(),
        CacheCommands::Sync => run_sync(),
        CacheCommands::Status => run_status(),
        CacheCommands::Query { sql, format } => run_query(&sql, format),
        CacheCommands::Clear => run_clear(),
    }
}

fn run_rebuild() -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let mut cache = EntityCache::open_without_sync(&project)?;

    println!("{} Rebuilding cache...", style("→").blue());
    let stats = cache.rebuild()?;

    println!(
        "{} Cache rebuilt in {}ms",
        style("✓").green(),
        stats.duration_ms
    );
    println!("  Files scanned:   {}", stats.files_scanned);
    println!("  Entities cached: {}", stats.entities_added);

    Ok(())
}

fn run_sync() -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let mut cache = EntityCache::open_without_sync(&project)?;

    let stats = cache.sync()?;

    if stats.entities_added == 0 && stats.entities_updated == 0 && stats.entities_removed == 0 {
        println!("{} Cache is up to date", style("✓").green());
    } else {
        println!(
            "{} Cache synced in {}ms",
            style("✓").green(),
            stats.duration_ms
        );
        if stats.entities_added > 0 {
            println!("  Added:   {}", style(stats.entities_added).green());
        }
        if stats.entities_updated > 0 {
            println!("  Updated: {}", style(stats.entities_updated).yellow());
        }
        if stats.entities_removed > 0 {
            println!("  Removed: {}", style(stats.entities_removed).red());
        }
    }

    Ok(())
}

fn run_status() -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let cache = EntityCache::open(&project)?;

    let stats = cache.statistics()?;

    println!("{}", style("Cache Status").bold());
    println!("{}", style("─".repeat(40)).dim());
    println!(
        "  Location:        {}",
        project.root().join(".tdt/cache.db").display()
    );
    println!("  Total entities:  {}", style(stats.total_entities).cyan());
    println!("  Total short IDs: {}", style(stats.total_short_ids).cyan());
    println!(
        "  Database size:   {} KB",
        style(stats.db_size_bytes / 1024).cyan()
    );

    if !stats.by_prefix.is_empty() {
        println!();
        println!("  {} {}", style("By Type:").bold(), "");
        let mut prefixes: Vec<_> = stats.by_prefix.iter().collect();
        prefixes.sort_by_key(|(k, _)| *k);
        for (prefix, count) in prefixes {
            println!("    {:<6} {}", prefix, count);
        }
    }

    Ok(())
}

fn run_query(sql: &str, format: OutputFormat) -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let cache = EntityCache::open(&project)?;

    // Get columns
    let columns = cache.query_columns(sql)?;
    let rows = cache.query_raw(sql)?;

    match format {
        OutputFormat::Json => {
            // Output as JSON array of objects
            let json_rows: Vec<serde_json::Value> = rows
                .iter()
                .map(|row| {
                    let mut obj = serde_json::Map::new();
                    for (i, col) in columns.iter().enumerate() {
                        if let Some(val) = row.get(i) {
                            obj.insert(col.clone(), serde_json::Value::String(val.clone()));
                        }
                    }
                    serde_json::Value::Object(obj)
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&json_rows).unwrap());
        }
        OutputFormat::Csv => {
            // CSV output
            println!("{}", columns.join(","));
            for row in rows {
                println!(
                    "{}",
                    row.iter()
                        .map(|s| escape_csv(s))
                        .collect::<Vec<_>>()
                        .join(",")
                );
            }
        }
        OutputFormat::Yaml => {
            // YAML output
            let yaml_rows: Vec<serde_json::Value> = rows
                .iter()
                .map(|row| {
                    let mut obj = serde_json::Map::new();
                    for (i, col) in columns.iter().enumerate() {
                        if let Some(val) = row.get(i) {
                            obj.insert(col.clone(), serde_json::Value::String(val.clone()));
                        }
                    }
                    serde_json::Value::Object(obj)
                })
                .collect();
            println!("{}", serde_yml::to_string(&yaml_rows).unwrap());
        }
        _ => {
            // Default: TSV table format
            println!("{}", columns.join("\t"));
            for row in rows {
                println!("{}", row.join("\t"));
            }
        }
    }

    Ok(())
}

fn run_clear() -> Result<()> {
    let project = Project::discover().map_err(|e| miette::miette!("{}", e))?;
    let cache_path = project.root().join(".tdt/cache.db");

    if cache_path.exists() {
        std::fs::remove_file(&cache_path)
            .map_err(|e| miette::miette!("Failed to remove cache: {}", e))?;

        // Also remove WAL and journal files if they exist
        let _ = std::fs::remove_file(project.root().join(".tdt/cache.db-journal"));
        let _ = std::fs::remove_file(project.root().join(".tdt/cache.db-wal"));
        let _ = std::fs::remove_file(project.root().join(".tdt/cache.db-shm"));

        println!("{} Cache cleared", style("✓").green());
    } else {
        println!("No cache to clear");
    }

    Ok(())
}

/// Escape a string for CSV output
fn escape_csv(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn create_test_project() -> (tempfile::TempDir, Project) {
        let tmp = tempdir().unwrap();
        let project = Project::init(tmp.path()).unwrap();
        (tmp, project)
    }

    fn write_test_entity(project: &Project, rel_path: &str, content: &str) {
        let full_path = project.root().join(rel_path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&full_path, content).unwrap();
    }

    #[test]
    fn test_cache_clear() {
        let (_tmp, project) = create_test_project();

        // Create cache
        {
            let _cache = EntityCache::open(&project).unwrap();
        }

        // Verify cache exists
        let cache_path = project.root().join(".tdt/cache.db");
        assert!(cache_path.exists());

        // Clear cache
        fs::remove_file(&cache_path).unwrap();
        assert!(!cache_path.exists());
    }

    #[test]
    fn test_cache_rebuild() {
        let (_tmp, project) = create_test_project();

        write_test_entity(
            &project,
            "requirements/inputs/REQ-01ABC.tdt.yaml",
            r#"
id: REQ-01ABC
title: Test Requirement
status: draft
author: Test
created: 2024-01-15T10:30:00Z
"#,
        );

        let mut cache = EntityCache::open_without_sync(&project).unwrap();
        let stats = cache.rebuild().unwrap();

        assert_eq!(stats.entities_added, 1);
    }

    #[test]
    fn test_cache_query() {
        let (_tmp, project) = create_test_project();

        write_test_entity(
            &project,
            "requirements/inputs/REQ-01ABC.tdt.yaml",
            r#"
id: REQ-01ABC
title: Test Requirement
status: draft
author: Test
created: 2024-01-15T10:30:00Z
"#,
        );

        let mut cache = EntityCache::open_without_sync(&project).unwrap();
        cache.rebuild().unwrap();

        let rows = cache.query_raw("SELECT id, title FROM entities").unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0][0], "REQ-01ABC");
    }
}
