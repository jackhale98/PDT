//! Stackup entity - Tolerance chain analysis with multiple contributors
//!
//! A stackup represents a tolerance chain with multiple dimensional contributors.
//! Supports worst-case, RSS (statistical), and Monte Carlo analysis methods.

use chrono::{DateTime, Utc};
use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::core::entity::{Entity, Status};
use crate::core::identity::{EntityId, EntityPrefix};
use crate::entities::feature::Feature;

/// Target/gap specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Target {
    /// Name of the target dimension/gap
    pub name: String,

    /// Nominal value
    pub nominal: f64,

    /// Upper specification limit
    pub upper_limit: f64,

    /// Lower specification limit
    pub lower_limit: f64,

    /// Units
    #[serde(default = "default_units")]
    pub units: String,

    /// Is this a critical dimension?
    #[serde(default)]
    pub critical: bool,
}

fn default_units() -> String {
    "mm".to_string()
}

/// Direction of contributor in stackup
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    /// Adds to the stack (positive contribution)
    Positive,
    /// Subtracts from the stack (negative contribution)
    Negative,
}

impl Default for Direction {
    fn default() -> Self {
        Direction::Positive
    }
}

/// Statistical distribution for Monte Carlo
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Distribution {
    /// Normal (Gaussian) distribution
    Normal,
    /// Uniform distribution
    Uniform,
    /// Triangular distribution
    Triangular,
}

impl Default for Distribution {
    fn default() -> Self {
        Distribution::Normal
    }
}

/// Cached feature reference info (denormalized for readability, validated on check)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureRef {
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

impl FeatureRef {
    /// Create a new FeatureRef with just the ID
    pub fn new(id: EntityId) -> Self {
        Self {
            id,
            name: None,
            component_id: None,
            component_name: None,
        }
    }

    /// Create a FeatureRef with all cached info populated
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

/// A contributor to the tolerance stackup
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contributor {
    /// Contributor name/description
    pub name: String,

    /// Optional reference to a Feature entity (with cached info)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feature: Option<FeatureRef>,

    /// Direction of contribution
    #[serde(default)]
    pub direction: Direction,

    /// Nominal value
    pub nominal: f64,

    /// Plus tolerance (positive number)
    pub plus_tol: f64,

    /// Minus tolerance (positive number)
    pub minus_tol: f64,

    /// Statistical distribution for Monte Carlo
    #[serde(default)]
    pub distribution: Distribution,

    /// Source reference (drawing number, etc.)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

impl Contributor {
    /// Get total tolerance band
    pub fn tolerance_band(&self) -> f64 {
        self.plus_tol + self.minus_tol
    }

    /// Get signed contribution based on direction
    pub fn signed_nominal(&self) -> f64 {
        match self.direction {
            Direction::Positive => self.nominal,
            Direction::Negative => -self.nominal,
        }
    }

    /// Sync values from a linked feature's primary dimension
    /// Returns true if any values were changed
    pub fn sync_from_feature(&mut self, feature: &Feature) -> bool {
        let mut changed = false;

        if let Some(dim) = feature.primary_dimension() {
            if (self.nominal - dim.nominal).abs() > f64::EPSILON {
                self.nominal = dim.nominal;
                changed = true;
            }
            if (self.plus_tol - dim.plus_tol).abs() > f64::EPSILON {
                self.plus_tol = dim.plus_tol;
                changed = true;
            }
            if (self.minus_tol - dim.minus_tol).abs() > f64::EPSILON {
                self.minus_tol = dim.minus_tol;
                changed = true;
            }
        }

        changed
    }

    /// Check if this contributor is out of sync with a feature
    pub fn is_out_of_sync(&self, feature: &Feature) -> bool {
        if let Some(dim) = feature.primary_dimension() {
            (self.nominal - dim.nominal).abs() > f64::EPSILON
                || (self.plus_tol - dim.plus_tol).abs() > f64::EPSILON
                || (self.minus_tol - dim.minus_tol).abs() > f64::EPSILON
        } else {
            false
        }
    }
}

/// Worst-case analysis results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorstCaseResult {
    /// Minimum possible result
    pub min: f64,

    /// Maximum possible result
    pub max: f64,

    /// Margin to specification limits
    pub margin: f64,

    /// Pass/fail/marginal
    pub result: AnalysisResult,
}

/// RSS (Root Sum Square) statistical analysis results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RssResult {
    /// Mean value
    pub mean: f64,

