//! Test service - business logic for verification/validation test protocols
//!
//! Provides CRUD operations and test execution for test protocols.

use crate::core::suspect::LinkRef;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::core::cache::{CachedTest, EntityCache};
use crate::core::entity::{Priority, Status};
use crate::core::identity::{EntityId, EntityPrefix};
use crate::core::loader;
use crate::core::project::Project;
use crate::entities::result::{Result as TestResult, StepResult, StepResultRecord, Verdict};
use crate::entities::test::{
    Environment, Equipment, ProcedureStep, SampleSize, Test, TestLevel, TestLinks, TestMethod,
    TestType,
};

use super::base::ServiceBase;
use super::common::{
    apply_pagination, CommonFilter, ListResult, ServiceError, ServiceResult, SortDirection,
};

/// Filter options specific to tests
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TestFilter {
    /// Common filter options (status, author, search, etc.)
    #[serde(flatten)]
    pub common: CommonFilter,

    /// Filter by test type (verification/validation)
    pub test_type: Option<TestType>,

    /// Filter by test level (unit/integration/system/acceptance)
    pub test_level: Option<TestLevel>,

    /// Filter by test method (inspection/analysis/demonstration/test)
    pub test_method: Option<TestMethod>,

    /// Filter by priority
    pub priority: Option<Priority>,

    /// Filter by category
    pub category: Option<String>,

    /// Show only orphan tests (no linked requirements)
    pub orphans_only: bool,

    /// Show only tests with no results recorded
    pub no_results: bool,
}

impl TestFilter {
    /// Create a filter for verification tests
    pub fn verification() -> Self {
        Self {
            test_type: Some(TestType::Verification),
            ..Default::default()
        }
    }

    /// Create a filter for validation tests
    pub fn validation() -> Self {
        Self {
            test_type: Some(TestType::Validation),
            ..Default::default()
        }
    }

    /// Create a filter for tests at a specific level
    pub fn at_level(level: TestLevel) -> Self {
        Self {
            test_level: Some(level),
            ..Default::default()
        }
    }

    /// Create a filter for orphan tests
    pub fn orphans() -> Self {
        Self {
            orphans_only: true,
            ..Default::default()
        }
    }
}

/// Sort field for tests
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TestSortField {
    Id,
    Title,
    Type,
    Level,
    Method,
    Status,
    Priority,
    Category,
    Author,
    #[default]
    Created,
}

/// Input for creating a new test
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CreateTest {
    /// Test title
    pub title: String,

    /// Author name
    pub author: String,

    /// Test type (verification or validation)
    #[serde(default)]
    pub test_type: TestType,

    /// Test level
    #[serde(default)]
    pub test_level: Option<TestLevel>,

    /// Test method
    #[serde(default)]
    pub test_method: Option<TestMethod>,

    /// Test objective
    pub objective: String,

    /// Description
    #[serde(default)]
    pub description: Option<String>,

    /// Category
    #[serde(default)]
    pub category: Option<String>,

    /// Priority
    #[serde(default)]
    pub priority: Priority,

    /// Estimated duration
    #[serde(default)]
    pub estimated_duration: Option<String>,

    /// Classification tags
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Input for updating an existing test
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateTest {
    /// Update title
    pub title: Option<String>,

    /// Update test type
    pub test_type: Option<TestType>,

    /// Update test level
    pub test_level: Option<TestLevel>,

    /// Update test method
    pub test_method: Option<TestMethod>,

    /// Update objective
    pub objective: Option<String>,

    /// Update description
    pub description: Option<String>,

    /// Update category
    pub category: Option<String>,

    /// Update priority
    pub priority: Option<Priority>,

    /// Update estimated duration
    pub estimated_duration: Option<String>,

    /// Update status
    pub status: Option<Status>,

    /// Replace tags
    pub tags: Option<Vec<String>>,
}

/// Statistics about tests
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TestStats {
    pub total: usize,
    pub by_type: TestTypeCounts,
    pub by_level: TestLevelCounts,
    pub by_method: TestMethodCounts,
    pub by_status: TestStatusCounts,
    pub by_priority: TestPriorityCounts,
    pub with_procedure: usize,
    pub with_equipment: usize,
    pub orphans: usize,
    pub total_steps: usize,
}

/// Counts by test type
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TestTypeCounts {
    pub verification: usize,
    pub validation: usize,
}

/// Counts by test level
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TestLevelCounts {
    pub unit: usize,
    pub integration: usize,
    pub system: usize,
    pub acceptance: usize,
    pub unspecified: usize,
}

/// Counts by test method
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TestMethodCounts {
    pub inspection: usize,
    pub analysis: usize,
    pub demonstration: usize,
    pub test: usize,
    pub unspecified: usize,
}

/// Counts by status
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TestStatusCounts {
    pub draft: usize,
    pub review: usize,
    pub approved: usize,
    pub released: usize,
    pub obsolete: usize,
}

/// Counts by priority
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TestPriorityCounts {
    pub critical: usize,
    pub high: usize,
    pub medium: usize,
    pub low: usize,
}

/// Input for running a test
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunTestInput {
    /// Test verdict
    pub verdict: Verdict,

    /// Who executed the test
    pub executed_by: String,

    /// Optional notes
    #[serde(default)]
    pub notes: Option<String>,

    /// Optional verdict rationale
    #[serde(default)]
    pub verdict_rationale: Option<String>,
}

/// Service for test management
pub struct TestService<'a> {
    project: &'a Project,
    cache: &'a EntityCache,
    base: ServiceBase<'a>,
}

