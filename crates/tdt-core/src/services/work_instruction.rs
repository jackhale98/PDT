//! Work Instruction service - business logic for operator procedures
//!
//! Provides CRUD operations and step/tool/material management for work instructions.

use crate::core::suspect::LinkRef;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::core::cache::EntityCache;
use crate::core::entity::Status;
use crate::core::identity::{EntityId, EntityPrefix};
use crate::core::loader;
use crate::core::project::Project;
use crate::entities::work_instruction::{
    Material, ProcedureStep, QualityCheck, Tool, WorkInstruction, WorkInstructionLinks, WorkSafety,
};
use crate::services::base::ServiceBase;

use super::common::{
    apply_pagination, CommonFilter, ListResult, ServiceError, ServiceResult, SortDirection,
};

/// Filter options specific to work instructions
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkInstructionFilter {
    /// Common filter options (status, author, search, etc.)
    #[serde(flatten)]
    pub common: CommonFilter,

    /// Filter by linked process ID
    pub process: Option<String>,

    /// Show only instructions with safety requirements
    pub has_safety: bool,

    /// Show only instructions with quality checks
    pub has_quality_checks: bool,

    /// Filter by document number
    pub document_number: Option<String>,
}

impl WorkInstructionFilter {
    /// Create a filter for work instructions linked to a process
    pub fn by_process(process_id: &str) -> Self {
        Self {
            process: Some(process_id.to_string()),
            ..Default::default()
        }
    }

    /// Create a filter for work instructions with safety requirements
    pub fn with_safety() -> Self {
        Self {
            has_safety: true,
            ..Default::default()
        }
    }
}

/// Sort field for work instructions
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkInstructionSortField {
    Id,
    #[default]
    Title,
    DocumentNumber,
    Status,
    Author,
    Created,
    StepCount,
}

/// Input for creating a new work instruction
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CreateWorkInstruction {
    /// Work instruction title
    pub title: String,

    /// Author name
    pub author: String,

    /// Document number (e.g., "WI-MACH-015")
    #[serde(default)]
    pub document_number: Option<String>,

    /// Document revision
    #[serde(default)]
    pub revision: Option<String>,

    /// Description/purpose
    #[serde(default)]
    pub description: Option<String>,

    /// Linked process ID
    #[serde(default)]
    pub process: Option<String>,

    /// Estimated duration in minutes
    #[serde(default)]
    pub estimated_duration_minutes: Option<f64>,

    /// Classification tags
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Input for updating an existing work instruction
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateWorkInstruction {
    /// Update title
    pub title: Option<String>,

    /// Update document number
    pub document_number: Option<String>,

    /// Update revision
    pub revision: Option<String>,

    /// Update description
    pub description: Option<String>,

    /// Update estimated duration
    pub estimated_duration_minutes: Option<f64>,

    /// Update status
    pub status: Option<Status>,

    /// Replace tags
    pub tags: Option<Vec<String>>,
}

/// Statistics about work instructions
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkInstructionStats {
    pub total: usize,
    pub by_status: WorkInstructionStatusCounts,
    pub with_procedure_steps: usize,
    pub with_tools: usize,
    pub with_materials: usize,
    pub with_safety: usize,
    pub with_quality_checks: usize,
    pub total_steps: usize,
    pub avg_steps_per_instruction: f64,
}

/// Counts by status
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkInstructionStatusCounts {
    pub draft: usize,
    pub review: usize,
    pub approved: usize,
    pub released: usize,
    pub obsolete: usize,
}

/// Service for work instruction management
pub struct WorkInstructionService<'a> {
    project: &'a Project,
    cache: &'a EntityCache,
    base: ServiceBase<'a>,
}

impl<'a> WorkInstructionService<'a> {
    /// Create a new work instruction service
    pub fn new(project: &'a Project, cache: &'a EntityCache) -> Self {
        Self {
            project,
            cache,
            base: ServiceBase::new(project, cache),
        }
    }

