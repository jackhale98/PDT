//! Control service - business logic for control plan item management
//!
//! Provides CRUD operations and characteristic/measurement management for controls.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::core::cache::EntityCache;
use crate::core::entity::Status;
use crate::core::identity::{EntityId, EntityPrefix};
use crate::core::loader;
use crate::core::project::Project;
use crate::entities::control::{
    Characteristic, Control, ControlCategory, ControlLimits, ControlLinks, ControlType,
    Measurement, Sampling,
};

use super::base::ServiceBase;
use super::common::{
    apply_pagination, CommonFilter, ListResult, ServiceError, ServiceResult, SortDirection,
};

/// Filter options specific to controls
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ControlFilter {
    /// Common filter options (status, author, search, etc.)
    #[serde(flatten)]
    pub common: CommonFilter,

    /// Filter by control type
    pub control_type: Option<ControlType>,

    /// Filter by control category (variable/attribute)
    pub control_category: Option<ControlCategory>,

    /// Filter by process ID
    pub process: Option<String>,

    /// Show only critical (CTQ) controls
    pub critical_only: bool,

    /// Show only controls with control limits defined
    pub has_limits: bool,

    /// Show only controls with measurement defined
    pub has_measurement: bool,

    /// Show only controls with sampling defined
    pub has_sampling: bool,
}

impl ControlFilter {
    /// Create a filter for SPC controls
    pub fn spc() -> Self {
        Self {
            control_type: Some(ControlType::Spc),
            ..Default::default()
        }
    }

    /// Create a filter for inspection controls
    pub fn inspection() -> Self {
        Self {
            control_type: Some(ControlType::Inspection),
            ..Default::default()
        }
    }

    /// Create a filter for critical (CTQ) controls
    pub fn critical() -> Self {
        Self {
            critical_only: true,
            ..Default::default()
        }
    }

    /// Create a filter for controls linked to a specific process
    pub fn for_process(process_id: &str) -> Self {
        Self {
            process: Some(process_id.to_string()),
            ..Default::default()
        }
    }
}

/// Sort field for controls
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ControlSortField {
    Id,
    #[default]
    Title,
    ControlType,
    Status,
    Author,
    Created,
}

/// Input for creating a new control
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CreateControl {
    /// Control title
    pub title: String,

    /// Author name
    pub author: String,

    /// Control type
    #[serde(default)]
    pub control_type: ControlType,

    /// Control category (variable/attribute)
    #[serde(default)]
    pub control_category: ControlCategory,

    /// Detailed description
    #[serde(default)]
    pub description: Option<String>,

    /// Characteristic being controlled
    #[serde(default)]
    pub characteristic: Option<Characteristic>,

    /// Linked process ID
    #[serde(default)]
    pub process: Option<String>,

    /// Linked feature ID
    #[serde(default)]
    pub feature: Option<String>,

    /// Classification tags
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Input for updating an existing control
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateControl {
    /// Update title
    pub title: Option<String>,

    /// Update control type
    pub control_type: Option<ControlType>,

    /// Update control category
    pub control_category: Option<ControlCategory>,

    /// Update description
    pub description: Option<String>,

    /// Update characteristic
    pub characteristic: Option<Characteristic>,

    /// Update measurement
    pub measurement: Option<Measurement>,

    /// Update sampling
    pub sampling: Option<Sampling>,

    /// Update control limits
    pub control_limits: Option<ControlLimits>,

    /// Update reaction plan
    pub reaction_plan: Option<String>,

    /// Update status
    pub status: Option<Status>,

    /// Replace tags
    pub tags: Option<Vec<String>>,

    /// Update linked process
    pub process: Option<String>,

    /// Update linked feature
    pub feature: Option<String>,
}

/// Statistics about controls
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ControlStats {
    pub total: usize,
    pub by_type: ControlTypeCounts,
    pub by_category: ControlCategoryCounts,
    pub by_status: StatusCounts,
    pub critical_count: usize,
    pub with_limits: usize,
    pub with_measurement: usize,
    pub with_sampling: usize,
    pub with_reaction_plan: usize,
}

/// Counts by control type
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ControlTypeCounts {
    pub spc: usize,
    pub inspection: usize,
    pub poka_yoke: usize,
    pub visual: usize,
    pub functional_test: usize,
    pub attribute: usize,
}

/// Counts by control category
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ControlCategoryCounts {
    pub variable: usize,
    pub attribute: usize,
}

/// Counts by status
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StatusCounts {
    pub draft: usize,
    pub review: usize,
    pub approved: usize,
    pub released: usize,
    pub obsolete: usize,
}

/// Service for control plan management
pub struct ControlService<'a> {
    project: &'a Project,
    cache: &'a EntityCache,
    base: ServiceBase<'a>,
}

