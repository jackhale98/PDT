//! SQLite-backed entity cache for fast lookups
//!
//! This module provides a local SQLite cache that:
//! - Maps short IDs (PREFIX@N) to full entity IDs
//! - Caches entity metadata for fast lookups
//! - Auto-detects file changes and syncs incrementally
//! - Supports direct SQL queries for power users
//!
//! IMPORTANT: The cache is user-local and gitignored.
//! Entity files must NEVER contain short IDs - only full ULIDs.

mod queries;
mod schema;
mod sync;
mod types;

// Re-export all types
pub use types::*;

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use chrono::{DateTime, TimeZone, Utc};
use miette::{IntoDiagnostic, Result};
use rusqlite::{params, Connection, OptionalExtension};
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

use crate::core::project::Project;

/// Cache file location within a project
const CACHE_FILE: &str = ".tdt/cache.db";

/// Current schema version - cache is rebuilt on version mismatch
const SCHEMA_VERSION: i32 = 7;

/// The entity cache backed by SQLite
pub struct EntityCache {
    conn: Connection,
    project_root: PathBuf,
}

impl EntityCache {
    /// Open or create cache for a project
    ///
    /// If the cache doesn't exist, it will be created and populated.
    /// If the cache is stale (files changed), it will be synced automatically.
    pub fn open(project: &Project) -> Result<Self> {
        let cache_path = project.root().join(CACHE_FILE);

        // Ensure .tdt directory exists
        if let Some(parent) = cache_path.parent() {
            fs::create_dir_all(parent).into_diagnostic()?;
        }

        let needs_init = !cache_path.exists();
        let conn = Connection::open(&cache_path).into_diagnostic()?;

        // Enable WAL mode for better concurrent access
        conn.execute_batch("PRAGMA journal_mode=WAL;")
            .into_diagnostic()?;

        let mut cache = Self {
            conn,
            project_root: project.root().to_path_buf(),
        };

        if needs_init {
            cache.init_schema()?;
            cache.rebuild()?;
        } else {
            // Check schema version - if mismatch, reinitialize (no migrations needed)
            if cache.needs_schema_rebuild()? {
                cache.reinitialize_schema()?;
            }
            // Auto-sync to detect file changes
            cache.auto_sync()?;
        }

        Ok(cache)
    }

    /// Check if schema version matches current version
    fn needs_schema_rebuild(&self) -> Result<bool> {
        let current_version: i32 = self
            .conn
            .query_row("SELECT version FROM schema_version LIMIT 1", [], |row| {
                row.get(0)
            })
            .unwrap_or(0);

        Ok(current_version != SCHEMA_VERSION)
    }

    /// Drop all tables and reinitialize schema, then rebuild
    fn reinitialize_schema(&mut self) -> Result<()> {
        // Drop all tables (keeping the database file)
        self.conn
            .execute_batch(
                r#"
                DROP TABLE IF EXISTS schema_version;
                DROP TABLE IF EXISTS short_ids;
                DROP TABLE IF EXISTS short_id_counters;
                DROP TABLE IF EXISTS entities;
                DROP TABLE IF EXISTS features;
                DROP TABLE IF EXISTS components;
                DROP TABLE IF EXISTS risks;
                DROP TABLE IF EXISTS tests;
                DROP TABLE IF EXISTS quotes;
                DROP TABLE IF EXISTS suppliers;
                DROP TABLE IF EXISTS processes;
                DROP TABLE IF EXISTS controls;
                DROP TABLE IF EXISTS works;
                DROP TABLE IF EXISTS ncrs;
                DROP TABLE IF EXISTS capas;
                DROP TABLE IF EXISTS assemblies;
                DROP TABLE IF EXISTS results;
                DROP TABLE IF EXISTS links;
                DROP TABLE IF EXISTS cache_meta;
                "#,
            )
            .into_diagnostic()?;

        // Reinitialize with current schema
        self.init_schema()?;
        self.rebuild()?;

        Ok(())
    }

    /// Auto-sync: quickly check if any files changed and sync if needed
    fn auto_sync(&mut self) -> Result<()> {
        // Get the most recent file mtime from cache
        let cached_max_mtime: Option<i64> = self
            .conn
            .query_row("SELECT MAX(file_mtime) FROM entities", [], |row| row.get(0))
            .optional()
            .into_diagnostic()?
            .flatten();

        // Quick check: scan for any file newer than cached max mtime
        let needs_sync = self.has_newer_files(cached_max_mtime.unwrap_or(0))?;

        if needs_sync {
            self.sync()?;
        }

        Ok(())
    }

