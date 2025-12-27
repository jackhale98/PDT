//! Requirement entity type

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::core::entity::{Entity, Priority, Status};
use crate::core::identity::EntityId;

/// Requirement type - design input or output
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum RequirementType {
    #[default]
    Input,
    Output,
}

impl std::fmt::Display for RequirementType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RequirementType::Input => write!(f, "input"),
            RequirementType::Output => write!(f, "output"),
        }
    }
}

/// Requirement level in V-model hierarchy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum Level {
    /// Stakeholder/user needs - top of V-model
    Stakeholder,
    /// System-level requirements
    #[default]
    System,
    /// Subsystem-level requirements
    Subsystem,
    /// Component-level requirements
    Component,
    /// Detailed implementation requirements
    Detail,
}

impl std::fmt::Display for Level {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Level::Stakeholder => write!(f, "stakeholder"),
            Level::System => write!(f, "system"),
            Level::Subsystem => write!(f, "subsystem"),
            Level::Component => write!(f, "component"),
            Level::Detail => write!(f, "detail"),
        }
    }
}

impl std::str::FromStr for Level {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "stakeholder" => Ok(Level::Stakeholder),
            "system" => Ok(Level::System),
            "subsystem" => Ok(Level::Subsystem),
            "component" => Ok(Level::Component),
            "detail" => Ok(Level::Detail),
            _ => Err(format!("Unknown level: {}", s)),
        }
    }
}

/// Source reference for a requirement
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Source {
    /// Source document name
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub document: String,

    /// Document revision
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub revision: String,

    /// Section reference
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub section: String,

    /// Date of the source
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<chrono::NaiveDate>,
}

/// Links to other entities
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Links {
    /// Design outputs that satisfy this input
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub satisfied_by: Vec<EntityId>,

    /// Tests that verify this requirement
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub verified_by: Vec<EntityId>,

    /// Parent requirements this derives from (requirement decomposition)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub derives_from: Vec<EntityId>,

    /// Child requirements derived from this one (reciprocal of derives_from)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub derived_by: Vec<EntityId>,

    /// Features this requirement is allocated to
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allocated_to: Vec<EntityId>,
}

/// A requirement entity (design input or output)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Requirement {
    /// Unique identifier
    pub id: EntityId,

    /// Requirement type (input or output)
    #[serde(rename = "type")]
    pub req_type: RequirementType,

    /// Requirement level in V-model hierarchy
    #[serde(default)]
    pub level: Level,

    /// Short title
    pub title: String,

    /// Source reference
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<Source>,

    /// Category (user-defined)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,

    /// Tags for filtering
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,

    /// Full requirement text
    pub text: String,

    /// Rationale for this requirement
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rationale: Option<String>,

    /// Acceptance criteria
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub acceptance_criteria: Vec<String>,

    /// Priority level
    #[serde(default)]
    pub priority: Priority,

    /// Current status
    #[serde(default)]
    pub status: Status,

    /// Links to other entities
    #[serde(default)]
    pub links: Links,

    /// Creation timestamp
    pub created: DateTime<Utc>,

    /// Author (who created this requirement)
    pub author: String,

    /// Revision number
    #[serde(default = "default_revision")]
    pub revision: u32,
}

fn default_revision() -> u32 {
    1
}

impl Entity for Requirement {
    const PREFIX: &'static str = "REQ";

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

impl Requirement {
    /// Create a new requirement with the given parameters
    pub fn new(req_type: RequirementType, title: String, text: String, author: String) -> Self {
        Self {
            id: EntityId::new(crate::core::EntityPrefix::Req),
            req_type,
            level: Level::default(),
            title,
            source: None,
            category: None,
            tags: Vec::new(),
            text,
            rationale: None,
            acceptance_criteria: Vec::new(),
            priority: Priority::default(),
            status: Status::default(),
            links: Links::default(),
            created: Utc::now(),
            author,
            revision: 1,
        }
    }

