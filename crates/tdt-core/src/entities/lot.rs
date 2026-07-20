//! LOT entity type - Production Lot / Batch (Device History Record)

use crate::core::suspect::LinkRef;
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::core::entity::{Entity, Status};
use crate::core::identity::EntityId;

/// Lot status (production state)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum LotStatus {
    #[default]
    InProgress,
    OnHold,
    Completed,
    Scrapped,
}

impl std::fmt::Display for LotStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LotStatus::InProgress => write!(f, "in_progress"),
            LotStatus::OnHold => write!(f, "on_hold"),
            LotStatus::Completed => write!(f, "completed"),
            LotStatus::Scrapped => write!(f, "scrapped"),
        }
    }
}

impl std::str::FromStr for LotStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "in_progress" | "inprogress" => Ok(LotStatus::InProgress),
            "on_hold" | "onhold" => Ok(LotStatus::OnHold),
            "completed" => Ok(LotStatus::Completed),
            "scrapped" => Ok(LotStatus::Scrapped),
            _ => Err(format!(
                "Invalid lot status: {}. Use in_progress, on_hold, completed, or scrapped",
                s
            )),
        }
    }
}

/// Execution step status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum ExecutionStatus {
    #[default]
    Pending,
    InProgress,
    Completed,
    Skipped,
}

impl std::fmt::Display for ExecutionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecutionStatus::Pending => write!(f, "pending"),
            ExecutionStatus::InProgress => write!(f, "in_progress"),
            ExecutionStatus::Completed => write!(f, "completed"),
            ExecutionStatus::Skipped => write!(f, "skipped"),
        }
    }
}

impl std::str::FromStr for ExecutionStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pending" => Ok(ExecutionStatus::Pending),
            "in_progress" | "inprogress" => Ok(ExecutionStatus::InProgress),
            "completed" => Ok(ExecutionStatus::Completed),
            "skipped" => Ok(ExecutionStatus::Skipped),
            _ => Err(format!(
                "Invalid execution status: {}. Use pending, in_progress, completed, or skipped",
                s
            )),
        }
    }
}

/// Approval status for step (PR-based quality sign-off)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum ApprovalStatus {
    #[default]
    NotRequired,
    Pending,
    Approved,
    Rejected,
}

impl std::fmt::Display for ApprovalStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApprovalStatus::NotRequired => write!(f, "not_required"),
            ApprovalStatus::Pending => write!(f, "pending"),
            ApprovalStatus::Approved => write!(f, "approved"),
            ApprovalStatus::Rejected => write!(f, "rejected"),
        }
    }
}

/// Reference to a work instruction used during step execution
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkInstructionRef {
    /// Work instruction ID (WORK-xxx)
    pub id: String,

    /// Revision at time of execution
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<u32>,
}

/// Individual approval record for a step
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct StepApproval {
    /// Approver name
    pub approver: String,

    /// Approver email
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,

    /// Role of approver (e.g., "quality", "engineering")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,

    /// Timestamp of approval
    pub timestamp: DateTime<Utc>,

    /// Comment/reason for approval
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,

    /// Whether signature was verified
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature_verified: Option<bool>,

    /// Signing key ID (GPG/SSH)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signing_key: Option<String>,
}

/// Material used in production (for traceability)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MaterialUsed {
    /// Component ID (CMP-xxx)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub component: Option<String>,

    /// Supplier lot number (free text for traceability)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supplier_lot: Option<String>,

    /// Quantity used
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quantity: Option<u32>,
}

