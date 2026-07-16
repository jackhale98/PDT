//! Risk service - business logic for FMEA risk management

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::core::cache::EntityCache;
use crate::core::entity::Status;
use crate::core::identity::{EntityId, EntityPrefix};
use crate::core::loader;
use crate::core::project::Project;
use crate::entities::risk::{
    InitialRisk, Mitigation, MitigationStatus, Risk, RiskLevel, RiskLinks, RiskType,
};

use super::base::ServiceBase;
use super::common::{
    apply_pagination, CommonFilter, ListResult, ServiceError, ServiceResult, SortDirection,
};

/// Filter options specific to risks
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RiskFilter {
    /// Common filter options (status, priority, author, search, etc.)
    #[serde(flatten)]
    pub common: CommonFilter,

    /// Filter by risk type
    pub risk_type: Option<RiskType>,

    /// Filter by risk level
    pub risk_level: Option<Vec<RiskLevel>>,

    /// Filter by minimum severity
    pub min_severity: Option<u8>,

    /// Filter by minimum occurrence
    pub min_occurrence: Option<u8>,

    /// Filter by minimum detection
    pub min_detection: Option<u8>,

    /// Filter by minimum RPN
    pub min_rpn: Option<u16>,

    /// Filter by maximum RPN
    pub max_rpn: Option<u16>,

    /// Show only unmitigated risks (no mitigations defined)
    pub unmitigated_only: bool,

    /// Show only risks with incomplete mitigations
    pub needs_mitigation: bool,

    /// Show only risks needing verification (no verified_by links)
    pub needs_verification: bool,

    /// Filter by category
    pub category: Option<String>,
}

impl RiskFilter {
    /// Create a filter for high-priority risks (critical and high level)
    pub fn high_priority() -> Self {
        Self {
            risk_level: Some(vec![RiskLevel::Critical, RiskLevel::High]),
            ..Default::default()
        }
    }

    /// Create a filter for unmitigated risks
    pub fn unmitigated() -> Self {
        Self {
            unmitigated_only: true,
            ..Default::default()
        }
    }

    /// Create a filter for design risks
    pub fn design() -> Self {
        Self {
            risk_type: Some(RiskType::Design),
            ..Default::default()
        }
    }

    /// Create a filter for process risks
    pub fn process() -> Self {
        Self {
            risk_type: Some(RiskType::Process),
            ..Default::default()
        }
    }
}

/// Sort field for risks
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskSortField {
    Id,
    Title,
    Type,
    Severity,
    Occurrence,
    Detection,
    #[default]
    Rpn,
    RiskLevel,
    Status,
    Author,
    Created,
    Category,
}

/// Input for creating a new risk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateRisk {
    /// Risk type
    pub risk_type: RiskType,

    /// Short title
    pub title: String,

    /// Detailed description
    pub description: String,

    /// Author name
    pub author: String,

    /// Category
    #[serde(default)]
    pub category: Option<String>,

    /// Tags
    #[serde(default)]
    pub tags: Vec<String>,

    /// Failure mode
    #[serde(default)]
    pub failure_mode: Option<String>,

    /// Cause
    #[serde(default)]
    pub cause: Option<String>,

    /// Effect
    #[serde(default)]
    pub effect: Option<String>,

    /// Severity (1-10)
    #[serde(default)]
    pub severity: Option<u8>,

    /// Occurrence (1-10)
    #[serde(default)]
    pub occurrence: Option<u8>,

    /// Detection (1-10)
    #[serde(default)]
    pub detection: Option<u8>,
}

impl Default for CreateRisk {
    fn default() -> Self {
        Self {
            risk_type: RiskType::Design,
            title: String::new(),
            description: String::new(),
            author: String::new(),
            category: None,
            tags: Vec::new(),
            failure_mode: None,
            cause: None,
            effect: None,
            severity: None,
            occurrence: None,
            detection: None,
        }
    }
}

