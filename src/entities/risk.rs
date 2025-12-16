//! Risk entity type (FMEA - Failure Mode and Effects Analysis)

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

use crate::core::entity::{Entity, Status};
use crate::core::identity::EntityId;

/// Risk type - categorizes risk by source/domain
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum RiskType {
    /// Design risks - product/component failure modes
    #[default]
    Design,
    /// Process risks - manufacturing/operational failure modes
    Process,
    /// Use risks - user interaction/usability failure modes
    Use,
    /// Software risks - software-specific failure modes
    Software,
}

impl std::fmt::Display for RiskType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RiskType::Design => write!(f, "design"),
            RiskType::Process => write!(f, "process"),
            RiskType::Use => write!(f, "use"),
            RiskType::Software => write!(f, "software"),
        }
    }
}

/// Risk level assessment
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum RiskLevel {
    Low,
    #[default]
    Medium,
    High,
    Critical,
}

impl std::fmt::Display for RiskLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RiskLevel::Low => write!(f, "low"),
            RiskLevel::Medium => write!(f, "medium"),
            RiskLevel::High => write!(f, "high"),
            RiskLevel::Critical => write!(f, "critical"),
        }
    }
}

/// Mitigation action type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum MitigationType {
    #[default]
    Prevention,
    Detection,
}

/// Mitigation status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum MitigationStatus {
    #[default]
    Proposed,
    InProgress,
    Completed,
    Verified,
}

impl std::fmt::Display for MitigationStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MitigationStatus::Proposed => write!(f, "proposed"),
            MitigationStatus::InProgress => write!(f, "in_progress"),
            MitigationStatus::Completed => write!(f, "completed"),
            MitigationStatus::Verified => write!(f, "verified"),
        }
    }
}

/// A risk mitigation action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mitigation {
    /// The mitigation action description
    pub action: String,

    /// Type of mitigation (prevention or detection)
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "type")]
    pub mitigation_type: Option<MitigationType>,

    /// Implementation status
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<MitigationStatus>,

    /// Person responsible for implementing
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,

    /// Target completion date
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due_date: Option<NaiveDate>,
}

/// Initial risk assessment (before mitigations)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InitialRisk {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub severity: Option<u8>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub occurrence: Option<u8>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detection: Option<u8>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rpn: Option<u16>,
}

/// Links to other entities
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RiskLinks {
    /// Related requirements or other entities
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub related_to: Vec<EntityId>,

    /// Design outputs that mitigate this risk
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mitigated_by: Vec<EntityId>,

    /// Tests that verify risk mitigation
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub verified_by: Vec<EntityId>,

    /// Entities affected by this risk (FEAT, CMP, ASM, PROC, etc.)
    /// Target type is inferred from the entity ID prefix
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub affects: Vec<EntityId>,
}

/// A risk entity (FMEA item)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Risk {
    /// Unique identifier
    pub id: EntityId,

    /// Risk type (design or process)
    #[serde(rename = "type")]
    pub risk_type: RiskType,

    /// Short title
    pub title: String,

    /// Category (user-defined)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,

    /// Tags for filtering
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,

    /// Detailed description of the risk
    pub description: String,

    /// How the failure manifests (FMEA: Failure Mode)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_mode: Option<String>,

    /// Root cause or mechanism (FMEA: Cause)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cause: Option<String>,

    /// Impact or consequence (FMEA: Effect)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effect: Option<String>,

    /// Severity rating 1-10 (FMEA: S)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub severity: Option<u8>,

    /// Occurrence/probability rating 1-10 (FMEA: O)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub occurrence: Option<u8>,

    /// Detection difficulty rating 1-10 (FMEA: D)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detection: Option<u8>,

    /// Risk Priority Number = S x O x D
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rpn: Option<u16>,

    /// Initial risk assessment (before mitigations)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial_risk: Option<InitialRisk>,

    /// List of risk mitigation actions
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mitigations: Vec<Mitigation>,

    /// Current status
    #[serde(default)]
    pub status: Status,

    /// Overall risk level assessment
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub risk_level: Option<RiskLevel>,

    /// Links to other entities
    #[serde(default)]
    pub links: RiskLinks,

    /// Creation timestamp
    pub created: DateTime<Utc>,

    /// Author (who created this risk)
    pub author: String,

    /// Revision number
    #[serde(default = "default_revision")]
    pub revision: u32,
}

fn default_revision() -> u32 {
    1
}

impl Entity for Risk {
    const PREFIX: &'static str = "RISK";

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

impl Risk {
    /// Create a new risk with the given parameters
    pub fn new(risk_type: RiskType, title: String, description: String, author: String) -> Self {
        Self {
            id: EntityId::new(crate::core::EntityPrefix::Risk),
            risk_type,
            title,
            category: None,
            tags: Vec::new(),
            description,
            failure_mode: None,
            cause: None,
            effect: None,
            severity: None,
            occurrence: None,
            detection: None,
            rpn: None,
            initial_risk: None,
            mitigations: Vec::new(),
            status: Status::default(),
            risk_level: None,
            links: RiskLinks::default(),
            created: Utc::now(),
            author,
            revision: 1,
        }
    }

    /// Calculate RPN from severity, occurrence, and detection
    pub fn calculate_rpn(&self) -> Option<u16> {
        match (self.severity, self.occurrence, self.detection) {
            (Some(s), Some(o), Some(d)) => Some(s as u16 * o as u16 * d as u16),
            _ => None,
        }
    }