impl<'a> TestService<'a> {
    /// Create a new test service
    pub fn new(project: &'a Project, cache: &'a EntityCache) -> Self {
        Self {
            project,
            cache,
            base: ServiceBase::new(project, cache),
        }
    }

    /// Get the directory for storing tests based on type
    fn get_directory(&self, test_type: TestType) -> PathBuf {
        match test_type {
            TestType::Verification => self.project.root().join("verification/protocols"),
            TestType::Validation => self.project.root().join("validation/protocols"),
        }
    }

    /// Get the results directory based on test type
    fn get_results_directory(&self, test_type: TestType) -> PathBuf {
        match test_type {
            TestType::Verification => self.project.root().join("verification/results"),
            TestType::Validation => self.project.root().join("validation/results"),
        }
    }

    /// Get all test directories
    fn get_all_directories(&self) -> Vec<PathBuf> {
        vec![
            self.get_directory(TestType::Verification),
            self.get_directory(TestType::Validation),
        ]
    }

    /// Get the file path for a test
    fn get_file_path(&self, id: &EntityId, test_type: TestType) -> PathBuf {
        self.get_directory(test_type)
            .join(format!("{}.tdt.yaml", id))
    }

    /// List tests using the cache (fast path)
    ///
    /// Returns cached test data without loading full entities from disk.
    /// Use this for list views and simple queries.
    pub fn list_cached(&self, filter: &TestFilter) -> ServiceResult<ListResult<CachedTest>> {
        // Convert filter values to strings for cache query
        let status_str = filter
            .common
            .status
            .as_ref()
            .and_then(|s| s.first())
            .map(|s| format!("{:?}", s).to_lowercase());

        let test_type_str = filter.test_type.as_ref().map(|t| match t {
            TestType::Verification => "verification",
            TestType::Validation => "validation",
        });

        let level_str = filter.test_level.as_ref().map(|l| match l {
            TestLevel::Unit => "unit",
            TestLevel::Integration => "integration",
            TestLevel::System => "system",
            TestLevel::Acceptance => "acceptance",
        });

        let method_str = filter.test_method.as_ref().map(|m| match m {
            TestMethod::Inspection => "inspection",
            TestMethod::Analysis => "analysis",
            TestMethod::Demonstration => "demonstration",
            TestMethod::Test => "test",
        });

        let priority_str = filter.priority.as_ref().map(|p| match p {
            Priority::Low => "low",
            Priority::Medium => "medium",
            Priority::High => "high",
            Priority::Critical => "critical",
        });

        // Query cache
        let mut cached = self.cache.list_tests(
            status_str.as_deref(),
            test_type_str,
            level_str,
            method_str,
            priority_str,
            filter.category.as_deref(),
            filter.common.author.as_deref(),
            filter.common.search.as_deref(),
            None, // Apply limit after all filters
        );

        // Apply additional filters not supported by cache query
        if let Some(days) = filter.common.recent_days {
            let cutoff = Utc::now() - chrono::Duration::days(days as i64);
            cached.retain(|t| t.created >= cutoff);
        }

        if let Some(tags) = &filter.common.tags {
            // Tags not stored in CachedTest - would need full entity load
            // For now, skip tag filtering in cached mode
            let _ = tags;
        }

        // Note: orphans_only and no_results filters require link/result data
        // which isn't available in the cache. These are handled in list()

        // Paginate
        Ok(apply_pagination(
            cached,
            filter.common.offset,
            filter.common.limit,
        ))
    }

    /// List tests with filtering and pagination
    pub fn list(
        &self,
        filter: &TestFilter,
        sort_by: TestSortField,
        sort_dir: SortDirection,
    ) -> ServiceResult<ListResult<Test>> {
        let mut tests = self.load_all()?;

        // Apply filters
        tests.retain(|t| self.matches_filter(t, filter));

        // Sort
        self.sort_tests(&mut tests, sort_by, sort_dir);

        // Paginate
        Ok(apply_pagination(
            tests,
            filter.common.offset,
            filter.common.limit,
        ))
    }

