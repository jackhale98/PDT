//! Quote entity type - Supplier quotations for components and assemblies

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

use crate::core::entity::{Entity, Status};
use crate::core::identity::{EntityId, EntityPrefix};

/// Quote status specific to quotations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum QuoteStatus {
    /// Quote requested but not yet received
    #[default]
    Pending,
    /// Quote received and under review
    Received,
    /// Quote accepted for use
    Accepted,
    /// Quote rejected (too expensive, lead time, etc.)
    Rejected,
    /// Quote has expired
    Expired,
}

impl std::fmt::Display for QuoteStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QuoteStatus::Pending => write!(f, "pending"),
            QuoteStatus::Received => write!(f, "received"),
            QuoteStatus::Accepted => write!(f, "accepted"),
            QuoteStatus::Rejected => write!(f, "rejected"),
            QuoteStatus::Expired => write!(f, "expired"),
        }
    }
}

impl std::str::FromStr for QuoteStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pending" => Ok(QuoteStatus::Pending),
            "received" => Ok(QuoteStatus::Received),
            "accepted" => Ok(QuoteStatus::Accepted),
            "rejected" => Ok(QuoteStatus::Rejected),
            "expired" => Ok(QuoteStatus::Expired),
            _ => Err(format!(
                "Invalid quote status: {}. Use pending, received, accepted, rejected, or expired",
                s
            )),
        }
    }
}

/// Currency code
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
#[derive(Default)]
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

/// Price break for quantity-based pricing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceBreak {
    /// Minimum quantity for this price
    pub min_qty: u32,

    /// Unit price at this quantity
    pub unit_price: f64,

    /// Lead time in days at this quantity
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lead_time_days: Option<u32>,
}

/// Non-recurring engineering (NRE) cost item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NreCost {
    /// Description of the NRE item
    pub description: String,

    /// Cost amount
    pub cost: f64,

    /// Is this a one-time cost or amortized?
    #[serde(default)]
    pub one_time: bool,
}

/// Links to other entities
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QuoteLinks {
    /// Related quotes (e.g., competing quotes)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub related_quotes: Vec<String>,
}

/// A Quote entity - supplier quotation for a component or assembly
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quote {
    /// Unique identifier
    pub id: EntityId,

    /// Quote title/description
    pub title: String,

    /// Supplier ID (SUP-...)
    pub supplier: String,

    /// Component ID this quote is for (mutually exclusive with assembly)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub component: Option<String>,

    /// Assembly ID this quote is for (mutually exclusive with component)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assembly: Option<String>,

    /// Supplier's quote reference number
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quote_ref: Option<String>,

    /// Detailed description or notes
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Currency for all prices
    #[serde(default)]
    pub currency: Currency,

    /// Price breaks (quantity-based pricing)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub price_breaks: Vec<PriceBreak>,

    /// Minimum order quantity
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub moq: Option<u32>,

    /// Tooling cost (one-time)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tooling_cost: Option<f64>,

    /// Non-recurring engineering costs
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub nre_costs: Vec<NreCost>,

    /// Standard lead time in days
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lead_time_days: Option<u32>,

    /// Date quote was received
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quote_date: Option<NaiveDate>,

    /// Date quote expires
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_until: Option<NaiveDate>,

    /// Quote-specific status
    #[serde(default)]
    pub quote_status: QuoteStatus,

    /// Tags for filtering
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,

    /// Entity status (draft, approved, etc.)
    #[serde(default)]
    pub status: Status,

    /// Links to other entities
    #[serde(default)]
    pub links: QuoteLinks,

    /// Creation timestamp
    pub created: DateTime<Utc>,

    /// Author (who created this quote)
    pub author: String,

    /// Entity revision number
    #[serde(default = "default_revision")]
    pub entity_revision: u32,
}

fn default_revision() -> u32 {
    1
}

