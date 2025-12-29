//! Mate entity - 1:1 contact between two features with fit calculation
//!
//! A mate represents direct contact between two features, such as a pin in a hole.
//! The fit analysis is automatically calculated based on the feature dimensions.

use chrono::{DateTime, Utc};
use miette::{miette, Result};
use serde::{Deserialize, Serialize};

use crate::core::entity::{Entity, Status};
use crate::core::identity::{EntityId, EntityPrefix};
use crate::entities::feature::Dimension;

/// Mate type classification - the intended fit between mating features
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum MateType {
    /// Clearance fit - guaranteed gap between parts
    #[default]
    Clearance,
    /// Transition fit - may be clearance or interference
    Transition,
    /// Interference fit - press fit, guaranteed overlap
    Interference,
}

impl std::fmt::Display for MateType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MateType::Clearance => write!(f, "clearance"),
            MateType::Transition => write!(f, "transition"),
            MateType::Interference => write!(f, "interference"),
        }
    }
}

impl std::str::FromStr for MateType {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "clearance" => Ok(MateType::Clearance),
            "transition" => Ok(MateType::Transition),
            "interference" => Ok(MateType::Interference),
            _ => Err(format!(
                "Invalid mate type: '{}'. Use 'clearance', 'transition', or 'interference'",
                s
            )),
        }
    }
}

/// Fit result classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FitResult {
    /// Guaranteed clearance (min_clearance > 0)
    Clearance,
    /// Guaranteed interference (max_clearance < 0)
    Interference,
    /// May be either (overlapping ranges)
    Transition,
}

impl std::fmt::Display for FitResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FitResult::Clearance => write!(f, "clearance"),
            FitResult::Interference => write!(f, "interference"),
            FitResult::Transition => write!(f, "transition"),
        }
    }
}

/// Automatically calculated fit analysis
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FitAnalysis {
    /// Minimum clearance at worst-case (hole_min - shaft_max)
    /// Negative means interference
    pub worst_case_min_clearance: f64,

    /// Maximum clearance at worst-case (hole_max - shaft_min)
    pub worst_case_max_clearance: f64,

    /// Resulting fit classification
    pub fit_result: FitResult,

    /// Statistical (RSS) fit analysis - optional, calculated on demand
    /// Uses normal distribution assumptions to estimate interference probability
    /// Future: Will support 3D torsor-based analysis for full GD&T
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub statistical: Option<StatisticalFit>,
}

impl FitAnalysis {
    /// Calculate fit from hole and shaft dimensions (legacy tuple interface)
    /// hole_dim: (nominal, plus_tol, minus_tol)
    /// shaft_dim: (nominal, plus_tol, minus_tol)
    pub fn calculate(hole_dim: (f64, f64, f64), shaft_dim: (f64, f64, f64)) -> Self {
        let (hole_nom, hole_plus, hole_minus) = hole_dim;
        let (shaft_nom, shaft_plus, shaft_minus) = shaft_dim;

        // Hole limits
        let hole_max = hole_nom + hole_plus;
        let hole_min = hole_nom - hole_minus;

        // Shaft limits
        let shaft_max = shaft_nom + shaft_plus;
        let shaft_min = shaft_nom - shaft_minus;

        // Clearance calculations (positive = clearance, negative = interference)
        let min_clearance = hole_min - shaft_max;
        let max_clearance = hole_max - shaft_min;

        // Determine fit result
        let fit_result = if min_clearance > 0.0 {
            FitResult::Clearance
        } else if max_clearance < 0.0 {
            FitResult::Interference
        } else {
            FitResult::Transition
        };

        FitAnalysis {
            worst_case_min_clearance: min_clearance,
            worst_case_max_clearance: max_clearance,
            fit_result,
            statistical: None, // Calculated on demand via with_statistical()
        }
    }