/// Record of a single work instruction step execution within a lot
///
/// Provides granular tracking at the WI procedure step level for electronic router/traveler
/// functionality. Each step can have its own operator sign-off, data collection, and approvals.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct WiStepExecution {
    /// Work instruction ID (WORK-xxx)
    pub work_instruction: String,

    /// Step number within the work instruction
    pub step_number: u32,

    /// Operator who performed the step
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operator: Option<String>,

    /// Operator email
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operator_email: Option<String>,

    /// Whether operator signature was verified (21 CFR Part 11 compliance)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operator_signature_verified: Option<bool>,

    /// Signing key ID (GPG/SSH) used for operator signature
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signing_key: Option<String>,

    /// Completion timestamp
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<DateTime<Utc>>,

    /// Collected data from step data_fields requirements
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub data: HashMap<String, serde_json::Value>,

    /// Equipment used with serial numbers (for calibration traceability)
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub equipment_used: HashMap<String, String>,

    /// Approvals for this step (if required by WI step approval requirements)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub approvals: Vec<StepApproval>,

    /// Approval status
    #[serde(default)]
    pub approval_status: ApprovalStatus,

    /// Git commit SHA for this step completion (audit trail)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit_sha: Option<String>,

    /// Notes about this step execution
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

impl WiStepExecution {
    /// Create a new WI step execution record
    pub fn new(work_instruction: String, step_number: u32) -> Self {
        Self {
            work_instruction,
            step_number,
            ..Default::default()
        }
    }

    /// Create with operator info
    pub fn with_operator(mut self, operator: String, email: Option<String>) -> Self {
        self.operator = Some(operator);
        self.operator_email = email;
        self
    }

    /// Mark as completed now
    pub fn complete(mut self) -> Self {
        self.completed_at = Some(Utc::now());
        self
    }

    /// Mark as completed with timestamp
    pub fn complete_at(mut self, timestamp: DateTime<Utc>) -> Self {
        self.completed_at = Some(timestamp);
        self
    }

    /// Add data value
    pub fn with_data(mut self, key: String, value: serde_json::Value) -> Self {
        self.data.insert(key, value);
        self
    }

    /// Add equipment used
    pub fn with_equipment(mut self, equipment: String, serial: String) -> Self {
        self.equipment_used.insert(equipment, serial);
        self
    }

    /// Mark operator signature as verified
    pub fn with_signature(mut self, signing_key: String) -> Self {
        self.operator_signature_verified = Some(true);
        self.signing_key = Some(signing_key);
        self
    }

    /// Add approval record
    pub fn add_approval(&mut self, approval: StepApproval) {
        self.approvals.push(approval);
    }

    /// Check if step is completed (has completed_at timestamp)
    pub fn is_completed(&self) -> bool {
        self.completed_at.is_some()
    }

    /// Check if step requires approval (has pending or awaiting approval)
    pub fn requires_approval(&self) -> bool {
        self.approval_status == ApprovalStatus::Pending
    }

    /// Check if step is fully approved
    pub fn is_approved(&self) -> bool {
        self.approval_status == ApprovalStatus::Approved
    }
}

/// Execution step record (DHR compliant)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExecutionStep {
    /// Process ID (PROC-xxx)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process: Option<String>,

    /// Process entity revision at time of execution
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process_revision: Option<u32>,

    /// Work instructions used during this step
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub work_instructions_used: Vec<WorkInstructionRef>,

    /// Execution status
    #[serde(default)]
    pub status: ExecutionStatus,

    /// Date step was started
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_date: Option<NaiveDate>,

    /// Date step was completed
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_date: Option<NaiveDate>,

    /// Operator who performed the step
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operator: Option<String>,

    /// Operator email
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operator_email: Option<String>,

    /// Notes about execution
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,

    /// Whether operator signature was verified (DHR compliance)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature_verified: Option<bool>,

    /// Signing key ID (GPG/SSH) used for operator signature
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signing_key: Option<String>,

    /// Git commit SHA for this step completion
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit_sha: Option<String>,

    /// Approval status for PR-based sign-off
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval_status: Option<ApprovalStatus>,

    /// Approval records for this step
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub approvals: Vec<StepApproval>,

    /// GitHub/GitLab PR number for approval workflow
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pr_number: Option<u64>,

    /// Measurement/inspection data (key-value pairs)
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub data: HashMap<String, serde_json::Value>,

    /// Detailed WI step executions (for electronic router/traveler)
    /// Provides granular tracking at the procedure step level within work instructions
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub wi_step_executions: Vec<WiStepExecution>,
}