impl<'a> ControlService<'a> {
    /// Create a new control service
    pub fn new(project: &'a Project, cache: &'a EntityCache) -> Self {
        Self {
            project,
            cache,
            base: ServiceBase::new(project, cache),
        }
    }

    /// Get the directory for storing controls
    fn get_directory(&self) -> PathBuf {
        self.project.root().join("manufacturing/controls")
    }

    /// Get the file path for a control
    fn get_file_path(&self, id: &EntityId) -> PathBuf {
        self.get_directory().join(format!("{}.tdt.yaml", id))
    }

    /// List controls with filtering and pagination
    pub fn list(
        &self,
        filter: &ControlFilter,
        sort_by: ControlSortField,
        sort_dir: SortDirection,
    ) -> ServiceResult<ListResult<Control>> {
        let mut controls = self.load_all()?;

        // Apply filters
        controls.retain(|ctrl| self.matches_filter(ctrl, filter));

        // Sort
        self.sort_controls(&mut controls, sort_by, sort_dir);

        // Paginate
        Ok(apply_pagination(
            controls,
            filter.common.offset,
            filter.common.limit,
        ))
    }

    /// List controls from cache (fast path for list display)
    ///
    /// Returns cached control data without loading full YAML files.
    /// Use this for list commands where full entity data isn't needed.
    pub fn list_cached(&self) -> Vec<crate::core::CachedEntity> {
        use crate::core::cache::EntityFilter;
        use crate::core::identity::EntityPrefix;

        let filter = EntityFilter {
            prefix: Some(EntityPrefix::Ctrl),
            ..Default::default()
        };
        self.cache.list_entities(&filter)
    }

    /// Load all controls from the filesystem
    pub fn load_all(&self) -> ServiceResult<Vec<Control>> {
        let dir = self.get_directory();
        if !dir.exists() {
            return Ok(Vec::new());
        }
        Ok(loader::load_all(&dir)?)
    }

    /// Get a single control by ID
    pub fn get(&self, id: &str) -> ServiceResult<Option<Control>> {
        let dir = self.get_directory();
        if let Some((_, ctrl)) = loader::load_entity::<Control>(&dir, id)? {
            return Ok(Some(ctrl));
        }
        Ok(None)
    }

    /// Get a control by ID, returning an error if not found
    pub fn get_required(&self, id: &str) -> ServiceResult<Control> {
        self.get(id)?
            .ok_or_else(|| ServiceError::NotFound(id.to_string()))
    }

    /// Get controls for a specific process
    pub fn get_by_process(&self, process_id: &str) -> ServiceResult<Vec<Control>> {
        let controls = self.load_all()?;
        Ok(controls
            .into_iter()
            .filter(|c| c.links.process.as_ref().is_some_and(|p| *p == process_id))
            .collect())
    }

    /// Create a new control
    pub fn create(&self, input: CreateControl) -> ServiceResult<Control> {
        let id = EntityId::new(EntityPrefix::Ctrl);

        let mut links = ControlLinks::default();
        if let Some(proc_id) = input.process {
            links.process = Some(proc_id.parse().map_err(|_| {
                ServiceError::InvalidInput(format!("Invalid process ID: {}", proc_id))
            })?);
        }
        if let Some(feat_id) = input.feature {
            links.feature = Some(feat_id.parse().map_err(|_| {
                ServiceError::InvalidInput(format!("Invalid feature ID: {}", feat_id))
            })?);
        }

        let control = Control {
            id: id.clone(),
            title: input.title,
            description: input.description,
            control_type: input.control_type,
            control_category: input.control_category,
            characteristic: input.characteristic.unwrap_or_default(),
            measurement: None,
            sampling: None,
            control_limits: None,
            reaction_plan: None,
            tags: input.tags,
            status: Status::Draft,
            links,
            created: Utc::now(),
            author: input.author,
            entity_revision: 1,
        };

        // Write to file
        let path = self.get_file_path(&id);
        self.base.save(&control, &path, Some("CTRL"))?;

        Ok(control)
    }

