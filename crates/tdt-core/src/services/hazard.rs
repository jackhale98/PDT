//! Hazard service for safety analysis management
//!
//! Provides CRUD operations and safety analysis for hazards.

use crate::core::suspect::LinkRef;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::core::cache::{CachedEntity, EntityCache, EntityFilter};
use crate::core::entity::{Entity, Status};
use crate::core::identity::{EntityId, EntityPrefix};
use crate::core::loader;
use crate::core::project::Project;
use crate::entities::hazard::{Hazard, HazardCategory, HazardSeverity};
use crate::services::base::ServiceBase;
use crate::services::common::{
    CommonFilter, ServiceError, ServiceResult, SortDirection, SortKey, Sortable,
};

/// Filter options for listing hazards
#[derive(Debug, Clone, Default)]
pub struct HazardFilter {
    /// Common filters (status, author, tags, search)
    pub common: CommonFilter,
    /// Filter by hazard category
    pub category: Option<HazardCategory>,
    /// Filter by severity
    pub severity: Option<HazardSeverity>,
    /// Show only uncontrolled hazards
    pub uncontrolled_only: bool,
    /// Show recent hazards (last N days)
    pub recent_days: Option<u32>,
    /// Sort field
    pub sort: HazardSortField,
    /// Sort direction
    pub sort_direction: SortDirection,
}

/// Fields available for sorting hazards
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HazardSortField {
    Id,
    Title,
    Category,
    Severity,
    RiskCount,
    ControlCount,
    Status,
    Author,
    #[default]
    Created,
}

impl Sortable for Hazard {
    type SortField = HazardSortField;

    fn sort_key(&self, field: &Self::SortField) -> SortKey {
        match field {
            HazardSortField::Id => SortKey::String(self.id.to_string()),
            HazardSortField::Title => SortKey::String(self.title.clone()),
            HazardSortField::Category => SortKey::String(self.category.to_string()),
            HazardSortField::Severity => {
                // Sort by severity: Catastrophic (5) > Severe (4) > Serious (3) > Minor (2) > Negligible (1)
                let severity_num = match self.severity {
                    HazardSeverity::Catastrophic => 5,
                    HazardSeverity::Severe => 4,
                    HazardSeverity::Serious => 3,
                    HazardSeverity::Minor => 2,
                    HazardSeverity::Negligible => 1,
                };
                SortKey::Number(severity_num)
            }
            HazardSortField::RiskCount => SortKey::Number(self.risk_count() as i64),
            HazardSortField::ControlCount => SortKey::Number(self.control_count() as i64),
            HazardSortField::Status => SortKey::String(self.status().to_string()),
            HazardSortField::Author => SortKey::String(self.author.clone()),
            HazardSortField::Created => SortKey::DateTime(self.created.timestamp()),
        }
    }
}

/// Input for creating a new hazard
#[derive(Debug, Clone)]
pub struct CreateHazard {
    /// Hazard title
    pub title: String,
    /// Hazard category
    pub category: HazardCategory,
    /// Detailed description
    pub description: String,
    /// Potential harms
    pub potential_harms: Vec<String>,
    /// Energy level
    pub energy_level: Option<String>,
    /// Severity
    pub severity: HazardSeverity,
    /// Exposure scenario
    pub exposure_scenario: Option<String>,
    /// Affected populations
    pub affected_populations: Vec<String>,
    /// Tags
    pub tags: Vec<String>,
    /// Author
    pub author: String,
}

/// Input for updating an existing hazard
#[derive(Debug, Clone, Default)]
pub struct UpdateHazard {
    /// Update title
    pub title: Option<String>,
    /// Update category
    pub category: Option<HazardCategory>,
    /// Update description
    pub description: Option<String>,
    /// Update energy level
    pub energy_level: Option<Option<String>>,
    /// Update severity
    pub severity: Option<HazardSeverity>,
    /// Update exposure scenario
    pub exposure_scenario: Option<Option<String>>,
    /// Update document status
    pub status: Option<Status>,
}

/// Statistics about hazards
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HazardStats {
    /// Total number of hazards
    pub total: usize,
    /// Counts by category
    pub by_category: HazardCategoryCounts,
    /// Counts by severity
    pub by_severity: HazardSeverityCounts,
    /// Number of controlled hazards
    pub controlled_count: usize,
    /// Number of uncontrolled hazards
    pub uncontrolled_count: usize,
    /// Total risks linked
    pub total_risks: usize,
    /// Total controls linked
    pub total_controls: usize,
}

