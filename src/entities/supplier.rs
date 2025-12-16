//! Supplier entity type - Approved suppliers for components and assemblies

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

use crate::core::entity::{Entity, Status};
use crate::core::identity::{EntityId, EntityPrefix};

/// Contact information for a person at the supplier
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Contact {
    /// Contact person name
    pub name: String,

    /// Role/title (e.g., "Sales", "Engineering", "Quality")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,

    /// Email address
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,

    /// Phone number
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phone: Option<String>,

    /// Is this the primary contact?
    #[serde(default)]
    pub primary: bool,
}

/// Address type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AddressType {
    #[default]
    Headquarters,
    Manufacturing,
    Shipping,
    Billing,
}

/// Physical address
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Address {
    /// Address type
    #[serde(default, rename = "type")]
    pub address_type: AddressType,

    /// Street address
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub street: Option<String>,

    /// City
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub city: Option<String>,

    /// State/Province
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,

    /// Postal/ZIP code
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub postal: Option<String>,

    /// Country
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
}

/// Quality certification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Certification {
    /// Certification name (e.g., "ISO 9001:2015", "AS9100D")
    pub name: String,

    /// Expiration date
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expiry: Option<NaiveDate>,

    /// Certificate number
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub certificate_number: Option<String>,
}

/// Supplier capability category
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    Machining,
    SheetMetal,
    Casting,
    Injection,
    Extrusion,
    Pcb,
    PcbAssembly,
    CableAssembly,
    Assembly,
    Testing,
    Finishing,
    Packaging,
}

impl std::fmt::Display for Capability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Capability::Machining => write!(f, "machining"),
            Capability::SheetMetal => write!(f, "sheet_metal"),
            Capability::Casting => write!(f, "casting"),
            Capability::Injection => write!(f, "injection"),
            Capability::Extrusion => write!(f, "extrusion"),
            Capability::Pcb => write!(f, "pcb"),
            Capability::PcbAssembly => write!(f, "pcb_assembly"),
            Capability::CableAssembly => write!(f, "cable_assembly"),
            Capability::Assembly => write!(f, "assembly"),
            Capability::Testing => write!(f, "testing"),
            Capability::Finishing => write!(f, "finishing"),
            Capability::Packaging => write!(f, "packaging"),
        }
    }
}

/// Currency code
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "UPPERCASE")]
pub enum Currency {
    #[default]
    Usd,
    Eur,
    Gbp,
    Cny,
    Jpy,
}

impl std::fmt::Display for Currency {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Currency::Usd => write!(f, "USD"),
            Currency::Eur => write!(f, "EUR"),
            Currency::Gbp => write!(f, "GBP"),
            Currency::Cny => write!(f, "CNY"),
            Currency::Jpy => write!(f, "JPY"),
        }
    }
}

/// Links to other entities
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SupplierLinks {
    /// Components this supplier is approved for
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub approved_for: Vec<String>,
}

/// A Supplier entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Supplier {
    /// Unique identifier
    pub id: EntityId,

    /// Company name
    pub name: String,

    /// Short name for display
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub short_name: Option<String>,

    /// Website URL
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub website: Option<String>,

    /// Contact people
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub contacts: Vec<Contact>,

    /// Physical addresses
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub addresses: Vec<Address>,

    /// Default payment terms
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payment_terms: Option<String>,

    /// Preferred currency
    #[serde(default)]
    pub currency: Currency,

    /// Quality certifications
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub certifications: Vec<Certification>,

    /// Manufacturing capabilities
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capabilities: Vec<Capability>,

    /// Notes about the supplier
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,

    /// Tags for filtering
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,

    /// Entity status (draft, approved, etc.)
    #[serde(default)]
    pub status: Status,

    /// Links to other entities
    #[serde(default)]
    pub links: SupplierLinks,

    /// Creation timestamp
    pub created: DateTime<Utc>,

    /// Author (who created this supplier)
    pub author: String,

    /// Entity revision number
    #[serde(default = "default_revision")]
    pub entity_revision: u32,
}

fn default_revision() -> u32 {
    1
}

impl Entity for Supplier {
    const PREFIX: &'static str = "SUP";

    fn id(&self) -> &EntityId {
        &self.id
    }