/// Input for updating an existing risk
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateRisk {
    /// Update title
    pub title: Option<String>,

    /// Update description
    pub description: Option<String>,

    /// Update risk type
    pub risk_type: Option<RiskType>,

    /// Update status
    pub status: Option<Status>,

    /// Update category
    pub category: Option<String>,

    /// Update tags (replaces existing)
    pub tags: Option<Vec<String>>,

    /// Update failure mode
    pub failure_mode: Option<String>,

    /// Update cause
    pub cause: Option<String>,

    /// Update effect
    pub effect: Option<String>,

    /// Update severity (1-10)
    pub severity: Option<u8>,

    /// Update occurrence (1-10)
    pub occurrence: Option<u8>,

    /// Update detection (1-10)
    pub detection: Option<u8>,

    /// Update mitigations (replaces existing)
    pub mitigations: Option<Vec<Mitigation>>,

    /// Override risk level (normally computed)
    pub risk_level: Option<RiskLevel>,
}

/// Service for risk management (FMEA)
pub struct RiskService<'a> {
    project: &'a Project,
    cache: &'a EntityCache,
    base: ServiceBase<'a>,
}

impl<'a> RiskService<'a> {
    /// Create a new risk service
    pub fn new(project: &'a Project, cache: &'a EntityCache) -> Self {
        Self {
            project,
            cache,
            base: ServiceBase::new(project, cache),
        }
    }

    /// Get the directory for storing risks
    fn get_directory(&self) -> PathBuf {
        self.project.root().join("risks")
    }

    /// List risks with filtering and pagination
    pub fn list(
        &self,
        filter: &RiskFilter,
        sort_by: RiskSortField,
        sort_dir: SortDirection,
    ) -> ServiceResult<ListResult<Risk>> {
        let mut risks = self.load_all()?;

        // Apply filters
        risks.retain(|risk| self.matches_filter(risk, filter));

        // Sort
        self.sort_risks(&mut risks, sort_by, sort_dir);

        // Paginate
        Ok(apply_pagination(
            risks,
            filter.common.offset,
            filter.common.limit,
        ))
    }

    /// List risks from cache (fast path for list display)
    ///
    /// Returns cached risk data without loading full YAML files.
    /// Use this for list commands where full entity data isn't needed.
    pub fn list_cached(&self) -> Vec<crate::core::CachedEntity> {
        use crate::core::cache::EntityFilter;
        use crate::core::identity::EntityPrefix;

        let filter = EntityFilter {
            prefix: Some(EntityPrefix::Risk),
            ..Default::default()
        };
        self.cache.list_entities(&filter)
    }

    /// Load all risks from the filesystem
    pub fn load_all(&self) -> ServiceResult<Vec<Risk>> {
        let dir = self.get_directory();
        if !dir.exists() {
            return Ok(Vec::new());
        }
        Ok(loader::load_all(&dir)?)
    }

    /// Get a single risk by ID
    pub fn get(&self, id: &str) -> ServiceResult<Option<Risk>> {
        let dir = self.get_directory();
        if let Some((_, risk)) = loader::load_entity::<Risk>(&dir, id)? {
            return Ok(Some(risk));
        }
        Ok(None)
    }

    /// Get a risk by ID, returning an error if not found
    pub fn get_required(&self, id: &str) -> ServiceResult<Risk> {
        self.get(id)?
            .ok_or_else(|| ServiceError::NotFound(id.to_string()))
    }

    /// Create a new risk
    pub fn create(&self, input: CreateRisk) -> ServiceResult<Risk> {
        // Validate severity/occurrence/detection values
        if let Some(s) = input.severity {
            if !(1..=10).contains(&s) {
                return Err(ServiceError::InvalidInput(
                    "Severity must be between 1 and 10".to_string(),
                ));
            }
        }
        if let Some(o) = input.occurrence {
            if !(1..=10).contains(&o) {
                return Err(ServiceError::InvalidInput(
                    "Occurrence must be between 1 and 10".to_string(),
                ));
            }
        }
        if let Some(d) = input.detection {
            if !(1..=10).contains(&d) {
                return Err(ServiceError::InvalidInput(
                    "Detection must be between 1 and 10".to_string(),
                ));
            }
        }

        let id = EntityId::new(EntityPrefix::Risk);

        // Calculate RPN if all values present
        let rpn = match (input.severity, input.occurrence, input.detection) {
            (Some(s), Some(o), Some(d)) => Some(s as u16 * o as u16 * d as u16),
            _ => None,
        };

        // Determine risk level from RPN
        let risk_level = rpn.map(|rpn| match rpn {
            0..=50 => RiskLevel::Low,
            51..=150 => RiskLevel::Medium,
            151..=400 => RiskLevel::High,
            _ => RiskLevel::Critical,
        });

        let risk = Risk {
            id: id.clone(),
            risk_type: input.risk_type,
            title: input.title,
            category: input.category,
            tags: input.tags,
            description: input.description,
            failure_mode: input.failure_mode,
            cause: input.cause,
            effect: input.effect,
            severity: input.severity,
            occurrence: input.occurrence,
            detection: input.detection,
            rpn,
            initial_risk: None,
            mitigations: Vec::new(),
            status: Status::Draft,
            risk_level,
            links: RiskLinks::default(),
            created: Utc::now(),
            author: input.author,
            revision: 1,
        };

        // Write to the type-specific directory (risks/design or
        // risks/process) — the same path the CLI reports to the user.
        let path = self
            .project
            .risk_directory(&risk.risk_type.to_string())
            .join(format!("{}.tdt.yaml", id));
        self.base.save(&risk, &path, Some("RISK"))?;

        Ok(risk)
    }

