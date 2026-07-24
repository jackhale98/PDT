//! Traceability service - link queries and coverage analysis

use miette::Result;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

use crate::core::cache::{CachedEntity, CachedLink, EntityCache, EntityFilter};
use crate::core::identity::EntityPrefix;
use crate::core::project::Project;

/// Direction for tracing links
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TraceDirection {
    /// Follow links from source to targets
    Forward,
    /// Follow links from targets to source (reverse)
    Backward,
    /// Follow links in both directions
    Both,
}

/// Options for trace queries
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TraceOptions {
    /// Maximum depth to traverse (None = unlimited)
    pub max_depth: Option<usize>,

    /// Link types to follow (None = all)
    pub link_types: Option<Vec<String>>,

    /// Entity types to include (None = all)
    pub entity_types: Option<Vec<EntityPrefix>>,

    /// Direction to trace
    #[serde(default)]
    pub direction: Option<TraceDirection>,

    /// Include the source entity in results
    #[serde(default)]
    pub include_source: bool,
}

/// A traced entity with its relationship to the source
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TracedEntity {
    /// Entity ID
    pub id: String,

    /// Entity type prefix
    pub prefix: EntityPrefix,

    /// Entity title
    pub title: String,

    /// Entity status
    pub status: String,

    /// Depth from source (0 = direct link)
    pub depth: usize,

    /// Link type that connects this entity
    pub link_type: String,

    /// Path from source to this entity (entity IDs)
    pub path: Vec<String>,
}

/// Result of a trace query
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TraceResult {
    /// Source entity ID
    pub source_id: String,

    /// Traced entities (excluding source unless include_source is true)
    pub entities: Vec<TracedEntity>,

    /// Total entities found
    pub total: usize,

    /// Maximum depth reached
    pub max_depth_reached: usize,

    /// Entity counts by type
    pub by_type: HashMap<String, usize>,

    /// Entity counts by depth
    pub by_depth: HashMap<usize, usize>,
}

/// Coverage statistics for a specific relationship
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CoverageStats {
    /// Total entities of the source type
    pub total: usize,

    /// Entities with at least one link of the specified type
    pub covered: usize,

    /// Entities without any links of the specified type
    pub uncovered: usize,

    /// Coverage percentage (0-100)
    pub percentage: f64,

    /// IDs of uncovered entities
    pub uncovered_ids: Vec<String>,
}

/// Comprehensive coverage report
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CoverageReport {
    /// Requirements verified by tests
    pub requirements_verified: CoverageStats,

    /// Requirements satisfied by design outputs
    pub requirements_satisfied: CoverageStats,

    /// Risks mitigated by controls
    pub risks_mitigated: CoverageStats,

    /// Risks verified by tests
    pub risks_verified: CoverageStats,

    /// Tests linked to requirements
    pub tests_linked: CoverageStats,

    /// Components with suppliers
    pub components_with_suppliers: CoverageStats,

    /// Overall traceability health score (0-100)
    pub health_score: f64,
}

/// A link in the traceability graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceLink {
    pub source_id: String,
    pub target_id: String,
    pub link_type: String,
}

impl From<CachedLink> for TraceLink {
    fn from(cached: CachedLink) -> Self {
        TraceLink {
            source_id: cached.source_id,
            target_id: cached.target_id,
            link_type: cached.link_type,
        }
    }
}

/// Design Structure Matrix (DSM) for dependency analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesignStructureMatrix {
    /// Entity IDs in matrix order
    pub entity_ids: Vec<String>,

    /// Entity labels (titles) for display
    pub labels: Vec<String>,

    /// Matrix cells: cells[row][col] = link exists from row to col
    pub cells: Vec<Vec<bool>>,

    /// Link types for each cell (if link exists)
    pub link_types: Vec<Vec<Option<String>>>,

    /// Entity types for coloring
    pub entity_types: Vec<EntityPrefix>,
}

/// Service for traceability queries and coverage analysis
pub struct TraceabilityService<'a> {
    #[allow(dead_code)] // Reserved for future use (e.g., file path resolution)
    project: &'a Project,
    cache: &'a EntityCache,
}

