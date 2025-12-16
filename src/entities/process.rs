//! Process entity type - Manufacturing process definitions

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::core::entity::{Entity, Status};
use crate::core::identity::EntityId;

/// Process type classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum ProcessType {
    #[default]
    Machining,
    Assembly,
    Inspection,
    Test,
    Finishing,
    Packaging,
    Handling,
    HeatTreat,
    Welding,
    Coating,
}

impl std::fmt::Display for ProcessType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProcessType::Machining => write!(f, "machining"),
            ProcessType::Assembly => write!(f, "assembly"),
            ProcessType::Inspection => write!(f, "inspection"),
            ProcessType::Test => write!(f, "test"),
            ProcessType::Finishing => write!(f, "finishing"),
            ProcessType::Packaging => write!(f, "packaging"),
            ProcessType::Handling => write!(f, "handling"),
            ProcessType::HeatTreat => write!(f, "heat_treat"),
            ProcessType::Welding => write!(f, "welding"),
            ProcessType::Coating => write!(f, "coating"),
        }
    }
}

impl std::str::FromStr for ProcessType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "machining" => Ok(ProcessType::Machining),
            "assembly" => Ok(ProcessType::Assembly),
            "inspection" => Ok(ProcessType::Inspection),
            "test" => Ok(ProcessType::Test),
            "finishing" => Ok(ProcessType::Finishing),
            "packaging" => Ok(ProcessType::Packaging),
            "handling" => Ok(ProcessType::Handling),
            "heat_treat" | "heatttreat" => Ok(ProcessType::HeatTreat),
            "welding" => Ok(ProcessType::Welding),
            "coating" => Ok(ProcessType::Coating),
            _ => Err(format!(
                "Invalid process type: {}. Use machining, assembly, inspection, test, finishing, packaging, handling, heat_treat, welding, or coating",
                s
            )),
        }
    }
}

/// Operator skill level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum SkillLevel {
    Entry,
    #[default]
    Intermediate,
    Advanced,
    Expert,
}

impl std::fmt::Display for SkillLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SkillLevel::Entry => write!(f, "entry"),
            SkillLevel::Intermediate => write!(f, "intermediate"),
            SkillLevel::Advanced => write!(f, "advanced"),
            SkillLevel::Expert => write!(f, "expert"),
        }
    }
}

/// Equipment used in the process
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Equipment {
    /// Equipment name
    pub name: String,

    /// Equipment ID / asset number
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub equipment_id: Option<String>,

    /// Capability or specification required
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capability: Option<String>,
}

/// Process parameter
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProcessParameter {
    /// Parameter name (e.g., "Spindle Speed")
    pub name: String,

    /// Parameter value
    pub value: f64,

    /// Units (e.g., "RPM", "mm/min")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub units: Option<String>,

    /// Minimum allowed value
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,

    /// Maximum allowed value
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
}

/// Process capability data
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProcessCapability {
    /// Process capability index (Cpk)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpk: Option<f64>,

    /// Process performance index (Ppk)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ppk: Option<f64>,

    /// Sample size used for study
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample_size: Option<u32>,

    /// Date of capability study
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub study_date: Option<chrono::NaiveDate>,
}

/// Safety information
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProcessSafety {
    /// Required PPE items
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ppe: Vec<String>,

    /// Hazards present
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hazards: Vec<String>,
}

/// Links to other entities
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProcessLinks {
    /// Components produced by this process
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub produces: Vec<EntityId>,

    /// Control plan items for this process
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub controls: Vec<EntityId>,

    /// Work instructions for this process
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub work_instructions: Vec<EntityId>,

    /// Related risks
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub risks: Vec<EntityId>,

    /// CAPAs that modified this process (reciprocal of CAPA.processes_modified)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub modified_by_capa: Vec<EntityId>,

    /// Related entities
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub related_to: Vec<EntityId>,
}

