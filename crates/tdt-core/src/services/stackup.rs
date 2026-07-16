//! Stackup service for tolerance analysis management
//!
//! Provides CRUD operations and analysis methods for tolerance stackups.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::core::cache::{CachedEntity, EntityCache, EntityFilter};
use crate::core::entity::{Entity, Status};
use crate::core::identity::{EntityId, EntityPrefix};
use crate::core::loader;
use crate::core::project::Project;
use crate::entities::stackup::{
    AnalysisResult, Contributor, Direction, Disposition, Distribution, FeatureRef,
    MonteCarloResult, RssResult, Stackup, WorstCaseResult,
};
use crate::services::base::ServiceBase;
use crate::services::common::{
    CommonFilter, NoneLast, ServiceError, ServiceResult, SortDirection, SortKey, Sortable,
};

/// Reject sigma levels that would produce Inf/NaN analysis output.
fn validate_sigma_level(sigma: f64) -> ServiceResult<()> {
    if !sigma.is_finite() || sigma <= 0.0 {
        return Err(ServiceError::ValidationFailed(format!(
            "sigma_level must be a positive number (got {sigma})"
        )));
    }
    Ok(())
}

/// Reject mean-shift factors that would produce nonsensical analysis output.
fn validate_mean_shift_k(k: f64) -> ServiceResult<()> {
    if !k.is_finite() || k < 0.0 {
        return Err(ServiceError::ValidationFailed(format!(
            "mean_shift_k must be a non-negative number (got {k})"
        )));
    }
    Ok(())
}

/// Filter options for listing stackups
#[derive(Debug, Clone, Default)]
pub struct StackupFilter {
    /// Common filters (status, author, tags, search)
    pub common: CommonFilter,
    /// Filter by disposition
    pub disposition: Option<Disposition>,
    /// Filter by worst-case result
    pub result: Option<AnalysisResult>,
    /// Show only critical stackups
    pub critical_only: bool,
    /// Show recent stackups (last N days)
    pub recent_days: Option<u32>,
    /// Sort field
    pub sort: StackupSortField,
    /// Sort direction
    pub sort_direction: SortDirection,
}

/// Fields available for sorting stackups
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StackupSortField {
    Id,
    Title,
    Result,
    Cpk,
    Yield,
    Disposition,
    Status,
    Author,
    #[default]
    Created,
}

impl Sortable for Stackup {
    type SortField = StackupSortField;

    fn sort_key(&self, field: &Self::SortField) -> SortKey {
        match field {
            StackupSortField::Id => SortKey::String(self.id.to_string()),
            StackupSortField::Title => SortKey::String(self.title.clone()),
            StackupSortField::Result => {
                let result_str = self
                    .analysis_results
                    .worst_case
                    .as_ref()
                    .map(|wc| wc.result.to_string())
                    .unwrap_or_default();
                SortKey::String(result_str)
            }
            StackupSortField::Cpk => {
                let cpk = self.analysis_results.rss.as_ref().map(|r| r.cpk);
                SortKey::OptionalNumber(NoneLast(cpk.map(|c| (c * 1000.0) as i64)))
            }
            StackupSortField::Yield => {
                let yield_pct = self.analysis_results.rss.as_ref().map(|r| r.yield_percent);
                SortKey::OptionalNumber(NoneLast(yield_pct.map(|y| (y * 100.0) as i64)))
            }
            StackupSortField::Disposition => SortKey::String(self.disposition.to_string()),
            StackupSortField::Status => SortKey::String(self.status().to_string()),
            StackupSortField::Author => SortKey::String(self.author.clone()),
            StackupSortField::Created => SortKey::DateTime(self.created.timestamp()),
        }
    }
}

/// Input for creating a new stackup
#[derive(Debug, Clone)]
pub struct CreateStackup {
    /// Stackup title
    pub title: String,
    /// Target name
    pub target_name: String,
    /// Target nominal value
    pub target_nominal: f64,
    /// Target upper specification limit
    pub target_upper: f64,
    /// Target lower specification limit
    pub target_lower: f64,
    /// Units (default: mm)
    pub units: Option<String>,
    /// Is this a critical dimension?
    pub critical: bool,
    /// Description
    pub description: Option<String>,
    /// Sigma level (default: 6.0)
    pub sigma_level: Option<f64>,
    /// Mean shift factor (default: 0.0)
    pub mean_shift_k: Option<f64>,
    /// Include GD&T in analysis
    pub include_gdt: bool,
    /// Tags
    pub tags: Vec<String>,
    /// Author
    pub author: String,
}