    /// Calculate fit from two Dimension structs, auto-detecting which is hole vs shaft
    /// based on the `internal` field.
    ///
    /// Returns error if both dimensions have the same internal/external designation.
    pub fn from_dimensions(dim_a: &Dimension, dim_b: &Dimension) -> Result<Self> {
        // Auto-detect: internal=true is hole, internal=false is shaft
        let (hole_dim, shaft_dim) = if dim_a.internal && !dim_b.internal {
            (dim_a, dim_b)
        } else if !dim_a.internal && dim_b.internal {
            (dim_b, dim_a)
        } else if dim_a.internal && dim_b.internal {
            return Err(miette!(
                "Mate requires one internal and one external feature (both are internal)"
            ));
        } else {
            return Err(miette!(
                "Mate requires one internal and one external feature (both are external)"
            ));
        };

        // Hole limits (internal feature)
        let hole_max = hole_dim.nominal + hole_dim.plus_tol; // LMC
        let hole_min = hole_dim.nominal - hole_dim.minus_tol; // MMC

        // Shaft limits (external feature)
        let shaft_max = shaft_dim.nominal + shaft_dim.plus_tol; // MMC
        let shaft_min = shaft_dim.nominal - shaft_dim.minus_tol; // LMC

        // Clearance calculations (positive = clearance, negative = interference)
        let min_clearance = hole_min - shaft_max;
        let max_clearance = hole_max - shaft_min;

        // Determine fit result
        let fit_result = if min_clearance > 0.0 {
            FitResult::Clearance
        } else if max_clearance < 0.0 {
            FitResult::Interference
        } else {
            FitResult::Transition
        };

        Ok(FitAnalysis {
            worst_case_min_clearance: min_clearance,
            worst_case_max_clearance: max_clearance,
            fit_result,
            statistical: None, // Calculated on demand via with_statistical()
        })
    }

    /// Add statistical analysis to this fit analysis
    /// sigma_level: process capability (6.0 for ±3σ, 4.0 for ±2σ, etc.)
    pub fn with_statistical(
        mut self,
        hole_dim: &Dimension,
        shaft_dim: &Dimension,
        sigma_level: f64,
    ) -> Result<Self> {
        self.statistical = Some(StatisticalFit::calculate(hole_dim, shaft_dim, sigma_level)?);
        Ok(self)
    }

    /// Check if this is an acceptable clearance fit
    pub fn is_clearance(&self) -> bool {
        self.fit_result == FitResult::Clearance
    }

    /// Check if this is an acceptable interference fit
    pub fn is_interference(&self) -> bool {
        self.fit_result == FitResult::Interference
    }
}

/// Statistical fit analysis using RSS (Root Sum Square) method
/// Calculates clearance distribution assuming normal distributions for hole and shaft
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StatisticalFit {
    /// Mean clearance (hole_mean - shaft_mean)
    pub mean_clearance: f64,

    /// Standard deviation of clearance (RSS of hole and shaft σ)
    pub sigma_clearance: f64,

    /// Minimum clearance at 3σ (mean - 3σ)
    pub clearance_3sigma_min: f64,

    /// Maximum clearance at 3σ (mean + 3σ)
    pub clearance_3sigma_max: f64,

    /// Probability of interference (clearance < 0) as percentage
    pub probability_interference: f64,

    /// Fit classification at 3σ limits
    pub fit_result_3sigma: FitResult,
}

/// Standard normal cumulative distribution function (CDF)
/// Φ(z) = probability that a standard normal random variable is ≤ z
/// Uses Hastings approximation (error < 7.5e-8)
fn normal_cdf(z: f64) -> f64 {
    if z.is_nan() {
        return 0.5;
    }
    if z >= 8.0 {
        return 1.0;
    }
    if z <= -8.0 {
        return 0.0;
    }

    // Handle negative z by symmetry: Φ(-z) = 1 - Φ(z)
    let (z_abs, negate) = if z < 0.0 { (-z, true) } else { (z, false) };

    // Hastings approximation constants (A&S 26.2.17)
    const B0: f64 = 0.2316419;
    const B1: f64 = 0.319381530;
    const B2: f64 = -0.356563782;
    const B3: f64 = 1.781477937;
    const B4: f64 = -1.821255978;
    const B5: f64 = 1.330274429;

    let t = 1.0 / (1.0 + B0 * z_abs);
    let t2 = t * t;
    let t3 = t2 * t;
    let t4 = t3 * t;
    let t5 = t4 * t;

    let pdf = (-0.5 * z_abs * z_abs).exp() / (2.0 * std::f64::consts::PI).sqrt();
    let cdf = 1.0 - pdf * (B1 * t + B2 * t2 + B3 * t3 + B4 * t4 + B5 * t5);

    if negate {
        1.0 - cdf
    } else {
        cdf
    }
}