    /// Load all tests from both verification and validation directories
    pub fn load_all(&self) -> ServiceResult<Vec<Test>> {
        let mut all_tests = Vec::new();

        for dir in self.get_all_directories() {
            if dir.exists() {
                let tests: Vec<Test> = loader::load_all(&dir)?;
                all_tests.extend(tests);
            }
        }

        Ok(all_tests)
    }

    /// Get a single test by ID
    ///
    /// Uses the cache to find the file path, then loads the full entity from disk.
    pub fn get(&self, id: &str) -> ServiceResult<Option<Test>> {
        // Try to find in cache first for fast path lookup
        if let Some(cached) = self.cache.get_entity(id) {
            let path = if cached.file_path.is_absolute() {
                cached.file_path.clone()
            } else {
                self.project.root().join(&cached.file_path)
            };
            if path.exists() {
                if let Ok(test) = crate::yaml::parse_yaml_file::<Test>(&path) {
                    return Ok(Some(test));
                }
            }
        }

        // Fall back to directory scan
        for dir in self.get_all_directories() {
            if let Some((_, test)) = loader::load_entity::<Test>(&dir, id)? {
                return Ok(Some(test));
            }
        }
        Ok(None)
    }

    /// Get a test by ID, returning an error if not found
    pub fn get_required(&self, id: &str) -> ServiceResult<Test> {
        self.get(id)?
            .ok_or_else(|| ServiceError::NotFound(id.to_string()))
    }

    /// Get tests that verify a specific requirement
    pub fn get_by_requirement(&self, req_id: &str) -> ServiceResult<Vec<Test>> {
        let tests = self.load_all()?;
        Ok(tests
            .into_iter()
            .filter(|t| {
                t.links
                    .verifies
                    .iter()
                    .any(|id| id.to_string().contains(req_id))
                    || t.links
                        .validates
                        .iter()
                        .any(|id| id.to_string().contains(req_id))
            })
            .collect())
    }

    /// Get tests that mitigate a specific risk
    pub fn get_by_risk(&self, risk_id: &str) -> ServiceResult<Vec<Test>> {
        let tests = self.load_all()?;
        Ok(tests
            .into_iter()
            .filter(|t| {
                t.links
                    .mitigates
                    .iter()
                    .any(|id| id.to_string().contains(risk_id))
            })
            .collect())
    }

    /// Create a new test
    pub fn create(&self, input: CreateTest) -> ServiceResult<Test> {
        let id = EntityId::new(EntityPrefix::Test);

        let test = Test {
            id: id.clone(),
            test_type: input.test_type,
            test_level: input.test_level,
            test_method: input.test_method,
            title: input.title,
            category: input.category,
            tags: input.tags,
            objective: input.objective,
            description: input.description,
            preconditions: Vec::new(),
            equipment: Vec::new(),
            procedure: Vec::new(),
            acceptance_criteria: Vec::new(),
            sample_size: None,
            environment: None,
            estimated_duration: input.estimated_duration,
            priority: input.priority,
            status: Status::Draft,
            links: TestLinks::default(),
            created: Utc::now(),
            author: input.author,
            revision: 1,
        };

        // Write to file
        let path = self.get_file_path(&id, input.test_type);
        self.base.save(&test, &path, Some("TEST"))?;

        Ok(test)
    }

    /// Update an existing test
    pub fn update(&self, id: &str, input: UpdateTest) -> ServiceResult<Test> {
        let (path, mut test) = self.find_test(id)?;

        // Apply updates
        if let Some(title) = input.title {
            test.title = title;
        }
        if let Some(test_type) = input.test_type {
            test.test_type = test_type;
        }
        if let Some(test_level) = input.test_level {
            test.test_level = Some(test_level);
        }
        if let Some(test_method) = input.test_method {
            test.test_method = Some(test_method);
        }
        if let Some(objective) = input.objective {
            test.objective = objective;
        }
        if let Some(description) = input.description {
            test.description = Some(description);
        }
        if let Some(category) = input.category {
            test.category = Some(category);
        }
        if let Some(priority) = input.priority {
            test.priority = priority;
        }
        if let Some(estimated_duration) = input.estimated_duration {
            test.estimated_duration = Some(estimated_duration);
        }
        if let Some(status) = input.status {
            test.status = status;
        }
        if let Some(tags) = input.tags {
            test.tags = tags;
        }

        // Increment revision
        test.revision += 1;

        // Write back
        self.base.save(&test, &path, None)?;

        Ok(test)
    }

    /// Delete a test
    pub fn delete(&self, id: &str, force: bool) -> ServiceResult<()> {
        let (path, test) = self.find_test(id)?;

        // Check for references unless force is true
        if !force && !test.links.depends_on.is_empty() {
            return Err(ServiceError::HasReferences);
        }

        // Delete the file
        fs::remove_file(&path)?;

        Ok(())
    }

