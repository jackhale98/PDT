//! Manufacturing workflow support for lot management
//!
//! Provides git branch-based workflow for lot execution with DHR compliance.

use std::collections::HashMap;
use std::path::Path;

use crate::core::git::{Git, GitError};
use crate::core::Config;
use crate::entities::lot::{ExecutionStatus, ExecutionStep, Lot, WorkInstructionRef};
use crate::entities::process::Process;

/// Configuration for lot-based manufacturing workflow
#[derive(Debug, Clone)]
pub struct LotWorkflowConfig {
    /// Base branch to create lot branches from (default: "main")
    pub base_branch: String,

    /// Pattern for lot branch names (default: "lot/{lot_number}")
    pub branch_pattern: String,

    /// Whether to sign commits
    pub sign_commits: bool,

    /// Whether to create tags at lot lifecycle events
    pub create_tags: bool,
}

impl Default for LotWorkflowConfig {
    fn default() -> Self {
        Self {
            base_branch: "main".to_string(),
            branch_pattern: "lot/{lot_number}".to_string(),
            sign_commits: false,
            create_tags: true,
        }
    }
}

impl LotWorkflowConfig {
    /// Create config from the main Config struct
    pub fn from_config(config: &Config) -> Self {
        Self {
            base_branch: config
                .manufacturing
                .as_ref()
                .and_then(|m| m.base_branch.clone())
                .unwrap_or_else(|| "main".to_string()),
            branch_pattern: config
                .manufacturing
                .as_ref()
                .and_then(|m| m.branch_pattern.clone())
                .unwrap_or_else(|| "lot/{lot_number}".to_string()),
            sign_commits: config
                .manufacturing
                .as_ref()
                .map(|m| m.sign_commits)
                .unwrap_or(false),
            create_tags: config
                .manufacturing
                .as_ref()
                .map(|m| m.create_tags)
                .unwrap_or(true),
        }
    }

    /// Generate branch name for a lot
    pub fn branch_name(&self, lot: &Lot) -> String {
        let lot_number = lot
            .lot_number
            .clone()
            .unwrap_or_else(|| Self::id_short(&lot.id));
        self.branch_pattern.replace("{lot_number}", &lot_number)
    }

    /// Get a shortened version of an entity ID for display
    fn id_short(id: &crate::core::identity::EntityId) -> String {
        // Get the string representation and take first portion for brevity
        let full = id.to_string();
        // LOT-01HQ... -> take prefix + first 8 chars of ULID
        if full.len() > 12 {
            format!("{}...", &full[..12])
        } else {
            full
        }
    }
}

/// Manufacturing workflow engine for lot management
pub struct LotWorkflow<'a> {
    git: &'a Git,
    config: LotWorkflowConfig,
}

impl<'a> LotWorkflow<'a> {
    /// Create a new LotWorkflow instance
    pub fn new(git: &'a Git, config: LotWorkflowConfig) -> Self {
        Self { git, config }
    }

    /// Initialize a git branch for a lot
    ///
    /// Creates a new branch from base and optionally creates a start tag.
    pub fn init_lot_branch(&self, lot: &Lot) -> Result<String, GitError> {
        let branch_name = self.config.branch_name(lot);

        // Ensure we're on the base branch
        if !self.git.is_clean() {
            return Err(GitError::UncommittedChanges);
        }

        // Checkout base branch and create lot branch
        self.git.checkout_branch(&self.config.base_branch)?;
        self.git.create_and_checkout_branch(&branch_name)?;

        // Create start tag if enabled
        if self.config.create_tags {
            let tag_name = format!("{}/start", branch_name);
            let lot_id_str = LotWorkflowConfig::id_short(&lot.id);
            let message = format!(
                "Lot {} started",
                lot.lot_number.as_deref().unwrap_or(&lot_id_str)
            );
            self.git.create_tag(&tag_name, Some(&message))?;
        }

        Ok(branch_name)
    }

    /// Commit a step completion to the lot branch
    ///
    /// Records the step execution in git history.
    pub fn commit_step_completion(
        &self,
        lot: &Lot,
        step_idx: usize,
        operator: &str,
        files_changed: &[&Path],
        sign: bool,
    ) -> Result<String, GitError> {
        // Stage the changed files
        self.git.stage_files(files_changed)?;

        let step = lot.execution.get(step_idx);
        let process_info = step
            .and_then(|s| s.process.as_ref())
            .map(|p| p.as_str())
            .unwrap_or("unknown");

        let lot_id_str = LotWorkflowConfig::id_short(&lot.id);
        let message = format!(
            "lot({}): Step {} completed by {}\n\nProcess: {}",
            lot.lot_number.as_deref().unwrap_or(&lot_id_str),
            step_idx + 1,
            operator,
            process_info
        );

        let commit_sha = if sign || self.config.sign_commits {
            self.git.commit_signed(&message)?
        } else {
            self.git.commit(&message)?
        };

        Ok(commit_sha)
    }

