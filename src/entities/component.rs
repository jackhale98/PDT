//! Component entity type - Individual parts (purchased or manufactured)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::core::entity::{Entity, Status};
use crate::core::identity::EntityId;
use crate::entities::safety::{Asil, Dal, SwClass};

/// Make or buy decision
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum MakeBuy {
    Make,
    #[default]
    Buy,
}

impl std::fmt::Display for MakeBuy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MakeBuy::Make => write!(f, "make"),
            MakeBuy::Buy => write!(f, "buy"),
        }
    }
}

impl std::str::FromStr for MakeBuy {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "make" => Ok(MakeBuy::Make),
            "buy" => Ok(MakeBuy::Buy),
            _ => Err(format!(
                "Invalid make_buy value: {}. Use 'make' or 'buy'",
                s
            )),
        }
    }
}

/// Component category
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum ComponentCategory {
    #[default]
    Mechanical,
    Electrical,
    Software,
    Fastener,
    Consumable,
}

impl std::fmt::Display for ComponentCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ComponentCategory::Mechanical => write!(f, "mechanical"),
            ComponentCategory::Electrical => write!(f, "electrical"),
            ComponentCategory::Software => write!(f, "software"),
            ComponentCategory::Fastener => write!(f, "fastener"),
            ComponentCategory::Consumable => write!(f, "consumable"),
        }
    }
}

impl std::str::FromStr for ComponentCategory {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "mechanical" => Ok(ComponentCategory::Mechanical),
            "electrical" => Ok(ComponentCategory::Electrical),
            "software" => Ok(ComponentCategory::Software),
            "fastener" => Ok(ComponentCategory::Fastener),
            "consumable" => Ok(ComponentCategory::Consumable),
            _ => Err(format!(
                "Invalid category: {}. Use mechanical, electrical, software, fastener, or consumable",
                s
            )),
        }
    }
}

/// Supplier information for a component
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ComponentSupplier {
    /// Supplier entity ID (SUP-...) - links to SUP entity
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supplier_id: Option<String>,

    /// Supplier name (fallback if supplier_id not set)
    #[serde(default)]
    pub name: String,

    /// Supplier's part number
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supplier_pn: Option<String>,

    /// Lead time in days
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lead_time_days: Option<u32>,

    /// Minimum order quantity
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub moq: Option<u32>,

    /// Unit cost from this supplier
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit_cost: Option<f64>,
}

/// Document reference
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Document {
    /// Document type (e.g., "drawing", "specification", "datasheet")
    #[serde(rename = "type")]
    pub doc_type: String,

    /// Path to the document
    pub path: String,

    /// Document revision
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,
}

// ===== 3D SDT Tolerance Analysis Types =====

/// Component coordinate system for 3D tolerance analysis
///
/// Defines the local coordinate system for this component in the assembly.
/// The Y-axis is computed as Z × X.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoordinateSystem {
    /// Origin point [x, y, z] in assembly coordinates
    pub origin: [f64; 3],

    /// X-axis direction [dx, dy, dz] - must be unit vector
    pub x_axis: [f64; 3],

    /// Z-axis direction [dx, dy, dz] - must be unit vector, perpendicular to X
    pub z_axis: [f64; 3],
}

impl Default for CoordinateSystem {
    fn default() -> Self {
        Self {
            origin: [0.0, 0.0, 0.0],
            x_axis: [1.0, 0.0, 0.0],
            z_axis: [0.0, 0.0, 1.0],
        }
    }
}

/// Datum reference frame (DRF) for GD&T
///
/// References the feature entities that define the component's datum frame.
/// Auto-populated by scanning features with datum_label set.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DatumFrame {
    /// Primary datum feature (A)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub a: Option<EntityId>,

    /// Secondary datum feature (B)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub b: Option<EntityId>,

    /// Tertiary datum feature (C)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub c: Option<EntityId>,
}

/// Links to other entities
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ComponentLinks {
    /// Related entities (requirements, etc.)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub related_to: Vec<EntityId>,

    /// Assemblies that use this component
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub used_in: Vec<EntityId>,

    /// Components this replaces (supersedes)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub replaces: Vec<EntityId>,

    /// Components that replace this one (reciprocal of replaces)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub replaced_by: Vec<EntityId>,

    /// Interchangeable components (alternates)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub interchangeable_with: Vec<EntityId>,

    /// Risks affecting this component (reciprocal of RISK.affects)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub risks: Vec<EntityId>,
}

