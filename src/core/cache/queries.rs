//! Query methods for retrieving cached entities
//!
//! Methods for listing and retrieving cached entities with filtering and search.

use std::collections::HashMap;
use std::path::PathBuf;

use rusqlite::{params, OptionalExtension};

use super::{
    parse_datetime, CachedCapa, CachedComponent, CachedControl, CachedEntity, CachedFeature,
    CachedNcr, CachedProcess, CachedQuote, CachedRequirement, CachedResult, CachedRisk,
    CachedSupplier, CachedTest, CachedWork, EntityCache, EntityFilter,
};

impl EntityCache {
    /// Get entity by ID (full or partial match)
    pub fn get_entity(&self, id: &str) -> Option<CachedEntity> {
        // Try exact match first
        let result = self.conn.query_row(
            "SELECT id, prefix, title, status, author, created, file_path, priority, entity_type, category, tags FROM entities WHERE id = ?1",
            params![id],
            |row| {
                let tags_str: Option<String> = row.get(10)?;
                let tags = tags_str
                    .map(|s| s.split(',').filter(|t| !t.is_empty()).map(String::from).collect())
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
            },
        ).optional().ok().flatten();

        if result.is_some() {
            return result;
        }

        // Try partial match
        self.conn.query_row(
            "SELECT id, prefix, title, status, author, created, file_path, priority, entity_type, category, tags FROM entities WHERE id LIKE ?1",
            params![format!("%{}%", id)],
            |row| {
                let tags_str: Option<String> = row.get(10)?;
                let tags = tags_str
                    .map(|s| s.split(',').filter(|t| !t.is_empty()).map(String::from).collect())
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
            },
        ).optional().ok().flatten()
    }

    /// Get feature by ID with dimension data
    pub fn get_feature(&self, id: &str) -> Option<CachedFeature> {
        self.conn
            .query_row(
                r#"SELECT e.id, e.title, e.status, f.component_id, f.feature_type,
                      f.dim_name, f.dim_nominal, f.dim_plus_tol, f.dim_minus_tol, f.dim_internal,
                      e.author, e.created, e.file_path
               FROM features f
               JOIN entities e ON f.id = e.id
               WHERE f.id = ?1"#,
                params![id],
                |row| {
                    Ok(CachedFeature {
                        id: row.get(0)?,
                        title: row.get(1)?,
                        status: row.get(2)?,
                        component_id: row.get(3)?,
                        feature_type: row.get(4)?,
                        dim_name: row.get(5)?,
                        dim_nominal: row.get(6)?,
                        dim_plus_tol: row.get(7)?,
                        dim_minus_tol: row.get(8)?,
                        dim_internal: row.get::<_, Option<i32>>(9)?.map(|v| v != 0),
                        author: row.get(10)?,
                        created: parse_datetime(row.get::<_, String>(11)?),
                        file_path: PathBuf::from(row.get::<_, String>(12)?),
                    })
                },
            )
            .optional()
            .ok()
            .flatten()
    }

    /// Get all features for a component
    pub fn get_features_for_component(&self, component_id: &str) -> Vec<CachedFeature> {
        let mut stmt = match self.conn.prepare(
            r#"SELECT e.id, e.title, e.status, f.component_id, f.feature_type,
                      f.dim_name, f.dim_nominal, f.dim_plus_tol, f.dim_minus_tol, f.dim_internal,
                      e.author, e.created, e.file_path
               FROM features f
               JOIN entities e ON f.id = e.id
               WHERE f.component_id = ?1"#,
        ) {
            Ok(s) => s,
            Err(_) => return vec![],
        };

        let rows = match stmt.query_map(params![component_id], |row| {
            Ok(CachedFeature {
                id: row.get(0)?,
                title: row.get(1)?,
                status: row.get(2)?,
                component_id: row.get(3)?,
                feature_type: row.get(4)?,
                dim_name: row.get(5)?,
                dim_nominal: row.get(6)?,
                dim_plus_tol: row.get(7)?,
                dim_minus_tol: row.get(8)?,
                dim_internal: row.get::<_, Option<i32>>(9)?.map(|v| v != 0),
                author: row.get(10)?,
                created: parse_datetime(row.get::<_, String>(11)?),
                file_path: PathBuf::from(row.get::<_, String>(12)?),
            })
        }) {
            Ok(r) => r,
            Err(_) => return vec![],
        };

        rows.filter_map(|r| r.ok()).collect()
    }

    /// List entities with filters
    pub fn list_entities(&self, filter: &EntityFilter) -> Vec<CachedEntity> {
        let mut sql = String::from(
            "SELECT id, prefix, title, status, author, created, file_path, priority, entity_type, category, tags FROM entities WHERE 1=1",
        );
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = vec![];

        if let Some(ref prefix) = filter.prefix {
            sql.push_str(" AND prefix = ?");
            params_vec.push(Box::new(prefix.as_str().to_string()));
        }

        if let Some(ref status) = filter.status {
            sql.push_str(" AND status = ?");
            params_vec.push(Box::new(status.clone()));
        }

        if let Some(ref author) = filter.author {
            sql.push_str(" AND author = ?");
            params_vec.push(Box::new(author.clone()));
        }

        if let Some(ref priority) = filter.priority {
            sql.push_str(" AND priority = ?");
            params_vec.push(Box::new(priority.clone()));
        }

        if let Some(ref entity_type) = filter.entity_type {
            sql.push_str(" AND entity_type = ?");
            params_vec.push(Box::new(entity_type.clone()));
        }

        if let Some(ref category) = filter.category {
            sql.push_str(" AND category = ?");
            params_vec.push(Box::new(category.clone()));
        }

        if let Some(ref search) = filter.search {
            sql.push_str(" AND (title LIKE ? OR id LIKE ?)");
            let pattern = format!("%{}%", search);
            params_vec.push(Box::new(pattern.clone()));
            params_vec.push(Box::new(pattern));
        }

        sql.push_str(" ORDER BY created DESC");

        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }

        let mut stmt = match self.conn.prepare(&sql) {
            Ok(s) => s,
            Err(_) => return vec![],
        };

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();

        let rows = match stmt.query_map(params_refs.as_slice(), |row| {
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

    /// List suppliers with filtering
    /// Returns cached supplier data with support for status, capability, author filters
    pub fn list_suppliers(
        &self,
        status: Option<&str>,
        capability: Option<&str>,
        author: Option<&str>,
        search: Option<&str>,
        limit: Option<usize>,
    ) -> Vec<CachedSupplier> {
        let mut sql = String::from(
            r#"SELECT e.id, e.title, s.short_name, e.status, e.author, e.created,
                      s.website, s.capabilities, s.lead_time_days, e.file_path
               FROM entities e
               JOIN suppliers s ON e.id = s.id
               WHERE e.prefix = 'SUP'"#,
        );
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = vec![];

        if let Some(status) = status {
            sql.push_str(" AND e.status = ?");
            params_vec.push(Box::new(status.to_string()));
        }

        if let Some(capability) = capability {
            // Match capability in comma-separated list
            sql.push_str(" AND (',' || s.capabilities || ',' LIKE ?)");
            params_vec.push(Box::new(format!("%,{},%", capability)));
        }

        if let Some(author) = author {
            sql.push_str(" AND e.author LIKE ?");
            params_vec.push(Box::new(format!("%{}%", author)));
        }

        if let Some(search) = search {
            sql.push_str(" AND (e.title LIKE ? OR e.id LIKE ?)");
            let pattern = format!("%{}%", search);
            params_vec.push(Box::new(pattern.clone()));
            params_vec.push(Box::new(pattern));
        }

        sql.push_str(" ORDER BY e.title ASC");

        if let Some(limit) = limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }

        let mut stmt = match self.conn.prepare(&sql) {
            Ok(s) => s,
            Err(_) => return vec![],
        };

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();

        let rows = match stmt.query_map(params_refs.as_slice(), |row| {
            let caps_str: Option<String> = row.get(7)?;
            let capabilities = caps_str
                .map(|s| {
                    s.split(',')
                        .filter(|t| !t.is_empty())
                        .map(String::from)
                        .collect()
                })
                .unwrap_or_default();
            Ok(CachedSupplier {
                id: row.get(0)?,
                name: row.get(1)?,
                short_name: row.get(2)?,
                status: row.get(3)?,
                author: row.get(4)?,
                created: parse_datetime(row.get::<_, String>(5)?),
                website: row.get(6)?,
                capabilities,
                lead_time_days: row.get(8)?,
                file_path: PathBuf::from(row.get::<_, String>(9)?),
            })
        }) {
            Ok(r) => r,
            Err(_) => return vec![],
        };

        rows.filter_map(|r| r.ok()).collect()
    }

    /// List requirements with filtering
    pub fn list_requirements(
        &self,
        status: Option<&str>,
        priority: Option<&str>,
        req_type: Option<&str>,
        category: Option<&str>,
        author: Option<&str>,
        search: Option<&str>,
        limit: Option<usize>,
    ) -> Vec<CachedRequirement> {
        let mut sql = String::from(
            r#"SELECT e.id, e.title, e.status, e.priority, e.entity_type, e.level, e.category,
                      e.author, e.created, e.tags, e.file_path
               FROM entities e
               WHERE e.prefix = 'REQ'"#,
        );
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = vec![];

        if let Some(status) = status {
            sql.push_str(" AND e.status = ?");
            params_vec.push(Box::new(status.to_string()));
        }

        if let Some(priority) = priority {
            sql.push_str(" AND e.priority = ?");
            params_vec.push(Box::new(priority.to_string()));
        }

        if let Some(req_type) = req_type {
            sql.push_str(" AND e.entity_type = ?");
            params_vec.push(Box::new(req_type.to_string()));
        }

        if let Some(category) = category {
            sql.push_str(" AND e.category = ?");
            params_vec.push(Box::new(category.to_string()));
        }

        if let Some(author) = author {
            sql.push_str(" AND e.author LIKE ?");
            params_vec.push(Box::new(format!("%{}%", author)));
        }

        if let Some(search) = search {
            sql.push_str(" AND (e.title LIKE ? OR e.id LIKE ?)");
            let pattern = format!("%{}%", search);
            params_vec.push(Box::new(pattern.clone()));
            params_vec.push(Box::new(pattern));
        }

        sql.push_str(" ORDER BY e.created DESC");

        if let Some(limit) = limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }

        let mut stmt = match self.conn.prepare(&sql) {
            Ok(s) => s,
            Err(_) => return vec![],
        };

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();

        let rows = match stmt.query_map(params_refs.as_slice(), |row| {
            let tags_str: Option<String> = row.get(9)?;
            let tags = tags_str
                .map(|s| {
                    s.split(',')
                        .filter(|t| !t.is_empty())
                        .map(String::from)
                        .collect()
                })
                .unwrap_or_default();
            Ok(CachedRequirement {
                id: row.get(0)?,
                title: row.get(1)?,
                status: row.get(2)?,
                priority: row.get(3)?,
                req_type: row.get(4)?,
                level: row.get(5)?,
                category: row.get(6)?,
                author: row.get(7)?,
                created: parse_datetime(row.get::<_, String>(8)?),
                tags,
                file_path: PathBuf::from(row.get::<_, String>(10)?),
            })
        }) {
            Ok(r) => r,
            Err(_) => return vec![],
        };

        rows.filter_map(|r| r.ok()).collect()
    }