impl<'a> TraceabilityService<'a> {
    /// Create a new traceability service
    pub fn new(project: &'a Project, cache: &'a EntityCache) -> Self {
        Self { project, cache }
    }

    /// Trace from a source entity following links
    pub fn trace_from(&self, source_id: &str, options: &TraceOptions) -> Result<TraceResult> {
        let direction = options.direction.unwrap_or(TraceDirection::Forward);

        let mut result = TraceResult {
            source_id: source_id.to_string(),
            ..Default::default()
        };

        let mut visited: HashSet<String> = HashSet::new();
        let mut queue: VecDeque<(String, usize, Vec<String>, String)> = VecDeque::new();

        // Initialize with source
        visited.insert(source_id.to_string());
        queue.push_back((
            source_id.to_string(),
            0,
            vec![source_id.to_string()],
            "source".to_string(),
        ));

        while let Some((current_id, depth, path, _link_type)) = queue.pop_front() {
            // Check max depth
            if let Some(max) = options.max_depth {
                if depth > max {
                    continue;
                }
            }

            // Get links from this entity
            let links = match direction {
                TraceDirection::Forward => self.get_forward_links(&current_id),
                TraceDirection::Backward => self.get_backward_links(&current_id),
                TraceDirection::Both => {
                    let mut all = self.get_forward_links(&current_id);
                    all.extend(self.get_backward_links(&current_id));
                    all
                }
            };

            for link in links {
                let target_id = if link.source_id == current_id {
                    &link.target_id
                } else {
                    &link.source_id
                };

                // Skip if already visited
                if visited.contains(target_id) {
                    continue;
                }

                // Filter by link type
                if let Some(types) = &options.link_types {
                    if !types.contains(&link.link_type) {
                        continue;
                    }
                }

                // Get entity info from cache
                if let Some(entity) = self.cache.get_entity(target_id) {
                    let prefix = parse_prefix(&entity.prefix);

                    // Filter by entity type
                    if let Some(types) = &options.entity_types {
                        if !types.contains(&prefix) {
                            continue;
                        }
                    }

                    visited.insert(target_id.to_string());

                    let mut new_path = path.clone();
                    new_path.push(target_id.to_string());

                    let traced = TracedEntity {
                        id: target_id.to_string(),
                        prefix,
                        title: entity.title.clone(),
                        status: format!("{:?}", entity.status),
                        depth: depth + 1,
                        link_type: link.link_type.clone(),
                        path: new_path.clone(),
                    };

                    result.entities.push(traced);

                    // Update counts
                    *result.by_type.entry(entity.prefix.clone()).or_insert(0) += 1;
                    *result.by_depth.entry(depth + 1).or_insert(0) += 1;

                    if depth + 1 > result.max_depth_reached {
                        result.max_depth_reached = depth + 1;
                    }

                    // Add to queue for further traversal
                    queue.push_back((target_id.to_string(), depth + 1, new_path, link.link_type));
                }
            }
        }

        // Include source if requested
        if options.include_source {
            if let Some(entity) = self.cache.get_entity(source_id) {
                let prefix = parse_prefix(&entity.prefix);
                result.entities.insert(
                    0,
                    TracedEntity {
                        id: source_id.to_string(),
                        prefix,
                        title: entity.title.clone(),
                        status: format!("{:?}", entity.status),
                        depth: 0,
                        link_type: "source".to_string(),
                        path: vec![source_id.to_string()],
                    },
                );
            }
        }

        result.total = result.entities.len();
        Ok(result)
    }

    /// Trace to a target entity (find what links to it)
    pub fn trace_to(&self, target_id: &str, options: &TraceOptions) -> Result<TraceResult> {
        let mut opts = options.clone();
        opts.direction = Some(TraceDirection::Backward);
        self.trace_from(target_id, &opts)
    }

    /// Get all entities that directly link to the given entity
    pub fn get_direct_links(&self, entity_id: &str) -> Vec<TraceLink> {
        let mut links = self.get_forward_links(entity_id);
        links.extend(self.get_backward_links(entity_id));
        links
    }

    /// Get forward links from an entity
    fn get_forward_links(&self, entity_id: &str) -> Vec<TraceLink> {
        self.cache
            .get_links_from(entity_id)
            .into_iter()
            .map(TraceLink::from)
            .collect()
    }