/// A Component entity - individual part (purchased or manufactured)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Component {
    /// Unique identifier
    pub id: EntityId,

    /// Part number
    pub part_number: String,

    /// Part revision
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,

    /// Short title/description
    pub title: String,

    /// Detailed description
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Make or buy decision
    #[serde(default)]
    pub make_buy: MakeBuy,

    /// Component category
    #[serde(default)]
    pub category: ComponentCategory,

    /// IEC 62304 Software Safety Class (optional, for software items)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sw_class: Option<SwClass>,

    /// ISO 26262 ASIL (optional, for automotive items)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asil: Option<Asil>,

    /// DO-178C DAL (optional, for aerospace items)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dal: Option<Dal>,

    /// Material specification
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material: Option<String>,

    /// Mass in kilograms
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mass_kg: Option<f64>,

    /// Unit cost (manual override - prefer using selected_quote for pricing)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit_cost: Option<f64>,

    /// Selected quote ID for pricing (QUOT-...)
    /// When set, BOM costing will use this quote's price breaks instead of unit_cost
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_quote: Option<String>,

    /// Supplier information
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub suppliers: Vec<ComponentSupplier>,

    /// Associated documents
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub documents: Vec<Document>,

    // ===== 3D SDT Analysis Fields =====
    /// Component coordinate system for 3D tolerance analysis
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coordinate_system: Option<CoordinateSystem>,

    /// Datum reference frame - auto-populated from features with datum_label
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub datum_frame: Option<DatumFrame>,

    /// Tags for filtering
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,

    /// Current status
    #[serde(default)]
    pub status: Status,

    /// Links to other entities
    #[serde(default)]
    pub links: ComponentLinks,

    /// Creation timestamp
    pub created: DateTime<Utc>,

    /// Author (who created this component)
    pub author: String,

    /// Entity revision number
    #[serde(default = "default_revision")]
    pub entity_revision: u32,
}

fn default_revision() -> u32 {
    1
}

impl Entity for Component {
    const PREFIX: &'static str = "CMP";

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

impl Component {
    /// Create a new component with the given parameters
    pub fn new(
        part_number: String,
        title: String,
        make_buy: MakeBuy,
        category: ComponentCategory,
        author: String,
    ) -> Self {
        Self {
            id: EntityId::new(crate::core::EntityPrefix::Cmp),
            part_number,
            revision: None,
            title,
            description: None,
            make_buy,
            category,
            sw_class: None,
            asil: None,
            dal: None,
            material: None,
            mass_kg: None,
            unit_cost: None,
            selected_quote: None,
            suppliers: Vec::new(),
            documents: Vec::new(),
            coordinate_system: None,
            datum_frame: None,
            tags: Vec::new(),
            status: Status::default(),
            links: ComponentLinks::default(),
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
    fn test_component_creation() {
        let cmp = Component::new(
            "PN-001".to_string(),
            "Test Widget".to_string(),
            MakeBuy::Buy,
            ComponentCategory::Mechanical,
            "test".to_string(),
        );

        assert!(cmp.id.to_string().starts_with("CMP-"));
        assert_eq!(cmp.part_number, "PN-001");
        assert_eq!(cmp.title, "Test Widget");
        assert_eq!(cmp.make_buy, MakeBuy::Buy);
        assert_eq!(cmp.category, ComponentCategory::Mechanical);
    }

    #[test]
    fn test_component_roundtrip() {
        let cmp = Component::new(
            "PN-002".to_string(),
            "Another Widget".to_string(),
            MakeBuy::Make,
            ComponentCategory::Electrical,
            "test".to_string(),
        );

        let yaml = serde_yml::to_string(&cmp).unwrap();
        let parsed: Component = serde_yml::from_str(&yaml).unwrap();

        assert_eq!(cmp.id, parsed.id);
        assert_eq!(cmp.part_number, parsed.part_number);
        assert_eq!(cmp.title, parsed.title);
        assert_eq!(cmp.make_buy, parsed.make_buy);
        assert_eq!(cmp.category, parsed.category);
    }

    #[test]
    fn test_make_buy_serialization() {
        let cmp = Component::new(
            "PN-003".to_string(),
            "Make Part".to_string(),
            MakeBuy::Make,
            ComponentCategory::Mechanical,
            "test".to_string(),
        );

        let yaml = serde_yml::to_string(&cmp).unwrap();
        assert!(yaml.contains("make_buy: make"));
    }

    #[test]
    fn test_category_serialization() {
        let cmp = Component::new(
            "PN-004".to_string(),
            "Fastener".to_string(),
            MakeBuy::Buy,
            ComponentCategory::Fastener,
            "test".to_string(),
        );

        let yaml = serde_yml::to_string(&cmp).unwrap();
        assert!(yaml.contains("category: fastener"));
    }

    #[test]
    fn test_entity_trait_implementation() {
        let cmp = Component::new(
            "PN-005".to_string(),
            "Entity Test".to_string(),
            MakeBuy::Buy,
            ComponentCategory::Mechanical,
            "test_author".to_string(),
        );

        assert_eq!(Component::PREFIX, "CMP");
        assert_eq!(cmp.title(), "Entity Test");
        assert_eq!(cmp.status(), "draft");
        assert_eq!(cmp.author(), "test_author");
    }
}
