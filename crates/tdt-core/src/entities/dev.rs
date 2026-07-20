//! DEV entity type - Process Deviation
//!
//! Deviations document pre-approved departures from standard processes.
//! Key distinction from NCR: NCR is reactive (something went wrong),
//! DEV is proactive (intentionally doing something different, with approval).

use crate::core::suspect::LinkRef;
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

use crate::core::entity::{Entity, Status};
use crate::core::identity::EntityId;

/// Deviation type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum DeviationType {
    /// Time-limited deviation with expiration date
    #[default]
    Temporary,
    /// Requires formal change control (links to ECO)
    Permanent,
    /// Immediate action needed, expedited approval
    Emergency,
}

impl std::fmt::Display for DeviationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeviationType::Temporary => write!(f, "temporary"),
            DeviationType::Permanent => write!(f, "permanent"),
            DeviationType::Emergency => write!(f, "emergency"),
        }
    }
}

impl std::str::FromStr for DeviationType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "temporary" | "temp" => Ok(DeviationType::Temporary),
            "permanent" | "perm" => Ok(DeviationType::Permanent),
            "emergency" | "emerg" => Ok(DeviationType::Emergency),
            _ => Err(format!(
                "Invalid deviation type: {}. Use temporary, permanent, or emergency",
                s
            )),
        }
    }
}

/// Deviation status (workflow state)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum DevStatus {
    /// Awaiting approval
    #[default]
    Pending,
    /// Approved but not yet effective
    Approved,
    /// Currently in effect
    Active,
    /// Past expiration date
    Expired,
    /// Manually closed before expiration
    Closed,
    /// Deviation request rejected
    Rejected,
}

impl std::fmt::Display for DevStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DevStatus::Pending => write!(f, "pending"),
            DevStatus::Approved => write!(f, "approved"),
            DevStatus::Active => write!(f, "active"),
            DevStatus::Expired => write!(f, "expired"),
            DevStatus::Closed => write!(f, "closed"),
            DevStatus::Rejected => write!(f, "rejected"),
        }
    }
}

impl std::str::FromStr for DevStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pending" => Ok(DevStatus::Pending),
            "approved" => Ok(DevStatus::Approved),
            "active" => Ok(DevStatus::Active),
            "expired" => Ok(DevStatus::Expired),
            "closed" => Ok(DevStatus::Closed),
            "rejected" => Ok(DevStatus::Rejected),
            _ => Err(format!(
                "Invalid deviation status: {}. Use pending, approved, active, expired, closed, or rejected",
                s
            )),
        }
    }
}

/// Deviation category
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum DeviationCategory {
    /// Material substitution
    #[default]
    Material,
    /// Process parameter change
    Process,
    /// Equipment substitution
    Equipment,
    /// Tooling modification
    Tooling,
    /// Specification deviation
    Specification,
    /// Documentation deviation
    Documentation,
}

impl std::fmt::Display for DeviationCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeviationCategory::Material => write!(f, "material"),
            DeviationCategory::Process => write!(f, "process"),
            DeviationCategory::Equipment => write!(f, "equipment"),
            DeviationCategory::Tooling => write!(f, "tooling"),
            DeviationCategory::Specification => write!(f, "specification"),
            DeviationCategory::Documentation => write!(f, "documentation"),
        }
    }
}

impl std::str::FromStr for DeviationCategory {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "material" | "mat" => Ok(DeviationCategory::Material),
            "process" | "proc" => Ok(DeviationCategory::Process),
            "equipment" | "equip" => Ok(DeviationCategory::Equipment),
            "tooling" | "tool" => Ok(DeviationCategory::Tooling),
            "specification" | "spec" => Ok(DeviationCategory::Specification),
            "documentation" | "doc" => Ok(DeviationCategory::Documentation),
            _ => Err(format!(
                "Invalid category: {}. Use material, process, equipment, tooling, specification, or documentation",
                s
            )),
        }
    }
}

/// Risk level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum RiskLevel {
    /// Minimal impact, low probability of issue
    #[default]
    Low,
    /// Some impact, requires monitoring
    Medium,
    /// Significant impact, requires additional controls
    High,
}