/// Input for updating an existing stackup
#[derive(Debug, Clone, Default)]
pub struct UpdateStackup {
    /// Update title
    pub title: Option<String>,
    /// Update description
    pub description: Option<Option<String>>,
    /// Update target name
    pub target_name: Option<String>,
    /// Update target nominal
    pub target_nominal: Option<f64>,
    /// Update target upper limit
    pub target_upper: Option<f64>,
    /// Update target lower limit
    pub target_lower: Option<f64>,
    /// Update critical flag
    pub critical: Option<bool>,
    /// Update sigma level
    pub sigma_level: Option<f64>,
    /// Update mean shift factor
    pub mean_shift_k: Option<f64>,
    /// Update include_gdt flag
    pub include_gdt: Option<bool>,
    /// Update disposition
    pub disposition: Option<Disposition>,
    /// Update document status
    pub status: Option<Status>,
}

/// Statistics about stackups
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StackupStats {
    /// Total number of stackups
    pub total: usize,
    /// Counts by disposition
    pub by_disposition: DispositionCounts,
    /// Counts by worst-case result
    pub by_result: ResultCounts,
    /// Number of critical stackups
    pub critical_count: usize,
    /// Number with analysis run
    pub analyzed_count: usize,
    /// Average Cpk (for analyzed stackups)
    pub avg_cpk: Option<f64>,
    /// Average yield (for analyzed stackups)
    pub avg_yield: Option<f64>,
}

/// Disposition counts
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DispositionCounts {
    pub under_review: usize,
    pub approved: usize,
    pub rejected: usize,
}

/// Result counts (worst-case analysis)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResultCounts {
    pub pass: usize,
    pub marginal: usize,
    pub fail: usize,
    pub not_analyzed: usize,
}

/// Input for adding a contributor
#[derive(Debug, Clone)]
pub struct AddContributorInput {
    /// Contributor name
    pub name: String,
    /// Direction in stackup
    pub direction: Direction,
    /// Nominal value
    pub nominal: f64,
    /// Plus tolerance
    pub plus_tol: f64,
    /// Minus tolerance
    pub minus_tol: f64,
    /// Statistical distribution
    pub distribution: Option<Distribution>,
    /// Source reference
    pub source: Option<String>,
    /// Linked feature ID
    pub feature_id: Option<EntityId>,
}

/// Service for managing stackups
pub struct StackupService<'a> {
    project: &'a Project,
    base: ServiceBase<'a>,
    cache: &'a EntityCache,
}

impl<'a> StackupService<'a> {
    /// Create a new StackupService
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

    /// Get the stackups directory
    fn stackup_dir(&self) -> PathBuf {
        self.project().root().join("tolerances/stackups")
    }

    /// Get the file path for a stackup
    fn get_file_path(&self, id: &EntityId) -> PathBuf {
        self.stackup_dir().join(format!("{}.tdt.yaml", id))
    }

    /// Load all stackups
    fn load_all(&self) -> ServiceResult<Vec<Stackup>> {
        let dir = self.stackup_dir();
        if !dir.exists() {
            return Ok(Vec::new());
        }
        loader::load_all(&dir).map_err(ServiceError::from)
    }

