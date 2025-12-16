//! Result entity type (Test execution record)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::core::entity::{Entity, Status};
use crate::core::identity::EntityId;

/// Overall verdict for a test result
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum Verdict {
    #[default]
    Pass,
    Fail,
    Conditional,
    Incomplete,
    #[serde(rename = "not_applicable")]
    NotApplicable,
}

impl std::fmt::Display for Verdict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Verdict::Pass => write!(f, "pass"),
            Verdict::Fail => write!(f, "fail"),
            Verdict::Conditional => write!(f, "conditional"),
            Verdict::Incomplete => write!(f, "incomplete"),
            Verdict::NotApplicable => write!(f, "not_applicable"),
        }
    }
}

/// Result for a single step
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum StepResult {
    #[default]
    Pass,
    Fail,
    Skip,
    #[serde(rename = "not_applicable")]
    NotApplicable,
}

impl std::fmt::Display for StepResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StepResult::Pass => write!(f, "pass"),
            StepResult::Fail => write!(f, "fail"),
            StepResult::Skip => write!(f, "skip"),
            StepResult::NotApplicable => write!(f, "not_applicable"),
        }
    }
}

/// Sample identification information
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SampleInfo {
    /// Identifier for the sample tested
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample_id: Option<String>,

    /// Serial number of the unit tested
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub serial_number: Option<String>,

    /// Lot or batch number
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lot_number: Option<String>,

    /// Configuration or build version
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub configuration: Option<String>,
}

/// Actual environmental conditions during test
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResultEnvironment {
    /// Actual temperature during test
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<String>,

    /// Actual humidity during test
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub humidity: Option<String>,

    /// Where the test was conducted
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,

    /// Other environmental conditions
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub other: Option<String>,
}

/// Equipment used during the test
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EquipmentUsed {
    /// Equipment name
    pub name: String,

    /// Asset or equipment ID
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asset_id: Option<String>,

    /// Date of last calibration
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calibration_date: Option<String>,

    /// Date calibration expires
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calibration_due: Option<String>,
}

/// Measurement data for a step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Measurement {
    /// Measured value
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<f64>,

    /// Unit of measurement
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,

    /// Minimum acceptable value
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,

    /// Maximum acceptable value
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
}

/// Result for a single procedure step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepResultRecord {
    /// Step number from the procedure
    pub step: u32,

    /// Result for this step
    pub result: StepResult,

    /// What was actually observed
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed: Option<String>,

    /// Quantitative measurement data
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub measurement: Option<Measurement>,

    /// Additional notes for this step
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

/// A deviation from the test procedure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Deviation {
    /// Description of the deviation
    pub description: String,

    /// Impact of the deviation on results
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub impact: Option<String>,

    /// Justification for accepting the deviation
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub justification: Option<String>,
}

/// A failure record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Failure {
    /// Description of the failure
    pub description: String,

    /// Step where failure occurred
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step: Option<u32>,

    /// Root cause analysis
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_cause: Option<String>,

    /// Corrective action taken or required
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub corrective_action: Option<String>,

    /// ID of related action item
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action_id: Option<String>,
}

/// Attachment type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum AttachmentType {
    Data,
    Photo,
    Screenshot,
    Log,
    Report,
    #[default]
    Other,
}

/// An attachment to the result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attachment {
    /// Name of the attachment file
    pub filename: String,

    /// Relative path to the attachment
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,

    /// Type of attachment
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "type")]
    pub attachment_type: Option<AttachmentType>,

    /// Description of the attachment
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Links to other entities
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResultLinks {
    /// ID of the test protocol (same as test_id)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub test: Option<EntityId>,

    /// IDs of action items created from this result
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<EntityId>,

    /// NCR created from a failed test result
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_ncr: Option<EntityId>,
}