    /// Update an existing control
    pub fn update(&self, id: &str, input: UpdateControl) -> ServiceResult<Control> {
        let (path, mut control) = self.find_control(id)?;

        // Apply updates
        if let Some(title) = input.title {
            control.title = title;
        }
        if let Some(control_type) = input.control_type {
            control.control_type = control_type;
        }
        if let Some(control_category) = input.control_category {
            control.control_category = control_category;
        }
        if let Some(description) = input.description {
            control.description = Some(description);
        }
        if let Some(characteristic) = input.characteristic {
            control.characteristic = characteristic;
        }
        if let Some(measurement) = input.measurement {
            control.measurement = Some(measurement);
        }
        if let Some(sampling) = input.sampling {
            control.sampling = Some(sampling);
        }
        if let Some(control_limits) = input.control_limits {
            control.control_limits = Some(control_limits);
        }
        if let Some(reaction_plan) = input.reaction_plan {
            control.reaction_plan = Some(reaction_plan);
        }
        if let Some(status) = input.status {
            control.status = status;
        }
        if let Some(tags) = input.tags {
            control.tags = tags;
        }
        if let Some(process) = input.process {
            control.links.process = Some(process.parse().map_err(|_| {
                ServiceError::InvalidInput(format!("Invalid process ID: {}", process))
            })?);
        }
        if let Some(feature) = input.feature {
            control.links.feature = Some(feature.parse().map_err(|_| {
                ServiceError::InvalidInput(format!("Invalid feature ID: {}", feature))
            })?);
        }

        // Increment revision
        control.entity_revision += 1;

        // Write back
        self.base.save(&control, &path, None)?;

        Ok(control)
    }

    /// Delete a control
    pub fn delete(&self, id: &str, force: bool) -> ServiceResult<()> {
        let (path, control) = self.find_control(id)?;

        // Check for references unless force is true
        if !force {
            if !control.links.verifies.is_empty() {
                return Err(ServiceError::HasReferences);
            }

            // Check for incoming references from other entities
            let id_str = control.id.to_string();
            let links_to = self.cache.get_links_to(&id_str);
            if !links_to.is_empty() {
                return Err(ServiceError::HasReferences);
            }
        }

        // Delete the file
        fs::remove_file(&path)?;

        Ok(())
    }

    /// Set the characteristic for a control
    pub fn set_characteristic(
        &self,
        id: &str,
        characteristic: Characteristic,
    ) -> ServiceResult<Control> {
        let (path, mut control) = self.find_control(id)?;

        control.characteristic = characteristic;
        control.entity_revision += 1;

        self.base.save(&control, &path, None)?;

        Ok(control)
    }

    /// Set the measurement method for a control
    pub fn set_measurement(&self, id: &str, measurement: Measurement) -> ServiceResult<Control> {
        let (path, mut control) = self.find_control(id)?;

        control.measurement = Some(measurement);
        control.entity_revision += 1;

        self.base.save(&control, &path, None)?;

        Ok(control)
    }

    /// Set the sampling plan for a control
    pub fn set_sampling(&self, id: &str, sampling: Sampling) -> ServiceResult<Control> {
        let (path, mut control) = self.find_control(id)?;

        control.sampling = Some(sampling);
        control.entity_revision += 1;

        self.base.save(&control, &path, None)?;

        Ok(control)
    }

    /// Set the control limits (for SPC)
    pub fn set_control_limits(&self, id: &str, limits: ControlLimits) -> ServiceResult<Control> {
        let (path, mut control) = self.find_control(id)?;

        control.control_limits = Some(limits);
        control.entity_revision += 1;

        self.base.save(&control, &path, None)?;

        Ok(control)
    }