    /// Get backward links to an entity
    fn get_backward_links(&self, entity_id: &str) -> Vec<TraceLink> {
        self.cache
            .get_links_to(entity_id)
            .into_iter()
            .map(TraceLink::from)
            .collect()
    }

    /// Get coverage report for the project
    pub fn get_coverage(&self) -> CoverageReport {
        let requirements_verified = self.calculate_coverage(EntityPrefix::Req, "verified_by");
        let requirements_satisfied = self.calculate_coverage(EntityPrefix::Req, "satisfied_by");
        let risks_mitigated = self.calculate_coverage(EntityPrefix::Risk, "mitigated_by");
        let risks_verified = self.calculate_coverage(EntityPrefix::Risk, "verified_by");
        let tests_linked = self.calculate_coverage(EntityPrefix::Test, "verifies");
        let components_with_suppliers = self.calculate_supplier_coverage(EntityPrefix::Cmp);

        let weights = [
            (requirements_verified.percentage, 0.25),
            (requirements_satisfied.percentage, 0.20),
            (risks_mitigated.percentage, 0.20),
            (risks_verified.percentage, 0.15),
            (tests_linked.percentage, 0.15),
            (components_with_suppliers.percentage, 0.05),
        ];
        let health_score = weights.iter().map(|(p, w)| p * w).sum();

        CoverageReport {
            requirements_verified,
            requirements_satisfied,
            risks_mitigated,
            risks_verified,
            tests_linked,
            components_with_suppliers,
            health_score,
        }
    }

    /// Calculate coverage for a specific entity type and link type
    fn calculate_coverage(&self, entity_type: EntityPrefix, link_type: &str) -> CoverageStats {
        let filter = EntityFilter {
            prefix: Some(entity_type),
            ..Default::default()
        };
        let entities = self.cache.list_entities(&filter);

        let mut stats = CoverageStats {
            total: entities.len(),
            ..Default::default()
        };

        for entity in entities {
            let links = self.get_forward_links(&entity.id);
            let has_link = links.iter().any(|l| l.link_type == link_type);

            if has_link {
                stats.covered += 1;
            } else {
                stats.uncovered += 1;
                stats.uncovered_ids.push(entity.id.clone());
            }
        }

        if stats.total > 0 {
            stats.percentage = (stats.covered as f64 / stats.total as f64) * 100.0;
        }

        stats
    }

    /// Calculate supplier coverage for components
    fn calculate_supplier_coverage(&self, entity_type: EntityPrefix) -> CoverageStats {
        let filter = EntityFilter {
            prefix: Some(entity_type),
            ..Default::default()
        };
        let entities = self.cache.list_entities(&filter);

        let mut stats = CoverageStats {
            total: entities.len(),
            ..Default::default()
        };

        for entity in &entities {
            // Check if entity has supplier links
            let links = self.get_forward_links(&entity.id);
            let has_supplier = links
                .iter()
                .any(|l| l.link_type == "supplier" || l.target_id.starts_with("SUP-"));

            if has_supplier {
                stats.covered += 1;
            } else {
                stats.uncovered += 1;
                stats.uncovered_ids.push(entity.id.clone());
            }
        }

        if stats.total > 0 {
            stats.percentage = (stats.covered as f64 / stats.total as f64) * 100.0;
        }

        stats
    }

