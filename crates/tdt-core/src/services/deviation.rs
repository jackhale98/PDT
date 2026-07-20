//! Deviation service for process deviation management
//!
//! Provides CRUD operations and approval workflow for deviations.

use crate::core::suspect::LinkRef;
use chrono::{NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::core::cache::{CachedDeviation, EntityCache};
use crate::core::entity::Status;
use crate::core::identity::{EntityId, EntityPrefix};
use crate::core::loader;
use crate::core::project::Project;
use crate::entities::dev::{
    AuthorizationLevel, Dev, DevApproval, DevLinks, DevRisk, DevStatus, DeviationCategory,
    DeviationType, RiskLevel,
};
use crate::services::base::ServiceBase;
use crate::services::common::{
    CommonFilter, NoneLast, ServiceError, ServiceResult, SortDirection, SortKey, Sortable,
};

/// Filter options for listing deviations
#[derive(Debug, Clone, Default)]
pub struct DeviationFilter {
    /// Common filters (status, author, tags, search)
    pub common: CommonFilter,
    /// Filter by deviation status
    pub dev_status: Option<DevStatus>,
    /// Filter by deviation type
    pub deviation_type: Option<DeviationType>,
    /// Filter by category
    pub category: Option<DeviationCategory>,
    /// Filter by risk level
    pub risk_level: Option<RiskLevel>,
    /// Show only active deviations (pending, approved, active)
    pub active_only: bool,
    /// Show recent deviations (last N days)
    pub recent_days: Option<u32>,
    /// Sort field
    pub sort: DeviationSortField,
    /// Sort direction
    pub sort_direction: SortDirection,
}

/// Fields available for sorting deviations
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviationSortField {
    Id,
    Title,
    DeviationNumber,
    DeviationType,
    Category,
    Risk,
    DevStatus,
    Author,
    #[default]
    Created,
}

impl Sortable for Dev {
    type SortField = DeviationSortField;

    fn sort_key(&self, field: &Self::SortField) -> SortKey {
        match field {
            DeviationSortField::Id => SortKey::String(self.id.to_string()),
            DeviationSortField::Title => SortKey::String(self.title.clone()),
            DeviationSortField::DeviationNumber => {
                SortKey::OptionalString(NoneLast(self.deviation_number.clone()))
            }
            DeviationSortField::DeviationType => SortKey::String(self.deviation_type.to_string()),
            DeviationSortField::Category => SortKey::String(self.category.to_string()),
            DeviationSortField::Risk => {
                // Sort high risk first
                let order = match self.risk.level {
                    RiskLevel::High => 0,
                    RiskLevel::Medium => 1,
                    RiskLevel::Low => 2,
                };
                SortKey::Number(order)
            }
            DeviationSortField::DevStatus => SortKey::String(self.dev_status.to_string()),
            DeviationSortField::Author => SortKey::String(self.author.clone()),
            DeviationSortField::Created => SortKey::DateTime(self.created.timestamp()),
        }
    }
}

/// Input for creating a new deviation
#[derive(Debug, Clone)]
pub struct CreateDeviation {
    /// Title
    pub title: String,
    /// User-defined deviation number
    pub deviation_number: Option<String>,
    /// Deviation type
    pub deviation_type: DeviationType,
    /// Category
    pub category: DeviationCategory,
    /// Description
    pub description: Option<String>,
    /// Risk level
    pub risk_level: RiskLevel,
    /// Risk assessment
    pub risk_assessment: Option<String>,
    /// Effective date
    pub effective_date: Option<NaiveDate>,
    /// Expiration date
    pub expiration_date: Option<NaiveDate>,
    /// Notes
    pub notes: Option<String>,
    /// Initial status
    pub status: Option<Status>,
    /// Author
    pub author: String,
}

/// Input for updating an existing deviation
#[derive(Debug, Clone, Default)]
pub struct UpdateDeviation {
    /// Update title
    pub title: Option<String>,
    /// Update deviation number
    pub deviation_number: Option<Option<String>>,
    /// Update deviation type
    pub deviation_type: Option<DeviationType>,
    /// Update category
    pub category: Option<DeviationCategory>,
    /// Update description
    pub description: Option<Option<String>>,
    /// Update effective date
    pub effective_date: Option<Option<NaiveDate>>,
    /// Update expiration date
    pub expiration_date: Option<Option<NaiveDate>>,
    /// Update notes
    pub notes: Option<Option<String>>,
    /// Update document status
    pub status: Option<Status>,
    /// Update dev status
    pub dev_status: Option<DevStatus>,
}

/// Statistics about deviations
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DeviationStats {
    /// Total number of deviations
    pub total: usize,
    /// Counts by dev status
    pub by_dev_status: DevStatusCounts,
    /// Counts by type
    pub by_type: DeviationTypeCounts,
    /// Counts by category
    pub by_category: DeviationCategoryCounts,
    /// Counts by risk level
    pub by_risk: RiskLevelCounts,
    /// Number of active deviations
    pub active: usize,
}

/// Dev status counts
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DevStatusCounts {
    pub pending: usize,
    pub approved: usize,
    pub active: usize,
    pub expired: usize,
    pub closed: usize,
    pub rejected: usize,
}

/// Deviation type counts
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DeviationTypeCounts {
    pub temporary: usize,
    pub permanent: usize,
    pub emergency: usize,
}

/// Category counts
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DeviationCategoryCounts {
    pub material: usize,
    pub process: usize,
    pub equipment: usize,
    pub tooling: usize,
    pub specification: usize,
    pub documentation: usize,
}

/// Risk level counts
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RiskLevelCounts {
    pub low: usize,
    pub medium: usize,
    pub high: usize,
}

/// Service for managing deviations
pub struct DeviationService<'a> {
    project: &'a Project,
    base: ServiceBase<'a>,
    cache: &'a EntityCache,
    workflow_guard: Option<crate::services::workflow_guard::WorkflowGuard>,
}

