//! NCR service for non-conformance report management
//!
//! Provides CRUD operations and workflow management for NCRs.

use chrono::{NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::core::cache::{CachedNcr, EntityCache};
use crate::core::entity::Status;
use crate::core::identity::{EntityId, EntityPrefix};
use crate::core::loader;
use crate::core::project::Project;
use crate::core::suspect::LinkRef;
use crate::entities::ncr::{
    AffectedItems, ContainmentAction, ContainmentStatus, CostImpact, Defect, Detection,
    DetectionStage, Disposition, DispositionDecision, Ncr, NcrCategory, NcrLinks, NcrSeverity,
    NcrStatus, NcrType,
};
use crate::services::base::ServiceBase;
use crate::services::common::{
    CommonFilter, ServiceError, ServiceResult, SortDirection, SortKey, Sortable,
};

/// Filter options for listing NCRs
#[derive(Debug, Clone, Default)]
pub struct NcrFilter {
    /// Common filters (status, author, tags, search)
    pub common: CommonFilter,
    /// Filter by NCR type
    pub ncr_type: Option<NcrType>,
    /// Filter by severity
    pub severity: Option<NcrSeverity>,
    /// Filter by NCR status (workflow)
    pub ncr_status: Option<NcrStatus>,
    /// Filter by category
    pub category: Option<NcrCategory>,
    /// Show only open NCRs (not closed)
    pub open_only: bool,
    /// Show recent NCRs (last N days)
    pub recent_days: Option<u32>,
    /// Sort field
    pub sort: NcrSortField,
    /// Sort direction
    pub sort_direction: SortDirection,
}

/// Fields available for sorting NCRs
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NcrSortField {
    Id,
    Title,
    NcrType,
    Severity,
    NcrStatus,
    Category,
    Author,
    #[default]
    Created,
}

impl Sortable for Ncr {
    type SortField = NcrSortField;

    fn sort_key(&self, field: &Self::SortField) -> SortKey {
        match field {
            NcrSortField::Id => SortKey::String(self.id.to_string()),
            NcrSortField::Title => SortKey::String(self.title.clone()),
            NcrSortField::NcrType => SortKey::String(self.ncr_type.to_string()),
            NcrSortField::Severity => {
                // Sort critical first
                let order = match self.severity {
                    NcrSeverity::Critical => 0,
                    NcrSeverity::Major => 1,
                    NcrSeverity::Minor => 2,
                };
                SortKey::Number(order)
            }
            NcrSortField::NcrStatus => SortKey::String(self.ncr_status.to_string()),
            NcrSortField::Category => SortKey::String(self.category.to_string()),
            NcrSortField::Author => SortKey::String(self.author.clone()),
            NcrSortField::Created => SortKey::DateTime(self.created.timestamp()),
        }
    }
}

/// Input for creating a new NCR
#[derive(Debug, Clone)]
pub struct CreateNcr {
    /// Title
    pub title: String,
    /// NCR number
    pub ncr_number: Option<String>,
    /// NCR type
    pub ncr_type: NcrType,
    /// Severity
    pub severity: NcrSeverity,
    /// Category
    pub category: NcrCategory,
    /// Description
    pub description: Option<String>,
    /// Report date
    pub report_date: Option<NaiveDate>,
    /// Tags
    pub tags: Vec<String>,
    /// Initial status
    pub status: Option<Status>,
    /// Author
    pub author: String,
    /// Lot IDs to link
    pub lot_ids: Vec<String>,
}

/// Input for updating an existing NCR
#[derive(Debug, Clone, Default)]
pub struct UpdateNcr {
    /// Update title
    pub title: Option<String>,
    /// Update NCR number
    pub ncr_number: Option<Option<String>>,
    /// Update NCR type
    pub ncr_type: Option<NcrType>,
    /// Update severity
    pub severity: Option<NcrSeverity>,
    /// Update category
    pub category: Option<NcrCategory>,
    /// Update description
    pub description: Option<Option<String>>,
    /// Update tags
    pub tags: Option<Vec<String>>,
    /// Update document status
    pub status: Option<Status>,
    /// Update NCR status (workflow)
    pub ncr_status: Option<NcrStatus>,
}

/// Statistics about NCRs
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NcrStats {
    /// Total number of NCRs
    pub total: usize,
    /// Counts by NCR status
    pub by_ncr_status: NcrStatusCounts,
    /// Counts by type
    pub by_type: NcrTypeCounts,
    /// Counts by severity
    pub by_severity: NcrSeverityCounts,
    /// Counts by category
    pub by_category: NcrCategoryCounts,
    /// Number of open NCRs
    pub open: usize,
    /// Total rework cost
    pub total_rework_cost: f64,
    /// Total scrap cost
    pub total_scrap_cost: f64,
}

/// NCR status counts
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NcrStatusCounts {
    pub open: usize,
    pub containment: usize,
    pub investigation: usize,
    pub disposition: usize,
    pub closed: usize,
}

/// NCR type counts
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NcrTypeCounts {
    pub internal: usize,
    pub supplier: usize,
    pub customer: usize,
}

/// NCR severity counts
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NcrSeverityCounts {
    pub minor: usize,
    pub major: usize,
    pub critical: usize,
}

/// NCR category counts
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NcrCategoryCounts {
    pub dimensional: usize,
    pub cosmetic: usize,
    pub material: usize,
    pub functional: usize,
    pub documentation: usize,
    pub process: usize,
    pub packaging: usize,
}

/// Service for managing NCRs
pub struct NcrService<'a> {
    project: &'a Project,
    base: ServiceBase<'a>,
    cache: &'a EntityCache,
}