    /// Add a procedure step
    pub fn add_step(&self, id: &str, step: ProcedureStep) -> ServiceResult<Test> {
        let (path, mut test) = self.find_test(id)?;

        test.procedure.push(step);

        // Re-number steps sequentially
        for (i, s) in test.procedure.iter_mut().enumerate() {
            s.step = (i + 1) as u32;
        }

        test.revision += 1;

        self.base.save(&test, &path, None)?;

        Ok(test)
    }

    /// Remove a procedure step by step number
    pub fn remove_step(&self, id: &str, step_number: u32) -> ServiceResult<Test> {
        let (path, mut test) = self.find_test(id)?;

        let initial_len = test.procedure.len();
        test.procedure.retain(|s| s.step != step_number);

        if test.procedure.len() == initial_len {
            return Err(ServiceError::NotFound(format!(
                "Step {} not found",
                step_number
            )));
        }

        // Re-number steps sequentially
        for (i, s) in test.procedure.iter_mut().enumerate() {
            s.step = (i + 1) as u32;
        }

        test.revision += 1;

        self.base.save(&test, &path, None)?;

        Ok(test)
    }

    /// Add equipment requirement
    pub fn add_equipment(&self, id: &str, equipment: Equipment) -> ServiceResult<Test> {
        let (path, mut test) = self.find_test(id)?;

        test.equipment.push(equipment);
        test.revision += 1;

        self.base.save(&test, &path, None)?;

        Ok(test)
    }

    /// Remove equipment by name
    pub fn remove_equipment(&self, id: &str, equipment_name: &str) -> ServiceResult<Test> {
        let (path, mut test) = self.find_test(id)?;

        let initial_len = test.equipment.len();
        test.equipment.retain(|e| e.name != equipment_name);

        if test.equipment.len() == initial_len {
            return Err(ServiceError::NotFound(format!(
                "Equipment '{}' not found",
                equipment_name
            )));
        }

        test.revision += 1;

        self.base.save(&test, &path, None)?;

        Ok(test)
    }

    /// Add a precondition
    pub fn add_precondition(&self, id: &str, precondition: String) -> ServiceResult<Test> {
        let (path, mut test) = self.find_test(id)?;

        test.preconditions.push(precondition);
        test.revision += 1;

        self.base.save(&test, &path, None)?;

        Ok(test)
    }

    /// Add an acceptance criterion
    pub fn add_acceptance_criterion(&self, id: &str, criterion: String) -> ServiceResult<Test> {
        let (path, mut test) = self.find_test(id)?;

        test.acceptance_criteria.push(criterion);
        test.revision += 1;

        self.base.save(&test, &path, None)?;

        Ok(test)
    }

    /// Set sample size information
    pub fn set_sample_size(&self, id: &str, sample_size: SampleSize) -> ServiceResult<Test> {
        let (path, mut test) = self.find_test(id)?;

        test.sample_size = Some(sample_size);
        test.revision += 1;

        self.base.save(&test, &path, None)?;

        Ok(test)
    }

    /// Set environment conditions
    pub fn set_environment(&self, id: &str, environment: Environment) -> ServiceResult<Test> {
        let (path, mut test) = self.find_test(id)?;

        test.environment = Some(environment);
        test.revision += 1;

        self.base.save(&test, &path, None)?;

        Ok(test)
    }

    /// Add a verifies link (test verifies requirement)
    pub fn add_verifies_link(&self, id: &str, req_id: EntityId) -> ServiceResult<Test> {
        let (path, mut test) = self.find_test(id)?;

        if !test.links.verifies.iter().any(|l| *l == req_id) {
            test.links.verifies.push(LinkRef::from(req_id));
            test.revision += 1;

            self.base.save(&test, &path, None)?;
        }

        Ok(test)
    }

    /// Add a mitigates link (test mitigates risk)
    pub fn add_mitigates_link(&self, id: &str, risk_id: EntityId) -> ServiceResult<Test> {
        let (path, mut test) = self.find_test(id)?;

        if !test.links.mitigates.iter().any(|l| *l == risk_id) {
            test.links.mitigates.push(LinkRef::from(risk_id));
            test.revision += 1;

            self.base.save(&test, &path, None)?;
        }

        Ok(test)
    }