impl<'a> DeviationService<'a> {
    /// Create a new DeviationService
    pub fn new(project: &'a Project, cache: &'a EntityCache) -> Self {
        Self {
            project,
            base: ServiceBase::new(project, cache),
            cache,
            workflow_guard: None,
        }
    }

    /// Attach a workflow guard for authorization enforcement
    pub fn with_workflow(mut self, guard: crate::services::workflow_guard::WorkflowGuard) -> Self {
        self.workflow_guard = Some(guard);
        self
    }

    /// Get a reference to the project
    fn project(&self) -> &Project {
        self.project
    }

    /// Get the deviations directory
    fn dev_dir(&self) -> PathBuf {
        self.project().root().join("manufacturing/deviations")
    }

    /// Get the file path for a deviation
    fn get_file_path(&self, id: &EntityId) -> PathBuf {
        self.dev_dir().join(format!("{}.tdt.yaml", id))
    }

    /// Load all deviations
    fn load_all(&self) -> ServiceResult<Vec<Dev>> {
        let dir = self.dev_dir();
        if !dir.exists() {
            return Ok(Vec::new());
        }
        loader::load_all(&dir).map_err(ServiceError::from)
    }

    /// List deviations using the cache (fast path)
    ///
    /// Returns cached deviation data without loading full entities from disk.
    /// Use this for list views and simple queries where you don't need
    /// full entity data like description or mitigations.
    pub fn list_cached(&self, filter: &DeviationFilter) -> ServiceResult<Vec<CachedDeviation>> {
        // Convert filter values to strings for cache query
        let status = filter
            .common
            .status
            .as_ref()
            .and_then(|s| s.first())
            .map(|s| s.to_string());

        let dev_status = filter.dev_status.map(|s| s.to_string());
        let deviation_type = filter.deviation_type.map(|t| t.to_string());
        let category = filter.category.map(|c| c.to_string());
        let risk_level = filter.risk_level.map(|r| r.to_string());

        // Use deviation-specific cache query
        let mut cached = self.cache.list_deviations(
            status.as_deref(),
            dev_status.as_deref(),
            deviation_type.as_deref(),
            category.as_deref(),
            risk_level.as_deref(),
            filter.common.author.as_deref(),
            filter.common.search.as_deref(),
            None, // Apply limit after additional filters
        );

        // Apply active_only filter
        if filter.active_only {
            cached.retain(|d| {
                matches!(
                    d.dev_status.as_deref(),
                    Some("pending") | Some("approved") | Some("active")
                )
            });
        }

        // Apply recent_days filter
        if let Some(days) = filter.recent_days {
            let cutoff = Utc::now() - chrono::Duration::days(days as i64);
            cached.retain(|e| e.created >= cutoff);
        }

        // Apply limit
        if let Some(limit) = filter.common.limit {
            cached.truncate(limit);
        }

        Ok(cached)
    }

