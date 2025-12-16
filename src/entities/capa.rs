//! CAPA entity type - Corrective and Preventive Actions

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

use crate::core::entity::{Entity, Status};
use crate::core::identity::EntityId;

/// CAPA type classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum CapaType {
    #[default]
    Corrective,
    Preventive,
}


impl std::fmt::Display for CapaType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CapaType::Corrective => write!(f, "corrective"),
            CapaType::Preventive => write!(f, "preventive"),
        }
    }
}

impl std::str::FromStr for CapaType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "corrective" => Ok(CapaType::Corrective),
            "preventive" => Ok(CapaType::Preventive),
            _ => Err(format!(
                "Invalid CAPA type: {}. Use corrective or preventive",
                s
            )),
        }
    }
}

/// CAPA source type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum SourceType {
    #[default]
    Ncr,
    Audit,
    CustomerComplaint,
    TrendAnalysis,
    Risk,
}


impl std::fmt::Display for SourceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SourceType::Ncr => write!(f, "ncr"),
            SourceType::Audit => write!(f, "audit"),
            SourceType::CustomerComplaint => write!(f, "customer_complaint"),
            SourceType::TrendAnalysis => write!(f, "trend_analysis"),
            SourceType::Risk => write!(f, "risk"),
        }
    }
}

impl std::str::FromStr for SourceType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "ncr" => Ok(SourceType::Ncr),
            "audit" => Ok(SourceType::Audit),
            "customer_complaint" | "customercomplaint" => Ok(SourceType::CustomerComplaint),
            "trend_analysis" | "trendanalysis" => Ok(SourceType::TrendAnalysis),
            "risk" => Ok(SourceType::Risk),
            _ => Err(format!(
                "Invalid source type: {}. Use ncr, audit, customer_complaint, trend_analysis, or risk",
                s
            )),
        }
    }
}

/// Source information
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Source {
    /// Source type
    #[serde(default, rename = "type")]
    pub source_type: SourceType,

    /// Reference ID (e.g., NCR ID)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
}

/// Root cause analysis method
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum RcaMethod {
    #[default]
    FiveWhy,
    Fishbone,
    FaultTree,
    EightD,
}


impl std::fmt::Display for RcaMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RcaMethod::FiveWhy => write!(f, "five_why"),
            RcaMethod::Fishbone => write!(f, "fishbone"),
            RcaMethod::FaultTree => write!(f, "fault_tree"),
            RcaMethod::EightD => write!(f, "eight_d"),
        }
    }
}

/// Root cause analysis
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RootCauseAnalysis {
    /// Analysis method used
    #[serde(default)]
    pub method: RcaMethod,

    /// Identified root cause
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_cause: Option<String>,

    /// Contributing factors
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub contributing_factors: Vec<String>,
}

/// Action item status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum ActionStatus {
    #[default]
    Open,
    InProgress,
    Completed,
    Verified,
}


impl std::fmt::Display for ActionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ActionStatus::Open => write!(f, "open"),
            ActionStatus::InProgress => write!(f, "in_progress"),
            ActionStatus::Completed => write!(f, "completed"),
            ActionStatus::Verified => write!(f, "verified"),
        }
    }
}

/// Action type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum ActionType {
    #[default]
    Corrective,
    Preventive,
}


/// Individual action item
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ActionItem {
    /// Action number
    pub action_number: u32,

    /// Action description
    pub description: String,

    /// Action type
    #[serde(default)]
    pub action_type: ActionType,

    /// Responsible owner
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,

    /// Due date
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due_date: Option<NaiveDate>,

    /// Completion date
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_date: Option<NaiveDate>,

    /// Status
    #[serde(default)]
    pub status: ActionStatus,

    /// Evidence of completion
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<String>,
}

/// Effectiveness result
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectivenessResult {
    Effective,
    PartiallyEffective,
    Ineffective,
}

impl std::fmt::Display for EffectivenessResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EffectivenessResult::Effective => write!(f, "effective"),
            EffectivenessResult::PartiallyEffective => write!(f, "partially_effective"),
            EffectivenessResult::Ineffective => write!(f, "ineffective"),
        }
    }
}

/// Effectiveness verification
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Effectiveness {
    /// Verified flag
    #[serde(default)]
    pub verified: bool,

    /// Verification date
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verified_date: Option<NaiveDate>,

    /// Result
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<EffectivenessResult>,

    /// Evidence
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<String>,
}

/// Closure information
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Closure {
    /// Closed flag
    #[serde(default)]
    pub closed: bool,

    /// Closure date
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub closed_date: Option<NaiveDate>,

    /// Who closed it
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub closed_by: Option<String>,
}