impl<'a> NcrService<'a> {
    /// Create a new NcrService
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

    /// Get the NCRs directory
    fn ncr_dir(&self) -> PathBuf {
        self.project().root().join("manufacturing/ncrs")
    }

    /// Get the file path for an NCR
    fn get_file_path(&self, id: &EntityId) -> PathBuf {
        self.ncr_dir().join(format!("{}.tdt.yaml", id))
    }

    /// Load all NCRs
    fn load_all(&self) -> ServiceResult<Vec<Ncr>> {
        let dir = self.ncr_dir();
        if !dir.exists() {
            return Ok(Vec::new());
        }
        loader::load_all(&dir).map_err(ServiceError::from)
    }

    /// List NCRs using the cache (fast path)
    ///
    /// Returns cached NCR data without loading full entities from disk.
    /// Use this for list views and simple queries.
    pub fn list_cached(&self, filter: &NcrFilter) -> ServiceResult<Vec<CachedNcr>> {
        // Convert filter values to strings for cache query
        let status_str = filter
            .common
            .status
            .as_ref()
            .and_then(|s| s.first())
            .map(|s| format!("{:?}", s).to_lowercase());

        let ncr_type_str = filter.ncr_type.as_ref().map(|t| match t {
            NcrType::Internal => "internal",
            NcrType::Supplier => "supplier",
            NcrType::Customer => "customer",
        });

        let severity_str = filter.severity.as_ref().map(|s| match s {
            NcrSeverity::Minor => "minor",
            NcrSeverity::Major => "major",
            NcrSeverity::Critical => "critical",
        });

        let ncr_status_str = filter.ncr_status.as_ref().map(|s| match s {
            NcrStatus::Open => "open",
            NcrStatus::Containment => "containment",
            NcrStatus::Investigation => "investigation",
            NcrStatus::Disposition => "disposition",
            NcrStatus::Closed => "closed",
        });

        let category_str = filter.category.as_ref().map(|c| match c {
            NcrCategory::Dimensional => "dimensional",
            NcrCategory::Cosmetic => "cosmetic",
            NcrCategory::Material => "material",
            NcrCategory::Functional => "functional",
            NcrCategory::Documentation => "documentation",
            NcrCategory::Process => "process",
            NcrCategory::Packaging => "packaging",
        });

        // Query cache
        let mut cached = self.cache.list_ncrs(
            status_str.as_deref(),
            ncr_type_str,
            severity_str,
            ncr_status_str,
            category_str,
            filter.common.author.as_deref(),
            None, // Apply limit after all filters
        );

        // Apply additional filters not supported by cache query
        if let Some(days) = filter.recent_days {
            let cutoff = Utc::now() - chrono::Duration::days(days as i64);
            cached.retain(|n| n.created >= cutoff);
        }

        if filter.open_only {
            cached.retain(|n| n.ncr_status.as_ref().map(|s| s != "closed").unwrap_or(true));
        }

        // Apply limit
        if let Some(limit) = filter.common.limit {
            cached.truncate(limit);
        }

        Ok(cached)
    }

