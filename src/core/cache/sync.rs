//! Cache synchronization with filesystem
//!
//! Methods for rebuilding and incrementally syncing the cache with YAML files.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use miette::{IntoDiagnostic, Result};
use rusqlite::{params, OptionalExtension};
use walkdir::WalkDir;

use super::{compute_hash, get_file_mtime, EntityCache, SyncStats};

impl EntityCache {
    /// Full rebuild of cache from filesystem
    pub fn rebuild(&mut self) -> Result<SyncStats> {
        let start = std::time::Instant::now();
        let mut stats = SyncStats::default();

        // Clear existing entity data (but preserve short ID mappings)
        self.conn
            .execute_batch(
                r#"
            DELETE FROM entities;
            DELETE FROM features;
            DELETE FROM components;
            DELETE FROM risks;
            DELETE FROM hazards;
            DELETE FROM tests;
            DELETE FROM quotes;
            DELETE FROM suppliers;
            DELETE FROM processes;
            DELETE FROM controls;
            DELETE FROM works;
            DELETE FROM ncrs;
            DELETE FROM capas;
            DELETE FROM assemblies;
            DELETE FROM results;
            DELETE FROM links;
            "#,
            )
            .into_diagnostic()?;

        // Scan all entity directories
        for dir in Self::entity_directories() {
            let full_path = self.project_root.join(dir);
            if full_path.exists() {
                self.scan_directory(&full_path, &mut stats)?;
            }
        }

        stats.duration_ms = start.elapsed().as_millis() as u64;
        Ok(stats)
    }

    /// Scan a directory and cache all entities
    pub(super) fn scan_directory(&mut self, dir: &Path, stats: &mut SyncStats) -> Result<()> {
        for entry in WalkDir::new(dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            let path = entry.path();
            if !path.to_string_lossy().ends_with(".tdt.yaml") {
                continue;
            }

            stats.files_scanned += 1;

            if let Err(e) = self.cache_entity_file(path) {
                eprintln!("Warning: Failed to cache {}: {}", path.display(), e);
            } else {
                stats.entities_added += 1;
            }
        }

        Ok(())
    }

    /// Cache a single entity file
    pub(super) fn cache_entity_file(&mut self, path: &Path) -> Result<()> {
        let content = fs::read_to_string(path).into_diagnostic()?;
        let mtime = get_file_mtime(path)?;
        let hash = compute_hash(&content);
        let rel_path = path
            .strip_prefix(&self.project_root)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();

        let value: serde_yml::Value = serde_yml::from_str(&content).into_diagnostic()?;

        let id = value["id"]
            .as_str()
            .ok_or_else(|| miette::miette!("Missing 'id' field"))?;
        let title = value["title"]
            .as_str()
            .or_else(|| value["name"].as_str())
            .unwrap_or("");
        let status = value["status"].as_str().unwrap_or("draft");
        let author = value["author"].as_str().unwrap_or("");
        let created = value["created"].as_str().unwrap_or("");

        let priority = value["priority"].as_str();
        let entity_type = value["type"].as_str();
        let level = value["level"].as_str();
        let category = value["category"].as_str();
        let tags: Option<String> = value["tags"].as_sequence().map(|seq| {
            seq.iter()
                .filter_map(|v| v.as_str())
                .collect::<Vec<_>>()
                .join(",")
        });

        let prefix = id
            .split('-')
            .next()
            .ok_or_else(|| miette::miette!("Invalid ID format"))?;

        self.conn
            .execute(
                r#"INSERT OR REPLACE INTO entities
                   (id, prefix, title, status, author, created, file_path, file_mtime, file_hash,
                    priority, entity_type, level, category, tags)
                   VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)"#,
                params![
                    id,
                    prefix,
                    title,
                    status,
                    author,
                    created,
                    rel_path,
                    mtime,
                    hash,
                    priority,
                    entity_type,
                    level,
                    category,
                    tags
                ],
            )
            .into_diagnostic()?;

        self.ensure_short_id(id)?;

        match prefix {
            "FEAT" => self.cache_feature_data(id, &value)?,
            "CMP" => self.cache_component_data(id, &value)?,
            "RISK" => self.cache_risk_data(id, &value)?,
            "HAZ" => self.cache_hazard_data(id, &value)?,
            "TEST" => self.cache_test_data(id, &value)?,
            "QUOT" => self.cache_quote_data(id, &value)?,
            "SUP" => self.cache_supplier_data(id, &value)?,
            "PROC" => self.cache_process_data(id, &value)?,
            "CTRL" => self.cache_control_data(id, &value)?,
            "WORK" => self.cache_work_data(id, &value)?,
            "NCR" => self.cache_ncr_data(id, &value)?,
            "CAPA" => self.cache_capa_data(id, &value)?,
            "ASM" => self.cache_assembly_data(id, &value)?,
            "RSLT" => self.cache_result_data(id, &value)?,
            "LOT" => self.cache_lot_data(id, &value)?,
            "DEV" => self.cache_deviation_data(id, &value)?,
            _ => {}
        }

