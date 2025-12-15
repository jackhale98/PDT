//! Test entity type (Verification/Validation Protocol)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::core::entity::{Entity, Priority, Status};
use crate::core::identity::EntityId;

/// Test type - verification or validation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TestType {
    Verification,
    Validation,
}

impl Default for TestType {
    fn default() -> Self {
        TestType::Verification
    }
}

impl std::fmt::Display for TestType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TestType::Verification => write!(f, "verification"),
            TestType::Validation => write!(f, "validation"),
        }
    }
}

/// Test level in the V-model hierarchy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TestLevel {
    Unit,
    Integration,
    System,
    Acceptance,
}

impl Default for TestLevel {
    fn default() -> Self {
        TestLevel::System
    }
}

impl std::fmt::Display for TestLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TestLevel::Unit => write!(f, "unit"),
            TestLevel::Integration => write!(f, "integration"),
            TestLevel::System => write!(f, "system"),
            TestLevel::Acceptance => write!(f, "acceptance"),
        }
    }
}

/// Test method (IADT)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TestMethod {
    Inspection,
    Analysis,
    Demonstration,
    Test,
}

impl Default for TestMethod {
    fn default() -> Self {
        TestMethod::Test
    }
}

impl std::fmt::Display for TestMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TestMethod::Inspection => write!(f, "inspection"),
            TestMethod::Analysis => write!(f, "analysis"),
            TestMethod::Demonstration => write!(f, "demonstration"),
            TestMethod::Test => write!(f, "test"),
        }
    }
}

/// Equipment required for a test
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Equipment {
    /// Equipment name or description
    pub name: String,

    /// Required specification
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub specification: Option<String>,

    /// Whether calibration is required
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calibration_required: Option<bool>,
}

/// A procedure step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcedureStep {
    /// Step number
    pub step: u32,

    /// Action to perform
    pub action: String,

    /// Expected outcome
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected: Option<String>,

    /// Pass/fail criteria for this step
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acceptance: Option<String>,
}

/// Sample size information
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SampleSize {
    /// Number of samples
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quantity: Option<u32>,

    /// Rationale for sample size
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rationale: Option<String>,

    /// How samples are selected
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sampling_method: Option<String>,
}

/// Environmental conditions
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Environment {
    /// Temperature conditions
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<String>,

    /// Humidity conditions
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub humidity: Option<String>,

    /// Other environmental conditions
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub other: Option<String>,
}

/// Links to other entities
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TestLinks {
    /// Requirements this test verifies
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub verifies: Vec<EntityId>,

    /// User needs this test validates
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub validates: Vec<EntityId>,

    /// Risks whose mitigation this test verifies
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mitigates: Vec<EntityId>,

    /// Tests that must pass before this one
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends_on: Vec<EntityId>,
}

/// A test entity (verification/validation protocol)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Test {
    /// Unique identifier
    pub id: EntityId,

    /// Test type (verification or validation)
    #[serde(rename = "type")]
    pub test_type: TestType,

    /// Test level in V-model
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub test_level: Option<TestLevel>,

    /// Test method (IADT)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub test_method: Option<TestMethod>,

    /// Short title
    pub title: String,

    /// Category (user-defined)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,

    /// Tags for filtering
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,

    /// What this test verifies or validates
    pub objective: String,

    /// Detailed description
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Preconditions that must be met
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub preconditions: Vec<String>,

    /// Equipment required
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub equipment: Vec<Equipment>,

    /// Step-by-step procedure
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub procedure: Vec<ProcedureStep>,

    /// Overall pass/fail criteria
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub acceptance_criteria: Vec<String>,

    /// Sample size information
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample_size: Option<SampleSize>,

    /// Environmental conditions
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub environment: Option<Environment>,

    /// Estimated duration
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimated_duration: Option<String>,

    /// Priority level
    #[serde(default)]
    pub priority: Priority,

    /// Current status
    #[serde(default)]
    pub status: Status,

    /// Links to other entities
    #[serde(default)]
    pub links: TestLinks,

    /// Creation timestamp
    pub created: DateTime<Utc>,

    /// Author (who created this test)
    pub author: String,

    /// Revision number
    #[serde(default = "default_revision")]
    pub revision: u32,
}

fn default_revision() -> u32 {
    1
}

impl Entity for Test {
    const PREFIX: &'static str = "TEST";

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

impl Test {
    /// Create a new test with the given parameters
    pub fn new(test_type: TestType, title: String, objective: String, author: String) -> Self {
        Self {
            id: EntityId::new(crate::core::EntityPrefix::Test),
            test_type,
            test_level: None,
            test_method: None,
            title,
            category: None,
            tags: Vec::new(),
            objective,
            description: None,
            preconditions: Vec::new(),
            equipment: Vec::new(),
            procedure: Vec::new(),
            acceptance_criteria: Vec::new(),
            sample_size: None,
            environment: None,
            estimated_duration: None,
            priority: Priority::default(),
            status: Status::default(),
            links: TestLinks::default(),
            created: Utc::now(),
            author,
            revision: 1,
        }
    }

