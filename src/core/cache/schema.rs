//! Database schema initialization

use miette::{IntoDiagnostic, Result};
use rusqlite::params;

use super::{EntityCache, SCHEMA_VERSION};

impl EntityCache {
    /// Initialize database schema
    pub(super) fn init_schema(&mut self) -> Result<()> {
        self.conn
            .execute_batch(
                r#"
            -- Schema version tracking
            CREATE TABLE IF NOT EXISTS schema_version (
                version INTEGER PRIMARY KEY
            );

            -- Short ID mappings
            CREATE TABLE IF NOT EXISTS short_ids (
                short_id TEXT PRIMARY KEY,
                entity_id TEXT NOT NULL UNIQUE,
                prefix TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_short_ids_entity ON short_ids(entity_id);
            CREATE INDEX IF NOT EXISTS idx_short_ids_prefix ON short_ids(prefix);

            -- Next available short ID per prefix
            CREATE TABLE IF NOT EXISTS short_id_counters (
                prefix TEXT PRIMARY KEY,
                next_id INTEGER NOT NULL DEFAULT 1
            );

            -- Entity metadata (common fields for all entity types)
            CREATE TABLE IF NOT EXISTS entities (
                id TEXT PRIMARY KEY,
                prefix TEXT NOT NULL,
                title TEXT NOT NULL,
                status TEXT NOT NULL,
                author TEXT NOT NULL,
                created TEXT NOT NULL,
                file_path TEXT NOT NULL,
                file_mtime INTEGER NOT NULL,
                file_hash TEXT NOT NULL,
                priority TEXT,
                entity_type TEXT,
                level TEXT,
                category TEXT,
                tags TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_entities_prefix ON entities(prefix);
            CREATE INDEX IF NOT EXISTS idx_entities_status ON entities(status);
            CREATE INDEX IF NOT EXISTS idx_entities_priority ON entities(priority);
            CREATE INDEX IF NOT EXISTS idx_entities_entity_type ON entities(entity_type);
            CREATE INDEX IF NOT EXISTS idx_entities_category ON entities(category);
            CREATE INDEX IF NOT EXISTS idx_entities_file_path ON entities(file_path);

            -- Feature-specific data
            CREATE TABLE IF NOT EXISTS features (
                id TEXT PRIMARY KEY,
                component_id TEXT NOT NULL,
                feature_type TEXT NOT NULL,
                dim_name TEXT,
                dim_nominal REAL,
                dim_plus_tol REAL,
                dim_minus_tol REAL,
                dim_internal INTEGER,
                FOREIGN KEY (id) REFERENCES entities(id) ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS idx_features_component ON features(component_id);

            -- Component-specific data
            CREATE TABLE IF NOT EXISTS components (
                id TEXT PRIMARY KEY,
                part_number TEXT,
                revision TEXT,
                make_buy TEXT,
                category TEXT,
                FOREIGN KEY (id) REFERENCES entities(id) ON DELETE CASCADE
            );

            -- Risk-specific data
            CREATE TABLE IF NOT EXISTS risks (
                id TEXT PRIMARY KEY,
                risk_type TEXT,
                severity INTEGER,
                occurrence INTEGER,
                detection INTEGER,
                rpn INTEGER,
                risk_level TEXT,
                FOREIGN KEY (id) REFERENCES entities(id) ON DELETE CASCADE
            );

            -- Hazard-specific data
            CREATE TABLE IF NOT EXISTS hazards (
                id TEXT PRIMARY KEY,
                hazard_category TEXT,
                severity TEXT,
                energy_level TEXT,
                exposure_scenario TEXT,
                FOREIGN KEY (id) REFERENCES entities(id) ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS idx_hazards_category ON hazards(hazard_category);
            CREATE INDEX IF NOT EXISTS idx_hazards_severity ON hazards(severity);

            -- Test-specific data
            CREATE TABLE IF NOT EXISTS tests (
                id TEXT PRIMARY KEY,
                test_type TEXT,
                level TEXT,
                method TEXT,
                FOREIGN KEY (id) REFERENCES entities(id) ON DELETE CASCADE
            );

            -- Quote-specific data
            CREATE TABLE IF NOT EXISTS quotes (
                id TEXT PRIMARY KEY,
                quote_status TEXT,
                supplier_id TEXT,
                component_id TEXT,
                unit_price REAL,
                quantity INTEGER,
                lead_time_days INTEGER,
                currency TEXT,
                valid_until TEXT,
                FOREIGN KEY (id) REFERENCES entities(id) ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS idx_quotes_supplier ON quotes(supplier_id);
            CREATE INDEX IF NOT EXISTS idx_quotes_component ON quotes(component_id);
            CREATE INDEX IF NOT EXISTS idx_quotes_status ON quotes(quote_status);

            -- Supplier-specific data
            CREATE TABLE IF NOT EXISTS suppliers (
                id TEXT PRIMARY KEY,
                short_name TEXT,
                contact_name TEXT,
                email TEXT,
                phone TEXT,
                location TEXT,
                website TEXT,
                lead_time_days INTEGER,
                capabilities TEXT,
                FOREIGN KEY (id) REFERENCES entities(id) ON DELETE CASCADE
            );

            -- Process-specific data
            CREATE TABLE IF NOT EXISTS processes (
                id TEXT PRIMARY KEY,
                process_type TEXT,
                equipment TEXT,
                FOREIGN KEY (id) REFERENCES entities(id) ON DELETE CASCADE
            );

            -- Control-specific data
            CREATE TABLE IF NOT EXISTS controls (
                id TEXT PRIMARY KEY,
                control_type TEXT,
                inspection_method TEXT,
                frequency TEXT,
                process_id TEXT,
                FOREIGN KEY (id) REFERENCES entities(id) ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS idx_controls_process ON controls(process_id);

            -- Work instruction-specific data
            CREATE TABLE IF NOT EXISTS works (
                id TEXT PRIMARY KEY,
                process_id TEXT,
                FOREIGN KEY (id) REFERENCES entities(id) ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS idx_works_process ON works(process_id);

            -- NCR-specific data
            CREATE TABLE IF NOT EXISTS ncrs (
                id TEXT PRIMARY KEY,
                ncr_type TEXT,
                severity TEXT,
                ncr_status TEXT,
                category TEXT,
                disposition TEXT,
                component_id TEXT,
                process_id TEXT,
                FOREIGN KEY (id) REFERENCES entities(id) ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS idx_ncrs_ncr_status ON ncrs(ncr_status);
            CREATE INDEX IF NOT EXISTS idx_ncrs_severity ON ncrs(severity);

            -- CAPA-specific data
            CREATE TABLE IF NOT EXISTS capas (
                id TEXT PRIMARY KEY,
                capa_type TEXT,
                capa_status TEXT,
                effectiveness TEXT,
                FOREIGN KEY (id) REFERENCES entities(id) ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS idx_capas_capa_status ON capas(capa_status);

            -- Assembly-specific data
            CREATE TABLE IF NOT EXISTS assemblies (
                id TEXT PRIMARY KEY,
                part_number TEXT,
                revision TEXT,
                FOREIGN KEY (id) REFERENCES entities(id) ON DELETE CASCADE
            );

            -- Result-specific data
            CREATE TABLE IF NOT EXISTS results (
                id TEXT PRIMARY KEY,
                test_id TEXT,
                verdict TEXT,
                executed_by TEXT,
                executed_date TEXT,
                FOREIGN KEY (id) REFERENCES entities(id) ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS idx_results_test ON results(test_id);
            CREATE INDEX IF NOT EXISTS idx_results_verdict ON results(verdict);

            -- Lot-specific data (production batches / DHR)
            CREATE TABLE IF NOT EXISTS lots (
                id TEXT PRIMARY KEY,
                lot_number TEXT,
                quantity INTEGER,
                lot_status TEXT,
                product_id TEXT,
                start_date TEXT,
                completion_date TEXT,
                FOREIGN KEY (id) REFERENCES entities(id) ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS idx_lots_lot_status ON lots(lot_status);
            CREATE INDEX IF NOT EXISTS idx_lots_product ON lots(product_id);

            -- Deviation-specific data (process deviations)
            CREATE TABLE IF NOT EXISTS deviations (
                id TEXT PRIMARY KEY,
                deviation_number TEXT,
                deviation_type TEXT,
                category TEXT,
                dev_status TEXT,
                risk_level TEXT,
                effective_date TEXT,
                expiration_date TEXT,
                approved_by TEXT,
                approval_date TEXT,
                FOREIGN KEY (id) REFERENCES entities(id) ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS idx_deviations_dev_status ON deviations(dev_status);
            CREATE INDEX IF NOT EXISTS idx_deviations_type ON deviations(deviation_type);
            CREATE INDEX IF NOT EXISTS idx_deviations_category ON deviations(category);

            -- Entity links/relationships
            CREATE TABLE IF NOT EXISTS links (
                source_id TEXT NOT NULL,
                target_id TEXT NOT NULL,
                link_type TEXT NOT NULL,
                PRIMARY KEY (source_id, target_id, link_type)
            );
            CREATE INDEX IF NOT EXISTS idx_links_source ON links(source_id);
            CREATE INDEX IF NOT EXISTS idx_links_target ON links(target_id);
            CREATE INDEX IF NOT EXISTS idx_links_type ON links(link_type);

            -- Cache metadata
            CREATE TABLE IF NOT EXISTS cache_meta (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            "#,
            )
            .into_diagnostic()?;

        // Set schema version
        self.conn
            .execute(
                "INSERT OR REPLACE INTO schema_version (version) VALUES (?1)",
                params![SCHEMA_VERSION],
            )
            .into_diagnostic()?;

        Ok(())
    }
}
