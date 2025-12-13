//! Feature entity - Dimensional features on components

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::core::entity::{Entity, Status};
use crate::core::identity::{EntityId, EntityPrefix};
use crate::entities::stackup::Distribution;

/// Feature type classification - determines MMC/LMC behavior for tolerance analysis
///
/// This is the key distinction for tolerance stackups and fit calculations:
/// - **Internal**: Material is removed (holes, bores, pockets, slots). MMC = smallest size.
/// - **External**: Material remains (shafts, bosses, pins). MMC = largest size.
///
/// Specific geometry (counterbore, thread, etc.) can be described in the title/description.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeatureType {
    /// Internal feature (hole, bore, pocket, slot) - material is removed, MMC = smallest
    Internal,
    /// External feature (shaft, boss, pin) - material remains, MMC = largest
    External,
}

impl Default for FeatureType {
    fn default() -> Self {
        FeatureType::Internal
    }
}

impl std::fmt::Display for FeatureType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FeatureType::Internal => write!(f, "internal"),
            FeatureType::External => write!(f, "external"),
        }
    }
}

impl std::str::FromStr for FeatureType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "internal" => Ok(FeatureType::Internal),
            "external" => Ok(FeatureType::External),
            // Legacy mappings for backward compatibility
            "slot" | "pocket" | "counterbore" | "countersink" | "thread" => Ok(FeatureType::Internal),
            "boss" => Ok(FeatureType::External),
            "planar_surface" | "edge" | "other" => Ok(FeatureType::Internal),
            _ => Err(format!(
                "Invalid feature type: '{}'. Use 'internal' or 'external'",
                s
            )),
        }
    }
}

/// A dimensional characteristic with tolerances
/// Uses plus_tol and minus_tol instead of +/- symbol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dimension {
    /// Dimension name (e.g., "diameter", "length", "depth")
    pub name: String,

    /// Nominal value
    pub nominal: f64,

    /// Plus tolerance (stored as positive number)
    /// Example: 0.1 means +0.1
    pub plus_tol: f64,

    /// Minus tolerance (stored as positive number)
    /// Example: 0.05 means -0.05
    pub minus_tol: f64,

    /// Units (mm, in, etc.)
    #[serde(default = "default_units")]
    pub units: String,

    /// Whether this is an internal feature (hole, slot, pocket) vs external (shaft, boss)
    /// Internal: material is removed (MMC = smallest)
    /// External: material remains (MMC = largest)
    #[serde(default)]
    pub internal: bool,

    /// Statistical distribution for tolerance analysis
    /// Used when this feature is added to a stackup
    #[serde(default)]
    pub distribution: Distribution,
}

fn default_units() -> String {
    "mm".to_string()
}

impl Dimension {
    /// Get the maximum material condition value
    /// Internal features (holes): MMC = smallest = nominal - minus_tol
    /// External features (shafts): MMC = largest = nominal + plus_tol
    pub fn mmc(&self) -> f64 {
        if self.internal {
            self.nominal - self.minus_tol
        } else {
            self.nominal + self.plus_tol
        }
    }

    /// Get the least material condition value
    /// Internal features (holes): LMC = largest = nominal + plus_tol
    /// External features (shafts): LMC = smallest = nominal - minus_tol
    pub fn lmc(&self) -> f64 {
        if self.internal {
            self.nominal + self.plus_tol
        } else {
            self.nominal - self.minus_tol
        }
    }

    /// Get the total tolerance band
    pub fn tolerance_band(&self) -> f64 {
        self.plus_tol + self.minus_tol
    }
}

/// GD&T symbol types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GdtSymbol {
    Position,
    Flatness,
    Perpendicularity,
    Parallelism,
    Concentricity,
    Runout,
    TotalRunout,
    ProfileSurface,
    ProfileLine,
    Circularity,
    Cylindricity,
    Straightness,
    Angularity,
    Symmetry,
}

/// Material condition modifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MaterialCondition {
    /// Maximum Material Condition
    Mmc,
    /// Least Material Condition
    Lmc,
    /// Regardless of Feature Size
    Rfs,
}

impl Default for MaterialCondition {
    fn default() -> Self {
        MaterialCondition::Rfs
    }
}

/// Geometric Dimensioning and Tolerancing control
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GdtControl {
    /// GD&T symbol
    pub symbol: GdtSymbol,

    /// Tolerance value
    pub value: f64,

    /// Units
    #[serde(default = "default_units")]
    pub units: String,

    /// Datum references (e.g., ["A", "B", "C"])
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub datum_refs: Vec<String>,

    /// Material condition modifier
    #[serde(default)]
    pub material_condition: MaterialCondition,
}