/// A test result entity (execution record)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Result {
    /// Unique identifier
    pub id: EntityId,

    /// ID of the test protocol that was executed
    pub test_id: EntityId,

    /// Revision of the test protocol used
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub test_revision: Option<u32>,

    /// Optional title for this result
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    /// Overall test verdict
    pub verdict: Verdict,

    /// Explanation for the verdict
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verdict_rationale: Option<String>,

    /// Category (user-defined)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,

    /// Tags for filtering
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,

    /// When the test was executed
    pub executed_date: DateTime<Utc>,

    /// Person who executed the test
    pub executed_by: String,

    /// Person who reviewed the results
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reviewed_by: Option<String>,

    /// When the results were reviewed
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reviewed_date: Option<DateTime<Utc>>,

    /// Information about the sample tested
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample_info: Option<SampleInfo>,

    /// Actual environmental conditions during the test
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub environment: Option<ResultEnvironment>,

    /// Equipment used during the test
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub equipment_used: Vec<EquipmentUsed>,

    /// Results for each procedure step
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub step_results: Vec<StepResultRecord>,

    /// Any deviations from the test procedure
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deviations: Vec<Deviation>,

    /// Details of any failures encountered
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub failures: Vec<Failure>,

    /// Supporting attachments
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<Attachment>,

    /// Actual time to complete the test
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration: Option<String>,

    /// General notes and observations
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,

    /// Current status
    #[serde(default)]
    pub status: Status,

    /// Links to other entities
    #[serde(default)]
    pub links: ResultLinks,

    /// Creation timestamp
    pub created: DateTime<Utc>,

    /// Author (who created this result)
    pub author: String,

    /// Revision number
    #[serde(default = "default_revision")]
    pub revision: u32,
}

fn default_revision() -> u32 {
    1
}

impl Entity for Result {
    const PREFIX: &'static str = "RSLT";

    fn id(&self) -> &EntityId {
        &self.id
    }