    /// List components with filtering
    pub fn list_components(
        &self,
        status: Option<&str>,
        make_buy: Option<&str>,
        category: Option<&str>,
        author: Option<&str>,
        search: Option<&str>,
        limit: Option<usize>,
    ) -> Vec<CachedComponent> {
        let mut sql = String::from(
            r#"SELECT e.id, e.title, e.status, c.part_number, c.revision, c.make_buy,
                      c.category, e.author, e.created, e.file_path
               FROM entities e
               JOIN components c ON e.id = c.id
               WHERE e.prefix = 'CMP'"#,
        );
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = vec![];

        if let Some(status) = status {
            sql.push_str(" AND e.status = ?");
            params_vec.push(Box::new(status.to_string()));
        }

        if let Some(make_buy) = make_buy {
            sql.push_str(" AND c.make_buy = ?");
            params_vec.push(Box::new(make_buy.to_string()));
        }

        if let Some(category) = category {
            sql.push_str(" AND c.category = ?");
            params_vec.push(Box::new(category.to_string()));
        }

        if let Some(author) = author {
            sql.push_str(" AND e.author LIKE ?");
            params_vec.push(Box::new(format!("%{}%", author)));
        }

        if let Some(search) = search {
            sql.push_str(" AND (e.title LIKE ? OR e.id LIKE ? OR c.part_number LIKE ?)");
            let pattern = format!("%{}%", search);
            params_vec.push(Box::new(pattern.clone()));
            params_vec.push(Box::new(pattern.clone()));
            params_vec.push(Box::new(pattern));
        }

        sql.push_str(" ORDER BY e.title ASC");

        if let Some(limit) = limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }

        let mut stmt = match self.conn.prepare(&sql) {
            Ok(s) => s,
            Err(_) => return vec![],
        };

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();

        let rows = match stmt.query_map(params_refs.as_slice(), |row| {
            Ok(CachedComponent {
                id: row.get(0)?,
                title: row.get(1)?,
                status: row.get(2)?,
                part_number: row.get(3)?,
                revision: row.get(4)?,
                make_buy: row.get(5)?,
                category: row.get(6)?,
                author: row.get(7)?,
                created: parse_datetime(row.get::<_, String>(8)?),
                file_path: PathBuf::from(row.get::<_, String>(9)?),
            })
        }) {
            Ok(r) => r,
            Err(_) => return vec![],
        };