    /// Create a new requirement with a specific level
    pub fn with_level(mut self, level: Level) -> Self {
        self.level = level;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_requirement_roundtrip() {
        let req = Requirement::new(
            RequirementType::Input,
            "Test Requirement".to_string(),
            "The system shall do something.".to_string(),
            "test".to_string(),
        );

        let yaml = serde_yml::to_string(&req).unwrap();
        let parsed: Requirement = serde_yml::from_str(&yaml).unwrap();

        assert_eq!(req.id, parsed.id);
        assert_eq!(req.title, parsed.title);
        assert_eq!(req.text, parsed.text);
    }

    #[test]
    fn test_requirement_serializes_type_correctly() {
        let req = Requirement::new(
            RequirementType::Input,
            "Test".to_string(),
            "Text".to_string(),
            "test".to_string(),
        );

        let yaml = serde_yml::to_string(&req).unwrap();
        assert!(yaml.contains("type: input"));
    }

    // ========== Level enum tests ==========

    #[test]
    fn test_level_default_is_system() {
        assert_eq!(Level::default(), Level::System);
    }

    #[test]
    fn test_level_serialization() {
        // Test all variants serialize to lowercase
        assert_eq!(
            serde_yml::to_string(&Level::Stakeholder).unwrap().trim(),
            "stakeholder"
        );
        assert_eq!(
            serde_yml::to_string(&Level::System).unwrap().trim(),
            "system"
        );
        assert_eq!(
            serde_yml::to_string(&Level::Subsystem).unwrap().trim(),
            "subsystem"
        );
        assert_eq!(
            serde_yml::to_string(&Level::Component).unwrap().trim(),
            "component"
        );
        assert_eq!(
            serde_yml::to_string(&Level::Detail).unwrap().trim(),
            "detail"
        );
    }

    #[test]
    fn test_level_deserialization() {
        assert_eq!(
            serde_yml::from_str::<Level>("stakeholder").unwrap(),
            Level::Stakeholder
        );
        assert_eq!(
            serde_yml::from_str::<Level>("system").unwrap(),
            Level::System
        );
        assert_eq!(
            serde_yml::from_str::<Level>("subsystem").unwrap(),
            Level::Subsystem
        );
        assert_eq!(
            serde_yml::from_str::<Level>("component").unwrap(),
            Level::Component
        );
        assert_eq!(
            serde_yml::from_str::<Level>("detail").unwrap(),
            Level::Detail
        );
    }

    #[test]
    fn test_level_display() {
        assert_eq!(Level::Stakeholder.to_string(), "stakeholder");
        assert_eq!(Level::System.to_string(), "system");
        assert_eq!(Level::Subsystem.to_string(), "subsystem");
        assert_eq!(Level::Component.to_string(), "component");
        assert_eq!(Level::Detail.to_string(), "detail");
    }

    #[test]
    fn test_level_from_str() {
        assert_eq!("stakeholder".parse::<Level>().unwrap(), Level::Stakeholder);
        assert_eq!("system".parse::<Level>().unwrap(), Level::System);
        assert_eq!("SYSTEM".parse::<Level>().unwrap(), Level::System); // case insensitive
        assert_eq!("Subsystem".parse::<Level>().unwrap(), Level::Subsystem);
        assert!("invalid".parse::<Level>().is_err());
    }

    #[test]
    fn test_requirement_with_level() {
        let req = Requirement::new(
            RequirementType::Input,
            "Test".to_string(),
            "Text".to_string(),
            "test".to_string(),
        )
        .with_level(Level::Stakeholder);

        assert_eq!(req.level, Level::Stakeholder);
    }

    #[test]
    fn test_requirement_level_serializes() {
        let req = Requirement::new(
            RequirementType::Input,
            "Test".to_string(),
            "Text".to_string(),
            "test".to_string(),
        )
        .with_level(Level::Component);

        let yaml = serde_yml::to_string(&req).unwrap();
        assert!(yaml.contains("level: component"));
    }

    #[test]
    fn test_requirement_default_level_is_system() {
        let req = Requirement::new(
            RequirementType::Input,
            "Test".to_string(),
            "Text".to_string(),
            "test".to_string(),
        );

        assert_eq!(req.level, Level::System);
        let yaml = serde_yml::to_string(&req).unwrap();
        assert!(yaml.contains("level: system"));
    }

    #[test]
    fn test_requirement_without_level_deserializes_to_default() {
        // Simulate an old requirement file without level field
        // Use a valid 26-character ULID (same format as generated: 01KDGJC92W6EBFGZ5SJW6MFGW6)
        let yaml = r#"
id: REQ-01KDGJC92W6EBFGZ5SJW6MFGW6
type: input
title: "Test"
text: "The system shall do something."
priority: medium
status: draft
created: "2024-01-01T00:00:00Z"
author: "test"
"#;
        let req: Requirement = serde_yml::from_str(yaml).unwrap();
        assert_eq!(req.level, Level::System); // Default
    }
}
