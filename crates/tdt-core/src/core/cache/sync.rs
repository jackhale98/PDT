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
            DELETE FROM lots;
            DELETE FROM deviations;
            DELETE FROM links;
            DELETE FROM bom_items;
            DELETE FROM subassembly_items;
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
            "lots",
            "deviations",
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

        // Delete BOM items (for assemblies)
        self.conn
            .execute(
                "DELETE FROM bom_items WHERE assembly_id = ?1 OR component_id = ?1",
                params![id],
            )
            .into_diagnostic()?;

        // Delete subassembly items (for assemblies)
        self.conn
            .execute(
                "DELETE FROM subassembly_items WHERE parent_assembly_id = ?1 OR child_assembly_id = ?1",
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
            ("caused_by", "caused_by"),
            ("controlled_by", "controlled_by"),
            ("controls_hazard", "controls_hazard"),
            ("hazards", "hazards"),
            // Test/Result links
            ("component", "component"),
            ("assembly", "assembly"),
            ("tests", "tests"),
            ("feature", "feature"),
            ("features", "features"),
            ("replaces", "replaces"),
            ("ncrs", "ncrs"),
            ("created_ncr", "ncrs"),
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
            ("modified_by_capa", "modified_by_capa"),
            ("added_by_capa", "added_by_capa"),
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
                } else if let Some(target_obj) = value[field].as_mapping() {
                    // Single-value mapping link: `field: {id: XXX-..., title: ...}`
                    if let Some(target_id) = target_obj
                        .get(serde_yml::Value::String("id".to_string()))
                        .and_then(|v| v.as_str())
                    {
                        links.push((target_id.to_string(), link_type.to_string()));
                    }
                }
                links
            };

        for (field, link_type) in link_fields {
            // Check at top level
            for (target_id, ltype) in extract_links(value, field, link_type) {
                self.insert_link(source_id, &target_id, &ltype, Some(field))?;
            }
            // Also check nested under "links" object
            if let Some(links_obj) = value.get("links") {
                for (target_id, ltype) in extract_links(links_obj, field, link_type) {
                    self.insert_link(source_id, &target_id, &ltype, Some(field))?;
                }
            }
        }

        // Special handling for mate feature references (feature_a.id and feature_b.id)
        if source_id.starts_with("MATE-") {
            if let Some(feature_a) = value.get("feature_a") {
                if let Some(feature_id) = feature_a.get("id").and_then(|v| v.as_str()) {
                    self.insert_link(source_id, feature_id, "feature_a", None)?;
                }
            }
            if let Some(feature_b) = value.get("feature_b") {
                if let Some(feature_id) = feature_b.get("id").and_then(|v| v.as_str()) {
                    self.insert_link(source_id, feature_id, "feature_b", None)?;
                }
            }
        }

        // Special handling for stackup contributor feature references
        if source_id.starts_with("TOL-") {
            if let Some(contributors) = value.get("contributors").and_then(|v| v.as_sequence()) {
                for (i, contrib) in contributors.iter().enumerate() {
                    if let Some(feature) = contrib.get("feature") {
                        if let Some(feature_id) = feature.get("id").and_then(|v| v.as_str()) {
                            self.insert_link(
                                source_id,
                                feature_id,
                                &format!("contributor[{}]", i),
                                None,
                            )?;
                        }
                    }
                }
            }
        }

        // Special handling for assembly BOM and subassemblies
        if source_id.starts_with("ASM-") {
            // Clear existing BOM data for this assembly before re-inserting
            self.conn
                .execute(
                    "DELETE FROM bom_items WHERE assembly_id = ?1",
                    params![source_id],
                )
                .into_diagnostic()?;
            self.conn
                .execute(
                    "DELETE FROM subassembly_items WHERE parent_assembly_id = ?1",
                    params![source_id],
                )
                .into_diagnostic()?;

            // Extract subassembly references with quantities
            if let Some(subassemblies) = value.get("subassemblies").and_then(|v| v.as_sequence()) {
                for subasm in subassemblies {
                    // Subassemblies can be either a simple string ID or an object with id and quantity
                    if let Some(subasm_id) = subasm.as_str() {
                        // Simple string format: assume quantity = 1
                        self.insert_link(source_id, subasm_id, "contains_subassembly", None)?;
                        self.insert_subassembly_item(source_id, subasm_id, 1)?;
                    } else if let Some(subasm_obj) = subasm.as_mapping() {
                        // Object format: {id: "ASM-...", quantity: N}
                        if let Some(subasm_id) = subasm_obj
                            .get(serde_yml::Value::String("id".to_string()))
                            .and_then(|v| v.as_str())
                        {
                            let quantity = subasm_obj
                                .get(serde_yml::Value::String("quantity".to_string()))
                                .and_then(|v| v.as_u64())
                                .unwrap_or(1) as u32;
                            self.insert_link(source_id, subasm_id, "contains_subassembly", None)?;
                            self.insert_subassembly_item(source_id, subasm_id, quantity)?;
                        }
                    }
                }
            }

            // Extract BOM component references with quantities
            if let Some(bom) = value.get("bom").and_then(|v| v.as_sequence()) {
                for item in bom {
                    if let Some(component_id) = item.get("component_id").and_then(|v| v.as_str()) {
                        let quantity =
                            item.get("quantity").and_then(|v| v.as_u64()).unwrap_or(1) as u32;
                        let ref_des = item
                            .get("reference_designators")
                            .and_then(|v| v.as_sequence())
                            .map(|seq| {
                                serde_json::to_string(
                                    &seq.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>(),
                                )
                                .unwrap_or_default()
                            });

                        self.insert_link(source_id, component_id, "contains_component", None)?;
                        self.insert_bom_item(
                            source_id,
                            component_id,
                            quantity,
                            ref_des.as_deref(),
                        )?;
                    }
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
        field_name: Option<&str>,
    ) -> Result<()> {
        self.conn
            .execute(
                "INSERT OR IGNORE INTO links (source_id, target_id, link_type, field_name)                  VALUES (?1, ?2, ?3, ?4)",
                params![source_id, target_id, link_type, field_name],
            )
            .into_diagnostic()?;
        Ok(())
    }

    /// Insert a BOM item (assembly → component relationship with quantity)
    pub(super) fn insert_bom_item(
        &self,
        assembly_id: &str,
        component_id: &str,
        quantity: u32,
        reference_designators: Option<&str>,
    ) -> Result<()> {
        self.conn
            .execute(
                r#"INSERT OR REPLACE INTO bom_items (assembly_id, component_id, quantity, reference_designators)
                   VALUES (?1, ?2, ?3, ?4)"#,
                params![assembly_id, component_id, quantity, reference_designators],
            )
            .into_diagnostic()?;
        Ok(())
    }

    /// Insert a subassembly item (parent assembly → child assembly relationship with quantity)
    pub(super) fn insert_subassembly_item(
        &self,
        parent_assembly_id: &str,
        child_assembly_id: &str,
        quantity: u32,
    ) -> Result<()> {
        self.conn
            .execute(
                r#"INSERT OR REPLACE INTO subassembly_items (parent_assembly_id, child_assembly_id, quantity)
                   VALUES (?1, ?2, ?3)"#,
                params![parent_assembly_id, child_assembly_id, quantity],
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
        let severity = value["severity"].as_i64().map(|v| v as i32);
        let occurrence = value["occurrence"].as_i64().map(|v| v as i32);

        let risk_level: Option<String> = value["risk_level"]
            .as_str()
            .map(|s| s.to_string())
            .or_else(|| match (severity, occurrence) {
                (Some(s), Some(o)) => {
                    let product = s * o;
                    Some(
                        if product >= 64 {
                            "critical"
                        } else if product >= 36 {
                            "high"
                        } else if product >= 16 {
                            "medium"
                        } else {
                            "low"
                        }
                        .to_string(),
                    )
                }
                _ => None,
            });

        self.conn
            .execute(
                r#"INSERT OR REPLACE INTO risks (id, risk_type, severity, occurrence, detection, rpn, risk_level)
                   VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"#,
                params![
                    id,
                    value["type"].as_str(),
                    severity,
                    occurrence,
                    value["detection"].as_i64().map(|v| v as i32),
                    value["rpn"].as_i64().map(|v| v as i32),
                    risk_level
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
                    value["links"]["process"].as_str()
                ],
            )
            .into_diagnostic()?;
        Ok(())
    }

    pub(super) fn cache_work_data(&self, id: &str, value: &serde_yml::Value) -> Result<()> {
        self.conn
            .execute(
                r#"INSERT OR REPLACE INTO works (id, process_id) VALUES (?1, ?2)"#,
                params![id, value["links"]["process"].as_str()],
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
                    value["links"]["component"].as_str(),
                    value["links"]["process"].as_str()
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
        // Try test_id first, fall back to test for backwards compatibility
        let test_id = value["test_id"].as_str().or_else(|| value["test"].as_str());

        self.conn
            .execute(
                r#"INSERT OR REPLACE INTO results (id, test_id, verdict, executed_by, executed_date)
                   VALUES (?1, ?2, ?3, ?4, ?5)"#,
                params![
                    id,
                    test_id,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Project;
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
    fn test_link_field_name_recorded_for_aliases() {
        let (_tmp, project) = create_test_project();

        // `components` is alias-normalized to link_type `contains` in the
        // cache; field_name must record the real YAML field so suspect
        // marking can find the link in the file.
        write_test_entity(
            &project,
            "bom/components/CMP-01AA.tdt.yaml",
            r#"
id: CMP-01AA
title: Parent component
status: draft
author: Test
created: 2024-01-15T10:30:00Z
links:
  components:
    - id: CMP-01BB
      title: Child component
"#,
        );

        let mut cache = EntityCache::open_without_sync(&project).unwrap();
        cache.rebuild().unwrap();

        let links = cache.get_links_from("CMP-01AA");
        let link = links
            .iter()
            .find(|l| l.target_id == "CMP-01BB")
            .expect("child link cached");
        assert_eq!(link.link_type, "contains");
        assert_eq!(link.field_name.as_deref(), Some("components"));
    }

    #[test]
    fn test_extract_links_single_value_mapping() {
        let (_tmp, project) = create_test_project();

        // Single-value MAPPING link (`links: {component: {id: ..., title: ...}}`)
        // as written by `link add`, plus a single-value string link for contrast.
        write_test_entity(
            &project,
            "verification/protocols/TEST-01A.tdt.yaml",
            r#"
id: TEST-01A
title: Test with mapping link
status: draft
author: Test
created: 2024-01-15T10:30:00Z
links:
  component:
    id: CMP-01B
    title: Some Component
  assembly: ASM-01C
"#,
        );

        let mut cache = EntityCache::open_without_sync(&project).unwrap();
        cache.rebuild().unwrap();

        let links = cache.get_links_from("TEST-01A");
        assert!(
            links
                .iter()
                .any(|l| l.target_id == "CMP-01B" && l.link_type == "component"),
            "Expected single-value mapping link to CMP-01B to be cached, got {:?}",
            links
        );
        assert!(
            links
                .iter()
                .any(|l| l.target_id == "ASM-01C" && l.link_type == "assembly"),
            "Expected single-value string link to ASM-01C to be cached, got {:?}",
            links
        );
    }
}