    /// Find an NCR by ID
    ///
    /// Uses the cache to find the file path for faster lookup.
    fn find_ncr(&self, id: &str) -> ServiceResult<(PathBuf, Ncr)> {
        // Try to find in cache first for fast path lookup
        if let Some(cached) = self.cache.get_entity(id) {
            let path = if cached.file_path.is_absolute() {
                cached.file_path.clone()
            } else {
                self.project.root().join(&cached.file_path)
            };
            if path.exists() {
                if let Ok(ncr) = crate::yaml::parse_yaml_file::<Ncr>(&path) {
                    return Ok((path, ncr));
                }
            }
        }

        // Fall back to directory scan
        let dir = self.ncr_dir();
        if let Some((path, ncr)) = loader::load_entity::<Ncr>(&dir, id)? {
            return Ok((path, ncr));
        }
        Err(ServiceError::NotFound(format!("NCR: {}", id)))
    }

    /// List NCRs with filtering and sorting
    pub fn list(&self, filter: &NcrFilter) -> ServiceResult<Vec<Ncr>> {
        let mut ncrs = self.load_all()?;

        // Apply filters
        ncrs.retain(|n| self.matches_filter(n, filter));

        // Sort
        crate::services::common::sort_entities(&mut ncrs, filter.sort, filter.sort_direction);

        // Apply limit from common filter
        if let Some(limit) = filter.common.limit {
            ncrs.truncate(limit);
        }

        Ok(ncrs)
    }

    /// Check if an NCR matches the filter
    fn matches_filter(&self, ncr: &Ncr, filter: &NcrFilter) -> bool {
        // Common filters
        if !filter.common.matches_entity(ncr) {
            return false;
        }

        // NCR type filter
        if let Some(ref t) = filter.ncr_type {
            if &ncr.ncr_type != t {
                return false;
            }
        }

        // Severity filter
        if let Some(ref s) = filter.severity {
            if &ncr.severity != s {
                return false;
            }
        }

        // NCR status filter
        if let Some(ref status) = filter.ncr_status {
            if &ncr.ncr_status != status {
                return false;
            }
        }

        // Category filter
        if let Some(ref cat) = filter.category {
            if &ncr.category != cat {
                return false;
            }
        }

        // Open only filter
        if filter.open_only && ncr.ncr_status == NcrStatus::Closed {
            return false;
        }

        // Recent days filter
        if let Some(days) = filter.recent_days {
            let cutoff = Utc::now() - chrono::Duration::days(days as i64);
            if ncr.created < cutoff {
                return false;
            }
        }

        true
    }

    /// Get an NCR by ID
    pub fn get(&self, id: &str) -> ServiceResult<Option<Ncr>> {
        let dir = self.ncr_dir();
        if let Some((_, ncr)) = loader::load_entity::<Ncr>(&dir, id)? {
            return Ok(Some(ncr));
        }
        Ok(None)
    }

    /// Get an NCR by ID, returning an error if not found
    pub fn get_required(&self, id: &str) -> ServiceResult<Ncr> {
        self.get(id)?
            .ok_or_else(|| ServiceError::NotFound(format!("NCR: {}", id)))
    }