    /// List stackups using the cache (fast path)
    ///
    /// Returns cached entity data without loading full entities from disk.
    /// Use this for list views and simple queries.
    pub fn list_cached(&self, filter: &StackupFilter) -> ServiceResult<Vec<CachedEntity>> {
        // Build cache filter
        let status = filter
            .common
            .status
            .as_ref()
            .and_then(|s| s.first())
            .copied();

        let entity_filter = EntityFilter {
            prefix: Some(EntityPrefix::Tol),
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

        // Note: disposition, result, critical_only require full entity load
        // These are handled in the regular list() method

        // Apply limit
        if let Some(limit) = filter.common.limit {
            cached.truncate(limit);
        }

        Ok(cached)
    }

    /// Find a stackup by ID
    ///
    /// Uses the cache to find the file path for faster lookup.
    fn find_stackup(&self, id: &str) -> ServiceResult<(PathBuf, Stackup)> {
        // Try to find in cache first for fast path lookup
        if let Some(cached) = self.cache.get_entity(id) {
            let path = if cached.file_path.is_absolute() {
                cached.file_path.clone()
            } else {
                self.project.root().join(&cached.file_path)
            };
            if path.exists() {
                if let Ok(stackup) = crate::yaml::parse_yaml_file::<Stackup>(&path) {
                    return Ok((path, stackup));
                }
            }
        }

        // Fall back to directory scan
        let dir = self.stackup_dir();
        if let Some((path, stackup)) = loader::load_entity::<Stackup>(&dir, id)? {
            return Ok((path, stackup));
        }
        Err(ServiceError::NotFound(format!("Stackup: {}", id)))
    }

    /// List stackups with filtering and sorting
    pub fn list(&self, filter: &StackupFilter) -> ServiceResult<Vec<Stackup>> {
        let mut stackups = self.load_all()?;

        // Apply filters
        stackups.retain(|stackup| {
            // Common filter
            if !filter.common.matches_status_str(stackup.status()) {
                return false;
            }
            if !filter.common.matches_author(&stackup.author) {
                return false;
            }
            if !filter.common.matches_search(&[&stackup.title]) {
                return false;
            }

            // Disposition filter
            if let Some(ref disp) = filter.disposition {
                if &stackup.disposition != disp {
                    return false;
                }
            }

            // Result filter
            if let Some(ref result) = filter.result {
                if let Some(ref wc) = stackup.analysis_results.worst_case {
                    if &wc.result != result {
                        return false;
                    }
                } else {
                    return false; // No analysis, can't match result
                }
            }

            // Critical filter
            if filter.critical_only && !stackup.target.critical {
                return false;
            }

            // Recent filter
            if let Some(days) = filter.recent_days {
                let cutoff = Utc::now() - chrono::Duration::days(days as i64);
                if stackup.created < cutoff {
                    return false;
                }
            }

            true
        });

        // Sort
        crate::services::common::sort_entities(&mut stackups, filter.sort, filter.sort_direction);

        Ok(stackups)
    }

    /// Get a stackup by ID
    pub fn get(&self, id: &str) -> ServiceResult<Option<Stackup>> {
        match self.find_stackup(id) {
            Ok((_, stackup)) => Ok(Some(stackup)),
            Err(ServiceError::NotFound(_)) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Get a stackup by ID, returning error if not found
    pub fn get_required(&self, id: &str) -> ServiceResult<Stackup> {
        let (_, stackup) = self.find_stackup(id)?;
        Ok(stackup)
    }

    /// Create a new stackup
    pub fn create(&self, input: CreateStackup) -> ServiceResult<Stackup> {
        let id = EntityId::new(EntityPrefix::Tol);

        let mut stackup = Stackup::new(
            input.title,
            input.target_name,
            input.target_nominal,
            input.target_upper,
            input.target_lower,
            input.author,
        );
        stackup.id = id;
        stackup.description = input.description;
        stackup.target.critical = input.critical;

        if let Some(units) = input.units {
            stackup.target.units = units;
        }
        if let Some(sigma) = input.sigma_level {
            validate_sigma_level(sigma)?;
            stackup.sigma_level = sigma;
        }
        if let Some(k) = input.mean_shift_k {
            validate_mean_shift_k(k)?;
            stackup.mean_shift_k = k;
        }
        stackup.include_gdt = input.include_gdt;
        stackup.tags = input.tags;

        // Ensure directory exists
        let dir = self.stackup_dir();
        if !dir.exists() {
            fs::create_dir_all(&dir)?;
        }

        // Save
        let file_path = self.get_file_path(&stackup.id);
        self.base.save(&stackup, &file_path, Some("TOL"))?;

        Ok(stackup)
    }

    /// Update an existing stackup
    pub fn update(&self, id: &str, input: UpdateStackup) -> ServiceResult<Stackup> {
        let (_, mut stackup) = self.find_stackup(id)?;

        if let Some(title) = input.title {
            stackup.title = title;
        }
        if let Some(description) = input.description {
            stackup.description = description;
        }
        if let Some(target_name) = input.target_name {
            stackup.target.name = target_name;
        }
        if let Some(target_nominal) = input.target_nominal {
            stackup.target.nominal = target_nominal;
        }
        if let Some(target_upper) = input.target_upper {
            stackup.target.upper_limit = target_upper;
        }
        if let Some(target_lower) = input.target_lower {
            stackup.target.lower_limit = target_lower;
        }
        if let Some(critical) = input.critical {
            stackup.target.critical = critical;
        }
        if let Some(sigma) = input.sigma_level {
            validate_sigma_level(sigma)?;
            stackup.sigma_level = sigma;
        }
        if let Some(k) = input.mean_shift_k {
            validate_mean_shift_k(k)?;
            stackup.mean_shift_k = k;
        }
        if let Some(include_gdt) = input.include_gdt {
            stackup.include_gdt = include_gdt;
        }
        if let Some(disposition) = input.disposition {
            stackup.disposition = disposition;
        }
        if let Some(status) = input.status {
            stackup.status = status;
        }

        // Increment revision
        stackup.entity_revision += 1;

        // Save
        let file_path = self.get_file_path(&stackup.id);
        self.base.save(&stackup, &file_path, None)?;

        Ok(stackup)
    }

    /// Delete a stackup
    pub fn delete(&self, id: &str, force: bool) -> ServiceResult<()> {
        let (path, _) = self.find_stackup(id)?;

        if !force {
            // Could check for references here
        }

        fs::remove_file(&path)?;
        Ok(())
    }

    /// Add a contributor to a stackup. If `input.feature_id` is set, the
    /// referenced feature must exist on disk — otherwise this errors with
    /// `ValidationFailed`. This prevents stackups from being silently saved
    /// with dangling references that would later produce wrong analyses.
    pub fn add_contributor(&self, id: &str, input: AddContributorInput) -> ServiceResult<Stackup> {
        let (_, mut stackup) = self.find_stackup(id)?;

        let mut feature_distribution = None;
        let feature_ref = if let Some(feat_id) = input.feature_id {
            let feature = self.load_feature(&feat_id.to_string())?.ok_or_else(|| {
                ServiceError::ValidationFailed(format!(
                    "feature {} does not exist; cannot add stackup contributor referencing it",
                    feat_id
                ))
            })?;
            // Default the distribution from the feature's dimension (matching
            // the CLI `tol add` behavior): prefer the dimension whose values
            // match the input, else the feature's first dimension.
            feature_distribution = feature
                .dimensions
                .iter()
                .find(|d| {
                    d.nominal == input.nominal
                        && d.plus_tol == input.plus_tol
                        && d.minus_tol == input.minus_tol
                })
                .or_else(|| feature.dimensions.first())
                .map(|d| d.distribution);
            // Populate cached metadata so views and `validate` checks have it.
            Some(FeatureRef::with_cache(
                feat_id,
                Some(feature.title.clone()),
                Some(feature.component.clone()),
                None,
            ))
        } else {
            None
        };

        let contributor = Contributor {
            name: input.name,
            feature: feature_ref,
            direction: input.direction,
            nominal: input.nominal,
            plus_tol: input.plus_tol,
            minus_tol: input.minus_tol,
            distribution: input
                .distribution
                .or(feature_distribution)
                .unwrap_or_default(),
            source: input.source,
            gdt_position: None,
        };

        stackup.contributors.push(contributor);
        stackup.entity_revision += 1;

        let file_path = self.get_file_path(&stackup.id);
        self.base.save(&stackup, &file_path, None)?;

        Ok(stackup)
    }

    /// Load a feature by ID from the features directory. Returns `Ok(None)`
    /// if the feature does not exist (rather than an error) so callers can
    /// distinguish "missing" from "I/O failure".
    fn load_feature(&self, id: &str) -> ServiceResult<Option<crate::entities::feature::Feature>> {
        let feature_dir = self.project.root().join("tolerances/features");
        if !feature_dir.exists() {
            return Ok(None);
        }
        let result =
            loader::load_entity::<crate::entities::feature::Feature>(&feature_dir, id)
                .map_err(ServiceError::from)?;
        Ok(result.map(|(_, f)| f))
    }

    /// Remove a contributor from a stackup by index
    pub fn remove_contributor(&self, id: &str, index: usize) -> ServiceResult<Stackup> {
        let (_, mut stackup) = self.find_stackup(id)?;

        if index >= stackup.contributors.len() {
            return Err(ServiceError::ValidationFailed(format!(
                "Contributor index {} out of range (stackup has {} contributors)",
                index,
                stackup.contributors.len()
            )));
        }

        stackup.contributors.remove(index);
        stackup.entity_revision += 1;

        let file_path = self.get_file_path(&stackup.id);
        self.base.save(&stackup, &file_path, None)?;

        Ok(stackup)
    }

    /// Remove a contributor by feature ID
    pub fn remove_contributor_by_feature(
        &self,
        id: &str,
        feature_id: &str,
    ) -> ServiceResult<Stackup> {
        let (_, mut stackup) = self.find_stackup(id)?;

        let original_len = stackup.contributors.len();
        stackup.contributors.retain(|c| {
            c.feature
                .as_ref()
                .map(|f| f.id.to_string() != feature_id)
                .unwrap_or(true)
        });

        if stackup.contributors.len() == original_len {
            return Err(ServiceError::NotFound(format!(
                "No contributor with feature ID: {}",
                feature_id
            )));
        }

        stackup.entity_revision += 1;

        let file_path = self.get_file_path(&stackup.id);
        self.base.save(&stackup, &file_path, None)?;

        Ok(stackup)
    }

    /// Run all analyses on a stackup
    pub fn analyze(&self, id: &str, monte_carlo_iterations: Option<u32>) -> ServiceResult<Stackup> {
        let (_, mut stackup) = self.find_stackup(id)?;

        // Calculate all analysis types
        stackup.analysis_results.worst_case = Some(
            stackup
                .calculate_worst_case()
                .map_err(|e| ServiceError::ValidationFailed(e.to_string()))?,
        );
        stackup.analysis_results.rss = Some(
            stackup
                .calculate_rss()
                .map_err(|e| ServiceError::ValidationFailed(e.to_string()))?,
        );

        let iterations = monte_carlo_iterations.unwrap_or(10000);
        stackup.analysis_results.monte_carlo = Some(
            stackup
                .calculate_monte_carlo(iterations)
                .map_err(|e| ServiceError::ValidationFailed(e.to_string()))?,
        );
        stackup.analysis_results.analyzed_at = Some(Utc::now());
        stackup.analysis_results.input_hash = Some(stackup.contributor_input_hash());

        stackup.entity_revision += 1;

        let file_path = self.get_file_path(&stackup.id);
        self.base.save(&stackup, &file_path, None)?;

        Ok(stackup)
    }

    /// Calculate worst-case analysis only
    pub fn calculate_worst_case(&self, id: &str) -> ServiceResult<WorstCaseResult> {
        let (_, stackup) = self.find_stackup(id)?;
        stackup
            .calculate_worst_case()
            .map_err(|e| ServiceError::ValidationFailed(e.to_string()))
    }

    /// Calculate RSS analysis only
    pub fn calculate_rss(&self, id: &str) -> ServiceResult<RssResult> {
        let (_, stackup) = self.find_stackup(id)?;
        stackup
            .calculate_rss()
            .map_err(|e| ServiceError::ValidationFailed(e.to_string()))
    }

    /// Calculate Monte Carlo analysis only
    pub fn calculate_monte_carlo(
        &self,
        id: &str,
        iterations: u32,
    ) -> ServiceResult<MonteCarloResult> {
        let (_, stackup) = self.find_stackup(id)?;
        stackup
            .calculate_monte_carlo(iterations)
            .map_err(|e| ServiceError::ValidationFailed(e.to_string()))
    }

    /// Calculate Monte Carlo analysis with a specific seed (reproducible).
    pub fn calculate_monte_carlo_seeded(
        &self,
        id: &str,
        iterations: u32,
        seed: u64,
    ) -> ServiceResult<MonteCarloResult> {
        let (_, stackup) = self.find_stackup(id)?;
        stackup
            .calculate_monte_carlo_with_seed(iterations, seed)
            .map_err(|e| ServiceError::ValidationFailed(e.to_string()))
    }

    /// Set disposition for a stackup
    pub fn set_disposition(&self, id: &str, disposition: Disposition) -> ServiceResult<Stackup> {
        self.update(
            id,
            UpdateStackup {
                disposition: Some(disposition),
                ..Default::default()
            },
        )
    }

    /// Add a verifies link to a requirement
    pub fn add_verifies_link(&self, id: &str, requirement_id: &str) -> ServiceResult<Stackup> {
        let (_, mut stackup) = self.find_stackup(id)?;

        if !stackup.links.verifies.contains(&requirement_id.to_string()) {
            stackup.links.verifies.push(requirement_id.to_string());
        }
        stackup.entity_revision += 1;

        let file_path = self.get_file_path(&stackup.id);
        self.base.save(&stackup, &file_path, None)?;

        Ok(stackup)
    }

    /// Remove a verifies link
    pub fn remove_verifies_link(&self, id: &str, requirement_id: &str) -> ServiceResult<Stackup> {
        let (_, mut stackup) = self.find_stackup(id)?;

        stackup.links.verifies.retain(|r| r != requirement_id);
        stackup.entity_revision += 1;

        let file_path = self.get_file_path(&stackup.id);
        self.base.save(&stackup, &file_path, None)?;

        Ok(stackup)
    }

    /// Get statistics about stackups
    pub fn stats(&self) -> ServiceResult<StackupStats> {
        let stackups = self.load_all()?;

        let mut stats = StackupStats {
            total: stackups.len(),
            ..Default::default()
        };

        let mut cpk_sum = 0.0;
        let mut cpk_count = 0;
        let mut yield_sum = 0.0;
        let mut yield_count = 0;

        for stackup in &stackups {
            // Count by disposition
            match stackup.disposition {
                Disposition::UnderReview => stats.by_disposition.under_review += 1,
                Disposition::Approved => stats.by_disposition.approved += 1,
                Disposition::Rejected => stats.by_disposition.rejected += 1,
            }

            // Count by result
            if let Some(ref wc) = stackup.analysis_results.worst_case {
                match wc.result {
                    AnalysisResult::Pass => stats.by_result.pass += 1,
                    AnalysisResult::Marginal => stats.by_result.marginal += 1,
                    AnalysisResult::Fail => stats.by_result.fail += 1,
                }
                stats.analyzed_count += 1;
            } else {
                stats.by_result.not_analyzed += 1;
            }

            // Critical count
            if stackup.target.critical {
                stats.critical_count += 1;
            }

            // Cpk and yield averages
            if let Some(ref rss) = stackup.analysis_results.rss {
                if rss.cpk.is_finite() {
                    cpk_sum += rss.cpk;
                    cpk_count += 1;
                }
                if rss.yield_percent.is_finite() {
                    yield_sum += rss.yield_percent;
                    yield_count += 1;
                }
            }
        }

        if cpk_count > 0 {
            stats.avg_cpk = Some(cpk_sum / cpk_count as f64);
        }
        if yield_count > 0 {
            stats.avg_yield = Some(yield_sum / yield_count as f64);
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

    fn make_create_stackup(title: &str) -> CreateStackup {
        CreateStackup {
            title: title.to_string(),
            target_name: "Gap".to_string(),
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
            author: "Test Author".to_string(),
        }
    }

    #[test]
    fn test_create_stackup() {
        let (_tmp, project, cache) = setup();
        let service = StackupService::new(&project, &cache);

        let input = make_create_stackup("Test Stackup");
        let stackup = service.create(input).unwrap();

        assert!(stackup.id.to_string().starts_with("TOL-"));
        assert_eq!(stackup.title, "Test Stackup");
        assert_eq!(stackup.target.nominal, 1.0);
        assert_eq!(stackup.target.upper_limit, 1.5);
        assert_eq!(stackup.target.lower_limit, 0.5);
    }

    #[test]
    fn test_get_stackup() {
        let (_tmp, project, cache) = setup();
        let service = StackupService::new(&project, &cache);

        let input = make_create_stackup("Test Stackup");
        let created = service.create(input).unwrap();

        let retrieved = service.get(&created.id.to_string()).unwrap().unwrap();
        assert_eq!(retrieved.id, created.id);
        assert_eq!(retrieved.title, "Test Stackup");
    }

    #[test]
    fn test_list_with_filter() {
        let (_tmp, project, cache) = setup();
        let service = StackupService::new(&project, &cache);

        // Create multiple stackups
        service.create(make_create_stackup("Stackup A")).unwrap();
        let stackup_b = service.create(make_create_stackup("Stackup B")).unwrap();
        service.create(make_create_stackup("Stackup C")).unwrap();

        // Approve one
        service
            .set_disposition(&stackup_b.id.to_string(), Disposition::Approved)
            .unwrap();

        // List all
        let all = service.list(&StackupFilter::default()).unwrap();
        assert_eq!(all.len(), 3);

        // List approved only
        let approved = service
            .list(&StackupFilter {
                disposition: Some(Disposition::Approved),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(approved.len(), 1);
        assert_eq!(approved[0].id, stackup_b.id);
    }

    #[test]
    fn test_update_stackup() {
        let (_tmp, project, cache) = setup();
        let service = StackupService::new(&project, &cache);

        let input = make_create_stackup("Test Stackup");
        let created = service.create(input).unwrap();

        let updated = service
            .update(
                &created.id.to_string(),
                UpdateStackup {
                    title: Some("Updated Stackup".to_string()),
                    critical: Some(true),
                    ..Default::default()
                },
            )
            .unwrap();

        assert_eq!(updated.title, "Updated Stackup");
        assert!(updated.target.critical);
        assert_eq!(updated.entity_revision, 2);
    }

    #[test]
    fn test_delete_stackup() {
        let (_tmp, project, cache) = setup();
        let service = StackupService::new(&project, &cache);

        let input = make_create_stackup("Test Stackup");
        let created = service.create(input).unwrap();

        service.delete(&created.id.to_string(), false).unwrap();

        let result = service.get(&created.id.to_string()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_add_contributor() {
        let (_tmp, project, cache) = setup();
        let service = StackupService::new(&project, &cache);

        let input = make_create_stackup("Test Stackup");
        let stackup = service.create(input).unwrap();

        let stackup = service
            .add_contributor(
                &stackup.id.to_string(),
                AddContributorInput {
                    name: "Part A".to_string(),
                    direction: Direction::Positive,
                    nominal: 10.0,
                    plus_tol: 0.1,
                    minus_tol: 0.1,
                    distribution: None,
                    source: None,
                    feature_id: None,
                },
            )
            .unwrap();

        assert_eq!(stackup.contributors.len(), 1);
        assert_eq!(stackup.contributors[0].name, "Part A");
        assert_eq!(stackup.contributors[0].nominal, 10.0);
    }

    #[test]
    fn test_remove_contributor() {
        let (_tmp, project, cache) = setup();
        let service = StackupService::new(&project, &cache);

        let input = make_create_stackup("Test Stackup");
        let stackup = service.create(input).unwrap();

        // Add two contributors
        let stackup = service
            .add_contributor(
                &stackup.id.to_string(),
                AddContributorInput {
                    name: "Part A".to_string(),
                    direction: Direction::Positive,
                    nominal: 10.0,
                    plus_tol: 0.1,
                    minus_tol: 0.1,
                    distribution: None,
                    source: None,
                    feature_id: None,
                },
            )
            .unwrap();

        let stackup = service
            .add_contributor(
                &stackup.id.to_string(),
                AddContributorInput {
                    name: "Part B".to_string(),
                    direction: Direction::Negative,
                    nominal: 9.0,
                    plus_tol: 0.1,
                    minus_tol: 0.1,
                    distribution: None,
                    source: None,
                    feature_id: None,
                },
            )
            .unwrap();

        assert_eq!(stackup.contributors.len(), 2);

        // Remove first contributor
        let stackup = service
            .remove_contributor(&stackup.id.to_string(), 0)
            .unwrap();
        assert_eq!(stackup.contributors.len(), 1);
        assert_eq!(stackup.contributors[0].name, "Part B");
    }

    #[test]
    fn test_analyze() {
        let (_tmp, project, cache) = setup();
        let service = StackupService::new(&project, &cache);

        let input = make_create_stackup("Test Stackup");
        let stackup = service.create(input).unwrap();

        // Add contributors
        let stackup = service
            .add_contributor(
                &stackup.id.to_string(),
                AddContributorInput {
                    name: "Part A".to_string(),
                    direction: Direction::Positive,
                    nominal: 10.0,
                    plus_tol: 0.1,
                    minus_tol: 0.1,
                    distribution: None,
                    source: None,
                    feature_id: None,
                },
            )
            .unwrap();

        let stackup = service
            .add_contributor(
                &stackup.id.to_string(),
                AddContributorInput {
                    name: "Part B".to_string(),
                    direction: Direction::Negative,
                    nominal: 9.0,
                    plus_tol: 0.1,
                    minus_tol: 0.1,
                    distribution: None,
                    source: None,
                    feature_id: None,
                },
            )
            .unwrap();

        // Run analysis
        let stackup = service
            .analyze(&stackup.id.to_string(), Some(1000))
            .unwrap();

        // Check all analysis types are present
        assert!(stackup.analysis_results.worst_case.is_some());
        assert!(stackup.analysis_results.rss.is_some());
        assert!(stackup.analysis_results.monte_carlo.is_some());

        // Worst case: min = (10-0.1) - (9+0.1) = 0.8, max = (10+0.1) - (9-0.1) = 1.2
        let wc = stackup.analysis_results.worst_case.unwrap();
        assert!((wc.min - 0.8).abs() < 0.01);
        assert!((wc.max - 1.2).abs() < 0.01);
        assert_eq!(wc.result, AnalysisResult::Pass);

        // RSS mean should be 10 - 9 = 1.0
        let rss = stackup.analysis_results.rss.unwrap();
        assert!((rss.mean - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_verifies_links() {
        let (_tmp, project, cache) = setup();
        let service = StackupService::new(&project, &cache);

        let input = make_create_stackup("Test Stackup");
        let stackup = service.create(input).unwrap();

        let stackup = service
            .add_verifies_link(&stackup.id.to_string(), "REQ-001")
            .unwrap();
        assert!(stackup.links.verifies.contains(&"REQ-001".to_string()));

        let stackup = service
            .remove_verifies_link(&stackup.id.to_string(), "REQ-001")
            .unwrap();
        assert!(!stackup.links.verifies.contains(&"REQ-001".to_string()));
    }

    #[test]
    fn test_stats() {
        let (_tmp, project, cache) = setup();
        let service = StackupService::new(&project, &cache);

        // Create stackups
        let mut input = make_create_stackup("Stackup A");
        input.critical = true;
        let stackup_a = service.create(input).unwrap();

        let stackup_b = service.create(make_create_stackup("Stackup B")).unwrap();
        service.create(make_create_stackup("Stackup C")).unwrap();

        // Analyze one
        service
            .add_contributor(
                &stackup_a.id.to_string(),
                AddContributorInput {
                    name: "Part".to_string(),
                    direction: Direction::Positive,
                    nominal: 1.0,
                    plus_tol: 0.1,
                    minus_tol: 0.1,
                    distribution: None,
                    source: None,
                    feature_id: None,
                },
            )
            .unwrap();
        service
            .analyze(&stackup_a.id.to_string(), Some(100))
            .unwrap();

        // Approve one
        service
            .set_disposition(&stackup_b.id.to_string(), Disposition::Approved)
            .unwrap();

        let stats = service.stats().unwrap();
        assert_eq!(stats.total, 3);
        assert_eq!(stats.critical_count, 1);
        assert_eq!(stats.by_disposition.approved, 1);
        assert_eq!(stats.by_disposition.under_review, 2);
        assert_eq!(stats.analyzed_count, 1);
        assert_eq!(stats.by_result.not_analyzed, 2);
    }

    #[test]
    fn test_add_contributor_with_unknown_feature_errors() {
        let (_tmp, project, cache) = setup();
        let service = StackupService::new(&project, &cache);

        let stackup = service.create(make_create_stackup("S")).unwrap();
        let phantom_id = EntityId::new(EntityPrefix::Feat);

        let err = service
            .add_contributor(
                &stackup.id.to_string(),
                AddContributorInput {
                    name: "Bad".into(),
                    direction: Direction::Positive,
                    nominal: 1.0,
                    plus_tol: 0.1,
                    minus_tol: 0.1,
                    distribution: None,
                    source: None,
                    feature_id: Some(phantom_id.clone()),
                },
            )
            .unwrap_err();

        match err {
            ServiceError::ValidationFailed(msg) => {
                assert!(msg.contains(&phantom_id.to_string()));
                assert!(msg.contains("does not exist"));
            }
            other => panic!("expected ValidationFailed, got {:?}", other),
        }

        // The stackup should still have zero contributors — the bad add must
        // not have been persisted.
        let after = service.get(&stackup.id.to_string()).unwrap().unwrap();
        assert_eq!(after.contributors.len(), 0);
    }

    #[test]
    fn test_add_contributor_with_existing_feature_succeeds_and_caches_metadata() {
        use crate::entities::feature::{Feature, FeatureType};

        let (_tmp, project, cache) = setup();
        let service = StackupService::new(&project, &cache);

        // Hand-write a feature to disk so it's discoverable by the loader.
        let feature = Feature::new("CMP-TEST", FeatureType::Internal, "Test Hole", "Author");
        let feat_dir = project.root().join("tolerances/features");
        std::fs::create_dir_all(&feat_dir).unwrap();
        let feat_path = feat_dir.join(format!("{}.tdt.yaml", feature.id));
        std::fs::write(&feat_path, serde_yml::to_string(&feature).unwrap()).unwrap();

        let stackup = service.create(make_create_stackup("S")).unwrap();
        let stackup = service
            .add_contributor(
                &stackup.id.to_string(),
                AddContributorInput {
                    name: "Good".into(),
                    direction: Direction::Positive,
                    nominal: 1.0,
                    plus_tol: 0.1,
                    minus_tol: 0.1,
                    distribution: None,
                    source: None,
                    feature_id: Some(feature.id.clone()),
                },
            )
            .unwrap();

        let c = &stackup.contributors[0];
        let fr = c.feature.as_ref().expect("feature ref");
        assert_eq!(fr.id, feature.id);
        assert_eq!(fr.name.as_deref(), Some("Test Hole"));
        assert_eq!(fr.component_id.as_deref(), Some("CMP-TEST"));
    }
}
