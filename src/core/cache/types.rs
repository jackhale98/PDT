//! Cache type definitions
//!
//! All structs used for cached entity data and query results.

use std::collections::HashMap;
use std::path::PathBuf;

use chrono::{DateTime, Utc};

use crate::core::identity::EntityPrefix;

// =========================================================================
// Link Types
// =========================================================================

/// Link types for entity relationships
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkType {
    TracesTo,      // Requirement traces to another requirement
    TracesFrom,    // Reverse of traces_to
    Verifies,      // Test verifies a requirement
    VerifiedBy,    // Reverse of verifies
    Mitigates,     // Control/CAPA mitigates a risk
    MitigatedBy,   // Reverse of mitigates
    References,    // Generic reference to another entity
    ReferencedBy,  // Reverse of references
    Contains,      // Assembly contains component
    ContainedIn,   // Reverse of contains
    QuotesFor,     // Quote is for a component
    QuotedBy,      // Reverse of quotes_for
}

impl LinkType {
    pub fn as_str(&self) -> &'static str {
        match self {
            LinkType::TracesTo => "traces_to",
            LinkType::TracesFrom => "traces_from",
            LinkType::Verifies => "verifies",
            LinkType::VerifiedBy => "verified_by",
            LinkType::Mitigates => "mitigates",
            LinkType::MitigatedBy => "mitigated_by",
            LinkType::References => "references",
            LinkType::ReferencedBy => "referenced_by",
            LinkType::Contains => "contains",
            LinkType::ContainedIn => "contained_in",
            LinkType::QuotesFor => "quotes_for",
            LinkType::QuotedBy => "quoted_by",
        }
    }

    pub fn reverse(&self) -> Self {
        match self {
            LinkType::TracesTo => LinkType::TracesFrom,
            LinkType::TracesFrom => LinkType::TracesTo,
            LinkType::Verifies => LinkType::VerifiedBy,
            LinkType::VerifiedBy => LinkType::Verifies,
            LinkType::Mitigates => LinkType::MitigatedBy,
            LinkType::MitigatedBy => LinkType::Mitigates,
            LinkType::References => LinkType::ReferencedBy,
            LinkType::ReferencedBy => LinkType::References,
            LinkType::Contains => LinkType::ContainedIn,
            LinkType::ContainedIn => LinkType::Contains,
            LinkType::QuotesFor => LinkType::QuotedBy,
            LinkType::QuotedBy => LinkType::QuotesFor,
        }
    }
}

/// A cached link between two entities
#[derive(Debug, Clone)]
pub struct CachedLink {
    pub source_id: String,
    pub target_id: String,
    pub link_type: String,
}

// =========================================================================
// Cached Entity Types
// =========================================================================

/// Cached entity metadata (fast access without YAML parsing)
#[derive(Debug, Clone)]
pub struct CachedEntity {
    pub id: String,
    pub prefix: String,
    pub title: String,
    pub status: String,
    pub author: String,
    pub created: DateTime<Utc>,
    pub file_path: PathBuf,
    // Extended fields
    pub priority: Option<String>,
    pub entity_type: Option<String>,
    pub category: Option<String>,
    pub tags: Vec<String>,
}

/// Cached feature with dimension data
#[derive(Debug, Clone)]
pub struct CachedFeature {
    pub id: String,
    pub title: String,
    pub status: String,
    pub component_id: String,
    pub feature_type: String,
    pub dim_name: Option<String>,
    pub dim_nominal: Option<f64>,
    pub dim_plus_tol: Option<f64>,
    pub dim_minus_tol: Option<f64>,
    pub dim_internal: Option<bool>,
    pub author: String,
    pub created: DateTime<Utc>,
    pub file_path: PathBuf,
}

/// Cached supplier data
#[derive(Debug, Clone)]
pub struct CachedSupplier {
    pub id: String,
    pub name: String,
    pub short_name: Option<String>,
    pub status: String,
    pub author: String,
    pub created: DateTime<Utc>,
    pub website: Option<String>,
    pub capabilities: Vec<String>,
    pub lead_time_days: Option<i32>,
    pub file_path: PathBuf,
}

/// Cached requirement data
#[derive(Debug, Clone)]
pub struct CachedRequirement {
    pub id: String,
    pub title: String,
    pub status: String,
    pub priority: Option<String>,
    pub req_type: Option<String>,
    pub category: Option<String>,
    pub author: String,
    pub created: DateTime<Utc>,
    pub tags: Vec<String>,
    pub file_path: PathBuf,
}

/// Cached risk data
#[derive(Debug, Clone)]
pub struct CachedRisk {
    pub id: String,
    pub title: String,
    pub status: String,
    pub risk_type: Option<String>,
    pub severity: Option<i32>,
    pub occurrence: Option<i32>,
    pub detection: Option<i32>,
    pub rpn: Option<i32>,
    pub risk_level: Option<String>,
    pub category: Option<String>,
    pub author: String,
    pub created: DateTime<Utc>,
    pub file_path: PathBuf,
}

/// Cached test protocol data
#[derive(Debug, Clone)]
pub struct CachedTest {
    pub id: String,
    pub title: String,
    pub status: String,
    pub test_type: Option<String>,
    pub level: Option<String>,
    pub method: Option<String>,
    pub priority: Option<String>,
    pub category: Option<String>,
    pub author: String,
    pub created: DateTime<Utc>,
    pub file_path: PathBuf,
}