    /// Run a test and create a result entity
    pub fn run(&self, id: &str, input: RunTestInput) -> ServiceResult<TestResult> {
        let test = self.get_required(id)?;

        let result_id = EntityId::new(EntityPrefix::Rslt);

        // Scaffold step results from test procedure
        let step_results: Vec<StepResultRecord> = test
            .procedure
            .iter()
            .map(|step| {
                let step_result = match input.verdict {
                    Verdict::Pass => StepResult::Pass,
                    Verdict::Fail => StepResult::Fail,
                    Verdict::Conditional => StepResult::Pass,
                    Verdict::Incomplete => StepResult::Skip,
                    Verdict::NotApplicable => StepResult::NotApplicable,
                };

                StepResultRecord {
                    step: step.step,
                    result: step_result,
                    observed: None,
                    measurement: None,
                    notes: None,
                }
            })
            .collect();

        let result = TestResult {
            id: result_id.clone(),
            test_id: test.id.clone(),
            test_revision: Some(test.revision),
            title: Some(format!("Result for {}", test.title)),
            verdict: input.verdict,
            verdict_rationale: input.verdict_rationale,
            category: test.category.clone(),
            tags: Vec::new(),
            executed_date: Utc::now(),
            executed_by: input.executed_by.clone(),
            reviewed_by: None,
            reviewed_date: None,
            sample_info: None,
            environment: None,
            equipment_used: Vec::new(),
            step_results,
            deviations: Vec::new(),
            failures: Vec::new(),
            attachments: Vec::new(),
            duration: None,
            notes: input.notes,
            status: Status::Draft,
            links: Default::default(),
            created: Utc::now(),
            author: input.executed_by,
            revision: 1,
        };

        // Write result to file
        let results_dir = self.get_results_directory(test.test_type);
        let path = results_dir.join(format!("{}.tdt.yaml", result_id));
        self.base.save(&result, &path, Some("RSLT"))?;

        Ok(result)
    }

    /// Get statistics about tests
    pub fn stats(&self) -> ServiceResult<TestStats> {
        let tests = self.load_all()?;

        let mut stats = TestStats {
            total: tests.len(),
            ..Default::default()
        };

        for test in &tests {
            // Count by type
            match test.test_type {
                TestType::Verification => stats.by_type.verification += 1,
                TestType::Validation => stats.by_type.validation += 1,
            }

            // Count by level
            match test.test_level {
                Some(TestLevel::Unit) => stats.by_level.unit += 1,
                Some(TestLevel::Integration) => stats.by_level.integration += 1,
                Some(TestLevel::System) => stats.by_level.system += 1,
                Some(TestLevel::Acceptance) => stats.by_level.acceptance += 1,
                None => stats.by_level.unspecified += 1,
            }

            // Count by method
            match test.test_method {
                Some(TestMethod::Inspection) => stats.by_method.inspection += 1,
                Some(TestMethod::Analysis) => stats.by_method.analysis += 1,
                Some(TestMethod::Demonstration) => stats.by_method.demonstration += 1,
                Some(TestMethod::Test) => stats.by_method.test += 1,
                None => stats.by_method.unspecified += 1,
            }

            // Count by status
            match test.status {
                Status::Draft => stats.by_status.draft += 1,
                Status::Review => stats.by_status.review += 1,
                Status::Approved => stats.by_status.approved += 1,
                Status::Released => stats.by_status.released += 1,
                Status::Obsolete => stats.by_status.obsolete += 1,
            }

            // Count by priority
            match test.priority {
                Priority::Critical => stats.by_priority.critical += 1,
                Priority::High => stats.by_priority.high += 1,
                Priority::Medium => stats.by_priority.medium += 1,
                Priority::Low => stats.by_priority.low += 1,
            }

            // Count features
            if !test.procedure.is_empty() {
                stats.with_procedure += 1;
                stats.total_steps += test.procedure.len();
            }
            if !test.equipment.is_empty() {
                stats.with_equipment += 1;
            }
            if test.links.verifies.is_empty() && test.links.validates.is_empty() {
                stats.orphans += 1;
            }
        }

        Ok(stats)
    }

    // --- Private helper methods ---

    /// Find a test and its file path
    ///
    /// Uses the cache to find the file path for faster lookup.
    fn find_test(&self, id: &str) -> ServiceResult<(PathBuf, Test)> {
        // Try to find in cache first for fast path lookup
        if let Some(cached) = self.cache.get_entity(id) {
            let path = if cached.file_path.is_absolute() {
                cached.file_path.clone()
            } else {
                self.project.root().join(&cached.file_path)
            };
            if path.exists() {
                if let Ok(test) = crate::yaml::parse_yaml_file::<Test>(&path) {
                    return Ok((path, test));
                }
            }
        }

        // Fall back to directory scan
        for dir in self.get_all_directories() {
            if let Some((path, test)) = loader::load_entity::<Test>(&dir, id)? {
                return Ok((path, test));
            }
        }
        Err(ServiceError::NotFound(id.to_string()))
    }