    fn title(&self) -> &str {
        self.title.as_deref().unwrap_or("Untitled Result")
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

impl Result {
    /// Create a new result with the given parameters
    pub fn new(test_id: EntityId, verdict: Verdict, executed_by: String, author: String) -> Self {
        let now = Utc::now();
        Self {
            id: EntityId::new(crate::core::EntityPrefix::Rslt),
            test_id,
            test_revision: None,
            title: None,
            verdict,
            verdict_rationale: None,
            category: None,
            tags: Vec::new(),
            executed_date: now,
            executed_by,
            reviewed_by: None,
            reviewed_date: None,
            sample_info: None,
            environment: None,
            equipment_used: Vec::new(),
            step_results: Vec::new(),
            deviations: Vec::new(),
            failures: Vec::new(),
            attachments: Vec::new(),
            duration: None,
            notes: None,
            status: Status::default(),
            links: ResultLinks::default(),
            created: now,
            author,
            revision: 1,
        }
    }

    /// Get the number of step results
    pub fn step_count(&self) -> usize {
        self.step_results.len()
    }

    /// Check if the result has any failures
    pub fn has_failures(&self) -> bool {
        !self.failures.is_empty()
    }

    /// Check if the result has any deviations
    pub fn has_deviations(&self) -> bool {
        !self.deviations.is_empty()
    }

    /// Get the pass rate as a percentage (steps that passed / total steps)
    pub fn pass_rate(&self) -> Option<f64> {
        if self.step_results.is_empty() {
            return None;
        }
        let passed = self
            .step_results
            .iter()
            .filter(|s| s.result == StepResult::Pass)
            .count();
        Some((passed as f64 / self.step_results.len() as f64) * 100.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::EntityPrefix;

    #[test]
    fn test_result_creation() {
        let test_id = EntityId::new(EntityPrefix::Test);
        let result = Result::new(
            test_id.clone(),
            Verdict::Pass,
            "tester".to_string(),
            "author".to_string(),
        );

        assert!(result.id.to_string().starts_with("RSLT-"));
        assert_eq!(result.test_id, test_id);
        assert_eq!(result.verdict, Verdict::Pass);
        assert_eq!(result.executed_by, "tester");
        assert_eq!(result.status, Status::Draft);
    }

    #[test]
    fn test_result_roundtrip() {
        let test_id = EntityId::new(EntityPrefix::Test);
        let mut result = Result::new(
            test_id,
            Verdict::Conditional,
            "tester".to_string(),
            "author".to_string(),
        );
        result.verdict_rationale = Some("Minor deviation noted".to_string());
        result.title = Some("Test Run #1".to_string());

        let yaml = serde_yml::to_string(&result).unwrap();
        let parsed: Result = serde_yml::from_str(&yaml).unwrap();

        assert_eq!(result.id, parsed.id);
        assert_eq!(result.verdict, parsed.verdict);
        assert_eq!(result.title, parsed.title);
    }

    #[test]
    fn test_verdict_serialization() {
        let test_id = EntityId::new(EntityPrefix::Test);
        let result = Result::new(
            test_id,
            Verdict::NotApplicable,
            "tester".to_string(),
            "author".to_string(),
        );

        let yaml = serde_yml::to_string(&result).unwrap();
        assert!(yaml.contains("verdict: not_applicable"));
    }

    #[test]
    fn test_result_with_step_results() {
        let test_id = EntityId::new(EntityPrefix::Test);
        let mut result = Result::new(
            test_id,
            Verdict::Pass,
            "tester".to_string(),
            "author".to_string(),
        );

        result.step_results.push(StepResultRecord {
            step: 1,
            result: StepResult::Pass,
            observed: Some("As expected".to_string()),
            measurement: None,
            notes: None,
        });
        result.step_results.push(StepResultRecord {
            step: 2,
            result: StepResult::Pass,
            observed: None,
            measurement: Some(Measurement {
                value: Some(25.5),
                unit: Some("C".to_string()),
                min: Some(20.0),
                max: Some(30.0),
            }),
            notes: None,
        });

        let yaml = serde_yml::to_string(&result).unwrap();
        let parsed: Result = serde_yml::from_str(&yaml).unwrap();

        assert_eq!(parsed.step_results.len(), 2);
        assert_eq!(parsed.step_results[0].step, 1);
        assert_eq!(
            parsed.step_results[1].measurement.as_ref().unwrap().value,
            Some(25.5)
        );
    }

    #[test]
    fn test_result_with_failures() {
        let test_id = EntityId::new(EntityPrefix::Test);
        let mut result = Result::new(
            test_id,
            Verdict::Fail,
            "tester".to_string(),
            "author".to_string(),
        );

        result.failures.push(Failure {
            description: "Device overheated".to_string(),
            step: Some(3),
            root_cause: Some("Insufficient cooling".to_string()),
            corrective_action: Some("Add heatsink".to_string()),
            action_id: None,
        });

        let yaml = serde_yml::to_string(&result).unwrap();
        let parsed: Result = serde_yml::from_str(&yaml).unwrap();

        assert!(parsed.has_failures());
        assert_eq!(parsed.failures[0].description, "Device overheated");
    }

    #[test]
    fn test_entity_trait_implementation() {
        let test_id = EntityId::new(EntityPrefix::Test);
        let result = Result::new(
            test_id,
            Verdict::Pass,
            "tester".to_string(),
            "author".to_string(),
        );

        assert_eq!(Result::PREFIX, "RSLT");
        assert_eq!(result.status(), "draft");
        assert_eq!(result.author(), "author");
    }

    #[test]
    fn test_pass_rate() {
        let test_id = EntityId::new(EntityPrefix::Test);
        let mut result = Result::new(
            test_id,
            Verdict::Pass,
            "tester".to_string(),
            "author".to_string(),
        );

        // No steps - no pass rate
        assert!(result.pass_rate().is_none());

        // Add 3 steps: 2 pass, 1 fail
        result.step_results.push(StepResultRecord {
            step: 1,
            result: StepResult::Pass,
            observed: None,
            measurement: None,
            notes: None,
        });
        result.step_results.push(StepResultRecord {
            step: 2,
            result: StepResult::Pass,
            observed: None,
            measurement: None,
            notes: None,
        });
        result.step_results.push(StepResultRecord {
            step: 3,
            result: StepResult::Fail,
            observed: None,
            measurement: None,
            notes: None,
        });

        let rate = result.pass_rate().unwrap();
        assert!((rate - 66.67).abs() < 0.1);
    }

    #[test]
    fn test_step_count() {
        let test_id = EntityId::new(EntityPrefix::Test);
        let mut result = Result::new(
            test_id,
            Verdict::Pass,
            "tester".to_string(),
            "author".to_string(),
        );

        assert_eq!(result.step_count(), 0);

        result.step_results.push(StepResultRecord {
            step: 1,
            result: StepResult::Pass,
            observed: None,
            measurement: None,
            notes: None,
        });
        result.step_results.push(StepResultRecord {
            step: 2,
            result: StepResult::Pass,
            observed: None,
            measurement: None,
            notes: None,
        });

        assert_eq!(result.step_count(), 2);
    }
}