    /// Determine risk level based on RPN
    pub fn determine_risk_level(&self) -> Option<RiskLevel> {
        self.rpn
            .or_else(|| self.calculate_rpn())
            .map(|rpn| match rpn {
                0..=50 => RiskLevel::Low,
                51..=150 => RiskLevel::Medium,
                151..=400 => RiskLevel::High,
                _ => RiskLevel::Critical,
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::EntityPrefix;

    #[test]
    fn test_risk_creation() {
        let risk = Risk::new(
            RiskType::Design,
            "Battery Overheating".to_string(),
            "Risk of thermal runaway in battery pack".to_string(),
            "test".to_string(),
        );

        assert!(risk.id.to_string().starts_with("RISK-"));
        assert_eq!(risk.title, "Battery Overheating");
        assert_eq!(risk.risk_type, RiskType::Design);
        assert_eq!(risk.status, Status::Draft);
    }

    #[test]
    fn test_risk_roundtrip() {
        let mut risk = Risk::new(
            RiskType::Process,
            "Test Risk".to_string(),
            "Test description".to_string(),
            "test".to_string(),
        );
        risk.severity = Some(7);
        risk.occurrence = Some(5);
        risk.detection = Some(3);
        risk.rpn = Some(105);

        let yaml = serde_yml::to_string(&risk).unwrap();
        let parsed: Risk = serde_yml::from_str(&yaml).unwrap();

        assert_eq!(risk.id, parsed.id);
        assert_eq!(risk.title, parsed.title);
        assert_eq!(risk.severity, parsed.severity);
        assert_eq!(risk.rpn, parsed.rpn);
    }

    #[test]
    fn test_risk_serializes_type_correctly() {
        let risk = Risk::new(
            RiskType::Design,
            "Test".to_string(),
            "Description".to_string(),
            "test".to_string(),
        );

        let yaml = serde_yml::to_string(&risk).unwrap();
        assert!(yaml.contains("type: design"));
    }

    #[test]
    fn test_risk_serializes_process_type() {
        let risk = Risk::new(
            RiskType::Process,
            "Test".to_string(),
            "Description".to_string(),
            "test".to_string(),
        );

        let yaml = serde_yml::to_string(&risk).unwrap();
        assert!(yaml.contains("type: process"));
    }

    #[test]
    fn test_calculate_rpn() {
        let mut risk = Risk::new(
            RiskType::Design,
            "Test".to_string(),
            "Description".to_string(),
            "test".to_string(),
        );

        // No values set
        assert_eq!(risk.calculate_rpn(), None);

        // Set all values
        risk.severity = Some(8);
        risk.occurrence = Some(5);
        risk.detection = Some(4);
        assert_eq!(risk.calculate_rpn(), Some(160)); // 8 * 5 * 4 = 160
    }

    #[test]
    fn test_determine_risk_level() {
        let mut risk = Risk::new(
            RiskType::Design,
            "Test".to_string(),
            "Description".to_string(),
            "test".to_string(),
        );

        // Low risk: RPN <= 50
        risk.rpn = Some(30);
        assert_eq!(risk.determine_risk_level(), Some(RiskLevel::Low));

        // Medium risk: 51 <= RPN <= 150
        risk.rpn = Some(100);
        assert_eq!(risk.determine_risk_level(), Some(RiskLevel::Medium));

        // High risk: 151 <= RPN <= 400
        risk.rpn = Some(250);
        assert_eq!(risk.determine_risk_level(), Some(RiskLevel::High));

        // Critical risk: RPN > 400
        risk.rpn = Some(500);
        assert_eq!(risk.determine_risk_level(), Some(RiskLevel::Critical));
    }

    #[test]
    fn test_risk_with_mitigations() {
        let mut risk = Risk::new(
            RiskType::Design,
            "Overheating".to_string(),
            "Battery may overheat".to_string(),
            "test".to_string(),
        );

        risk.mitigations.push(Mitigation {
            action: "Add thermal cutoff".to_string(),
            mitigation_type: Some(MitigationType::Prevention),
            status: Some(MitigationStatus::Proposed),
            owner: Some("John".to_string()),
            due_date: None,
        });

        let yaml = serde_yml::to_string(&risk).unwrap();
        let parsed: Risk = serde_yml::from_str(&yaml).unwrap();

        assert_eq!(parsed.mitigations.len(), 1);
        assert_eq!(parsed.mitigations[0].action, "Add thermal cutoff");
        assert_eq!(
            parsed.mitigations[0].mitigation_type,
            Some(MitigationType::Prevention)
        );
    }

    #[test]
    fn test_entity_trait_implementation() {
        let risk = Risk::new(
            RiskType::Design,
            "Test Risk".to_string(),
            "Test description".to_string(),
            "author".to_string(),
        );

        assert_eq!(Risk::PREFIX, "RISK");
        assert_eq!(risk.title(), "Test Risk");
        assert_eq!(risk.status(), "draft");
        assert_eq!(risk.author(), "author");
    }

    #[test]
    fn test_risk_links() {
        let mut risk = Risk::new(
            RiskType::Design,
            "Test".to_string(),
            "Description".to_string(),
            "test".to_string(),
        );

        let req_id = EntityId::new(EntityPrefix::Req);
        let test_id = EntityId::new(EntityPrefix::Test);

        risk.links.related_to.push(req_id.clone());
        risk.links.verified_by.push(test_id.clone());

        let yaml = serde_yml::to_string(&risk).unwrap();
        let parsed: Risk = serde_yml::from_str(&yaml).unwrap();

        assert_eq!(parsed.links.related_to.len(), 1);
        assert_eq!(parsed.links.verified_by.len(), 1);
        assert_eq!(parsed.links.related_to[0], req_id);
        assert_eq!(parsed.links.verified_by[0], test_id);
    }
}