impl std::fmt::Display for RiskLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RiskLevel::Low => write!(f, "low"),
            RiskLevel::Medium => write!(f, "medium"),
            RiskLevel::High => write!(f, "high"),
        }
    }
}

impl std::str::FromStr for RiskLevel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "low" => Ok(RiskLevel::Low),
            "medium" | "med" => Ok(RiskLevel::Medium),
            "high" => Ok(RiskLevel::High),
            _ => Err(format!(
                "Invalid risk level: {}. Use low, medium, or high",
                s
            )),
        }
    }
}

/// Authorization level for approval
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum AuthorizationLevel {
    /// Engineering approval
    #[default]
    Engineering,
    /// Quality approval
    Quality,
    /// Management approval
    Management,
}

impl std::fmt::Display for AuthorizationLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthorizationLevel::Engineering => write!(f, "engineering"),
            AuthorizationLevel::Quality => write!(f, "quality"),
            AuthorizationLevel::Management => write!(f, "management"),
        }
    }
}

impl std::str::FromStr for AuthorizationLevel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "engineering" | "eng" => Ok(AuthorizationLevel::Engineering),
            "quality" | "qa" | "qe" => Ok(AuthorizationLevel::Quality),
            "management" | "mgmt" => Ok(AuthorizationLevel::Management),
            _ => Err(format!(
                "Invalid authorization level: {}. Use engineering, quality, or management",
                s
            )),
        }
    }
}

/// Risk assessment for deviation
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DevRisk {
    /// Risk level
    #[serde(default)]
    pub level: RiskLevel,

    /// Risk assessment text
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assessment: Option<String>,

    /// Mitigation measures
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mitigations: Vec<String>,
}

/// Approval information
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DevApproval {
    /// Who approved the deviation
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approved_by: Option<String>,

    /// Date of approval
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval_date: Option<NaiveDate>,

    /// Authorization level
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authorization_level: Option<AuthorizationLevel>,
}

/// Links for DEV entity
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DevLinks {
    /// Affected process entities
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub processes: Vec<LinkRef>,

    /// LOT entities this applies to
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lots: Vec<LinkRef>,

    /// Affected component entities
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub components: Vec<LinkRef>,

    /// Requirement entities being deviated from
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requirements: Vec<LinkRef>,

    /// Related NCRs (if deviation arose from NCR)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ncrs: Vec<LinkRef>,

    /// ECO/DCN reference if permanent
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub change_order: Option<LinkRef>,
}

/// Process Deviation entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dev {
    /// Unique identifier (DEV-xxx)
    pub id: EntityId,

    /// Descriptive title
    pub title: String,

    /// User-defined deviation number
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deviation_number: Option<String>,

    /// Deviation type
    #[serde(default)]
    pub deviation_type: DeviationType,

    /// Category
    #[serde(default)]
    pub category: DeviationCategory,

    /// Description of the deviation
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Risk assessment
    #[serde(default)]
    pub risk: DevRisk,

    /// Approval information
    #[serde(default)]
    pub approval: DevApproval,

    /// Effective date
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effective_date: Option<NaiveDate>,

    /// Expiration date (null for permanent)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expiration_date: Option<NaiveDate>,

    /// Deviation status
    #[serde(default)]
    pub dev_status: DevStatus,

    /// Notes
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,

    /// Entity links
    #[serde(default)]
    pub links: DevLinks,

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

impl Entity for Dev {
    const PREFIX: &'static str = "DEV";

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

impl Dev {
    /// Create a new Dev
    pub fn new(title: String, author: String) -> Self {
        Self {
            id: EntityId::new(crate::core::identity::EntityPrefix::Dev),
            title,
            deviation_number: None,
            deviation_type: DeviationType::default(),
            category: DeviationCategory::default(),
            description: None,
            risk: DevRisk::default(),
            approval: DevApproval::default(),
            effective_date: None,
            expiration_date: None,
            dev_status: DevStatus::default(),
            notes: None,
            links: DevLinks::default(),
            status: Status::Draft,
            created: Utc::now(),
            author,
            entity_revision: 1,
        }
    }

