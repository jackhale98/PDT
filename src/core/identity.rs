//! Entity identity system using type-prefixed ULIDs

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use thiserror::Error;
use ulid::Ulid;

/// Entity type prefixes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum EntityPrefix {
    /// Requirement (input or output)
    Req,
    /// Risk / FMEA item
    Risk,
    /// Verification or validation protocol
    Test,
    /// Test result / execution record
    Rslt,
    /// Tolerance stackup
    Tol,
    /// Feature mate (for stackups)
    Mate,
    /// Assembly
    Asm,
    /// Component
    Cmp,
    /// Feature (on a component)
    Feat,
    /// Manufacturing process
    Proc,
    /// Control plan item
    Ctrl,
    /// Quote / cost record
    Quot,
    /// Action item
    Act,
}

impl EntityPrefix {
    /// Get the string representation of the prefix
    pub fn as_str(&self) -> &'static str {
        match self {
            EntityPrefix::Req => "REQ",
            EntityPrefix::Risk => "RISK",
            EntityPrefix::Test => "TEST",
            EntityPrefix::Rslt => "RSLT",
            EntityPrefix::Tol => "TOL",
            EntityPrefix::Mate => "MATE",
            EntityPrefix::Asm => "ASM",
            EntityPrefix::Cmp => "CMP",
            EntityPrefix::Feat => "FEAT",
            EntityPrefix::Proc => "PROC",
            EntityPrefix::Ctrl => "CTRL",
            EntityPrefix::Quot => "QUOT",
            EntityPrefix::Act => "ACT",
        }
    }

    /// Get all valid prefixes
    pub fn all() -> &'static [EntityPrefix] {
        &[
            EntityPrefix::Req,
            EntityPrefix::Risk,
            EntityPrefix::Test,
            EntityPrefix::Rslt,
            EntityPrefix::Tol,
            EntityPrefix::Mate,
            EntityPrefix::Asm,
            EntityPrefix::Cmp,
            EntityPrefix::Feat,
            EntityPrefix::Proc,
            EntityPrefix::Ctrl,
            EntityPrefix::Quot,
            EntityPrefix::Act,
        ]
    }

    /// Try to determine entity prefix from a filename
    /// Looks for patterns like "REQ-xxx.pdt.yaml" or "req.schema.json"
    pub fn from_filename(filename: &str) -> Option<Self> {
        let upper = filename.to_uppercase();
        for prefix in Self::all() {
            let prefix_str = prefix.as_str();
            // Match "REQ-xxx" pattern at start
            if upper.starts_with(&format!("{}-", prefix_str)) {
                return Some(*prefix);
            }
            // Match "req.schema.json" pattern
            if upper.starts_with(&format!("{}.", prefix_str)) {
                return Some(*prefix);
            }
        }
        None
    }

    /// Try to determine entity prefix from a file path by examining parent directories
    pub fn from_path(path: &std::path::Path) -> Option<Self> {
        // First try the filename
        if let Some(filename) = path.file_name() {
            if let Some(prefix) = Self::from_filename(&filename.to_string_lossy()) {
                return Some(prefix);
            }
        }

        // Then look at parent directories
        for component in path.components() {
            if let std::path::Component::Normal(os_str) = component {
                let dir_name = os_str.to_string_lossy().to_lowercase();
                match dir_name.as_str() {
                    "requirements" | "inputs" | "outputs" => return Some(EntityPrefix::Req),
                    "risks" | "design" | "process" => return Some(EntityPrefix::Risk),
                    "verification" | "validation" | "protocols" => return Some(EntityPrefix::Test),
                    "results" => return Some(EntityPrefix::Rslt),
                    "tolerances" | "stackups" => return Some(EntityPrefix::Tol),
                    "mates" => return Some(EntityPrefix::Mate),
                    "assemblies" => return Some(EntityPrefix::Asm),
                    "components" => return Some(EntityPrefix::Cmp),
                    "features" => return Some(EntityPrefix::Feat),
                    "manufacturing" | "processes" => return Some(EntityPrefix::Proc),
                    "controls" => return Some(EntityPrefix::Ctrl),
                    "quotes" | "bom" => return Some(EntityPrefix::Quot),
                    _ => {}
                }
            }
        }
        None
    }
}