    /// Get the directory for storing work instructions
    fn get_directory(&self) -> PathBuf {
        self.project.root().join("manufacturing/work_instructions")
    }

    /// Get the file path for a work instruction
    fn get_file_path(&self, id: &EntityId) -> PathBuf {
        self.get_directory().join(format!("{}.tdt.yaml", id))
    }

    /// List work instructions with filtering and pagination
    pub fn list(
        &self,
        filter: &WorkInstructionFilter,
        sort_by: WorkInstructionSortField,
        sort_dir: SortDirection,
    ) -> ServiceResult<ListResult<WorkInstruction>> {
        let mut instructions = self.load_all()?;

        // Apply filters
        instructions.retain(|wi| self.matches_filter(wi, filter));

        // Sort
        self.sort_work_instructions(&mut instructions, sort_by, sort_dir);

        // Paginate
        Ok(apply_pagination(
            instructions,
            filter.common.offset,
            filter.common.limit,
        ))
    }

    /// List work instructions from cache (fast path for list display)
    ///
    /// Returns cached work instruction data without loading full YAML files.
    /// Use this for list commands where full entity data isn't needed.
    pub fn list_cached(&self) -> Vec<crate::core::CachedEntity> {
        use crate::core::cache::EntityFilter;
        use crate::core::identity::EntityPrefix;

        let filter = EntityFilter {
            prefix: Some(EntityPrefix::Work),
            ..Default::default()
        };
        self.cache.list_entities(&filter)
    }

    /// Load all work instructions from the filesystem
    pub fn load_all(&self) -> ServiceResult<Vec<WorkInstruction>> {
        let dir = self.get_directory();
        if !dir.exists() {
            return Ok(Vec::new());
        }
        Ok(loader::load_all(&dir)?)
    }

    /// Get a single work instruction by ID
    pub fn get(&self, id: &str) -> ServiceResult<Option<WorkInstruction>> {
        let dir = self.get_directory();
        if let Some((_, wi)) = loader::load_entity::<WorkInstruction>(&dir, id)? {
            return Ok(Some(wi));
        }
        Ok(None)
    }

    /// Get a work instruction by ID, returning an error if not found
    pub fn get_required(&self, id: &str) -> ServiceResult<WorkInstruction> {
        self.get(id)?
            .ok_or_else(|| ServiceError::NotFound(id.to_string()))
    }

    /// Get work instructions linked to a specific process
    pub fn get_by_process(&self, process_id: &str) -> ServiceResult<Vec<WorkInstruction>> {
        let instructions = self.load_all()?;
        Ok(instructions
            .into_iter()
            .filter(|wi| {
                wi.links
                    .process
                    .as_ref()
                    .is_some_and(|p| p.to_string().contains(process_id))
            })
            .collect())
    }

    /// Create a new work instruction
    pub fn create(&self, input: CreateWorkInstruction) -> ServiceResult<WorkInstruction> {
        let id = EntityId::new(EntityPrefix::Work);

        let links = WorkInstructionLinks {
            // Store the reference as given; validation happens via `tdt validate`
            process: input.process.map(LinkRef::from),
            controls: Vec::new(),
        };

        let instruction = WorkInstruction {
            id: id.clone(),
            title: input.title,
            document_number: input.document_number,
            revision: input.revision,
            description: input.description,
            safety: None,
            tools_required: Vec::new(),
            materials_required: Vec::new(),
            procedure: Vec::new(),
            quality_checks: Vec::new(),
            estimated_duration_minutes: input.estimated_duration_minutes,
            tags: input.tags,
            status: Status::Draft,
            links,
            created: Utc::now(),
            author: input.author,
            entity_revision: 1,
        };

        // Write to file
        let path = self.get_file_path(&id);
        self.base.save(&instruction, &path, Some("WORK"))?;

        Ok(instruction)
    }