    /// Set the reaction plan for a control
    pub fn set_reaction_plan(&self, id: &str, reaction_plan: &str) -> ServiceResult<Control> {
        let (path, mut control) = self.find_control(id)?;

        control.reaction_plan = Some(reaction_plan.to_string());
        control.entity_revision += 1;

        self.base.save(&control, &path, None)?;

        Ok(control)
    }

    /// Mark a control as critical (CTQ)
    pub fn set_critical(&self, id: &str, critical: bool) -> ServiceResult<Control> {
        let (path, mut control) = self.find_control(id)?;

        control.characteristic.critical = critical;
        control.entity_revision += 1;

        self.base.save(&control, &path, None)?;

        Ok(control)
    }

    /// Get statistics about controls
    pub fn stats(&self) -> ServiceResult<ControlStats> {
        let controls = self.load_all()?;

        let mut stats = ControlStats {
            total: controls.len(),
            ..Default::default()
        };

        for ctrl in &controls {
            // Count by type
            match ctrl.control_type {
                ControlType::Spc => stats.by_type.spc += 1,
                ControlType::Inspection => stats.by_type.inspection += 1,
                ControlType::PokaYoke => stats.by_type.poka_yoke += 1,
                ControlType::Visual => stats.by_type.visual += 1,
                ControlType::FunctionalTest => stats.by_type.functional_test += 1,
                ControlType::Attribute => stats.by_type.attribute += 1,
            }

            // Count by category
            match ctrl.control_category {
                ControlCategory::Variable => stats.by_category.variable += 1,
                ControlCategory::Attribute => stats.by_category.attribute += 1,
            }

            // Count by status
            match ctrl.status {
                Status::Draft => stats.by_status.draft += 1,
                Status::Review => stats.by_status.review += 1,
                Status::Approved => stats.by_status.approved += 1,
                Status::Released => stats.by_status.released += 1,
                Status::Obsolete => stats.by_status.obsolete += 1,
            }

            // Count features
            if ctrl.characteristic.critical {
                stats.critical_count += 1;
            }
            if ctrl.control_limits.is_some() {
                stats.with_limits += 1;
            }
            if ctrl.measurement.is_some() {
                stats.with_measurement += 1;
            }
            if ctrl.sampling.is_some() {
                stats.with_sampling += 1;
            }
            if ctrl.reaction_plan.is_some() {
                stats.with_reaction_plan += 1;
            }
        }

        Ok(stats)
    }

    // --- Private helper methods ---

    /// Find a control and its file path (cache-first lookup)
    fn find_control(&self, id: &str) -> ServiceResult<(PathBuf, Control)> {
        // Try to find in cache first for fast path lookup
        if let Some(cached) = self.cache.get_entity(id) {
            let path = if cached.file_path.is_absolute() {
                cached.file_path.clone()
            } else {
                self.project.root().join(&cached.file_path)
            };
            if path.exists() {
                if let Ok(control) = crate::yaml::parse_yaml_file::<Control>(&path) {
                    return Ok((path, control));
                }
            }
        }

        // Fall back to directory scan
        let dir = self.get_directory();
        if let Some((path, ctrl)) = loader::load_entity::<Control>(&dir, id)? {
            return Ok((path, ctrl));
        }
        Err(ServiceError::NotFound(id.to_string()))
    }

