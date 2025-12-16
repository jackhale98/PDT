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
}
