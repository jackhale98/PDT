//! Entity trait - common interface for all entity types

use chrono::{DateTime, Utc};
use serde::{de::DeserializeOwned, Serialize};

use crate::core::identity::EntityId;

/// Common trait for all PDT entities
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Draft,
    Review,
    Approved,
    Released,
    Obsolete,
}

impl Default for Status {
    fn default() -> Self {
        Status::Draft
    }
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

/// Priority values common across entity types
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    Low,
    Medium,
    High,
    Critical,
}

impl Default for Priority {
    fn default() -> Self {
        Priority::Medium
    }
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