    /// Complete a lot and optionally merge the branch
    ///
    /// Merges the lot branch back to base and creates completion tag.
    pub fn complete_lot(&self, lot: &Lot, sign: bool) -> Result<String, GitError> {
        let branch_name = self.config.branch_name(lot);
        let lot_id_str = LotWorkflowConfig::id_short(&lot.id);

        // Checkout base branch
        self.git.checkout_branch(&self.config.base_branch)?;

        // Merge the lot branch
        let merge_message = format!(
            "Merge lot {} ({})\n\nCompleted manufacturing for lot.",
            lot.lot_number.as_deref().unwrap_or(&lot_id_str),
            branch_name
        );

        let merge_sha = if sign || self.config.sign_commits {
            self.git.merge_branch_signed(&branch_name, &merge_message)?
        } else {
            self.git.merge_branch(&branch_name, &merge_message)?
        };

        // Create completion tag if enabled
        if self.config.create_tags {
            let tag_name = format!("{}/complete", branch_name);
            let tag_message = format!(
                "Lot {} completed",
                lot.lot_number.as_deref().unwrap_or(&lot_id_str)
            );
            if sign || self.config.sign_commits {
                self.git.create_signed_tag(&tag_name, &tag_message)?;
            } else {
                self.git.create_tag(&tag_name, Some(&tag_message))?;
            }
        }

        // Optionally delete the lot branch
        // For now, keep it for reference

        Ok(merge_sha)
    }

    /// Check if lot branch exists
    pub fn lot_branch_exists(&self, lot: &Lot) -> bool {
        let branch_name = self.config.branch_name(lot);
        self.git.branch_exists(&branch_name)
    }

    /// Switch to the lot's branch
    pub fn checkout_lot_branch(&self, lot: &Lot) -> Result<(), GitError> {
        let branch_name = self.config.branch_name(lot);
        self.git.checkout_branch(&branch_name)
    }
}

/// Create execution steps from a product's manufacturing routing
///
/// Loads each process in the routing and creates pending execution steps
/// with the process revision captured at creation time.
pub fn create_execution_steps_from_routing(
    routing: &[String],
    processes: &HashMap<String, Process>,
) -> Vec<ExecutionStep> {
    routing
        .iter()
        .filter_map(|proc_id| {
            processes.get(proc_id).map(|proc| {
                let mut step = ExecutionStep::default();
                step.process = Some(proc_id.clone());
                step.process_revision = Some(proc.entity_revision);
                step.status = ExecutionStatus::Pending;

                // Auto-populate work instructions from process links
                step.work_instructions_used = proc
                    .links
                    .work_instructions
                    .iter()
                    .map(|wi_id| WorkInstructionRef {
                        id: wi_id.to_string(),
                        revision: None, // Will be set when step is executed
                    })
                    .collect();

                step
            })
        })
        .collect()
}

/// Check if a step requires operator signature based on its process
pub fn step_requires_signature(step: &ExecutionStep, processes: &HashMap<String, Process>) -> bool {
    step.process
        .as_ref()
        .and_then(|proc_id| processes.get(proc_id))
        .map(|proc| proc.require_signature)
        .unwrap_or(false)
}

/// Check if a step requires PR-based approval based on its process
pub fn step_requires_approval(step: &ExecutionStep, processes: &HashMap<String, Process>) -> bool {
    step.process
        .as_ref()
        .and_then(|proc_id| processes.get(proc_id))
        .and_then(|proc| proc.step_approval.as_ref())
        .map(|approval| approval.require_approval)
        .unwrap_or(false)
}

/// Get minimum approvals required for a step
pub fn step_min_approvals(step: &ExecutionStep, processes: &HashMap<String, Process>) -> u32 {
    step.process
        .as_ref()
        .and_then(|proc_id| processes.get(proc_id))
        .and_then(|proc| proc.step_approval.as_ref())
        .map(|approval| approval.min_approvals)
        .unwrap_or(1)
}

/// Get required roles for step approval
pub fn step_required_roles(
    step: &ExecutionStep,
    processes: &HashMap<String, Process>,
) -> Vec<String> {
    step.process
        .as_ref()
        .and_then(|proc_id| processes.get(proc_id))
        .and_then(|proc| proc.step_approval.as_ref())
        .map(|approval| approval.required_roles.clone())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lot_workflow_config_default() {
        let config = LotWorkflowConfig::default();
        assert_eq!(config.base_branch, "main");
        assert_eq!(config.branch_pattern, "lot/{lot_number}");
        assert!(!config.sign_commits);
        assert!(config.create_tags);
    }

    #[test]
    fn test_branch_name_generation() {
        let config = LotWorkflowConfig::default();
        let mut lot = Lot::new("Test Lot".to_string(), "author".to_string());
        lot.lot_number = Some("2024-001".to_string());

        let branch_name = config.branch_name(&lot);
        assert_eq!(branch_name, "lot/2024-001");
    }

    #[test]
    fn test_branch_name_fallback_to_id() {
        let config = LotWorkflowConfig::default();
        let lot = Lot::new("Test Lot".to_string(), "author".to_string());

        let branch_name = config.branch_name(&lot);
        assert!(branch_name.starts_with("lot/"));
    }
}