/// Drawing reference
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DrawingRef {
    /// Drawing number
    #[serde(default)]
    pub number: String,

    /// Drawing revision
    #[serde(default)]
    pub revision: String,

    /// Zone on drawing (e.g., "B3")
    #[serde(default)]
    pub zone: String,
}

/// Feature links
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FeatureLinks {
    /// Mates using this feature
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub used_in_mates: Vec<String>,

    /// Stackups using this feature
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub used_in_stackups: Vec<String>,

    /// Requirements allocated to this feature (reciprocal of REQ.allocated_to)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allocated_from: Vec<EntityId>,

    /// Risks affecting this feature (reciprocal of RISK.affects)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub risks: Vec<EntityId>,
}

/// Feature entity - dimensional feature on a component
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Feature {
    /// Unique identifier (FEAT-...)
    pub id: EntityId,

    /// REQUIRED: Parent component ID (CMP-...)
    pub component: String,

    /// Feature type classification
    pub feature_type: FeatureType,

    /// Feature title/name
    pub title: String,

    /// Detailed description
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Dimensional characteristics with tolerances
    #[serde(default)]
    pub dimensions: Vec<Dimension>,

    /// GD&T controls
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gdt: Vec<GdtControl>,

    /// Drawing reference
    #[serde(default)]
    pub drawing: DrawingRef,

    /// Classification tags
    #[serde(default)]
    pub tags: Vec<String>,

    /// Current status
    #[serde(default)]
    pub status: Status,

    /// Links to other entities
    #[serde(default)]
    pub links: FeatureLinks,

    /// Creation timestamp
    pub created: DateTime<Utc>,

    /// Author name
    pub author: String,

    /// Revision counter
    #[serde(default = "default_revision")]
    pub entity_revision: u32,
}

fn default_revision() -> u32 {
    1
}

impl Entity for Feature {
    const PREFIX: &'static str = "FEAT";

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

impl Default for Feature {
    fn default() -> Self {
        Self {
            id: EntityId::new(EntityPrefix::Feat),
            component: String::new(),
            feature_type: FeatureType::default(),
            title: String::new(),
            description: None,
            dimensions: Vec::new(),
            gdt: Vec::new(),
            drawing: DrawingRef::default(),
            tags: Vec::new(),
            status: Status::default(),
            links: FeatureLinks::default(),
            created: Utc::now(),
            author: String::new(),
            entity_revision: 1,
        }
    }
}

impl Feature {
    /// Create a new feature with required fields
    pub fn new(
        component: impl Into<String>,
        feature_type: FeatureType,
        title: impl Into<String>,
        author: impl Into<String>,
    ) -> Self {
        Self {
            id: EntityId::new(EntityPrefix::Feat),
            component: component.into(),
            feature_type,
            title: title.into(),
            author: author.into(),
            created: Utc::now(),
            ..Default::default()
        }
    }

    /// Add a dimension to this feature
    pub fn add_dimension(&mut self, name: impl Into<String>, nominal: f64, plus_tol: f64, minus_tol: f64, internal: bool) {
        self.dimensions.push(Dimension {
            name: name.into(),
            nominal,
            plus_tol,
            minus_tol,
            units: "mm".to_string(),
            internal,
            distribution: Distribution::default(),
        });
    }

    /// Get the primary dimension (first one, typically the main characteristic)
    pub fn primary_dimension(&self) -> Option<&Dimension> {
        self.dimensions.first()
    }