    /// 3-sigma spread (±3σ)
    pub sigma_3: f64,

    /// Margin to specification limits at 3σ
    pub margin: f64,

    /// Process capability index (Cpk)
    pub cpk: f64,

    /// Estimated yield percentage
    pub yield_percent: f64,
}

/// Monte Carlo simulation results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonteCarloResult {
    /// Number of iterations
    pub iterations: u32,

    /// Mean result
    pub mean: f64,

    /// Standard deviation
    pub std_dev: f64,

    /// Minimum value seen
    pub min: f64,

    /// Maximum value seen
    pub max: f64,

    /// Estimated yield percentage (within spec)
    pub yield_percent: f64,

    /// Lower percentile (2.5% for 95% CI)
    pub percentile_2_5: f64,

    /// Upper percentile (97.5% for 95% CI)
    pub percentile_97_5: f64,
}

/// Analysis result classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AnalysisResult {
    /// Within specification
    Pass,
    /// Close to limit (margin < 10% of tolerance)
    Marginal,
    /// Out of specification
    Fail,
}

impl std::fmt::Display for AnalysisResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AnalysisResult::Pass => write!(f, "pass"),
            AnalysisResult::Marginal => write!(f, "marginal"),
            AnalysisResult::Fail => write!(f, "fail"),
        }
    }
}

/// Combined analysis results
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AnalysisResults {
    /// Worst-case analysis
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worst_case: Option<WorstCaseResult>,

    /// RSS statistical analysis
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rss: Option<RssResult>,

    /// Monte Carlo simulation
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub monte_carlo: Option<MonteCarloResult>,
}

/// Disposition status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Disposition {
    /// Under review
    UnderReview,
    /// Approved
    Approved,
    /// Rejected
    Rejected,
}

impl Default for Disposition {
    fn default() -> Self {
        Disposition::UnderReview
    }
}

impl std::fmt::Display for Disposition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Disposition::UnderReview => write!(f, "under_review"),
            Disposition::Approved => write!(f, "approved"),
            Disposition::Rejected => write!(f, "rejected"),
        }
    }
}

/// Stackup links
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StackupLinks {
    /// Requirements verified by this stackup
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub verifies: Vec<String>,

    /// Mates used in this stackup
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mates_used: Vec<String>,
}

/// Stackup entity - tolerance chain analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stackup {
    /// Unique identifier (TOL-...)
    pub id: EntityId,

    /// Stackup title/name
    pub title: String,

    /// Detailed description
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Target specification
    pub target: Target,

    /// List of contributors to the stackup
    #[serde(default)]
    pub contributors: Vec<Contributor>,

    /// Analysis results (auto-calculated)
    #[serde(default)]
    pub analysis_results: AnalysisResults,

    /// Review disposition
    #[serde(default)]
    pub disposition: Disposition,

    /// Classification tags
    #[serde(default)]
    pub tags: Vec<String>,

    /// Current status
    #[serde(default)]
    pub status: Status,

    /// Links to other entities
    #[serde(default)]
    pub links: StackupLinks,

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

