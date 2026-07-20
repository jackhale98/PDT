//! CAPA service for Corrective/Preventive Action management
//!
//! Provides CRUD operations and workflow management for CAPAs.

use crate::core::suspect::LinkRef;
use chrono::{NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::core::cache::{CachedCapa, EntityCache};
use crate::core::entity::{Entity, Status};
use crate::core::identity::EntityId;
use crate::core::loader;
use crate::core::project::Project;
use crate::entities::capa::{
    ActionItem, ActionStatus, ActionType, Capa, CapaStatus, CapaType, Closure, Effectiveness,
    EffectivenessResult, RcaMethod, RootCauseAnalysis, Source, SourceType, Timeline,
};
use crate::services::base::ServiceBase;
use crate::services::common::{
    CommonFilter, ServiceError, ServiceResult, SortDirection, SortKey, Sortable,
};

/// Filter options for listing CAPAs
#[derive(Debug, Clone, Default)]
pub struct CapaFilter {
    /// Common filters (status, author, tags, search)
    pub common: CommonFilter,
    /// Filter by CAPA type
    pub capa_type: Option<CapaType>,
    /// Filter by CAPA workflow status
    pub capa_status: Option<CapaStatus>,
    /// Show only overdue CAPAs
    pub overdue_only: bool,
    /// Show only open CAPAs (not closed)
    pub open_only: bool,
    /// Show recent CAPAs (last N days)
    pub recent_days: Option<u32>,
    /// Sort field
    pub sort: CapaSortField,
    /// Sort direction
    pub sort_direction: SortDirection,
}

/// Fields available for sorting CAPAs
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapaSortField {
    Id,
    Title,
    CapaType,
    CapaStatus,
    TargetDate,
    Status,
    Author,
    #[default]
    Created,
}

impl Sortable for Capa {
    type SortField = CapaSortField;

    fn sort_key(&self, field: &Self::SortField) -> SortKey {
        match field {
            CapaSortField::Id => SortKey::String(self.id.to_string()),
            CapaSortField::Title => SortKey::String(self.title.clone()),
            CapaSortField::CapaType => SortKey::String(self.capa_type.to_string()),
            CapaSortField::CapaStatus => SortKey::String(self.capa_status.to_string()),
            CapaSortField::TargetDate => {
                let ts = self
                    .timeline
                    .as_ref()
                    .and_then(|t| t.target_date)
                    .map(|d| d.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp());
                match ts {
                    Some(t) => SortKey::DateTime(t),
                    None => SortKey::DateTime(i64::MAX), // None goes last
                }
            }
            CapaSortField::Status => SortKey::String(self.status().to_string()),
            CapaSortField::Author => SortKey::String(self.author.clone()),
            CapaSortField::Created => SortKey::DateTime(self.created.timestamp()),
        }
    }
}

/// Input for creating a new CAPA
#[derive(Debug, Clone)]
pub struct CreateCapa {
    /// CAPA title
    pub title: String,
    /// CAPA type
    pub capa_type: CapaType,
    /// CAPA number (optional)
    pub capa_number: Option<String>,
    /// Problem statement
    pub problem_statement: Option<String>,
    /// Source type
    pub source_type: Option<SourceType>,
    /// Source reference
    pub source_reference: Option<String>,
    /// Target date
    pub target_date: Option<NaiveDate>,
    /// Tags
    pub tags: Vec<String>,
    /// Author
    pub author: String,
}

/// Input for updating an existing CAPA
#[derive(Debug, Clone, Default)]
pub struct UpdateCapa {
    /// Update title
    pub title: Option<String>,
    /// Update CAPA type
    pub capa_type: Option<CapaType>,
    /// Update CAPA number
    pub capa_number: Option<Option<String>>,
    /// Update problem statement
    pub problem_statement: Option<Option<String>>,
    /// Update target date
    pub target_date: Option<Option<NaiveDate>>,
    /// Update document status
    pub status: Option<Status>,
}

/// Input for adding an action item
#[derive(Debug, Clone)]
pub struct AddActionInput {
    /// Action description
    pub description: String,
    /// Action type
    pub action_type: ActionType,
    /// Responsible owner
    pub owner: Option<String>,
    /// Due date
    pub due_date: Option<NaiveDate>,
}

/// Statistics about CAPAs
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CapaStats {
    /// Total number of CAPAs
    pub total: usize,
    /// Counts by CAPA type
    pub by_type: CapaTypeCounts,
    /// Counts by workflow status
    pub by_status: CapaWorkflowCounts,
    /// Number of overdue CAPAs
    pub overdue_count: usize,
    /// Number of effective CAPAs
    pub effective_count: usize,
    /// Number of ineffective CAPAs
    pub ineffective_count: usize,
    /// Total actions
    pub total_actions: usize,
    /// Completed actions
    pub completed_actions: usize,
}

/// CAPA type counts
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CapaTypeCounts {
    pub corrective: usize,
    pub preventive: usize,
}

/// CAPA workflow status counts
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CapaWorkflowCounts {
    pub initiation: usize,
    pub investigation: usize,
    pub implementation: usize,
    pub verification: usize,
    pub closed: usize,
}

/// Service for managing CAPAs
pub struct CapaService<'a> {
    project: &'a Project,
    base: ServiceBase<'a>,
    cache: &'a EntityCache,
}

