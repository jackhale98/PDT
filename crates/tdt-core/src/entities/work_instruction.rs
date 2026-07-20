//! Work Instruction entity type - Operator procedures and step-by-step instructions

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::core::entity::{Entity, Status};
use crate::core::identity::EntityId;
use crate::core::suspect::LinkRef;

/// PPE item requirement
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PpeItem {
    /// PPE item name
    pub item: String,

    /// Standard/specification (e.g., "ANSI Z87.1")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub standard: Option<String>,
}

/// Safety hazard and control
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Hazard {
    /// Hazard description
    pub hazard: String,

    /// Control measure
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub control: Option<String>,
}

/// Safety requirements for the work instruction
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkSafety {
    /// Required PPE items
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ppe_required: Vec<PpeItem>,

    /// Hazards present
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hazards: Vec<Hazard>,
}

/// Tool required for the procedure
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Tool {
    /// Tool name
    pub name: String,

    /// Part number or specification
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub part_number: Option<String>,
}

/// Material required for the procedure
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Material {
    /// Material name
    pub name: String,

    /// Specification or part number
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub specification: Option<String>,
}

/// Approval requirements for a procedure step (electronic router/traveler)
///
/// Used for regulated manufacturing environments (FDA 21 CFR Part 11, ISO 13485)
/// to define sign-off requirements at specific work instruction steps.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StepApprovalRequirement {
    /// Require sign-off after completing this step
    #[serde(default)]
    pub requires_signoff: bool,

    /// Minimum number of approvals required (default: 1)
    #[serde(
        default = "default_min_approvals",
        skip_serializing_if = "is_default_min_approvals"
    )]
    pub min_approvals: u32,

    /// Required roles for approval (any of these roles can approve)
    /// Values: "engineering", "quality", "management", "admin"
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_roles: Vec<String>,

    /// Specific team members who must approve (by username from team roster)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_approvers: Vec<String>,

    /// Require digital signature (GPG/SSH/gitsign) for 21 CFR Part 11 compliance
    #[serde(default)]
    pub require_signature: bool,

    /// Whether this is a quality hold point (production stops until approved)
    #[serde(default)]
    pub quality_hold_point: bool,
}

fn default_min_approvals() -> u32 {
    1
}

fn is_default_min_approvals(val: &u32) -> bool {
    *val == 1
}

impl Default for StepApprovalRequirement {
    fn default() -> Self {
        Self {
            requires_signoff: false,
            min_approvals: 1, // Must match serde default
            required_roles: Vec::new(),
            required_approvers: Vec::new(),
            require_signature: false,
            quality_hold_point: false,
        }
    }
}

impl StepApprovalRequirement {
    /// Create a simple sign-off requirement (no specific roles)
    pub fn signoff() -> Self {
        Self {
            requires_signoff: true,
            ..Default::default()
        }
    }

    /// Create a quality hold point with role requirement
    pub fn quality_hold(roles: Vec<String>) -> Self {
        Self {
            requires_signoff: true,
            required_roles: roles,
            quality_hold_point: true,
            ..Default::default()
        }
    }

    /// Create a signed approval requirement for 21 CFR Part 11
    pub fn signed_approval(roles: Vec<String>) -> Self {
        Self {
            requires_signoff: true,
            required_roles: roles,
            require_signature: true,
            ..Default::default()
        }
    }
}

/// Data field definition for a procedure step
///
/// Defines data to be collected during step execution (e.g., measurements, serial numbers)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StepDataField {
    /// Field key/name
    pub key: String,

    /// Human-readable label/description
    pub label: String,

    /// Data type: "text", "number", "boolean", "select"
    #[serde(default = "default_data_type")]
    pub data_type: String,

    /// Whether this field is required
    #[serde(default)]
    pub required: bool,

    /// Units for numeric fields (e.g., "mm", "Nm")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub units: Option<String>,

    /// Allowed values for select type
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<String>,
}

fn default_data_type() -> String {
    "text".to_string()
}

impl Default for StepDataField {
    fn default() -> Self {
        Self {
            key: String::new(),
            label: String::new(),
            data_type: "text".to_string(), // Must match serde default
            required: false,
            units: None,
            options: Vec::new(),
        }
    }
}