/// A Process entity - manufacturing process definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Process {
    /// Unique identifier
    pub id: EntityId,

    /// Process title
    pub title: String,

    /// Detailed description
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Process type classification
    #[serde(default)]
    pub process_type: ProcessType,

    /// Operation number (e.g., "OP-010")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_number: Option<String>,

    /// Equipment used
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub equipment: Vec<Equipment>,

    /// Process parameters
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parameters: Vec<ProcessParameter>,

    /// Cycle time in minutes
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cycle_time_minutes: Option<f64>,

    /// Setup time in minutes
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub setup_time_minutes: Option<f64>,

    /// Process capability data
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capability: Option<ProcessCapability>,

    /// Required operator skill level
    #[serde(default)]
    pub operator_skill: SkillLevel,

    /// Safety information
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub safety: Option<ProcessSafety>,

    /// Tags for filtering
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,

    /// Current status
    #[serde(default)]
    pub status: Status,

    /// Links to other entities
    #[serde(default)]
    pub links: ProcessLinks,

    /// Creation timestamp
    pub created: DateTime<Utc>,

    /// Author (who created this process)
    pub author: String,

    /// Entity revision number
    #[serde(default = "default_revision")]
    pub entity_revision: u32,
}

fn default_revision() -> u32 {
    1
}

impl Entity for Process {
    const PREFIX: &'static str = "PROC";

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

impl Process {
    /// Create a new process with the given parameters
    pub fn new(title: String, process_type: ProcessType, author: String) -> Self {
        Self {
            id: EntityId::new(crate::core::EntityPrefix::Proc),
            title,
            description: None,
            process_type,
            operation_number: None,
            equipment: Vec::new(),
            parameters: Vec::new(),
            cycle_time_minutes: None,
            setup_time_minutes: None,
            capability: None,
            operator_skill: SkillLevel::default(),
            safety: None,
            tags: Vec::new(),
            status: Status::default(),
            links: ProcessLinks::default(),
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
    fn test_process_creation() {
        let proc = Process::new(
            "CNC Milling".to_string(),
            ProcessType::Machining,
            "test".to_string(),
        );

        assert!(proc.id.to_string().starts_with("PROC-"));
        assert_eq!(proc.title, "CNC Milling");
        assert_eq!(proc.process_type, ProcessType::Machining);
    }

    #[test]
    fn test_process_roundtrip() {
        let proc = Process::new(
            "Assembly Op".to_string(),
            ProcessType::Assembly,
            "test".to_string(),
        );

        let yaml = serde_yml::to_string(&proc).unwrap();
        let parsed: Process = serde_yml::from_str(&yaml).unwrap();

        assert_eq!(proc.id, parsed.id);
        assert_eq!(proc.title, parsed.title);
        assert_eq!(proc.process_type, parsed.process_type);
    }

    #[test]
    fn test_process_type_serialization() {
        let proc = Process::new(
            "Heat Treat".to_string(),
            ProcessType::HeatTreat,
            "test".to_string(),
        );

        let yaml = serde_yml::to_string(&proc).unwrap();
        assert!(yaml.contains("process_type: heat_treat"));
    }

    #[test]
    fn test_entity_trait_implementation() {
        let proc = Process::new(
            "Test Process".to_string(),
            ProcessType::Inspection,
            "test_author".to_string(),
        );

        assert_eq!(Process::PREFIX, "PROC");
        assert_eq!(proc.title(), "Test Process");
        assert_eq!(proc.status(), "draft");
        assert_eq!(proc.author(), "test_author");
    }

    #[test]
    fn test_process_type_from_str() {
        assert_eq!(
            "machining".parse::<ProcessType>().unwrap(),
            ProcessType::Machining
        );
        assert_eq!(
            "heat_treat".parse::<ProcessType>().unwrap(),
            ProcessType::HeatTreat
        );
        assert!("invalid".parse::<ProcessType>().is_err());
    }
}