    /// Create a new Dev with deviation number
    pub fn with_deviation_number(title: String, deviation_number: String, author: String) -> Self {
        let mut dev = Self::new(title, author);
        dev.deviation_number = Some(deviation_number);
        dev
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dev_creation() {
        let dev = Dev::new("Test Deviation".to_string(), "Test Author".to_string());
        assert!(dev.id.to_string().starts_with("DEV-"));
        assert_eq!(dev.title, "Test Deviation");
        assert_eq!(dev.author, "Test Author");
        assert_eq!(dev.dev_status, DevStatus::Pending);
        assert_eq!(dev.deviation_type, DeviationType::Temporary);
    }

    #[test]
    fn test_deviation_type_parsing() {
        assert_eq!(
            "temporary".parse::<DeviationType>().unwrap(),
            DeviationType::Temporary
        );
        assert_eq!(
            "permanent".parse::<DeviationType>().unwrap(),
            DeviationType::Permanent
        );
        assert_eq!(
            "emergency".parse::<DeviationType>().unwrap(),
            DeviationType::Emergency
        );
    }

    #[test]
    fn test_dev_status_parsing() {
        assert_eq!("pending".parse::<DevStatus>().unwrap(), DevStatus::Pending);
        assert_eq!(
            "approved".parse::<DevStatus>().unwrap(),
            DevStatus::Approved
        );
        assert_eq!("active".parse::<DevStatus>().unwrap(), DevStatus::Active);
        assert_eq!("expired".parse::<DevStatus>().unwrap(), DevStatus::Expired);
        assert_eq!("closed".parse::<DevStatus>().unwrap(), DevStatus::Closed);
        assert_eq!(
            "rejected".parse::<DevStatus>().unwrap(),
            DevStatus::Rejected
        );
    }

    #[test]
    fn test_category_parsing() {
        assert_eq!(
            "material".parse::<DeviationCategory>().unwrap(),
            DeviationCategory::Material
        );
        assert_eq!(
            "process".parse::<DeviationCategory>().unwrap(),
            DeviationCategory::Process
        );
        assert_eq!(
            "equipment".parse::<DeviationCategory>().unwrap(),
            DeviationCategory::Equipment
        );
    }

    #[test]
    fn test_risk_level_parsing() {
        assert_eq!("low".parse::<RiskLevel>().unwrap(), RiskLevel::Low);
        assert_eq!("medium".parse::<RiskLevel>().unwrap(), RiskLevel::Medium);
        assert_eq!("high".parse::<RiskLevel>().unwrap(), RiskLevel::High);
    }

    #[test]
    fn test_dev_serialization() {
        let dev = Dev::new("Test Deviation".to_string(), "Test Author".to_string());
        let yaml = serde_yml::to_string(&dev).unwrap();
        assert!(yaml.contains("DEV-"));
        assert!(yaml.contains("Test Deviation"));
    }

    #[test]
    fn test_dev_deserialization() {
        let yaml = r#"
id: DEV-01HC2JB7SMQX7RS1Y0GFKBHPTD
title: "Material Substitution"
deviation_number: "DEV-2024-042"
deviation_type: temporary
category: material
description: |
  Substituting 316L for 304 stainless steel
risk:
  level: low
  assessment: "316L meets or exceeds requirements"
  mitigations:
    - "First article inspection"
approval:
  approved_by: "R. Williams"
  approval_date: 2024-01-15
  authorization_level: engineering
effective_date: 2024-01-16
expiration_date: 2024-03-15
dev_status: active
links:
  processes: []
  lots: []
status: approved
created: 2024-01-15T10:00:00Z
author: "J. Smith"
entity_revision: 1
"#;
        let dev: Dev = serde_yml::from_str(yaml).unwrap();
        assert_eq!(dev.title, "Material Substitution");
        assert_eq!(dev.deviation_number, Some("DEV-2024-042".to_string()));
        assert_eq!(dev.deviation_type, DeviationType::Temporary);
        assert_eq!(dev.category, DeviationCategory::Material);
        assert_eq!(dev.dev_status, DevStatus::Active);
        assert_eq!(dev.risk.level, RiskLevel::Low);
        assert_eq!(dev.risk.mitigations.len(), 1);
        assert!(dev.approval.approved_by.is_some());
    }
}