    /// Update an existing work instruction
    pub fn update(&self, id: &str, input: UpdateWorkInstruction) -> ServiceResult<WorkInstruction> {
        let (path, mut instruction) = self.find_work_instruction(id)?;

        // Apply updates
        if let Some(title) = input.title {
            instruction.title = title;
        }
        if let Some(document_number) = input.document_number {
            instruction.document_number = Some(document_number);
        }
        if let Some(revision) = input.revision {
            instruction.revision = Some(revision);
        }
        if let Some(description) = input.description {
            instruction.description = Some(description);
        }
        if let Some(estimated_duration_minutes) = input.estimated_duration_minutes {
            instruction.estimated_duration_minutes = Some(estimated_duration_minutes);
        }
        if let Some(status) = input.status {
            instruction.status = status;
        }
        if let Some(tags) = input.tags {
            instruction.tags = tags;
        }

        // Increment revision
        instruction.entity_revision += 1;

        // Write back
        self.base.save(&instruction, &path, None)?;

        Ok(instruction)
    }

    /// Delete a work instruction
    pub fn delete(&self, id: &str, force: bool) -> ServiceResult<()> {
        let (path, instruction) = self.find_work_instruction(id)?;

        // Check for references unless force is true
        if !force && !instruction.links.controls.is_empty() {
            return Err(ServiceError::HasReferences);
        }

        // Delete the file
        fs::remove_file(&path)?;

        Ok(())
    }

    /// Add a procedure step
    pub fn add_step(&self, id: &str, step: ProcedureStep) -> ServiceResult<WorkInstruction> {
        let (path, mut instruction) = self.find_work_instruction(id)?;

        instruction.procedure.push(step);

        // Re-number steps sequentially
        for (i, s) in instruction.procedure.iter_mut().enumerate() {
            s.step = (i + 1) as u32;
        }

        instruction.entity_revision += 1;

        self.base.save(&instruction, &path, None)?;

        Ok(instruction)
    }

    /// Remove a procedure step by step number
    pub fn remove_step(&self, id: &str, step_number: u32) -> ServiceResult<WorkInstruction> {
        let (path, mut instruction) = self.find_work_instruction(id)?;

        let initial_len = instruction.procedure.len();
        instruction.procedure.retain(|s| s.step != step_number);

        if instruction.procedure.len() == initial_len {
            return Err(ServiceError::NotFound(format!(
                "Step {} not found",
                step_number
            )));
        }

        // Re-number steps sequentially
        for (i, s) in instruction.procedure.iter_mut().enumerate() {
            s.step = (i + 1) as u32;
        }

        instruction.entity_revision += 1;

        self.base.save(&instruction, &path, None)?;

        Ok(instruction)
    }

    /// Add a required tool
    pub fn add_tool(&self, id: &str, tool: Tool) -> ServiceResult<WorkInstruction> {
        let (path, mut instruction) = self.find_work_instruction(id)?;

        instruction.tools_required.push(tool);
        instruction.entity_revision += 1;

        self.base.save(&instruction, &path, None)?;

        Ok(instruction)
    }

    /// Remove a tool by name
    pub fn remove_tool(&self, id: &str, tool_name: &str) -> ServiceResult<WorkInstruction> {
        let (path, mut instruction) = self.find_work_instruction(id)?;

        let initial_len = instruction.tools_required.len();
        instruction.tools_required.retain(|t| t.name != tool_name);

        if instruction.tools_required.len() == initial_len {
            return Err(ServiceError::NotFound(format!(
                "Tool '{}' not found",
                tool_name
            )));
        }

        instruction.entity_revision += 1;

        self.base.save(&instruction, &path, None)?;

        Ok(instruction)
    }

    /// Add a required material
    pub fn add_material(&self, id: &str, material: Material) -> ServiceResult<WorkInstruction> {
        let (path, mut instruction) = self.find_work_instruction(id)?;

        instruction.materials_required.push(material);
        instruction.entity_revision += 1;

        self.base.save(&instruction, &path, None)?;

        Ok(instruction)
    }