impl Entity for Stackup {
    const PREFIX: &'static str = "TOL";

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

impl Default for Stackup {
    fn default() -> Self {
        Self {
            id: EntityId::new(EntityPrefix::Tol),
            title: String::new(),
            description: None,
            target: Target {
                name: String::new(),
                nominal: 0.0,
                upper_limit: 0.0,
                lower_limit: 0.0,
                units: "mm".to_string(),
                critical: false,
            },
            contributors: Vec::new(),
            analysis_results: AnalysisResults::default(),
            disposition: Disposition::default(),
            tags: Vec::new(),
            status: Status::default(),
            links: StackupLinks::default(),
            created: Utc::now(),
            author: String::new(),
            entity_revision: 1,
        }
    }
}

impl Stackup {
    /// Create a new stackup with target specification
    pub fn new(
        title: impl Into<String>,
        target_name: impl Into<String>,
        target_nominal: f64,
        target_upper: f64,
        target_lower: f64,
        author: impl Into<String>,
    ) -> Self {
        Self {
            id: EntityId::new(EntityPrefix::Tol),
            title: title.into(),
            target: Target {
                name: target_name.into(),
                nominal: target_nominal,
                upper_limit: target_upper,
                lower_limit: target_lower,
                units: "mm".to_string(),
                critical: false,
            },
            author: author.into(),
            created: Utc::now(),
            ..Default::default()
        }
    }

    /// Add a contributor to the stackup
    pub fn add_contributor(&mut self, contributor: Contributor) {
        self.contributors.push(contributor);
    }

    /// Run all analyses
    pub fn analyze(&mut self) {
        self.analysis_results.worst_case = Some(self.calculate_worst_case());
        self.analysis_results.rss = Some(self.calculate_rss());
        self.analysis_results.monte_carlo = Some(self.calculate_monte_carlo(10000));
    }

    /// Calculate worst-case analysis
    pub fn calculate_worst_case(&self) -> WorstCaseResult {
        let mut min_result = 0.0;
        let mut max_result = 0.0;

        for contrib in &self.contributors {
            match contrib.direction {
                Direction::Positive => {
                    min_result += contrib.nominal - contrib.minus_tol;
                    max_result += contrib.nominal + contrib.plus_tol;
                }
                Direction::Negative => {
                    min_result -= contrib.nominal + contrib.plus_tol;
                    max_result -= contrib.nominal - contrib.minus_tol;
                }
            }
        }

        // Calculate margin (minimum distance to spec limits)
        let upper_margin = self.target.upper_limit - max_result;
        let lower_margin = min_result - self.target.lower_limit;
        let margin = upper_margin.min(lower_margin);

        // Determine result
        let tolerance_band = self.target.upper_limit - self.target.lower_limit;
        let marginal_threshold = tolerance_band * 0.1;

        let result = if margin > marginal_threshold {
            AnalysisResult::Pass
        } else if margin > 0.0 {
            AnalysisResult::Marginal
        } else {
            AnalysisResult::Fail
        };

        WorstCaseResult {
            min: min_result,
            max: max_result,
            margin,
            result,
        }
    }

    /// Calculate RSS (Root Sum Square) statistical analysis
    pub fn calculate_rss(&self) -> RssResult {
        let mut mean = 0.0;
        let mut variance = 0.0;

        for contrib in &self.contributors {
            let signed_nom = contrib.signed_nominal();
            mean += signed_nom;

            // Assume 3-sigma process: tolerance = 6σ, so σ = tolerance/6
            let contrib_sigma = contrib.tolerance_band() / 6.0;
            variance += contrib_sigma * contrib_sigma;
        }

        let sigma = variance.sqrt();
        let sigma_3 = 3.0 * sigma;

        // Calculate Cpk
        let upper_margin = self.target.upper_limit - mean;
        let lower_margin = mean - self.target.lower_limit;
        let cpk = if sigma > 0.0 {
            (upper_margin.min(lower_margin)) / (3.0 * sigma)
        } else {
            f64::INFINITY
        };

        // Estimate yield from Cpk (simplified)
        let yield_percent = if cpk >= 2.0 {
            99.9999
        } else if cpk >= 1.67 {
            99.9997
        } else if cpk >= 1.33 {
            99.99
        } else if cpk >= 1.0 {
            99.73
        } else if cpk >= 0.67 {
            95.45
        } else if cpk >= 0.33 {
            68.27
        } else {
            50.0
        };

        // Margin at 3σ
        let margin = (self.target.upper_limit - (mean + sigma_3))
            .min((mean - sigma_3) - self.target.lower_limit);

        RssResult {
            mean,
            sigma_3,
            margin,
            cpk,
            yield_percent,
        }
    }