    /// Create a new NCR
    pub fn create(&self, input: CreateNcr) -> ServiceResult<Ncr> {
        let now = Utc::now();
        let id = EntityId::new(EntityPrefix::Ncr);

        let ncr = Ncr {
            id: id.clone(),
            title: input.title,
            ncr_number: input.ncr_number,
            report_date: input.report_date.or_else(|| Some(now.date_naive())),
            description: input.description,
            ncr_type: input.ncr_type,
            severity: input.severity,
            category: input.category,
            detection: None,
            affected_items: None,
            defect: None,
            containment: Vec::new(),
            disposition: None,
            cost_impact: None,
            ncr_status: NcrStatus::Open,
            tags: input.tags,
            status: input.status.unwrap_or(Status::Draft),
            links: NcrLinks {
                lot: input
                    .lot_ids
                    .into_iter()
                    .filter_map(|s| s.parse::<EntityId>().ok())
                    .map(LinkRef::from)
                    .collect(),
                ..Default::default()
            },
            created: now,
            author: input.author,
            entity_revision: 1,
        };

        // Ensure directory exists
        let dir = self.ncr_dir();
        fs::create_dir_all(&dir)?;

        // Save
        let file_path = self.get_file_path(&id);
        self.base.save(&ncr, &file_path, Some("NCR"))?;

        Ok(ncr)
    }

    /// Update an existing NCR
    pub fn update(&self, id: &str, input: UpdateNcr) -> ServiceResult<Ncr> {
        let (_, mut ncr) = self.find_ncr(id)?;

        // Apply updates
        if let Some(title) = input.title {
            ncr.title = title;
        }
        if let Some(ncr_number) = input.ncr_number {
            ncr.ncr_number = ncr_number;
        }
        if let Some(ncr_type) = input.ncr_type {
            ncr.ncr_type = ncr_type;
        }
        if let Some(severity) = input.severity {
            ncr.severity = severity;
        }
        if let Some(category) = input.category {
            ncr.category = category;
        }
        if let Some(description) = input.description {
            ncr.description = description;
        }
        if let Some(tags) = input.tags {
            ncr.tags = tags;
        }
        if let Some(status) = input.status {
            ncr.status = status;
        }
        if let Some(ncr_status) = input.ncr_status {
            ncr.ncr_status = ncr_status;
        }

        ncr.entity_revision += 1;

        // Save
        let file_path = self.get_file_path(&ncr.id);
        self.base.save(&ncr, &file_path, None)?;

        Ok(ncr)
    }