    /// Update an existing risk
    pub fn update(&self, id: &str, input: UpdateRisk) -> ServiceResult<Risk> {
        let (path, mut risk) = self.find_risk(id)?;

        // Apply updates
        if let Some(title) = input.title {
            risk.title = title;
        }
        if let Some(description) = input.description {
            risk.description = description;
        }
        if let Some(risk_type) = input.risk_type {
            risk.risk_type = risk_type;
        }
        if let Some(status) = input.status {
            risk.status = status;
        }
        if let Some(category) = input.category {
            risk.category = Some(category);
        }
        if let Some(tags) = input.tags {
            risk.tags = tags;
        }
        if let Some(failure_mode) = input.failure_mode {
            risk.failure_mode = Some(failure_mode);
        }
        if let Some(cause) = input.cause {
            risk.cause = Some(cause);
        }
        if let Some(effect) = input.effect {
            risk.effect = Some(effect);
        }

        // Track initial risk if updating S/O/D for the first time with mitigations
        let had_sod =
            risk.severity.is_some() && risk.occurrence.is_some() && risk.detection.is_some();
        let updating_sod =
            input.severity.is_some() || input.occurrence.is_some() || input.detection.is_some();

        if had_sod && updating_sod && !risk.mitigations.is_empty() && risk.initial_risk.is_none() {
            // Store initial risk before mitigation
            risk.initial_risk = Some(InitialRisk {
                severity: risk.severity,
                occurrence: risk.occurrence,
                detection: risk.detection,
                rpn: risk.rpn,
            });
        }

        if let Some(severity) = input.severity {
            if !(1..=10).contains(&severity) {
                return Err(ServiceError::InvalidInput(
                    "Severity must be between 1 and 10".to_string(),
                ));
            }
            risk.severity = Some(severity);
        }
        if let Some(occurrence) = input.occurrence {
            if !(1..=10).contains(&occurrence) {
                return Err(ServiceError::InvalidInput(
                    "Occurrence must be between 1 and 10".to_string(),
                ));
            }
            risk.occurrence = Some(occurrence);
        }
        if let Some(detection) = input.detection {
            if !(1..=10).contains(&detection) {
                return Err(ServiceError::InvalidInput(
                    "Detection must be between 1 and 10".to_string(),
                ));
            }
            risk.detection = Some(detection);
        }

        if let Some(mitigations) = input.mitigations {
            risk.mitigations = mitigations;
        }

        // Recalculate RPN
        risk.rpn = risk.calculate_rpn();

        // Update risk level (use override if provided, otherwise compute)
        risk.risk_level = input.risk_level.or_else(|| risk.determine_risk_level());

        // Increment revision
        risk.revision += 1;

        // Write back
        self.base.save(&risk, &path, None)?;

        Ok(risk)
    }

    /// Delete a risk
    pub fn delete(&self, id: &str, force: bool) -> ServiceResult<()> {
        let (path, risk) = self.find_risk(id)?;

        // Check for references unless force is true
        if !force {
            let references = self.find_references(&risk.id)?;
            if !references.is_empty() {
                return Err(ServiceError::HasReferences);
            }
        }

        // Delete the file
        fs::remove_file(&path)?;

        Ok(())
    }

