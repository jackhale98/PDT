//! Mate service for fit analysis management
//!
//! Provides CRUD operations and fit calculation for mating features.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::core::cache::{CachedEntity, EntityCache, EntityFilter};
use crate::core::entity::{Entity, Status};
use crate::core::identity::{EntityId, EntityPrefix};
use crate::core::loader;
use crate::core::project::Project;
use crate::entities::feature::Feature;
use crate::entities::mate::{FitResult, Mate, MateFeatureRef, MateType};
use crate::services::base::ServiceBase;
use crate::services::common::{
    CommonFilter, ServiceError, ServiceResult, SortDirection, SortKey, Sortable,
};

/// Filter options for listing mates
#[derive(Debug, Clone, Default)]
pub struct MateFilter {
    /// Common filters (status, author, tags, search)
    pub common: CommonFilter,
    /// Filter by mate type
    pub mate_type: Option<MateType>,
    /// Filter by fit result (from analysis)
    pub fit_result: Option<FitResult>,
    /// Show recent mates (last N days)
    pub recent_days: Option<u32>,
    /// Sort field
    pub sort: MateSortField,
    /// Sort direction
    pub sort_direction: SortDirection,
}

/// Fields available for sorting mates
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MateSortField {
    Id,
    Title,
    MateType,
    FitResult,
    Status,
    Author,
    #[default]
    Created,
}

impl Sortable for Mate {
    type SortField = MateSortField;

    fn sort_key(&self, field: &Self::SortField) -> SortKey {
        match field {
            MateSortField::Id => SortKey::String(self.id.to_string()),
            MateSortField::Title => SortKey::String(self.title.clone()),
            MateSortField::MateType => SortKey::String(self.mate_type.to_string()),
            MateSortField::FitResult => {
                let fit_str = self
                    .fit_analysis
                    .as_ref()
                    .map(|f| f.fit_result.to_string())
                    .unwrap_or_default();
                SortKey::String(fit_str)
            }
            MateSortField::Status => SortKey::String(self.status().to_string()),
            MateSortField::Author => SortKey::String(self.author.clone()),
            MateSortField::Created => SortKey::DateTime(self.created.timestamp()),
        }
    }
}

/// Input for creating a new mate
#[derive(Debug, Clone)]
pub struct CreateMate {
    /// Mate title
    pub title: String,
    /// First feature ID
    pub feature_a: EntityId,
    /// Second feature ID
    pub feature_b: EntityId,
    /// Mate type
    pub mate_type: MateType,
    /// Description
    pub description: Option<String>,
    /// Notes
    pub notes: Option<String>,
    /// Tags
    pub tags: Vec<String>,
    /// Author
    pub author: String,
}

/// Input for updating an existing mate
#[derive(Debug, Clone, Default)]
pub struct UpdateMate {
    /// Update title
    pub title: Option<String>,
    /// Update description
    pub description: Option<Option<String>>,
    /// Update notes
    pub notes: Option<Option<String>>,
    /// Update mate type
    pub mate_type: Option<MateType>,
    /// Update document status
    pub status: Option<Status>,
}

/// Statistics about mates
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MateStats {
    /// Total number of mates
    pub total: usize,
    /// Counts by mate type
    pub by_type: MateTypeCounts,
    /// Counts by fit result
    pub by_fit: FitResultCounts,
    /// Number with fit analysis
    pub analyzed_count: usize,
    /// Number where intended type matches actual fit
    pub matches_intent: usize,
}

/// Mate type counts
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MateTypeCounts {
    pub clearance: usize,
    pub transition: usize,
    pub interference: usize,
}

/// Fit result counts
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FitResultCounts {
    pub clearance: usize,
    pub transition: usize,
    pub interference: usize,
    pub not_analyzed: usize,
}

/// Result of a recalculation operation
#[derive(Debug, Clone)]
pub struct RecalcResult {
    /// Mate that was recalculated
    pub mate: Mate,
    /// Whether the fit changed
    pub changed: bool,
    /// Error if feature lookup failed
    pub error: Option<String>,
}

/// Service for managing mates
pub struct MateService<'a> {
    project: &'a Project,
    base: ServiceBase<'a>,
    cache: &'a EntityCache,
}