    /// Check if any entity files are newer than the given mtime
    fn has_newer_files(&self, max_cached_mtime: i64) -> Result<bool> {
        let entity_dirs = Self::entity_directories();

        for dir in entity_dirs {
            let full_path = self.project_root.join(dir);
            if !full_path.exists() {
                continue;
            }

            for entry in WalkDir::new(&full_path)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
            {
                let path = entry.path();
                if !path.to_string_lossy().ends_with(".tdt.yaml") {
                    continue;
                }

                let mtime = get_file_mtime(path)?;
                if mtime > max_cached_mtime {
                    return Ok(true);
                }
            }
        }

        // Also check if any cached files were deleted
        let cached_count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM entities", [], |row| row.get(0))
            .into_diagnostic()?;

        let mut actual_count = 0i64;
        for dir in entity_dirs {
            let full_path = self.project_root.join(dir);
            if full_path.exists() {
                for entry in WalkDir::new(&full_path)
                    .into_iter()
                    .filter_map(|e| e.ok())
                    .filter(|e| e.file_type().is_file())
                {
                    if entry.path().to_string_lossy().ends_with(".tdt.yaml") {
                        actual_count += 1;
                    }
                }
            }
        }

        if actual_count != cached_count {
            return Ok(true);
        }

        Ok(false)
    }