    /// Find a deviation by ID
    ///
    /// Uses the cache to find the file path for faster lookup.
    fn find_deviation(&self, id: &str) -> ServiceResult<(PathBuf, Dev)> {
        // Try to find in cache first for fast path lookup
        if let Some(cached) = self.cache.get_entity(id) {
            let path = if cached.file_path.is_absolute() {
                cached.file_path.clone()
            } else {
                self.project.root().join(&cached.file_path)
            };
            if path.exists() {
                if let Ok(dev) = crate::yaml::parse_yaml_file::<Dev>(&path) {
                    return Ok((path, dev));
                }
            }
        }

        // Fall back to directory scan
        let dir = self.dev_dir();
        if let Some((path, dev)) = loader::load_entity::<Dev>(&dir, id)? {
            return Ok((path, dev));
        }
        Err(ServiceError::NotFound(format!("Deviation: {}", id)))
    }

    /// List deviations with filtering and sorting
    pub fn list(&self, filter: &DeviationFilter) -> ServiceResult<Vec<Dev>> {
        let mut deviations = self.load_all()?;

        // Apply filters
        deviations.retain(|d| self.matches_filter(d, filter));

        // Sort
        crate::services::common::sort_entities(&mut deviations, filter.sort, filter.sort_direction);

        // Apply limit from common filter
        if let Some(limit) = filter.common.limit {
            deviations.truncate(limit);
        }

        Ok(deviations)
    }

    /// Check if a deviation matches the filter
    fn matches_filter(&self, dev: &Dev, filter: &DeviationFilter) -> bool {
        // Common filters
        if !filter.common.matches_entity(dev) {
            return false;
        }

        // Dev status filter
        if let Some(ref status) = filter.dev_status {
            if &dev.dev_status != status {
                return false;
            }
        }

        // Type filter
        if let Some(ref dt) = filter.deviation_type {
            if &dev.deviation_type != dt {
                return false;
            }
        }

        // Category filter
        if let Some(ref cat) = filter.category {
            if &dev.category != cat {
                return false;
            }
        }

        // Risk level filter
        if let Some(ref level) = filter.risk_level {
            if &dev.risk.level != level {
                return false;
            }
        }

        // Active only filter
        if filter.active_only
            && !matches!(
                dev.dev_status,
                DevStatus::Pending | DevStatus::Approved | DevStatus::Active
            )
        {
            return false;
        }

        // Recent days filter
        if let Some(days) = filter.recent_days {
            let cutoff = Utc::now() - chrono::Duration::days(days as i64);
            if dev.created < cutoff {
                return false;
            }
        }

        true
    }

    /// Get a deviation by ID
    pub fn get(&self, id: &str) -> ServiceResult<Option<Dev>> {
        let dir = self.dev_dir();
        if let Some((_, dev)) = loader::load_entity::<Dev>(&dir, id)? {
            return Ok(Some(dev));
        }
        Ok(None)
    }