impl StatisticalFit {
    /// Calculate statistical fit from hole and shaft dimensions
    /// sigma_level: process capability (6.0 for ±3σ, 4.0 for ±2σ, etc.)
    pub fn calculate(
        hole_dim: &Dimension,
        shaft_dim: &Dimension,
        sigma_level: f64,
    ) -> Result<Self> {
        // Verify we have one internal and one external
        if !hole_dim.internal {
            return Err(miette!("First dimension must be internal (hole)"));
        }
        if shaft_dim.internal {
            return Err(miette!("Second dimension must be external (shaft)"));
        }

        // Calculate process means (center of tolerance band)
        let hole_mean_offset = (hole_dim.plus_tol - hole_dim.minus_tol) / 2.0;
        let hole_mean = hole_dim.nominal + hole_mean_offset;

        let shaft_mean_offset = (shaft_dim.plus_tol - shaft_dim.minus_tol) / 2.0;
        let shaft_mean = shaft_dim.nominal + shaft_mean_offset;

        // Mean clearance
        let mean_clearance = hole_mean - shaft_mean;

        // Calculate σ for each (σ = tolerance_band / sigma_level)
        let hole_sigma = hole_dim.tolerance_band() / sigma_level;
        let shaft_sigma = shaft_dim.tolerance_band() / sigma_level;

        // RSS combination: σ_clearance = √(σ_hole² + σ_shaft²)
        let sigma_clearance = (hole_sigma * hole_sigma + shaft_sigma * shaft_sigma).sqrt();

        // 3σ limits
        let clearance_3sigma_min = mean_clearance - 3.0 * sigma_clearance;
        let clearance_3sigma_max = mean_clearance + 3.0 * sigma_clearance;

        // Probability of interference: P(clearance < 0) = Φ(-mean/σ)
        let z = if sigma_clearance > 0.0 {
            -mean_clearance / sigma_clearance
        } else if mean_clearance >= 0.0 {
            f64::NEG_INFINITY // 0% interference
        } else {
            f64::INFINITY // 100% interference
        };
        let probability_interference = normal_cdf(z) * 100.0;

        // Determine fit result at 3σ
        let fit_result_3sigma = if clearance_3sigma_min > 0.0 {
            FitResult::Clearance
        } else if clearance_3sigma_max < 0.0 {
            FitResult::Interference
        } else {
            FitResult::Transition
        };

        Ok(StatisticalFit {
            mean_clearance,
            sigma_clearance,
            clearance_3sigma_min,
            clearance_3sigma_max,
            probability_interference,
            fit_result_3sigma,
        })
    }

    /// Calculate from dimensions, auto-detecting hole vs shaft
    pub fn from_dimensions(dim_a: &Dimension, dim_b: &Dimension, sigma_level: f64) -> Result<Self> {
        if dim_a.internal && !dim_b.internal {
            Self::calculate(dim_a, dim_b, sigma_level)
        } else if !dim_a.internal && dim_b.internal {
            Self::calculate(dim_b, dim_a, sigma_level)
        } else if dim_a.internal && dim_b.internal {
            Err(miette!(
                "Statistical fit requires one internal and one external feature (both are internal)"
            ))
        } else {
            Err(miette!(
                "Statistical fit requires one internal and one external feature (both are external)"
            ))
        }
    }
}

/// Mate links
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MateLinks {
    /// Stackups using this mate
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub used_in_stackups: Vec<String>,

    /// Requirements verified by this mate
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub verifies: Vec<String>,
}

/// Cached feature reference info for mates (denormalized for readability, validated on check)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MateFeatureRef {
    /// Feature entity ID (FEAT-...)
    pub id: EntityId,

    /// Feature name (cached from feature entity)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Component ID that owns this feature (cached)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub component_id: Option<String>,

    /// Component name/title (cached for readability)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub component_name: Option<String>,
}

impl std::fmt::Display for MateFeatureRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Display the name if available, otherwise the ID
        if let Some(ref name) = self.name {
            write!(f, "{}", name)
        } else {
            write!(f, "{}", self.id)
        }
    }
}

impl MateFeatureRef {
    /// Create a new MateFeatureRef with just the ID
    pub fn new(id: EntityId) -> Self {
        Self {
            id,
            name: None,
            component_id: None,
            component_name: None,
        }
    }

