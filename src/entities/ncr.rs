//! NCR entity type - Non-Conformance Reports for quality issues

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

use crate::core::entity::{Entity, Status};
use crate::core::identity::EntityId;

/// NCR type classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum NcrType {
    /// Internal quality issue
    #[default]
    Internal,
    /// Supplier quality issue
    Supplier,
    /// Customer complaint
    Customer,
}

impl std::fmt::Display for NcrType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NcrType::Internal => write!(f, "internal"),
            NcrType::Supplier => write!(f, "supplier"),
            NcrType::Customer => write!(f, "customer"),
        }
    }
}

impl std::str::FromStr for NcrType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "internal" => Ok(NcrType::Internal),
            "supplier" => Ok(NcrType::Supplier),
            "customer" => Ok(NcrType::Customer),
            _ => Err(format!(
                "Invalid NCR type: {}. Use internal, supplier, or customer",
                s
            )),
        }
    }
}

/// NCR severity level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum NcrSeverity {
    #[default]
    Minor,
    Major,
    Critical,
}

impl std::fmt::Display for NcrSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NcrSeverity::Minor => write!(f, "minor"),
            NcrSeverity::Major => write!(f, "major"),
            NcrSeverity::Critical => write!(f, "critical"),
        }
    }
}

impl std::str::FromStr for NcrSeverity {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "minor" => Ok(NcrSeverity::Minor),
            "major" => Ok(NcrSeverity::Major),
            "critical" => Ok(NcrSeverity::Critical),
            _ => Err(format!(
                "Invalid NCR severity: {}. Use minor, major, or critical",
                s
            )),
        }
    }
}

/// NCR category
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum NcrCategory {
    #[default]
    Dimensional,
    Cosmetic,
    Material,
    Functional,
    Documentation,
    Process,
    Packaging,
}

impl std::fmt::Display for NcrCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NcrCategory::Dimensional => write!(f, "dimensional"),
            NcrCategory::Cosmetic => write!(f, "cosmetic"),
            NcrCategory::Material => write!(f, "material"),
            NcrCategory::Functional => write!(f, "functional"),
            NcrCategory::Documentation => write!(f, "documentation"),
            NcrCategory::Process => write!(f, "process"),
            NcrCategory::Packaging => write!(f, "packaging"),
        }
    }
}

impl std::str::FromStr for NcrCategory {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "dimensional" => Ok(NcrCategory::Dimensional),
            "cosmetic" => Ok(NcrCategory::Cosmetic),
            "material" => Ok(NcrCategory::Material),
            "functional" => Ok(NcrCategory::Functional),
            "documentation" => Ok(NcrCategory::Documentation),
            "process" => Ok(NcrCategory::Process),
            "packaging" => Ok(NcrCategory::Packaging),
            _ => Err(format!(
                "Invalid NCR category: {}. Use dimensional, cosmetic, material, functional, documentation, process, or packaging",
                s
            )),
        }
    }
}

/// Detection stage
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum DetectionStage {
    /// Incoming inspection
    Incoming,
    /// In-process inspection
    #[default]
    InProcess,
    /// Final inspection
    Final,
    /// Customer detection
    Customer,
    /// Field detection
    Field,
}

/// Detection information
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Detection {
    /// Where the issue was found
    #[serde(default)]
    pub found_at: DetectionStage,

    /// Who found it
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub found_by: Option<String>,

    /// When it was found
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub found_date: Option<NaiveDate>,

    /// Operation/step where found
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation: Option<String>,
}

/// Affected items information
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AffectedItems {
    /// Part number
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub part_number: Option<String>,

    /// Lot/batch number
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lot_number: Option<String>,

    /// Serial numbers of affected units
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub serial_numbers: Vec<String>,

    /// Quantity affected
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quantity_affected: Option<u32>,
}

/// Defect description
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Defect {
    /// Characteristic affected
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub characteristic: Option<String>,

    /// Specification/tolerance
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub specification: Option<String>,

    /// Actual measured value
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actual: Option<String>,

    /// Deviation from nominal
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deviation: Option<f64>,
}

/// Containment action status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum ContainmentStatus {
    #[default]
    Open,
    Completed,
}

/// Containment action
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContainmentAction {
    /// Action description
    pub action: String,

    /// Date performed
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<NaiveDate>,

    /// Who completed it
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_by: Option<String>,

    /// Status
    #[serde(default)]
    pub status: ContainmentStatus,
}

/// Disposition decision
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum DispositionDecision {
    UseAsIs,
    Rework,
    #[default]
    Scrap,
    ReturnToSupplier,
}

impl std::fmt::Display for DispositionDecision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DispositionDecision::UseAsIs => write!(f, "use_as_is"),
            DispositionDecision::Rework => write!(f, "rework"),
            DispositionDecision::Scrap => write!(f, "scrap"),
            DispositionDecision::ReturnToSupplier => write!(f, "return_to_supplier"),
        }
    }
}

/// Disposition information
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Disposition {
    /// Decision
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision: Option<DispositionDecision>,

    /// Decision date
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision_date: Option<NaiveDate>,

    /// Who made the decision
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision_by: Option<String>,

    /// Justification
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub justification: Option<String>,

    /// MRB (Material Review Board) required
    #[serde(default)]
    pub mrb_required: bool,
}