/// Procedure step
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProcedureStep {
    /// Step number
    pub step: u32,

    /// Action to perform
    pub action: String,

    /// Verification/check point
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verification: Option<String>,

    /// Caution/warning note
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caution: Option<String>,

    /// Image reference
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,

    /// Estimated time in minutes
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimated_time_minutes: Option<f64>,

    // === Electronic Router Fields ===
    /// Approval requirements for this step (if any)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval: Option<StepApprovalRequirement>,

    /// Data fields to collect at this step
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub data_fields: Vec<StepDataField>,

    /// Equipment/tools required for this step (for traceability - require serial number at execution)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub equipment: Vec<String>,
}

impl ProcedureStep {
    /// Create a new procedure step
    pub fn new(step: u32, action: String) -> Self {
        Self {
            step,
            action,
            ..Default::default()
        }
    }

    /// Add a verification point
    pub fn with_verification(mut self, verification: String) -> Self {
        self.verification = Some(verification);
        self
    }

    /// Add a caution note
    pub fn with_caution(mut self, caution: String) -> Self {
        self.caution = Some(caution);
        self
    }

    /// Add approval requirements
    pub fn with_approval(mut self, approval: StepApprovalRequirement) -> Self {
        self.approval = Some(approval);
        self
    }

    /// Add a data field to collect
    pub fn with_data_field(mut self, key: String, label: String) -> Self {
        self.data_fields.push(StepDataField {
            key,
            label,
            ..Default::default()
        });
        self
    }

    /// Check if this step requires approval
    pub fn requires_approval(&self) -> bool {
        self.approval.as_ref().is_some_and(|a| a.requires_signoff)
    }

    /// Check if this step is a quality hold point
    pub fn is_hold_point(&self) -> bool {
        self.approval.as_ref().is_some_and(|a| a.quality_hold_point)
    }
}

/// In-process quality check
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QualityCheck {
    /// Step number where check occurs
    pub at_step: u32,

    /// Characteristic to check
    pub characteristic: String,

    /// Specification/tolerance
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub specification: Option<String>,
}

/// Links to other entities
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkInstructionLinks {
    /// Parent process
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process: Option<LinkRef>,

    /// Control plan items referenced
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub controls: Vec<LinkRef>,
}

/// A Work Instruction entity - operator procedure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkInstruction {
    /// Unique identifier
    pub id: EntityId,

    /// Work instruction title
    pub title: String,

    /// Document number (e.g., "WI-MACH-015")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document_number: Option<String>,

    /// Document revision
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,

    /// Detailed description/purpose
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Safety requirements
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub safety: Option<WorkSafety>,

    /// Tools required
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools_required: Vec<Tool>,

    /// Materials required
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub materials_required: Vec<Material>,

    /// Procedure steps
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub procedure: Vec<ProcedureStep>,

    /// Quality checks during procedure
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub quality_checks: Vec<QualityCheck>,

    /// Estimated total duration in minutes
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimated_duration_minutes: Option<f64>,

    /// Tags for filtering
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,

    /// Current status
    #[serde(default)]
    pub status: Status,

    /// Links to other entities
    #[serde(default)]
    pub links: WorkInstructionLinks,

    /// Creation timestamp
    pub created: DateTime<Utc>,

    /// Author (who created this work instruction)
    pub author: String,

    /// Entity revision number
    #[serde(default = "default_revision")]
    pub entity_revision: u32,
}

fn default_revision() -> u32 {
    1
}