impl<'a> MateService<'a> {
    /// Create a new MateService
    pub fn new(project: &'a Project, cache: &'a EntityCache) -> Self {
        Self {
            project,
            base: ServiceBase::new(project, cache),
            cache,
        }
    }

    /// Get a reference to the project
    fn project(&self) -> &Project {
        self.project
    }

    /// Get the mates directory
    fn mate_dir(&self) -> PathBuf {
        self.project().root().join("tolerances/mates")
    }

    /// Get the features directory
    fn feature_dir(&self) -> PathBuf {
        self.project().root().join("tolerances/features")
    }

    /// Get the file path for a mate
    fn get_file_path(&self, id: &EntityId) -> PathBuf {
        self.mate_dir().join(format!("{}.tdt.yaml", id))
    }

    /// Load all mates
    fn load_all(&self) -> ServiceResult<Vec<Mate>> {
        let dir = self.mate_dir();
        if !dir.exists() {
            return Ok(Vec::new());
        }
        loader::load_all(&dir).map_err(ServiceError::from)
    }

    /// List mates using the cache (fast path)
    ///
    /// Returns cached entity data without loading full entities from disk.
    /// Use this for list views and simple queries.
    pub fn list_cached(&self, filter: &MateFilter) -> ServiceResult<Vec<CachedEntity>> {
        // Build cache filter
        let status = filter
            .common
            .status
            .as_ref()
            .and_then(|s| s.first())
            .copied();

        let entity_filter = EntityFilter {
            prefix: Some(EntityPrefix::Mate),
            status,
            author: filter.common.author.clone(),
            search: filter.common.search.clone(),
            limit: None, // Apply limit after all filters
            priority: filter
                .common
                .priority
                .as_ref()
                .and_then(|p| p.first())
                .copied(),
            entity_type: None,
            category: None,
        };

        let mut cached = self.cache.list_entities(&entity_filter);

        // Apply additional filters not supported by cache query
        if let Some(days) = filter.recent_days {
            let cutoff = Utc::now() - chrono::Duration::days(days as i64);
            cached.retain(|e| e.created >= cutoff);
        }

        // Note: mate_type, fit_result require full entity load
        // These are handled in the regular list() method

        // Apply limit
        if let Some(limit) = filter.common.limit {
            cached.truncate(limit);
        }

        Ok(cached)
    }

    /// Find a mate by ID
    ///
    /// Uses the cache to find the file path for faster lookup.
    fn find_mate(&self, id: &str) -> ServiceResult<(PathBuf, Mate)> {
        // Try to find in cache first for fast path lookup
        if let Some(cached) = self.cache.get_entity(id) {
            let path = if cached.file_path.is_absolute() {
                cached.file_path.clone()
            } else {
                self.project.root().join(&cached.file_path)
            };
            if path.exists() {
                if let Ok(mate) = crate::yaml::parse_yaml_file::<Mate>(&path) {
                    return Ok((path, mate));
                }
            }
        }

        // Fall back to directory scan
        let dir = self.mate_dir();
        if let Some((path, mate)) = loader::load_entity::<Mate>(&dir, id)? {
            return Ok((path, mate));
        }
        Err(ServiceError::NotFound(format!("Mate: {}", id)))
    }

    /// Find a feature by ID
    fn find_feature(&self, id: &str) -> ServiceResult<Feature> {
        let dir = self.feature_dir();
        if let Some((_, feature)) = loader::load_entity::<Feature>(&dir, id)? {
            return Ok(feature);
        }
        Err(ServiceError::NotFound(format!("Feature: {}", id)))
    }

    /// List mates with filtering and sorting
    pub fn list(&self, filter: &MateFilter) -> ServiceResult<Vec<Mate>> {
        let mut mates = self.load_all()?;

        // Apply filters
        mates.retain(|mate| {
            // Common filter
            if !filter.common.matches_status_str(mate.status()) {
                return false;
            }
            if !filter.common.matches_author(&mate.author) {
                return false;
            }
            if !filter.common.matches_search(&[&mate.title]) {
                return false;
            }

            // Mate type filter
            if let Some(ref mt) = filter.mate_type {
                if &mate.mate_type != mt {
                    return false;
                }
            }

            // Fit result filter
            if let Some(ref fr) = filter.fit_result {
                if let Some(ref analysis) = mate.fit_analysis {
                    if &analysis.fit_result != fr {
                        return false;
                    }
                } else {
                    return false;
                }
            }

            // Recent filter
            if let Some(days) = filter.recent_days {
                let cutoff = Utc::now() - chrono::Duration::days(days as i64);
                if mate.created < cutoff {
                    return false;
                }
            }

            true
        });

        // Sort
        crate::services::common::sort_entities(&mut mates, filter.sort, filter.sort_direction);

        Ok(mates)
    }