    /// Check if a test matches the given filter
    fn matches_filter(&self, test: &Test, filter: &TestFilter) -> bool {
        // Test type filter
        if let Some(test_type) = &filter.test_type {
            if test.test_type != *test_type {
                return false;
            }
        }

        // Test level filter
        if let Some(test_level) = &filter.test_level {
            if test.test_level != Some(*test_level) {
                return false;
            }
        }

        // Test method filter
        if let Some(test_method) = &filter.test_method {
            if test.test_method != Some(*test_method) {
                return false;
            }
        }

        // Priority filter
        if let Some(priority) = &filter.priority {
            if test.priority != *priority {
                return false;
            }
        }

        // Category filter
        if let Some(category) = &filter.category {
            if !test
                .category
                .as_ref()
                .is_some_and(|c| c.to_lowercase().contains(&category.to_lowercase()))
            {
                return false;
            }
        }

        // Orphans only filter
        if filter.orphans_only
            && (!test.links.verifies.is_empty() || !test.links.validates.is_empty())
        {
            return false;
        }

        // Common filters
        if !filter.common.matches_status(&test.status) {
            return false;
        }
        if !filter.common.matches_author(&test.author) {
            return false;
        }
        if !filter.common.matches_tags(&test.tags) {
            return false;
        }
        if !filter
            .common
            .matches_search(&[&test.title, &test.objective])
        {
            return false;
        }
        if !filter.common.matches_recent(&test.created) {
            return false;
        }

        true
    }