    /// Check if a control matches the given filter
    fn matches_filter(&self, ctrl: &Control, filter: &ControlFilter) -> bool {
        // Control type filter
        if let Some(control_type) = &filter.control_type {
            if ctrl.control_type != *control_type {
                return false;
            }
        }

        // Control category filter
        if let Some(control_category) = &filter.control_category {
            if ctrl.control_category != *control_category {
                return false;
            }
        }

        // Process filter
        if let Some(proc_id) = &filter.process {
            if !ctrl
                .links
                .process
                .as_ref()
                .is_some_and(|p| p.to_string().contains(proc_id))
            {
                return false;
            }
        }

        // Critical only filter
        if filter.critical_only && !ctrl.characteristic.critical {
            return false;
        }

        // Has limits filter
        if filter.has_limits && ctrl.control_limits.is_none() {
            return false;
        }

        // Has measurement filter
        if filter.has_measurement && ctrl.measurement.is_none() {
            return false;
        }

        // Has sampling filter
        if filter.has_sampling && ctrl.sampling.is_none() {
            return false;
        }

        // Common filters
        if !filter.common.matches_status(&ctrl.status) {
            return false;
        }
        if !filter.common.matches_author(&ctrl.author) {
            return false;
        }
        if !filter.common.matches_tags(&ctrl.tags) {
            return false;
        }
        if !filter.common.matches_search(&[&ctrl.title]) {
            return false;
        }
        if !filter.common.matches_recent(&ctrl.created) {
            return false;
        }

        true
    }