    /// Get a deviation by ID, returning an error if not found
    pub fn get_required(&self, id: &str) -> ServiceResult<Dev> {
        self.get(id)?
            .ok_or_else(|| ServiceError::NotFound(format!("Deviation: {}", id)))
    }

    /// Create a new deviation
    pub fn create(&self, input: CreateDeviation) -> ServiceResult<Dev> {
        let now = Utc::now();
        let id = EntityId::new(EntityPrefix::Dev);

        let dev = Dev {
            id: id.clone(),
            title: input.title,
            deviation_number: input.deviation_number,
            deviation_type: input.deviation_type,
            category: input.category,
            description: input.description,
            risk: DevRisk {
                level: input.risk_level,
                assessment: input.risk_assessment,
                mitigations: Vec::new(),
            },
            approval: DevApproval::default(),
            effective_date: input.effective_date,
            expiration_date: input.expiration_date,
            dev_status: DevStatus::Pending,
            notes: input.notes,
            links: DevLinks::default(),
            status: input.status.unwrap_or(Status::Draft),
            created: now,
            author: input.author,
            entity_revision: 1,
        };

        // Ensure directory exists
        let dir = self.dev_dir();
        fs::create_dir_all(&dir)?;

        // Save
        let file_path = self.get_file_path(&id);
        self.base.save(&dev, &file_path, Some("DEV"))?;

        Ok(dev)
    }

    /// Update an existing deviation
    pub fn update(&self, id: &str, input: UpdateDeviation) -> ServiceResult<Dev> {
        let (_, mut dev) = self.find_deviation(id)?;

        // Apply updates
        if let Some(title) = input.title {
            dev.title = title;
        }
        if let Some(deviation_number) = input.deviation_number {
            dev.deviation_number = deviation_number;
        }
        if let Some(deviation_type) = input.deviation_type {
            dev.deviation_type = deviation_type;
        }
        if let Some(category) = input.category {
            dev.category = category;
        }
        if let Some(description) = input.description {
            dev.description = description;
        }
        if let Some(effective_date) = input.effective_date {
            dev.effective_date = effective_date;
        }
        if let Some(expiration_date) = input.expiration_date {
            dev.expiration_date = expiration_date;
        }
        if let Some(notes) = input.notes {
            dev.notes = notes;
        }
        if let Some(status) = input.status {
            dev.status = status;
        }
        if let Some(dev_status) = input.dev_status {
            dev.dev_status = dev_status;
        }

        dev.entity_revision += 1;

        // Save
        let file_path = self.get_file_path(&dev.id);
        self.base.save(&dev, &file_path, None)?;

        Ok(dev)
    }

    /// Delete a deviation
    pub fn delete(&self, id: &str, force: bool) -> ServiceResult<()> {
        let (path, _dev) = self.find_deviation(id)?;

        // Check for references unless force is true
        if !force {
            let links_to = self.cache.get_links_to(id);
            if !links_to.is_empty() {
                return Err(ServiceError::HasReferences);
            }
        }

        // Delete the file
        fs::remove_file(&path)?;

        Ok(())
    }

    /// Approve a deviation
    ///
    /// When a `WorkflowGuard` is attached and enabled, the current user must be
    /// authorized to approve DEV entities. The guard's resolved identity overrides
    /// the `approved_by` parameter.
    pub fn approve(
        &self,
        id: &str,
        mut approved_by: String,
        authorization_level: AuthorizationLevel,
        activate: bool,
    ) -> ServiceResult<Dev> {
        // Authorization check via workflow guard (if present)
        if let Some(ref guard) = self.workflow_guard {
            if let Some(user) = guard.check_approval_auth(EntityPrefix::Dev)? {
                approved_by = user.name.clone();
            }
        }

        let (_, mut dev) = self.find_deviation(id)?;

        // Validate current status
        if dev.dev_status != DevStatus::Pending {
            return Err(ServiceError::ValidationFailed(format!(
                "Cannot approve deviation in '{}' status, must be 'pending'",
                dev.dev_status
            )));
        }

        // Update approval info
        dev.approval.approved_by = Some(approved_by);
        dev.approval.approval_date = Some(Utc::now().date_naive());
        dev.approval.authorization_level = Some(authorization_level);

        // Set status
        dev.dev_status = if activate {
            DevStatus::Active
        } else {
            DevStatus::Approved
        };

        dev.entity_revision += 1;

        // Save
        let file_path = self.get_file_path(&dev.id);
        self.base.save(&dev, &file_path, None)?;

        Ok(dev)
    }