    /// Add a mitigation to a risk
    pub fn add_mitigation(&self, id: &str, mitigation: Mitigation) -> ServiceResult<Risk> {
        let (path, mut risk) = self.find_risk(id)?;

        // Store initial risk if this is first mitigation and we have S/O/D
        if risk.mitigations.is_empty()
            && risk.initial_risk.is_none()
            && risk.severity.is_some()
            && risk.occurrence.is_some()
            && risk.detection.is_some()
        {
            risk.initial_risk = Some(InitialRisk {
                severity: risk.severity,
                occurrence: risk.occurrence,
                detection: risk.detection,
                rpn: risk.rpn,
            });
        }

        risk.mitigations.push(mitigation);
        risk.revision += 1;

        self.base.save(&risk, &path, None)?;

        Ok(risk)
    }

    /// Update mitigation status
    pub fn update_mitigation_status(
        &self,
        id: &str,
        mitigation_index: usize,
        status: MitigationStatus,
    ) -> ServiceResult<Risk> {
        let (path, mut risk) = self.find_risk(id)?;

        if mitigation_index >= risk.mitigations.len() {
            return Err(ServiceError::InvalidInput(format!(
                "Mitigation index {} out of range (0-{})",
                mitigation_index,
                risk.mitigations.len().saturating_sub(1)
            )));
        }

        risk.mitigations[mitigation_index].status = Some(status);
        risk.revision += 1;

        self.base.save(&risk, &path, None)?;

        Ok(risk)
    }

    /// Find a risk and its file path (cache-first lookup)
    fn find_risk(&self, id: &str) -> ServiceResult<(PathBuf, Risk)> {
        // Try to find in cache first for fast path lookup
        if let Some(cached) = self.cache.get_entity(id) {
            let path = if cached.file_path.is_absolute() {
                cached.file_path.clone()
            } else {
                self.project.root().join(&cached.file_path)
            };
            if path.exists() {
                if let Ok(risk) = crate::yaml::parse_yaml_file::<Risk>(&path) {
                    return Ok((path, risk));
                }
            }
        }

        // Fall back to directory scan
        let dir = self.get_directory();
        if let Some((path, risk)) = loader::load_entity::<Risk>(&dir, id)? {
            return Ok((path, risk));
        }
        Err(ServiceError::NotFound(id.to_string()))
    }

    /// Find entities that reference this risk
    fn find_references(&self, id: &EntityId) -> ServiceResult<Vec<EntityId>> {
        let id_str = id.to_string();
        let links = self.cache.get_links_to(&id_str);
        let mut refs = Vec::new();
        for link in links {
            if let Ok(entity_id) = EntityId::parse(&link.source_id) {
                refs.push(entity_id);
            }
        }
        Ok(refs)
    }

    /// Check if a risk matches the given filter
    fn matches_filter(&self, risk: &Risk, filter: &RiskFilter) -> bool {
        // Type filter
        if let Some(risk_type) = &filter.risk_type {
            if risk.risk_type != *risk_type {
                return false;
            }
        }

        // Risk level filter
        if let Some(levels) = &filter.risk_level {
            let computed_level = risk.get_risk_level();
            if let Some(level) = computed_level {
                if !levels.contains(&level) {
                    return false;
                }
            } else {
                return false; // No level = doesn't match level filter
            }
        }

        // Category filter
        if let Some(category) = &filter.category {
            if risk.category.as_ref().map(|c| c.to_lowercase()) != Some(category.to_lowercase()) {
                return false;
            }
        }

        // Severity filter
        if let Some(min_sev) = filter.min_severity {
            if risk.severity.map(|s| s < min_sev).unwrap_or(true) {
                return false;
            }
        }

        // Occurrence filter
        if let Some(min_occ) = filter.min_occurrence {
            if risk.occurrence.map(|o| o < min_occ).unwrap_or(true) {
                return false;
            }
        }

        // Detection filter
        if let Some(min_det) = filter.min_detection {
            if risk.detection.map(|d| d < min_det).unwrap_or(true) {
                return false;
            }
        }

        // RPN range filter
        let rpn = risk.get_rpn();
        if let Some(min_rpn) = filter.min_rpn {
            if rpn.map(|r| r < min_rpn).unwrap_or(true) {
                return false;
            }
        }
        if let Some(max_rpn) = filter.max_rpn {
            if rpn.map(|r| r > max_rpn).unwrap_or(true) {
                return false;
            }
        }

        // Unmitigated filter - check for real mitigations (non-empty action)
        if filter.unmitigated_only {
            let has_real_mitigations = risk.mitigations.iter().any(|m| !m.action.trim().is_empty());
            if has_real_mitigations {
                return false;
            }
        }

        // Needs mitigation filter (has mitigations but none are completed/verified)
        if filter.needs_mitigation {
            let has_incomplete = risk.mitigations.iter().any(|m| {
                m.status
                    .map(|s| s != MitigationStatus::Completed && s != MitigationStatus::Verified)
                    .unwrap_or(true)
            });
            if !has_incomplete && !risk.mitigations.is_empty() {
                return false;
            }
        }

        // Needs verification filter
        if filter.needs_verification && !risk.links.verified_by.is_empty() {
            return false;
        }

        // Common filters
        if !filter.common.matches_status(&risk.status) {
            return false;
        }
        if !filter.common.matches_author(&risk.author) {
            return false;
        }
        if !filter.common.matches_tags(&risk.tags) {
            return false;
        }
        if !filter
            .common
            .matches_search(&[&risk.title, &risk.description])
        {
            return false;
        }
        if !filter.common.matches_recent(&risk.created) {
            return false;
        }

        true
    }