/// Cached component data
#[derive(Debug, Clone)]
pub struct CachedComponent {
    pub id: String,
    pub title: String,
    pub status: String,
    pub part_number: Option<String>,
    pub revision: Option<String>,
    pub make_buy: Option<String>,
    pub category: Option<String>,
    pub author: String,
    pub created: DateTime<Utc>,
    pub file_path: PathBuf,
}

/// Cached quote data
#[derive(Debug, Clone)]
pub struct CachedQuote {
    pub id: String,
    pub title: String,
    pub status: String,
    pub quote_status: Option<String>,
    pub supplier_id: Option<String>,
    pub component_id: Option<String>,
    pub unit_price: Option<f64>,
    pub quantity: Option<i32>,
    pub lead_time_days: Option<i32>,
    pub currency: Option<String>,
    pub valid_until: Option<String>,
    pub author: String,
    pub created: DateTime<Utc>,
    pub file_path: PathBuf,
}

/// Cached NCR data
#[derive(Debug, Clone)]
pub struct CachedNcr {
    pub id: String,
    pub title: String,
    pub status: String,
    pub ncr_type: Option<String>,
    pub severity: Option<String>,
    pub ncr_status: Option<String>,
    pub category: Option<String>,
    pub author: String,
    pub created: DateTime<Utc>,
    pub file_path: PathBuf,
}

/// Cached CAPA data
#[derive(Debug, Clone)]
pub struct CachedCapa {
    pub id: String,
    pub title: String,
    pub status: String,
    pub capa_type: Option<String>,
    pub capa_status: Option<String>,
    pub author: String,
    pub created: DateTime<Utc>,
    pub file_path: PathBuf,
}

/// Cached process data
#[derive(Debug, Clone)]
pub struct CachedProcess {
    pub id: String,
    pub title: String,
    pub status: String,
    pub process_type: Option<String>,
    pub category: Option<String>,
    pub author: String,
    pub created: DateTime<Utc>,
    pub file_path: PathBuf,
}

/// Cached control data
#[derive(Debug, Clone)]
pub struct CachedControl {
    pub id: String,
    pub title: String,
    pub status: String,
    pub control_type: Option<String>,
    pub process_id: Option<String>,
    pub category: Option<String>,
    pub author: String,
    pub created: DateTime<Utc>,
    pub file_path: PathBuf,
}

/// Cached work instruction data
#[derive(Debug, Clone)]
pub struct CachedWork {
    pub id: String,
    pub title: String,
    pub status: String,
    pub process_id: Option<String>,
    pub author: String,
    pub created: DateTime<Utc>,
    pub file_path: PathBuf,
}

/// Cached test result data
#[derive(Debug, Clone)]
pub struct CachedResult {
    pub id: String,
    pub title: String,
    pub status: String,
    pub test_id: Option<String>,
    pub verdict: Option<String>,
    pub executed_by: Option<String>,
    pub executed_date: Option<String>,
    pub author: String,
    pub created: DateTime<Utc>,
    pub file_path: PathBuf,
}

// =========================================================================
// Aggregate Query Result Types
// =========================================================================

/// Count of entities grouped by a field
#[derive(Debug, Clone)]
pub struct GroupCount {
    pub group: String,
    pub count: usize,
}

/// Risk distribution summary
#[derive(Debug, Clone, Default)]
pub struct RiskDistribution {
    pub total: usize,
    pub by_level: Vec<GroupCount>,
    pub by_status: Vec<GroupCount>,
    pub average_rpn: Option<f64>,
    pub max_rpn: Option<i32>,
}

/// Requirement coverage summary
#[derive(Debug, Clone, Default)]
pub struct RequirementCoverage {
    pub total_requirements: usize,
    pub with_tests: usize,
    pub without_tests: usize,
    pub coverage_percent: f64,
}

/// Quote summary by supplier
#[derive(Debug, Clone)]
pub struct SupplierQuoteSummary {
    pub supplier_id: Option<String>,
    pub supplier_name: Option<String>,
    pub quote_count: usize,
    pub total_value: Option<f64>,
    pub avg_lead_time: Option<f64>,
}

/// Project-wide statistics
#[derive(Debug, Clone, Default)]
pub struct ProjectStats {
    pub entity_counts: Vec<GroupCount>,
    pub status_counts: Vec<GroupCount>,
}

// =========================================================================
// Operation Result Types
// =========================================================================

/// Statistics from sync operation
#[derive(Debug, Default)]
pub struct SyncStats {
    pub files_scanned: usize,
    pub entities_added: usize,
    pub entities_updated: usize,
    pub entities_removed: usize,
    pub duration_ms: u64,
}

/// Cache statistics
#[derive(Debug, Default)]
pub struct CacheStats {
    pub total_entities: usize,
    pub total_short_ids: usize,
    pub by_prefix: HashMap<String, usize>,
    pub db_size_bytes: u64,
}

/// Filter for listing entities
#[derive(Debug, Default)]
pub struct EntityFilter {
    pub prefix: Option<EntityPrefix>,
    pub status: Option<String>,
    pub author: Option<String>,
    pub search: Option<String>,
    pub limit: Option<usize>,
    pub priority: Option<String>,
    pub entity_type: Option<String>,
    pub category: Option<String>,
}