    /// Reject a deviation
    pub fn reject(&self, id: &str, reason: Option<String>) -> ServiceResult<Dev> {
        let (_, mut dev) = self.find_deviation(id)?;

        // Validate current status
        if dev.dev_status != DevStatus::Pending {
            return Err(ServiceError::ValidationFailed(format!(
                "Cannot reject deviation in '{}' status, must be 'pending'",
                dev.dev_status
            )));
        }

        // Update status
        dev.dev_status = DevStatus::Rejected;

        // Add reason to notes
        if let Some(reason) = reason {
            let note = format!("\n\n## Rejection Reason\n{}", reason);
            dev.notes = Some(dev.notes.unwrap_or_default() + &note);
        }

        dev.entity_revision += 1;

        // Save
        let file_path = self.get_file_path(&dev.id);
        self.base.save(&dev, &file_path, None)?;

        Ok(dev)
    }

    /// Activate an approved deviation
    pub fn activate(&self, id: &str) -> ServiceResult<Dev> {
        let (_, mut dev) = self.find_deviation(id)?;

        // Validate current status
        if dev.dev_status != DevStatus::Approved {
            return Err(ServiceError::ValidationFailed(format!(
                "Cannot activate deviation in '{}' status, must be 'approved'",
                dev.dev_status
            )));
        }

        dev.dev_status = DevStatus::Active;
        dev.entity_revision += 1;

        // Save
        let file_path = self.get_file_path(&dev.id);
        self.base.save(&dev, &file_path, None)?;

        Ok(dev)
    }

    /// Close/expire a deviation
    pub fn close(&self, id: &str, reason: Option<String>) -> ServiceResult<Dev> {
        let (_, mut dev) = self.find_deviation(id)?;

        // Update status
        dev.dev_status = DevStatus::Closed;

        // Add reason to notes
        if let Some(reason) = reason {
            let note = format!("\n\n## Closure Reason\n{}", reason);
            dev.notes = Some(dev.notes.unwrap_or_default() + &note);
        }

        dev.entity_revision += 1;

        // Save
        let file_path = self.get_file_path(&dev.id);
        self.base.save(&dev, &file_path, None)?;

        Ok(dev)
    }

    /// Set a deviation to expired status
    pub fn expire(&self, id: &str) -> ServiceResult<Dev> {
        let (_, mut dev) = self.find_deviation(id)?;

        dev.dev_status = DevStatus::Expired;
        dev.entity_revision += 1;

        // Save
        let file_path = self.get_file_path(&dev.id);
        self.base.save(&dev, &file_path, None)?;

        Ok(dev)
    }

    /// Set risk assessment
    pub fn set_risk(
        &self,
        id: &str,
        level: RiskLevel,
        assessment: Option<String>,
    ) -> ServiceResult<Dev> {
        let (_, mut dev) = self.find_deviation(id)?;

        dev.risk.level = level;
        dev.risk.assessment = assessment;
        dev.entity_revision += 1;

        // Save
        let file_path = self.get_file_path(&dev.id);
        self.base.save(&dev, &file_path, None)?;

        Ok(dev)
    }