    /// Sort risks by the given field
    fn sort_risks(&self, risks: &mut [Risk], sort_by: RiskSortField, sort_dir: SortDirection) {
        risks.sort_by(|a, b| {
            let cmp = match sort_by {
                RiskSortField::Id => a.id.to_string().cmp(&b.id.to_string()),
                RiskSortField::Title => a.title.cmp(&b.title),
                RiskSortField::Type => format!("{}", a.risk_type).cmp(&format!("{}", b.risk_type)),
                RiskSortField::Severity => a.severity.cmp(&b.severity),
                RiskSortField::Occurrence => a.occurrence.cmp(&b.occurrence),
                RiskSortField::Detection => a.detection.cmp(&b.detection),
                RiskSortField::Rpn => a.get_rpn().cmp(&b.get_rpn()),
                RiskSortField::RiskLevel => {
                    let level_order = |l: Option<RiskLevel>| match l {
                        Some(RiskLevel::Critical) => 0,
                        Some(RiskLevel::High) => 1,
                        Some(RiskLevel::Medium) => 2,
                        Some(RiskLevel::Low) => 3,
                        None => 4,
                    };
                    level_order(a.get_risk_level()).cmp(&level_order(b.get_risk_level()))
                }
                RiskSortField::Status => format!("{:?}", a.status).cmp(&format!("{:?}", b.status)),
                RiskSortField::Author => a.author.cmp(&b.author),
                RiskSortField::Created => a.created.cmp(&b.created),
                RiskSortField::Category => a.category.cmp(&b.category),
            };

            match sort_dir {
                SortDirection::Ascending => cmp,
                SortDirection::Descending => cmp.reverse(),
            }
        });
    }

    /// Get count of risks matching a filter
    pub fn count(&self, filter: &RiskFilter) -> ServiceResult<usize> {
        let risks = self.load_all()?;
        Ok(risks
            .iter()
            .filter(|risk| self.matches_filter(risk, filter))
            .count())
    }

    /// Get statistics about risks
    pub fn stats(&self) -> ServiceResult<RiskStats> {
        let risks = self.load_all()?;

        let mut stats = RiskStats {
            total: risks.len(),
            ..Default::default()
        };

        for risk in &risks {
            match risk.risk_type {
                RiskType::Design => stats.by_type.design += 1,
                RiskType::Process => stats.by_type.process += 1,
                RiskType::Use => stats.by_type.r#use += 1,
                RiskType::Software => stats.by_type.software += 1,
            }

            if let Some(level) = risk.get_risk_level() {
                match level {
                    RiskLevel::Low => stats.by_level.low += 1,
                    RiskLevel::Medium => stats.by_level.medium += 1,
                    RiskLevel::High => stats.by_level.high += 1,
                    RiskLevel::Critical => stats.by_level.critical += 1,
                }
            }

            match risk.status {
                Status::Draft => stats.by_status.draft += 1,
                Status::Review => stats.by_status.review += 1,
                Status::Approved => stats.by_status.approved += 1,
                Status::Released => stats.by_status.released += 1,
                Status::Obsolete => stats.by_status.obsolete += 1,
            }

            // Check if risk has any mitigations with actual content (not just placeholder entries)
            let has_real_mitigations = risk.mitigations.iter().any(|m| !m.action.trim().is_empty());
            if !has_real_mitigations {
                stats.unmitigated += 1;
            }

            if risk.links.verified_by.is_empty() {
                stats.unverified += 1;
            }

            // RPN statistics
            if let Some(rpn) = risk.get_rpn() {
                stats.rpn_stats.count += 1;
                stats.rpn_stats.sum += rpn as u32;
                if rpn > stats.rpn_stats.max {
                    stats.rpn_stats.max = rpn;
                }
                if stats.rpn_stats.min == 0 || rpn < stats.rpn_stats.min {
                    stats.rpn_stats.min = rpn;
                }
            }
        }

        // Calculate average RPN
        if stats.rpn_stats.count > 0 {
            stats.rpn_stats.avg = stats.rpn_stats.sum as f64 / stats.rpn_stats.count as f64;
        }

        Ok(stats)
    }