impl<'a> CapaService<'a> {
    /// Create a new CapaService
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

    /// Get the CAPAs directory
    fn capa_dir(&self) -> PathBuf {
        self.project().root().join("quality/capas")
    }

    /// Get the file path for a CAPA
    fn get_file_path(&self, id: &EntityId) -> PathBuf {
        self.capa_dir().join(format!("{}.tdt.yaml", id))
    }

    /// Load all CAPAs
    fn load_all(&self) -> ServiceResult<Vec<Capa>> {
        let dir = self.capa_dir();
        if !dir.exists() {
            return Ok(Vec::new());
        }
        loader::load_all(&dir).map_err(ServiceError::from)
    }

    /// List CAPAs using the cache (fast path)
    ///
    /// Returns cached CAPA data without loading full entities from disk.
    /// Use this for list views and simple queries.
    pub fn list_cached(&self, filter: &CapaFilter) -> ServiceResult<Vec<CachedCapa>> {
        // Convert filter values to strings for cache query
        let status_str = filter
            .common
            .status
            .as_ref()
            .and_then(|s| s.first())
            .map(|s| format!("{:?}", s).to_lowercase());

        let capa_type_str = filter.capa_type.as_ref().map(|t| match t {
            CapaType::Corrective => "corrective",
            CapaType::Preventive => "preventive",
        });

        let capa_status_str = filter.capa_status.as_ref().map(|s| match s {
            CapaStatus::Initiation => "initiation",
            CapaStatus::Investigation => "investigation",
            CapaStatus::Implementation => "implementation",
            CapaStatus::Verification => "verification",
            CapaStatus::Closed => "closed",
        });

        // Query cache
        let mut cached = self.cache.list_capas(
            status_str.as_deref(),
            capa_type_str,
            capa_status_str,
            filter.common.author.as_deref(),
            None, // Apply limit after all filters
        );

        // Apply additional filters not supported by cache query
        if let Some(days) = filter.recent_days {
            let cutoff = Utc::now() - chrono::Duration::days(days as i64);
            cached.retain(|c| c.created >= cutoff);
        }

        if filter.open_only {
            cached.retain(|c| {
                c.capa_status
                    .as_ref()
                    .map(|s| s != "closed")
                    .unwrap_or(true)
            });
        }

        // Note: overdue_only requires full entity load with timeline
        // This is handled in the regular list() method

        // Apply limit
        if let Some(limit) = filter.common.limit {
            cached.truncate(limit);
        }

        Ok(cached)
    }

    /// Find a CAPA by ID
    ///
    /// Uses the cache to find the file path for faster lookup.
    fn find_capa(&self, id: &str) -> ServiceResult<(PathBuf, Capa)> {
        // Try to find in cache first for fast path lookup
        if let Some(cached) = self.cache.get_entity(id) {
            let path = if cached.file_path.is_absolute() {
                cached.file_path.clone()
            } else {
                self.project.root().join(&cached.file_path)
            };
            if path.exists() {
                if let Ok(capa) = crate::yaml::parse_yaml_file::<Capa>(&path) {
                    return Ok((path, capa));
                }
            }
        }

        // Fall back to directory scan
        let dir = self.capa_dir();
        if let Some((path, capa)) = loader::load_entity::<Capa>(&dir, id)? {
            return Ok((path, capa));
        }
        Err(ServiceError::NotFound(format!("CAPA: {}", id)))
    }

