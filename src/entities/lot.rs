//! LOT entity type - Production Lot / Batch (Device History Record)

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::core::entity::{Entity, Status};
use crate::core::identity::EntityId;

/// Lot status (production state)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum LotStatus {
    #[default]
    InProgress,
    OnHold,
    Completed,
    Scrapped,
}

impl std::fmt::Display for LotStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LotStatus::InProgress => write!(f, "in_progress"),
            LotStatus::OnHold => write!(f, "on_hold"),
            LotStatus::Completed => write!(f, "completed"),
            LotStatus::Scrapped => write!(f, "scrapped"),
        }
    }
}

impl std::str::FromStr for LotStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "in_progress" | "inprogress" => Ok(LotStatus::InProgress),
            "on_hold" | "onhold" => Ok(LotStatus::OnHold),
            "completed" => Ok(LotStatus::Completed),
            "scrapped" => Ok(LotStatus::Scrapped),
            _ => Err(format!(
                "Invalid lot status: {}. Use in_progress, on_hold, completed, or scrapped",
                s
            )),
        }
    }
}

/// Execution step status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum ExecutionStatus {
    #[default]
    Pending,
    InProgress,
    Completed,
    Skipped,
}

impl std::fmt::Display for ExecutionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecutionStatus::Pending => write!(f, "pending"),
            ExecutionStatus::InProgress => write!(f, "in_progress"),
            ExecutionStatus::Completed => write!(f, "completed"),
            ExecutionStatus::Skipped => write!(f, "skipped"),
        }
    }
}

impl std::str::FromStr for ExecutionStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pending" => Ok(ExecutionStatus::Pending),
            "in_progress" | "inprogress" => Ok(ExecutionStatus::InProgress),
            "completed" => Ok(ExecutionStatus::Completed),
            "skipped" => Ok(ExecutionStatus::Skipped),
            _ => Err(format!(
                "Invalid execution status: {}. Use pending, in_progress, completed, or skipped",
                s
            )),
        }
    }
}

/// Material used in production (for traceability)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MaterialUsed {
    /// Component ID (CMP-xxx)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub component: Option<String>,

    /// Supplier lot number (free text for traceability)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supplier_lot: Option<String>,

    /// Quantity used
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quantity: Option<u32>,
}

/// Execution step record
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExecutionStep {
    /// Process ID (PROC-xxx)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process: Option<String>,

    /// Execution status
    #[serde(default)]
    pub status: ExecutionStatus,

    /// Date completed
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_date: Option<NaiveDate>,

    /// Operator who performed the step
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operator: Option<String>,

    /// Notes about execution
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,

    /// Measurement/inspection data (key-value pairs)
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub data: HashMap<String, serde_json::Value>,
}

/// Links for LOT entity
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LotLinks {
    /// Product being made (ASM or CMP ID)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub product: Option<String>,

    /// Process entities in sequence
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub processes: Vec<String>,

    /// Work instruction entities
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub work_instructions: Vec<String>,

    /// NCRs raised during production
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ncrs: Vec<String>,

    /// In-process inspection results
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub results: Vec<String>,
}

/// Production Lot / Batch entity (Device History Record)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lot {
    /// Unique identifier (LOT-xxx)
    pub id: EntityId,

    /// Descriptive title
    pub title: String,

    /// User-defined lot number
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lot_number: Option<String>,

    /// Quantity in this lot
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quantity: Option<u32>,

    /// Production status
    #[serde(default)]
    pub lot_status: LotStatus,

    /// Production start date
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_date: Option<NaiveDate>,

    /// Production completion date
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completion_date: Option<NaiveDate>,

    /// Materials used (for traceability)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub materials_used: Vec<MaterialUsed>,

    /// Process execution records
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub execution: Vec<ExecutionStep>,

    /// Production notes
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,

    /// Entity links
    #[serde(default)]
    pub links: LotLinks,

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

impl Entity for Lot {
    const PREFIX: &'static str = "LOT";

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

impl Lot {
    /// Create a new Lot
    pub fn new(title: String, author: String) -> Self {
        Self {
            id: EntityId::new(crate::core::identity::EntityPrefix::Lot),
            title,
            lot_number: None,
            quantity: None,
            lot_status: LotStatus::default(),
            start_date: None,
            completion_date: None,
            materials_used: Vec::new(),
            execution: Vec::new(),
            notes: None,
            links: LotLinks::default(),
            status: Status::Draft,
            created: Utc::now(),
            author,
            entity_revision: 1,
        }
    }

    /// Create a new Lot with lot number
    pub fn with_lot_number(title: String, lot_number: String, author: String) -> Self {
        let mut lot = Self::new(title, author);
        lot.lot_number = Some(lot_number);
        lot
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lot_creation() {
        let lot = Lot::new("Test Lot".to_string(), "Test Author".to_string());
        assert!(lot.id.to_string().starts_with("LOT-"));
        assert_eq!(lot.title, "Test Lot");
        assert_eq!(lot.author, "Test Author");
        assert_eq!(lot.lot_status, LotStatus::InProgress);
    }

    #[test]
    fn test_lot_status_parsing() {
        assert_eq!(
            "in_progress".parse::<LotStatus>().unwrap(),
            LotStatus::InProgress
        );
        assert_eq!(
            "completed".parse::<LotStatus>().unwrap(),
            LotStatus::Completed
        );
        assert_eq!("on_hold".parse::<LotStatus>().unwrap(), LotStatus::OnHold);
        assert_eq!(
            "scrapped".parse::<LotStatus>().unwrap(),
            LotStatus::Scrapped
        );
    }

    #[test]
    fn test_lot_serialization() {
        let lot = Lot::new("Test Lot".to_string(), "Test Author".to_string());
        let yaml = serde_yml::to_string(&lot).unwrap();
        assert!(yaml.contains("LOT-"));
        assert!(yaml.contains("Test Lot"));
    }

    #[test]
    fn test_lot_deserialization() {
        let yaml = r#"
id: LOT-01HC2JB7SMQX7RS1Y0GFKBHPTD
title: "Production Lot 001"
lot_number: "2024-001"
quantity: 25
lot_status: in_progress
materials_used:
  - component: CMP-01HC2JB7SMQX7RS1Y0GFKBHPTE
    supplier_lot: "ABC-123"
    quantity: 25
execution: []
links:
  product: ASM-01HC2JB7SMQX7RS1Y0GFKBHPTF
  processes: []
status: draft
created: 2024-01-15T10:00:00Z
author: "Test Author"
entity_revision: 1
"#;
        let lot: Lot = serde_yml::from_str(yaml).unwrap();
        assert_eq!(lot.title, "Production Lot 001");
        assert_eq!(lot.lot_number, Some("2024-001".to_string()));
        assert_eq!(lot.quantity, Some(25));
        assert_eq!(lot.lot_status, LotStatus::InProgress);
        assert_eq!(lot.materials_used.len(), 1);
    }
}