    /// Remove a material by name
    pub fn remove_material(&self, id: &str, material_name: &str) -> ServiceResult<WorkInstruction> {
        let (path, mut instruction) = self.find_work_instruction(id)?;

        let initial_len = instruction.materials_required.len();
        instruction
            .materials_required
            .retain(|m| m.name != material_name);

        if instruction.materials_required.len() == initial_len {
            return Err(ServiceError::NotFound(format!(
                "Material '{}' not found",
                material_name
            )));
        }

        instruction.entity_revision += 1;

        self.base.save(&instruction, &path, None)?;

        Ok(instruction)
    }

    /// Add a quality check
    pub fn add_quality_check(
        &self,
        id: &str,
        check: QualityCheck,
    ) -> ServiceResult<WorkInstruction> {
        let (path, mut instruction) = self.find_work_instruction(id)?;

        instruction.quality_checks.push(check);
        instruction.entity_revision += 1;

        self.base.save(&instruction, &path, None)?;

        Ok(instruction)
    }

    /// Remove a quality check by step number
    pub fn remove_quality_check(&self, id: &str, at_step: u32) -> ServiceResult<WorkInstruction> {
        let (path, mut instruction) = self.find_work_instruction(id)?;

        let initial_len = instruction.quality_checks.len();
        instruction
            .quality_checks
            .retain(|qc| qc.at_step != at_step);

        if instruction.quality_checks.len() == initial_len {
            return Err(ServiceError::NotFound(format!(
                "Quality check at step {} not found",
                at_step
            )));
        }

        instruction.entity_revision += 1;

        self.base.save(&instruction, &path, None)?;

        Ok(instruction)
    }

    /// Set safety requirements
    pub fn set_safety(&self, id: &str, safety: WorkSafety) -> ServiceResult<WorkInstruction> {
        let (path, mut instruction) = self.find_work_instruction(id)?;

        instruction.safety = Some(safety);
        instruction.entity_revision += 1;

        self.base.save(&instruction, &path, None)?;

        Ok(instruction)
    }

    /// Clear safety requirements
    pub fn clear_safety(&self, id: &str) -> ServiceResult<WorkInstruction> {
        let (path, mut instruction) = self.find_work_instruction(id)?;

        instruction.safety = None;
        instruction.entity_revision += 1;

        self.base.save(&instruction, &path, None)?;

        Ok(instruction)
    }

    /// Calculate estimated duration from step times
    pub fn calculate_estimated_duration(&self, id: &str) -> ServiceResult<Option<f64>> {
        let instruction = self.get_required(id)?;

        let total: f64 = instruction
            .procedure
            .iter()
            .filter_map(|s| s.estimated_time_minutes)
            .sum();

        if total > 0.0 {
            Ok(Some(total))
        } else {
            Ok(instruction.estimated_duration_minutes)
        }
    }

    /// Get statistics about work instructions
    pub fn stats(&self) -> ServiceResult<WorkInstructionStats> {
        let instructions = self.load_all()?;

        let mut stats = WorkInstructionStats {
            total: instructions.len(),
            ..Default::default()
        };

        for wi in &instructions {
            // Count by status
            match wi.status {
                Status::Draft => stats.by_status.draft += 1,
                Status::Review => stats.by_status.review += 1,
                Status::Approved => stats.by_status.approved += 1,
                Status::Released => stats.by_status.released += 1,
                Status::Obsolete => stats.by_status.obsolete += 1,
            }

            // Count features
            if !wi.procedure.is_empty() {
                stats.with_procedure_steps += 1;
                stats.total_steps += wi.procedure.len();
            }
            if !wi.tools_required.is_empty() {
                stats.with_tools += 1;
            }
            if !wi.materials_required.is_empty() {
                stats.with_materials += 1;
            }
            if wi.safety.is_some() {
                stats.with_safety += 1;
            }
            if !wi.quality_checks.is_empty() {
                stats.with_quality_checks += 1;
            }
        }

        // Calculate average steps
        if stats.with_procedure_steps > 0 {
            stats.avg_steps_per_instruction =
                stats.total_steps as f64 / stats.with_procedure_steps as f64;
        }

        Ok(stats)
    }

    // --- Private helper methods ---