    /// Generate a Design Structure Matrix for dependency analysis
    pub fn generate_dsm(&self, entity_types: &[EntityPrefix]) -> DesignStructureMatrix {
        let mut all_entities: Vec<CachedEntity> = Vec::new();

        // Collect all entities of the specified types
        for prefix in entity_types {
            let filter = EntityFilter {
                prefix: Some(*prefix),
                ..Default::default()
            };
            let entities = self.cache.list_entities(&filter);
            all_entities.extend(entities);
        }

        let n = all_entities.len();

        // Create ID to index mapping
        let id_to_idx: HashMap<&str, usize> = all_entities
            .iter()
            .enumerate()
            .map(|(i, e)| (e.id.as_str(), i))
            .collect();

        let mut cells = vec![vec![false; n]; n];
        let mut link_types = vec![vec![None; n]; n];

        // Fill in the matrix
        for (row, entity) in all_entities.iter().enumerate() {
            let links = self.get_forward_links(&entity.id);

            for link in links {
                if let Some(&col) = id_to_idx.get(link.target_id.as_str()) {
                    cells[row][col] = true;
                    link_types[row][col] = Some(link.link_type.clone());
                }
            }
        }

        DesignStructureMatrix {
            entity_ids: all_entities.iter().map(|e| e.id.clone()).collect(),
            labels: all_entities.iter().map(|e| e.title.clone()).collect(),
            cells,
            link_types,
            entity_types: all_entities
                .iter()
                .map(|e| parse_prefix(&e.prefix))
                .collect(),
        }
    }

    /// Find orphaned entities (no incoming or outgoing links)
    pub fn find_orphans(&self, entity_type: Option<EntityPrefix>) -> Vec<TracedEntity> {
        let mut orphans = Vec::new();

        let prefixes: Vec<EntityPrefix> = if let Some(p) = entity_type {
            vec![p]
        } else {
            EntityPrefix::all().to_vec()
        };

        for prefix in prefixes {
            let filter = EntityFilter {
                prefix: Some(prefix),
                ..Default::default()
            };
            let entities = self.cache.list_entities(&filter);

            for entity in entities {
                let forward = self.get_forward_links(&entity.id);
                let backward = self.get_backward_links(&entity.id);

                if forward.is_empty() && backward.is_empty() {
                    orphans.push(TracedEntity {
                        id: entity.id.clone(),
                        prefix,
                        title: entity.title.clone(),
                        status: format!("{:?}", entity.status),
                        depth: 0,
                        link_type: "orphan".to_string(),
                        path: vec![entity.id.clone()],
                    });
                }
            }
        }

        orphans
    }

    /// Find circular dependencies in the link graph
    pub fn find_cycles(&self, entity_type: Option<EntityPrefix>) -> Vec<Vec<String>> {
        let mut cycles = Vec::new();
        let mut visited: HashSet<String> = HashSet::new();
        let mut rec_stack: HashSet<String> = HashSet::new();

        let prefixes: Vec<EntityPrefix> = if let Some(p) = entity_type {
            vec![p]
        } else {
            EntityPrefix::all().to_vec()
        };

        for prefix in prefixes {
            let filter = EntityFilter {
                prefix: Some(prefix),
                ..Default::default()
            };
            let entities = self.cache.list_entities(&filter);

            for entity in entities {
                if !visited.contains(&entity.id) {
                    let mut path = Vec::new();
                    self.detect_cycle_dfs(
                        &entity.id,
                        &mut visited,
                        &mut rec_stack,
                        &mut path,
                        &mut cycles,
                    );
                }
            }
        }

        cycles
    }

    /// DFS helper for cycle detection
    fn detect_cycle_dfs(
        &self,
        node: &str,
        visited: &mut HashSet<String>,
        rec_stack: &mut HashSet<String>,
        path: &mut Vec<String>,
        cycles: &mut Vec<Vec<String>>,
    ) {
        visited.insert(node.to_string());
        rec_stack.insert(node.to_string());
        path.push(node.to_string());

        let links = self.get_forward_links(node);

        for link in links {
            if !visited.contains(&link.target_id) {
                self.detect_cycle_dfs(&link.target_id, visited, rec_stack, path, cycles);
            } else if rec_stack.contains(&link.target_id) {
                // Found a cycle - extract it from path
                if let Some(start_idx) = path.iter().position(|x| x == &link.target_id) {
                    let cycle: Vec<String> = path[start_idx..].to_vec();
                    cycles.push(cycle);
                }
            }
        }

        path.pop();
        rec_stack.remove(node);
    }

    /// Get impact analysis - what would be affected if entity changes
    pub fn get_impact(&self, entity_id: &str, max_depth: Option<usize>) -> Result<TraceResult> {
        self.trace_from(
            entity_id,
            &TraceOptions {
                max_depth,
                direction: Some(TraceDirection::Backward),
                include_source: true,
                ..Default::default()
            },
        )
    }

