//! Safety classification enums for hardware/software items
//!
//! These classifications are used by regulatory frameworks to determine
//! the level of rigor required for development, testing, and documentation.

use serde::{Deserialize, Serialize};

/// IEC 62304 Software Safety Class
///
/// Used for medical device software classification based on potential harm.
/// - Class A: No injury or damage to health is possible
/// - Class B: Non-serious injury is possible
/// - Class C: Death or serious injury is possible
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SwClass {
    /// Class A - No injury possible
    A,
    /// Class B - Non-serious injury possible
    B,
    /// Class C - Death or serious injury possible
    C,
}

impl std::fmt::Display for SwClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SwClass::A => write!(f, "A"),
            SwClass::B => write!(f, "B"),
            SwClass::C => write!(f, "C"),
        }
    }
}

impl std::str::FromStr for SwClass {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "A" => Ok(SwClass::A),
            "B" => Ok(SwClass::B),
            "C" => Ok(SwClass::C),
            _ => Err(format!("Unknown SW class: {}. Expected A, B, or C", s)),
        }
    }
}

/// ISO 26262 Automotive Safety Integrity Level (ASIL)
///
/// Used for automotive functional safety classification.
/// ASIL levels from lowest (QM) to highest (D) rigor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Asil {
    /// Quality Management - no specific safety requirements
    QM,
    /// ASIL A - lowest safety integrity level
    A,
    /// ASIL B
    B,
    /// ASIL C
    C,
    /// ASIL D - highest safety integrity level
    D,
}

impl std::fmt::Display for Asil {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Asil::QM => write!(f, "QM"),
            Asil::A => write!(f, "A"),
            Asil::B => write!(f, "B"),
            Asil::C => write!(f, "C"),
            Asil::D => write!(f, "D"),
        }
    }
}

impl std::str::FromStr for Asil {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "QM" => Ok(Asil::QM),
            "A" => Ok(Asil::A),
            "B" => Ok(Asil::B),
            "C" => Ok(Asil::C),
            "D" => Ok(Asil::D),
            _ => Err(format!(
                "Unknown ASIL: {}. Expected QM, A, B, C, or D",
                s
            )),
        }
    }
}

/// DO-178C Design Assurance Level (DAL)
///
/// Used for airborne systems and equipment certification.
/// DAL levels from lowest (E) to highest (A) rigor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Dal {
    /// Level E - No effect on aircraft safety (no requirements)
    E,
    /// Level D - Minor effect on aircraft safety
    D,
    /// Level C - Major effect on aircraft safety
    C,
    /// Level B - Hazardous/Severe-Major effect
    B,
    /// Level A - Catastrophic effect (highest rigor)
    A,
}

impl std::fmt::Display for Dal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Dal::E => write!(f, "E"),
            Dal::D => write!(f, "D"),
            Dal::C => write!(f, "C"),
            Dal::B => write!(f, "B"),
            Dal::A => write!(f, "A"),
        }
    }
}

impl std::str::FromStr for Dal {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "E" => Ok(Dal::E),
            "D" => Ok(Dal::D),
            "C" => Ok(Dal::C),
            "B" => Ok(Dal::B),
            "A" => Ok(Dal::A),
            _ => Err(format!(
                "Unknown DAL: {}. Expected E, D, C, B, or A",
                s
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sw_class_display() {
        assert_eq!(SwClass::A.to_string(), "A");
        assert_eq!(SwClass::B.to_string(), "B");
        assert_eq!(SwClass::C.to_string(), "C");
    }

    #[test]
    fn test_sw_class_from_str() {
        assert_eq!("a".parse::<SwClass>().unwrap(), SwClass::A);
        assert_eq!("B".parse::<SwClass>().unwrap(), SwClass::B);
        assert_eq!("c".parse::<SwClass>().unwrap(), SwClass::C);
        assert!("X".parse::<SwClass>().is_err());
    }

    #[test]
    fn test_sw_class_serialization() {
        assert_eq!(serde_yml::to_string(&SwClass::A).unwrap().trim(), "A");
        assert_eq!(serde_yml::to_string(&SwClass::C).unwrap().trim(), "C");
    }

    #[test]
    fn test_asil_display() {
        assert_eq!(Asil::QM.to_string(), "QM");
        assert_eq!(Asil::A.to_string(), "A");
        assert_eq!(Asil::D.to_string(), "D");
    }

    #[test]
    fn test_asil_from_str() {
        assert_eq!("qm".parse::<Asil>().unwrap(), Asil::QM);
        assert_eq!("QM".parse::<Asil>().unwrap(), Asil::QM);
        assert_eq!("d".parse::<Asil>().unwrap(), Asil::D);
        assert!("X".parse::<Asil>().is_err());
    }

    #[test]
    fn test_asil_serialization() {
        assert_eq!(serde_yml::to_string(&Asil::QM).unwrap().trim(), "QM");
        assert_eq!(serde_yml::to_string(&Asil::D).unwrap().trim(), "D");
    }

    #[test]
    fn test_dal_display() {
        assert_eq!(Dal::E.to_string(), "E");
        assert_eq!(Dal::A.to_string(), "A");
    }

    #[test]
    fn test_dal_from_str() {
        assert_eq!("e".parse::<Dal>().unwrap(), Dal::E);
        assert_eq!("A".parse::<Dal>().unwrap(), Dal::A);
        assert!("X".parse::<Dal>().is_err());
    }

    #[test]
    fn test_dal_serialization() {
        assert_eq!(serde_yml::to_string(&Dal::E).unwrap().trim(), "E");
        assert_eq!(serde_yml::to_string(&Dal::A).unwrap().trim(), "A");
    }
}