    /// Get risk matrix data (for visualization)
    pub fn get_risk_matrix(&self) -> ServiceResult<RiskMatrix> {
        let risks = self.load_all()?;

        // Use a HashMap to collect cells by (severity, occurrence)
        use std::collections::HashMap;
        let mut cell_map: HashMap<(u8, u8), RiskMatrixCell> = HashMap::new();

        for risk in &risks {
            if let (Some(severity), Some(occurrence)) = (risk.severity, risk.occurrence) {
                let key = (severity, occurrence);
                let cell = cell_map.entry(key).or_insert_with(|| RiskMatrixCell {
                    severity,
                    occurrence,
                    count: 0,
                    risk_ids: Vec::new(),
                    risk_level: calculate_risk_level_from_coords(severity, occurrence),
                });
                cell.count += 1;
                cell.risk_ids.push(risk.id.to_string());
            }
        }

        Ok(RiskMatrix {
            cells: cell_map.into_values().collect(),
            max_severity: 10,
            max_occurrence: 10,
        })
    }
}

/// Calculate risk level from severity and occurrence coordinates
fn calculate_risk_level_from_coords(severity: u8, occurrence: u8) -> RiskLevel {
    let product = severity as u16 * occurrence as u16;
    if product >= 64 {
        RiskLevel::Critical
    } else if product >= 36 {
        RiskLevel::High
    } else if product >= 16 {
        RiskLevel::Medium
    } else {
        RiskLevel::Low
    }
}

/// Statistics about risks
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RiskStats {
    pub total: usize,
    pub unmitigated: usize,
    pub unverified: usize,
    pub by_type: RiskTypeCounts,
    pub by_level: RiskLevelCounts,
    pub by_status: StatusCounts,
    pub rpn_stats: RpnStats,
}

/// Counts by risk type
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RiskTypeCounts {
    pub design: usize,
    pub process: usize,
    pub r#use: usize,
    pub software: usize,
}

/// Counts by risk level
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RiskLevelCounts {
    pub low: usize,
    pub medium: usize,
    pub high: usize,
    pub critical: usize,
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

/// RPN statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RpnStats {
    pub count: usize,
    pub min: u16,
    pub max: u16,
    pub sum: u32,
    pub avg: f64,
}

/// Risk matrix for visualization (severity vs occurrence)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskMatrix {
    /// Flat array of cells (only cells with risks are included)
    pub cells: Vec<RiskMatrixCell>,
    /// Maximum severity value (typically 10)
    pub max_severity: u8,
    /// Maximum occurrence value (typically 10)
    pub max_occurrence: u8,
}

impl Default for RiskMatrix {
    fn default() -> Self {
        Self {
            cells: Vec::new(),
            max_severity: 10,
            max_occurrence: 10,
        }
    }
}

