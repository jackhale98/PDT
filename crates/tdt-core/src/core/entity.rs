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
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
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
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
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

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // Status tests
    // =========================================================================

    #[test]
    fn test_status_default() {
        assert_eq!(Status::default(), Status::Draft);
    }

    #[test]
    fn test_status_display() {
        assert_eq!(format!("{}", Status::Draft), "draft");
        assert_eq!(format!("{}", Status::Review), "review");
        assert_eq!(format!("{}", Status::Approved), "approved");
        assert_eq!(format!("{}", Status::Released), "released");
        assert_eq!(format!("{}", Status::Obsolete), "obsolete");
    }

    #[test]
    fn test_status_from_str_valid() {
        assert_eq!("draft".parse::<Status>().unwrap(), Status::Draft);
        assert_eq!("review".parse::<Status>().unwrap(), Status::Review);
        assert_eq!("approved".parse::<Status>().unwrap(), Status::Approved);
        assert_eq!("released".parse::<Status>().unwrap(), Status::Released);
        assert_eq!("obsolete".parse::<Status>().unwrap(), Status::Obsolete);
    }

    #[test]
    fn test_status_from_str_case_insensitive() {
        assert_eq!("DRAFT".parse::<Status>().unwrap(), Status::Draft);
        assert_eq!("Draft".parse::<Status>().unwrap(), Status::Draft);
        assert_eq!("REVIEW".parse::<Status>().unwrap(), Status::Review);
        assert_eq!("Approved".parse::<Status>().unwrap(), Status::Approved);
    }

    #[test]
    fn test_status_from_str_invalid() {
        let result = "invalid".parse::<Status>();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown status"));
    }

    #[test]
    fn test_status_ordering() {
        // Status has PartialOrd based on declaration order
        assert!(Status::Draft < Status::Review);
        assert!(Status::Review < Status::Approved);
        assert!(Status::Approved < Status::Released);
        assert!(Status::Released < Status::Obsolete);
    }

    #[test]
    fn test_status_serde_serialize() {
        let status = Status::Approved;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"approved\"");
    }

    #[test]
    fn test_status_serde_deserialize() {
        let status: Status = serde_json::from_str("\"released\"").unwrap();
        assert_eq!(status, Status::Released);
    }

    #[test]
    fn test_status_yaml_roundtrip() {
        let status = Status::Review;
        let yaml = serde_yml::to_string(&status).unwrap();
        assert!(yaml.contains("review"));
        let parsed: Status = serde_yml::from_str(&yaml).unwrap();
        assert_eq!(parsed, Status::Review);
    }

    #[test]
    #[allow(clippy::clone_on_copy)] // clone is exactly what this test exercises
    fn test_status_clone_copy() {
        let status = Status::Draft;
        let cloned = status.clone();
        let copied = status;
        assert_eq!(status, cloned);
        assert_eq!(status, copied);
    }

    // =========================================================================
    // Priority tests
    // =========================================================================

    #[test]
    fn test_priority_default() {
        assert_eq!(Priority::default(), Priority::Medium);
    }

    #[test]
    fn test_priority_display() {
        assert_eq!(format!("{}", Priority::Low), "low");
        assert_eq!(format!("{}", Priority::Medium), "medium");
        assert_eq!(format!("{}", Priority::High), "high");
        assert_eq!(format!("{}", Priority::Critical), "critical");
    }

    #[test]
    fn test_priority_from_str_valid() {
        assert_eq!("low".parse::<Priority>().unwrap(), Priority::Low);
        assert_eq!("medium".parse::<Priority>().unwrap(), Priority::Medium);
        assert_eq!("high".parse::<Priority>().unwrap(), Priority::High);
        assert_eq!("critical".parse::<Priority>().unwrap(), Priority::Critical);
    }

    #[test]
    fn test_priority_from_str_case_insensitive() {
        assert_eq!("LOW".parse::<Priority>().unwrap(), Priority::Low);
        assert_eq!("Low".parse::<Priority>().unwrap(), Priority::Low);
        assert_eq!("CRITICAL".parse::<Priority>().unwrap(), Priority::Critical);
        assert_eq!("High".parse::<Priority>().unwrap(), Priority::High);
    }

    #[test]
    fn test_priority_from_str_invalid() {
        let result = "urgent".parse::<Priority>();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown priority"));
    }

    #[test]
    fn test_priority_ordering() {
        // Priority has PartialOrd based on declaration order
        assert!(Priority::Low < Priority::Medium);
        assert!(Priority::Medium < Priority::High);
        assert!(Priority::High < Priority::Critical);
    }

    #[test]
    fn test_priority_serde_serialize() {
        let priority = Priority::High;
        let json = serde_json::to_string(&priority).unwrap();
        assert_eq!(json, "\"high\"");
    }

    #[test]
    fn test_priority_serde_deserialize() {
        let priority: Priority = serde_json::from_str("\"critical\"").unwrap();
        assert_eq!(priority, Priority::Critical);
    }

    #[test]
    fn test_priority_yaml_roundtrip() {
        let priority = Priority::Low;
        let yaml = serde_yml::to_string(&priority).unwrap();
        assert!(yaml.contains("low"));
        let parsed: Priority = serde_yml::from_str(&yaml).unwrap();
        assert_eq!(parsed, Priority::Low);
    }

    #[test]
    #[allow(clippy::clone_on_copy)] // clone is exactly what this test exercises
    fn test_priority_clone_copy() {
        let priority = Priority::Critical;
        let cloned = priority.clone();
        let copied = priority;
        assert_eq!(priority, cloned);
        assert_eq!(priority, copied);
    }

    // =========================================================================
    // Edge case tests
    // =========================================================================

    #[test]
    fn test_status_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(Status::Draft);
        set.insert(Status::Review);
        set.insert(Status::Draft); // Duplicate
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_status_debug() {
        let status = Status::Approved;
        let debug = format!("{:?}", status);
        assert_eq!(debug, "Approved");
    }

    #[test]
    fn test_priority_debug() {
        let priority = Priority::High;
        let debug = format!("{:?}", priority);
        assert_eq!(debug, "High");
    }
}