/// Cost impact
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CostImpact {
    /// Rework cost
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rework_cost: Option<f64>,

    /// Scrap cost
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scrap_cost: Option<f64>,

    /// Currency
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
}

/// NCR workflow status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum NcrStatus {
    #[default]
    Open,
    Containment,
    Investigation,
    Disposition,
    Closed,
}

impl std::fmt::Display for NcrStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NcrStatus::Open => write!(f, "open"),
            NcrStatus::Containment => write!(f, "containment"),
            NcrStatus::Investigation => write!(f, "investigation"),
            NcrStatus::Disposition => write!(f, "disposition"),
            NcrStatus::Closed => write!(f, "closed"),
        }
    }
}

/// Links to other entities
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NcrLinks {
    /// Affected component
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub component: Option<EntityId>,

    /// Process where found
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process: Option<EntityId>,

    /// Control that detected
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub control: Option<EntityId>,

    /// Linked CAPA if opened
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capa: Option<EntityId>,

    /// Test result that created this NCR (reciprocal of RSLT.created_ncr)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_result: Option<EntityId>,
}

/// An NCR entity - Non-Conformance Report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ncr {
    /// Unique identifier
    pub id: EntityId,

    /// NCR title/summary
    pub title: String,

    /// NCR number (e.g., "NCR-2024-0042")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ncr_number: Option<String>,

    /// Report date
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report_date: Option<NaiveDate>,

    /// Detailed description
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// NCR type
    #[serde(default)]
    pub ncr_type: NcrType,

    /// Severity level
    #[serde(default)]
    pub severity: NcrSeverity,

    /// Category
    #[serde(default)]
    pub category: NcrCategory,

    /// Detection information
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detection: Option<Detection>,

    /// Affected items
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub affected_items: Option<AffectedItems>,

    /// Defect details
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub defect: Option<Defect>,

    /// Containment actions
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub containment: Vec<ContainmentAction>,

    /// Disposition
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disposition: Option<Disposition>,

    /// Cost impact
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_impact: Option<CostImpact>,

    /// NCR workflow status
    #[serde(default)]
    pub ncr_status: NcrStatus,

    /// Tags for filtering
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,

    /// Entity status (draft/review/approved/etc.)
    #[serde(default)]
    pub status: Status,

    /// Links to other entities
    #[serde(default)]
    pub links: NcrLinks,

    /// Creation timestamp
    pub created: DateTime<Utc>,

    /// Author (who created this NCR)
    pub author: String,

    /// Entity revision number
    #[serde(default = "default_revision")]
    pub entity_revision: u32,
}

fn default_revision() -> u32 {
    1
}

impl Entity for Ncr {
    const PREFIX: &'static str = "NCR";

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

impl Ncr {
    /// Create a new NCR with the given parameters
    pub fn new(title: String, ncr_type: NcrType, severity: NcrSeverity, author: String) -> Self {
        Self {
            id: EntityId::new(crate::core::EntityPrefix::Ncr),
            title,
            ncr_number: None,
            report_date: Some(chrono::Local::now().date_naive()),
            description: None,
            ncr_type,
            severity,
            category: NcrCategory::default(),
            detection: None,
            affected_items: None,
            defect: None,
            containment: Vec::new(),
            disposition: None,
            cost_impact: None,
            ncr_status: NcrStatus::default(),
            tags: Vec::new(),
            status: Status::default(),
            links: NcrLinks::default(),
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
    fn test_ncr_creation() {
        let ncr = Ncr::new(
            "Bore Diameter Out of Tolerance".to_string(),
            NcrType::Internal,
            NcrSeverity::Minor,
            "test".to_string(),
        );

        assert!(ncr.id.to_string().starts_with("NCR-"));
        assert_eq!(ncr.title, "Bore Diameter Out of Tolerance");
        assert_eq!(ncr.ncr_type, NcrType::Internal);
        assert_eq!(ncr.severity, NcrSeverity::Minor);
    }

    #[test]
    fn test_ncr_roundtrip() {
        let ncr = Ncr::new(
            "Material Defect".to_string(),
            NcrType::Supplier,
            NcrSeverity::Major,
            "test".to_string(),
        );

        let yaml = serde_yml::to_string(&ncr).unwrap();
        let parsed: Ncr = serde_yml::from_str(&yaml).unwrap();

        assert_eq!(ncr.id, parsed.id);
        assert_eq!(ncr.title, parsed.title);
        assert_eq!(ncr.ncr_type, parsed.ncr_type);
    }

    #[test]
    fn test_entity_trait_implementation() {
        let ncr = Ncr::new(
            "Test NCR".to_string(),
            NcrType::Internal,
            NcrSeverity::Minor,
            "test_author".to_string(),
        );

        assert_eq!(Ncr::PREFIX, "NCR");
        assert_eq!(ncr.title(), "Test NCR");
        assert_eq!(ncr.status(), "draft");
        assert_eq!(ncr.author(), "test_author");
    }

    #[test]
    fn test_ncr_type_from_str() {
        assert_eq!("internal".parse::<NcrType>().unwrap(), NcrType::Internal);
        assert_eq!("supplier".parse::<NcrType>().unwrap(), NcrType::Supplier);
        assert!("invalid".parse::<NcrType>().is_err());
    }
}