    /// Create a MateFeatureRef with all cached info populated
    pub fn with_cache(
        id: EntityId,
        name: Option<String>,
        component_id: Option<String>,
        component_name: Option<String>,
    ) -> Self {
        Self {
            id,
            name,
            component_id,
            component_name,
        }
    }
}

/// Mate entity - 1:1 contact between two features
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mate {
    /// Unique identifier (MATE-...)
    pub id: EntityId,

    /// Mate title/name
    pub title: String,

    /// Detailed description
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// First feature (FEAT-...) - typically the hole/bore (with cached info)
    pub feature_a: MateFeatureRef,

    /// Second feature (FEAT-...) - typically the shaft/pin (with cached info)
    pub feature_b: MateFeatureRef,

    /// Mate type classification
    pub mate_type: MateType,

    /// Automatically calculated fit analysis
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fit_analysis: Option<FitAnalysis>,

    /// Additional notes
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,

    /// Classification tags
    #[serde(default)]
    pub tags: Vec<String>,

    /// Current status
    #[serde(default)]
    pub status: Status,

    /// Links to other entities
    #[serde(default)]
    pub links: MateLinks,

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

impl Entity for Mate {
    const PREFIX: &'static str = "MATE";

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

impl Mate {
    /// Create a new mate with required fields (accepts feature EntityIds)
    pub fn new(
        title: impl Into<String>,
        feature_a: EntityId,
        feature_b: EntityId,
        mate_type: MateType,
        author: impl Into<String>,
    ) -> Self {
        Self {
            id: EntityId::new(EntityPrefix::Mate),
            title: title.into(),
            description: None,
            feature_a: MateFeatureRef::new(feature_a),
            feature_b: MateFeatureRef::new(feature_b),
            mate_type,
            fit_analysis: None,
            notes: None,
            tags: Vec::new(),
            status: Status::default(),
            links: MateLinks::default(),
            created: Utc::now(),
            author: author.into(),
            entity_revision: 1,
        }
    }

    /// Create a new mate with full feature references (including cached info)
    pub fn with_features(
        title: impl Into<String>,
        feature_a: MateFeatureRef,
        feature_b: MateFeatureRef,
        mate_type: MateType,
        author: impl Into<String>,
    ) -> Self {
        Self {
            id: EntityId::new(EntityPrefix::Mate),
            title: title.into(),
            description: None,
            feature_a,
            feature_b,
            mate_type,
            fit_analysis: None,
            notes: None,
            tags: Vec::new(),
            status: Status::default(),
            links: MateLinks::default(),
            created: Utc::now(),
            author: author.into(),
            entity_revision: 1,
        }
    }

    /// Set fit analysis from feature dimensions (legacy tuple interface)
    pub fn calculate_fit(&mut self, hole_dim: (f64, f64, f64), shaft_dim: (f64, f64, f64)) {
        self.fit_analysis = Some(FitAnalysis::calculate(hole_dim, shaft_dim));
    }

    /// Calculate fit analysis from two Dimension structs
    /// Auto-detects which is hole vs shaft based on the `internal` field
    pub fn calculate_fit_from_dimensions(
        &mut self,
        dim_a: &Dimension,
        dim_b: &Dimension,
    ) -> Result<()> {
        self.fit_analysis = Some(FitAnalysis::from_dimensions(dim_a, dim_b)?);
        Ok(())
    }

    /// Check if fit analysis has been calculated
    pub fn has_analysis(&self) -> bool {
        self.fit_analysis.is_some()
    }

    /// Get fit result summary string
    pub fn fit_summary(&self) -> String {
        match &self.fit_analysis {
            Some(analysis) => format!(
                "{} ({:.4} to {:.4})",
                analysis.fit_result,
                analysis.worst_case_min_clearance,
                analysis.worst_case_max_clearance
            ),
            None => "Not calculated".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mate_creation() {
        let feat_a = EntityId::new(EntityPrefix::Feat);
        let feat_b = EntityId::new(EntityPrefix::Feat);
        let mate = Mate::new(
            "Pin-Hole Mate",
            feat_a.clone(),
            feat_b.clone(),
            MateType::Clearance,
            "Author",
        );
        assert_eq!(mate.title, "Pin-Hole Mate");
        assert_eq!(mate.feature_a.id, feat_a);
        assert_eq!(mate.feature_b.id, feat_b);
        assert_eq!(mate.mate_type, MateType::Clearance);
        assert!(mate.fit_analysis.is_none());
    }

    #[test]
    fn test_clearance_fit_calculation() {
        // Hole: 10.0 +0.1/-0.0 => 10.0 to 10.1
        // Shaft: 9.9 +0.0/-0.1 => 9.8 to 9.9
        // Min clearance: 10.0 - 9.9 = 0.1
        // Max clearance: 10.1 - 9.8 = 0.3
        let analysis = FitAnalysis::calculate((10.0, 0.1, 0.0), (9.9, 0.0, 0.1));

        assert!((analysis.worst_case_min_clearance - 0.1).abs() < 1e-10);
        assert!((analysis.worst_case_max_clearance - 0.3).abs() < 1e-10);
        assert_eq!(analysis.fit_result, FitResult::Clearance);
    }

    #[test]
    fn test_interference_fit_calculation() {
        // Hole: 10.0 +0.0/-0.1 => 9.9 to 10.0
        // Shaft: 10.1 +0.1/-0.0 => 10.1 to 10.2
        // Min clearance: 9.9 - 10.2 = -0.3
        // Max clearance: 10.0 - 10.1 = -0.1
        let analysis = FitAnalysis::calculate((10.0, 0.0, 0.1), (10.1, 0.1, 0.0));

        assert!((analysis.worst_case_min_clearance - (-0.3)).abs() < 1e-10);
        assert!((analysis.worst_case_max_clearance - (-0.1)).abs() < 1e-10);
        assert_eq!(analysis.fit_result, FitResult::Interference);
    }

    #[test]
    fn test_transition_fit_calculation() {
        // Hole: 10.0 +0.1/-0.1 => 9.9 to 10.1
        // Shaft: 10.0 +0.1/-0.1 => 9.9 to 10.1
        // Min clearance: 9.9 - 10.1 = -0.2
        // Max clearance: 10.1 - 9.9 = 0.2
        let analysis = FitAnalysis::calculate((10.0, 0.1, 0.1), (10.0, 0.1, 0.1));

        assert!((analysis.worst_case_min_clearance - (-0.2)).abs() < 1e-10);
        assert!((analysis.worst_case_max_clearance - 0.2).abs() < 1e-10);
        assert_eq!(analysis.fit_result, FitResult::Transition);
    }

    #[test]
    fn test_entity_trait_implementation() {
        let feat_a = EntityId::new(EntityPrefix::Feat);
        let feat_b = EntityId::new(EntityPrefix::Feat);
        let mate = Mate::new("Test Mate", feat_a, feat_b, MateType::Clearance, "Author");
        assert!(mate.id().to_string().starts_with("MATE-"));
        assert_eq!(mate.title(), "Test Mate");
        assert_eq!(mate.author(), "Author");
        assert_eq!(mate.status(), "draft");
        assert_eq!(Mate::PREFIX, "MATE");
    }

    #[test]
    fn test_mate_roundtrip() {
        let feat_a = EntityId::new(EntityPrefix::Feat);
        let feat_b = EntityId::new(EntityPrefix::Feat);
        let mut mate = Mate::new(
            "Pin-Hole Mate",
            feat_a.clone(),
            feat_b.clone(),
            MateType::Clearance,
            "Author",
        );
        mate.description = Some("Locating pin engagement".to_string());
        // Hole: 10.0 +0.1/-0.0 => 10.0 to 10.1
        // Shaft: 9.8 +0.05/-0.05 => 9.75 to 9.85
        // Min clearance: 10.0 - 9.85 = 0.15 > 0 => Clearance
        mate.calculate_fit((10.0, 0.1, 0.0), (9.8, 0.05, 0.05));
        mate.notes = Some("Critical fit for alignment".to_string());
        mate.tags = vec!["alignment".to_string(), "critical".to_string()];

        let yaml = serde_yml::to_string(&mate).unwrap();
        let parsed: Mate = serde_yml::from_str(&yaml).unwrap();

        assert_eq!(parsed.title, "Pin-Hole Mate");
        assert_eq!(parsed.feature_a.id, feat_a);
        assert_eq!(parsed.feature_b.id, feat_b);
        assert!(parsed.fit_analysis.is_some());
        assert_eq!(
            parsed.fit_analysis.as_ref().unwrap().fit_result,
            FitResult::Clearance
        );
    }

    #[test]
    fn test_mate_type_serialization() {
        let feat_a = EntityId::new(EntityPrefix::Feat);
        let feat_b = EntityId::new(EntityPrefix::Feat);
        let mate = Mate::new("Test", feat_a, feat_b, MateType::Interference, "Author");
        let yaml = serde_yml::to_string(&mate).unwrap();
        assert!(yaml.contains("interference"));

        let parsed: Mate = serde_yml::from_str(&yaml).unwrap();
        assert_eq!(parsed.mate_type, MateType::Interference);
    }

    #[test]
    fn test_fit_summary() {
        let feat_a = EntityId::new(EntityPrefix::Feat);
        let feat_b = EntityId::new(EntityPrefix::Feat);
        let mut mate = Mate::new("Test", feat_a, feat_b, MateType::Clearance, "Author");

        // Before calculation
        assert_eq!(mate.fit_summary(), "Not calculated");

        // After calculation
        mate.calculate_fit((10.0, 0.1, 0.0), (9.9, 0.0, 0.1));
        let summary = mate.fit_summary();
        assert!(summary.contains("clearance"));
    }

    #[test]
    fn test_from_dimensions_auto_detect() {
        use crate::entities::stackup::Distribution;

        // Hole (internal=true): 10.0 +0.1/-0.0 => 10.0 to 10.1
        let hole_dim = Dimension {
            name: "bore".to_string(),
            nominal: 10.0,
            plus_tol: 0.1,
            minus_tol: 0.0,
            units: "mm".to_string(),
            internal: true,
            distribution: Distribution::default(),
        };

        // Shaft (internal=false): 9.9 +0.0/-0.1 => 9.8 to 9.9
        let shaft_dim = Dimension {
            name: "pin".to_string(),
            nominal: 9.9,
            plus_tol: 0.0,
            minus_tol: 0.1,
            units: "mm".to_string(),
            internal: false,
            distribution: Distribution::default(),
        };

        // Test with hole first
        let analysis = FitAnalysis::from_dimensions(&hole_dim, &shaft_dim).unwrap();
        assert!((analysis.worst_case_min_clearance - 0.1).abs() < 1e-10);
        assert!((analysis.worst_case_max_clearance - 0.3).abs() < 1e-10);
        assert_eq!(analysis.fit_result, FitResult::Clearance);

        // Test with shaft first - should auto-detect and give same result
        let analysis2 = FitAnalysis::from_dimensions(&shaft_dim, &hole_dim).unwrap();
        assert!((analysis2.worst_case_min_clearance - 0.1).abs() < 1e-10);
        assert!((analysis2.worst_case_max_clearance - 0.3).abs() < 1e-10);
        assert_eq!(analysis2.fit_result, FitResult::Clearance);
    }

    #[test]
    fn test_from_dimensions_both_internal_error() {
        use crate::entities::stackup::Distribution;

        let dim1 = Dimension {
            name: "hole1".to_string(),
            nominal: 10.0,
            plus_tol: 0.1,
            minus_tol: 0.0,
            units: "mm".to_string(),
            internal: true,
            distribution: Distribution::default(),
        };

        let dim2 = Dimension {
            name: "hole2".to_string(),
            nominal: 10.0,
            plus_tol: 0.1,
            minus_tol: 0.0,
            units: "mm".to_string(),
            internal: true,
            distribution: Distribution::default(),
        };

        let result = FitAnalysis::from_dimensions(&dim1, &dim2);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("both are internal"));
    }

    #[test]
    fn test_from_dimensions_both_external_error() {
        use crate::entities::stackup::Distribution;

        let dim1 = Dimension {
            name: "shaft1".to_string(),
            nominal: 10.0,
            plus_tol: 0.1,
            minus_tol: 0.0,
            units: "mm".to_string(),
            internal: false,
            distribution: Distribution::default(),
        };

        let dim2 = Dimension {
            name: "shaft2".to_string(),
            nominal: 10.0,
            plus_tol: 0.1,
            minus_tol: 0.0,
            units: "mm".to_string(),
            internal: false,
            distribution: Distribution::default(),
        };

        let result = FitAnalysis::from_dimensions(&dim1, &dim2);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("both are external"));
    }

    // ===== Phase 4: Statistical Mate Analysis Tests =====

    #[test]
    fn test_statistical_fit_clearance() {
        use crate::entities::stackup::Distribution;

        // H7/f7 type fit - guaranteed clearance
        // Hole: 10.0 +0.015/-0.000 (H7 on ø10)
        // Shaft: 9.990 +0.000/-0.013 (f7 on ø10)
        // Statistically: mean_clearance > 0, very low interference probability

        let hole_dim = Dimension {
            name: "bore".to_string(),
            nominal: 10.0,
            plus_tol: 0.015,
            minus_tol: 0.0,
            units: "mm".to_string(),
            internal: true,
            distribution: Distribution::Normal,
        };

        let shaft_dim = Dimension {
            name: "pin".to_string(),
            nominal: 9.990,
            plus_tol: 0.0,
            minus_tol: 0.013,
            units: "mm".to_string(),
            internal: false,
            distribution: Distribution::Normal,
        };

        let stat = StatisticalFit::calculate(&hole_dim, &shaft_dim, 6.0).unwrap();

        // Mean clearance should be positive (hole_mean - shaft_mean)
        // hole_mean = 10.0 + (0.015 - 0.0)/2 = 10.0075
        // shaft_mean = 9.990 + (0.0 - 0.013)/2 = 9.9835
        // mean_clearance ≈ 0.024
        assert!(
            stat.mean_clearance > 0.0,
            "Mean clearance should be positive for clearance fit, got {}",
            stat.mean_clearance
        );

        // Probability of interference should be very low
        assert!(
            stat.probability_interference < 1.0,
            "P(interference) should be very low for clearance fit, got {}%",
            stat.probability_interference
        );
    }

    #[test]
    fn test_statistical_fit_transition() {
        use crate::entities::stackup::Distribution;

        // Equal tolerances centered at same nominal -> ~50% interference probability
        let hole_dim = Dimension {
            name: "bore".to_string(),
            nominal: 10.0,
            plus_tol: 0.1,
            minus_tol: 0.1,
            units: "mm".to_string(),
            internal: true,
            distribution: Distribution::Normal,
        };

        let shaft_dim = Dimension {
            name: "pin".to_string(),
            nominal: 10.0,
            plus_tol: 0.1,
            minus_tol: 0.1,
            units: "mm".to_string(),
            internal: false,
            distribution: Distribution::Normal,
        };

        let stat = StatisticalFit::calculate(&hole_dim, &shaft_dim, 6.0).unwrap();

        // Mean clearance should be ~0 (same nominals)
        assert!(
            stat.mean_clearance.abs() < 0.01,
            "Mean clearance should be ~0 for equal fit, got {}",
            stat.mean_clearance
        );

        // Probability of interference should be ~50%
        assert!(
            (stat.probability_interference - 50.0).abs() < 5.0,
            "P(interference) should be ~50% for transition fit, got {}%",
            stat.probability_interference
        );
    }

    #[test]
    fn test_statistical_sigma_clearance() {
        use crate::entities::stackup::Distribution;

        let hole_dim = Dimension {
            name: "bore".to_string(),
            nominal: 10.0,
            plus_tol: 0.1,
            minus_tol: 0.1, // tol_band = 0.2
            units: "mm".to_string(),
            internal: true,
            distribution: Distribution::Normal,
        };

        let shaft_dim = Dimension {
            name: "pin".to_string(),
            nominal: 9.8,
            plus_tol: 0.1,
            minus_tol: 0.1, // tol_band = 0.2
            units: "mm".to_string(),
            internal: false,
            distribution: Distribution::Normal,
        };

        let stat = StatisticalFit::calculate(&hole_dim, &shaft_dim, 6.0).unwrap();

        // σ_clearance = √(σ_hole² + σ_shaft²)
        // σ_hole = 0.2/6, σ_shaft = 0.2/6
        // σ_clearance = √(2) × (0.2/6) ≈ 0.0471
        let expected_sigma = (2.0_f64).sqrt() * (0.2 / 6.0);
        assert!(
            (stat.sigma_clearance - expected_sigma).abs() < 0.001,
            "Sigma clearance should be {:.4}, got {:.4}",
            expected_sigma,
            stat.sigma_clearance
        );
    }
}