impl Entity for WorkInstruction {
    const PREFIX: &'static str = "WORK";

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

impl WorkInstruction {
    /// Create a new work instruction with the given parameters
    pub fn new(title: String, author: String) -> Self {
        Self {
            id: EntityId::new(crate::core::EntityPrefix::Work),
            title,
            document_number: None,
            revision: None,
            description: None,
            safety: None,
            tools_required: Vec::new(),
            materials_required: Vec::new(),
            procedure: Vec::new(),
            quality_checks: Vec::new(),
            estimated_duration_minutes: None,
            tags: Vec::new(),
            status: Status::default(),
            links: WorkInstructionLinks::default(),
            created: Utc::now(),
            author,
            entity_revision: 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_work_instruction_creation() {
        let work = WorkInstruction::new("CNC Mill Setup".to_string(), "test".to_string());

        assert!(work.id.to_string().starts_with("WORK-"));
        assert_eq!(work.title, "CNC Mill Setup");
    }

    #[test]
    fn test_work_instruction_roundtrip() {
        let work = WorkInstruction::new("Assembly Procedure".to_string(), "test".to_string());

        let yaml = serde_yml::to_string(&work).unwrap();
        let parsed: WorkInstruction = serde_yml::from_str(&yaml).unwrap();

        assert_eq!(work.id, parsed.id);
        assert_eq!(work.title, parsed.title);
    }

    #[test]
    fn test_entity_trait_implementation() {
        let work = WorkInstruction::new(
            "Test Work Instruction".to_string(),
            "test_author".to_string(),
        );

        assert_eq!(WorkInstruction::PREFIX, "WORK");
        assert_eq!(work.title(), "Test Work Instruction");
        assert_eq!(work.status(), "draft");
        assert_eq!(work.author(), "test_author");
    }

    // === Electronic Router Tests ===

    #[test]
    fn test_step_approval_requirement_default() {
        let req = StepApprovalRequirement::default();
        assert!(!req.requires_signoff);
        assert_eq!(req.min_approvals, 1);
        assert!(req.required_roles.is_empty());
        assert!(req.required_approvers.is_empty());
        assert!(!req.require_signature);
        assert!(!req.quality_hold_point);
    }

    #[test]
    fn test_step_approval_requirement_signoff() {
        let req = StepApprovalRequirement::signoff();
        assert!(req.requires_signoff);
        assert_eq!(req.min_approvals, 1);
        assert!(req.required_roles.is_empty());
    }

    #[test]
    fn test_step_approval_requirement_quality_hold() {
        let req = StepApprovalRequirement::quality_hold(vec!["quality".to_string()]);
        assert!(req.requires_signoff);
        assert!(req.quality_hold_point);
        assert_eq!(req.required_roles, vec!["quality"]);
    }

    #[test]
    fn test_step_approval_requirement_signed() {
        let req = StepApprovalRequirement::signed_approval(vec![
            "quality".to_string(),
            "engineering".to_string(),
        ]);
        assert!(req.requires_signoff);
        assert!(req.require_signature);
        assert_eq!(req.required_roles.len(), 2);
    }

    #[test]
    fn test_step_approval_requirement_serialization() {
        let req = StepApprovalRequirement {
            requires_signoff: true,
            min_approvals: 2,
            required_roles: vec!["quality".to_string()],
            require_signature: true,
            quality_hold_point: true,
            ..Default::default()
        };

        let yaml = serde_yml::to_string(&req).unwrap();
        assert!(yaml.contains("requires_signoff: true"));
        assert!(yaml.contains("min_approvals: 2"));
        assert!(yaml.contains("quality"));
        assert!(yaml.contains("require_signature: true"));
        assert!(yaml.contains("quality_hold_point: true"));

        let parsed: StepApprovalRequirement = serde_yml::from_str(&yaml).unwrap();
        assert_eq!(req, parsed);
    }

    #[test]
    fn test_step_approval_requirement_min_approvals_default_not_serialized() {
        let req = StepApprovalRequirement {
            requires_signoff: true,
            min_approvals: 1, // Default value
            ..Default::default()
        };

        let yaml = serde_yml::to_string(&req).unwrap();
        // min_approvals: 1 should not appear in output (it's the default)
        assert!(!yaml.contains("min_approvals"));
    }

    #[test]
    fn test_step_data_field_default() {
        let field = StepDataField::default();
        assert!(field.key.is_empty());
        assert!(field.label.is_empty());
        assert_eq!(field.data_type, "text");
        assert!(!field.required);
        assert!(field.units.is_none());
        assert!(field.options.is_empty());
    }

    #[test]
    fn test_step_data_field_serialization() {
        let field = StepDataField {
            key: "torque_value".to_string(),
            label: "Torque (Nm)".to_string(),
            data_type: "number".to_string(),
            required: true,
            units: Some("Nm".to_string()),
            options: vec![],
        };

        let yaml = serde_yml::to_string(&field).unwrap();
        let parsed: StepDataField = serde_yml::from_str(&yaml).unwrap();
        assert_eq!(field, parsed);
    }

    #[test]
    fn test_procedure_step_new() {
        let step = ProcedureStep::new(1, "Load raw material".to_string());
        assert_eq!(step.step, 1);
        assert_eq!(step.action, "Load raw material");
        assert!(step.verification.is_none());
        assert!(step.approval.is_none());
        assert!(step.data_fields.is_empty());
        assert!(step.equipment.is_empty());
    }

    #[test]
    fn test_procedure_step_builder_pattern() {
        let step = ProcedureStep::new(3, "Torque screws".to_string())
            .with_verification("All screws at 25 Nm".to_string())
            .with_caution("Do not over-torque".to_string())
            .with_approval(StepApprovalRequirement::quality_hold(vec![
                "quality".to_string()
            ]))
            .with_data_field(
                "screw_1_torque".to_string(),
                "Screw 1 Torque (Nm)".to_string(),
            );

        assert_eq!(step.step, 3);
        assert_eq!(step.verification.as_deref(), Some("All screws at 25 Nm"));
        assert_eq!(step.caution.as_deref(), Some("Do not over-torque"));
        assert!(step.requires_approval());
        assert!(step.is_hold_point());
        assert_eq!(step.data_fields.len(), 1);
        assert_eq!(step.data_fields[0].key, "screw_1_torque");
    }

    #[test]
    fn test_procedure_step_requires_approval() {
        let step_no_approval = ProcedureStep::new(1, "Simple step".to_string());
        assert!(!step_no_approval.requires_approval());
        assert!(!step_no_approval.is_hold_point());

        let step_with_approval = ProcedureStep::new(2, "Critical step".to_string())
            .with_approval(StepApprovalRequirement::signoff());
        assert!(step_with_approval.requires_approval());
        assert!(!step_with_approval.is_hold_point());

        let step_hold_point = ProcedureStep::new(3, "Quality check".to_string()).with_approval(
            StepApprovalRequirement::quality_hold(vec!["quality".to_string()]),
        );
        assert!(step_hold_point.requires_approval());
        assert!(step_hold_point.is_hold_point());
    }

    #[test]
    fn test_procedure_step_with_approval_serialization() {
        let step = ProcedureStep {
            step: 4,
            action: "Inspect critical dimensions".to_string(),
            verification: Some("All dims within spec".to_string()),
            approval: Some(StepApprovalRequirement {
                requires_signoff: true,
                required_roles: vec!["quality".to_string()],
                require_signature: true,
                quality_hold_point: true,
                ..Default::default()
            }),
            data_fields: vec![StepDataField {
                key: "dim_a".to_string(),
                label: "Dimension A (mm)".to_string(),
                data_type: "number".to_string(),
                required: true,
                units: Some("mm".to_string()),
                options: vec![],
            }],
            equipment: vec!["Caliper (Cal# CAL-001)".to_string()],
            ..Default::default()
        };

        let yaml = serde_yml::to_string(&step).unwrap();
        assert!(yaml.contains("step: 4"));
        assert!(yaml.contains("Inspect critical dimensions"));
        assert!(yaml.contains("approval:"));
        assert!(yaml.contains("quality_hold_point: true"));
        assert!(yaml.contains("data_fields:"));
        assert!(yaml.contains("dim_a"));
        assert!(yaml.contains("equipment:"));
        assert!(yaml.contains("Caliper"));

        let parsed: ProcedureStep = serde_yml::from_str(&yaml).unwrap();
        assert_eq!(parsed.step, 4);
        assert!(parsed.requires_approval());
        assert!(parsed.is_hold_point());
        assert_eq!(parsed.data_fields.len(), 1);
        assert_eq!(parsed.equipment.len(), 1);
    }

    #[test]
    fn test_work_instruction_with_router_steps() {
        let mut work =
            WorkInstruction::new("Assembly with QA holds".to_string(), "test".to_string());

        work.procedure = vec![
            ProcedureStep::new(1, "Load components".to_string()),
            ProcedureStep::new(2, "Apply adhesive".to_string())
                .with_caution("Use within 30 min of mixing".to_string()),
            ProcedureStep::new(3, "Verify bond".to_string())
                .with_verification("Bond passes pull test".to_string())
                .with_approval(StepApprovalRequirement::quality_hold(vec![
                    "quality".to_string()
                ]))
                .with_data_field("pull_force".to_string(), "Pull Force (N)".to_string()),
            ProcedureStep::new(4, "Final assembly".to_string()),
        ];

        let yaml = serde_yml::to_string(&work).unwrap();
        let parsed: WorkInstruction = serde_yml::from_str(&yaml).unwrap();

        assert_eq!(parsed.procedure.len(), 4);
        assert!(!parsed.procedure[0].requires_approval());
        assert!(!parsed.procedure[1].requires_approval());
        assert!(parsed.procedure[2].requires_approval());
        assert!(parsed.procedure[2].is_hold_point());
        assert_eq!(parsed.procedure[2].data_fields.len(), 1);
        assert!(!parsed.procedure[3].requires_approval());
    }
}