/// Hazard category counts
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HazardCategoryCounts {
    pub electrical: usize,
    pub mechanical: usize,
    pub thermal: usize,
    pub chemical: usize,
    pub biological: usize,
    pub radiation: usize,
    pub ergonomic: usize,
    pub software: usize,
    pub environmental: usize,
}

/// Hazard severity counts
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HazardSeverityCounts {
    pub negligible: usize,
    pub minor: usize,
    pub serious: usize,
    pub severe: usize,
    pub catastrophic: usize,
}

/// Service for managing hazards
pub struct HazardService<'a> {
    project: &'a Project,
    base: ServiceBase<'a>,
    cache: &'a EntityCache,
}

impl<'a> HazardService<'a> {
    /// Create a new HazardService
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

    /// Get the hazards directory
    fn hazard_dir(&self) -> PathBuf {
        self.project().root().join("safety/hazards")
    }

    /// Get the file path for a hazard
    fn get_file_path(&self, id: &EntityId) -> PathBuf {
        self.hazard_dir().join(format!("{}.tdt.yaml", id))
    }

    /// Load all hazards
    fn load_all(&self) -> ServiceResult<Vec<Hazard>> {
        let dir = self.hazard_dir();
        if !dir.exists() {
            return Ok(Vec::new());
        }
        loader::load_all(&dir).map_err(ServiceError::from)
    }