    /// Get dependency analysis - what does entity depend on
    pub fn get_dependencies(
        &self,
        entity_id: &str,
        max_depth: Option<usize>,
    ) -> Result<TraceResult> {
        self.trace_from(
            entity_id,
            &TraceOptions {
                max_depth,
                direction: Some(TraceDirection::Forward),
                include_source: true,
                ..Default::default()
            },
        )
    }

    /// Generate a Domain Mapping Matrix (DMM) showing relationships between two entity types
    ///
    /// For example, generate_dmm(Req, Test, "verified_by") shows which requirements are verified
    /// by which tests. Rows are source entities, columns are target entities.
    pub fn generate_dmm(
        &self,
        source_type: EntityPrefix,
        target_type: EntityPrefix,
        link_type: Option<&str>,
    ) -> DomainMappingMatrix {
        // Get all entities of source type
        let source_filter = EntityFilter {
            prefix: Some(source_type),
            ..Default::default()
        };
        let source_entities = self.cache.list_entities(&source_filter);

        // Get all entities of target type
        let target_filter = EntityFilter {
            prefix: Some(target_type),
            ..Default::default()
        };
        let target_entities = self.cache.list_entities(&target_filter);

        let n_rows = source_entities.len();
        let n_cols = target_entities.len();

        // Create ID to index mappings
        let target_id_to_idx: HashMap<&str, usize> = target_entities
            .iter()
            .enumerate()
            .map(|(i, e)| (e.id.as_str(), i))
            .collect();

        let mut cells = vec![vec![false; n_cols]; n_rows];
        let mut link_types_matrix = vec![vec![None; n_cols]; n_rows];

        // Fill in the matrix
        for (row, source) in source_entities.iter().enumerate() {
            let links = self.get_forward_links(&source.id);

            for link in links {
                // Filter by link type if specified
                if let Some(lt) = link_type {
                    if link.link_type != lt {
                        continue;
                    }
                }

                if let Some(&col) = target_id_to_idx.get(link.target_id.as_str()) {
                    cells[row][col] = true;
                    link_types_matrix[row][col] = Some(link.link_type.clone());
                }
            }
        }

        // Count coverage statistics
        let sources_with_links = cells.iter().filter(|row| row.iter().any(|&c| c)).count();
        let targets_with_links = (0..n_cols)
            .filter(|&col| cells.iter().any(|row| row[col]))
            .count();

        DomainMappingMatrix {
            source_type,
            target_type,
            source_ids: source_entities.iter().map(|e| e.id.clone()).collect(),
            source_labels: source_entities.iter().map(|e| e.title.clone()).collect(),
            target_ids: target_entities.iter().map(|e| e.id.clone()).collect(),
            target_labels: target_entities.iter().map(|e| e.title.clone()).collect(),
            cells,
            link_types: link_types_matrix,
            source_coverage: if n_rows > 0 {
                (sources_with_links as f64 / n_rows as f64) * 100.0
            } else {
                0.0
            },
            target_coverage: if n_cols > 0 {
                (targets_with_links as f64 / n_cols as f64) * 100.0
            } else {
                0.0
            },
        }
    }

    /// Find broken links - links that point to non-existent entities
    pub fn find_broken_links(&self) -> Vec<BrokenLink> {
        let mut broken = Vec::new();
        let mut seen_links: HashSet<(String, String, String)> = HashSet::new();

        // Iterate over all entities and check their outgoing links
        for prefix in EntityPrefix::all() {
            let filter = EntityFilter {
                prefix: Some(*prefix),
                ..Default::default()
            };
            let entities = self.cache.list_entities(&filter);

            for entity in entities {
                let links = self.get_forward_links(&entity.id);

                for link in links {
                    // Avoid duplicates
                    let key = (
                        link.source_id.clone(),
                        link.target_id.clone(),
                        link.link_type.clone(),
                    );
                    if seen_links.contains(&key) {
                        continue;
                    }
                    seen_links.insert(key);

                    // Check if target exists
                    let target_exists = self.cache.get_entity(&link.target_id).is_some();

                    if !target_exists {
                        broken.push(BrokenLink {
                            source_id: link.source_id.clone(),
                            target_id: link.target_id.clone(),
                            link_type: link.link_type.clone(),
                            source_exists: true, // Source must exist since we found it
                            target_exists: false,
                        });
                    }
                }
            }
        }

        broken
    }