        rows.filter_map(|r| r.ok()).collect()
    }

    /// List tests with filtering
    pub fn list_tests(
        &self,
        status: Option<&str>,
        test_type: Option<&str>,
        level: Option<&str>,
        method: Option<&str>,
        priority: Option<&str>,
        category: Option<&str>,
        author: Option<&str>,
        search: Option<&str>,
        limit: Option<usize>,
    ) -> Vec<CachedTest> {
        let mut sql = String::from(
            r#"SELECT e.id, e.title, e.status, t.test_type, t.level, t.method,
                      e.priority, e.category, e.author, e.created, e.file_path
               FROM entities e
               JOIN tests t ON e.id = t.id
               WHERE e.prefix = 'TEST'"#,
        );
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = vec![];

        if let Some(status) = status {
            sql.push_str(" AND e.status = ?");
            params_vec.push(Box::new(status.to_string()));
        }

        if let Some(test_type) = test_type {
            sql.push_str(" AND t.test_type = ?");
            params_vec.push(Box::new(test_type.to_string()));
        }

        if let Some(level) = level {
            sql.push_str(" AND t.level = ?");
            params_vec.push(Box::new(level.to_string()));
        }

        if let Some(method) = method {
            sql.push_str(" AND t.method = ?");
            params_vec.push(Box::new(method.to_string()));
        }

        if let Some(priority) = priority {
            sql.push_str(" AND e.priority = ?");
            params_vec.push(Box::new(priority.to_string()));
        }

        if let Some(category) = category {
            sql.push_str(" AND e.category = ?");
            params_vec.push(Box::new(category.to_string()));
        }

        if let Some(author) = author {
            sql.push_str(" AND e.author LIKE ?");
            params_vec.push(Box::new(format!("%{}%", author)));
        }

        if let Some(search) = search {
            sql.push_str(" AND (e.title LIKE ? OR e.id LIKE ?)");
            let pattern = format!("%{}%", search);
            params_vec.push(Box::new(pattern.clone()));
            params_vec.push(Box::new(pattern));
        }

        sql.push_str(" ORDER BY e.created DESC");

        if let Some(limit) = limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }

        let mut stmt = match self.conn.prepare(&sql) {
            Ok(s) => s,
            Err(_) => return vec![],
        };

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();

        let rows = match stmt.query_map(params_refs.as_slice(), |row| {
            Ok(CachedTest {
                id: row.get(0)?,
                title: row.get(1)?,
                status: row.get(2)?,
                test_type: row.get(3)?,
                level: row.get(4)?,
                method: row.get(5)?,
                priority: row.get(6)?,
                category: row.get(7)?,
                author: row.get(8)?,
                created: parse_datetime(row.get::<_, String>(9)?),
                file_path: PathBuf::from(row.get::<_, String>(10)?),
            })
        }) {
            Ok(r) => r,
            Err(_) => return vec![],
        };

        rows.filter_map(|r| r.ok()).collect()
    }

    /// List quotes with filtering
    pub fn list_quotes(
        &self,
        status: Option<&str>,
        quote_status: Option<&str>,
        supplier_id: Option<&str>,
        component_id: Option<&str>,
        author: Option<&str>,
        search: Option<&str>,
        limit: Option<usize>,
    ) -> Vec<CachedQuote> {
        let mut sql = String::from(
            r#"SELECT e.id, e.title, e.status, q.quote_status, q.supplier_id, q.component_id, q.unit_price,
                      q.quantity, q.lead_time_days, q.currency, q.valid_until, e.author, e.created, e.file_path
               FROM entities e
               JOIN quotes q ON e.id = q.id
               WHERE e.prefix = 'QUOT'"#,
        );
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = vec![];

        if let Some(status) = status {
            sql.push_str(" AND e.status = ?");
            params_vec.push(Box::new(status.to_string()));
        }

        if let Some(quote_status) = quote_status {
            sql.push_str(" AND q.quote_status = ?");
            params_vec.push(Box::new(quote_status.to_string()));
        }

        if let Some(supplier_id) = supplier_id {
            sql.push_str(" AND q.supplier_id = ?");
            params_vec.push(Box::new(supplier_id.to_string()));
        }

        if let Some(component_id) = component_id {
            sql.push_str(" AND q.component_id = ?");
            params_vec.push(Box::new(component_id.to_string()));
        }

        if let Some(author) = author {
            sql.push_str(" AND e.author LIKE ?");
            params_vec.push(Box::new(format!("%{}%", author)));
        }

        if let Some(search) = search {
            sql.push_str(" AND (e.title LIKE ? OR e.id LIKE ?)");
            let pattern = format!("%{}%", search);
            params_vec.push(Box::new(pattern.clone()));
            params_vec.push(Box::new(pattern));
        }

        sql.push_str(" ORDER BY e.created DESC");

        if let Some(limit) = limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }

        let mut stmt = match self.conn.prepare(&sql) {
            Ok(s) => s,
            Err(_) => return vec![],
        };

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();

        let rows = match stmt.query_map(params_refs.as_slice(), |row| {
            Ok(CachedQuote {
                id: row.get(0)?,
                title: row.get(1)?,
                status: row.get(2)?,
                quote_status: row.get(3)?,
                supplier_id: row.get(4)?,
                component_id: row.get(5)?,
                unit_price: row.get(6)?,
                quantity: row.get(7)?,
                lead_time_days: row.get(8)?,
                currency: row.get(9)?,
                valid_until: row.get(10)?,
                author: row.get(11)?,
                created: parse_datetime(row.get::<_, String>(12)?),
                file_path: PathBuf::from(row.get::<_, String>(13)?),
            })
        }) {
            Ok(r) => r,
            Err(_) => return vec![],
        };

        rows.filter_map(|r| r.ok()).collect()
    }

    /// List NCRs with filtering
    pub fn list_ncrs(
        &self,
        status: Option<&str>,
        ncr_type: Option<&str>,
        severity: Option<&str>,
        ncr_status: Option<&str>,
        category: Option<&str>,
        author: Option<&str>,
        limit: Option<usize>,
    ) -> Vec<CachedNcr> {
        let mut sql = String::from(
            r#"SELECT e.id, e.title, e.status, n.ncr_type, n.severity, n.ncr_status,
                      e.category, e.author, e.created, e.file_path
               FROM entities e
               JOIN ncrs n ON e.id = n.id
               WHERE e.prefix = 'NCR'"#,
        );
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = vec![];

        if let Some(status) = status {
            sql.push_str(" AND e.status = ?");
            params_vec.push(Box::new(status.to_string()));
        }

        if let Some(ncr_type) = ncr_type {
            sql.push_str(" AND n.ncr_type = ?");
            params_vec.push(Box::new(ncr_type.to_string()));
        }

        if let Some(severity) = severity {
            sql.push_str(" AND n.severity = ?");
            params_vec.push(Box::new(severity.to_string()));
        }

        if let Some(ncr_status) = ncr_status {
            sql.push_str(" AND n.ncr_status = ?");
            params_vec.push(Box::new(ncr_status.to_string()));
        }

        if let Some(category) = category {
            sql.push_str(" AND e.category = ?");
            params_vec.push(Box::new(category.to_string()));
        }

        if let Some(author) = author {
            sql.push_str(" AND e.author LIKE ?");
            params_vec.push(Box::new(format!("%{}%", author)));
        }

        sql.push_str(" ORDER BY e.created DESC");

        if let Some(limit) = limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }

        let mut stmt = match self.conn.prepare(&sql) {
            Ok(s) => s,
            Err(_) => return vec![],
        };

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();

        let rows = match stmt.query_map(params_refs.as_slice(), |row| {
            Ok(CachedNcr {
                id: row.get(0)?,
                title: row.get(1)?,
                status: row.get(2)?,
                ncr_type: row.get(3)?,
                severity: row.get(4)?,
                ncr_status: row.get(5)?,
                category: row.get(6)?,
                author: row.get(7)?,
                created: parse_datetime(row.get::<_, String>(8)?),
                file_path: PathBuf::from(row.get::<_, String>(9)?),
            })
        }) {
            Ok(r) => r,
            Err(_) => return vec![],
        };

        rows.filter_map(|r| r.ok()).collect()
    }

    /// List CAPAs with filtering
    pub fn list_capas(
        &self,
        status: Option<&str>,
        capa_type: Option<&str>,
        capa_status: Option<&str>,
        author: Option<&str>,
        limit: Option<usize>,
    ) -> Vec<CachedCapa> {
        let mut sql = String::from(
            r#"SELECT e.id, e.title, e.status, c.capa_type, c.capa_status,
                      e.author, e.created, e.file_path
               FROM entities e
               JOIN capas c ON e.id = c.id
               WHERE e.prefix = 'CAPA'"#,
        );
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = vec![];

        if let Some(status) = status {
            sql.push_str(" AND e.status = ?");
            params_vec.push(Box::new(status.to_string()));
        }

        if let Some(capa_type) = capa_type {
            sql.push_str(" AND c.capa_type = ?");
            params_vec.push(Box::new(capa_type.to_string()));
        }

        if let Some(capa_status) = capa_status {
            sql.push_str(" AND c.capa_status = ?");
            params_vec.push(Box::new(capa_status.to_string()));
        }

        if let Some(author) = author {
            sql.push_str(" AND e.author LIKE ?");
            params_vec.push(Box::new(format!("%{}%", author)));
        }

        sql.push_str(" ORDER BY e.created DESC");

        if let Some(limit) = limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }

        let mut stmt = match self.conn.prepare(&sql) {
            Ok(s) => s,
            Err(_) => return vec![],
        };

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();

        let rows = match stmt.query_map(params_refs.as_slice(), |row| {
            Ok(CachedCapa {
                id: row.get(0)?,
                title: row.get(1)?,
                status: row.get(2)?,
                capa_type: row.get(3)?,
                capa_status: row.get(4)?,
                author: row.get(5)?,
                created: parse_datetime(row.get::<_, String>(6)?),
                file_path: PathBuf::from(row.get::<_, String>(7)?),
            })
        }) {
            Ok(r) => r,
            Err(_) => return vec![],
        };

        rows.filter_map(|r| r.ok()).collect()
    }

    /// List risks with filtering
    pub fn list_risks(
        &self,
        status: Option<&str>,
        risk_type: Option<&str>,
        risk_level: Option<&str>,
        category: Option<&str>,
        min_rpn: Option<i32>,
        author: Option<&str>,
        search: Option<&str>,
        limit: Option<usize>,
    ) -> Vec<CachedRisk> {
        let mut sql = String::from(
            r#"SELECT e.id, e.title, e.status, r.risk_type, r.severity, r.occurrence, r.detection,
                      r.rpn, r.risk_level, e.category, e.author, e.created, e.file_path
               FROM entities e
               JOIN risks r ON e.id = r.id
               WHERE e.prefix = 'RISK'"#,
        );
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = vec![];

        if let Some(status) = status {
            sql.push_str(" AND e.status = ?");
            params_vec.push(Box::new(status.to_string()));
        }

        if let Some(risk_type) = risk_type {
            sql.push_str(" AND r.risk_type = ?");
            params_vec.push(Box::new(risk_type.to_string()));
        }

        if let Some(risk_level) = risk_level {
            sql.push_str(" AND r.risk_level = ?");
            params_vec.push(Box::new(risk_level.to_string()));
        }

        if let Some(category) = category {
            sql.push_str(" AND e.category = ?");
            params_vec.push(Box::new(category.to_string()));
        }

        if let Some(min_rpn) = min_rpn {
            sql.push_str(" AND r.rpn >= ?");
            params_vec.push(Box::new(min_rpn));
        }

        if let Some(author) = author {
            sql.push_str(" AND e.author LIKE ?");
            params_vec.push(Box::new(format!("%{}%", author)));
        }

        if let Some(search) = search {
            sql.push_str(" AND (e.title LIKE ? OR e.id LIKE ?)");
            let pattern = format!("%{}%", search);
            params_vec.push(Box::new(pattern.clone()));
            params_vec.push(Box::new(pattern));
        }

        sql.push_str(" ORDER BY r.rpn DESC, e.created DESC");

        if let Some(limit) = limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }

        let mut stmt = match self.conn.prepare(&sql) {
            Ok(s) => s,
            Err(_) => return vec![],
        };

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();

        let rows = match stmt.query_map(params_refs.as_slice(), |row| {
            Ok(CachedRisk {
                id: row.get(0)?,
                title: row.get(1)?,
                status: row.get(2)?,
                risk_type: row.get(3)?,
                severity: row.get(4)?,
                occurrence: row.get(5)?,
                detection: row.get(6)?,
                rpn: row.get(7)?,
                risk_level: row.get(8)?,
                category: row.get(9)?,
                author: row.get(10)?,
                created: parse_datetime(row.get::<_, String>(11)?),
                file_path: PathBuf::from(row.get::<_, String>(12)?),
            })
        }) {
            Ok(r) => r,
            Err(_) => return vec![],
        };

        rows.filter_map(|r| r.ok()).collect()
    }

    /// List features with filtering
    pub fn list_features(
        &self,
        status: Option<&str>,
        feature_type: Option<&str>,
        component_id: Option<&str>,
        author: Option<&str>,
        search: Option<&str>,
        limit: Option<usize>,
    ) -> Vec<CachedFeature> {
        let mut sql = String::from(
            r#"SELECT e.id, e.title, e.status, f.component_id, f.feature_type,
                      f.dim_name, f.dim_nominal, f.dim_plus_tol, f.dim_minus_tol, f.dim_internal,
                      e.author, e.created, e.file_path
               FROM entities e
               JOIN features f ON e.id = f.id
               WHERE e.prefix = 'FEAT'"#,
        );
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = vec![];

        if let Some(status) = status {
            sql.push_str(" AND e.status = ?");
            params_vec.push(Box::new(status.to_string()));
        }

        if let Some(feature_type) = feature_type {
            sql.push_str(" AND f.feature_type = ?");
            params_vec.push(Box::new(feature_type.to_string()));
        }

        if let Some(component_id) = component_id {
            sql.push_str(" AND f.component_id = ?");
            params_vec.push(Box::new(component_id.to_string()));
        }

        if let Some(author) = author {
            sql.push_str(" AND e.author LIKE ?");
            params_vec.push(Box::new(format!("%{}%", author)));
        }

        if let Some(search) = search {
            sql.push_str(" AND (e.title LIKE ? OR e.id LIKE ?)");
            let pattern = format!("%{}%", search);
            params_vec.push(Box::new(pattern.clone()));
            params_vec.push(Box::new(pattern));
        }

        sql.push_str(" ORDER BY e.created DESC");

        if let Some(limit) = limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }

        let mut stmt = match self.conn.prepare(&sql) {
            Ok(s) => s,
            Err(_) => return vec![],
        };

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();

        let rows = match stmt.query_map(params_refs.as_slice(), |row| {
            Ok(CachedFeature {
                id: row.get(0)?,
                title: row.get(1)?,
                status: row.get(2)?,
                component_id: row.get(3)?,
                feature_type: row.get(4)?,
                dim_name: row.get(5)?,
                dim_nominal: row.get(6)?,
                dim_plus_tol: row.get(7)?,
                dim_minus_tol: row.get(8)?,
                dim_internal: row.get(9)?,
                author: row.get(10)?,
                created: parse_datetime(row.get::<_, String>(11)?),
                file_path: PathBuf::from(row.get::<_, String>(12)?),
            })
        }) {
            Ok(r) => r,
            Err(_) => return vec![],
        };

        rows.filter_map(|r| r.ok()).collect()
    }

    /// Get all cached features (for validation)
    pub fn get_all_features(&self) -> HashMap<String, CachedFeature> {
        let mut result = HashMap::new();

        let mut stmt = match self.conn.prepare(
            r#"SELECT e.id, e.title, e.status, f.component_id, f.feature_type,
                      f.dim_name, f.dim_nominal, f.dim_plus_tol, f.dim_minus_tol, f.dim_internal,
                      e.author, e.created, e.file_path
               FROM features f
               JOIN entities e ON f.id = e.id"#,
        ) {
            Ok(s) => s,
            Err(_) => return result,
        };

        let rows = match stmt.query_map([], |row| {
            Ok(CachedFeature {
                id: row.get(0)?,
                title: row.get(1)?,
                status: row.get(2)?,
                component_id: row.get(3)?,
                feature_type: row.get(4)?,
                dim_name: row.get(5)?,
                dim_nominal: row.get(6)?,
                dim_plus_tol: row.get(7)?,
                dim_minus_tol: row.get(8)?,
                dim_internal: row.get::<_, Option<i32>>(9)?.map(|v| v != 0),
                author: row.get(10)?,
                created: parse_datetime(row.get::<_, String>(11)?),
                file_path: PathBuf::from(row.get::<_, String>(12)?),
            })
        }) {
            Ok(r) => r,
            Err(_) => return result,
        };

        for row in rows.flatten() {
            result.insert(row.id.clone(), row);
        }

        result
    }

    /// List processes with filtering
    pub fn list_processes(
        &self,
        status: Option<&str>,
        process_type: Option<&str>,
        category: Option<&str>,
        author: Option<&str>,
        search: Option<&str>,
        limit: Option<usize>,
    ) -> Vec<CachedProcess> {
        let mut sql = String::from(
            r#"SELECT e.id, e.title, e.status, p.process_type, e.category,
                      e.author, e.created, e.file_path
               FROM entities e
               LEFT JOIN processes p ON e.id = p.id
               WHERE e.prefix = 'PROC'"#,
        );
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = vec![];

        if let Some(status) = status {
            sql.push_str(" AND e.status = ?");
            params_vec.push(Box::new(status.to_string()));
        }

        if let Some(process_type) = process_type {
            sql.push_str(" AND p.process_type = ?");
            params_vec.push(Box::new(process_type.to_string()));
        }

        if let Some(category) = category {
            sql.push_str(" AND e.category = ?");
            params_vec.push(Box::new(category.to_string()));
        }

        if let Some(author) = author {
            sql.push_str(" AND e.author LIKE ?");
            params_vec.push(Box::new(format!("%{}%", author)));
        }

        if let Some(search) = search {
            sql.push_str(" AND (e.title LIKE ? OR e.id LIKE ?)");
            let pattern = format!("%{}%", search);
            params_vec.push(Box::new(pattern.clone()));
            params_vec.push(Box::new(pattern));
        }

        sql.push_str(" ORDER BY e.created DESC");

        if let Some(limit) = limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }

        let mut stmt = match self.conn.prepare(&sql) {
            Ok(s) => s,
            Err(_) => return vec![],
        };

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();

        let rows = match stmt.query_map(params_refs.as_slice(), |row| {
            Ok(CachedProcess {
                id: row.get(0)?,
                title: row.get(1)?,
                status: row.get(2)?,
                process_type: row.get(3)?,
                category: row.get(4)?,
                author: row.get(5)?,
                created: parse_datetime(row.get::<_, String>(6)?),
                file_path: PathBuf::from(row.get::<_, String>(7)?),
            })
        }) {
            Ok(r) => r,
            Err(_) => return vec![],
        };

        rows.filter_map(|r| r.ok()).collect()
    }

    /// List controls with filtering
    pub fn list_controls(
        &self,
        status: Option<&str>,
        control_type: Option<&str>,
        process_id: Option<&str>,
        category: Option<&str>,
        author: Option<&str>,
        search: Option<&str>,
        limit: Option<usize>,
    ) -> Vec<CachedControl> {
        let mut sql = String::from(
            r#"SELECT e.id, e.title, e.status, c.control_type, c.process_id, e.category,
                      e.author, e.created, e.file_path
               FROM entities e
               LEFT JOIN controls c ON e.id = c.id
               WHERE e.prefix = 'CTRL'"#,
        );
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = vec![];

        if let Some(status) = status {
            sql.push_str(" AND e.status = ?");
            params_vec.push(Box::new(status.to_string()));
        }

        if let Some(control_type) = control_type {
            sql.push_str(" AND c.control_type = ?");
            params_vec.push(Box::new(control_type.to_string()));
        }

        if let Some(process_id) = process_id {
            sql.push_str(" AND c.process_id = ?");
            params_vec.push(Box::new(process_id.to_string()));
        }

        if let Some(category) = category {
            sql.push_str(" AND e.category = ?");
            params_vec.push(Box::new(category.to_string()));
        }

        if let Some(author) = author {
            sql.push_str(" AND e.author LIKE ?");
            params_vec.push(Box::new(format!("%{}%", author)));
        }

        if let Some(search) = search {
            sql.push_str(" AND (e.title LIKE ? OR e.id LIKE ?)");
            let pattern = format!("%{}%", search);
            params_vec.push(Box::new(pattern.clone()));
            params_vec.push(Box::new(pattern));
        }

        sql.push_str(" ORDER BY e.created DESC");

        if let Some(limit) = limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }

        let mut stmt = match self.conn.prepare(&sql) {
            Ok(s) => s,
            Err(_) => return vec![],
        };

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();

        let rows = match stmt.query_map(params_refs.as_slice(), |row| {
            Ok(CachedControl {
                id: row.get(0)?,
                title: row.get(1)?,
                status: row.get(2)?,
                control_type: row.get(3)?,
                process_id: row.get(4)?,
                category: row.get(5)?,
                author: row.get(6)?,
                created: parse_datetime(row.get::<_, String>(7)?),
                file_path: PathBuf::from(row.get::<_, String>(8)?),
            })
        }) {
            Ok(r) => r,
            Err(_) => return vec![],
        };

        rows.filter_map(|r| r.ok()).collect()
    }

    /// List work instructions with filtering
    pub fn list_works(
        &self,
        status: Option<&str>,
        process_id: Option<&str>,
        author: Option<&str>,
        search: Option<&str>,
        limit: Option<usize>,
    ) -> Vec<CachedWork> {
        let mut sql = String::from(
            r#"SELECT e.id, e.title, e.status, w.process_id,
                      e.author, e.created, e.file_path
               FROM entities e
               LEFT JOIN works w ON e.id = w.id
               WHERE e.prefix = 'WORK'"#,
        );
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = vec![];

        if let Some(status) = status {
            sql.push_str(" AND e.status = ?");
            params_vec.push(Box::new(status.to_string()));
        }

        if let Some(process_id) = process_id {
            sql.push_str(" AND w.process_id = ?");
            params_vec.push(Box::new(process_id.to_string()));
        }

        if let Some(author) = author {
            sql.push_str(" AND e.author LIKE ?");
            params_vec.push(Box::new(format!("%{}%", author)));
        }

        if let Some(search) = search {
            sql.push_str(" AND (e.title LIKE ? OR e.id LIKE ?)");
            let pattern = format!("%{}%", search);
            params_vec.push(Box::new(pattern.clone()));
            params_vec.push(Box::new(pattern));
        }

        sql.push_str(" ORDER BY e.created DESC");

        if let Some(limit) = limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }

        let mut stmt = match self.conn.prepare(&sql) {
            Ok(s) => s,
            Err(_) => return vec![],
        };

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();

        let rows = match stmt.query_map(params_refs.as_slice(), |row| {
            Ok(CachedWork {
                id: row.get(0)?,
                title: row.get(1)?,
                status: row.get(2)?,
                process_id: row.get(3)?,
                author: row.get(4)?,
                created: parse_datetime(row.get::<_, String>(5)?),
                file_path: PathBuf::from(row.get::<_, String>(6)?),
            })
        }) {
            Ok(r) => r,
            Err(_) => return vec![],
        };

        rows.filter_map(|r| r.ok()).collect()
    }

    /// List test results with filtering
    pub fn list_results(
        &self,
        status: Option<&str>,
        test_id: Option<&str>,
        verdict: Option<&str>,
        author: Option<&str>,
        search: Option<&str>,
        limit: Option<usize>,
    ) -> Vec<CachedResult> {
        let mut sql = String::from(
            r#"SELECT e.id, e.title, e.status, r.test_id, r.verdict,
                      r.executed_by, r.executed_date, e.author, e.created, e.file_path
               FROM entities e
               LEFT JOIN results r ON e.id = r.id
               WHERE e.prefix = 'RSLT'"#,
        );
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = vec![];

        if let Some(status) = status {
            sql.push_str(" AND e.status = ?");
            params_vec.push(Box::new(status.to_string()));
        }

        if let Some(test_id) = test_id {
            sql.push_str(" AND r.test_id = ?");
            params_vec.push(Box::new(test_id.to_string()));
        }

        if let Some(verdict) = verdict {
            sql.push_str(" AND r.verdict = ?");
            params_vec.push(Box::new(verdict.to_string()));
        }

        if let Some(author) = author {
            sql.push_str(" AND e.author LIKE ?");
            params_vec.push(Box::new(format!("%{}%", author)));
        }

        if let Some(search) = search {
            sql.push_str(" AND (e.title LIKE ? OR e.id LIKE ?)");
            let pattern = format!("%{}%", search);
            params_vec.push(Box::new(pattern.clone()));
            params_vec.push(Box::new(pattern));
        }

        sql.push_str(" ORDER BY e.created DESC");

        if let Some(limit) = limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }

        let mut stmt = match self.conn.prepare(&sql) {
            Ok(s) => s,
            Err(_) => return vec![],
        };

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();

        let rows = match stmt.query_map(params_refs.as_slice(), |row| {
            Ok(CachedResult {
                id: row.get(0)?,
                title: row.get(1)?,
                status: row.get(2)?,
                test_id: row.get(3)?,
                verdict: row.get(4)?,
                executed_by: row.get(5)?,
                executed_date: row.get(6)?,
                author: row.get(7)?,
                created: parse_datetime(row.get::<_, String>(8)?),
                file_path: PathBuf::from(row.get::<_, String>(9)?),
            })
        }) {
            Ok(r) => r,
            Err(_) => return vec![],
        };

        rows.filter_map(|r| r.ok()).collect()
    }

    // =========================================================================
    // Aggregate Query Methods
    // =========================================================================

    // =========================================================================
    // Global Search Methods
    // =========================================================================

    /// Search across all entity types
    ///
    /// Searches in title field across all entities in the cache.
    /// Supports filtering by entity type prefixes, status, author, and tag.
    pub fn search_all(
        &self,
        query: &str,
        type_prefixes: Option<&[&str]>,
        status: Option<&str>,
        author: Option<&str>,
        tag: Option<&str>,
        case_sensitive: bool,
        limit: usize,
    ) -> Vec<super::SearchResult> {
        let mut sql = String::from(
            r#"SELECT e.id, e.prefix, e.title, e.status, e.author
               FROM entities e
               WHERE 1=1"#,
        );

        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        // Search query (title)
        if case_sensitive {
            sql.push_str(" AND e.title LIKE ?");
            params_vec.push(Box::new(format!("%{}%", query)));
        } else {
            sql.push_str(" AND LOWER(e.title) LIKE LOWER(?)");
            params_vec.push(Box::new(format!("%{}%", query)));
        }

        // Filter by entity type(s)
        if let Some(prefixes) = type_prefixes {
            if !prefixes.is_empty() {
                let placeholders: Vec<String> = prefixes
                    .iter()
                    .enumerate()
                    .map(|(i, _)| format!("?{}", params_vec.len() + i + 1))
                    .collect();
                sql.push_str(&format!(" AND e.prefix IN ({})", placeholders.join(",")));
                for prefix in prefixes {
                    params_vec.push(Box::new(prefix.to_string()));
                }
            }
        }

        // Filter by status
        if let Some(s) = status {
            sql.push_str(&format!(" AND e.status = ?{}", params_vec.len() + 1));
            params_vec.push(Box::new(s.to_string()));
        }

        // Filter by author
        if let Some(a) = author {
            sql.push_str(&format!(" AND e.author LIKE ?{}", params_vec.len() + 1));
            params_vec.push(Box::new(format!("%{}%", a)));
        }

        // Filter by tag
        if let Some(t) = tag {
            sql.push_str(&format!(" AND e.tags LIKE ?{}", params_vec.len() + 1));
            params_vec.push(Box::new(format!("%{}%", t)));
        }

        sql.push_str(" ORDER BY e.created DESC");
        sql.push_str(&format!(" LIMIT {}", limit));

        let mut stmt = match self.conn.prepare(&sql) {
            Ok(s) => s,
            Err(_) => return vec![],
        };

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();

        let rows = match stmt.query_map(params_refs.as_slice(), |row| {
            Ok(super::SearchResult {
                id: row.get(0)?,
                entity_type: row.get(1)?,
                title: row.get(2)?,
                status: row.get(3)?,
                author: row.get(4)?,
            })
        }) {
            Ok(r) => r,
            Err(_) => return vec![],
        };

        rows.filter_map(|r| r.ok()).collect()
    }

    /// List recently modified entities (by file modification time)
    pub fn list_recent(
        &self,
        type_prefixes: Option<&[&str]>,
        limit: usize,
    ) -> Vec<CachedEntity> {
        let mut sql = String::from(
            "SELECT id, prefix, title, status, author, created, file_path, priority, entity_type, category, tags, file_mtime FROM entities WHERE 1=1",
        );
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = vec![];

        // Filter by entity types
        if let Some(prefixes) = type_prefixes {
            if !prefixes.is_empty() {
                let placeholders: Vec<String> = prefixes.iter().enumerate()
                    .map(|(i, _)| format!("?{}", i + 1))
                    .collect();
                sql.push_str(&format!(" AND prefix IN ({})", placeholders.join(",")));
                for prefix in prefixes {
                    params_vec.push(Box::new(prefix.to_string()));
                }
            }
        }

        // Order by file modification time (most recent first)
        sql.push_str(" ORDER BY file_mtime DESC");
        sql.push_str(&format!(" LIMIT {}", limit));

        let mut stmt = match self.conn.prepare(&sql) {
            Ok(s) => s,
            Err(_) => return vec![],
        };

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();

        let rows = match stmt.query_map(params_refs.as_slice(), |row| {
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

    /// List all unique tags across all entities
    pub fn list_all_tags(&self) -> Vec<(String, usize)> {
        // Query all tags from entities table
        let mut stmt = match self.conn.prepare(
            "SELECT tags FROM entities WHERE tags IS NOT NULL AND tags != ''",
        ) {
            Ok(s) => s,
            Err(_) => return vec![],
        };

        let rows = match stmt.query_map([], |row| {
            let tags_str: String = row.get(0)?;
            Ok(tags_str)
        }) {
            Ok(r) => r,
            Err(_) => return vec![],
        };

        // Count tag occurrences
        let mut tag_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for row in rows.filter_map(|r| r.ok()) {
            for tag in row.split(',').filter(|t| !t.is_empty()) {
                *tag_counts.entry(tag.to_string()).or_insert(0) += 1;
            }
        }

        // Sort by count (descending) then alphabetically
        let mut tags: Vec<(String, usize)> = tag_counts.into_iter().collect();
        tags.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        tags
    }

    /// Get entities by tag
    pub fn list_by_tag(&self, tag: &str, limit: Option<usize>) -> Vec<CachedEntity> {
        let mut sql = String::from(
            "SELECT id, prefix, title, status, author, created, file_path, priority, entity_type, category, tags FROM entities WHERE tags LIKE ?1",
        );

        sql.push_str(" ORDER BY created DESC");
        if let Some(lim) = limit {
            sql.push_str(&format!(" LIMIT {}", lim));
        }

        let mut stmt = match self.conn.prepare(&sql) {
            Ok(s) => s,
            Err(_) => return vec![],
        };

        let pattern = format!("%{}%", tag);
        let rows = match stmt.query_map(params![pattern], |row| {
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
}
