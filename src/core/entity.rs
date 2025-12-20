//! Entity trait - common interface for all entity types

use chrono::{DateTime, Utc};
use serde::{de::DeserializeOwned, Serialize};

use crate::core::identity::EntityId;

/// Common trait for all TDT entities
pub trait Entity: Serialize + DeserializeOwned {
    /// The entity type prefix (e.g., "REQ", "RISK")
    const PREFIX: &'static str;

    /// Get the entity's unique ID
    fn id(&self) -> &EntityId;

    /// Get the entity's title
    fn title(&self) -> &str;

    /// Get the entity's status
    fn status(&self) -> &str;

    /// Get the creation timestamp
    fn created(&self) -> DateTime<Utc>;

    /// Get the author
    fn author(&self) -> &str;
}

/// Status values common across entity types
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum Status {
    #[default]
    Draft,
    Review,
    Approved,
    Released,
    Obsolete,
}

impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Status::Draft => write!(f, "draft"),
            Status::Review => write!(f, "review"),
            Status::Approved => write!(f, "approved"),
            Status::Released => write!(f, "released"),
            Status::Obsolete => write!(f, "obsolete"),
        }
    }
}

impl std::str::FromStr for Status {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "draft" => Ok(Status::Draft),
            "review" => Ok(Status::Review),
            "approved" => Ok(Status::Approved),
            "released" => Ok(Status::Released),
            "obsolete" => Ok(Status::Obsolete),
            _ => Err(format!("Unknown status: {}", s)),
        }
    }
}

/// Priority values common across entity types
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum Priority {
    Low,
    #[default]
    Medium,
    High,
    Critical,
}

impl std::fmt::Display for Priority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Priority::Low => write!(f, "low"),
            Priority::Medium => write!(f, "medium"),
            Priority::High => write!(f, "high"),
            Priority::Critical => write!(f, "critical"),
        }
    }
}

impl std::str::FromStr for Priority {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "low" => Ok(Priority::Low),
            "medium" => Ok(Priority::Medium),
            "high" => Ok(Priority::High),
            "critical" => Ok(Priority::Critical),
            _ => Err(format!("Unknown priority: {}", s)),
        }
    }
}
