//! Work Instruction entity type - Operator procedures and step-by-step instructions

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::core::entity::{Entity, Status};
use crate::core::identity::EntityId;

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
    pub process: Option<EntityId>,

    /// Control plan items referenced
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub controls: Vec<EntityId>,
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
}