/// A cell in the risk matrix
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RiskMatrixCell {
    /// Severity value (1-10)
    pub severity: u8,
    /// Occurrence value (1-10)
    pub occurrence: u8,
    /// Number of risks in this cell
    pub count: usize,
    /// IDs of risks in this cell
    pub risk_ids: Vec<String>,
    /// Aggregated risk level for this cell
    pub risk_level: RiskLevel,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::risk::MitigationType;
    use tempfile::TempDir;

    fn setup_test_project() -> (TempDir, Project, EntityCache) {
        let tmp = TempDir::new().unwrap();

        // Initialize project structure
        fs::create_dir_all(tmp.path().join(".tdt")).unwrap();
        fs::create_dir_all(tmp.path().join("risks")).unwrap();

        // Create config file
        fs::write(tmp.path().join(".tdt/config.yaml"), "author: Test Author\n").unwrap();

        let project = Project::discover_from(tmp.path()).unwrap();
        let cache = EntityCache::open(&project).unwrap();

        (tmp, project, cache)
    }

    #[test]
    fn test_create_risk() {
        let (_tmp, project, cache) = setup_test_project();
        let service = RiskService::new(&project, &cache);

        let input = CreateRisk {
            risk_type: RiskType::Design,
            title: "Battery Overheating".into(),
            description: "Risk of thermal runaway".into(),
            author: "Test Author".into(),
            severity: Some(8),
            occurrence: Some(5),
            detection: Some(4),
            ..Default::default()
        };

        let risk = service.create(input).unwrap();

        assert_eq!(risk.title, "Battery Overheating");
        assert_eq!(risk.risk_type, RiskType::Design);
        assert_eq!(risk.severity, Some(8));
        assert_eq!(risk.rpn, Some(160)); // 8 * 5 * 4
        assert_eq!(risk.risk_level, Some(RiskLevel::High)); // 151-400 = High
    }

    #[test]
    fn test_validation_rejects_invalid_severity() {
        let (_tmp, project, cache) = setup_test_project();
        let service = RiskService::new(&project, &cache);

        let input = CreateRisk {
            risk_type: RiskType::Design,
            title: "Test".into(),
            description: "Test".into(),
            author: "Test".into(),
            severity: Some(11), // Invalid
            ..Default::default()
        };

        let result = service.create(input);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_risk() {
        let (_tmp, project, cache) = setup_test_project();
        let service = RiskService::new(&project, &cache);

        let created = service
            .create(CreateRisk {
                risk_type: RiskType::Process,
                title: "Find Me".into(),
                description: "Can you find this?".into(),
                author: "Test".into(),
                ..Default::default()
            })
            .unwrap();

        let found = service.get(&created.id.to_string()).unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().title, "Find Me");
    }

    #[test]
    fn test_update_risk() {
        let (_tmp, project, cache) = setup_test_project();
        let service = RiskService::new(&project, &cache);

        let created = service
            .create(CreateRisk {
                risk_type: RiskType::Design,
                title: "Original".into(),
                description: "Original description".into(),
                author: "Test".into(),
                severity: Some(5),
                occurrence: Some(5),
                detection: Some(5),
                ..Default::default()
            })
            .unwrap();

        let updated = service
            .update(
                &created.id.to_string(),
                UpdateRisk {
                    title: Some("Updated Title".into()),
                    severity: Some(8),
                    ..Default::default()
                },
            )
            .unwrap();

        assert_eq!(updated.title, "Updated Title");
        assert_eq!(updated.severity, Some(8));
        assert_eq!(updated.rpn, Some(200)); // 8 * 5 * 5
        assert_eq!(updated.revision, 2);
    }

    #[test]
    fn test_delete_risk() {
        let (_tmp, project, cache) = setup_test_project();
        let service = RiskService::new(&project, &cache);

        let created = service
            .create(CreateRisk {
                risk_type: RiskType::Design,
                title: "Delete Me".into(),
                description: "I will be deleted".into(),
                author: "Test".into(),
                ..Default::default()
            })
            .unwrap();

        service.delete(&created.id.to_string(), false).unwrap();

        let found = service.get(&created.id.to_string()).unwrap();
        assert!(found.is_none());
    }

    #[test]
    fn test_add_mitigation() {
        let (_tmp, project, cache) = setup_test_project();
        let service = RiskService::new(&project, &cache);

        let created = service
            .create(CreateRisk {
                risk_type: RiskType::Design,
                title: "Risky".into(),
                description: "Needs mitigation".into(),
                author: "Test".into(),
                severity: Some(7),
                occurrence: Some(6),
                detection: Some(5),
                ..Default::default()
            })
            .unwrap();

        let mitigation = Mitigation {
            action: "Add thermal cutoff".into(),
            mitigation_type: Some(MitigationType::Prevention),
            status: Some(MitigationStatus::Proposed),
            owner: Some("Engineer".into()),
            due_date: None,
        };

        let updated = service
            .add_mitigation(&created.id.to_string(), mitigation)
            .unwrap();

        assert_eq!(updated.mitigations.len(), 1);
        assert_eq!(updated.mitigations[0].action, "Add thermal cutoff");
        // Initial risk should be captured
        assert!(updated.initial_risk.is_some());
    }

    #[test]
    fn test_list_with_filter() {
        let (_tmp, project, cache) = setup_test_project();
        let service = RiskService::new(&project, &cache);

        // Create design risk
        service
            .create(CreateRisk {
                risk_type: RiskType::Design,
                title: "Design Risk".into(),
                description: "A design risk".into(),
                author: "Test".into(),
                severity: Some(8),
                occurrence: Some(5),
                detection: Some(4),
                ..Default::default()
            })
            .unwrap();

        // Create process risk
        service
            .create(CreateRisk {
                risk_type: RiskType::Process,
                title: "Process Risk".into(),
                description: "A process risk".into(),
                author: "Test".into(),
                severity: Some(3),
                occurrence: Some(2),
                detection: Some(2),
                ..Default::default()
            })
            .unwrap();

        // List all
        let all = service
            .list(
                &RiskFilter::default(),
                RiskSortField::Created,
                SortDirection::Ascending,
            )
            .unwrap();
        assert_eq!(all.items.len(), 2);

        // List only design
        let design_only = service
            .list(
                &RiskFilter::design(),
                RiskSortField::Created,
                SortDirection::Ascending,
            )
            .unwrap();
        assert_eq!(design_only.items.len(), 1);
        assert_eq!(design_only.items[0].title, "Design Risk");

        // List high priority
        let high_priority = service
            .list(
                &RiskFilter::high_priority(),
                RiskSortField::Rpn,
                SortDirection::Descending,
            )
            .unwrap();
        assert_eq!(high_priority.items.len(), 1);
        assert_eq!(high_priority.items[0].title, "Design Risk");
    }

    #[test]
    fn test_stats() {
        let (_tmp, project, cache) = setup_test_project();
        let service = RiskService::new(&project, &cache);

        service
            .create(CreateRisk {
                risk_type: RiskType::Design,
                title: "Risk 1".into(),
                description: "First risk".into(),
                author: "Test".into(),
                severity: Some(8),
                occurrence: Some(5),
                detection: Some(4),
                ..Default::default()
            })
            .unwrap();

        service
            .create(CreateRisk {
                risk_type: RiskType::Process,
                title: "Risk 2".into(),
                description: "Second risk".into(),
                author: "Test".into(),
                severity: Some(3),
                occurrence: Some(2),
                detection: Some(2),
                ..Default::default()
            })
            .unwrap();

        let stats = service.stats().unwrap();
        assert_eq!(stats.total, 2);
        assert_eq!(stats.by_type.design, 1);
        assert_eq!(stats.by_type.process, 1);
        assert_eq!(stats.by_level.high, 1); // RPN 160
        assert_eq!(stats.by_level.low, 1); // RPN 12
        assert_eq!(stats.rpn_stats.count, 2);
        assert_eq!(stats.rpn_stats.min, 12);
        assert_eq!(stats.rpn_stats.max, 160);
    }

    #[test]
    fn test_risk_matrix() {
        let (_tmp, project, cache) = setup_test_project();
        let service = RiskService::new(&project, &cache);

        service
            .create(CreateRisk {
                risk_type: RiskType::Design,
                title: "Risk 1".into(),
                description: "First risk".into(),
                author: "Test".into(),
                severity: Some(8),
                occurrence: Some(5),
                detection: Some(4),
                ..Default::default()
            })
            .unwrap();

        service
            .create(CreateRisk {
                risk_type: RiskType::Design,
                title: "Risk 2".into(),
                description: "Second risk at same position".into(),
                author: "Test".into(),
                severity: Some(8),
                occurrence: Some(5),
                detection: Some(3),
                ..Default::default()
            })
            .unwrap();

        let matrix = service.get_risk_matrix().unwrap();
        // Find cell with severity=8, occurrence=5
        let cell = matrix
            .cells
            .iter()
            .find(|c| c.severity == 8 && c.occurrence == 5)
            .expect("Should have cell at severity=8, occurrence=5");
        assert_eq!(cell.count, 2);
        assert_eq!(cell.risk_ids.len(), 2);
    }
}