    /// Run Monte Carlo simulation
    pub fn calculate_monte_carlo(&self, iterations: u32) -> MonteCarloResult {
        let mut rng = rand::rng();
        let mut results: Vec<f64> = Vec::with_capacity(iterations as usize);

        for _ in 0..iterations {
            let mut result = 0.0;

            for contrib in &self.contributors {
                let value = match contrib.distribution {
                    Distribution::Normal => {
                        // Box-Muller transform for normal distribution
                        let mean = contrib.nominal;
                        let sigma = contrib.tolerance_band() / 6.0;
                        let u1: f64 = rng.random();
                        let u2: f64 = rng.random();
                        let z = (-2.0_f64 * u1.ln()).sqrt()
                            * (2.0_f64 * std::f64::consts::PI * u2).cos();
                        mean + sigma * z
                    }
                    Distribution::Uniform => {
                        let min = contrib.nominal - contrib.minus_tol;
                        let max = contrib.nominal + contrib.plus_tol;
                        rng.random_range(min..=max)
                    }
                    Distribution::Triangular => {
                        let min = contrib.nominal - contrib.minus_tol;
                        let max = contrib.nominal + contrib.plus_tol;
                        let mode = contrib.nominal;
                        // Triangular distribution using inverse transform
                        let u: f64 = rng.random();
                        let fc = (mode - min) / (max - min);
                        if u < fc {
                            min + (u * (max - min) * (mode - min)).sqrt()
                        } else {
                            max - ((1.0 - u) * (max - min) * (max - mode)).sqrt()
                        }
                    }
                };

                match contrib.direction {
                    Direction::Positive => result += value,
                    Direction::Negative => result -= value,
                }
            }

            results.push(result);
        }

        // Calculate statistics
        results.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let n = results.len() as f64;
        let mean: f64 = results.iter().sum::<f64>() / n;
        let variance: f64 = results.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n;
        let std_dev = variance.sqrt();

        let min = results.first().copied().unwrap_or(0.0);
        let max = results.last().copied().unwrap_or(0.0);

        // Calculate yield (percentage within spec)
        let in_spec = results
            .iter()
            .filter(|&x| *x >= self.target.lower_limit && *x <= self.target.upper_limit)
            .count();
        let yield_percent = (in_spec as f64 / n) * 100.0;

        // Percentiles
        let p2_5_idx = ((iterations as f64) * 0.025) as usize;
        let p97_5_idx = ((iterations as f64) * 0.975) as usize;
        let percentile_2_5 = results.get(p2_5_idx).copied().unwrap_or(min);
        let percentile_97_5 = results.get(p97_5_idx).copied().unwrap_or(max);

        MonteCarloResult {
            iterations,
            mean,
            std_dev,
            min,
            max,
            yield_percent,
            percentile_2_5,
            percentile_97_5,
        }
    }

    /// Get number of contributors
    pub fn contributor_count(&self) -> usize {
        self.contributors.len()
    }

