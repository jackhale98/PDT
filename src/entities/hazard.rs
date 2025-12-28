//! Hazard entity type for safety analysis
//!
//! Hazards are potential sources of harm in a product or system.
//! They are distinct from risks - a hazard is the source, while risks
//! quantify the probability and severity of harm from that hazard.
//!
//! Standards alignment:
//! - ISO 14971 (Medical devices)
//! - ISO 26262 (Automotive - HARA)
//! - IEC 61508 (Functional safety)
//! - DO-178C (Aerospace)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

use crate::core::entity::{Entity, Status};
use crate::core::identity::EntityId;
use crate::core::workflow::ApprovalRecord;

/// Hazard category - type of hazard by energy/mechanism
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum HazardCategory {
    /// Electrical hazards (shock, burns, fire)
    #[default]
    Electrical,
    /// Mechanical hazards (crushing, cutting, entanglement)
    Mechanical,
    /// Thermal hazards (burns, frostbite)
    Thermal,
    /// Chemical hazards (toxicity, corrosion)
    Chemical,
    /// Biological hazards (infection, allergens)
    Biological,
    /// Radiation hazards (ionizing, non-ionizing)
    Radiation,
    /// Ergonomic hazards (repetitive strain, posture)
    Ergonomic,
    /// Software/cyber hazards (malfunction, security)
    Software,
    /// Environmental hazards (noise, vibration)
    Environmental,
}

impl std::fmt::Display for HazardCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HazardCategory::Electrical => write!(f, "electrical"),
            HazardCategory::Mechanical => write!(f, "mechanical"),
            HazardCategory::Thermal => write!(f, "thermal"),
            HazardCategory::Chemical => write!(f, "chemical"),
            HazardCategory::Biological => write!(f, "biological"),
            HazardCategory::Radiation => write!(f, "radiation"),
            HazardCategory::Ergonomic => write!(f, "ergonomic"),
            HazardCategory::Software => write!(f, "software"),
            HazardCategory::Environmental => write!(f, "environmental"),
        }
    }
}

impl FromStr for HazardCategory {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "electrical" | "electric" => Ok(HazardCategory::Electrical),
            "mechanical" => Ok(HazardCategory::Mechanical),
            "thermal" | "heat" | "cold" => Ok(HazardCategory::Thermal),
            "chemical" => Ok(HazardCategory::Chemical),
            "biological" | "bio" => Ok(HazardCategory::Biological),
            "radiation" | "rad" => Ok(HazardCategory::Radiation),
            "ergonomic" | "ergo" => Ok(HazardCategory::Ergonomic),
            "software" | "sw" | "cyber" => Ok(HazardCategory::Software),
            "environmental" | "env" => Ok(HazardCategory::Environmental),
            _ => Err(format!("Unknown hazard category: {}", s)),
        }
    }
}

/// Hazard severity level (independent of probability)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum HazardSeverity {
    /// Negligible - no injury or minor discomfort
    Negligible,
    /// Minor - temporary injury, first aid
    #[default]
    Minor,
    /// Serious - injury requiring medical attention
    Serious,
    /// Severe - permanent injury or disability
    Severe,
    /// Catastrophic - death or multiple fatalities
    Catastrophic,
}

impl std::fmt::Display for HazardSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HazardSeverity::Negligible => write!(f, "negligible"),
            HazardSeverity::Minor => write!(f, "minor"),
            HazardSeverity::Serious => write!(f, "serious"),
            HazardSeverity::Severe => write!(f, "severe"),
            HazardSeverity::Catastrophic => write!(f, "catastrophic"),
        }
    }
}

impl FromStr for HazardSeverity {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "negligible" | "none" => Ok(HazardSeverity::Negligible),
            "minor" | "low" => Ok(HazardSeverity::Minor),
            "serious" | "medium" | "moderate" => Ok(HazardSeverity::Serious),
            "severe" | "high" => Ok(HazardSeverity::Severe),
            "catastrophic" | "critical" | "fatal" => Ok(HazardSeverity::Catastrophic),
            _ => Err(format!("Unknown hazard severity: {}", s)),
        }
    }
}