    /// Check if a CAPA is overdue
    fn is_overdue(&self, capa: &Capa) -> bool {
        if capa.capa_status == CapaStatus::Closed {
            return false;
        }
        if let Some(ref timeline) = capa.timeline {
            if let Some(target) = timeline.target_date {
                return target < chrono::Local::now().date_naive();
            }
        }
        false
    }

    /// List CAPAs with filtering and sorting
    pub fn list(&self, filter: &CapaFilter) -> ServiceResult<Vec<Capa>> {
        let mut capas = self.load_all()?;

        // Apply filters
        capas.retain(|capa| {
            // Common filter
            if !filter.common.matches_status_str(capa.status()) {
                return false;
            }
            if !filter.common.matches_author(&capa.author) {
                return false;
            }
            let search_fields: Vec<&str> =
                vec![&capa.title, capa.problem_statement.as_deref().unwrap_or("")];
            if !filter.common.matches_search(&search_fields) {
                return false;
            }

            // CAPA type filter
            if let Some(ref ct) = filter.capa_type {
                if &capa.capa_type != ct {
                    return false;
                }
            }

            // CAPA status filter
            if let Some(ref cs) = filter.capa_status {
                if &capa.capa_status != cs {
                    return false;
                }
            }

            // Overdue filter
            if filter.overdue_only && !self.is_overdue(capa) {
                return false;
            }

            // Open filter
            if filter.open_only && capa.capa_status == CapaStatus::Closed {
                return false;
            }

            // Recent filter
            if let Some(days) = filter.recent_days {
                let cutoff = Utc::now() - chrono::Duration::days(days as i64);
                if capa.created < cutoff {
                    return false;
                }
            }

            true
        });

        // Sort
        crate::services::common::sort_entities(&mut capas, filter.sort, filter.sort_direction);

        Ok(capas)
    }