    /// Validate all links and return a summary
    pub fn validate_links(&self) -> LinkValidationResult {
        let broken = self.find_broken_links();

        // Count total links by iterating over all entities
        let mut total_links = 0;
        for prefix in EntityPrefix::all() {
            let filter = EntityFilter {
                prefix: Some(*prefix),
                ..Default::default()
            };
            let entities = self.cache.list_entities(&filter);

            for entity in entities {
                total_links += self.get_forward_links(&entity.id).len();
            }
        }

        LinkValidationResult {
            total_links,
            valid_links: total_links - broken.len(),
            broken_links: broken.len(),
            broken,
        }
    }
}

/// Domain Mapping Matrix - shows relationships between two different entity types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainMappingMatrix {
    /// Source entity type (rows)
    pub source_type: EntityPrefix,

    /// Target entity type (columns)
    pub target_type: EntityPrefix,

    /// Source entity IDs
    pub source_ids: Vec<String>,

    /// Source entity labels (titles)
    pub source_labels: Vec<String>,

    /// Target entity IDs
    pub target_ids: Vec<String>,

    /// Target entity labels (titles)
    pub target_labels: Vec<String>,

    /// Matrix cells: cells[row][col] = link exists from source to target
    pub cells: Vec<Vec<bool>>,

    /// Link types for each cell (if link exists)
    pub link_types: Vec<Vec<Option<String>>>,

    /// Percentage of source entities with at least one link
    pub source_coverage: f64,

    /// Percentage of target entities with at least one link
    pub target_coverage: f64,
}

/// A broken link (points to non-existent entity)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrokenLink {
    /// Source entity ID
    pub source_id: String,

    /// Target entity ID (may not exist)
    pub target_id: String,

    /// Link type
    pub link_type: String,

    /// Whether source entity exists
    pub source_exists: bool,

    /// Whether target entity exists
    pub target_exists: bool,
}

/// Result of link validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkValidationResult {
    /// Total number of links
    pub total_links: usize,

    /// Number of valid links
    pub valid_links: usize,

    /// Number of broken links
    pub broken_links: usize,

    /// Details of broken links
    pub broken: Vec<BrokenLink>,
}