/// Links to other entities
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HazardLinks {
    /// Components/assemblies where this hazard originates
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub originates_from: Vec<EntityId>,

    /// Risks caused by this hazard
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub causes: Vec<EntityId>,

    /// Controls that address this hazard
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub controlled_by: Vec<EntityId>,

    /// Tests that verify hazard controls
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub verified_by: Vec<EntityId>,

    /// Related requirements (safety requirements)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub related_to: Vec<EntityId>,
}

/// A hazard entity - potential source of harm
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hazard {
    /// Unique identifier (HAZ-ULID)
    pub id: EntityId,

    /// Short title describing the hazard
    pub title: String,

    /// Hazard category (electrical, mechanical, etc.)
    pub category: HazardCategory,

    /// Detailed description of the hazard
    pub description: String,

    /// List of potential harms from this hazard
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub potential_harms: Vec<String>,

    /// Energy level or magnitude (e.g., "300V DC", "500 psi", "150°C")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub energy_level: Option<String>,

    /// Maximum severity of potential harm
    #[serde(default)]
    pub severity: HazardSeverity,

    /// Exposure scenario - how someone could be exposed
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exposure_scenario: Option<String>,

    /// Affected populations (operators, maintenance, patients, etc.)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub affected_populations: Vec<String>,

    /// Tags for filtering
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,

    /// Current status
    #[serde(default)]
    pub status: Status,

    /// Links to other entities
    #[serde(default)]
    pub links: HazardLinks,

    /// Creation timestamp
    pub created: DateTime<Utc>,

    /// Author (who identified this hazard)
    pub author: String,

    /// Revision number
    #[serde(default = "default_revision")]
    pub revision: u32,

    /// Approval records for workflow
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub approvals: Vec<ApprovalRecord>,
}

fn default_revision() -> u32 {
    1
}

impl Entity for Hazard {
    const PREFIX: &'static str = "HAZ";

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

impl Hazard {
    /// Create a new hazard with required fields
    pub fn new(
        id: EntityId,
        title: String,
        category: HazardCategory,
        description: String,
        author: String,
    ) -> Self {
        Self {
            id,
            title,
            category,
            description,
            potential_harms: Vec::new(),
            energy_level: None,
            severity: HazardSeverity::default(),
            exposure_scenario: None,
            affected_populations: Vec::new(),
            tags: Vec::new(),
            status: Status::Draft,
            links: HazardLinks::default(),
            created: Utc::now(),
            author,
            revision: 1,
            approvals: Vec::new(),
        }
    }

    /// Get the number of risks caused by this hazard
    pub fn risk_count(&self) -> usize {
        self.links.causes.len()
    }

    /// Get the number of controls for this hazard
    pub fn control_count(&self) -> usize {
        self.links.controlled_by.len()
    }

    /// Check if hazard has any controls
    pub fn is_controlled(&self) -> bool {
        !self.links.controlled_by.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hazard_category_display() {
        assert_eq!(HazardCategory::Electrical.to_string(), "electrical");
        assert_eq!(HazardCategory::Mechanical.to_string(), "mechanical");
    }

    #[test]
    fn test_hazard_category_from_str() {
        assert_eq!(
            HazardCategory::from_str("electrical").unwrap(),
            HazardCategory::Electrical
        );
        assert_eq!(
            HazardCategory::from_str("MECHANICAL").unwrap(),
            HazardCategory::Mechanical
        );
    }

    #[test]
    fn test_hazard_severity_from_str() {
        assert_eq!(
            HazardSeverity::from_str("catastrophic").unwrap(),
            HazardSeverity::Catastrophic
        );
        assert_eq!(
            HazardSeverity::from_str("critical").unwrap(),
            HazardSeverity::Catastrophic
        );
    }

    #[test]
    fn test_new_hazard() {
        use crate::core::identity::{EntityId, EntityPrefix};

        let id = EntityId::new(EntityPrefix::Haz);
        let hazard = Hazard::new(
            id,
            "High voltage".to_string(),
            HazardCategory::Electrical,
            "300V DC in motor controller".to_string(),
            "Test Author".to_string(),
        );

        assert_eq!(hazard.title, "High voltage");
        assert_eq!(hazard.category, HazardCategory::Electrical);
        assert_eq!(hazard.status, Status::Draft);
        assert!(!hazard.is_controlled());
    }
}