    /// Delete an NCR
    pub fn delete(&self, id: &str, force: bool) -> ServiceResult<()> {
        let (path, _ncr) = self.find_ncr(id)?;

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

    /// Advance NCR to next workflow status
    pub fn advance_status(&self, id: &str) -> ServiceResult<Ncr> {
        let (_, mut ncr) = self.find_ncr(id)?;

        // Advance to next status
        ncr.ncr_status = match ncr.ncr_status {
            NcrStatus::Open => NcrStatus::Containment,
            NcrStatus::Containment => NcrStatus::Investigation,
            NcrStatus::Investigation => NcrStatus::Disposition,
            NcrStatus::Disposition => NcrStatus::Closed,
            NcrStatus::Closed => {
                return Err(ServiceError::ValidationFailed(
                    "NCR is already closed".to_string(),
                ))
            }
        };

        ncr.entity_revision += 1;

        // Save
        let file_path = self.get_file_path(&ncr.id);
        self.base.save(&ncr, &file_path, None)?;

        Ok(ncr)
    }

    /// Close an NCR with disposition
    pub fn close(
        &self,
        id: &str,
        decision: DispositionDecision,
        justification: Option<String>,
        decision_by: String,
    ) -> ServiceResult<Ncr> {
        let (_, mut ncr) = self.find_ncr(id)?;

        // Validate not already closed
        if ncr.ncr_status == NcrStatus::Closed {
            return Err(ServiceError::ValidationFailed(
                "NCR is already closed".to_string(),
            ));
        }

        // Set disposition
        ncr.disposition = Some(Disposition {
            decision: Some(decision),
            decision_date: Some(Utc::now().date_naive()),
            decision_by: Some(decision_by),
            justification,
            mrb_required: false,
        });

        // Set status to closed
        ncr.ncr_status = NcrStatus::Closed;
        ncr.entity_revision += 1;

        // Save
        let file_path = self.get_file_path(&ncr.id);
        self.base.save(&ncr, &file_path, None)?;

        Ok(ncr)
    }

    /// Add a containment action
    pub fn add_containment(&self, id: &str, action: String) -> ServiceResult<Ncr> {
        let (_, mut ncr) = self.find_ncr(id)?;

        ncr.containment.push(ContainmentAction {
            action,
            date: None,
            completed_by: None,
            status: ContainmentStatus::Open,
        });

        ncr.entity_revision += 1;

        // Save
        let file_path = self.get_file_path(&ncr.id);
        self.base.save(&ncr, &file_path, None)?;

        Ok(ncr)
    }

    /// Complete a containment action
    pub fn complete_containment(
        &self,
        id: &str,
        index: usize,
        completed_by: String,
    ) -> ServiceResult<Ncr> {
        let (_, mut ncr) = self.find_ncr(id)?;

        if index >= ncr.containment.len() {
            return Err(ServiceError::ValidationFailed(format!(
                "Containment action index {} out of range (0-{})",
                index,
                ncr.containment.len().saturating_sub(1)
            )));
        }

        ncr.containment[index].status = ContainmentStatus::Completed;
        ncr.containment[index].completed_by = Some(completed_by);
        ncr.containment[index].date = Some(Utc::now().date_naive());

        ncr.entity_revision += 1;

        // Save
        let file_path = self.get_file_path(&ncr.id);
        self.base.save(&ncr, &file_path, None)?;

        Ok(ncr)
    }

    /// Remove a containment action
    pub fn remove_containment(&self, id: &str, index: usize) -> ServiceResult<Ncr> {
        let (_, mut ncr) = self.find_ncr(id)?;

        if index >= ncr.containment.len() {
            return Err(ServiceError::ValidationFailed(format!(
                "Containment action index {} out of range (0-{})",
                index,
                ncr.containment.len().saturating_sub(1)
            )));
        }

        ncr.containment.remove(index);
        ncr.entity_revision += 1;

        // Save
        let file_path = self.get_file_path(&ncr.id);
        self.base.save(&ncr, &file_path, None)?;

        Ok(ncr)
    }

    /// Set detection information
    pub fn set_detection(
        &self,
        id: &str,
        found_at: DetectionStage,
        found_by: Option<String>,
        found_date: Option<NaiveDate>,
        operation: Option<String>,
    ) -> ServiceResult<Ncr> {
        let (_, mut ncr) = self.find_ncr(id)?;

        ncr.detection = Some(Detection {
            found_at,
            found_by,
            found_date,
            operation,
        });

        ncr.entity_revision += 1;

        // Save
        let file_path = self.get_file_path(&ncr.id);
        self.base.save(&ncr, &file_path, None)?;

        Ok(ncr)
    }

    /// Set affected items
    pub fn set_affected_items(
        &self,
        id: &str,
        part_number: Option<String>,
        lot_number: Option<String>,
        serial_numbers: Vec<String>,
        quantity_affected: Option<u32>,
    ) -> ServiceResult<Ncr> {
        let (_, mut ncr) = self.find_ncr(id)?;

        ncr.affected_items = Some(AffectedItems {
            part_number,
            lot_number,
            serial_numbers,
            quantity_affected,
        });

        ncr.entity_revision += 1;

        // Save
        let file_path = self.get_file_path(&ncr.id);
        self.base.save(&ncr, &file_path, None)?;

        Ok(ncr)
    }

    /// Set defect information
    pub fn set_defect(
        &self,
        id: &str,
        characteristic: Option<String>,
        specification: Option<String>,
        actual: Option<String>,
        deviation: Option<f64>,
    ) -> ServiceResult<Ncr> {
        let (_, mut ncr) = self.find_ncr(id)?;

        ncr.defect = Some(Defect {
            characteristic,
            specification,
            actual,
            deviation,
        });

        ncr.entity_revision += 1;

        // Save
        let file_path = self.get_file_path(&ncr.id);
        self.base.save(&ncr, &file_path, None)?;

        Ok(ncr)
    }

    /// Set cost impact
    pub fn set_cost(
        &self,
        id: &str,
        rework_cost: Option<f64>,
        scrap_cost: Option<f64>,
        currency: Option<String>,
    ) -> ServiceResult<Ncr> {
        let (_, mut ncr) = self.find_ncr(id)?;

        ncr.cost_impact = Some(CostImpact {
            rework_cost,
            scrap_cost,
            currency,
        });

        ncr.entity_revision += 1;

        // Save
        let file_path = self.get_file_path(&ncr.id);
        self.base.save(&ncr, &file_path, None)?;

        Ok(ncr)
    }

    /// Set component link
    pub fn set_component_link(&self, id: &str, component_id: EntityId) -> ServiceResult<Ncr> {
        let (_, mut ncr) = self.find_ncr(id)?;

        ncr.links.component = Some(LinkRef::from(component_id));
        ncr.entity_revision += 1;

        // Save
        let file_path = self.get_file_path(&ncr.id);
        self.base.save(&ncr, &file_path, None)?;

        Ok(ncr)
    }

    /// Set CAPA link
    pub fn set_capa_link(&self, id: &str, capa_id: EntityId) -> ServiceResult<Ncr> {
        let (_, mut ncr) = self.find_ncr(id)?;

        ncr.links.capa = Some(LinkRef::from(capa_id));
        ncr.entity_revision += 1;

        // Save
        let file_path = self.get_file_path(&ncr.id);
        self.base.save(&ncr, &file_path, None)?;

        Ok(ncr)
    }

    /// Add a lot link
    pub fn add_lot_link(&self, id: &str, lot_id: EntityId) -> ServiceResult<Ncr> {
        let (_, mut ncr) = self.find_ncr(id)?;

        if !ncr.links.lot.iter().any(|l| *l == lot_id) {
            ncr.links.lot.push(LinkRef::from(lot_id));
        }
        ncr.entity_revision += 1;

        let file_path = self.get_file_path(&ncr.id);
        self.base.save(&ncr, &file_path, None)?;

        Ok(ncr)
    }

    /// Remove a lot link
    pub fn remove_lot_link(&self, id: &str, lot_id: &EntityId) -> ServiceResult<Ncr> {
        let (_, mut ncr) = self.find_ncr(id)?;

        let original_len = ncr.links.lot.len();
        ncr.links.lot.retain(|l| *l != *lot_id);

        if ncr.links.lot.len() == original_len {
            return Err(ServiceError::NotFound(format!(
                "Lot link '{}' not found on NCR",
                lot_id
            )));
        }

        ncr.entity_revision += 1;

        let file_path = self.get_file_path(&ncr.id);
        self.base.save(&ncr, &file_path, None)?;

        Ok(ncr)
    }

    /// Calculate statistics
    pub fn stats(&self) -> ServiceResult<NcrStats> {
        let ncrs = self.load_all()?;

        let mut stats = NcrStats {
            total: ncrs.len(),
            ..Default::default()
        };

        for ncr in &ncrs {
            // Count by NCR status
            match ncr.ncr_status {
                NcrStatus::Open => stats.by_ncr_status.open += 1,
                NcrStatus::Containment => stats.by_ncr_status.containment += 1,
                NcrStatus::Investigation => stats.by_ncr_status.investigation += 1,
                NcrStatus::Disposition => stats.by_ncr_status.disposition += 1,
                NcrStatus::Closed => stats.by_ncr_status.closed += 1,
            }

            // Count by type
            match ncr.ncr_type {
                NcrType::Internal => stats.by_type.internal += 1,
                NcrType::Supplier => stats.by_type.supplier += 1,
                NcrType::Customer => stats.by_type.customer += 1,
            }

            // Count by severity
            match ncr.severity {
                NcrSeverity::Minor => stats.by_severity.minor += 1,
                NcrSeverity::Major => stats.by_severity.major += 1,
                NcrSeverity::Critical => stats.by_severity.critical += 1,
            }

            // Count by category
            match ncr.category {
                NcrCategory::Dimensional => stats.by_category.dimensional += 1,
                NcrCategory::Cosmetic => stats.by_category.cosmetic += 1,
                NcrCategory::Material => stats.by_category.material += 1,
                NcrCategory::Functional => stats.by_category.functional += 1,
                NcrCategory::Documentation => stats.by_category.documentation += 1,
                NcrCategory::Process => stats.by_category.process += 1,
                NcrCategory::Packaging => stats.by_category.packaging += 1,
            }

            // Count open
            if ncr.ncr_status != NcrStatus::Closed {
                stats.open += 1;
            }

            // Sum costs
            if let Some(ref cost) = ncr.cost_impact {
                if let Some(rework) = cost.rework_cost {
                    stats.total_rework_cost += rework;
                }
                if let Some(scrap) = cost.scrap_cost {
                    stats.total_scrap_cost += scrap;
                }
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

    fn create_test_ncr(service: &NcrService) -> Ncr {
        service
            .create(CreateNcr {
                title: "Test NCR".to_string(),
                ncr_number: Some("NCR-2024-001".to_string()),
                ncr_type: NcrType::Internal,
                severity: NcrSeverity::Minor,
                category: NcrCategory::Dimensional,
                description: Some("Test description".to_string()),
                report_date: None,
                tags: Vec::new(),
                status: None,
                author: "author".to_string(),
                lot_ids: Vec::new(),
            })
            .unwrap()
    }

    #[test]
    fn test_create_ncr() {
        let (_tmp, project, cache) = setup();
        let service = NcrService::new(&project, &cache);

        let ncr = service
            .create(CreateNcr {
                title: "Bore Diameter Out of Tolerance".to_string(),
                ncr_number: Some("NCR-2024-042".to_string()),
                ncr_type: NcrType::Internal,
                severity: NcrSeverity::Major,
                category: NcrCategory::Dimensional,
                description: Some("Bore 0.5mm oversize".to_string()),
                report_date: None,
                tags: Vec::new(),
                status: None,
                author: "author".to_string(),
                lot_ids: Vec::new(),
            })
            .unwrap();

        assert!(ncr.id.to_string().starts_with("NCR-"));
        assert_eq!(ncr.title, "Bore Diameter Out of Tolerance");
        assert_eq!(ncr.ncr_type, NcrType::Internal);
        assert_eq!(ncr.ncr_status, NcrStatus::Open);
    }

    #[test]
    fn test_get_ncr() {
        let (_tmp, project, cache) = setup();
        let service = NcrService::new(&project, &cache);

        let created = create_test_ncr(&service);
        let retrieved = service.get(&created.id.to_string()).unwrap().unwrap();

        assert_eq!(created.id, retrieved.id);
        assert_eq!(created.title, retrieved.title);
    }

    #[test]
    fn test_update_ncr() {
        let (_tmp, project, cache) = setup();
        let service = NcrService::new(&project, &cache);

        let created = create_test_ncr(&service);
        let updated = service
            .update(
                &created.id.to_string(),
                UpdateNcr {
                    title: Some("Updated NCR".to_string()),
                    severity: Some(NcrSeverity::Critical),
                    ..Default::default()
                },
            )
            .unwrap();

        assert_eq!(updated.title, "Updated NCR");
        assert_eq!(updated.severity, NcrSeverity::Critical);
        assert_eq!(updated.entity_revision, 2);
    }

    #[test]
    fn test_delete_ncr() {
        let (_tmp, project, cache) = setup();
        let service = NcrService::new(&project, &cache);

        let created = create_test_ncr(&service);
        service.delete(&created.id.to_string(), false).unwrap();

        let result = service.get(&created.id.to_string()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_advance_status() {
        let (_tmp, project, cache) = setup();
        let service = NcrService::new(&project, &cache);

        let created = create_test_ncr(&service);
        assert_eq!(created.ncr_status, NcrStatus::Open);

        let advanced = service.advance_status(&created.id.to_string()).unwrap();
        assert_eq!(advanced.ncr_status, NcrStatus::Containment);

        let advanced = service.advance_status(&advanced.id.to_string()).unwrap();
        assert_eq!(advanced.ncr_status, NcrStatus::Investigation);
    }

    #[test]
    fn test_close_ncr() {
        let (_tmp, project, cache) = setup();
        let service = NcrService::new(&project, &cache);

        let created = create_test_ncr(&service);
        let closed = service
            .close(
                &created.id.to_string(),
                DispositionDecision::Rework,
                Some("Can be salvaged".to_string()),
                "reviewer".to_string(),
            )
            .unwrap();

        assert_eq!(closed.ncr_status, NcrStatus::Closed);
        assert!(closed.disposition.is_some());
        assert_eq!(
            closed.disposition.unwrap().decision,
            Some(DispositionDecision::Rework)
        );
    }

    #[test]
    fn test_add_containment() {
        let (_tmp, project, cache) = setup();
        let service = NcrService::new(&project, &cache);

        let created = create_test_ncr(&service);
        let updated = service
            .add_containment(
                &created.id.to_string(),
                "Quarantine affected parts".to_string(),
            )
            .unwrap();

        assert_eq!(updated.containment.len(), 1);
        assert_eq!(updated.containment[0].action, "Quarantine affected parts");
        assert_eq!(updated.containment[0].status, ContainmentStatus::Open);
    }

    #[test]
    fn test_complete_containment() {
        let (_tmp, project, cache) = setup();
        let service = NcrService::new(&project, &cache);

        let created = create_test_ncr(&service);
        let with_containment = service
            .add_containment(&created.id.to_string(), "Quarantine parts".to_string())
            .unwrap();

        let completed = service
            .complete_containment(&with_containment.id.to_string(), 0, "inspector".to_string())
            .unwrap();

        assert_eq!(
            completed.containment[0].status,
            ContainmentStatus::Completed
        );
        assert_eq!(
            completed.containment[0].completed_by,
            Some("inspector".to_string())
        );
    }

    #[test]
    fn test_set_cost() {
        let (_tmp, project, cache) = setup();
        let service = NcrService::new(&project, &cache);

        let created = create_test_ncr(&service);
        let updated = service
            .set_cost(
                &created.id.to_string(),
                Some(500.0),
                Some(1200.0),
                Some("USD".to_string()),
            )
            .unwrap();

        assert!(updated.cost_impact.is_some());
        let cost = updated.cost_impact.unwrap();
        assert_eq!(cost.rework_cost, Some(500.0));
        assert_eq!(cost.scrap_cost, Some(1200.0));
    }

    #[test]
    fn test_list_with_filter() {
        let (_tmp, project, cache) = setup();
        let service = NcrService::new(&project, &cache);

        // Create NCRs with different severities
        service
            .create(CreateNcr {
                title: "Minor Issue".to_string(),
                ncr_number: None,
                ncr_type: NcrType::Internal,
                severity: NcrSeverity::Minor,
                category: NcrCategory::Cosmetic,
                description: None,
                report_date: None,
                tags: Vec::new(),
                status: None,
                author: "author".to_string(),
                lot_ids: Vec::new(),
            })
            .unwrap();
        service
            .create(CreateNcr {
                title: "Critical Issue".to_string(),
                ncr_number: None,
                ncr_type: NcrType::Internal,
                severity: NcrSeverity::Critical,
                category: NcrCategory::Functional,
                description: None,
                report_date: None,
                tags: Vec::new(),
                status: None,
                author: "author".to_string(),
                lot_ids: Vec::new(),
            })
            .unwrap();

        // Filter by severity
        let critical_ncrs = service
            .list(&NcrFilter {
                severity: Some(NcrSeverity::Critical),
                ..Default::default()
            })
            .unwrap();

        assert_eq!(critical_ncrs.len(), 1);
        assert_eq!(critical_ncrs[0].severity, NcrSeverity::Critical);
    }

    #[test]
    fn test_stats() {
        let (_tmp, project, cache) = setup();
        let service = NcrService::new(&project, &cache);

        // Create some NCRs
        let ncr1 = create_test_ncr(&service);
        service
            .set_cost(&ncr1.id.to_string(), Some(100.0), Some(200.0), None)
            .unwrap();

        service
            .create(CreateNcr {
                title: "Second NCR".to_string(),
                ncr_number: None,
                ncr_type: NcrType::Supplier,
                severity: NcrSeverity::Major,
                category: NcrCategory::Material,
                description: None,
                report_date: None,
                tags: Vec::new(),
                status: None,
                author: "author".to_string(),
                lot_ids: Vec::new(),
            })
            .unwrap();

        let stats = service.stats().unwrap();

        assert_eq!(stats.total, 2);
        assert_eq!(stats.by_type.internal, 1);
        assert_eq!(stats.by_type.supplier, 1);
        assert_eq!(stats.open, 2);
        assert_eq!(stats.total_rework_cost, 100.0);
        assert_eq!(stats.total_scrap_cost, 200.0);
    }
}