    /// Find a work instruction and its file path (cache-first lookup)
    fn find_work_instruction(&self, id: &str) -> ServiceResult<(PathBuf, WorkInstruction)> {
        // Try to find in cache first for fast path lookup
        if let Some(cached) = self.cache.get_entity(id) {
            let path = if cached.file_path.is_absolute() {
                cached.file_path.clone()
            } else {
                self.project.root().join(&cached.file_path)
            };
            if path.exists() {
                if let Ok(wi) = crate::yaml::parse_yaml_file::<WorkInstruction>(&path) {
                    return Ok((path, wi));
                }
            }
        }

        // Fall back to directory scan
        let dir = self.get_directory();
        if let Some((path, wi)) = loader::load_entity::<WorkInstruction>(&dir, id)? {
            return Ok((path, wi));
        }
        Err(ServiceError::NotFound(id.to_string()))
    }

    /// Check if a work instruction matches the given filter
    fn matches_filter(&self, wi: &WorkInstruction, filter: &WorkInstructionFilter) -> bool {
        // Process filter
        if let Some(process) = &filter.process {
            if !wi
                .links
                .process
                .as_ref()
                .is_some_and(|p| p.to_string().contains(process))
            {
                return false;
            }
        }

        // Has safety filter
        if filter.has_safety && wi.safety.is_none() {
            return false;
        }

        // Has quality checks filter
        if filter.has_quality_checks && wi.quality_checks.is_empty() {
            return false;
        }

        // Document number filter
        if let Some(doc_num) = &filter.document_number {
            if !wi
                .document_number
                .as_ref()
                .is_some_and(|d| d.contains(doc_num))
            {
                return false;
            }
        }

        // Common filters
        if !filter.common.matches_status(&wi.status) {
            return false;
        }
        if !filter.common.matches_author(&wi.author) {
            return false;
        }
        if !filter.common.matches_tags(&wi.tags) {
            return false;
        }
        if !filter.common.matches_search(&[&wi.title]) {
            return false;
        }
        if !filter.common.matches_recent(&wi.created) {
            return false;
        }

        true
    }