    /// Add a mitigation measure
    pub fn add_mitigation(&self, id: &str, mitigation: String) -> ServiceResult<Dev> {
        let (_, mut dev) = self.find_deviation(id)?;

        // Check if mitigation already exists
        if dev.risk.mitigations.contains(&mitigation) {
            return Err(ServiceError::ValidationFailed(
                "Mitigation already exists".to_string(),
            ));
        }

        dev.risk.mitigations.push(mitigation);
        dev.entity_revision += 1;

        // Save
        let file_path = self.get_file_path(&dev.id);
        self.base.save(&dev, &file_path, None)?;

        Ok(dev)
    }

    /// Remove a mitigation measure
    pub fn remove_mitigation(&self, id: &str, mitigation: &str) -> ServiceResult<Dev> {
        let (_, mut dev) = self.find_deviation(id)?;

        let original_len = dev.risk.mitigations.len();
        dev.risk.mitigations.retain(|m| m != mitigation);

        if dev.risk.mitigations.len() == original_len {
            return Err(ServiceError::ValidationFailed(
                "Mitigation not found".to_string(),
            ));
        }

        dev.entity_revision += 1;

        // Save
        let file_path = self.get_file_path(&dev.id);
        self.base.save(&dev, &file_path, None)?;

        Ok(dev)
    }

    /// Add a process link
    pub fn add_process_link(&self, id: &str, process_id: String) -> ServiceResult<Dev> {
        let (_, mut dev) = self.find_deviation(id)?;

        if !dev.links.processes.iter().any(|l| *l == process_id) {
            dev.links.processes.push(LinkRef::from(process_id));
            dev.entity_revision += 1;

            let file_path = self.get_file_path(&dev.id);
            self.base.save(&dev, &file_path, None)?;
        }

        Ok(dev)
    }

    /// Add a lot link
    pub fn add_lot_link(&self, id: &str, lot_id: String) -> ServiceResult<Dev> {
        let (_, mut dev) = self.find_deviation(id)?;

        if !dev.links.lots.iter().any(|l| *l == lot_id) {
            dev.links.lots.push(LinkRef::from(lot_id));
            dev.entity_revision += 1;

            let file_path = self.get_file_path(&dev.id);
            self.base.save(&dev, &file_path, None)?;
        }

        Ok(dev)
    }

    /// Add a component link
    pub fn add_component_link(&self, id: &str, component_id: String) -> ServiceResult<Dev> {
        let (_, mut dev) = self.find_deviation(id)?;

        if !dev.links.components.iter().any(|l| *l == component_id) {
            dev.links.components.push(LinkRef::from(component_id));
            dev.entity_revision += 1;

            let file_path = self.get_file_path(&dev.id);
            self.base.save(&dev, &file_path, None)?;
        }

        Ok(dev)
    }