    /// List hazards using the cache (fast path)
    ///
    /// Returns cached entity data without loading full entities from disk.
    /// Use this for list views and simple queries.
    pub fn list_cached(&self, filter: &HazardFilter) -> ServiceResult<Vec<CachedEntity>> {
        // Build cache filter
        let status = filter
            .common
            .status
            .as_ref()
            .and_then(|s| s.first())
            .copied();

        let entity_filter = EntityFilter {
            prefix: Some(EntityPrefix::Haz),
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

        // Note: category, severity, uncontrolled_only require full entity load
        // These are handled in the regular list() method

        // Apply limit
        if let Some(limit) = filter.common.limit {
            cached.truncate(limit);
        }

        Ok(cached)
    }

    /// Find a hazard by ID
    ///
    /// Uses the cache to find the file path for faster lookup.
    fn find_hazard(&self, id: &str) -> ServiceResult<(PathBuf, Hazard)> {
        // Try to find in cache first for fast path lookup
        if let Some(cached) = self.cache.get_entity(id) {
            let path = if cached.file_path.is_absolute() {
                cached.file_path.clone()
            } else {
                self.project.root().join(&cached.file_path)
            };
            if path.exists() {
                if let Ok(hazard) = crate::yaml::parse_yaml_file::<Hazard>(&path) {
                    return Ok((path, hazard));
                }
            }
        }

        // Fall back to directory scan
        let dir = self.hazard_dir();
        if let Some((path, hazard)) = loader::load_entity::<Hazard>(&dir, id)? {
            return Ok((path, hazard));
        }
        Err(ServiceError::NotFound(format!("Hazard: {}", id)))
    }

    /// List hazards with filtering and sorting
    pub fn list(&self, filter: &HazardFilter) -> ServiceResult<Vec<Hazard>> {
        let mut hazards = self.load_all()?;

        // Apply filters
        hazards.retain(|hazard| {
            // Common filter
            if !filter.common.matches_status_str(hazard.status()) {
                return false;
            }
            if !filter.common.matches_author(&hazard.author) {
                return false;
            }
            if !filter
                .common
                .matches_search(&[&hazard.title, &hazard.description])
            {
                return false;
            }

            // Category filter
            if let Some(ref cat) = filter.category {
                if &hazard.category != cat {
                    return false;
                }
            }

            // Severity filter
            if let Some(ref sev) = filter.severity {
                if &hazard.severity != sev {
                    return false;
                }
            }

            // Uncontrolled filter
            if filter.uncontrolled_only && hazard.is_controlled() {
                return false;
            }

            // Recent filter
            if let Some(days) = filter.recent_days {
                let cutoff = Utc::now() - chrono::Duration::days(days as i64);
                if hazard.created < cutoff {
                    return false;
                }
            }

            true
        });

        // Sort
        crate::services::common::sort_entities(&mut hazards, filter.sort, filter.sort_direction);

        Ok(hazards)
    }

    /// Get a hazard by ID
    pub fn get(&self, id: &str) -> ServiceResult<Option<Hazard>> {
        match self.find_hazard(id) {
            Ok((_, hazard)) => Ok(Some(hazard)),
            Err(ServiceError::NotFound(_)) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Get a hazard by ID, returning error if not found
    pub fn get_required(&self, id: &str) -> ServiceResult<Hazard> {
        let (_, hazard) = self.find_hazard(id)?;
        Ok(hazard)
    }

    /// Create a new hazard
    pub fn create(&self, input: CreateHazard) -> ServiceResult<Hazard> {
        let id = EntityId::new(EntityPrefix::Haz);

        let mut hazard = Hazard::new(
            id,
            input.title,
            input.category,
            input.description,
            input.author,
        );
        hazard.potential_harms = input.potential_harms;
        hazard.energy_level = input.energy_level;
        hazard.severity = input.severity;
        hazard.exposure_scenario = input.exposure_scenario;
        hazard.affected_populations = input.affected_populations;
        hazard.tags = input.tags;

        // Ensure directory exists
        let dir = self.hazard_dir();
        if !dir.exists() {
            fs::create_dir_all(&dir)?;
        }

        // Save
        let file_path = self.get_file_path(&hazard.id);
        self.base.save(&hazard, &file_path, Some("HAZ"))?;

        Ok(hazard)
    }

    /// Update an existing hazard
    pub fn update(&self, id: &str, input: UpdateHazard) -> ServiceResult<Hazard> {
        let (_, mut hazard) = self.find_hazard(id)?;

        if let Some(title) = input.title {
            hazard.title = title;
        }
        if let Some(category) = input.category {
            hazard.category = category;
        }
        if let Some(description) = input.description {
            hazard.description = description;
        }
        if let Some(energy_level) = input.energy_level {
            hazard.energy_level = energy_level;
        }
        if let Some(severity) = input.severity {
            hazard.severity = severity;
        }
        if let Some(exposure_scenario) = input.exposure_scenario {
            hazard.exposure_scenario = exposure_scenario;
        }
        if let Some(status) = input.status {
            hazard.status = status;
        }

        // Increment revision
        hazard.revision += 1;

        // Save
        let file_path = self.get_file_path(&hazard.id);
        self.base.save(&hazard, &file_path, None)?;

        Ok(hazard)
    }

    /// Delete a hazard
    pub fn delete(&self, id: &str, force: bool) -> ServiceResult<()> {
        let (path, hazard) = self.find_hazard(id)?;

        if !force {
            // Check for linked risks
            if !hazard.links.causes.is_empty() {
                return Err(ServiceError::ValidationFailed(format!(
                    "Hazard has {} linked risk(s). Use --force to delete anyway.",
                    hazard.links.causes.len()
                )));
            }
        }

        fs::remove_file(&path)?;
        Ok(())
    }

    /// Add a potential harm to a hazard
    pub fn add_harm(&self, id: &str, harm: &str) -> ServiceResult<Hazard> {
        let (_, mut hazard) = self.find_hazard(id)?;

        if !hazard.potential_harms.contains(&harm.to_string()) {
            hazard.potential_harms.push(harm.to_string());
        }
        hazard.revision += 1;

        let file_path = self.get_file_path(&hazard.id);
        self.base.save(&hazard, &file_path, None)?;

        Ok(hazard)
    }

    /// Remove a potential harm from a hazard
    pub fn remove_harm(&self, id: &str, harm: &str) -> ServiceResult<Hazard> {
        let (_, mut hazard) = self.find_hazard(id)?;

        hazard.potential_harms.retain(|h| h != harm);
        hazard.revision += 1;

        let file_path = self.get_file_path(&hazard.id);
        self.base.save(&hazard, &file_path, None)?;

        Ok(hazard)
    }

    /// Add an affected population
    pub fn add_population(&self, id: &str, population: &str) -> ServiceResult<Hazard> {
        let (_, mut hazard) = self.find_hazard(id)?;

        if !hazard
            .affected_populations
            .contains(&population.to_string())
        {
            hazard.affected_populations.push(population.to_string());
        }
        hazard.revision += 1;

        let file_path = self.get_file_path(&hazard.id);
        self.base.save(&hazard, &file_path, None)?;

        Ok(hazard)
    }

    /// Link a risk to this hazard
    pub fn add_risk_link(&self, id: &str, risk_id: &EntityId) -> ServiceResult<Hazard> {
        let (_, mut hazard) = self.find_hazard(id)?;

        if !hazard.links.causes.iter().any(|l| *l == *risk_id) {
            hazard.links.causes.push(LinkRef::from(risk_id));
        }
        hazard.revision += 1;

        let file_path = self.get_file_path(&hazard.id);
        self.base.save(&hazard, &file_path, None)?;

        Ok(hazard)
    }

    /// Link a control to this hazard
    pub fn add_control_link(&self, id: &str, control_id: &EntityId) -> ServiceResult<Hazard> {
        let (_, mut hazard) = self.find_hazard(id)?;

        if !hazard.links.controlled_by.iter().any(|l| *l == *control_id) {
            hazard.links.controlled_by.push(LinkRef::from(control_id));
        }
        hazard.revision += 1;

        let file_path = self.get_file_path(&hazard.id);
        self.base.save(&hazard, &file_path, None)?;

        Ok(hazard)
    }

    /// Remove a control link from this hazard
    pub fn remove_control_link(&self, id: &str, control_id: &EntityId) -> ServiceResult<Hazard> {
        let (_, mut hazard) = self.find_hazard(id)?;

        hazard.links.controlled_by.retain(|c| c != control_id);
        hazard.revision += 1;

        let file_path = self.get_file_path(&hazard.id);
        self.base.save(&hazard, &file_path, None)?;

        Ok(hazard)
    }

    /// Get statistics about hazards
    pub fn stats(&self) -> ServiceResult<HazardStats> {
        let hazards = self.load_all()?;

        let mut stats = HazardStats {
            total: hazards.len(),
            ..Default::default()
        };

        for hazard in &hazards {
            // Count by category
            match hazard.category {
                HazardCategory::Electrical => stats.by_category.electrical += 1,
                HazardCategory::Mechanical => stats.by_category.mechanical += 1,
                HazardCategory::Thermal => stats.by_category.thermal += 1,
                HazardCategory::Chemical => stats.by_category.chemical += 1,
                HazardCategory::Biological => stats.by_category.biological += 1,
                HazardCategory::Radiation => stats.by_category.radiation += 1,
                HazardCategory::Ergonomic => stats.by_category.ergonomic += 1,
                HazardCategory::Software => stats.by_category.software += 1,
                HazardCategory::Environmental => stats.by_category.environmental += 1,
            }

            // Count by severity
            match hazard.severity {
                HazardSeverity::Negligible => stats.by_severity.negligible += 1,
                HazardSeverity::Minor => stats.by_severity.minor += 1,
                HazardSeverity::Serious => stats.by_severity.serious += 1,
                HazardSeverity::Severe => stats.by_severity.severe += 1,
                HazardSeverity::Catastrophic => stats.by_severity.catastrophic += 1,
            }

            // Controlled/uncontrolled
            if hazard.is_controlled() {
                stats.controlled_count += 1;
            } else {
                stats.uncontrolled_count += 1;
            }

            // Link counts
            stats.total_risks += hazard.risk_count();
            stats.total_controls += hazard.control_count();
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

    fn make_create_hazard(title: &str) -> CreateHazard {
        CreateHazard {
            title: title.to_string(),
            category: HazardCategory::Electrical,
            description: "Test description".to_string(),
            potential_harms: Vec::new(),
            energy_level: None,
            severity: HazardSeverity::Minor,
            exposure_scenario: None,
            affected_populations: Vec::new(),
            tags: Vec::new(),
            author: "Test Author".to_string(),
        }
    }

    #[test]
    fn test_create_hazard() {
        let (_tmp, project, cache) = setup();
        let service = HazardService::new(&project, &cache);

        let mut input = make_create_hazard("High voltage");
        input.severity = HazardSeverity::Severe;

        let hazard = service.create(input).unwrap();

        assert!(hazard.id.to_string().starts_with("HAZ-"));
        assert_eq!(hazard.title, "High voltage");
        assert_eq!(hazard.category, HazardCategory::Electrical);
        assert_eq!(hazard.severity, HazardSeverity::Severe);
    }

    #[test]
    fn test_get_hazard() {
        let (_tmp, project, cache) = setup();
        let service = HazardService::new(&project, &cache);

        let input = make_create_hazard("Test Hazard");
        let created = service.create(input).unwrap();

        let retrieved = service.get(&created.id.to_string()).unwrap().unwrap();
        assert_eq!(retrieved.id, created.id);
        assert_eq!(retrieved.title, "Test Hazard");
    }

    #[test]
    fn test_list_with_filter() {
        let (_tmp, project, cache) = setup();
        let service = HazardService::new(&project, &cache);

        // Create multiple hazards
        service.create(make_create_hazard("Hazard A")).unwrap();

        let mut input_b = make_create_hazard("Hazard B");
        input_b.category = HazardCategory::Mechanical;
        let hazard_b = service.create(input_b).unwrap();

        service.create(make_create_hazard("Hazard C")).unwrap();

        // List all
        let all = service.list(&HazardFilter::default()).unwrap();
        assert_eq!(all.len(), 3);

        // List mechanical only
        let mechanical = service
            .list(&HazardFilter {
                category: Some(HazardCategory::Mechanical),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(mechanical.len(), 1);
        assert_eq!(mechanical[0].id, hazard_b.id);
    }

    #[test]
    fn test_update_hazard() {
        let (_tmp, project, cache) = setup();
        let service = HazardService::new(&project, &cache);

        let input = make_create_hazard("Test Hazard");
        let created = service.create(input).unwrap();

        let updated = service
            .update(
                &created.id.to_string(),
                UpdateHazard {
                    title: Some("Updated Hazard".to_string()),
                    severity: Some(HazardSeverity::Catastrophic),
                    ..Default::default()
                },
            )
            .unwrap();

        assert_eq!(updated.title, "Updated Hazard");
        assert_eq!(updated.severity, HazardSeverity::Catastrophic);
        assert_eq!(updated.revision, 2);
    }

    #[test]
    fn test_delete_hazard() {
        let (_tmp, project, cache) = setup();
        let service = HazardService::new(&project, &cache);

        let input = make_create_hazard("Test Hazard");
        let created = service.create(input).unwrap();

        service.delete(&created.id.to_string(), false).unwrap();

        let result = service.get(&created.id.to_string()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_add_harm() {
        let (_tmp, project, cache) = setup();
        let service = HazardService::new(&project, &cache);

        let input = make_create_hazard("Test Hazard");
        let hazard = service.create(input).unwrap();

        let hazard = service
            .add_harm(&hazard.id.to_string(), "Electric shock")
            .unwrap();
        assert!(hazard
            .potential_harms
            .contains(&"Electric shock".to_string()));

        let hazard = service
            .remove_harm(&hazard.id.to_string(), "Electric shock")
            .unwrap();
        assert!(!hazard
            .potential_harms
            .contains(&"Electric shock".to_string()));
    }

    #[test]
    fn test_control_links() {
        let (_tmp, project, cache) = setup();
        let service = HazardService::new(&project, &cache);

        let input = make_create_hazard("Test Hazard");
        let hazard = service.create(input).unwrap();
        assert!(!hazard.is_controlled());

        let control_id = EntityId::new(EntityPrefix::Ctrl);
        let hazard = service
            .add_control_link(&hazard.id.to_string(), &control_id)
            .unwrap();
        assert!(hazard.is_controlled());
        assert_eq!(hazard.control_count(), 1);

        let hazard = service
            .remove_control_link(&hazard.id.to_string(), &control_id)
            .unwrap();
        assert!(!hazard.is_controlled());
    }

    #[test]
    fn test_stats() {
        let (_tmp, project, cache) = setup();
        let service = HazardService::new(&project, &cache);

        // Create hazards with different categories and severities
        let mut input_a = make_create_hazard("Hazard A");
        input_a.severity = HazardSeverity::Catastrophic;
        service.create(input_a).unwrap();

        let mut input_b = make_create_hazard("Hazard B");
        input_b.category = HazardCategory::Mechanical;
        input_b.severity = HazardSeverity::Serious;
        service.create(input_b).unwrap();

        let mut input_c = make_create_hazard("Hazard C");
        input_c.category = HazardCategory::Thermal;
        service.create(input_c).unwrap();

        let stats = service.stats().unwrap();
        assert_eq!(stats.total, 3);
        assert_eq!(stats.by_category.electrical, 1);
        assert_eq!(stats.by_category.mechanical, 1);
        assert_eq!(stats.by_category.thermal, 1);
        assert_eq!(stats.by_severity.catastrophic, 1);
        assert_eq!(stats.by_severity.serious, 1);
        assert_eq!(stats.by_severity.minor, 1);
        assert_eq!(stats.uncontrolled_count, 3);
    }

    #[test]
    fn test_uncontrolled_filter() {
        let (_tmp, project, cache) = setup();
        let service = HazardService::new(&project, &cache);

        // Create two hazards
        let hazard_a = service.create(make_create_hazard("Hazard A")).unwrap();
        let hazard_b = service.create(make_create_hazard("Hazard B")).unwrap();

        // Control one of them
        let control_id = EntityId::new(EntityPrefix::Ctrl);
        service
            .add_control_link(&hazard_a.id.to_string(), &control_id)
            .unwrap();

        // Filter for uncontrolled only
        let uncontrolled = service
            .list(&HazardFilter {
                uncontrolled_only: true,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(uncontrolled.len(), 1);
        assert_eq!(uncontrolled[0].id, hazard_b.id);
    }
}