    /// Check if this feature has any GD&T controls
    pub fn has_gdt(&self) -> bool {
        !self.gdt.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feature_creation() {
        let feat = Feature::new("CMP-123", FeatureType::Internal, "Mounting Hole A", "Test Author");
        assert_eq!(feat.component, "CMP-123");
        assert_eq!(feat.feature_type, FeatureType::Internal);
        assert_eq!(feat.title, "Mounting Hole A");
        assert_eq!(feat.author, "Test Author");
        assert_eq!(feat.status, Status::Draft);
    }

    #[test]
    fn test_dimension_calculations_external() {
        // External feature (shaft): MMC = largest, LMC = smallest
        let dim = Dimension {
            name: "diameter".to_string(),
            nominal: 10.0,
            plus_tol: 0.1,
            minus_tol: 0.05,
            units: "mm".to_string(),
            internal: false,
            distribution: Distribution::default(),
        };

        assert!((dim.mmc() - 10.1).abs() < 1e-10);  // largest
        assert!((dim.lmc() - 9.95).abs() < 1e-10); // smallest
        assert!((dim.tolerance_band() - 0.15).abs() < 1e-10);
    }

    #[test]
    fn test_dimension_calculations_internal() {
        // Internal feature (hole): MMC = smallest, LMC = largest
        let dim = Dimension {
            name: "diameter".to_string(),
            nominal: 10.0,
            plus_tol: 0.1,
            minus_tol: 0.05,
            units: "mm".to_string(),
            internal: true,
            distribution: Distribution::default(),
        };

        assert!((dim.mmc() - 9.95).abs() < 1e-10);  // smallest (MMC for hole)
        assert!((dim.lmc() - 10.1).abs() < 1e-10); // largest (LMC for hole)
        assert!((dim.tolerance_band() - 0.15).abs() < 1e-10);
    }

    #[test]
    fn test_add_dimension() {
        let mut feat = Feature::new("CMP-123", FeatureType::Internal, "Test Hole", "Author");
        feat.add_dimension("diameter", 10.0, 0.1, 0.05, true); // internal=true for hole

        assert_eq!(feat.dimensions.len(), 1);
        assert_eq!(feat.dimensions[0].name, "diameter");
        assert_eq!(feat.dimensions[0].nominal, 10.0);
        assert!(feat.dimensions[0].internal);
    }

    #[test]
    fn test_entity_trait_implementation() {
        let feat = Feature::new("CMP-123", FeatureType::External, "Test Shaft", "Author");
        assert!(feat.id().to_string().starts_with("FEAT-"));
        assert_eq!(feat.title(), "Test Shaft");
        assert_eq!(feat.author(), "Author");
        assert_eq!(feat.status(), "draft");
        assert_eq!(Feature::PREFIX, "FEAT");
    }

    #[test]
    fn test_feature_roundtrip() {
        let mut feat = Feature::new("CMP-123", FeatureType::Internal, "Mounting Hole", "Author");
        feat.description = Some("Primary mounting hole".to_string());
        feat.add_dimension("diameter", 10.0, 0.1, 0.05, true); // internal=true for hole
        feat.gdt.push(GdtControl {
            symbol: GdtSymbol::Position,
            value: 0.25,
            units: "mm".to_string(),
            datum_refs: vec!["A".to_string(), "B".to_string(), "C".to_string()],
            material_condition: MaterialCondition::Mmc,
        });
        feat.drawing = DrawingRef {
            number: "DWG-001".to_string(),
            revision: "A".to_string(),
            zone: "B3".to_string(),
        };
        feat.tags = vec!["mounting".to_string(), "primary".to_string()];

        let yaml = serde_yml::to_string(&feat).unwrap();
        let parsed: Feature = serde_yml::from_str(&yaml).unwrap();

        assert_eq!(parsed.component, "CMP-123");
        assert_eq!(parsed.feature_type, FeatureType::Internal);
        assert_eq!(parsed.dimensions.len(), 1);
        assert_eq!(parsed.gdt.len(), 1);
        assert_eq!(parsed.gdt[0].symbol, GdtSymbol::Position);
        assert_eq!(parsed.gdt[0].datum_refs.len(), 3);
        assert_eq!(parsed.drawing.number, "DWG-001");
    }

    #[test]
    fn test_feature_type_serialization() {
        let feat = Feature::new("CMP-123", FeatureType::External, "Mating Surface", "Author");
        let yaml = serde_yml::to_string(&feat).unwrap();
        assert!(yaml.contains("external"));

        let parsed: Feature = serde_yml::from_str(&yaml).unwrap();
        assert_eq!(parsed.feature_type, FeatureType::External);
    }

    #[test]
    fn test_tolerance_format() {
        // Verify that tolerances use plus_tol/minus_tol format (not +/- symbol)
        let mut feat = Feature::new("CMP-123", FeatureType::Internal, "Test Hole", "Author");
        feat.add_dimension("diameter", 10.0, 0.1, 0.05, true);

        let yaml = serde_yml::to_string(&feat).unwrap();
        assert!(yaml.contains("plus_tol"));
        assert!(yaml.contains("minus_tol"));
        assert!(yaml.contains("internal: true"));
        // Should NOT contain the +/- symbol that users can't type
        assert!(!yaml.contains("±"));
    }
}