    /// Get a mate by ID
    pub fn get(&self, id: &str) -> ServiceResult<Option<Mate>> {
        match self.find_mate(id) {
            Ok((_, mate)) => Ok(Some(mate)),
            Err(ServiceError::NotFound(_)) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Get a mate by ID, returning error if not found
    pub fn get_required(&self, id: &str) -> ServiceResult<Mate> {
        let (_, mate) = self.find_mate(id)?;
        Ok(mate)
    }

    /// Create a new mate
    pub fn create(&self, input: CreateMate) -> ServiceResult<Mate> {
        let id = EntityId::new(EntityPrefix::Mate);

        let mut mate = Mate::new(
            input.title,
            input.feature_a,
            input.feature_b,
            input.mate_type,
            input.author,
        );
        mate.id = id;
        mate.description = input.description;
        mate.notes = input.notes;
        mate.tags = input.tags;

        // Ensure directory exists
        let dir = self.mate_dir();
        if !dir.exists() {
            fs::create_dir_all(&dir)?;
        }

        // Save
        let file_path = self.get_file_path(&mate.id);
        self.base.save(&mate, &file_path, Some("MATE"))?;

        Ok(mate)
    }

    /// Update an existing mate
    pub fn update(&self, id: &str, input: UpdateMate) -> ServiceResult<Mate> {
        let (_, mut mate) = self.find_mate(id)?;

        if let Some(title) = input.title {
            mate.title = title;
        }
        if let Some(description) = input.description {
            mate.description = description;
        }
        if let Some(notes) = input.notes {
            mate.notes = notes;
        }
        if let Some(mate_type) = input.mate_type {
            mate.mate_type = mate_type;
        }
        if let Some(status) = input.status {
            mate.status = status;
        }

        // Increment revision
        mate.entity_revision += 1;

        // Save
        let file_path = self.get_file_path(&mate.id);
        self.base.save(&mate, &file_path, None)?;

        Ok(mate)
    }

    /// Delete a mate. With `force=false`, refuses to delete if any stackup
    /// references this mate via `links.mates_used`. The error message lists the
    /// referencing stackup IDs so the caller can decide what to do.
    pub fn delete(&self, id: &str, force: bool) -> ServiceResult<()> {
        let (path, mate) = self.find_mate(id)?;

        if !force {
            // Check references against the resolved full ID, not the raw input:
            // `id` may be a partial ULID or short ID that would never match the
            // full IDs stored in links.mates_used.
            let full_id = mate.id.to_string();

            // Generic incoming-link guard (same protection every other entity
            // type gets from ServiceBase::delete).
            let links_to = self.cache.get_links_to(&full_id);
            if !links_to.is_empty() {
                return Err(ServiceError::HasReferences);
            }

            let referencing = self.stackups_referencing(&full_id)?;
            if !referencing.is_empty() {
                return Err(ServiceError::ValidationFailed(format!(
                    "mate {} is referenced by {} stackup(s) via links.mates_used: {}. \
                     Remove the reference(s) first or pass --force.",
                    full_id,
                    referencing.len(),
                    referencing.join(", ")
                )));
            }
        }

        fs::remove_file(&path)?;
        Ok(())
    }

    /// Return all stackup IDs that list `mate_id` in `links.mates_used`.
    /// Reads stackups directly from disk so it works without the cache being
    /// up to date (mates_used isn't currently indexed by the link cache).
    fn stackups_referencing(&self, mate_id: &str) -> ServiceResult<Vec<String>> {
        let stackup_dir = self.project.root().join("tolerances/stackups");
        if !stackup_dir.exists() {
            return Ok(Vec::new());
        }
        let stackups: Vec<crate::entities::stackup::Stackup> =
            loader::load_all(&stackup_dir).map_err(ServiceError::from)?;
        Ok(stackups
            .into_iter()
            .filter(|s| s.links.mates_used.iter().any(|m| m == mate_id))
            .map(|s| s.id.to_string())
            .collect())
    }

    /// Recalculate fit analysis for a mate from its linked features
    pub fn recalculate(&self, id: &str) -> ServiceResult<RecalcResult> {
        let (_, mut mate) = self.find_mate(id)?;

        // Load features
        let feature_a = match self.find_feature(&mate.feature_a.id.to_string()) {
            Ok(f) => f,
            Err(e) => {
                return Ok(RecalcResult {
                    mate,
                    changed: false,
                    error: Some(format!("Feature A: {}", e)),
                });
            }
        };

        let feature_b = match self.find_feature(&mate.feature_b.id.to_string()) {
            Ok(f) => f,
            Err(e) => {
                return Ok(RecalcResult {
                    mate,
                    changed: false,
                    error: Some(format!("Feature B: {}", e)),
                });
            }
        };

        // Get primary dimensions
        let dim_a = match feature_a.primary_dimension() {
            Some(d) => d,
            None => {
                return Ok(RecalcResult {
                    mate,
                    changed: false,
                    error: Some("Feature A has no dimensions".to_string()),
                });
            }
        };

        let dim_b = match feature_b.primary_dimension() {
            Some(d) => d,
            None => {
                return Ok(RecalcResult {
                    mate,
                    changed: false,
                    error: Some("Feature B has no dimensions".to_string()),
                });
            }
        };

        // Update cached feature info
        mate.feature_a = MateFeatureRef::with_cache(
            mate.feature_a.id.clone(),
            Some(feature_a.title.clone()),
            Some(feature_a.component.clone()),
            None,
        );
        mate.feature_b = MateFeatureRef::with_cache(
            mate.feature_b.id.clone(),
            Some(feature_b.title.clone()),
            Some(feature_b.component.clone()),
            None,
        );

        // Calculate fit
        let old_fit = mate.fit_analysis.clone();
        match mate.calculate_fit_from_dimensions(dim_a, dim_b) {
            Ok(()) => {}
            Err(e) => {
                return Ok(RecalcResult {
                    mate,
                    changed: false,
                    error: Some(e.to_string()),
                });
            }
        }

        let changed = old_fit != mate.fit_analysis;

        mate.entity_revision += 1;

        // Save
        let file_path = self.get_file_path(&mate.id);
        self.base.save(&mate, &file_path, None)?;

        Ok(RecalcResult {
            mate,
            changed,
            error: None,
        })
    }

    /// Recalculate all mates
    pub fn recalculate_all(&self) -> ServiceResult<Vec<RecalcResult>> {
        let mates = self.load_all()?;
        let mut results = Vec::with_capacity(mates.len());

        for mate in mates {
            let result = self.recalculate(&mate.id.to_string())?;
            results.push(result);
        }

        Ok(results)
    }

    /// Check if a mate's fit matches its intended type
    pub fn check_fit_matches_intent(&self, mate: &Mate) -> Option<bool> {
        mate.fit_analysis.as_ref().map(|analysis| {
            match mate.mate_type {
                MateType::Clearance => analysis.fit_result == FitResult::Clearance,
                MateType::Interference => analysis.fit_result == FitResult::Interference,
                MateType::Transition => true, // Transition is always acceptable
            }
        })
    }

    /// Add a verifies link to a requirement
    pub fn add_verifies_link(&self, id: &str, requirement_id: &str) -> ServiceResult<Mate> {
        let (_, mut mate) = self.find_mate(id)?;

        if !mate.links.verifies.contains(&requirement_id.to_string()) {
            mate.links.verifies.push(requirement_id.to_string());
        }
        mate.entity_revision += 1;

        let file_path = self.get_file_path(&mate.id);
        self.base.save(&mate, &file_path, None)?;

        Ok(mate)
    }

    /// Remove a verifies link
    pub fn remove_verifies_link(&self, id: &str, requirement_id: &str) -> ServiceResult<Mate> {
        let (_, mut mate) = self.find_mate(id)?;

        mate.links.verifies.retain(|r| r != requirement_id);
        mate.entity_revision += 1;

        let file_path = self.get_file_path(&mate.id);
        self.base.save(&mate, &file_path, None)?;

        Ok(mate)
    }

    /// Get statistics about mates
    pub fn stats(&self) -> ServiceResult<MateStats> {
        let mates = self.load_all()?;

        let mut stats = MateStats {
            total: mates.len(),
            ..Default::default()
        };

        for mate in &mates {
            // Count by type
            match mate.mate_type {
                MateType::Clearance => stats.by_type.clearance += 1,
                MateType::Transition => stats.by_type.transition += 1,
                MateType::Interference => stats.by_type.interference += 1,
            }

            // Count by fit result
            if let Some(ref analysis) = mate.fit_analysis {
                match analysis.fit_result {
                    FitResult::Clearance => stats.by_fit.clearance += 1,
                    FitResult::Transition => stats.by_fit.transition += 1,
                    FitResult::Interference => stats.by_fit.interference += 1,
                }
                stats.analyzed_count += 1;

                // Check if fit matches intent
                if let Some(true) = self.check_fit_matches_intent(mate) {
                    stats.matches_intent += 1;
                }
            } else {
                stats.by_fit.not_analyzed += 1;
            }
        }

        Ok(stats)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup() -> (TempDir, Project, EntityCache) {
        let tmp = TempDir::new().unwrap();
        let project = Project::init(tmp.path()).unwrap();
        let cache = EntityCache::open(&project).unwrap();
        (tmp, project, cache)
    }

    fn make_create_mate(title: &str) -> CreateMate {
        CreateMate {
            title: title.to_string(),
            feature_a: EntityId::new(EntityPrefix::Feat),
            feature_b: EntityId::new(EntityPrefix::Feat),
            mate_type: MateType::Clearance,
            description: None,
            notes: None,
            tags: Vec::new(),
            author: "Test Author".to_string(),
        }
    }

    #[test]
    fn test_create_mate() {
        let (_tmp, project, cache) = setup();
        let service = MateService::new(&project, &cache);

        let input = make_create_mate("Test Mate");
        let mate = service.create(input).unwrap();

        assert!(mate.id.to_string().starts_with("MATE-"));
        assert_eq!(mate.title, "Test Mate");
        assert_eq!(mate.mate_type, MateType::Clearance);
    }

    #[test]
    fn test_get_mate() {
        let (_tmp, project, cache) = setup();
        let service = MateService::new(&project, &cache);

        let input = make_create_mate("Test Mate");
        let created = service.create(input).unwrap();

        let retrieved = service.get(&created.id.to_string()).unwrap().unwrap();
        assert_eq!(retrieved.id, created.id);
        assert_eq!(retrieved.title, "Test Mate");
    }

    #[test]
    fn test_list_with_filter() {
        let (_tmp, project, cache) = setup();
        let service = MateService::new(&project, &cache);

        // Create multiple mates
        service.create(make_create_mate("Mate A")).unwrap();

        let mut input_b = make_create_mate("Mate B");
        input_b.mate_type = MateType::Interference;
        let mate_b = service.create(input_b).unwrap();

        service.create(make_create_mate("Mate C")).unwrap();

        // List all
        let all = service.list(&MateFilter::default()).unwrap();
        assert_eq!(all.len(), 3);

        // List interference only
        let interference = service
            .list(&MateFilter {
                mate_type: Some(MateType::Interference),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(interference.len(), 1);
        assert_eq!(interference[0].id, mate_b.id);
    }

    #[test]
    fn test_update_mate() {
        let (_tmp, project, cache) = setup();
        let service = MateService::new(&project, &cache);

        let input = make_create_mate("Test Mate");
        let created = service.create(input).unwrap();

        let updated = service
            .update(
                &created.id.to_string(),
                UpdateMate {
                    title: Some("Updated Mate".to_string()),
                    mate_type: Some(MateType::Transition),
                    ..Default::default()
                },
            )
            .unwrap();

        assert_eq!(updated.title, "Updated Mate");
        assert_eq!(updated.mate_type, MateType::Transition);
        assert_eq!(updated.entity_revision, 2);
    }

    #[test]
    fn test_delete_mate() {
        let (_tmp, project, cache) = setup();
        let service = MateService::new(&project, &cache);

        let input = make_create_mate("Test Mate");
        let created = service.create(input).unwrap();

        service.delete(&created.id.to_string(), false).unwrap();

        let result = service.get(&created.id.to_string()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_verifies_links() {
        let (_tmp, project, cache) = setup();
        let service = MateService::new(&project, &cache);

        let input = make_create_mate("Test Mate");
        let mate = service.create(input).unwrap();

        let mate = service
            .add_verifies_link(&mate.id.to_string(), "REQ-001")
            .unwrap();
        assert!(mate.links.verifies.contains(&"REQ-001".to_string()));

        let mate = service
            .remove_verifies_link(&mate.id.to_string(), "REQ-001")
            .unwrap();
        assert!(!mate.links.verifies.contains(&"REQ-001".to_string()));
    }

    #[test]
    fn test_stats() {
        let (_tmp, project, cache) = setup();
        let service = MateService::new(&project, &cache);

        // Create mates with different types
        service.create(make_create_mate("Mate A")).unwrap();

        let mut input_b = make_create_mate("Mate B");
        input_b.mate_type = MateType::Interference;
        service.create(input_b).unwrap();

        let mut input_c = make_create_mate("Mate C");
        input_c.mate_type = MateType::Transition;
        service.create(input_c).unwrap();

        let stats = service.stats().unwrap();
        assert_eq!(stats.total, 3);
        assert_eq!(stats.by_type.clearance, 1);
        assert_eq!(stats.by_type.interference, 1);
        assert_eq!(stats.by_type.transition, 1);
        assert_eq!(stats.by_fit.not_analyzed, 3);
    }

    #[test]
    fn test_delete_mate_blocked_when_referenced_by_stackup() {
        use crate::entities::stackup::Stackup;
        use crate::services::stackup::{CreateStackup, StackupService};

        let (_tmp, project, cache) = setup();
        let mate_service = MateService::new(&project, &cache);
        let stackup_service = StackupService::new(&project, &cache);

        let mate = mate_service.create(make_create_mate("Mate")).unwrap();
        let stackup = stackup_service
            .create(CreateStackup {
                title: "S".into(),
                target_name: "Gap".into(),
                target_nominal: 1.0,
                target_upper: 1.5,
                target_lower: 0.5,
                units: None,
                critical: false,
                description: None,
                sigma_level: None,
                mean_shift_k: None,
                include_gdt: false,
                tags: Vec::new(),
                author: "Test".into(),
            })
            .unwrap();

        // Manually wire the stackup → mate reference (no service helper yet).
        let stackup_path = project
            .root()
            .join("tolerances/stackups")
            .join(format!("{}.tdt.yaml", stackup.id));
        let mut s: Stackup = crate::yaml::parse_yaml_file(&stackup_path).unwrap();
        s.links.mates_used.push(mate.id.to_string());
        let yaml = serde_yml::to_string(&s).unwrap();
        std::fs::write(&stackup_path, yaml).unwrap();

        // Without --force, deletion must be refused with a descriptive error.
        let err = mate_service
            .delete(&mate.id.to_string(), false)
            .unwrap_err();
        match err {
            ServiceError::ValidationFailed(msg) => {
                assert!(msg.contains(&stackup.id.to_string()));
                assert!(msg.contains("mates_used"));
            }
            other => panic!("expected ValidationFailed, got {:?}", other),
        }

        // With force, deletion proceeds.
        mate_service.delete(&mate.id.to_string(), true).unwrap();
        assert!(mate_service.get(&mate.id.to_string()).unwrap().is_none());
    }

    #[test]
    fn test_check_fit_matches_intent() {
        let (_tmp, project, cache) = setup();
        let service = MateService::new(&project, &cache);

        let input = make_create_mate("Test Mate");
        let mut mate = service.create(input).unwrap();

        // No analysis - returns None
        assert!(service.check_fit_matches_intent(&mate).is_none());

        // Add fit analysis with clearance result
        mate.calculate_fit((10.0, 0.1, 0.0), (9.9, 0.0, 0.1));

        // Clearance fit for clearance intent - matches
        assert_eq!(service.check_fit_matches_intent(&mate), Some(true));

        // Change intent to interference - doesn't match
        mate.mate_type = MateType::Interference;
        assert_eq!(service.check_fit_matches_intent(&mate), Some(false));

        // Transition intent always matches
        mate.mate_type = MateType::Transition;
        assert_eq!(service.check_fit_matches_intent(&mate), Some(true));
    }
}