    /// Sort work instructions by the given field
    fn sort_work_instructions(
        &self,
        instructions: &mut [WorkInstruction],
        sort_by: WorkInstructionSortField,
        sort_dir: SortDirection,
    ) {
        instructions.sort_by(|a, b| {
            let cmp = match sort_by {
                WorkInstructionSortField::Id => a.id.to_string().cmp(&b.id.to_string()),
                WorkInstructionSortField::Title => a.title.cmp(&b.title),
                WorkInstructionSortField::DocumentNumber => {
                    a.document_number.cmp(&b.document_number)
                }
                WorkInstructionSortField::Status => {
                    format!("{:?}", a.status).cmp(&format!("{:?}", b.status))
                }
                WorkInstructionSortField::Author => a.author.cmp(&b.author),
                WorkInstructionSortField::Created => a.created.cmp(&b.created),
                WorkInstructionSortField::StepCount => a.procedure.len().cmp(&b.procedure.len()),
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
    use crate::entities::work_instruction::{Hazard, PpeItem};
    use tempfile::TempDir;

    fn setup_test_project() -> (TempDir, Project, EntityCache) {
        let tmp = TempDir::new().unwrap();

        // Initialize project structure
        fs::create_dir_all(tmp.path().join(".tdt")).unwrap();
        fs::create_dir_all(tmp.path().join("manufacturing/work_instructions")).unwrap();

        // Create config file
        fs::write(tmp.path().join(".tdt/config.yaml"), "author: Test Author\n").unwrap();

        let project = Project::discover_from(tmp.path()).unwrap();
        let cache = EntityCache::open(&project).unwrap();

        (tmp, project, cache)
    }

    #[test]
    fn test_create_work_instruction() {
        let (_tmp, project, cache) = setup_test_project();
        let service = WorkInstructionService::new(&project, &cache);

        let input = CreateWorkInstruction {
            title: "CNC Mill Setup".into(),
            author: "Test Author".into(),
            document_number: Some("WI-MACH-001".into()),
            ..Default::default()
        };

        let wi = service.create(input).unwrap();

        assert_eq!(wi.title, "CNC Mill Setup");
        assert_eq!(wi.document_number, Some("WI-MACH-001".to_string()));
        assert_eq!(wi.status, Status::Draft);
    }

    #[test]
    fn test_get_work_instruction() {
        let (_tmp, project, cache) = setup_test_project();
        let service = WorkInstructionService::new(&project, &cache);

        let created = service
            .create(CreateWorkInstruction {
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
    fn test_update_work_instruction() {
        let (_tmp, project, cache) = setup_test_project();
        let service = WorkInstructionService::new(&project, &cache);

        let created = service
            .create(CreateWorkInstruction {
                title: "Original".into(),
                author: "Test".into(),
                ..Default::default()
            })
            .unwrap();

        let updated = service
            .update(
                &created.id.to_string(),
                UpdateWorkInstruction {
                    title: Some("Updated Title".into()),
                    document_number: Some("WI-001".into()),
                    ..Default::default()
                },
            )
            .unwrap();

        assert_eq!(updated.title, "Updated Title");
        assert_eq!(updated.document_number, Some("WI-001".to_string()));
        assert_eq!(updated.entity_revision, 2);
    }

    #[test]
    fn test_delete_work_instruction() {
        let (_tmp, project, cache) = setup_test_project();
        let service = WorkInstructionService::new(&project, &cache);

        let created = service
            .create(CreateWorkInstruction {
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
    fn test_add_step() {
        let (_tmp, project, cache) = setup_test_project();
        let service = WorkInstructionService::new(&project, &cache);

        let created = service
            .create(CreateWorkInstruction {
                title: "With Steps".into(),
                author: "Test".into(),
                ..Default::default()
            })
            .unwrap();

        let step1 = ProcedureStep {
            step: 1,
            action: "Prepare workpiece".into(),
            verification: Some("Verify dimensions".into()),
            caution: None,
            image: None,
            estimated_time_minutes: Some(5.0),
            ..Default::default()
        };

        let step2 = ProcedureStep {
            step: 2,
            action: "Load into machine".into(),
            verification: None,
            caution: Some("Watch fingers!".into()),
            image: None,
            estimated_time_minutes: Some(3.0),
            ..Default::default()
        };

        service.add_step(&created.id.to_string(), step1).unwrap();
        let updated = service.add_step(&created.id.to_string(), step2).unwrap();

        assert_eq!(updated.procedure.len(), 2);
        assert_eq!(updated.procedure[0].action, "Prepare workpiece");
        assert_eq!(updated.procedure[1].action, "Load into machine");
    }

    #[test]
    fn test_remove_step() {
        let (_tmp, project, cache) = setup_test_project();
        let service = WorkInstructionService::new(&project, &cache);

        let created = service
            .create(CreateWorkInstruction {
                title: "Remove Step Test".into(),
                author: "Test".into(),
                ..Default::default()
            })
            .unwrap();

        service
            .add_step(
                &created.id.to_string(),
                ProcedureStep {
                    step: 1,
                    action: "Step 1".into(),
                    ..Default::default()
                },
            )
            .unwrap();
        service
            .add_step(
                &created.id.to_string(),
                ProcedureStep {
                    step: 2,
                    action: "Step 2".into(),
                    ..Default::default()
                },
            )
            .unwrap();
        service
            .add_step(
                &created.id.to_string(),
                ProcedureStep {
                    step: 3,
                    action: "Step 3".into(),
                    ..Default::default()
                },
            )
            .unwrap();

        let updated = service.remove_step(&created.id.to_string(), 2).unwrap();

        assert_eq!(updated.procedure.len(), 2);
        // Steps should be renumbered
        assert_eq!(updated.procedure[0].step, 1);
        assert_eq!(updated.procedure[1].step, 2);
        assert_eq!(updated.procedure[1].action, "Step 3");
    }

    #[test]
    fn test_add_tool() {
        let (_tmp, project, cache) = setup_test_project();
        let service = WorkInstructionService::new(&project, &cache);

        let created = service
            .create(CreateWorkInstruction {
                title: "With Tools".into(),
                author: "Test".into(),
                ..Default::default()
            })
            .unwrap();

        let tool = Tool {
            name: "Torque Wrench".into(),
            part_number: Some("TW-25".into()),
        };

        let updated = service.add_tool(&created.id.to_string(), tool).unwrap();

        assert_eq!(updated.tools_required.len(), 1);
        assert_eq!(updated.tools_required[0].name, "Torque Wrench");
    }

    #[test]
    fn test_remove_tool() {
        let (_tmp, project, cache) = setup_test_project();
        let service = WorkInstructionService::new(&project, &cache);

        let created = service
            .create(CreateWorkInstruction {
                title: "Remove Tool Test".into(),
                author: "Test".into(),
                ..Default::default()
            })
            .unwrap();

        service
            .add_tool(
                &created.id.to_string(),
                Tool {
                    name: "Wrench".into(),
                    part_number: None,
                },
            )
            .unwrap();
        service
            .add_tool(
                &created.id.to_string(),
                Tool {
                    name: "Hammer".into(),
                    part_number: None,
                },
            )
            .unwrap();

        let updated = service
            .remove_tool(&created.id.to_string(), "Wrench")
            .unwrap();

        assert_eq!(updated.tools_required.len(), 1);
        assert_eq!(updated.tools_required[0].name, "Hammer");
    }

    #[test]
    fn test_add_material() {
        let (_tmp, project, cache) = setup_test_project();
        let service = WorkInstructionService::new(&project, &cache);

        let created = service
            .create(CreateWorkInstruction {
                title: "With Materials".into(),
                author: "Test".into(),
                ..Default::default()
            })
            .unwrap();

        let material = Material {
            name: "Cutting fluid".into(),
            specification: Some("ISO 6743".into()),
        };

        let updated = service
            .add_material(&created.id.to_string(), material)
            .unwrap();

        assert_eq!(updated.materials_required.len(), 1);
        assert_eq!(updated.materials_required[0].name, "Cutting fluid");
    }

    #[test]
    fn test_add_quality_check() {
        let (_tmp, project, cache) = setup_test_project();
        let service = WorkInstructionService::new(&project, &cache);

        let created = service
            .create(CreateWorkInstruction {
                title: "With QC".into(),
                author: "Test".into(),
                ..Default::default()
            })
            .unwrap();

        let check = QualityCheck {
            at_step: 3,
            characteristic: "Hole diameter".into(),
            specification: Some("10.0 +/- 0.1 mm".into()),
        };

        let updated = service
            .add_quality_check(&created.id.to_string(), check)
            .unwrap();

        assert_eq!(updated.quality_checks.len(), 1);
        assert_eq!(updated.quality_checks[0].characteristic, "Hole diameter");
    }

    #[test]
    fn test_set_safety() {
        let (_tmp, project, cache) = setup_test_project();
        let service = WorkInstructionService::new(&project, &cache);

        let created = service
            .create(CreateWorkInstruction {
                title: "With Safety".into(),
                author: "Test".into(),
                ..Default::default()
            })
            .unwrap();

        let safety = WorkSafety {
            ppe_required: vec![
                PpeItem {
                    item: "Safety glasses".into(),
                    standard: Some("ANSI Z87.1".into()),
                },
                PpeItem {
                    item: "Hearing protection".into(),
                    standard: None,
                },
            ],
            hazards: vec![Hazard {
                hazard: "Flying chips".into(),
                control: Some("Machine guarding".into()),
            }],
        };

        let updated = service.set_safety(&created.id.to_string(), safety).unwrap();

        assert!(updated.safety.is_some());
        let safety = updated.safety.unwrap();
        assert_eq!(safety.ppe_required.len(), 2);
        assert_eq!(safety.hazards.len(), 1);
    }

    #[test]
    fn test_list_with_filter() {
        let (_tmp, project, cache) = setup_test_project();
        let service = WorkInstructionService::new(&project, &cache);

        // Create work instructions
        let wi1 = service
            .create(CreateWorkInstruction {
                title: "Machining Setup".into(),
                author: "Test".into(),
                document_number: Some("WI-MACH-001".into()),
                ..Default::default()
            })
            .unwrap();

        service
            .create(CreateWorkInstruction {
                title: "Assembly Procedure".into(),
                author: "Test".into(),
                document_number: Some("WI-ASM-001".into()),
                ..Default::default()
            })
            .unwrap();

        // Add safety to first one
        service
            .set_safety(
                &wi1.id.to_string(),
                WorkSafety {
                    ppe_required: vec![PpeItem {
                        item: "Gloves".into(),
                        standard: None,
                    }],
                    hazards: Vec::new(),
                },
            )
            .unwrap();

        // List all
        let all = service
            .list(
                &WorkInstructionFilter::default(),
                WorkInstructionSortField::Title,
                SortDirection::Ascending,
            )
            .unwrap();
        assert_eq!(all.items.len(), 2);

        // List with safety
        let with_safety = service
            .list(
                &WorkInstructionFilter::with_safety(),
                WorkInstructionSortField::Title,
                SortDirection::Ascending,
            )
            .unwrap();
        assert_eq!(with_safety.items.len(), 1);
        assert_eq!(with_safety.items[0].title, "Machining Setup");
    }

    #[test]
    fn test_stats() {
        let (_tmp, project, cache) = setup_test_project();
        let service = WorkInstructionService::new(&project, &cache);

        // Create work instruction with steps and tools
        let wi = service
            .create(CreateWorkInstruction {
                title: "Stats Test".into(),
                author: "Test".into(),
                ..Default::default()
            })
            .unwrap();

        service
            .add_step(
                &wi.id.to_string(),
                ProcedureStep {
                    step: 1,
                    action: "Step 1".into(),
                    ..Default::default()
                },
            )
            .unwrap();
        service
            .add_step(
                &wi.id.to_string(),
                ProcedureStep {
                    step: 2,
                    action: "Step 2".into(),
                    ..Default::default()
                },
            )
            .unwrap();
        service
            .add_tool(
                &wi.id.to_string(),
                Tool {
                    name: "Wrench".into(),
                    part_number: None,
                },
            )
            .unwrap();

        let stats = service.stats().unwrap();

        assert_eq!(stats.total, 1);
        assert_eq!(stats.by_status.draft, 1);
        assert_eq!(stats.with_procedure_steps, 1);
        assert_eq!(stats.total_steps, 2);
        assert_eq!(stats.with_tools, 1);
        assert_eq!(stats.avg_steps_per_instruction, 2.0);
    }

    #[test]
    fn test_calculate_estimated_duration() {
        let (_tmp, project, cache) = setup_test_project();
        let service = WorkInstructionService::new(&project, &cache);

        let wi = service
            .create(CreateWorkInstruction {
                title: "Duration Test".into(),
                author: "Test".into(),
                ..Default::default()
            })
            .unwrap();

        service
            .add_step(
                &wi.id.to_string(),
                ProcedureStep {
                    step: 1,
                    action: "Step 1".into(),
                    estimated_time_minutes: Some(5.0),
                    ..Default::default()
                },
            )
            .unwrap();
        service
            .add_step(
                &wi.id.to_string(),
                ProcedureStep {
                    step: 2,
                    action: "Step 2".into(),
                    estimated_time_minutes: Some(10.0),
                    ..Default::default()
                },
            )
            .unwrap();
        service
            .add_step(
                &wi.id.to_string(),
                ProcedureStep {
                    step: 3,
                    action: "Step 3".into(),
                    estimated_time_minutes: Some(3.0),
                    ..Default::default()
                },
            )
            .unwrap();

        let duration = service
            .calculate_estimated_duration(&wi.id.to_string())
            .unwrap();

        assert_eq!(duration, Some(18.0)); // 5 + 10 + 3
    }
}
