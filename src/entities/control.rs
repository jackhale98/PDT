//! Control entity type - Control plan items (SPC, inspection, etc.)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::core::entity::{Entity, Status};
use crate::core::identity::EntityId;

/// Control type classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum ControlType {
    /// Statistical Process Control
    Spc,
    /// Dimensional/attribute inspection
    #[default]
    Inspection,
    /// Error-proofing device
    PokaYoke,
    /// Visual inspection
    Visual,
    /// Functional test
    FunctionalTest,
    /// Attribute check (pass/fail)
    Attribute,
}

impl std::fmt::Display for ControlType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ControlType::Spc => write!(f, "spc"),
            ControlType::Inspection => write!(f, "inspection"),
            ControlType::PokaYoke => write!(f, "poka_yoke"),
            ControlType::Visual => write!(f, "visual"),
            ControlType::FunctionalTest => write!(f, "functional_test"),
            ControlType::Attribute => write!(f, "attribute"),
        }
    }
}

impl std::str::FromStr for ControlType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "spc" => Ok(ControlType::Spc),
            "inspection" => Ok(ControlType::Inspection),
            "poka_yoke" | "pokayoke" => Ok(ControlType::PokaYoke),
            "visual" => Ok(ControlType::Visual),
            "functional_test" | "functionaltest" => Ok(ControlType::FunctionalTest),
            "attribute" => Ok(ControlType::Attribute),
            _ => Err(format!(
                "Invalid control type: {}. Use spc, inspection, poka_yoke, visual, functional_test, or attribute",
                s
            )),
        }
    }
}

/// Control category (variable vs attribute data)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum ControlCategory {
    /// Variable (continuous) data
    #[default]
    Variable,
    /// Attribute (discrete/pass-fail) data
    Attribute,
}

impl std::fmt::Display for ControlCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ControlCategory::Variable => write!(f, "variable"),
            ControlCategory::Attribute => write!(f, "attribute"),
        }
    }
}

/// Characteristic being controlled
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Characteristic {
    /// Characteristic name (e.g., "Bore Diameter")
    pub name: String,

    /// Nominal value
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nominal: Option<f64>,

    /// Upper specification limit
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upper_limit: Option<f64>,

    /// Lower specification limit
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lower_limit: Option<f64>,

    /// Units of measurement
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub units: Option<String>,

    /// Critical to quality (CTQ) / special characteristic
    #[serde(default)]
    pub critical: bool,
}

/// Measurement method information
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Measurement {
    /// Measurement method description
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,

    /// Equipment/gage used
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub equipment: Option<String>,

    /// Gage R&R percentage (MSA result)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gage_rr_percent: Option<f64>,
}

/// Sampling plan
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum SamplingType {
    /// Continuous sampling
    #[default]
    Continuous,
    /// Periodic (time-based)
    Periodic,
    /// Per lot/batch
    Lot,
    /// First article only
    FirstArticle,
}

/// Sampling configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Sampling {
    /// Sampling type
    #[serde(default, rename = "type")]
    pub sampling_type: SamplingType,

    /// Frequency (e.g., "5 parts", "every 2 hours")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frequency: Option<String>,

    /// Sample size per check
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample_size: Option<u32>,
}

/// Statistical control limits (for SPC)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ControlLimits {
    /// Upper control limit
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ucl: Option<f64>,

    /// Lower control limit
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lcl: Option<f64>,

    /// Target/centerline
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<f64>,
}

/// Links to other entities
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ControlLinks {
    /// Parent process (required for context)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process: Option<EntityId>,

    /// Feature being controlled (optional)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feature: Option<EntityId>,

    /// Requirements verified by this control
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub verifies: Vec<EntityId>,

    /// CAPA that added this control (reciprocal of CAPA.controls_added)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub added_by_capa: Vec<EntityId>,
}

/// A Control entity - control plan item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Control {
    /// Unique identifier
    pub id: EntityId,

    /// Control title
    pub title: String,

    /// Detailed description
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Control type
    #[serde(default)]
    pub control_type: ControlType,

    /// Control category (variable vs attribute)
    #[serde(default)]
    pub control_category: ControlCategory,

    /// Characteristic being controlled
    #[serde(default)]
    pub characteristic: Characteristic,

    /// Measurement method
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub measurement: Option<Measurement>,

    /// Sampling plan
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sampling: Option<Sampling>,

    /// Control limits (for SPC)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub control_limits: Option<ControlLimits>,

    /// Reaction plan for out-of-control conditions
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reaction_plan: Option<String>,

    /// Tags for filtering
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,

    /// Current status
    #[serde(default)]
    pub status: Status,

    /// Links to other entities
    #[serde(default)]
    pub links: ControlLinks,

    /// Creation timestamp
    pub created: DateTime<Utc>,

    /// Author (who created this control)
    pub author: String,

    /// Entity revision number
    #[serde(default = "default_revision")]
    pub entity_revision: u32,
}

fn default_revision() -> u32 {
    1
}

impl Entity for Control {
    const PREFIX: &'static str = "CTRL";

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

impl Control {
    /// Create a new control with the given parameters
    pub fn new(title: String, control_type: ControlType, author: String) -> Self {
        Self {
            id: EntityId::new(crate::core::EntityPrefix::Ctrl),
            title,
            description: None,
            control_type,
            control_category: ControlCategory::default(),
            characteristic: Characteristic::default(),
            measurement: None,
            sampling: None,
            control_limits: None,
            reaction_plan: None,
            tags: Vec::new(),
            status: Status::default(),
            links: ControlLinks::default(),
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
    fn test_control_creation() {
        let ctrl = Control::new(
            "Bore Diameter SPC".to_string(),
            ControlType::Spc,
            "test".to_string(),
        );

        assert!(ctrl.id.to_string().starts_with("CTRL-"));
        assert_eq!(ctrl.title, "Bore Diameter SPC");
        assert_eq!(ctrl.control_type, ControlType::Spc);
    }

    #[test]
    fn test_control_roundtrip() {
        let ctrl = Control::new(
            "Visual Check".to_string(),
            ControlType::Visual,
            "test".to_string(),
        );

        let yaml = serde_yml::to_string(&ctrl).unwrap();
        let parsed: Control = serde_yml::from_str(&yaml).unwrap();

        assert_eq!(ctrl.id, parsed.id);
        assert_eq!(ctrl.title, parsed.title);
        assert_eq!(ctrl.control_type, parsed.control_type);
    }

    #[test]
    fn test_control_type_serialization() {
        let ctrl = Control::new(
            "Error Proofing".to_string(),
            ControlType::PokaYoke,
            "test".to_string(),
        );

        let yaml = serde_yml::to_string(&ctrl).unwrap();
        assert!(yaml.contains("control_type: poka_yoke"));
    }

    #[test]
    fn test_entity_trait_implementation() {
        let ctrl = Control::new(
            "Test Control".to_string(),
            ControlType::Inspection,
            "test_author".to_string(),
        );

        assert_eq!(Control::PREFIX, "CTRL");
        assert_eq!(ctrl.title(), "Test Control");
        assert_eq!(ctrl.status(), "draft");
        assert_eq!(ctrl.author(), "test_author");
    }

    #[test]
    fn test_control_type_from_str() {
        assert_eq!("spc".parse::<ControlType>().unwrap(), ControlType::Spc);
        assert_eq!(
            "poka_yoke".parse::<ControlType>().unwrap(),
            ControlType::PokaYoke
        );
        assert!("invalid".parse::<ControlType>().is_err());
    }
}