        self.cache_entity_links(id, &value)?;

        Ok(())
    }

    /// Incremental sync - only update changed files
    pub fn sync(&mut self) -> Result<SyncStats> {
        let start = std::time::Instant::now();
        let mut stats = SyncStats::default();

        let mut current_files: HashMap<String, std::path::PathBuf> = HashMap::new();

        for dir in Self::entity_directories() {
            let full_path = self.project_root.join(dir);
            if full_path.exists() {
                for entry in WalkDir::new(&full_path)
                    .into_iter()
                    .filter_map(|e| e.ok())
                    .filter(|e| e.file_type().is_file())
                {
                    let path = entry.path();
                    if path.to_string_lossy().ends_with(".tdt.yaml") {
                        let rel_path = path
                            .strip_prefix(&self.project_root)
                            .unwrap_or(path)
                            .to_string_lossy()
                            .to_string();
                        current_files.insert(rel_path, path.to_path_buf());
                        stats.files_scanned += 1;
                    }
                }
            }
        }

        let mut cached_files: HashMap<String, (i64, String)> = HashMap::new();
        {
            let mut stmt = self
                .conn
                .prepare("SELECT file_path, file_mtime, file_hash FROM entities")
                .into_diagnostic()?;
            let rows = stmt
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                })
                .into_diagnostic()?;

            for row in rows {
                let (path, mtime, hash) = row.into_diagnostic()?;
                cached_files.insert(path, (mtime, hash));
            }
        }

        for (rel_path, full_path) in &current_files {
            let needs_update = if let Some((cached_mtime, cached_hash)) = cached_files.get(rel_path)
            {
                let current_mtime = get_file_mtime(full_path)?;
                if current_mtime != *cached_mtime {
                    let content = fs::read_to_string(full_path).into_diagnostic()?;
                    let current_hash = compute_hash(&content);
                    current_hash != *cached_hash
                } else {
                    false
                }
            } else {
                true
            };

            if needs_update {
                if cached_files.contains_key(rel_path) {
                    stats.entities_updated += 1;
                } else {
                    stats.entities_added += 1;
                }
                self.cache_entity_file(full_path)?;
            }
        }

        for rel_path in cached_files.keys() {
            if !current_files.contains_key(rel_path) {
                let entity_id: Option<String> = self
                    .conn
                    .query_row(
                        "SELECT id FROM entities WHERE file_path = ?1",
                        params![rel_path],
                        |row| row.get(0),
                    )
                    .optional()
                    .into_diagnostic()?;

                if let Some(id) = entity_id {
                    self.remove_entity(&id)?;
                    stats.entities_removed += 1;
                }
            }
        }

        stats.duration_ms = start.elapsed().as_millis() as u64;
        Ok(stats)
    }

    /// Remove an entity from the cache
    pub(super) fn remove_entity(&self, id: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM entities WHERE id = ?1", params![id])
            .into_diagnostic()?;

        // Delete from type-specific tables
        for table in &[
            "features",
            "components",
            "risks",
            "hazards",
            "tests",
            "quotes",
            "suppliers",
            "processes",
            "controls",
            "works",
            "ncrs",
            "capas",
            "assemblies",
            "results",
        ] {
            self.conn
                .execute(&format!("DELETE FROM {} WHERE id = ?1", table), params![id])
                .into_diagnostic()?;
        }

        // Delete links
        self.conn
            .execute(
                "DELETE FROM links WHERE source_id = ?1 OR target_id = ?1",
                params![id],
            )
            .into_diagnostic()?;

        Ok(())
    }

    /// Extract and cache links from an entity
    pub(super) fn cache_entity_links(
        &self,
        source_id: &str,
        value: &serde_yml::Value,
    ) -> Result<()> {
        self.conn
            .execute("DELETE FROM links WHERE source_id = ?1", params![source_id])
            .into_diagnostic()?;

        let link_fields = [
            // Basic traceability
            ("traces_to", "traces_to"),
            ("traces_from", "traces_from"),
            ("verifies", "verifies"),
            ("verified_by", "verified_by"),
            ("mitigates", "mitigates"),
            ("mitigated_by", "mitigated_by"),
            ("references", "references"),
            ("related_to", "related_to"),
            // BOM structure
            ("components", "contains"),
            ("children", "contains"),
            ("parent", "contained_in"),
            ("used_in", "used_in"),
            // Requirement links
            ("satisfied_by", "satisfied_by"),
            ("requirements", "requirements"),
            ("derives_from", "derives_from"),
            ("derived_by", "derived_by"),
            ("allocated_to", "allocated_to"),
            ("allocated_from", "allocated_from"),
            // Risk links
            ("risks", "risks"),
            ("requirement", "requirement"),
            ("affects", "affects"),
            ("controls", "controls"),
            // Hazard links
            ("originates_from", "originates_from"),
            ("causes", "causes"),
            ("controlled_by", "controlled_by"),
            // Test/Result links
            ("component", "component"),
            ("assembly", "assembly"),
            ("tests", "tests"),
            ("ncrs", "ncrs"),
            ("from_result", "from_result"),
            // Process/Manufacturing links
            ("processes", "processes"),
            ("produces", "produces"),
            ("process", "process"),
            ("work_instructions", "work_instructions"),
            // Supplier/Quote links
            ("supplier", "supplier"),
            // NCR/CAPA links
            ("capa", "capa"),
            ("processes_modified", "processes_modified"),
            ("controls_added", "controls_added"),
        ];

        // Helper to extract links from a value
        let extract_links =
            |value: &serde_yml::Value, field: &str, link_type: &str| -> Vec<(String, String)> {
                let mut links = Vec::new();
                if let Some(targets) = value[field].as_sequence() {
                    for target in targets {
                        if let Some(target_id) = target.as_str() {
                            links.push((target_id.to_string(), link_type.to_string()));
                        } else if let Some(target_obj) = target.as_mapping() {
                            if let Some(target_id) = target_obj
                                .get(serde_yml::Value::String("id".to_string()))
                                .and_then(|v| v.as_str())
                            {
                                links.push((target_id.to_string(), link_type.to_string()));
                            }
                        }
                    }
                } else if let Some(target_id) = value[field].as_str() {
                    links.push((target_id.to_string(), link_type.to_string()));
                }
                links
            };

        for (field, link_type) in link_fields {
            // Check at top level
            for (target_id, ltype) in extract_links(value, field, link_type) {
                self.insert_link(source_id, &target_id, &ltype)?;
            }
            // Also check nested under "links" object
            if let Some(links_obj) = value.get("links") {
                for (target_id, ltype) in extract_links(links_obj, field, link_type) {
                    self.insert_link(source_id, &target_id, &ltype)?;
                }
            }
        }

        Ok(())
    }

    pub(super) fn insert_link(
        &self,
        source_id: &str,
        target_id: &str,
        link_type: &str,
    ) -> Result<()> {
        self.conn
            .execute(
                "INSERT OR IGNORE INTO links (source_id, target_id, link_type) VALUES (?1, ?2, ?3)",
                params![source_id, target_id, link_type],
            )
            .into_diagnostic()?;
        Ok(())
    }

    // Entity-specific caching methods

    pub(super) fn cache_feature_data(&self, id: &str, value: &serde_yml::Value) -> Result<()> {
        let component_id = value["component"].as_str().unwrap_or("");
        let feature_type = value["feature_type"].as_str().unwrap_or("internal");

        let dims = value["dimensions"].as_sequence();
        let (dim_name, dim_nominal, dim_plus_tol, dim_minus_tol, dim_internal) =
            if let Some(dims) = dims {
                if let Some(first) = dims.first() {
                    (
                        first["name"].as_str().map(String::from),
                        first["nominal"].as_f64(),
                        first["plus_tol"].as_f64(),
                        first["minus_tol"].as_f64(),
                        first["internal"].as_bool(),
                    )
                } else {
                    (None, None, None, None, None)
                }
            } else {
                (None, None, None, None, None)
            };

        self.conn
            .execute(
                r#"INSERT OR REPLACE INTO features
                   (id, component_id, feature_type, dim_name, dim_nominal, dim_plus_tol, dim_minus_tol, dim_internal)
                   VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"#,
                params![
                    id, component_id, feature_type, dim_name, dim_nominal, dim_plus_tol, dim_minus_tol,
                    dim_internal.map(|b| if b { 1 } else { 0 })
                ],
            )
            .into_diagnostic()?;

        Ok(())
    }

    pub(super) fn cache_component_data(&self, id: &str, value: &serde_yml::Value) -> Result<()> {
        self.conn
            .execute(
                r#"INSERT OR REPLACE INTO components (id, part_number, revision, make_buy, category)
                   VALUES (?1, ?2, ?3, ?4, ?5)"#,
                params![
                    id,
                    value["part_number"].as_str(),
                    value["revision"].as_str(),
                    value["make_buy"].as_str(),
                    value["category"].as_str()
                ],
            )
            .into_diagnostic()?;
        Ok(())
    }

    pub(super) fn cache_risk_data(&self, id: &str, value: &serde_yml::Value) -> Result<()> {
        self.conn
            .execute(
                r#"INSERT OR REPLACE INTO risks (id, risk_type, severity, occurrence, detection, rpn, risk_level)
                   VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"#,
                params![
                    id,
                    value["risk_type"].as_str(),
                    value["severity"].as_i64().map(|v| v as i32),
                    value["occurrence"].as_i64().map(|v| v as i32),
                    value["detection"].as_i64().map(|v| v as i32),
                    value["rpn"].as_i64().map(|v| v as i32),
                    value["risk_level"].as_str()
                ],
            )
            .into_diagnostic()?;
        Ok(())
    }

    pub(super) fn cache_hazard_data(&self, id: &str, value: &serde_yml::Value) -> Result<()> {
        self.conn
            .execute(
                r#"INSERT OR REPLACE INTO hazards (id, hazard_category, severity, energy_level, exposure_scenario)
                   VALUES (?1, ?2, ?3, ?4, ?5)"#,
                params![
                    id,
                    value["category"].as_str(),
                    value["severity"].as_str(),
                    value["energy_level"].as_str(),
                    value["exposure_scenario"].as_str()
                ],
            )
            .into_diagnostic()?;
        Ok(())
    }

    pub(super) fn cache_test_data(&self, id: &str, value: &serde_yml::Value) -> Result<()> {
        self.conn
            .execute(
                r#"INSERT OR REPLACE INTO tests (id, test_type, level, method) VALUES (?1, ?2, ?3, ?4)"#,
                params![id, value["type"].as_str(), value["level"].as_str(), value["method"].as_str()],
            )
            .into_diagnostic()?;
        Ok(())
    }

    pub(super) fn cache_quote_data(&self, id: &str, value: &serde_yml::Value) -> Result<()> {
        self.conn
            .execute(
                r#"INSERT OR REPLACE INTO quotes
                   (id, quote_status, supplier_id, component_id, unit_price, quantity, lead_time_days, currency, valid_until)
                   VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"#,
                params![
                    id,
                    value["quote_status"].as_str(),
                    value["supplier"].as_str(),
                    value["component"].as_str(),
                    value["unit_price"].as_f64(),
                    value["quantity"].as_i64().map(|v| v as i32),
                    value["lead_time_days"].as_i64().map(|v| v as i32),
                    value["currency"].as_str(),
                    value["valid_until"].as_str()
                ],
            )
            .into_diagnostic()?;
        Ok(())
    }

    pub(super) fn cache_supplier_data(&self, id: &str, value: &serde_yml::Value) -> Result<()> {
        let contact = if let Some(contacts) = value["contacts"].as_sequence() {
            contacts.first().cloned().unwrap_or(serde_yml::Value::Null)
        } else {
            value["contact"].clone()
        };

        let capabilities: Option<String> = value["capabilities"].as_sequence().map(|seq| {
            seq.iter()
                .filter_map(|v| v.as_str())
                .collect::<Vec<_>>()
                .join(",")
        });

        self.conn
            .execute(
                r#"INSERT OR REPLACE INTO suppliers
                   (id, short_name, contact_name, email, phone, location, website, lead_time_days, capabilities)
                   VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"#,
                params![
                    id,
                    value["short_name"].as_str(),
                    contact["name"].as_str(),
                    contact["email"].as_str(),
                    contact["phone"].as_str(),
                    value["location"].as_str(),
                    value["website"].as_str(),
                    value["lead_time_days"].as_i64().map(|v| v as i32),
                    capabilities
                ],
            )
            .into_diagnostic()?;
        Ok(())
    }

    pub(super) fn cache_process_data(&self, id: &str, value: &serde_yml::Value) -> Result<()> {
        let equipment = value["equipment"].as_sequence().map(|seq| {
            seq.iter()
                .filter_map(|v| v.as_str())
                .collect::<Vec<_>>()
                .join(",")
        });

        self.conn
            .execute(
                r#"INSERT OR REPLACE INTO processes (id, process_type, equipment) VALUES (?1, ?2, ?3)"#,
                params![id, value["process_type"].as_str(), equipment],
            )
            .into_diagnostic()?;
        Ok(())
    }

    pub(super) fn cache_control_data(&self, id: &str, value: &serde_yml::Value) -> Result<()> {
        self.conn
            .execute(
                r#"INSERT OR REPLACE INTO controls (id, control_type, inspection_method, frequency, process_id)
                   VALUES (?1, ?2, ?3, ?4, ?5)"#,
                params![
                    id,
                    value["control_type"].as_str(),
                    value["inspection_method"].as_str(),
                    value["frequency"].as_str(),
                    value["process"].as_str()
                ],
            )
            .into_diagnostic()?;
        Ok(())
    }

    pub(super) fn cache_work_data(&self, id: &str, value: &serde_yml::Value) -> Result<()> {
        self.conn
            .execute(
                r#"INSERT OR REPLACE INTO works (id, process_id) VALUES (?1, ?2)"#,
                params![id, value["process"].as_str()],
            )
            .into_diagnostic()?;
        Ok(())
    }

    pub(super) fn cache_ncr_data(&self, id: &str, value: &serde_yml::Value) -> Result<()> {
        self.conn
            .execute(
                r#"INSERT OR REPLACE INTO ncrs
                   (id, ncr_type, severity, ncr_status, category, disposition, component_id, process_id)
                   VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"#,
                params![
                    id,
                    value["ncr_type"].as_str(),
                    value["severity"].as_str(),
                    value["ncr_status"].as_str(),
                    value["category"].as_str(),
                    value["disposition"]["decision"].as_str(),
                    value["component"].as_str(),
                    value["process"].as_str()
                ],
            )
            .into_diagnostic()?;
        Ok(())
    }

    pub(super) fn cache_capa_data(&self, id: &str, value: &serde_yml::Value) -> Result<()> {
        self.conn
            .execute(
                r#"INSERT OR REPLACE INTO capas (id, capa_type, capa_status, effectiveness) VALUES (?1, ?2, ?3, ?4)"#,
                params![id, value["capa_type"].as_str(), value["capa_status"].as_str(), value["effectiveness"].as_str()],
            )
            .into_diagnostic()?;
        Ok(())
    }

    pub(super) fn cache_assembly_data(&self, id: &str, value: &serde_yml::Value) -> Result<()> {
        self.conn
            .execute(
                r#"INSERT OR REPLACE INTO assemblies (id, part_number, revision) VALUES (?1, ?2, ?3)"#,
                params![id, value["part_number"].as_str(), value["revision"].as_str()],
            )
            .into_diagnostic()?;
        Ok(())
    }

    pub(super) fn cache_result_data(&self, id: &str, value: &serde_yml::Value) -> Result<()> {
        self.conn
            .execute(
                r#"INSERT OR REPLACE INTO results (id, test_id, verdict, executed_by, executed_date)
                   VALUES (?1, ?2, ?3, ?4, ?5)"#,
                params![
                    id,
                    value["test"].as_str(),
                    value["verdict"].as_str(),
                    value["executed_by"].as_str(),
                    value["executed_date"].as_str()
                ],
            )
            .into_diagnostic()?;
        Ok(())
    }

    pub(super) fn cache_lot_data(&self, id: &str, value: &serde_yml::Value) -> Result<()> {
        self.conn
            .execute(
                r#"INSERT OR REPLACE INTO lots
                   (id, lot_number, quantity, lot_status, product_id, start_date, completion_date)
                   VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"#,
                params![
                    id,
                    value["lot_number"].as_str(),
                    value["quantity"].as_i64(),
                    value["lot_status"].as_str(),
                    value["links"]["product"].as_str(),
                    value["start_date"].as_str(),
                    value["completion_date"].as_str()
                ],
            )
            .into_diagnostic()?;
        Ok(())
    }

    pub(super) fn cache_deviation_data(&self, id: &str, value: &serde_yml::Value) -> Result<()> {
        self.conn
            .execute(
                r#"INSERT OR REPLACE INTO deviations
                   (id, deviation_number, deviation_type, category, dev_status, risk_level,
                    effective_date, expiration_date, approved_by, approval_date)
                   VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)"#,
                params![
                    id,
                    value["deviation_number"].as_str(),
                    value["deviation_type"].as_str(),
                    value["category"].as_str(),
                    value["dev_status"].as_str(),
                    value["risk"]["level"].as_str(),
                    value["effective_date"].as_str(),
                    value["expiration_date"].as_str(),
                    value["approval"]["approved_by"].as_str(),
                    value["approval"]["approval_date"].as_str()
                ],
            )
            .into_diagnostic()?;
        Ok(())
    }
}