    /// Check if analysis has been run
    pub fn has_analysis(&self) -> bool {
        self.analysis_results.worst_case.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stackup_creation() {
        let stackup = Stackup::new("Gap Analysis", "Gap", 1.0, 1.5, 0.5, "Author");
        assert_eq!(stackup.title, "Gap Analysis");
        assert_eq!(stackup.target.nominal, 1.0);
        assert_eq!(stackup.target.upper_limit, 1.5);
        assert_eq!(stackup.target.lower_limit, 0.5);
    }

    #[test]
    fn test_add_contributor() {
        let mut stackup = Stackup::new("Test", "Gap", 1.0, 1.5, 0.5, "Author");
        stackup.add_contributor(Contributor {
            name: "Part A".to_string(),
            feature: None,
            direction: Direction::Positive,
            nominal: 10.0,
            plus_tol: 0.1,
            minus_tol: 0.1,
            distribution: Distribution::Normal,
            source: None,
        });

        assert_eq!(stackup.contributor_count(), 1);
    }

    #[test]
    fn test_worst_case_analysis_pass() {
        let mut stackup = Stackup::new("Test", "Gap", 1.0, 1.5, 0.5, "Author");

        // Part A: 10 ±0.1 (positive)
        stackup.add_contributor(Contributor {
            name: "Part A".to_string(),
            feature: None,
            direction: Direction::Positive,
            nominal: 10.0,
            plus_tol: 0.1,
            minus_tol: 0.1,
            distribution: Distribution::Normal,
            source: None,
        });

        // Part B: 9 ±0.1 (negative)
        stackup.add_contributor(Contributor {
            name: "Part B".to_string(),
            feature: None,
            direction: Direction::Negative,
            nominal: 9.0,
            plus_tol: 0.1,
            minus_tol: 0.1,
            distribution: Distribution::Normal,
            source: None,
        });

        // Worst case: min = (10-0.1) - (9+0.1) = 0.8
        //             max = (10+0.1) - (9-0.1) = 1.2
        let wc = stackup.calculate_worst_case();

        assert!((wc.min - 0.8).abs() < 1e-10);
        assert!((wc.max - 1.2).abs() < 1e-10);
        assert_eq!(wc.result, AnalysisResult::Pass);
    }

    #[test]
    fn test_worst_case_analysis_fail() {
        let mut stackup = Stackup::new("Test", "Gap", 1.0, 1.1, 0.9, "Author");

        // Tight tolerance that will fail worst-case
        stackup.add_contributor(Contributor {
            name: "Part A".to_string(),
            feature: None,
            direction: Direction::Positive,
            nominal: 10.0,
            plus_tol: 0.2,
            minus_tol: 0.2,
            distribution: Distribution::Normal,
            source: None,
        });

        stackup.add_contributor(Contributor {
            name: "Part B".to_string(),
            feature: None,
            direction: Direction::Negative,
            nominal: 9.0,
            plus_tol: 0.2,
            minus_tol: 0.2,
            distribution: Distribution::Normal,
            source: None,
        });

        // Worst case: min = (10-0.2) - (9+0.2) = 0.6
        //             max = (10+0.2) - (9-0.2) = 1.4
        // Spec: 0.9 to 1.1 => FAIL
        let wc = stackup.calculate_worst_case();
        assert_eq!(wc.result, AnalysisResult::Fail);
    }

    #[test]
    fn test_rss_analysis() {
        let mut stackup = Stackup::new("Test", "Gap", 1.0, 1.5, 0.5, "Author");

        stackup.add_contributor(Contributor {
            name: "Part A".to_string(),
            feature: None,
            direction: Direction::Positive,
            nominal: 10.0,
            plus_tol: 0.1,
            minus_tol: 0.1,
            distribution: Distribution::Normal,
            source: None,
        });

        stackup.add_contributor(Contributor {
            name: "Part B".to_string(),
            feature: None,
            direction: Direction::Negative,
            nominal: 9.0,
            plus_tol: 0.1,
            minus_tol: 0.1,
            distribution: Distribution::Normal,
            source: None,
        });

        let rss = stackup.calculate_rss();

        // Mean should be 10 - 9 = 1.0
        assert!((rss.mean - 1.0).abs() < 1e-10);
        // Cpk should be positive for this setup
        assert!(rss.cpk > 0.0);
    }

    #[test]
    fn test_monte_carlo_analysis() {
        let mut stackup = Stackup::new("Test", "Gap", 1.0, 1.5, 0.5, "Author");

        stackup.add_contributor(Contributor {
            name: "Part A".to_string(),
            feature: None,
            direction: Direction::Positive,
            nominal: 10.0,
            plus_tol: 0.1,
            minus_tol: 0.1,
            distribution: Distribution::Normal,
            source: None,
        });

        stackup.add_contributor(Contributor {
            name: "Part B".to_string(),
            feature: None,
            direction: Direction::Negative,
            nominal: 9.0,
            plus_tol: 0.1,
            minus_tol: 0.1,
            distribution: Distribution::Normal,
            source: None,
        });

        let mc = stackup.calculate_monte_carlo(1000);

        // Mean should be close to 1.0
        assert!((mc.mean - 1.0).abs() < 0.1);
        // Yield should be high for this setup
        assert!(mc.yield_percent > 90.0);
    }

    #[test]
    fn test_entity_trait_implementation() {
        let stackup = Stackup::new("Test Stackup", "Gap", 1.0, 1.5, 0.5, "Author");
        assert!(stackup.id().to_string().starts_with("TOL-"));
        assert_eq!(stackup.title(), "Test Stackup");
        assert_eq!(stackup.author(), "Author");
        assert_eq!(stackup.status(), "draft");
        assert_eq!(Stackup::PREFIX, "TOL");
    }

    #[test]
    fn test_stackup_roundtrip() {
        let mut stackup = Stackup::new("Gap Analysis", "Gap", 1.0, 1.5, 0.5, "Author");
        stackup.description = Some("Main gap stackup".to_string());
        stackup.target.critical = true;

        // Create a feature ID for testing
        let feat_id = EntityId::new(EntityPrefix::Feat);

        stackup.add_contributor(Contributor {
            name: "Part A Length".to_string(),
            feature: Some(FeatureRef {
                id: feat_id,
                name: Some("Length A".to_string()),
                component_id: Some("CMP-001".to_string()),
                component_name: Some("Housing".to_string()),
            }),
            direction: Direction::Positive,
            nominal: 10.0,
            plus_tol: 0.1,
            minus_tol: 0.05,
            distribution: Distribution::Normal,
            source: Some("DWG-001 Rev A".to_string()),
        });

        stackup.analyze();

        let yaml = serde_yml::to_string(&stackup).unwrap();
        let parsed: Stackup = serde_yml::from_str(&yaml).unwrap();

        assert_eq!(parsed.title, "Gap Analysis");
        assert_eq!(parsed.contributors.len(), 1);
        assert!(parsed.analysis_results.worst_case.is_some());
        assert!(parsed.analysis_results.rss.is_some());
        assert!(parsed.analysis_results.monte_carlo.is_some());
    }

    #[test]
    fn test_direction_serialization() {
        let contrib = Contributor {
            name: "Test".to_string(),
            feature: None,
            direction: Direction::Negative,
            nominal: 10.0,
            plus_tol: 0.1,
            minus_tol: 0.1,
            distribution: Distribution::Uniform,
            source: None,
        };

        let yaml = serde_yml::to_string(&contrib).unwrap();
        assert!(yaml.contains("negative"));
        assert!(yaml.contains("uniform"));
    }

    #[test]
    fn test_sync_from_feature() {
        use crate::entities::feature::{Feature, FeatureType};

        // Create a feature with a dimension
        let mut feature = Feature::new("CMP-001", FeatureType::Internal, "Test Hole", "Author");
        feature.add_dimension("diameter", 10.0, 0.1, 0.05, true);

        // Create a contributor with outdated values
        let mut contrib = Contributor {
            name: "Test".to_string(),
            feature: Some(FeatureRef {
                id: feature.id.clone(),
                name: None,
                component_id: None,
                component_name: None,
            }),
            direction: Direction::Positive,
            nominal: 9.5,   // Wrong value
            plus_tol: 0.2,  // Wrong value
            minus_tol: 0.1, // Wrong value
            distribution: Distribution::Normal,
            source: None,
        };

        // Check that it's out of sync
        assert!(contrib.is_out_of_sync(&feature));

        // Sync from feature
        let changed = contrib.sync_from_feature(&feature);
        assert!(changed);

        // Verify values were updated
        assert!((contrib.nominal - 10.0).abs() < f64::EPSILON);
        assert!((contrib.plus_tol - 0.1).abs() < f64::EPSILON);
        assert!((contrib.minus_tol - 0.05).abs() < f64::EPSILON);

        // Check that it's now in sync
        assert!(!contrib.is_out_of_sync(&feature));

        // Second sync should not change anything
        let changed_again = contrib.sync_from_feature(&feature);
        assert!(!changed_again);
    }
}