impl Entity for Quote {
    const PREFIX: &'static str = "QUOT";

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

impl Quote {
    /// Create a new quote for a component
    pub fn new_for_component(
        supplier_id: impl Into<String>,
        component: impl Into<String>,
        title: impl Into<String>,
        author: impl Into<String>,
    ) -> Self {
        Self {
            id: EntityId::new(EntityPrefix::Quot),
            title: title.into(),
            supplier: supplier_id.into(),
            component: Some(component.into()),
            assembly: None,
            quote_ref: None,
            description: None,
            currency: Currency::default(),
            price_breaks: Vec::new(),
            moq: None,
            tooling_cost: None,
            nre_costs: Vec::new(),
            lead_time_days: None,
            quote_date: None,
            valid_until: None,
            quote_status: QuoteStatus::default(),
            tags: Vec::new(),
            status: Status::default(),
            links: QuoteLinks::default(),
            created: Utc::now(),
            author: author.into(),
            entity_revision: 1,
        }
    }

    /// Create a new quote for an assembly
    pub fn new_for_assembly(
        supplier_id: impl Into<String>,
        assembly: impl Into<String>,
        title: impl Into<String>,
        author: impl Into<String>,
    ) -> Self {
        Self {
            id: EntityId::new(EntityPrefix::Quot),
            title: title.into(),
            supplier: supplier_id.into(),
            component: None,
            assembly: Some(assembly.into()),
            quote_ref: None,
            description: None,
            currency: Currency::default(),
            price_breaks: Vec::new(),
            moq: None,
            tooling_cost: None,
            nre_costs: Vec::new(),
            lead_time_days: None,
            quote_date: None,
            valid_until: None,
            quote_status: QuoteStatus::default(),
            tags: Vec::new(),
            status: Status::default(),
            links: QuoteLinks::default(),
            created: Utc::now(),
            author: author.into(),
            entity_revision: 1,
        }
    }

    /// Get the linked item ID (component or assembly)
    pub fn linked_item(&self) -> Option<&str> {
        self.component.as_deref().or(self.assembly.as_deref())
    }

    /// Check if quote is for a component
    pub fn is_for_component(&self) -> bool {
        self.component.is_some()
    }

    /// Check if quote is for an assembly
    pub fn is_for_assembly(&self) -> bool {
        self.assembly.is_some()
    }

    /// Add a price break
    pub fn add_price_break(&mut self, min_qty: u32, unit_price: f64, lead_time_days: Option<u32>) {
        self.price_breaks.push(PriceBreak {
            min_qty,
            unit_price,
            lead_time_days,
        });
    }

    /// Get unit price for a given quantity
    pub fn price_for_qty(&self, qty: u32) -> Option<f64> {
        // Find the highest min_qty that is <= qty
        self.price_breaks
            .iter()
            .filter(|pb| pb.min_qty <= qty)
            .max_by_key(|pb| pb.min_qty)
            .map(|pb| pb.unit_price)
    }

    /// Get lead time for a given quantity
    /// Returns the lead time from the applicable price break, or the default lead_time_days
    pub fn lead_time_for_qty(&self, qty: u32) -> Option<u32> {
        // Find the highest min_qty that is <= qty
        self.price_breaks
            .iter()
            .filter(|pb| pb.min_qty <= qty)
            .max_by_key(|pb| pb.min_qty)
            .and_then(|pb| pb.lead_time_days)
            .or(self.lead_time_days)
    }

    /// Calculate total NRE cost
    pub fn total_nre(&self) -> f64 {
        let nre_sum: f64 = self.nre_costs.iter().map(|n| n.cost).sum();
        nre_sum + self.tooling_cost.unwrap_or(0.0)
    }