/// Links for LOT entity
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LotLinks {
    /// Product being made (ASM or CMP ID)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub product: Option<LinkRef>,

    /// Process entities in sequence
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub processes: Vec<LinkRef>,

    /// Work instruction entities
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub work_instructions: Vec<LinkRef>,

    /// NCRs raised during production
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ncrs: Vec<LinkRef>,

    /// In-process inspection results
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub results: Vec<LinkRef>,
}

/// Production Lot / Batch entity (Device History Record)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lot {
    /// Unique identifier (LOT-xxx)
    pub id: EntityId,

    /// Descriptive title
    pub title: String,

    /// User-defined lot number
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lot_number: Option<String>,

    /// Quantity in this lot
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quantity: Option<u32>,

    /// Production status
    #[serde(default)]
    pub lot_status: LotStatus,

    /// Production start date
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_date: Option<NaiveDate>,

    /// Production completion date
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completion_date: Option<NaiveDate>,

    /// Materials used (for traceability)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub materials_used: Vec<MaterialUsed>,

    /// Process execution records
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub execution: Vec<ExecutionStep>,

    /// Production notes
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,

    /// Git branch name for this lot's DHR workflow
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub git_branch: Option<String>,

    /// Whether the lot branch has been merged to main
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub branch_merged: bool,

    /// Entity links
    #[serde(default)]
    pub links: LotLinks,

    /// Document status
    #[serde(default)]
    pub status: Status,

    /// Creation timestamp
    pub created: DateTime<Utc>,

    /// Author
    pub author: String,

    /// Entity revision number
    #[serde(default = "default_revision")]
    pub entity_revision: u32,
}

fn default_revision() -> u32 {
    1
}

impl Entity for Lot {
    const PREFIX: &'static str = "LOT";

    fn id(&self) -> &EntityId {
        &self.id
    }

    fn title(&self) -> &str {
        &self.title
    }

    fn status(&self) -> &str {
        match self.status {
            Status::Draft => "draft",
            Status::Review => "review",
            Status::Approved => "approved",
            Status::Released => "released",
            Status::Obsolete => "obsolete",
        }
    }

    fn created(&self) -> DateTime<Utc> {
        self.created
    }

    fn author(&self) -> &str {
        &self.author
    }
}

impl Lot {
    /// Create a new Lot
    pub fn new(title: String, author: String) -> Self {
        Self {
            id: EntityId::new(crate::core::identity::EntityPrefix::Lot),
            title,
            lot_number: None,
            quantity: None,
            lot_status: LotStatus::default(),
            start_date: None,
            completion_date: None,
            materials_used: Vec::new(),
            execution: Vec::new(),
            notes: None,
            git_branch: None,
            branch_merged: false,
            links: LotLinks::default(),
            status: Status::Draft,
            created: Utc::now(),
            author,
            entity_revision: 1,
        }
    }