    /// Get the number of procedure steps
    pub fn step_count(&self) -> usize {
        self.procedure.len()
    }

    /// Check if the test has any linked requirements
    pub fn has_verifications(&self) -> bool {
        !self.links.verifies.is_empty()
    }

    /// Check if the test has any linked risks
    pub fn has_mitigations(&self) -> bool {
        !self.links.mitigates.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_test_creation() {
        let test = Test::new(
            TestType::Verification,
            "Temperature Range Test".to_string(),
            "Verify device operates within temperature range".to_string(),
            "test".to_string(),
        );

        assert!(test.id.to_string().starts_with("TEST-"));
        assert_eq!(test.title, "Temperature Range Test");
        assert_eq!(test.test_type, TestType::Verification);
        assert_eq!(test.status, Status::Draft);
    }

    #[test]
    fn test_test_roundtrip() {
        let mut test = Test::new(
            TestType::Validation,
            "User Acceptance Test".to_string(),
            "Validate user requirements".to_string(),
            "test".to_string(),
        );
        test.test_level = Some(TestLevel::Acceptance);
        test.test_method = Some(TestMethod::Demonstration);
        test.priority = Priority::High;

        let yaml = serde_yml::to_string(&test).unwrap();
        let parsed: Test = serde_yml::from_str(&yaml).unwrap();

        assert_eq!(test.id, parsed.id);
        assert_eq!(test.title, parsed.title);
        assert_eq!(test.test_level, parsed.test_level);
        assert_eq!(test.priority, parsed.priority);
    }

    #[test]
    fn test_test_serializes_type_correctly() {
        let test = Test::new(
            TestType::Verification,
            "Test".to_string(),
            "Objective".to_string(),
            "test".to_string(),
        );

        let yaml = serde_yml::to_string(&test).unwrap();
        assert!(yaml.contains("type: verification"));
    }

    #[test]
    fn test_test_serializes_validation_type() {
        let test = Test::new(
            TestType::Validation,
            "Test".to_string(),
            "Objective".to_string(),
            "test".to_string(),
        );

        let yaml = serde_yml::to_string(&test).unwrap();
        assert!(yaml.contains("type: validation"));
    }

    #[test]
    fn test_test_with_procedure() {
        let mut test = Test::new(
            TestType::Verification,
            "Procedure Test".to_string(),
            "Test with procedure".to_string(),
            "test".to_string(),
        );

        test.procedure.push(ProcedureStep {
            step: 1,
            action: "Do something".to_string(),
            expected: Some("Something happens".to_string()),
            acceptance: Some("Pass if something happens".to_string()),
        });

        let yaml = serde_yml::to_string(&test).unwrap();
        let parsed: Test = serde_yml::from_str(&yaml).unwrap();

        assert_eq!(parsed.procedure.len(), 1);
        assert_eq!(parsed.procedure[0].step, 1);
        assert_eq!(parsed.procedure[0].action, "Do something");
    }

    #[test]
    fn test_entity_trait_implementation() {
        let test = Test::new(
            TestType::Verification,
            "Test Protocol".to_string(),
            "Test objective".to_string(),
            "author".to_string(),
        );

        assert_eq!(Test::PREFIX, "TEST");
        assert_eq!(test.title(), "Test Protocol");
        assert_eq!(test.status(), "draft");
        assert_eq!(test.author(), "author");
    }

    #[test]
    fn test_test_links() {
        let mut test = Test::new(
            TestType::Verification,
            "Test".to_string(),
            "Objective".to_string(),
            "test".to_string(),
        );

        let req_id = EntityId::new(crate::core::EntityPrefix::Req);
        let risk_id = EntityId::new(crate::core::EntityPrefix::Risk);

        test.links.verifies.push(req_id.clone());
        test.links.mitigates.push(risk_id.clone());

        let yaml = serde_yml::to_string(&test).unwrap();
        let parsed: Test = serde_yml::from_str(&yaml).unwrap();

        assert_eq!(parsed.links.verifies.len(), 1);
        assert_eq!(parsed.links.mitigates.len(), 1);
        assert_eq!(parsed.links.verifies[0], req_id);
        assert_eq!(parsed.links.mitigates[0], risk_id);
    }

    #[test]
    fn test_step_count() {
        let mut test = Test::new(
            TestType::Verification,
            "Test".to_string(),
            "Objective".to_string(),
            "test".to_string(),
        );

        assert_eq!(test.step_count(), 0);

        test.procedure.push(ProcedureStep {
            step: 1,
            action: "Step 1".to_string(),
            expected: None,
            acceptance: None,
        });
        test.procedure.push(ProcedureStep {
            step: 2,
            action: "Step 2".to_string(),
            expected: None,
            acceptance: None,
        });

        assert_eq!(test.step_count(), 2);
    }

    #[test]
    fn test_has_verifications() {
        let mut test = Test::new(
            TestType::Verification,
            "Test".to_string(),
            "Objective".to_string(),
            "test".to_string(),
        );

        assert!(!test.has_verifications());

        test.links
            .verifies
            .push(EntityId::new(crate::core::EntityPrefix::Req));
        assert!(test.has_verifications());
    }
}