impl fmt::Display for EntityPrefix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for EntityPrefix {
    type Err = IdParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "REQ" => Ok(EntityPrefix::Req),
            "RISK" => Ok(EntityPrefix::Risk),
            "TEST" => Ok(EntityPrefix::Test),
            "RSLT" => Ok(EntityPrefix::Rslt),
            "TOL" => Ok(EntityPrefix::Tol),
            "MATE" => Ok(EntityPrefix::Mate),
            "ASM" => Ok(EntityPrefix::Asm),
            "CMP" => Ok(EntityPrefix::Cmp),
            "FEAT" => Ok(EntityPrefix::Feat),
            "PROC" => Ok(EntityPrefix::Proc),
            "CTRL" => Ok(EntityPrefix::Ctrl),
            "QUOT" => Ok(EntityPrefix::Quot),
            "ACT" => Ok(EntityPrefix::Act),
            _ => Err(IdParseError::InvalidPrefix(s.to_string())),
        }
    }
}

/// A unique entity identifier combining a type prefix and ULID
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EntityId {
    prefix: EntityPrefix,
    ulid: Ulid,
}

impl EntityId {
    /// Create a new EntityId with the given prefix
    pub fn new(prefix: EntityPrefix) -> Self {
        Self {
            prefix,
            ulid: Ulid::new(),
        }
    }

    /// Create an EntityId from a prefix and existing ULID
    pub fn from_parts(prefix: EntityPrefix, ulid: Ulid) -> Self {
        Self { prefix, ulid }
    }

    /// Get the entity prefix
    pub fn prefix(&self) -> EntityPrefix {
        self.prefix
    }

    /// Get the ULID component
    pub fn ulid(&self) -> Ulid {
        self.ulid
    }

    /// Parse an EntityId from a string
    pub fn parse(s: &str) -> Result<Self, IdParseError> {
        s.parse()
    }
}

impl fmt::Display for EntityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-{}", self.prefix, self.ulid)
    }
}

impl FromStr for EntityId {
    type Err = IdParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (prefix_str, ulid_str) = s
            .split_once('-')
            .ok_or_else(|| IdParseError::MissingDelimiter(s.to_string()))?;

        let prefix = prefix_str.parse()?;
        let ulid = Ulid::from_string(ulid_str)
            .map_err(|e| IdParseError::InvalidUlid(ulid_str.to_string(), e.to_string()))?;

        Ok(Self { prefix, ulid })
    }
}

impl Serialize for EntityId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for EntityId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

/// Errors that can occur when parsing entity IDs
#[derive(Debug, Error)]
pub enum IdParseError {
    #[error("invalid entity prefix: '{0}' (valid: REQ, RISK, TEST, RSLT, TOL, MATE, ASM, CMP, FEAT, PROC, CTRL, QUOT, ACT)")]
    InvalidPrefix(String),

    #[error("missing '-' delimiter in entity ID: '{0}'")]
    MissingDelimiter(String),

    #[error("invalid ULID '{0}': {1}")]
    InvalidUlid(String, String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entity_id_generation() {
        let id = EntityId::new(EntityPrefix::Req);
        assert!(id.to_string().starts_with("REQ-"));
        assert_eq!(id.to_string().len(), 30); // REQ- (4) + ULID (26) = 30
    }

    #[test]
    fn test_entity_id_parsing() {
        // Generate a valid ID first, then parse it back
        let original = EntityId::new(EntityPrefix::Req);
        let id_str = original.to_string();
        let parsed = EntityId::parse(&id_str).unwrap();
        assert_eq!(parsed.prefix(), EntityPrefix::Req);
        assert_eq!(original, parsed);
    }

    #[test]
    fn test_entity_id_roundtrip() {
        let original = EntityId::new(EntityPrefix::Risk);
        let serialized = original.to_string();
        let parsed = EntityId::parse(&serialized).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn test_entity_id_invalid_prefix() {
        let err = EntityId::parse("XXX-01HQ3K4N5M6P7R8S9T0UVWXYZ").unwrap_err();
        assert!(matches!(err, IdParseError::InvalidPrefix(_)));
    }

    #[test]
    fn test_entity_id_missing_delimiter() {
        let err = EntityId::parse("REQ01HQ3K4N5M6P7R8S9T0UVWXYZ").unwrap_err();
        assert!(matches!(err, IdParseError::MissingDelimiter(_)));
    }

    #[test]
    fn test_entity_id_invalid_ulid() {
        let err = EntityId::parse("REQ-notaulid").unwrap_err();
        assert!(matches!(err, IdParseError::InvalidUlid(_, _)));
    }

    #[test]
    fn test_all_prefixes_parse() {
        for prefix in EntityPrefix::all() {
            let id = EntityId::new(*prefix);
            let parsed = EntityId::parse(&id.to_string()).unwrap();
            assert_eq!(parsed.prefix(), *prefix);
        }
    }
}