    fn title(&self) -> &str {
        &self.name
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

impl Supplier {
    /// Create a new supplier
    pub fn new(name: impl Into<String>, author: impl Into<String>) -> Self {
        Self {
            id: EntityId::new(EntityPrefix::Sup),
            name: name.into(),
            short_name: None,
            website: None,
            contacts: Vec::new(),
            addresses: Vec::new(),
            payment_terms: None,
            currency: Currency::default(),
            certifications: Vec::new(),
            capabilities: Vec::new(),
            notes: None,
            tags: Vec::new(),
            status: Status::default(),
            links: SupplierLinks::default(),
            created: Utc::now(),
            author: author.into(),
            entity_revision: 1,
        }
    }

    /// Add a contact
    pub fn add_contact(&mut self, contact: Contact) {
        self.contacts.push(contact);
    }

    /// Add an address
    pub fn add_address(&mut self, address: Address) {
        self.addresses.push(address);
    }

    /// Add a certification
    pub fn add_certification(&mut self, cert: Certification) {
        self.certifications.push(cert);
    }

    /// Get the primary contact, if any
    pub fn primary_contact(&self) -> Option<&Contact> {
        self.contacts
            .iter()
            .find(|c| c.primary)
            .or(self.contacts.first())
    }

    /// Get display name (short_name if available, otherwise name)
    pub fn display_name(&self) -> &str {
        self.short_name.as_deref().unwrap_or(&self.name)
    }

    /// Check if any certifications are expired
    pub fn has_expired_certs(&self) -> bool {
        let today = Utc::now().date_naive();
        self.certifications
            .iter()
            .any(|c| c.expiry.is_some_and(|exp| exp < today))
    }

    /// Get certifications expiring within N days
    pub fn certs_expiring_soon(&self, days: i64) -> Vec<&Certification> {
        let today = Utc::now().date_naive();
        let threshold = today + chrono::Duration::days(days);
        self.certifications
            .iter()
            .filter(|c| {
                c.expiry
                    .is_some_and(|exp| exp <= threshold && exp >= today)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_supplier_creation() {
        let supplier = Supplier::new("Acme Corp", "test");

        assert!(supplier.id.to_string().starts_with("SUP-"));
        assert_eq!(supplier.name, "Acme Corp");
        assert_eq!(supplier.status, Status::Draft);
    }

    #[test]
    fn test_supplier_display_name() {
        let mut supplier = Supplier::new("Acme Corporation Inc.", "test");
        assert_eq!(supplier.display_name(), "Acme Corporation Inc.");

        supplier.short_name = Some("Acme".to_string());
        assert_eq!(supplier.display_name(), "Acme");
    }

    #[test]
    fn test_supplier_primary_contact() {
        let mut supplier = Supplier::new("Acme Corp", "test");

        supplier.add_contact(Contact {
            name: "John Smith".to_string(),
            role: Some("Sales".to_string()),
            primary: false,
            ..Default::default()
        });
        supplier.add_contact(Contact {
            name: "Jane Doe".to_string(),
            role: Some("Engineering".to_string()),
            primary: true,
            ..Default::default()
        });

        let primary = supplier.primary_contact().unwrap();
        assert_eq!(primary.name, "Jane Doe");
    }

    #[test]
    fn test_supplier_roundtrip() {
        let mut supplier = Supplier::new("Acme Corp", "test");
        supplier.short_name = Some("Acme".to_string());
        supplier.payment_terms = Some("Net 30".to_string());
        supplier.capabilities.push(Capability::Machining);
        supplier.capabilities.push(Capability::SheetMetal);

        let yaml = serde_yml::to_string(&supplier).unwrap();
        let parsed: Supplier = serde_yml::from_str(&yaml).unwrap();

        assert_eq!(supplier.id, parsed.id);
        assert_eq!(supplier.name, parsed.name);
        assert_eq!(supplier.short_name, parsed.short_name);
        assert_eq!(parsed.capabilities.len(), 2);
    }

    #[test]
    fn test_entity_trait_implementation() {
        let supplier = Supplier::new("Acme Corp", "test_author");

        assert_eq!(Supplier::PREFIX, "SUP");
        assert_eq!(supplier.title(), "Acme Corp");
        assert_eq!(supplier.status(), "draft");
        assert_eq!(supplier.author(), "test_author");
    }
}