/// Timeline information
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Timeline {
    /// Date initiated
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initiated_date: Option<NaiveDate>,

    /// Target completion date
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_date: Option<NaiveDate>,
}

/// CAPA workflow status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum CapaStatus {
    #[default]
    Initiation,
    Investigation,
    Implementation,
    Verification,
    Closed,
}


impl std::fmt::Display for CapaStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CapaStatus::Initiation => write!(f, "initiation"),
            CapaStatus::Investigation => write!(f, "investigation"),
            CapaStatus::Implementation => write!(f, "implementation"),
            CapaStatus::Verification => write!(f, "verification"),
            CapaStatus::Closed => write!(f, "closed"),
        }
    }
}

/// Links to other entities
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CapaLinks {
    /// Source NCRs
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ncrs: Vec<EntityId>,

    /// Related risks
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub risks: Vec<EntityId>,

    /// Processes modified
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub processes_modified: Vec<EntityId>,

    /// Controls added
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub controls_added: Vec<EntityId>,
}

/// A CAPA entity - Corrective/Preventive Action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capa {
    /// Unique identifier
    pub id: EntityId,

    /// CAPA title
    pub title: String,

    /// CAPA number (e.g., "CAPA-2024-0015")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capa_number: Option<String>,

    /// CAPA type
    #[serde(default)]
    pub capa_type: CapaType,

    /// Source information
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<Source>,

    /// Problem statement
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub problem_statement: Option<String>,

    /// Root cause analysis
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_cause_analysis: Option<RootCauseAnalysis>,

    /// Action items
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<ActionItem>,

    /// Effectiveness verification
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effectiveness: Option<Effectiveness>,

    /// Closure information
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub closure: Option<Closure>,

    /// Timeline
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeline: Option<Timeline>,

    /// CAPA workflow status
    #[serde(default)]
    pub capa_status: CapaStatus,

    /// Tags for filtering
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,

    /// Entity status (draft/review/approved/etc.)
    #[serde(default)]
    pub status: Status,

    /// Links to other entities
    #[serde(default)]
    pub links: CapaLinks,

    /// Creation timestamp
    pub created: DateTime<Utc>,

    /// Author (who created this CAPA)
    pub author: String,

    /// Entity revision number
    #[serde(default = "default_revision")]
    pub entity_revision: u32,
}

fn default_revision() -> u32 {
    1
}

impl Entity for Capa {
    const PREFIX: &'static str = "CAPA";

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

impl Capa {
    /// Create a new CAPA with the given parameters
    pub fn new(title: String, capa_type: CapaType, author: String) -> Self {
        Self {
            id: EntityId::new(crate::core::EntityPrefix::Capa),
            title,
            capa_number: None,
            capa_type,
            source: None,
            problem_statement: None,
            root_cause_analysis: None,
            actions: Vec::new(),
            effectiveness: None,
            closure: None,
            timeline: Some(Timeline {
                initiated_date: Some(chrono::Local::now().date_naive()),
                target_date: None,
            }),
            capa_status: CapaStatus::default(),
            tags: Vec::new(),
            status: Status::default(),
            links: CapaLinks::default(),
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
    fn test_capa_creation() {
        let capa = Capa::new(
            "Tool Wear Detection Improvement".to_string(),
            CapaType::Corrective,
            "test".to_string(),
        );

        assert!(capa.id.to_string().starts_with("CAPA-"));
        assert_eq!(capa.title, "Tool Wear Detection Improvement");
        assert_eq!(capa.capa_type, CapaType::Corrective);
    }

    #[test]
    fn test_capa_roundtrip() {
        let capa = Capa::new(
            "Process Improvement".to_string(),
            CapaType::Preventive,
            "test".to_string(),
        );

        let yaml = serde_yml::to_string(&capa).unwrap();
        let parsed: Capa = serde_yml::from_str(&yaml).unwrap();

        assert_eq!(capa.id, parsed.id);
        assert_eq!(capa.title, parsed.title);
        assert_eq!(capa.capa_type, parsed.capa_type);
    }

    #[test]
    fn test_entity_trait_implementation() {
        let capa = Capa::new(
            "Test CAPA".to_string(),
            CapaType::Corrective,
            "test_author".to_string(),
        );

        assert_eq!(Capa::PREFIX, "CAPA");
        assert_eq!(capa.title(), "Test CAPA");
        assert_eq!(capa.status(), "draft");
        assert_eq!(capa.author(), "test_author");
    }

    #[test]
    fn test_capa_type_from_str() {
        assert_eq!(
            "corrective".parse::<CapaType>().unwrap(),
            CapaType::Corrective
        );
        assert_eq!(
            "preventive".parse::<CapaType>().unwrap(),
            CapaType::Preventive
        );
        assert!("invalid".parse::<CapaType>().is_err());
    }
}