    /// Get a CAPA by ID
    pub fn get(&self, id: &str) -> ServiceResult<Option<Capa>> {
        match self.find_capa(id) {
            Ok((_, capa)) => Ok(Some(capa)),
            Err(ServiceError::NotFound(_)) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Get a CAPA by ID, returning error if not found
    pub fn get_required(&self, id: &str) -> ServiceResult<Capa> {
        let (_, capa) = self.find_capa(id)?;
        Ok(capa)
    }

    /// Create a new CAPA
    pub fn create(&self, input: CreateCapa) -> ServiceResult<Capa> {
        let mut capa = Capa::new(input.title, input.capa_type, input.author);

        capa.capa_number = input.capa_number;
        capa.problem_statement = input.problem_statement;
        capa.tags = input.tags;

        if input.source_type.is_some() || input.source_reference.is_some() {
            capa.source = Some(Source {
                source_type: input.source_type.unwrap_or_default(),
                reference: input.source_reference,
            });
        }

        if let Some(target) = input.target_date {
            if let Some(ref mut timeline) = capa.timeline {
                timeline.target_date = Some(target);
            } else {
                capa.timeline = Some(Timeline {
                    initiated_date: Some(chrono::Local::now().date_naive()),
                    target_date: Some(target),
                });
            }
        }

        // Ensure directory exists
        let dir = self.capa_dir();
        if !dir.exists() {
            fs::create_dir_all(&dir)?;
        }

        // Save
        let file_path = self.get_file_path(&capa.id);
        self.base.save(&capa, &file_path, Some("CAPA"))?;

        Ok(capa)
    }

    /// Update an existing CAPA
    pub fn update(&self, id: &str, input: UpdateCapa) -> ServiceResult<Capa> {
        let (_, mut capa) = self.find_capa(id)?;

        if let Some(title) = input.title {
            capa.title = title;
        }
        if let Some(capa_type) = input.capa_type {
            capa.capa_type = capa_type;
        }
        if let Some(capa_number) = input.capa_number {
            capa.capa_number = capa_number;
        }
        if let Some(problem_statement) = input.problem_statement {
            capa.problem_statement = problem_statement;
        }
        if let Some(target_date) = input.target_date {
            if let Some(ref mut timeline) = capa.timeline {
                timeline.target_date = target_date;
            } else if target_date.is_some() {
                capa.timeline = Some(Timeline {
                    initiated_date: None,
                    target_date,
                });
            }
        }
        if let Some(status) = input.status {
            capa.status = status;
        }

        // Increment revision
        capa.entity_revision += 1;

        // Save
        let file_path = self.get_file_path(&capa.id);
        self.base.save(&capa, &file_path, None)?;

        Ok(capa)
    }

    /// Delete a CAPA
    pub fn delete(&self, id: &str, force: bool) -> ServiceResult<()> {
        let (path, capa) = self.find_capa(id)?;

        if !force {
            // Check if CAPA has incomplete actions
            let incomplete = capa
                .actions
                .iter()
                .filter(|a| {
                    a.status != ActionStatus::Completed && a.status != ActionStatus::Verified
                })
                .count();
            if incomplete > 0 {
                return Err(ServiceError::ValidationFailed(format!(
                    "CAPA has {} incomplete action(s). Use --force to delete anyway.",
                    incomplete
                )));
            }
        }

        fs::remove_file(&path)?;
        Ok(())
    }

    /// Advance CAPA to the next workflow status
    pub fn advance_status(&self, id: &str) -> ServiceResult<Capa> {
        let (_, mut capa) = self.find_capa(id)?;

        capa.capa_status = match capa.capa_status {
            CapaStatus::Initiation => CapaStatus::Investigation,
            CapaStatus::Investigation => CapaStatus::Implementation,
            CapaStatus::Implementation => CapaStatus::Verification,
            CapaStatus::Verification => CapaStatus::Closed,
            CapaStatus::Closed => {
                return Err(ServiceError::ValidationFailed(
                    "CAPA is already closed".to_string(),
                ));
            }
        };

        capa.entity_revision += 1;

        let file_path = self.get_file_path(&capa.id);
        self.base.save(&capa, &file_path, None)?;

        Ok(capa)
    }

    /// Set CAPA to a specific workflow status
    pub fn set_status(&self, id: &str, status: CapaStatus) -> ServiceResult<Capa> {
        let (_, mut capa) = self.find_capa(id)?;

        capa.capa_status = status;
        capa.entity_revision += 1;

        let file_path = self.get_file_path(&capa.id);
        self.base.save(&capa, &file_path, None)?;

        Ok(capa)
    }

    /// Set root cause analysis
    pub fn set_root_cause(
        &self,
        id: &str,
        method: RcaMethod,
        root_cause: Option<String>,
        contributing_factors: Vec<String>,
    ) -> ServiceResult<Capa> {
        let (_, mut capa) = self.find_capa(id)?;

        capa.root_cause_analysis = Some(RootCauseAnalysis {
            method,
            root_cause,
            contributing_factors,
        });
        capa.entity_revision += 1;

        let file_path = self.get_file_path(&capa.id);
        self.base.save(&capa, &file_path, None)?;

        Ok(capa)
    }

    /// Add an action item to a CAPA
    pub fn add_action(&self, id: &str, input: AddActionInput) -> ServiceResult<Capa> {
        let (_, mut capa) = self.find_capa(id)?;

        let next_number = capa
            .actions
            .iter()
            .map(|a| a.action_number)
            .max()
            .unwrap_or(0)
            + 1;

        let action = ActionItem {
            action_number: next_number,
            description: input.description,
            action_type: input.action_type,
            owner: input.owner,
            due_date: input.due_date,
            completed_date: None,
            status: ActionStatus::Open,
            evidence: None,
        };

        capa.actions.push(action);
        capa.entity_revision += 1;

        let file_path = self.get_file_path(&capa.id);
        self.base.save(&capa, &file_path, None)?;

        Ok(capa)
    }

    /// Update an action item status
    pub fn update_action_status(
        &self,
        id: &str,
        action_number: u32,
        status: ActionStatus,
    ) -> ServiceResult<Capa> {
        let (_, mut capa) = self.find_capa(id)?;

        let action = capa
            .actions
            .iter_mut()
            .find(|a| a.action_number == action_number)
            .ok_or_else(|| {
                ServiceError::NotFound(format!("Action #{} not found in CAPA", action_number))
            })?;

        action.status = status;
        if status == ActionStatus::Completed || status == ActionStatus::Verified {
            action.completed_date = Some(chrono::Local::now().date_naive());
        }

        capa.entity_revision += 1;

        let file_path = self.get_file_path(&capa.id);
        self.base.save(&capa, &file_path, None)?;

        Ok(capa)
    }

    /// Record effectiveness verification
    pub fn verify_effectiveness(
        &self,
        id: &str,
        result: EffectivenessResult,
        evidence: Option<String>,
    ) -> ServiceResult<Capa> {
        let (_, mut capa) = self.find_capa(id)?;

        capa.effectiveness = Some(Effectiveness {
            verified: true,
            verified_date: Some(chrono::Local::now().date_naive()),
            result: Some(result),
            evidence,
        });

        // If effective, auto-close
        if result == EffectivenessResult::Effective {
            capa.capa_status = CapaStatus::Closed;
            capa.closure = Some(Closure {
                closed: true,
                closed_date: Some(chrono::Local::now().date_naive()),
                closed_by: None,
            });
        }

        capa.entity_revision += 1;

        let file_path = self.get_file_path(&capa.id);
        self.base.save(&capa, &file_path, None)?;

        Ok(capa)
    }

    /// Close a CAPA
    pub fn close(&self, id: &str, closed_by: Option<String>) -> ServiceResult<Capa> {
        let (_, mut capa) = self.find_capa(id)?;

        if capa.capa_status == CapaStatus::Closed {
            return Err(ServiceError::ValidationFailed(
                "CAPA is already closed".to_string(),
            ));
        }

        capa.capa_status = CapaStatus::Closed;
        capa.closure = Some(Closure {
            closed: true,
            closed_date: Some(chrono::Local::now().date_naive()),
            closed_by,
        });
        capa.entity_revision += 1;

        let file_path = self.get_file_path(&capa.id);
        self.base.save(&capa, &file_path, None)?;

        Ok(capa)
    }

    /// Link an NCR to this CAPA
    pub fn add_ncr_link(&self, id: &str, ncr_id: &EntityId) -> ServiceResult<Capa> {
        let (_, mut capa) = self.find_capa(id)?;

        if !capa.links.ncrs.iter().any(|l| *l == *ncr_id) {
            capa.links.ncrs.push(LinkRef::from(ncr_id));
        }
        capa.entity_revision += 1;

        let file_path = self.get_file_path(&capa.id);
        self.base.save(&capa, &file_path, None)?;

        Ok(capa)
    }

    /// Link a risk to this CAPA
    pub fn add_risk_link(&self, id: &str, risk_id: &EntityId) -> ServiceResult<Capa> {
        let (_, mut capa) = self.find_capa(id)?;

        if !capa.links.risks.iter().any(|l| *l == *risk_id) {
            capa.links.risks.push(LinkRef::from(risk_id));
        }
        capa.entity_revision += 1;

        let file_path = self.get_file_path(&capa.id);
        self.base.save(&capa, &file_path, None)?;

        Ok(capa)
    }

    /// Get statistics about CAPAs
    pub fn stats(&self) -> ServiceResult<CapaStats> {
        let capas = self.load_all()?;

        let mut stats = CapaStats {
            total: capas.len(),
            ..Default::default()
        };

        for capa in &capas {
            // Count by type
            match capa.capa_type {
                CapaType::Corrective => stats.by_type.corrective += 1,
                CapaType::Preventive => stats.by_type.preventive += 1,
            }

            // Count by workflow status
            match capa.capa_status {
                CapaStatus::Initiation => stats.by_status.initiation += 1,
                CapaStatus::Investigation => stats.by_status.investigation += 1,
                CapaStatus::Implementation => stats.by_status.implementation += 1,
                CapaStatus::Verification => stats.by_status.verification += 1,
                CapaStatus::Closed => stats.by_status.closed += 1,
            }

            // Overdue count
            if self.is_overdue(capa) {
                stats.overdue_count += 1;
            }

            // Effectiveness counts
            if let Some(ref eff) = capa.effectiveness {
                if let Some(result) = eff.result {
                    match result {
                        EffectivenessResult::Effective => stats.effective_count += 1,
                        EffectivenessResult::Ineffective => stats.ineffective_count += 1,
                        EffectivenessResult::PartiallyEffective => {}
                    }
                }
            }

            // Action counts
            stats.total_actions += capa.actions.len();
            stats.completed_actions += capa
                .actions
                .iter()
                .filter(|a| {
                    a.status == ActionStatus::Completed || a.status == ActionStatus::Verified
                })
                .count();
        }

        Ok(stats)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::identity::EntityPrefix;
    use tempfile::TempDir;

    fn setup() -> (TempDir, Project, EntityCache) {
        let tmp = TempDir::new().unwrap();
        let project = Project::init(tmp.path()).unwrap();
        let cache = EntityCache::open(&project).unwrap();
        (tmp, project, cache)
    }

    fn make_create_capa(title: &str) -> CreateCapa {
        CreateCapa {
            title: title.to_string(),
            capa_type: CapaType::Corrective,
            capa_number: None,
            problem_statement: None,
            source_type: None,
            source_reference: None,
            target_date: None,
            tags: Vec::new(),
            author: "Test Author".to_string(),
        }
    }

    #[test]
    fn test_create_capa() {
        let (_tmp, project, cache) = setup();
        let service = CapaService::new(&project, &cache);

        let input = make_create_capa("Process Improvement");
        let capa = service.create(input).unwrap();

        assert!(capa.id.to_string().starts_with("CAPA-"));
        assert_eq!(capa.title, "Process Improvement");
        assert_eq!(capa.capa_type, CapaType::Corrective);
        assert_eq!(capa.capa_status, CapaStatus::Initiation);
    }

    #[test]
    fn test_get_capa() {
        let (_tmp, project, cache) = setup();
        let service = CapaService::new(&project, &cache);

        let input = make_create_capa("Test CAPA");
        let created = service.create(input).unwrap();

        let retrieved = service.get(&created.id.to_string()).unwrap().unwrap();
        assert_eq!(retrieved.id, created.id);
        assert_eq!(retrieved.title, "Test CAPA");
    }

    #[test]
    fn test_list_with_filter() {
        let (_tmp, project, cache) = setup();
        let service = CapaService::new(&project, &cache);

        // Create multiple CAPAs
        service.create(make_create_capa("CAPA A")).unwrap();

        let mut input_b = make_create_capa("CAPA B");
        input_b.capa_type = CapaType::Preventive;
        let capa_b = service.create(input_b).unwrap();

        service.create(make_create_capa("CAPA C")).unwrap();

        // List all
        let all = service.list(&CapaFilter::default()).unwrap();
        assert_eq!(all.len(), 3);

        // List preventive only
        let preventive = service
            .list(&CapaFilter {
                capa_type: Some(CapaType::Preventive),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(preventive.len(), 1);
        assert_eq!(preventive[0].id, capa_b.id);
    }

    #[test]
    fn test_update_capa() {
        let (_tmp, project, cache) = setup();
        let service = CapaService::new(&project, &cache);

        let input = make_create_capa("Test CAPA");
        let created = service.create(input).unwrap();

        let updated = service
            .update(
                &created.id.to_string(),
                UpdateCapa {
                    title: Some("Updated CAPA".to_string()),
                    capa_type: Some(CapaType::Preventive),
                    ..Default::default()
                },
            )
            .unwrap();

        assert_eq!(updated.title, "Updated CAPA");
        assert_eq!(updated.capa_type, CapaType::Preventive);
        assert_eq!(updated.entity_revision, 2);
    }

    #[test]
    fn test_delete_capa() {
        let (_tmp, project, cache) = setup();
        let service = CapaService::new(&project, &cache);

        let input = make_create_capa("Test CAPA");
        let created = service.create(input).unwrap();

        service.delete(&created.id.to_string(), false).unwrap();

        let result = service.get(&created.id.to_string()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_workflow_advance() {
        let (_tmp, project, cache) = setup();
        let service = CapaService::new(&project, &cache);

        let input = make_create_capa("Test CAPA");
        let capa = service.create(input).unwrap();
        assert_eq!(capa.capa_status, CapaStatus::Initiation);

        let capa = service.advance_status(&capa.id.to_string()).unwrap();
        assert_eq!(capa.capa_status, CapaStatus::Investigation);

        let capa = service.advance_status(&capa.id.to_string()).unwrap();
        assert_eq!(capa.capa_status, CapaStatus::Implementation);

        let capa = service.advance_status(&capa.id.to_string()).unwrap();
        assert_eq!(capa.capa_status, CapaStatus::Verification);

        let capa = service.advance_status(&capa.id.to_string()).unwrap();
        assert_eq!(capa.capa_status, CapaStatus::Closed);

        // Can't advance past closed
        let result = service.advance_status(&capa.id.to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_add_action() {
        let (_tmp, project, cache) = setup();
        let service = CapaService::new(&project, &cache);

        let input = make_create_capa("Test CAPA");
        let capa = service.create(input).unwrap();

        let capa = service
            .add_action(
                &capa.id.to_string(),
                AddActionInput {
                    description: "Update procedure".to_string(),
                    action_type: ActionType::Corrective,
                    owner: Some("John".to_string()),
                    due_date: None,
                },
            )
            .unwrap();

        assert_eq!(capa.actions.len(), 1);
        assert_eq!(capa.actions[0].action_number, 1);
        assert_eq!(capa.actions[0].description, "Update procedure");
    }

    #[test]
    fn test_update_action_status() {
        let (_tmp, project, cache) = setup();
        let service = CapaService::new(&project, &cache);

        let input = make_create_capa("Test CAPA");
        let capa = service.create(input).unwrap();

        let capa = service
            .add_action(
                &capa.id.to_string(),
                AddActionInput {
                    description: "Fix issue".to_string(),
                    action_type: ActionType::Corrective,
                    owner: None,
                    due_date: None,
                },
            )
            .unwrap();

        let capa = service
            .update_action_status(&capa.id.to_string(), 1, ActionStatus::Completed)
            .unwrap();

        assert_eq!(capa.actions[0].status, ActionStatus::Completed);
        assert!(capa.actions[0].completed_date.is_some());
    }

    #[test]
    fn test_verify_effectiveness() {
        let (_tmp, project, cache) = setup();
        let service = CapaService::new(&project, &cache);

        let input = make_create_capa("Test CAPA");
        let capa = service.create(input).unwrap();

        let capa = service
            .verify_effectiveness(
                &capa.id.to_string(),
                EffectivenessResult::Effective,
                Some("Defect rate reduced by 90%".to_string()),
            )
            .unwrap();

        assert!(capa.effectiveness.is_some());
        let eff = capa.effectiveness.unwrap();
        assert!(eff.verified);
        assert_eq!(eff.result, Some(EffectivenessResult::Effective));

        // Auto-closed because effective
        assert_eq!(capa.capa_status, CapaStatus::Closed);
    }

    #[test]
    fn test_ncr_link() {
        let (_tmp, project, cache) = setup();
        let service = CapaService::new(&project, &cache);

        let input = make_create_capa("Test CAPA");
        let capa = service.create(input).unwrap();

        let ncr_id = EntityId::new(EntityPrefix::Ncr);
        let capa = service.add_ncr_link(&capa.id.to_string(), &ncr_id).unwrap();

        assert!(capa.links.ncrs.iter().any(|l| *l == ncr_id));
    }

    #[test]
    fn test_stats() {
        let (_tmp, project, cache) = setup();
        let service = CapaService::new(&project, &cache);

        // Create CAPAs
        let capa_a = service.create(make_create_capa("CAPA A")).unwrap();

        let mut input_b = make_create_capa("CAPA B");
        input_b.capa_type = CapaType::Preventive;
        let capa_b = service.create(input_b).unwrap();

        // Add action to CAPA A
        service
            .add_action(
                &capa_a.id.to_string(),
                AddActionInput {
                    description: "Action 1".to_string(),
                    action_type: ActionType::Corrective,
                    owner: None,
                    due_date: None,
                },
            )
            .unwrap();

        // Verify effectiveness on CAPA B
        service
            .verify_effectiveness(&capa_b.id.to_string(), EffectivenessResult::Effective, None)
            .unwrap();

        let stats = service.stats().unwrap();
        assert_eq!(stats.total, 2);
        assert_eq!(stats.by_type.corrective, 1);
        assert_eq!(stats.by_type.preventive, 1);
        assert_eq!(stats.by_status.initiation, 1);
        assert_eq!(stats.by_status.closed, 1);
        assert_eq!(stats.total_actions, 1);
        assert_eq!(stats.effective_count, 1);
    }
}