    /// Sort controls by the given field
    fn sort_controls(
        &self,
        controls: &mut [Control],
        sort_by: ControlSortField,
        sort_dir: SortDirection,
    ) {
        controls.sort_by(|a, b| {
            let cmp = match sort_by {
                ControlSortField::Id => a.id.to_string().cmp(&b.id.to_string()),
                ControlSortField::Title => a.title.cmp(&b.title),
                ControlSortField::ControlType => {
                    format!("{:?}", a.control_type).cmp(&format!("{:?}", b.control_type))
                }
                ControlSortField::Status => {
                    format!("{:?}", a.status).cmp(&format!("{:?}", b.status))
                }
                ControlSortField::Author => a.author.cmp(&b.author),
                ControlSortField::Created => a.created.cmp(&b.created),
            };

            match sort_dir {
                SortDirection::Ascending => cmp,
                SortDirection::Descending => cmp.reverse(),
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::control::SamplingType;
    use tempfile::TempDir;

    fn setup_test_project() -> (TempDir, Project, EntityCache) {
        let tmp = TempDir::new().unwrap();

        // Initialize project structure
        fs::create_dir_all(tmp.path().join(".tdt")).unwrap();
        fs::create_dir_all(tmp.path().join("manufacturing/controls")).unwrap();

        // Create config file
        fs::write(tmp.path().join(".tdt/config.yaml"), "author: Test Author\n").unwrap();

        let project = Project::discover_from(tmp.path()).unwrap();
        let cache = EntityCache::open(&project).unwrap();

        (tmp, project, cache)
    }

    #[test]
    fn test_create_control() {
        let (_tmp, project, cache) = setup_test_project();
        let service = ControlService::new(&project, &cache);

        let input = CreateControl {
            title: "Bore Diameter SPC".into(),
            author: "Test Author".into(),
            control_type: ControlType::Spc,
            ..Default::default()
        };

        let ctrl = service.create(input).unwrap();

        assert_eq!(ctrl.title, "Bore Diameter SPC");
        assert_eq!(ctrl.control_type, ControlType::Spc);
        assert_eq!(ctrl.status, Status::Draft);
    }

    #[test]
    fn test_get_control() {
        let (_tmp, project, cache) = setup_test_project();
        let service = ControlService::new(&project, &cache);

        let created = service
            .create(CreateControl {
                title: "Find Me".into(),
                author: "Test".into(),
                ..Default::default()
            })
            .unwrap();

        let found = service.get(&created.id.to_string()).unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().title, "Find Me");
    }

    #[test]
    fn test_update_control() {
        let (_tmp, project, cache) = setup_test_project();
        let service = ControlService::new(&project, &cache);

        let created = service
            .create(CreateControl {
                title: "Original".into(),
                author: "Test".into(),
                ..Default::default()
            })
            .unwrap();

        let updated = service
            .update(
                &created.id.to_string(),
                UpdateControl {
                    title: Some("Updated Title".into()),
                    control_type: Some(ControlType::Visual),
                    ..Default::default()
                },
            )
            .unwrap();

        assert_eq!(updated.title, "Updated Title");
        assert_eq!(updated.control_type, ControlType::Visual);
        assert_eq!(updated.entity_revision, 2);
    }

    #[test]
    fn test_delete_control() {
        let (_tmp, project, cache) = setup_test_project();
        let service = ControlService::new(&project, &cache);

        let created = service
            .create(CreateControl {
                title: "Delete Me".into(),
                author: "Test".into(),
                ..Default::default()
            })
            .unwrap();

        service.delete(&created.id.to_string(), false).unwrap();

        let found = service.get(&created.id.to_string()).unwrap();
        assert!(found.is_none());
    }

    #[test]
    fn test_set_characteristic() {
        let (_tmp, project, cache) = setup_test_project();
        let service = ControlService::new(&project, &cache);

        let created = service
            .create(CreateControl {
                title: "With Characteristic".into(),
                author: "Test".into(),
                ..Default::default()
            })
            .unwrap();

        let characteristic = Characteristic {
            name: "Bore Diameter".into(),
            nominal: Some(25.0),
            upper_limit: Some(25.05),
            lower_limit: Some(24.95),
            units: Some("mm".into()),
            critical: true,
        };

        let updated = service
            .set_characteristic(&created.id.to_string(), characteristic)
            .unwrap();

        assert_eq!(updated.characteristic.name, "Bore Diameter");
        assert_eq!(updated.characteristic.nominal, Some(25.0));
        assert!(updated.characteristic.critical);
    }

    #[test]
    fn test_set_measurement() {
        let (_tmp, project, cache) = setup_test_project();
        let service = ControlService::new(&project, &cache);

        let created = service
            .create(CreateControl {
                title: "With Measurement".into(),
                author: "Test".into(),
                ..Default::default()
            })
            .unwrap();

        let measurement = Measurement {
            method: Some("Digital caliper".into()),
            equipment: Some("Mitutoyo 500-196".into()),
            gage_rr_percent: Some(8.5),
        };

        let updated = service
            .set_measurement(&created.id.to_string(), measurement)
            .unwrap();

        assert!(updated.measurement.is_some());
        let meas = updated.measurement.unwrap();
        assert_eq!(meas.method, Some("Digital caliper".to_string()));
        assert_eq!(meas.gage_rr_percent, Some(8.5));
    }

    #[test]
    fn test_set_sampling() {
        let (_tmp, project, cache) = setup_test_project();
        let service = ControlService::new(&project, &cache);

        let created = service
            .create(CreateControl {
                title: "With Sampling".into(),
                author: "Test".into(),
                ..Default::default()
            })
            .unwrap();

        let sampling = Sampling {
            sampling_type: SamplingType::Periodic,
            frequency: Some("Every 2 hours".into()),
            sample_size: Some(5),
        };

        let updated = service
            .set_sampling(&created.id.to_string(), sampling)
            .unwrap();

        assert!(updated.sampling.is_some());
        let samp = updated.sampling.unwrap();
        assert_eq!(samp.sampling_type, SamplingType::Periodic);
        assert_eq!(samp.sample_size, Some(5));
    }

    #[test]
    fn test_set_control_limits() {
        let (_tmp, project, cache) = setup_test_project();
        let service = ControlService::new(&project, &cache);

        let created = service
            .create(CreateControl {
                title: "With Limits".into(),
                author: "Test".into(),
                control_type: ControlType::Spc,
                ..Default::default()
            })
            .unwrap();

        let limits = ControlLimits {
            ucl: Some(25.03),
            lcl: Some(24.97),
            target: Some(25.0),
        };

        let updated = service
            .set_control_limits(&created.id.to_string(), limits)
            .unwrap();

        assert!(updated.control_limits.is_some());
        let lim = updated.control_limits.unwrap();
        assert_eq!(lim.ucl, Some(25.03));
        assert_eq!(lim.lcl, Some(24.97));
        assert_eq!(lim.target, Some(25.0));
    }

    #[test]
    fn test_set_reaction_plan() {
        let (_tmp, project, cache) = setup_test_project();
        let service = ControlService::new(&project, &cache);

        let created = service
            .create(CreateControl {
                title: "With Reaction Plan".into(),
                author: "Test".into(),
                ..Default::default()
            })
            .unwrap();

        let updated = service
            .set_reaction_plan(
                &created.id.to_string(),
                "Stop production and notify supervisor",
            )
            .unwrap();

        assert_eq!(
            updated.reaction_plan,
            Some("Stop production and notify supervisor".to_string())
        );
    }

    #[test]
    fn test_set_critical() {
        let (_tmp, project, cache) = setup_test_project();
        let service = ControlService::new(&project, &cache);

        let created = service
            .create(CreateControl {
                title: "CTQ Test".into(),
                author: "Test".into(),
                ..Default::default()
            })
            .unwrap();

        assert!(!created.characteristic.critical);

        let updated = service.set_critical(&created.id.to_string(), true).unwrap();
        assert!(updated.characteristic.critical);

        let reverted = service
            .set_critical(&created.id.to_string(), false)
            .unwrap();
        assert!(!reverted.characteristic.critical);
    }

    #[test]
    fn test_list_with_filter() {
        let (_tmp, project, cache) = setup_test_project();
        let service = ControlService::new(&project, &cache);

        // Create SPC control
        service
            .create(CreateControl {
                title: "SPC Control".into(),
                author: "Test".into(),
                control_type: ControlType::Spc,
                ..Default::default()
            })
            .unwrap();

        // Create inspection control
        service
            .create(CreateControl {
                title: "Inspection Control".into(),
                author: "Test".into(),
                control_type: ControlType::Inspection,
                ..Default::default()
            })
            .unwrap();

        // List all
        let all = service
            .list(
                &ControlFilter::default(),
                ControlSortField::Created,
                SortDirection::Ascending,
            )
            .unwrap();
        assert_eq!(all.items.len(), 2);

        // List SPC only
        let spc = service
            .list(
                &ControlFilter::spc(),
                ControlSortField::Created,
                SortDirection::Ascending,
            )
            .unwrap();
        assert_eq!(spc.items.len(), 1);
        assert_eq!(spc.items[0].title, "SPC Control");
    }

    #[test]
    fn test_list_critical_only() {
        let (_tmp, project, cache) = setup_test_project();
        let service = ControlService::new(&project, &cache);

        // Create non-critical control
        service
            .create(CreateControl {
                title: "Normal Control".into(),
                author: "Test".into(),
                ..Default::default()
            })
            .unwrap();

        // Create critical control
        let critical = service
            .create(CreateControl {
                title: "Critical Control".into(),
                author: "Test".into(),
                characteristic: Some(Characteristic {
                    name: "Critical Dim".into(),
                    critical: true,
                    ..Default::default()
                }),
                ..Default::default()
            })
            .unwrap();

        // List critical only
        let critical_list = service
            .list(
                &ControlFilter::critical(),
                ControlSortField::Created,
                SortDirection::Ascending,
            )
            .unwrap();
        assert_eq!(critical_list.items.len(), 1);
        assert_eq!(critical_list.items[0].id, critical.id);
    }

    #[test]
    fn test_stats() {
        let (_tmp, project, cache) = setup_test_project();
        let service = ControlService::new(&project, &cache);

        // Create controls with different types
        let ctrl1 = service
            .create(CreateControl {
                title: "SPC 1".into(),
                author: "Test".into(),
                control_type: ControlType::Spc,
                characteristic: Some(Characteristic {
                    name: "Dim".into(),
                    critical: true,
                    ..Default::default()
                }),
                ..Default::default()
            })
            .unwrap();

        service
            .create(CreateControl {
                title: "Visual 1".into(),
                author: "Test".into(),
                control_type: ControlType::Visual,
                ..Default::default()
            })
            .unwrap();

        // Add limits to SPC control
        service
            .set_control_limits(
                &ctrl1.id.to_string(),
                ControlLimits {
                    ucl: Some(10.0),
                    lcl: Some(5.0),
                    target: Some(7.5),
                },
            )
            .unwrap();

        let stats = service.stats().unwrap();

        assert_eq!(stats.total, 2);
        assert_eq!(stats.by_type.spc, 1);
        assert_eq!(stats.by_type.visual, 1);
        assert_eq!(stats.critical_count, 1);
        assert_eq!(stats.with_limits, 1);
        assert_eq!(stats.by_status.draft, 2);
    }
}