    /// Get the list of entity directories to scan
    fn entity_directories() -> &'static [&'static str] {
        &[
            "requirements/inputs",
            "requirements/outputs",
            "risks/design",
            "risks/process",
            "bom/assemblies",
            "bom/components",
            "bom/quotes",
            "bom/suppliers",
            "tolerances/features",
            "tolerances/mates",
            "tolerances/stackups",
            "verification/protocols",
            "verification/results",
            "validation/protocols",
            "validation/results",
            "manufacturing/processes",
            "manufacturing/controls",
            "manufacturing/work_instructions",
            "manufacturing/ncrs",
            "manufacturing/capas",
        ]
    }

    /// Open cache without auto-sync (for testing)
    pub fn open_without_sync(project: &Project) -> Result<Self> {
        let cache_path = project.root().join(CACHE_FILE);

        if let Some(parent) = cache_path.parent() {
            fs::create_dir_all(parent).into_diagnostic()?;
        }

        let needs_init = !cache_path.exists();
        let conn = Connection::open(&cache_path).into_diagnostic()?;
        conn.execute_batch("PRAGMA journal_mode=WAL;")
            .into_diagnostic()?;

        let mut cache = Self {
            conn,
            project_root: project.root().to_path_buf(),
        };

        if needs_init {
            cache.init_schema()?;
        }

        Ok(cache)
    }

    /// Initialize the database schema
    pub fn ensure_short_id(&mut self, entity_id: &str) -> Result<String> {
        // Check if already exists
        let existing: Option<String> = self
            .conn
            .query_row(
                "SELECT short_id FROM short_ids WHERE entity_id = ?1",
                params![entity_id],
                |row| row.get(0),
            )
            .optional()
            .into_diagnostic()?;

        if let Some(short_id) = existing {
            return Ok(short_id);
        }

        // Extract prefix
        let prefix = entity_id
            .split('-')
            .next()
            .ok_or_else(|| miette::miette!("Invalid entity ID format"))?;

        // Get next ID for this prefix
        let next_id: i64 = self
            .conn
            .query_row(
                "SELECT next_id FROM short_id_counters WHERE prefix = ?1",
                params![prefix],
                |row| row.get(0),
            )
            .optional()
            .into_diagnostic()?
            .unwrap_or(1);

        let short_id = format!("{}@{}", prefix, next_id);

        // Insert short ID mapping
        self.conn
            .execute(
                "INSERT INTO short_ids (short_id, entity_id, prefix) VALUES (?1, ?2, ?3)",
                params![short_id, entity_id, prefix],
            )
            .into_diagnostic()?;

        // Update counter
        self.conn
            .execute(
                "INSERT OR REPLACE INTO short_id_counters (prefix, next_id) VALUES (?1, ?2)",
                params![prefix, next_id + 1],
            )
            .into_diagnostic()?;

        Ok(short_id)
    }

    /// Resolve a short ID to full entity ID
    pub fn resolve_short_id(&self, short_id: &str) -> Option<String> {
        // Normalize: PREFIX@N format, case-insensitive prefix
        if let Some(at_pos) = short_id.find('@') {
            let prefix = &short_id[..at_pos];
            let num = &short_id[at_pos + 1..];
            let normalized = format!("{}@{}", prefix.to_ascii_uppercase(), num);

            self.conn
                .query_row(
                    "SELECT entity_id FROM short_ids WHERE short_id = ?1",
                    params![normalized],
                    |row| row.get(0),
                )
                .optional()
                .ok()
                .flatten()
        } else {
            None
        }
    }

    /// Get short ID for an entity
    pub fn get_short_id(&self, entity_id: &str) -> Option<String> {
        self.conn
            .query_row(
                "SELECT short_id FROM short_ids WHERE entity_id = ?1",
                params![entity_id],
                |row| row.get(0),
            )
            .optional()
            .ok()
            .flatten()
    }

    /// Count entities grouped by prefix (entity type)
    pub fn count_by_prefix(&self) -> Vec<GroupCount> {
        let mut stmt = match self.conn.prepare(
            "SELECT prefix, COUNT(*) as cnt FROM entities GROUP BY prefix ORDER BY cnt DESC",
        ) {
            Ok(s) => s,
            Err(_) => return vec![],
        };

        let rows = match stmt.query_map([], |row| {
            Ok(GroupCount {
                group: row.get(0)?,
                count: row.get::<_, i64>(1)? as usize,
            })
        }) {
            Ok(r) => r,
            Err(_) => return vec![],
        };

        rows.filter_map(|r| r.ok()).collect()
    }

    /// Count entities grouped by status
    pub fn count_by_status(&self) -> Vec<GroupCount> {
        let mut stmt = match self.conn.prepare(
            "SELECT status, COUNT(*) as cnt FROM entities GROUP BY status ORDER BY cnt DESC",
        ) {
            Ok(s) => s,
            Err(_) => return vec![],
        };

        let rows = match stmt.query_map([], |row| {
            Ok(GroupCount {
                group: row.get(0)?,
                count: row.get::<_, i64>(1)? as usize,
            })
        }) {
            Ok(r) => r,
            Err(_) => return vec![],
        };

        rows.filter_map(|r| r.ok()).collect()
    }

    /// Get risk distribution summary
    #[allow(clippy::field_reassign_with_default)]
    pub fn risk_distribution(&self) -> RiskDistribution {
        let mut dist = RiskDistribution::default();

        // Total count
        dist.total = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM entities WHERE prefix = 'RISK'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap_or(0) as usize;

        // By risk level
        if let Ok(mut stmt) = self.conn.prepare(
            "SELECT risk_level, COUNT(*) FROM risks WHERE risk_level IS NOT NULL GROUP BY risk_level ORDER BY COUNT(*) DESC",
        ) {
            if let Ok(rows) = stmt.query_map([], |row| {
                Ok(GroupCount {
                    group: row.get::<_, String>(0).unwrap_or_default(),
                    count: row.get::<_, i64>(1).unwrap_or(0) as usize,
                })
            }) {
                dist.by_level = rows.filter_map(|r| r.ok()).collect();
            }
        }

        // By status
        if let Ok(mut stmt) = self.conn.prepare(
            "SELECT e.status, COUNT(*) FROM entities e WHERE e.prefix = 'RISK' GROUP BY e.status ORDER BY COUNT(*) DESC",
        ) {
            if let Ok(rows) = stmt.query_map([], |row| {
                Ok(GroupCount {
                    group: row.get::<_, String>(0).unwrap_or_default(),
                    count: row.get::<_, i64>(1).unwrap_or(0) as usize,
                })
            }) {
                dist.by_status = rows.filter_map(|r| r.ok()).collect();
            }
        }

        // Average and max RPN
        if let Ok(row) = self.conn.query_row(
            "SELECT AVG(rpn), MAX(rpn) FROM risks WHERE rpn IS NOT NULL",
            [],
            |row| Ok((row.get::<_, Option<f64>>(0)?, row.get::<_, Option<i32>>(1)?)),
        ) {
            dist.average_rpn = row.0;
            dist.max_rpn = row.1;
        }

        dist
    }

    /// Get requirement test coverage summary
    pub fn requirement_coverage(&self) -> RequirementCoverage {
        let mut cov = RequirementCoverage::default();

        // Total requirements
        cov.total_requirements = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM entities WHERE prefix = 'REQ'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap_or(0) as usize;

        // Requirements with tests (have a 'verifies' link to them)
        cov.with_tests = self
            .conn
            .query_row(
                "SELECT COUNT(DISTINCT target_id) FROM links WHERE link_type = 'verifies' AND target_id LIKE 'REQ-%'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap_or(0) as usize;

        cov.without_tests = cov.total_requirements.saturating_sub(cov.with_tests);

        cov.coverage_percent = if cov.total_requirements > 0 {
            (cov.with_tests as f64 / cov.total_requirements as f64) * 100.0
        } else {
            0.0
        };

        cov
    }

    /// Get quote summary by supplier
    pub fn quote_summary_by_supplier(&self) -> Vec<SupplierQuoteSummary> {
        let mut stmt = match self.conn.prepare(
            r#"SELECT q.supplier_id, q.supplier_name, COUNT(*) as cnt,
                      SUM(q.unit_price * COALESCE(q.quantity, 1)) as total,
                      AVG(q.lead_time_days) as avg_lead
               FROM quotes q
               GROUP BY q.supplier_id, q.supplier_name
               ORDER BY cnt DESC"#,
        ) {
            Ok(s) => s,
            Err(_) => return vec![],
        };

        let rows = match stmt.query_map([], |row| {
            Ok(SupplierQuoteSummary {
                supplier_id: row.get(0)?,
                supplier_name: row.get(1)?,
                quote_count: row.get::<_, i64>(2)? as usize,
                total_value: row.get(3)?,
                avg_lead_time: row.get(4)?,
            })
        }) {
            Ok(r) => r,
            Err(_) => return vec![],
        };

        rows.filter_map(|r| r.ok()).collect()
    }

    /// Get project-wide statistics
    pub fn project_stats(&self) -> ProjectStats {
        ProjectStats {
            entity_counts: self.count_by_prefix(),
            status_counts: self.count_by_status(),
        }
    }

    // =========================================================================
    // Link Query Methods (for trace operations)
    // =========================================================================

    /// Get all outgoing links from an entity (what it links TO)
    pub fn get_links_from(&self, source_id: &str) -> Vec<CachedLink> {
        let mut stmt = match self
            .conn
            .prepare("SELECT source_id, target_id, link_type FROM links WHERE source_id = ?1")
        {
            Ok(s) => s,
            Err(_) => return vec![],
        };

        let rows = match stmt.query_map(params![source_id], |row| {
            Ok(CachedLink {
                source_id: row.get(0)?,
                target_id: row.get(1)?,
                link_type: row.get(2)?,
            })
        }) {
            Ok(r) => r,
            Err(_) => return vec![],
        };

        rows.filter_map(|r| r.ok()).collect()
    }

    /// Get all incoming links to an entity (what links TO it)
    pub fn get_links_to(&self, target_id: &str) -> Vec<CachedLink> {
        let mut stmt = match self
            .conn
            .prepare("SELECT source_id, target_id, link_type FROM links WHERE target_id = ?1")
        {
            Ok(s) => s,
            Err(_) => return vec![],
        };

        let rows = match stmt.query_map(params![target_id], |row| {
            Ok(CachedLink {
                source_id: row.get(0)?,
                target_id: row.get(1)?,
                link_type: row.get(2)?,
            })
        }) {
            Ok(r) => r,
            Err(_) => return vec![],
        };

        rows.filter_map(|r| r.ok()).collect()
    }

    /// Get links of a specific type from an entity
    pub fn get_links_from_of_type(&self, source_id: &str, link_type: &str) -> Vec<String> {
        let mut stmt = match self
            .conn
            .prepare("SELECT target_id FROM links WHERE source_id = ?1 AND link_type = ?2")
        {
            Ok(s) => s,
            Err(_) => return vec![],
        };

        let rows = match stmt.query_map(params![source_id, link_type], |row| row.get(0)) {
            Ok(r) => r,
            Err(_) => return vec![],
        };

        rows.filter_map(|r| r.ok()).collect()
    }

    /// Get links of a specific type to an entity
    pub fn get_links_to_of_type(&self, target_id: &str, link_type: &str) -> Vec<String> {
        let mut stmt = match self
            .conn
            .prepare("SELECT source_id FROM links WHERE target_id = ?1 AND link_type = ?2")
        {
            Ok(s) => s,
            Err(_) => return vec![],
        };

        let rows = match stmt.query_map(params![target_id, link_type], |row| row.get(0)) {
            Ok(r) => r,
            Err(_) => return vec![],
        };

        rows.filter_map(|r| r.ok()).collect()
    }

    /// Get assembly-specific info (part_number, revision) for an assembly ID
    pub fn get_assembly_info(&self, id: &str) -> Option<(Option<String>, Option<String>)> {
        let mut stmt = match self
            .conn
            .prepare("SELECT part_number, revision FROM assemblies WHERE id = ?1")
        {
            Ok(s) => s,
            Err(_) => return None,
        };

        stmt.query_row(params![id], |row| {
            Ok((
                row.get::<_, Option<String>>(0)?,
                row.get::<_, Option<String>>(1)?,
            ))
        })
        .ok()
    }

    /// Trace forward from an entity (recursive)
    /// Returns all entities reachable from source via outgoing links
    pub fn trace_from(&self, source_id: &str, max_depth: usize) -> Vec<(String, String, usize)> {
        let mut results = Vec::new();
        let mut visited = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();

        queue.push_back((source_id.to_string(), 0usize));
        visited.insert(source_id.to_string());

        while let Some((current_id, depth)) = queue.pop_front() {
            if depth >= max_depth {
                continue;
            }

            for link in self.get_links_from(&current_id) {
                if !visited.contains(&link.target_id) {
                    visited.insert(link.target_id.clone());
                    results.push((link.target_id.clone(), link.link_type.clone(), depth + 1));
                    queue.push_back((link.target_id, depth + 1));
                }
            }
        }

        results
    }

    /// Trace backward to an entity (recursive)
    /// Returns all entities that can reach target via outgoing links
    pub fn trace_to(&self, target_id: &str, max_depth: usize) -> Vec<(String, String, usize)> {
        let mut results = Vec::new();
        let mut visited = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();

        queue.push_back((target_id.to_string(), 0usize));
        visited.insert(target_id.to_string());

        while let Some((current_id, depth)) = queue.pop_front() {
            if depth >= max_depth {
                continue;
            }

            for link in self.get_links_to(&current_id) {
                if !visited.contains(&link.source_id) {
                    visited.insert(link.source_id.clone());
                    results.push((link.source_id.clone(), link.link_type.clone(), depth + 1));
                    queue.push_back((link.source_id, depth + 1));
                }
            }
        }

        results
    }

    /// Find orphan entities (no incoming or outgoing links)
    pub fn find_orphans(&self, prefix: Option<&str>) -> Vec<CachedEntity> {
        let sql = if let Some(p) = prefix {
            format!(
                r#"SELECT e.id, e.prefix, e.title, e.status, e.author, e.created, e.file_path,
                          e.priority, e.entity_type, e.category, e.tags
                   FROM entities e
                   WHERE e.prefix = '{}'
                   AND NOT EXISTS (SELECT 1 FROM links WHERE source_id = e.id)
                   AND NOT EXISTS (SELECT 1 FROM links WHERE target_id = e.id)"#,
                p
            )
        } else {
            r#"SELECT e.id, e.prefix, e.title, e.status, e.author, e.created, e.file_path,
                      e.priority, e.entity_type, e.category, e.tags
               FROM entities e
               WHERE NOT EXISTS (SELECT 1 FROM links WHERE source_id = e.id)
               AND NOT EXISTS (SELECT 1 FROM links WHERE target_id = e.id)"#
                .to_string()
        };

        let mut stmt = match self.conn.prepare(&sql) {
            Ok(s) => s,
            Err(_) => return vec![],
        };

        let rows = match stmt.query_map([], |row| {
            let tags_str: Option<String> = row.get(10)?;
            let tags = tags_str
                .map(|s| {
                    s.split(',')
                        .filter(|t| !t.is_empty())
                        .map(String::from)
                        .collect()
                })
                .unwrap_or_default();
            Ok(CachedEntity {
                id: row.get(0)?,
                prefix: row.get(1)?,
                title: row.get(2)?,
                status: row.get(3)?,
                author: row.get(4)?,
                created: parse_datetime(row.get::<_, String>(5)?),
                file_path: PathBuf::from(row.get::<_, String>(6)?),
                priority: row.get(7)?,
                entity_type: row.get(8)?,
                category: row.get(9)?,
                tags,
            })
        }) {
            Ok(r) => r,
            Err(_) => return vec![],
        };

        rows.filter_map(|r| r.ok()).collect()
    }

    /// Count links by type (for statistics)
    pub fn count_links_by_type(&self) -> HashMap<String, usize> {
        let mut result = HashMap::new();

        let mut stmt = match self
            .conn
            .prepare("SELECT link_type, COUNT(*) FROM links GROUP BY link_type")
        {
            Ok(s) => s,
            Err(_) => return result,
        };

        let rows = match stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, usize>(1)?))
        }) {
            Ok(r) => r,
            Err(_) => return result,
        };

        for row in rows.flatten() {
            result.insert(row.0, row.1);
        }

        result
    }

    /// Get all entity IDs that have links (either direction)
    pub fn get_linked_entity_ids(&self) -> std::collections::HashSet<String> {
        let mut result = std::collections::HashSet::new();

        if let Ok(mut stmt) = self.conn.prepare(
            "SELECT DISTINCT source_id FROM links UNION SELECT DISTINCT target_id FROM links",
        ) {
            if let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(0)) {
                for row in rows.flatten() {
                    result.insert(row);
                }
            }
        }

        result
    }

    /// Get cache statistics
    pub fn statistics(&self) -> Result<CacheStats> {
        let total_entities: usize = self
            .conn
            .query_row("SELECT COUNT(*) FROM entities", [], |row| row.get(0))
            .into_diagnostic()?;

        let total_short_ids: usize = self
            .conn
            .query_row("SELECT COUNT(*) FROM short_ids", [], |row| row.get(0))
            .into_diagnostic()?;

        let mut by_prefix = HashMap::new();
        {
            let mut stmt = self
                .conn
                .prepare("SELECT prefix, COUNT(*) FROM entities GROUP BY prefix")
                .into_diagnostic()?;
            let rows = stmt
                .query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, usize>(1)?))
                })
                .into_diagnostic()?;

            for row in rows {
                let (prefix, count) = row.into_diagnostic()?;
                by_prefix.insert(prefix, count);
            }
        }

        let db_path = self.project_root.join(CACHE_FILE);
        let db_size_bytes = fs::metadata(&db_path).map(|m| m.len()).unwrap_or(0);

        Ok(CacheStats {
            total_entities,
            total_short_ids,
            by_prefix,
            db_size_bytes,
        })
    }

    /// Execute raw SQL query (read-only)
    pub fn query_raw(&self, sql: &str) -> Result<Vec<Vec<String>>> {
        let mut stmt = self.conn.prepare(sql).into_diagnostic()?;
        let column_count = stmt.column_count();

        let rows = stmt
            .query_map([], |row| {
                let mut values = Vec::with_capacity(column_count);
                for i in 0..column_count {
                    let value: String = row
                        .get::<_, rusqlite::types::Value>(i)
                        .map(|v| match v {
                            rusqlite::types::Value::Null => "NULL".to_string(),
                            rusqlite::types::Value::Integer(i) => i.to_string(),
                            rusqlite::types::Value::Real(f) => f.to_string(),
                            rusqlite::types::Value::Text(s) => s,
                            rusqlite::types::Value::Blob(_) => "<blob>".to_string(),
                        })
                        .unwrap_or_default();
                    values.push(value);
                }
                Ok(values)
            })
            .into_diagnostic()?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .into_diagnostic()
    }

    /// Get column names for a query
    pub fn query_columns(&self, sql: &str) -> Result<Vec<String>> {
        let stmt = self.conn.prepare(sql).into_diagnostic()?;
        Ok(stmt.column_names().iter().map(|s| s.to_string()).collect())
    }

    /// Clear the entire cache (for testing or reset)
    pub fn clear(&mut self) -> Result<()> {
        self.conn
            .execute_batch(
                r#"
            DELETE FROM entities;
            DELETE FROM features;
            DELETE FROM components;
            DELETE FROM risks;
            DELETE FROM short_ids;
            DELETE FROM short_id_counters;
            "#,
            )
            .into_diagnostic()?;
        Ok(())
    }
}

/// Get file modification time as Unix timestamp
fn get_file_mtime(path: &Path) -> Result<i64> {
    let metadata = fs::metadata(path).into_diagnostic()?;
    let mtime = metadata
        .modified()
        .into_diagnostic()?
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    Ok(mtime)
}

/// Compute SHA256 hash of content
fn compute_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Parse datetime string to DateTime<Utc>
fn parse_datetime(s: String) -> DateTime<Utc> {
    chrono::DateTime::parse_from_rfc3339(&s)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc.with_ymd_and_hms(2000, 1, 1, 0, 0, 0).unwrap())
}

#[cfg(test)]
mod tests;