    /// Sort tests by the given field
    fn sort_tests(&self, tests: &mut [Test], sort_by: TestSortField, sort_dir: SortDirection) {
        tests.sort_by(|a, b| {
            let cmp = match sort_by {
                TestSortField::Id => a.id.to_string().cmp(&b.id.to_string()),
                TestSortField::Title => a.title.cmp(&b.title),
                TestSortField::Type => a.test_type.to_string().cmp(&b.test_type.to_string()),
                TestSortField::Level => {
                    let level_order = |l: &Option<TestLevel>| match l {
                        Some(TestLevel::Unit) => 0,
                        Some(TestLevel::Integration) => 1,
                        Some(TestLevel::System) => 2,
                        Some(TestLevel::Acceptance) => 3,
                        None => 4,
                    };
                    level_order(&a.test_level).cmp(&level_order(&b.test_level))
                }
                TestSortField::Method => {
                    let method_str =
                        |m: &Option<TestMethod>| m.map(|m| m.to_string()).unwrap_or_default();
                    method_str(&a.test_method).cmp(&method_str(&b.test_method))
                }
                TestSortField::Status => format!("{:?}", a.status).cmp(&format!("{:?}", b.status)),
                TestSortField::Priority => {
                    let priority_order = |p: &Priority| match p {
                        Priority::Critical => 0,
                        Priority::High => 1,
                        Priority::Medium => 2,
                        Priority::Low => 3,
                    };
                    priority_order(&a.priority).cmp(&priority_order(&b.priority))
                }
                TestSortField::Category => a.category.cmp(&b.category),
                TestSortField::Author => a.author.cmp(&b.author),
                TestSortField::Created => a.created.cmp(&b.created),
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
    use tempfile::TempDir;

    fn setup_test_project() -> (TempDir, Project, EntityCache) {
        let tmp = TempDir::new().unwrap();

        // Initialize project structure
        fs::create_dir_all(tmp.path().join(".tdt")).unwrap();
        fs::create_dir_all(tmp.path().join("verification/protocols")).unwrap();
        fs::create_dir_all(tmp.path().join("verification/results")).unwrap();
        fs::create_dir_all(tmp.path().join("validation/protocols")).unwrap();
        fs::create_dir_all(tmp.path().join("validation/results")).unwrap();

        // Create config file
        fs::write(tmp.path().join(".tdt/config.yaml"), "author: Test Author\n").unwrap();

        let project = Project::discover_from(tmp.path()).unwrap();
        let cache = EntityCache::open(&project).unwrap();

        (tmp, project, cache)
    }

    #[test]
    fn test_create_verification_test() {
        let (_tmp, project, cache) = setup_test_project();
        let service = TestService::new(&project, &cache);

        let input = CreateTest {
            title: "Temperature Test".into(),
            author: "Test Author".into(),
            test_type: TestType::Verification,
            test_level: Some(TestLevel::System),
            test_method: Some(TestMethod::Test),
            objective: "Verify temperature range".into(),
            priority: Priority::High,
            ..Default::default()
        };

        let test = service.create(input).unwrap();

        assert_eq!(test.title, "Temperature Test");
        assert_eq!(test.test_type, TestType::Verification);
        assert_eq!(test.test_level, Some(TestLevel::System));
        assert_eq!(test.priority, Priority::High);
    }

    #[test]
    fn test_create_validation_test() {
        let (_tmp, project, cache) = setup_test_project();
        let service = TestService::new(&project, &cache);

        let input = CreateTest {
            title: "User Acceptance Test".into(),
            author: "Test Author".into(),
            test_type: TestType::Validation,
            test_level: Some(TestLevel::Acceptance),
            test_method: Some(TestMethod::Demonstration),
            objective: "Validate user requirements".into(),
            ..Default::default()
        };

        let test = service.create(input).unwrap();

        assert_eq!(test.test_type, TestType::Validation);
        assert_eq!(test.test_level, Some(TestLevel::Acceptance));
    }

    #[test]
    fn test_get_test() {
        let (_tmp, project, cache) = setup_test_project();
        let service = TestService::new(&project, &cache);

        let created = service
            .create(CreateTest {
                title: "Find Me".into(),
                author: "Test".into(),
                objective: "Test objective".into(),
                ..Default::default()
            })
            .unwrap();

        let found = service.get(&created.id.to_string()).unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().title, "Find Me");
    }

    #[test]
    fn test_update_test() {
        let (_tmp, project, cache) = setup_test_project();
        let service = TestService::new(&project, &cache);

        let created = service
            .create(CreateTest {
                title: "Original".into(),
                author: "Test".into(),
                objective: "Original objective".into(),
                ..Default::default()
            })
            .unwrap();

        let updated = service
            .update(
                &created.id.to_string(),
                UpdateTest {
                    title: Some("Updated Title".into()),
                    priority: Some(Priority::Critical),
                    ..Default::default()
                },
            )
            .unwrap();

        assert_eq!(updated.title, "Updated Title");
        assert_eq!(updated.priority, Priority::Critical);
        assert_eq!(updated.revision, 2);
    }

    #[test]
    fn test_delete_test() {
        let (_tmp, project, cache) = setup_test_project();
        let service = TestService::new(&project, &cache);

        let created = service
            .create(CreateTest {
                title: "Delete Me".into(),
                author: "Test".into(),
                objective: "To be deleted".into(),
                ..Default::default()
            })
            .unwrap();

        service.delete(&created.id.to_string(), false).unwrap();

        let found = service.get(&created.id.to_string()).unwrap();
        assert!(found.is_none());
    }

    #[test]
    fn test_add_step() {
        let (_tmp, project, cache) = setup_test_project();
        let service = TestService::new(&project, &cache);

        let created = service
            .create(CreateTest {
                title: "With Steps".into(),
                author: "Test".into(),
                objective: "Test with procedure".into(),
                ..Default::default()
            })
            .unwrap();

        let step1 = ProcedureStep {
            step: 1,
            action: "Apply power".into(),
            expected: Some("Device powers on".into()),
            acceptance: None,
        };

        let step2 = ProcedureStep {
            step: 2,
            action: "Check indicator".into(),
            expected: Some("LED is green".into()),
            acceptance: Some("LED must be green within 5s".into()),
        };

        service.add_step(&created.id.to_string(), step1).unwrap();
        let updated = service.add_step(&created.id.to_string(), step2).unwrap();

        assert_eq!(updated.procedure.len(), 2);
        assert_eq!(updated.procedure[0].action, "Apply power");
        assert_eq!(updated.procedure[1].action, "Check indicator");
    }

    #[test]
    fn test_remove_step() {
        let (_tmp, project, cache) = setup_test_project();
        let service = TestService::new(&project, &cache);

        let created = service
            .create(CreateTest {
                title: "Remove Step Test".into(),
                author: "Test".into(),
                objective: "Test step removal".into(),
                ..Default::default()
            })
            .unwrap();

        service
            .add_step(
                &created.id.to_string(),
                ProcedureStep {
                    step: 1,
                    action: "Step 1".into(),
                    expected: None,
                    acceptance: None,
                },
            )
            .unwrap();
        service
            .add_step(
                &created.id.to_string(),
                ProcedureStep {
                    step: 2,
                    action: "Step 2".into(),
                    expected: None,
                    acceptance: None,
                },
            )
            .unwrap();

        let updated = service.remove_step(&created.id.to_string(), 1).unwrap();

        assert_eq!(updated.procedure.len(), 1);
        assert_eq!(updated.procedure[0].step, 1); // Renumbered
        assert_eq!(updated.procedure[0].action, "Step 2");
    }

    #[test]
    fn test_add_equipment() {
        let (_tmp, project, cache) = setup_test_project();
        let service = TestService::new(&project, &cache);

        let created = service
            .create(CreateTest {
                title: "With Equipment".into(),
                author: "Test".into(),
                objective: "Test with equipment".into(),
                ..Default::default()
            })
            .unwrap();

        let equipment = Equipment {
            name: "Multimeter".into(),
            specification: Some("±0.1% accuracy".into()),
            calibration_required: Some(true),
        };

        let updated = service
            .add_equipment(&created.id.to_string(), equipment)
            .unwrap();

        assert_eq!(updated.equipment.len(), 1);
        assert_eq!(updated.equipment[0].name, "Multimeter");
        assert_eq!(updated.equipment[0].calibration_required, Some(true));
    }

    #[test]
    fn test_add_verifies_link() {
        let (_tmp, project, cache) = setup_test_project();
        let service = TestService::new(&project, &cache);

        let created = service
            .create(CreateTest {
                title: "Linked Test".into(),
                author: "Test".into(),
                objective: "Test with links".into(),
                ..Default::default()
            })
            .unwrap();

        let req_id = EntityId::new(EntityPrefix::Req);
        let updated = service
            .add_verifies_link(&created.id.to_string(), req_id.clone())
            .unwrap();

        assert_eq!(updated.links.verifies.len(), 1);
        assert_eq!(updated.links.verifies[0], req_id);
    }

    #[test]
    fn test_run_test() {
        let (_tmp, project, cache) = setup_test_project();
        let service = TestService::new(&project, &cache);

        let test = service
            .create(CreateTest {
                title: "Runnable Test".into(),
                author: "Test".into(),
                objective: "Test to run".into(),
                ..Default::default()
            })
            .unwrap();

        // Add some steps
        service
            .add_step(
                &test.id.to_string(),
                ProcedureStep {
                    step: 1,
                    action: "Step 1".into(),
                    expected: None,
                    acceptance: None,
                },
            )
            .unwrap();
        service
            .add_step(
                &test.id.to_string(),
                ProcedureStep {
                    step: 2,
                    action: "Step 2".into(),
                    expected: None,
                    acceptance: None,
                },
            )
            .unwrap();

        let result = service
            .run(
                &test.id.to_string(),
                RunTestInput {
                    verdict: Verdict::Pass,
                    executed_by: "Tester".into(),
                    notes: Some("All good".into()),
                    verdict_rationale: None,
                },
            )
            .unwrap();

        assert_eq!(result.verdict, Verdict::Pass);
        assert_eq!(result.executed_by, "Tester");
        assert_eq!(result.step_results.len(), 2);
        assert_eq!(result.test_id, test.id);
    }

    #[test]
    fn test_list_with_filter() {
        let (_tmp, project, cache) = setup_test_project();
        let service = TestService::new(&project, &cache);

        // Create verification test
        service
            .create(CreateTest {
                title: "Verification Test".into(),
                author: "Test".into(),
                test_type: TestType::Verification,
                test_level: Some(TestLevel::System),
                objective: "Verify something".into(),
                ..Default::default()
            })
            .unwrap();

        // Create validation test
        service
            .create(CreateTest {
                title: "Validation Test".into(),
                author: "Test".into(),
                test_type: TestType::Validation,
                test_level: Some(TestLevel::Acceptance),
                objective: "Validate something".into(),
                ..Default::default()
            })
            .unwrap();

        // List all
        let all = service
            .list(
                &TestFilter::default(),
                TestSortField::Title,
                SortDirection::Ascending,
            )
            .unwrap();
        assert_eq!(all.items.len(), 2);

        // List verification only
        let verification = service
            .list(
                &TestFilter::verification(),
                TestSortField::Title,
                SortDirection::Ascending,
            )
            .unwrap();
        assert_eq!(verification.items.len(), 1);
        assert_eq!(verification.items[0].test_type, TestType::Verification);

        // List by level
        let system_level = service
            .list(
                &TestFilter::at_level(TestLevel::System),
                TestSortField::Title,
                SortDirection::Ascending,
            )
            .unwrap();
        assert_eq!(system_level.items.len(), 1);
    }

    #[test]
    fn test_stats() {
        let (_tmp, project, cache) = setup_test_project();
        let service = TestService::new(&project, &cache);

        // Create tests with different attributes
        let test1 = service
            .create(CreateTest {
                title: "Test 1".into(),
                author: "Test".into(),
                test_type: TestType::Verification,
                test_level: Some(TestLevel::System),
                test_method: Some(TestMethod::Test),
                priority: Priority::High,
                objective: "Objective 1".into(),
                ..Default::default()
            })
            .unwrap();

        service
            .add_step(
                &test1.id.to_string(),
                ProcedureStep {
                    step: 1,
                    action: "Step".into(),
                    expected: None,
                    acceptance: None,
                },
            )
            .unwrap();

        service
            .create(CreateTest {
                title: "Test 2".into(),
                author: "Test".into(),
                test_type: TestType::Validation,
                test_level: Some(TestLevel::Acceptance),
                test_method: Some(TestMethod::Demonstration),
                priority: Priority::Medium,
                objective: "Objective 2".into(),
                ..Default::default()
            })
            .unwrap();

        let stats = service.stats().unwrap();

        assert_eq!(stats.total, 2);
        assert_eq!(stats.by_type.verification, 1);
        assert_eq!(stats.by_type.validation, 1);
        assert_eq!(stats.by_level.system, 1);
        assert_eq!(stats.by_level.acceptance, 1);
        assert_eq!(stats.by_method.test, 1);
        assert_eq!(stats.by_method.demonstration, 1);
        assert_eq!(stats.by_priority.high, 1);
        assert_eq!(stats.by_priority.medium, 1);
        assert_eq!(stats.with_procedure, 1);
        assert_eq!(stats.total_steps, 1);
        assert_eq!(stats.orphans, 2); // No requirements linked
    }
}