    /// Check if quote is expired
    pub fn is_expired(&self) -> bool {
        if let Some(valid_until) = self.valid_until {
            let today = Utc::now().date_naive();
            valid_until < today
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quote_creation_for_component() {
        let quote = Quote::new_for_component("SUP-123", "CMP-456", "Bracket Quote", "test");

        assert!(quote.id.to_string().starts_with("QUOT-"));
        assert_eq!(quote.supplier, "SUP-123");
        assert_eq!(quote.component, Some("CMP-456".to_string()));
        assert_eq!(quote.assembly, None);
        assert_eq!(quote.title, "Bracket Quote");
        assert_eq!(quote.quote_status, QuoteStatus::Pending);
        assert!(quote.is_for_component());
        assert!(!quote.is_for_assembly());
    }

    #[test]
    fn test_quote_creation_for_assembly() {
        let quote = Quote::new_for_assembly("SUP-123", "ASM-456", "Assembly Quote", "test");

        assert!(quote.id.to_string().starts_with("QUOT-"));
        assert_eq!(quote.supplier, "SUP-123");
        assert_eq!(quote.component, None);
        assert_eq!(quote.assembly, Some("ASM-456".to_string()));
        assert!(!quote.is_for_component());
        assert!(quote.is_for_assembly());
    }

    #[test]
    fn test_linked_item() {
        let cmp_quote = Quote::new_for_component("SUP-123", "CMP-456", "Quote", "test");
        assert_eq!(cmp_quote.linked_item(), Some("CMP-456"));

        let asm_quote = Quote::new_for_assembly("SUP-123", "ASM-789", "Quote", "test");
        assert_eq!(asm_quote.linked_item(), Some("ASM-789"));
    }

    #[test]
    fn test_price_breaks() {
        let mut quote = Quote::new_for_component("SUP-123", "CMP-456", "Test Quote", "test");
        quote.add_price_break(1, 10.00, None);
        quote.add_price_break(100, 8.00, Some(14));
        quote.add_price_break(1000, 6.00, Some(21));

        assert_eq!(quote.price_for_qty(1), Some(10.00));
        assert_eq!(quote.price_for_qty(50), Some(10.00));
        assert_eq!(quote.price_for_qty(100), Some(8.00));
        assert_eq!(quote.price_for_qty(500), Some(8.00));
        assert_eq!(quote.price_for_qty(1000), Some(6.00));
        assert_eq!(quote.price_for_qty(5000), Some(6.00));
    }

    #[test]
    fn test_total_nre() {
        let mut quote = Quote::new_for_component("SUP-123", "CMP-456", "Test Quote", "test");
        quote.tooling_cost = Some(5000.0);
        quote.nre_costs.push(NreCost {
            description: "Design fee".to_string(),
            cost: 2000.0,
            one_time: true,
        });
        quote.nre_costs.push(NreCost {
            description: "Setup fee".to_string(),
            cost: 500.0,
            one_time: true,
        });

        assert_eq!(quote.total_nre(), 7500.0);
    }

    #[test]
    fn test_quote_roundtrip() {
        let mut quote = Quote::new_for_component("SUP-123", "CMP-456", "Test Quote", "test");
        quote.add_price_break(1, 10.00, Some(7));
        quote.currency = Currency::Eur;
        quote.moq = Some(100);

        let yaml = serde_yml::to_string(&quote).unwrap();
        let parsed: Quote = serde_yml::from_str(&yaml).unwrap();

        assert_eq!(quote.id, parsed.id);
        assert_eq!(quote.supplier, parsed.supplier);
        assert_eq!(quote.component, parsed.component);
        assert_eq!(quote.currency, parsed.currency);
        assert_eq!(parsed.price_breaks.len(), 1);
    }

    #[test]
    fn test_quote_status_serialization() {
        let mut quote = Quote::new_for_component("SUP-123", "CMP-456", "Test Quote", "test");
        quote.quote_status = QuoteStatus::Accepted;

        let yaml = serde_yml::to_string(&quote).unwrap();
        assert!(yaml.contains("quote_status: accepted"));
    }

    #[test]
    fn test_entity_trait_implementation() {
        let quote = Quote::new_for_component("SUP-123", "CMP-456", "Entity Test", "test_author");

        assert_eq!(Quote::PREFIX, "QUOT");
        assert_eq!(quote.title(), "Entity Test");
        assert_eq!(quote.status(), "draft");
        assert_eq!(quote.author(), "test_author");
    }
}