    /// Calculate statistics
    pub fn stats(&self) -> ServiceResult<DeviationStats> {
        let deviations = self.load_all()?;

        let mut stats = DeviationStats {
            total: deviations.len(),
            ..Default::default()
        };

        for dev in &deviations {
            // Count by dev status
            match dev.dev_status {
                DevStatus::Pending => stats.by_dev_status.pending += 1,
                DevStatus::Approved => stats.by_dev_status.approved += 1,
                DevStatus::Active => stats.by_dev_status.active += 1,
                DevStatus::Expired => stats.by_dev_status.expired += 1,
                DevStatus::Closed => stats.by_dev_status.closed += 1,
                DevStatus::Rejected => stats.by_dev_status.rejected += 1,
            }

            // Count by type
            match dev.deviation_type {
                DeviationType::Temporary => stats.by_type.temporary += 1,
                DeviationType::Permanent => stats.by_type.permanent += 1,
                DeviationType::Emergency => stats.by_type.emergency += 1,
            }

            // Count by category
            match dev.category {
                DeviationCategory::Material => stats.by_category.material += 1,
                DeviationCategory::Process => stats.by_category.process += 1,
                DeviationCategory::Equipment => stats.by_category.equipment += 1,
                DeviationCategory::Tooling => stats.by_category.tooling += 1,
                DeviationCategory::Specification => stats.by_category.specification += 1,
                DeviationCategory::Documentation => stats.by_category.documentation += 1,
            }

            // Count by risk level
            match dev.risk.level {
                RiskLevel::Low => stats.by_risk.low += 1,
                RiskLevel::Medium => stats.by_risk.medium += 1,
                RiskLevel::High => stats.by_risk.high += 1,
            }

            // Count active
            if matches!(
                dev.dev_status,
                DevStatus::Pending | DevStatus::Approved | DevStatus::Active
            ) {
                stats.active += 1;
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

    fn create_test_deviation(service: &DeviationService) -> Dev {
        service
            .create(CreateDeviation {
                title: "Test Deviation".to_string(),
                deviation_number: Some("DEV-2024-001".to_string()),
                deviation_type: DeviationType::Temporary,
                category: DeviationCategory::Material,
                description: Some("Test description".to_string()),
                risk_level: RiskLevel::Low,
                risk_assessment: None,
                effective_date: None,
                expiration_date: None,
                notes: None,
                status: None,
                author: "author".to_string(),
            })
            .unwrap()
    }

    #[test]
    fn test_create_deviation() {
        let (_tmp, project, cache) = setup();
        let service = DeviationService::new(&project, &cache);

        let dev = service
            .create(CreateDeviation {
                title: "Material Substitution".to_string(),
                deviation_number: Some("DEV-2024-042".to_string()),
                deviation_type: DeviationType::Temporary,
                category: DeviationCategory::Material,
                description: Some("Substituting 316L for 304".to_string()),
                risk_level: RiskLevel::Low,
                risk_assessment: Some("Low risk change".to_string()),
                effective_date: None,
                expiration_date: None,
                notes: None,
                status: None,
                author: "author".to_string(),
            })
            .unwrap();

        assert!(dev.id.to_string().starts_with("DEV-"));
        assert_eq!(dev.title, "Material Substitution");
        assert_eq!(dev.deviation_type, DeviationType::Temporary);
        assert_eq!(dev.dev_status, DevStatus::Pending);
    }

    #[test]
    fn test_get_deviation() {
        let (_tmp, project, cache) = setup();
        let service = DeviationService::new(&project, &cache);

        let created = create_test_deviation(&service);
        let retrieved = service.get(&created.id.to_string()).unwrap().unwrap();

        assert_eq!(created.id, retrieved.id);
        assert_eq!(created.title, retrieved.title);
    }

    #[test]
    fn test_update_deviation() {
        let (_tmp, project, cache) = setup();
        let service = DeviationService::new(&project, &cache);

        let created = create_test_deviation(&service);
        let updated = service
            .update(
                &created.id.to_string(),
                UpdateDeviation {
                    title: Some("Updated Deviation".to_string()),
                    category: Some(DeviationCategory::Process),
                    ..Default::default()
                },
            )
            .unwrap();

        assert_eq!(updated.title, "Updated Deviation");
        assert_eq!(updated.category, DeviationCategory::Process);
        assert_eq!(updated.entity_revision, 2);
    }

    #[test]
    fn test_delete_deviation() {
        let (_tmp, project, cache) = setup();
        let service = DeviationService::new(&project, &cache);

        let created = create_test_deviation(&service);
        service.delete(&created.id.to_string(), false).unwrap();

        let result = service.get(&created.id.to_string()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_approve_deviation() {
        let (_tmp, project, cache) = setup();
        let service = DeviationService::new(&project, &cache);

        let created = create_test_deviation(&service);
        let approved = service
            .approve(
                &created.id.to_string(),
                "approver".to_string(),
                AuthorizationLevel::Engineering,
                false,
            )
            .unwrap();

        assert_eq!(approved.dev_status, DevStatus::Approved);
        assert_eq!(approved.approval.approved_by, Some("approver".to_string()));
        assert!(approved.approval.approval_date.is_some());
    }

    #[test]
    fn test_approve_and_activate() {
        let (_tmp, project, cache) = setup();
        let service = DeviationService::new(&project, &cache);

        let created = create_test_deviation(&service);
        let activated = service
            .approve(
                &created.id.to_string(),
                "approver".to_string(),
                AuthorizationLevel::Quality,
                true,
            )
            .unwrap();

        assert_eq!(activated.dev_status, DevStatus::Active);
    }

    #[test]
    fn test_reject_deviation() {
        let (_tmp, project, cache) = setup();
        let service = DeviationService::new(&project, &cache);

        let created = create_test_deviation(&service);
        let rejected = service
            .reject(&created.id.to_string(), Some("Not approved".to_string()))
            .unwrap();

        assert_eq!(rejected.dev_status, DevStatus::Rejected);
        assert!(rejected.notes.unwrap().contains("Not approved"));
    }

    #[test]
    fn test_close_deviation() {
        let (_tmp, project, cache) = setup();
        let service = DeviationService::new(&project, &cache);

        let created = create_test_deviation(&service);
        let closed = service
            .close(
                &created.id.to_string(),
                Some("No longer needed".to_string()),
            )
            .unwrap();

        assert_eq!(closed.dev_status, DevStatus::Closed);
    }

    #[test]
    fn test_add_mitigation() {
        let (_tmp, project, cache) = setup();
        let service = DeviationService::new(&project, &cache);

        let created = create_test_deviation(&service);
        let updated = service
            .add_mitigation(
                &created.id.to_string(),
                "First article inspection".to_string(),
            )
            .unwrap();

        assert_eq!(updated.risk.mitigations.len(), 1);
        assert_eq!(updated.risk.mitigations[0], "First article inspection");
    }

    #[test]
    fn test_set_risk() {
        let (_tmp, project, cache) = setup();
        let service = DeviationService::new(&project, &cache);

        let created = create_test_deviation(&service);
        let updated = service
            .set_risk(
                &created.id.to_string(),
                RiskLevel::Medium,
                Some("Moderate risk".to_string()),
            )
            .unwrap();

        assert_eq!(updated.risk.level, RiskLevel::Medium);
        assert_eq!(updated.risk.assessment, Some("Moderate risk".to_string()));
    }

    #[test]
    fn test_list_with_filter() {
        let (_tmp, project, cache) = setup();
        let service = DeviationService::new(&project, &cache);

        // Create deviations with different statuses
        let dev1 = create_test_deviation(&service);
        service
            .approve(
                &dev1.id.to_string(),
                "approver".to_string(),
                AuthorizationLevel::Engineering,
                true,
            )
            .unwrap();
        create_test_deviation(&service); // This one stays pending

        // Filter for active status
        let active_devs = service
            .list(&DeviationFilter {
                dev_status: Some(DevStatus::Active),
                ..Default::default()
            })
            .unwrap();

        assert_eq!(active_devs.len(), 1);
        assert_eq!(active_devs[0].dev_status, DevStatus::Active);
    }

    #[test]
    fn test_stats() {
        let (_tmp, project, cache) = setup();
        let service = DeviationService::new(&project, &cache);

        // Create some deviations
        create_test_deviation(&service);
        let dev2 = create_test_deviation(&service);
        service
            .approve(
                &dev2.id.to_string(),
                "approver".to_string(),
                AuthorizationLevel::Engineering,
                false,
            )
            .unwrap();

        let stats = service.stats().unwrap();

        assert_eq!(stats.total, 2);
        assert_eq!(stats.by_dev_status.pending, 1);
        assert_eq!(stats.by_dev_status.approved, 1);
        assert_eq!(stats.by_type.temporary, 2);
        assert_eq!(stats.active, 2); // pending and approved are both "active"
    }
}
