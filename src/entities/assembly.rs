//! Assembly entity - Groups of components and sub-assemblies

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::core::entity::{Entity, Status};
use crate::core::identity::{EntityId, EntityPrefix};
use crate::entities::safety::{Asil, Dal, SwClass};

/// BOM line item - references a component with quantity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BomItem {
    /// Component ID (CMP-...)
    pub component_id: String,

    /// Quantity of this component in the assembly
    pub quantity: u32,

    /// Reference designators (e.g., ["R1", "R2", "R3"])
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reference_designators: Vec<String>,

    /// Assembly-specific notes (e.g., "Use thread locker")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

/// Document reference
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    /// Document type (drawing, spec, datasheet, etc.)
    #[serde(rename = "type")]
    pub doc_type: String,

    /// Path to document file
    pub path: String,

    /// Document revision
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub revision: String,
}

/// Assembly links
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AssemblyLinks {
    /// Related entities
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub related_to: Vec<String>,

    /// Parent assembly ID if this is a sub-assembly
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,

    /// Risks affecting this assembly (reciprocal of RISK.affects)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub risks: Vec<String>,
}

/// Assembly entity - collection of components and sub-assemblies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Assembly {
    /// Unique identifier (ASM-...)
    pub id: EntityId,

    /// Assembly part number
    pub part_number: String,

    /// Part revision
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,

    /// Assembly title/name
    pub title: String,

    /// Detailed description
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Bill of materials (component references with quantities)
    #[serde(default)]
    pub bom: Vec<BomItem>,

    /// Sub-assembly references (ASM-... IDs)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub subassemblies: Vec<String>,

    /// IEC 62304 Software Safety Class (optional, for software items)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sw_class: Option<SwClass>,

    /// ISO 26262 ASIL (optional, for automotive items)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asil: Option<Asil>,

    /// DO-178C DAL (optional, for aerospace items)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dal: Option<Dal>,

    /// Associated documents
    #[serde(default)]
    pub documents: Vec<Document>,

    /// Classification tags
    #[serde(default)]
    pub tags: Vec<String>,

    /// Current status
    #[serde(default)]
    pub status: Status,

    /// Links to other entities
    #[serde(default)]
    pub links: AssemblyLinks,

    /// Creation timestamp
    pub created: DateTime<Utc>,

    /// Author name
    pub author: String,

    /// Revision counter for entity updates
    #[serde(default = "default_revision")]
    pub entity_revision: u32,
}

fn default_revision() -> u32 {
    1
}

impl Entity for Assembly {
    const PREFIX: &'static str = "ASM";

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

impl Default for Assembly {
    fn default() -> Self {
        Self {
            id: EntityId::new(EntityPrefix::Asm),
            part_number: String::new(),
            revision: None,
            title: String::new(),
            description: None,
            bom: Vec::new(),
            subassemblies: Vec::new(),
            sw_class: None,
            asil: None,
            dal: None,
            documents: Vec::new(),
            tags: Vec::new(),
            status: Status::default(),
            links: AssemblyLinks::default(),
            created: Utc::now(),
            author: String::new(),
            entity_revision: 1,
        }
    }
}

impl Assembly {
    /// Create a new assembly with required fields
    pub fn new(
        part_number: impl Into<String>,
        title: impl Into<String>,
        author: impl Into<String>,
    ) -> Self {
        Self {
            id: EntityId::new(EntityPrefix::Asm),
            part_number: part_number.into(),
            title: title.into(),
            author: author.into(),
            created: Utc::now(),
            ..Default::default()
        }
    }

    /// Add a component to the BOM
    pub fn add_component(&mut self, component_id: String, quantity: u32) {
        self.bom.push(BomItem {
            component_id,
            quantity,
            reference_designators: Vec::new(),
            notes: None,
        });
    }

    /// Add a sub-assembly reference
    pub fn add_subassembly(&mut self, assembly_id: String) {
        self.subassemblies.push(assembly_id);
    }

    /// Get total count of distinct BOM items
    pub fn bom_item_count(&self) -> usize {
        self.bom.len()
    }

    /// Get total quantity of all components
    pub fn total_component_count(&self) -> u32 {
        self.bom.iter().map(|item| item.quantity).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_assembly_creation() {
        let asm = Assembly::new("ASM-001", "Main Assembly", "Test Author");
        assert_eq!(asm.part_number, "ASM-001");
        assert_eq!(asm.title, "Main Assembly");
        assert_eq!(asm.author, "Test Author");
        assert_eq!(asm.status, Status::Draft);
        assert!(asm.bom.is_empty());
    }

    #[test]
    fn test_add_component() {
        let mut asm = Assembly::new("ASM-001", "Test Assembly", "Author");
        asm.add_component("CMP-123".to_string(), 4);

        assert_eq!(asm.bom.len(), 1);
        assert_eq!(asm.bom[0].component_id, "CMP-123");
        assert_eq!(asm.bom[0].quantity, 4);
    }

    #[test]
    fn test_total_component_count() {
        let mut asm = Assembly::new("ASM-001", "Test Assembly", "Author");
        asm.add_component("CMP-001".to_string(), 4);
        asm.add_component("CMP-002".to_string(), 2);

        assert_eq!(asm.bom_item_count(), 2);
        assert_eq!(asm.total_component_count(), 6);
    }

    #[test]
    fn test_entity_trait_implementation() {
        let asm = Assembly::new("ASM-001", "Test Assembly", "Author");
        assert!(asm.id().to_string().starts_with("ASM-"));
        assert_eq!(asm.title(), "Test Assembly");
        assert_eq!(asm.author(), "Author");
        assert_eq!(asm.status(), "draft");
        assert_eq!(Assembly::PREFIX, "ASM");
    }

    #[test]
    fn test_assembly_roundtrip() {
        let mut asm = Assembly::new("ASM-001", "Main Assembly", "Author");
        asm.revision = Some("A".to_string());
        asm.description = Some("Main product assembly".to_string());
        asm.add_component("CMP-001".to_string(), 4);
        asm.bom[0].reference_designators = vec!["R1".to_string(), "R2".to_string()];
        asm.bom[0].notes = Some("Use thread locker".to_string());
        asm.add_subassembly("ASM-SUB-001".to_string());
        asm.tags = vec!["main".to_string(), "production".to_string()];

        let yaml = serde_yml::to_string(&asm).unwrap();
        let parsed: Assembly = serde_yml::from_str(&yaml).unwrap();

        assert_eq!(parsed.part_number, "ASM-001");
        assert_eq!(parsed.revision.as_deref(), Some("A"));
        assert_eq!(parsed.title, "Main Assembly");
        assert_eq!(parsed.bom.len(), 1);
        assert_eq!(parsed.bom[0].quantity, 4);
        assert_eq!(parsed.bom[0].reference_designators.len(), 2);
        assert_eq!(parsed.subassemblies.len(), 1);
    }

    #[test]
    fn test_bom_item_with_full_details() {
        let item = BomItem {
            component_id: "CMP-001".to_string(),
            quantity: 4,
            reference_designators: vec![
                "U1".to_string(),
                "U2".to_string(),
                "U3".to_string(),
                "U4".to_string(),
            ],
            notes: Some("IC chips - handle with ESD precautions".to_string()),
        };

        let yaml = serde_yml::to_string(&item).unwrap();
        assert!(yaml.contains("reference_designators"));
        assert!(yaml.contains("notes"));
    }
}