/// Parse entity prefix from string
fn parse_prefix(s: &str) -> EntityPrefix {
    match s.to_uppercase().as_str() {
        "REQ" => EntityPrefix::Req,
        "RISK" => EntityPrefix::Risk,
        "TEST" => EntityPrefix::Test,
        "RSLT" => EntityPrefix::Rslt,
        "CMP" => EntityPrefix::Cmp,
        "ASM" => EntityPrefix::Asm,
        "FEAT" => EntityPrefix::Feat,
        "MATE" => EntityPrefix::Mate,
        "TOL" => EntityPrefix::Tol,
        "PROC" => EntityPrefix::Proc,
        "CTRL" => EntityPrefix::Ctrl,
        "WORK" => EntityPrefix::Work,
        "LOT" => EntityPrefix::Lot,
        "DEV" => EntityPrefix::Dev,
        "NCR" => EntityPrefix::Ncr,
        "CAPA" => EntityPrefix::Capa,
        "QUOT" => EntityPrefix::Quot,
        "SUP" => EntityPrefix::Sup,
        "HAZ" => EntityPrefix::Haz,
        _ => EntityPrefix::Req, // Default fallback
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn setup_test_project() -> (TempDir, Project, EntityCache) {
        let tmp = TempDir::new().unwrap();

        // Initialize project structure
        fs::create_dir_all(tmp.path().join(".tdt")).unwrap();
        fs::create_dir_all(tmp.path().join("requirements/inputs")).unwrap();
        fs::create_dir_all(tmp.path().join("tests")).unwrap();

        // Create config file
        fs::write(tmp.path().join(".tdt/config.yaml"), "author: Test Author\n").unwrap();

        let project = Project::discover_from(tmp.path()).unwrap();
        let cache = EntityCache::open(&project).unwrap();

        (tmp, project, cache)
    }

    #[test]
    fn test_trace_options_default() {
        let opts = TraceOptions::default();
        assert!(opts.max_depth.is_none());
        assert!(opts.link_types.is_none());
        assert!(opts.entity_types.is_none());
        assert!(!opts.include_source);
    }

    #[test]
    fn test_trace_result_default() {
        let result = TraceResult::default();
        assert!(result.source_id.is_empty());
        assert!(result.entities.is_empty());
        assert_eq!(result.total, 0);
    }

    #[test]
    fn test_coverage_stats_default() {
        let stats = CoverageStats::default();
        assert_eq!(stats.total, 0);
        assert_eq!(stats.covered, 0);
        assert_eq!(stats.uncovered, 0);
        assert_eq!(stats.percentage, 0.0);
    }

    #[test]
    fn test_parse_prefix() {
        assert_eq!(parse_prefix("REQ"), EntityPrefix::Req);
        assert_eq!(parse_prefix("req"), EntityPrefix::Req);
        assert_eq!(parse_prefix("RISK"), EntityPrefix::Risk);
        assert_eq!(parse_prefix("TEST"), EntityPrefix::Test);
        assert_eq!(parse_prefix("CMP"), EntityPrefix::Cmp);
    }

    #[test]
    fn test_service_creation() {
        let (_tmp, project, cache) = setup_test_project();
        let _service = TraceabilityService::new(&project, &cache);
        // Just verify it can be created
    }

    #[test]
    fn test_coverage_report_default() {
        let report = CoverageReport::default();
        assert_eq!(report.health_score, 0.0);
        assert_eq!(report.requirements_verified.total, 0);
    }

    #[test]
    fn test_dsm_structure() {
        let dsm = DesignStructureMatrix {
            entity_ids: vec!["A".into(), "B".into()],
            labels: vec!["Entity A".into(), "Entity B".into()],
            cells: vec![vec![false, true], vec![false, false]],
            link_types: vec![vec![None, Some("verifies".into())], vec![None, None]],
            entity_types: vec![EntityPrefix::Req, EntityPrefix::Test],
        };

        assert_eq!(dsm.entity_ids.len(), 2);
        assert!(dsm.cells[0][1]);
        assert!(!dsm.cells[1][0]);
    }

    #[test]
    fn test_trace_link_from_cached() {
        let cached = CachedLink {
            field_name: None,
            source_id: "REQ-001".into(),
            target_id: "TEST-001".into(),
            link_type: "verified_by".into(),
        };

        let trace_link: TraceLink = cached.into();

        assert_eq!(trace_link.source_id, "REQ-001");
        assert_eq!(trace_link.target_id, "TEST-001");
        assert_eq!(trace_link.link_type, "verified_by");
    }

    #[test]
    fn test_empty_coverage() {
        let (_tmp, project, cache) = setup_test_project();
        let service = TraceabilityService::new(&project, &cache);

        let coverage = service.get_coverage();

        // Empty project should have 0 coverage
        assert_eq!(coverage.requirements_verified.total, 0);
        assert_eq!(coverage.health_score, 0.0);
    }

    #[test]
    fn test_find_orphans_empty_project() {
        let (_tmp, project, cache) = setup_test_project();
        let service = TraceabilityService::new(&project, &cache);

        let orphans = service.find_orphans(None);

        // Empty project has no orphans
        assert!(orphans.is_empty());
    }

    #[test]
    fn test_find_cycles_empty_project() {
        let (_tmp, project, cache) = setup_test_project();
        let service = TraceabilityService::new(&project, &cache);

        let cycles = service.find_cycles(None);

        // Empty project has no cycles
        assert!(cycles.is_empty());
    }

    #[test]
    fn test_generate_dsm_empty() {
        let (_tmp, project, cache) = setup_test_project();
        let service = TraceabilityService::new(&project, &cache);

        let dsm = service.generate_dsm(&[EntityPrefix::Req, EntityPrefix::Test]);

        // Empty project has empty DSM
        assert!(dsm.entity_ids.is_empty());
        assert!(dsm.cells.is_empty());
    }
}