    /// Create a new Lot with lot number
    pub fn with_lot_number(title: String, lot_number: String, author: String) -> Self {
        let mut lot = Self::new(title, author);
        lot.lot_number = Some(lot_number);
        lot
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lot_creation() {
        let lot = Lot::new("Test Lot".to_string(), "Test Author".to_string());
        assert!(lot.id.to_string().starts_with("LOT-"));
        assert_eq!(lot.title, "Test Lot");
        assert_eq!(lot.author, "Test Author");
        assert_eq!(lot.lot_status, LotStatus::InProgress);
    }

    #[test]
    fn test_lot_status_parsing() {
        assert_eq!(
            "in_progress".parse::<LotStatus>().unwrap(),
            LotStatus::InProgress
        );
        assert_eq!(
            "completed".parse::<LotStatus>().unwrap(),
            LotStatus::Completed
        );
        assert_eq!("on_hold".parse::<LotStatus>().unwrap(), LotStatus::OnHold);
        assert_eq!(
            "scrapped".parse::<LotStatus>().unwrap(),
            LotStatus::Scrapped
        );
    }

    #[test]
    fn test_lot_serialization() {
        let lot = Lot::new("Test Lot".to_string(), "Test Author".to_string());
        let yaml = serde_yml::to_string(&lot).unwrap();
        assert!(yaml.contains("LOT-"));
        assert!(yaml.contains("Test Lot"));
    }

    #[test]
    fn test_lot_deserialization() {
        let yaml = r#"
id: LOT-01HC2JB7SMQX7RS1Y0GFKBHPTD
title: "Production Lot 001"
lot_number: "2024-001"
quantity: 25
lot_status: in_progress
materials_used:
  - component: CMP-01HC2JB7SMQX7RS1Y0GFKBHPTE
    supplier_lot: "ABC-123"
    quantity: 25
execution: []
links:
  product: ASM-01HC2JB7SMQX7RS1Y0GFKBHPTF
  processes: []
status: draft
created: 2024-01-15T10:00:00Z
author: "Test Author"
entity_revision: 1
"#;
        let lot: Lot = serde_yml::from_str(yaml).unwrap();
        assert_eq!(lot.title, "Production Lot 001");
        assert_eq!(lot.lot_number, Some("2024-001".to_string()));
        assert_eq!(lot.quantity, Some(25));
        assert_eq!(lot.lot_status, LotStatus::InProgress);
        assert_eq!(lot.materials_used.len(), 1);
    }

    // === WiStepExecution Tests ===

    #[test]
    fn test_wi_step_execution_new() {
        let exec = WiStepExecution::new("WORK-01ABC".to_string(), 1);
        assert_eq!(exec.work_instruction, "WORK-01ABC");
        assert_eq!(exec.step_number, 1);
        assert!(exec.operator.is_none());
        assert!(exec.completed_at.is_none());
        assert!(exec.data.is_empty());
        assert!(exec.equipment_used.is_empty());
        assert_eq!(exec.approval_status, ApprovalStatus::NotRequired);
    }

    #[test]
    fn test_wi_step_execution_builder_pattern() {
        let exec = WiStepExecution::new("WORK-01ABC".to_string(), 3)
            .with_operator("jsmith".to_string(), Some("jsmith@example.com".to_string()))
            .with_data("torque_nm".to_string(), serde_json::json!(25.5))
            .with_equipment(
                "Torque Wrench TW-001".to_string(),
                "CAL-2024-001".to_string(),
            )
            .with_signature("GPG:jsmith-key".to_string())
            .complete();

        assert_eq!(exec.work_instruction, "WORK-01ABC");
        assert_eq!(exec.step_number, 3);
        assert_eq!(exec.operator, Some("jsmith".to_string()));
        assert_eq!(exec.operator_email, Some("jsmith@example.com".to_string()));
        assert!(exec.completed_at.is_some());
        assert_eq!(exec.data.get("torque_nm"), Some(&serde_json::json!(25.5)));
        assert_eq!(
            exec.equipment_used.get("Torque Wrench TW-001"),
            Some(&"CAL-2024-001".to_string())
        );
        assert_eq!(exec.operator_signature_verified, Some(true));
        assert_eq!(exec.signing_key, Some("GPG:jsmith-key".to_string()));
    }

    #[test]
    fn test_wi_step_execution_status_checks() {
        let mut exec = WiStepExecution::new("WORK-01ABC".to_string(), 1);
        assert!(!exec.is_completed());
        assert!(!exec.requires_approval());
        assert!(!exec.is_approved());

        // Complete the step
        exec = exec.complete();
        assert!(exec.is_completed());

        // Set approval pending
        exec.approval_status = ApprovalStatus::Pending;
        assert!(exec.requires_approval());
        assert!(!exec.is_approved());

        // Approve
        exec.approval_status = ApprovalStatus::Approved;
        assert!(!exec.requires_approval());
        assert!(exec.is_approved());
    }

    #[test]
    fn test_wi_step_execution_add_approval() {
        let mut exec = WiStepExecution::new("WORK-01ABC".to_string(), 4);
        exec.approval_status = ApprovalStatus::Pending;

        let approval = StepApproval {
            approver: "bwilson".to_string(),
            email: Some("bwilson@example.com".to_string()),
            role: Some("quality".to_string()),
            timestamp: Utc::now(),
            comment: Some("Dimensions verified within spec".to_string()),
            signature_verified: Some(true),
            signing_key: Some("SSH:bwilson-key".to_string()),
        };

        exec.add_approval(approval);
        exec.approval_status = ApprovalStatus::Approved;

        assert_eq!(exec.approvals.len(), 1);
        assert_eq!(exec.approvals[0].approver, "bwilson");
        assert_eq!(exec.approvals[0].role, Some("quality".to_string()));
        assert!(exec.is_approved());
    }

    #[test]
    fn test_wi_step_execution_serialization() {
        let exec = WiStepExecution::new("WORK-01ABC".to_string(), 2)
            .with_operator("jsmith".to_string(), None)
            .with_data("measurement_mm".to_string(), serde_json::json!(12.5))
            .complete();

        let yaml = serde_yml::to_string(&exec).unwrap();
        assert!(yaml.contains("work_instruction: WORK-01ABC"));
        assert!(yaml.contains("step_number: 2"));
        assert!(yaml.contains("jsmith"));

        let parsed: WiStepExecution = serde_yml::from_str(&yaml).unwrap();
        assert_eq!(exec, parsed);
    }

    #[test]
    fn test_wi_step_execution_in_execution_step() {
        let mut execution_step = ExecutionStep {
            process: Some("PROC-01ABC".to_string()),
            status: ExecutionStatus::InProgress,
            ..Default::default()
        };

        let wi_exec1 = WiStepExecution::new("WORK-01ABC".to_string(), 1)
            .with_operator("jsmith".to_string(), None)
            .complete();

        let wi_exec2 = WiStepExecution::new("WORK-01ABC".to_string(), 2)
            .with_operator("jsmith".to_string(), None);

        execution_step.wi_step_executions.push(wi_exec1);
        execution_step.wi_step_executions.push(wi_exec2);

        assert_eq!(execution_step.wi_step_executions.len(), 2);
        assert!(execution_step.wi_step_executions[0].is_completed());
        assert!(!execution_step.wi_step_executions[1].is_completed());
    }

    #[test]
    fn test_lot_with_wi_step_executions_deserialization() {
        let yaml = r#"
id: LOT-01HC2JB7SMQX7RS1Y0GFKBHPTD
title: "Production Lot 001"
lot_status: in_progress
execution:
  - process: PROC-01ABC
    status: in_progress
    work_instructions_used:
      - id: WORK-01XYZ
        revision: 2
    wi_step_executions:
      - work_instruction: WORK-01XYZ
        step_number: 1
        operator: jsmith
        completed_at: 2024-01-15T10:00:00Z
        data:
          torque_nm: 25.5
        equipment_used:
          "Torque Wrench TW-001": "CAL-2024-001"
      - work_instruction: WORK-01XYZ
        step_number: 2
        operator: jsmith
        approval_status: pending
links: {}
status: draft
created: 2024-01-15T10:00:00Z
author: "Test Author"
entity_revision: 1
"#;
        let lot: Lot = serde_yml::from_str(yaml).unwrap();
        assert_eq!(lot.execution.len(), 1);
        assert_eq!(lot.execution[0].wi_step_executions.len(), 2);
        assert_eq!(lot.execution[0].wi_step_executions[0].step_number, 1);
        assert!(lot.execution[0].wi_step_executions[0].is_completed());
        assert_eq!(
            lot.execution[0].wi_step_executions[1].approval_status,
            ApprovalStatus::Pending
        );
    }
}
