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
#[derive(Default)]
pub enum Direction {
    /// Adds to the stack (positive contribution)
    #[default]
    Positive,
    /// Subtracts from the stack (negative contribution)
    Negative,
}

/// Statistical distribution for Monte Carlo
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum Distribution {
    /// Normal (Gaussian) distribution
    #[default]
    Normal,
    /// Uniform distribution
    Uniform,
    /// Triangular distribution
    Triangular,
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

/// GD&T contribution to tolerance stackup
/// Currently supports 1D position tolerance contribution
/// Future: Will support 3D Small Displacement Torsors for full GD&T analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GdtContribution {
    /// Position tolerance value (diameter of cylindrical zone)
    /// In 1D analysis, this adds to the contributor's tolerance band
    pub position_tolerance: f64,

    /// Actual feature size for bonus tolerance calculation (MMC/LMC)
    /// If None, uses MMC (worst-case, no bonus)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actual_size: Option<f64>,

    /// Calculated bonus tolerance (actual departure from MMC/LMC)
    /// Auto-calculated when actual_size is provided
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bonus: Option<f64>,

    /// Effective position tolerance (position_tolerance + bonus)
    /// Auto-calculated
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effective_tolerance: Option<f64>,

    // Future 3D fields (reserved for torsor-based analysis):
    // pub torsor_constraints: Option<TorsorConstraints>,
    // pub datum_reference_frame: Option<DatumReferenceFrame>,
    // pub projected_tolerance_zone: Option<f64>,
}

impl GdtContribution {
    /// Create a new GD&T contribution with just position tolerance (no bonus)
    pub fn new(position_tolerance: f64) -> Self {
        Self {
            position_tolerance,
            actual_size: None,
            bonus: None,
            effective_tolerance: Some(position_tolerance),
        }
    }

    /// Create with bonus calculation from actual size and MMC
    pub fn with_bonus(position_tolerance: f64, actual_size: f64, mmc: f64) -> Self {
        let bonus = (actual_size - mmc).abs();
        let effective = position_tolerance + bonus;
        Self {
            position_tolerance,
            actual_size: Some(actual_size),
            bonus: Some(bonus),
            effective_tolerance: Some(effective),
        }
    }

    /// Get effective tolerance (with bonus if applicable)
    pub fn effective(&self) -> f64 {
        self.effective_tolerance.unwrap_or(self.position_tolerance)
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

    /// GD&T position tolerance contribution (optional)
    /// When present, adds to the tolerance band for statistical analysis
    /// Future: Will support full 3D torsor-based analysis
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gdt_position: Option<GdtContribution>,
}

impl Contributor {
    /// Get total tolerance band (dimensional only)
    pub fn tolerance_band(&self) -> f64 {
        self.plus_tol + self.minus_tol
    }

    /// Get total tolerance band including GD&T position if present
    /// For 1D analysis, position tolerance is treated as additional bilateral tolerance
    /// (half added to each side, contributing to variance)
    pub fn total_tolerance_band(&self) -> f64 {
        let dim_band = self.plus_tol + self.minus_tol;
        if let Some(ref gdt) = self.gdt_position {
            // Position tolerance zone diameter adds to total variation
            // In 1D, we treat it as ± (effective / 2)
            dim_band + gdt.effective()
        } else {
            dim_band
        }
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

    /// Process capability index (Cp) - ignores centering
    /// Cp = (USL - LSL) / (6σ)
    #[serde(default)]
    pub cp: f64,

    /// Process capability index (Cpk) - accounts for centering
    /// Cpk = min(USL-μ, μ-LSL) / (3σ)
    pub cpk: f64,

    /// Estimated yield percentage
    pub yield_percent: f64,

    /// Sensitivity analysis: variance contribution percentage for each contributor
    /// Index matches contributors array; values sum to 100%
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sensitivity: Vec<f64>,

    /// Shifted mean when mean_shift_k > 0 (Bender method)
    /// Shifts toward nearest spec limit for worst-case analysis
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shifted_mean: Option<f64>,
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

    /// Process performance index (Pp) - uses sample std_dev
    /// Pp = (USL - LSL) / (6s)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pp: Option<f64>,

    /// Process performance index (Ppk) - uses sample std_dev, accounts for centering
    /// Ppk = min(USL-μ, μ-LSL) / (3s)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ppk: Option<f64>,
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

// ===== 3D SDT Tolerance Analysis Types =====

/// 3D analysis configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Analysis3DConfig {
    /// Enable 3D analysis for this stackup
    #[serde(default)]
    pub enabled: bool,

    /// Analysis method: "jacobian_torsor" or "monte_carlo_3d"
    #[serde(default = "default_3d_method")]
    pub method: String,

    /// Number of iterations for Monte Carlo 3D
    #[serde(default = "default_3d_iterations")]
    pub monte_carlo_iterations: u32,
}

fn default_3d_method() -> String {
    "jacobian_torsor".to_string()
}

fn default_3d_iterations() -> u32 {
    10000
}

impl Default for Analysis3DConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            method: default_3d_method(),
            monte_carlo_iterations: default_3d_iterations(),
        }
    }
}

/// Statistics for a single DOF in 3D analysis
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TorsorStats {
    /// Worst-case minimum value
    pub wc_min: f64,

    /// Worst-case maximum value
    pub wc_max: f64,

    /// Statistical mean
    #[serde(default)]
    pub rss_mean: f64,

    /// Statistical 3-sigma spread
    #[serde(default)]
    pub rss_3sigma: f64,

    /// Monte Carlo mean (if available)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mc_mean: Option<f64>,

    /// Monte Carlo std dev (if available)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mc_std_dev: Option<f64>,
}

/// Result torsor with statistics for all 6 DOFs
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResultTorsor {
    /// Translation along X
    pub u: TorsorStats,

    /// Translation along Y
    pub v: TorsorStats,

    /// Translation along Z
    pub w: TorsorStats,

    /// Rotation about X (radians)
    pub alpha: TorsorStats,

    /// Rotation about Y (radians)
    pub beta: TorsorStats,

    /// Rotation about Z (radians)
    pub gamma: TorsorStats,
}

/// 3D sensitivity entry for a contributor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sensitivity3DEntry {
    /// Contributor name
    pub name: String,

    /// Feature ID if linked
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feature_id: Option<String>,

    /// Variance contribution per DOF [u, v, w, α, β, γ] as percentages
    pub contribution_pct: [f64; 6],
}

/// Jacobian matrix summary for chain analysis
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct JacobianSummary {
    /// Number of contributors in chain
    pub chain_length: usize,

    /// Total constrained DOFs across chain
    pub total_constrained_dof: usize,

    /// Free DOFs in result
    pub result_free_dof: Vec<String>,
}

/// Combined 3D analysis results
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Analysis3DResults {
    /// Result torsor with 6-DOF statistics
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_torsor: Option<ResultTorsor>,

    /// 3D sensitivity analysis per contributor
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sensitivity_3d: Vec<Sensitivity3DEntry>,

    /// Jacobian chain summary
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jacobian_summary: Option<JacobianSummary>,

    /// Analysis timestamp
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub analyzed_at: Option<DateTime<Utc>>,
}

/// Disposition status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum Disposition {
    /// Under review
    #[default]
    UnderReview,
    /// Approved
    Approved,
    /// Rejected
    Rejected,
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

    /// Sigma level for statistical analysis (tolerance = sigma_level × σ)
    /// Default 6.0 means tolerance band spans ±3σ (99.73% of distribution)
    /// Lower values (e.g., 4.0) are more conservative (assume wider process variation)
    #[serde(default = "default_sigma_level")]
    pub sigma_level: f64,

    /// Mean shift factor (Bender k-factor) for process drift modeling
    /// Common values: 0.0 (none), 1.5 (automotive/Bender method)
    /// Shifts the mean toward the nearest spec limit for worst-case analysis
    #[serde(default)]
    pub mean_shift_k: f64,

    /// Include GD&T position tolerances in statistical analysis
    /// When true, contributors with gdt_position will use total_tolerance_band()
    #[serde(default)]
    pub include_gdt: bool,

    // ===== 3D SDT Analysis Fields =====

    /// Functional measurement direction [dx, dy, dz] for 3D analysis
    /// The result torsor will be projected onto this direction for 1D comparison
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub functional_direction: Option<[f64; 3]>,

    /// 3D analysis configuration
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub analysis_3d: Option<Analysis3DConfig>,

    /// 3D analysis results (auto-calculated)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub analysis_results_3d: Option<Analysis3DResults>,

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

fn default_sigma_level() -> f64 {
    6.0
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
            sigma_level: default_sigma_level(),
            mean_shift_k: 0.0,
            include_gdt: false,
            functional_direction: None,
            analysis_3d: None,
            analysis_results_3d: None,
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
        let mut individual_variances: Vec<f64> = Vec::with_capacity(self.contributors.len());

        for contrib in &self.contributors {
            // For unilateral tolerances, shift mean to center of tolerance band
            // This ensures the process is centered within the tolerance zone
            let mean_offset = (contrib.plus_tol - contrib.minus_tol) / 2.0;
            let process_mean = contrib.nominal + mean_offset;

            mean += match contrib.direction {
                Direction::Positive => process_mean,
                Direction::Negative => -process_mean,
            };

            // σ = tolerance_band / sigma_level (default 6.0 for ±3σ process)
            // When include_gdt is true, include GD&T position tolerance in the band
            let tol_band = if self.include_gdt {
                contrib.total_tolerance_band()
            } else {
                contrib.tolerance_band()
            };
            let contrib_sigma = tol_band / self.sigma_level;
            let contrib_variance = contrib_sigma * contrib_sigma;
            variance += contrib_variance;
            individual_variances.push(contrib_variance);
        }

        let sigma = variance.sqrt();
        let sigma_3 = 3.0 * sigma;

        // Calculate sensitivity (variance contribution percentage)
        // sensitivity_i = (σ_i² / Σσ_j²) × 100%
        let sensitivity = if variance > 0.0 {
            individual_variances
                .iter()
                .map(|v| (v / variance) * 100.0)
                .collect()
        } else {
            Vec::new()
        };

        // Apply mean shift (Bender k-factor) if configured
        // Shift toward nearest spec limit for worst-case analysis
        let (cpk_mean, shifted_mean) = if self.mean_shift_k > 0.0 && sigma > 0.0 {
            let upper_margin = self.target.upper_limit - mean;
            let lower_margin = mean - self.target.lower_limit;

            // Shift toward the nearest limit (worst-case direction)
            let shifted = if upper_margin < lower_margin {
                // Closer to upper limit, shift upward
                mean + self.mean_shift_k * sigma
            } else {
                // Closer to lower limit, shift downward
                mean - self.mean_shift_k * sigma
            };
            (shifted, Some(shifted))
        } else {
            (mean, None)
        };

        // Calculate Cp (ignores centering) = (USL - LSL) / (6σ)
        let spec_range = self.target.upper_limit - self.target.lower_limit;
        let cp = if sigma > 0.0 {
            spec_range / (6.0 * sigma)
        } else {
            f64::INFINITY
        };

        // Calculate Cpk using cpk_mean (shifted if k > 0, original otherwise)
        let upper_margin = self.target.upper_limit - cpk_mean;
        let lower_margin = cpk_mean - self.target.lower_limit;
        let cpk = if sigma > 0.0 {
            (upper_margin.min(lower_margin)) / (3.0 * sigma)
        } else {
            f64::INFINITY
        };

        // Calculate yield using normal distribution CDF (based on original mean)
        // Φ(z) = probability that a standard normal random variable is ≤ z
        let yield_upper_margin = self.target.upper_limit - mean;
        let yield_lower_margin = mean - self.target.lower_limit;
        let z_upper = if sigma > 0.0 {
            yield_upper_margin / sigma
        } else {
            f64::INFINITY
        };
        let z_lower = if sigma > 0.0 {
            -yield_lower_margin / sigma
        } else {
            f64::NEG_INFINITY
        };

        // Yield = Φ(z_upper) - Φ(z_lower)
        let yield_percent = (normal_cdf(z_upper) - normal_cdf(z_lower)) * 100.0;

        // Margin at 3σ
        let margin = (self.target.upper_limit - (mean + sigma_3))
            .min((mean - sigma_3) - self.target.lower_limit);

        RssResult {
            mean,
            sigma_3,
            margin,
            cp,
            cpk,
            yield_percent,
            sensitivity,
            shifted_mean,
        }
    }

    /// Run Monte Carlo simulation
    pub fn calculate_monte_carlo(&self, iterations: u32) -> MonteCarloResult {
        let (result, _samples) = self.calculate_monte_carlo_with_samples(iterations);
        result
    }

    /// Run Monte Carlo simulation and return both results and raw samples
    pub fn calculate_monte_carlo_with_samples(
        &self,
        iterations: u32,
    ) -> (MonteCarloResult, Vec<f64>) {
        let mut rng = rand::rng();
        let mut results: Vec<f64> = Vec::with_capacity(iterations as usize);

        for _ in 0..iterations {
            let mut result = 0.0;

            for contrib in &self.contributors {
                // Get tolerance band (with or without GD&T based on include_gdt flag)
                let tol_band = if self.include_gdt {
                    contrib.total_tolerance_band()
                } else {
                    contrib.tolerance_band()
                };

                let value = match contrib.distribution {
                    Distribution::Normal => {
                        // Box-Muller transform for normal distribution
                        // For unilateral tolerances, center the mean within the tolerance band
                        let mean_offset = (contrib.plus_tol - contrib.minus_tol) / 2.0;
                        let mean = contrib.nominal + mean_offset;
                        let sigma = tol_band / self.sigma_level;
                        let u1: f64 = rng.random();
                        let u2: f64 = rng.random();
                        let z = (-2.0_f64 * u1.ln()).sqrt()
                            * (2.0_f64 * std::f64::consts::PI * u2).cos();
                        mean + sigma * z
                    }
                    Distribution::Uniform => {
                        // For uniform, use the full tolerance band as the range
                        let half_band = tol_band / 2.0;
                        let center = contrib.nominal + (contrib.plus_tol - contrib.minus_tol) / 2.0;
                        rng.random_range((center - half_band)..=(center + half_band))
                    }
                    Distribution::Triangular => {
                        // For triangular, use the full tolerance band
                        let half_band = tol_band / 2.0;
                        let center = contrib.nominal + (contrib.plus_tol - contrib.minus_tol) / 2.0;
                        let min = center - half_band;
                        let max = center + half_band;
                        let mode = center;
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

        // Keep unsorted copy for CSV export
        let raw_samples = results.clone();

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

        // Calculate Pp and Ppk using sample std_dev (process performance indices)
        // These differ from Cp/Cpk in that they use actual sample statistics
        // rather than assumed process capability
        let (pp, ppk) = if std_dev > 0.0 {
            // Pp = (USL - LSL) / (6s) where s = sample std_dev
            let spec_range = self.target.upper_limit - self.target.lower_limit;
            let pp_val = spec_range / (6.0 * std_dev);

            // Ppk = min(USL-μ, μ-LSL) / (3s)
            let upper_margin = self.target.upper_limit - mean;
            let lower_margin = mean - self.target.lower_limit;
            let ppk_val = (upper_margin.min(lower_margin)) / (3.0 * std_dev);

            (Some(pp_val), Some(ppk_val))
        } else {
            (None, None)
        };

        (
            MonteCarloResult {
                iterations,
                mean,
                std_dev,
                min,
                max,
                yield_percent,
                percentile_2_5,
                percentile_97_5,
                pp,
                ppk,
            },
            raw_samples,
        )
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
    fn test_normal_cdf() {
        // Test known values from standard normal distribution table
        // Φ(0) = 0.5
        let v0 = normal_cdf(0.0);
        assert!((v0 - 0.5).abs() < 1e-6, "Φ(0) = {}, expected 0.5", v0);

        // Φ(1) ≈ 0.8413
        let v1 = normal_cdf(1.0);
        assert!((v1 - 0.8413).abs() < 0.01, "Φ(1) = {}, expected 0.8413", v1);

        // Φ(-1) ≈ 0.1587
        let v_neg1 = normal_cdf(-1.0);
        assert!(
            (v_neg1 - 0.1587).abs() < 0.01,
            "Φ(-1) = {}, expected 0.1587",
            v_neg1
        );

        // Φ(2) ≈ 0.9772
        let v2 = normal_cdf(2.0);
        assert!((v2 - 0.9772).abs() < 0.01, "Φ(2) = {}, expected 0.9772", v2);

        // Φ(3) ≈ 0.9987 (corresponds to ±3σ covering 99.73%)
        let v3 = normal_cdf(3.0);
        assert!((v3 - 0.9987).abs() < 0.01, "Φ(3) = {}, expected 0.9987", v3);

        // Extreme values
        assert!((normal_cdf(10.0) - 1.0).abs() < 1e-6);
        assert!((normal_cdf(-10.0) - 0.0).abs() < 1e-6);
    }

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
            gdt_position: None,
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
            gdt_position: None,
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
            gdt_position: None,
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
            gdt_position: None,
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
            gdt_position: None,
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
            gdt_position: None,
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
            gdt_position: None,
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
            gdt_position: None,
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
            gdt_position: None,
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
            gdt_position: None,
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
            gdt_position: None,
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
            gdt_position: None,
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

    #[test]
    fn test_pin_in_hole_clearance_analysis() {
        // Simulates a real-world pin-in-hole clearance analysis
        // Hole: diameter 10mm, Pin: diameter 8mm
        // Expected clearance (gap) = Hole - Pin = 10 - 8 = 2mm
        let mut stackup = Stackup::new(
            "Pin-Hole Clearance",
            "Gap",
            2.0, // nominal gap
            3.0, // upper limit
            0.0, // lower limit
            "Test",
        );

        // Hole feature (positive contributor - adds to gap)
        stackup.add_contributor(Contributor {
            name: "Hole - diameter".to_string(),
            feature: None,
            direction: Direction::Positive,
            nominal: 10.0,
            plus_tol: 0.015, // H7 hole tolerance
            minus_tol: 0.0,
            distribution: Distribution::Normal,
            source: None,
            gdt_position: None,
        });

        // Pin/shaft feature (negative contributor - subtracts from gap)
        stackup.add_contributor(Contributor {
            name: "OD - diameter".to_string(),
            feature: None,
            direction: Direction::Negative,
            nominal: 8.0,
            plus_tol: 0.0,
            minus_tol: 0.009, // f7 shaft tolerance
            distribution: Distribution::Normal,
            source: None,
            gdt_position: None,
        });

        // Run analysis
        let wc = stackup.calculate_worst_case();
        let rss = stackup.calculate_rss();
        let mc = stackup.calculate_monte_carlo(10000);

        // Worst case:
        // min = (10 - 0) - (8 + 0) = 2.0
        // max = (10 + 0.015) - (8 - 0.009) = 2.024
        assert!(
            (wc.min - 2.0).abs() < 0.001,
            "Worst case min should be ~2.0, got {}",
            wc.min
        );
        assert!(
            (wc.max - 2.024).abs() < 0.001,
            "Worst case max should be ~2.024, got {}",
            wc.max
        );

        // RSS mean should be approximately 2.0 (with small offset for asymmetric tolerances)
        // Mean offset for hole: (0.015 - 0) / 2 = 0.0075
        // Mean offset for pin: (0 - 0.009) / 2 = -0.0045
        // Expected mean = (10 + 0.0075) - (8 - 0.0045) = 10.0075 - 7.9955 = 2.012
        assert!(
            (rss.mean - 2.012).abs() < 0.01,
            "RSS mean should be ~2.012, got {}",
            rss.mean
        );

        // Monte Carlo mean should be close to RSS mean
        assert!(
            (mc.mean - 2.0).abs() < 0.1,
            "Monte Carlo mean should be ~2.0, got {}",
            mc.mean
        );

        // Both analyses should pass (gap is within spec)
        assert_eq!(wc.result, AnalysisResult::Pass);
        assert!(mc.yield_percent > 99.0);
    }

    #[test]
    fn test_stackup_yaml_roundtrip_direction() {
        // Test that direction is preserved correctly through YAML serialization
        let mut stackup = Stackup::new("Direction Test", "Gap", 2.0, 3.0, 0.0, "Test");

        stackup.add_contributor(Contributor {
            name: "Positive".to_string(),
            feature: None,
            direction: Direction::Positive,
            nominal: 10.0,
            plus_tol: 0.1,
            minus_tol: 0.1,
            distribution: Distribution::Normal,
            source: None,
            gdt_position: None,
        });

        stackup.add_contributor(Contributor {
            name: "Negative".to_string(),
            feature: None,
            direction: Direction::Negative,
            nominal: 8.0,
            plus_tol: 0.1,
            minus_tol: 0.1,
            distribution: Distribution::Normal,
            source: None,
            gdt_position: None,
        });

        // Serialize to YAML
        let yaml = serde_yml::to_string(&stackup).unwrap();

        // Verify YAML contains correct direction strings
        assert!(
            yaml.contains("direction: positive"),
            "YAML should contain 'direction: positive'"
        );
        assert!(
            yaml.contains("direction: negative"),
            "YAML should contain 'direction: negative'"
        );

        // Deserialize back
        let parsed: Stackup = serde_yml::from_str(&yaml).unwrap();

        // Verify directions are preserved
        assert_eq!(
            parsed.contributors[0].direction,
            Direction::Positive,
            "First contributor should be Positive"
        );
        assert_eq!(
            parsed.contributors[1].direction,
            Direction::Negative,
            "Second contributor should be Negative"
        );

        // Run RSS calculation on deserialized stackup
        let rss = parsed.calculate_rss();

        // Mean should be 10 - 8 = 2.0 (symmetric tolerances, no offset)
        assert!(
            (rss.mean - 2.0).abs() < 0.001,
            "RSS mean after YAML roundtrip should be ~2.0, got {}",
            rss.mean
        );
    }

    // ============================================================
    // Phase 1: Sensitivity Analysis Tests
    // ============================================================

    #[test]
    fn test_sensitivity_analysis_equal_tolerances() {
        let mut stackup = Stackup::new("Test", "Gap", 1.0, 2.0, 0.0, "Author");

        // Two equal contributors with identical tolerance bands
        stackup.add_contributor(Contributor {
            name: "Part A".to_string(),
            feature: None,
            direction: Direction::Positive,
            nominal: 10.0,
            plus_tol: 0.1,
            minus_tol: 0.1, // 0.2 band
            distribution: Distribution::Normal,
            source: None,
            gdt_position: None,
        });
        stackup.add_contributor(Contributor {
            name: "Part B".to_string(),
            feature: None,
            direction: Direction::Negative,
            nominal: 9.0,
            plus_tol: 0.1,
            minus_tol: 0.1, // 0.2 band
            distribution: Distribution::Normal,
            source: None,
            gdt_position: None,
        });

        let rss = stackup.calculate_rss();

        // Equal tolerances should give ~50% each
        assert_eq!(
            rss.sensitivity.len(),
            2,
            "Should have sensitivity for 2 contributors"
        );
        assert!(
            (rss.sensitivity[0] - 50.0).abs() < 0.1,
            "Part A should be ~50%, got {}",
            rss.sensitivity[0]
        );
        assert!(
            (rss.sensitivity[1] - 50.0).abs() < 0.1,
            "Part B should be ~50%, got {}",
            rss.sensitivity[1]
        );
        assert!(
            (rss.sensitivity.iter().sum::<f64>() - 100.0).abs() < 0.01,
            "Sensitivity should sum to 100%"
        );
    }

    #[test]
    fn test_sensitivity_unequal_tolerances() {
        let mut stackup = Stackup::new("Test", "Gap", 1.0, 2.0, 0.0, "Author");

        // Part A: 0.2 tolerance band
        // Part B: 0.1 tolerance band
        // Variance ratio: (0.2)² / (0.1)² = 4:1
        // Expected: 80% and 20%
        stackup.add_contributor(Contributor {
            name: "Part A".to_string(),
            feature: None,
            direction: Direction::Positive,
            nominal: 10.0,
            plus_tol: 0.1,
            minus_tol: 0.1, // 0.2 band
            distribution: Distribution::Normal,
            source: None,
            gdt_position: None,
        });
        stackup.add_contributor(Contributor {
            name: "Part B".to_string(),
            feature: None,
            direction: Direction::Negative,
            nominal: 9.0,
            plus_tol: 0.05,
            minus_tol: 0.05, // 0.1 band
            distribution: Distribution::Normal,
            source: None,
            gdt_position: None,
        });

        let rss = stackup.calculate_rss();

        // Variance contribution: (0.2/6)² = 0.00111, (0.1/6)² = 0.000278
        // Total = 0.001389, Part A = 80%, Part B = 20%
        assert!(
            (rss.sensitivity[0] - 80.0).abs() < 0.1,
            "Part A should be ~80%, got {}",
            rss.sensitivity[0]
        );
        assert!(
            (rss.sensitivity[1] - 20.0).abs() < 0.1,
            "Part B should be ~20%, got {}",
            rss.sensitivity[1]
        );
    }

    #[test]
    fn test_sensitivity_empty_stackup() {
        let stackup = Stackup::new("Test", "Gap", 1.0, 2.0, 0.0, "Author");
        let rss = stackup.calculate_rss();

        // Empty stackup should have empty sensitivity
        assert!(
            rss.sensitivity.is_empty(),
            "Empty stackup should have empty sensitivity"
        );
    }

    #[test]
    fn test_sensitivity_single_contributor() {
        let mut stackup = Stackup::new("Test", "Gap", 1.0, 2.0, 0.0, "Author");

        stackup.add_contributor(Contributor {
            name: "Only Part".to_string(),
            feature: None,
            direction: Direction::Positive,
            nominal: 1.0,
            plus_tol: 0.1,
            minus_tol: 0.1,
            distribution: Distribution::Normal,
            source: None,
            gdt_position: None,
        });

        let rss = stackup.calculate_rss();

        // Single contributor should be 100%
        assert_eq!(rss.sensitivity.len(), 1);
        assert!(
            (rss.sensitivity[0] - 100.0).abs() < 0.01,
            "Single contributor should be 100%, got {}",
            rss.sensitivity[0]
        );
    }

    #[test]
    fn test_sensitivity_three_contributors() {
        let mut stackup = Stackup::new("Test", "Gap", 1.0, 2.0, 0.0, "Author");

        // Three contributors with tolerance bands: 0.3, 0.2, 0.1
        // Variances: 0.0025, 0.00111, 0.000278 (ratio 9:4:1)
        // Expected: ~64.3%, ~28.6%, ~7.1%
        stackup.add_contributor(Contributor {
            name: "Part A".to_string(),
            feature: None,
            direction: Direction::Positive,
            nominal: 10.0,
            plus_tol: 0.15,
            minus_tol: 0.15, // 0.3 band
            distribution: Distribution::Normal,
            source: None,
            gdt_position: None,
        });
        stackup.add_contributor(Contributor {
            name: "Part B".to_string(),
            feature: None,
            direction: Direction::Positive,
            nominal: 5.0,
            plus_tol: 0.1,
            minus_tol: 0.1, // 0.2 band
            distribution: Distribution::Normal,
            source: None,
            gdt_position: None,
        });
        stackup.add_contributor(Contributor {
            name: "Part C".to_string(),
            feature: None,
            direction: Direction::Negative,
            nominal: 14.0,
            plus_tol: 0.05,
            minus_tol: 0.05, // 0.1 band
            distribution: Distribution::Normal,
            source: None,
            gdt_position: None,
        });

        let rss = stackup.calculate_rss();

        // Check that contributions sum to 100%
        let total: f64 = rss.sensitivity.iter().sum();
        assert!(
            (total - 100.0).abs() < 0.01,
            "Sensitivity should sum to 100%, got {}",
            total
        );

        // Part A should have the highest contribution
        assert!(
            rss.sensitivity[0] > rss.sensitivity[1],
            "Part A should have higher sensitivity than Part B"
        );
        assert!(
            rss.sensitivity[1] > rss.sensitivity[2],
            "Part B should have higher sensitivity than Part C"
        );
    }

    // ===== Phase 2: Configurable Sigma Level Tests =====

    #[test]
    fn test_configurable_sigma_default_6() {
        // New stackup should have sigma_level = 6.0 (default)
        let stackup = Stackup::new("Test", "Gap", 1.0, 2.0, 0.0, "Author");
        assert!(
            (stackup.sigma_level - 6.0).abs() < 0.001,
            "Default sigma_level should be 6.0, got {}",
            stackup.sigma_level
        );
    }

    #[test]
    fn test_sigma_affects_rss_variance() {
        // sigma=4 gives larger variance than sigma=6
        // Since σ = tolerance_band / sigma_level:
        //   At sigma_level=4: σ = tol/4 (larger variance)
        //   At sigma_level=6: σ = tol/6 (smaller variance)
        // Variance ratio should be (6/4)² = 2.25
        let mut stackup4 = Stackup::new("Test 4σ", "Gap", 1.0, 2.0, 0.0, "Author");
        stackup4.sigma_level = 4.0;
        stackup4.add_contributor(Contributor {
            name: "Part A".to_string(),
            feature: None,
            direction: Direction::Positive,
            nominal: 1.0,
            plus_tol: 0.1,
            minus_tol: 0.1, // 0.2 band
            distribution: Distribution::Normal,
            source: None,
            gdt_position: None,
        });

        let mut stackup6 = Stackup::new("Test 6σ", "Gap", 1.0, 2.0, 0.0, "Author");
        stackup6.sigma_level = 6.0;
        stackup6.add_contributor(Contributor {
            name: "Part A".to_string(),
            feature: None,
            direction: Direction::Positive,
            nominal: 1.0,
            plus_tol: 0.1,
            minus_tol: 0.1, // 0.2 band
            distribution: Distribution::Normal,
            source: None,
            gdt_position: None,
        });

        let rss4 = stackup4.calculate_rss();
        let rss6 = stackup6.calculate_rss();

        // sigma_3 = 3 * std_dev, so the ratio should be 6/4 = 1.5
        // (since σ₄ = tol/4 and σ₆ = tol/6, ratio = 6/4)
        let ratio = rss4.sigma_3 / rss6.sigma_3;
        assert!(
            (ratio - 1.5).abs() < 0.01,
            "RSS 3σ ratio should be 1.5 (6/4), got {}",
            ratio
        );
    }

    #[test]
    fn test_sigma_backward_compatibility() {
        // Old YAML without sigma_level should parse with default 6.0
        let yaml = r#"
id: TOL-01HC2JB7SMQX7RS1Y0GFKBHPTD
title: "Test Stackup"
target:
  name: "Gap"
  nominal: 1.0
  upper_limit: 2.0
  lower_limit: 0.0
contributors: []
created: 2024-01-01T00:00:00Z
author: "Test"
"#;

        let stackup: Stackup = serde_yml::from_str(yaml).expect("Should parse old YAML");
        assert!(
            (stackup.sigma_level - 6.0).abs() < 0.001,
            "Missing sigma_level should default to 6.0, got {}",
            stackup.sigma_level
        );
    }

    #[test]
    fn test_sigma_serialization_roundtrip() {
        // Verify sigma_level is properly serialized and deserialized
        let mut stackup = Stackup::new("Test", "Gap", 1.0, 2.0, 0.0, "Author");
        stackup.sigma_level = 4.0;

        let yaml = serde_yml::to_string(&stackup).unwrap();
        let parsed: Stackup = serde_yml::from_str(&yaml).unwrap();

        assert!(
            (parsed.sigma_level - 4.0).abs() < 0.001,
            "sigma_level should roundtrip, got {}",
            parsed.sigma_level
        );
    }

    // ===== Phase 3: Mean Shift Factor Tests =====

    #[test]
    fn test_mean_shift_default_zero() {
        // New stackup should have mean_shift_k = 0.0 (no shift)
        let stackup = Stackup::new("Test", "Gap", 1.0, 2.0, 0.0, "Author");
        assert!(
            stackup.mean_shift_k.abs() < 0.001,
            "Default mean_shift_k should be 0.0, got {}",
            stackup.mean_shift_k
        );
    }

    #[test]
    fn test_mean_shift_affects_cpk() {
        // k=1.5 shift should reduce Cpk (more conservative)
        // Setup: target centered at 1.0, USL=2.0, LSL=0.0
        // Process mean at 1.0 with ±3σ = 0.5
        // Without shift: Cpk = (1.0 - 0.0) / (3 * 0.167) ≈ 2.0
        // With k=1.5 shift toward nearest limit: shifted_mean changes, Cpk decreases

        let mut stackup_no_shift = Stackup::new("No Shift", "Gap", 1.0, 2.0, 0.0, "Author");
        stackup_no_shift.add_contributor(Contributor {
            name: "Part A".to_string(),
            feature: None,
            direction: Direction::Positive,
            nominal: 1.0,
            plus_tol: 0.5,
            minus_tol: 0.5, // 1.0 band, σ = 1.0/6 ≈ 0.167
            distribution: Distribution::Normal,
            source: None,
            gdt_position: None,
        });

        let mut stackup_with_shift = Stackup::new("With Shift", "Gap", 1.0, 2.0, 0.0, "Author");
        stackup_with_shift.mean_shift_k = 1.5;
        stackup_with_shift.add_contributor(Contributor {
            name: "Part A".to_string(),
            feature: None,
            direction: Direction::Positive,
            nominal: 1.0,
            plus_tol: 0.5,
            minus_tol: 0.5,
            distribution: Distribution::Normal,
            source: None,
            gdt_position: None,
        });

        let rss_no_shift = stackup_no_shift.calculate_rss();
        let rss_with_shift = stackup_with_shift.calculate_rss();

        // Shifted version should have lower Cpk (more conservative)
        assert!(
            rss_with_shift.cpk < rss_no_shift.cpk,
            "Mean shift should reduce Cpk: no_shift={:.2}, with_shift={:.2}",
            rss_no_shift.cpk,
            rss_with_shift.cpk
        );

        // Shifted mean should be present
        assert!(
            rss_with_shift.shifted_mean.is_some(),
            "Shifted mean should be populated when k > 0"
        );
    }

    #[test]
    fn test_mean_shift_toward_nearest_limit() {
        // Shift should go toward the nearest spec limit (worst-case direction)
        // Setup: target at 1.0, but process mean is at 0.7 (closer to LSL=0.0)
        // Shift should go toward LSL (subtract k×σ)

        let mut stackup = Stackup::new("Test", "Gap", 1.0, 2.0, 0.0, "Author");
        stackup.mean_shift_k = 1.0;
        stackup.add_contributor(Contributor {
            name: "Part A".to_string(),
            feature: None,
            direction: Direction::Positive,
            nominal: 0.7, // Closer to LSL
            plus_tol: 0.3,
            minus_tol: 0.3, // σ = 0.6/6 = 0.1
            distribution: Distribution::Normal,
            source: None,
            gdt_position: None,
        });

        let rss = stackup.calculate_rss();
        let shifted = rss.shifted_mean.expect("Should have shifted_mean");

        // Mean is 0.7, closer to LSL(0.0) than USL(2.0)
        // Should shift toward LSL: shifted = 0.7 - 1.0 × 0.1 = 0.6
        assert!(
            shifted < rss.mean,
            "Shift should go toward LSL (lower): mean={}, shifted={}",
            rss.mean,
            shifted
        );
    }

    #[test]
    fn test_mean_shift_backward_compatibility() {
        // Old YAML without mean_shift_k should parse with default 0.0
        let yaml = r#"
id: TOL-01HC2JB7SMQX7RS1Y0GFKBHPTD
title: "Test Stackup"
target:
  name: "Gap"
  nominal: 1.0
  upper_limit: 2.0
  lower_limit: 0.0
contributors: []
created: 2024-01-01T00:00:00Z
author: "Test"
"#;

        let stackup: Stackup = serde_yml::from_str(yaml).expect("Should parse old YAML");
        assert!(
            stackup.mean_shift_k.abs() < 0.001,
            "Missing mean_shift_k should default to 0.0, got {}",
            stackup.mean_shift_k
        );
    }

    // ===== Phase 5B: Process Capability Variants (Cp/Pp/Ppk) Tests =====

    #[test]
    fn test_cp_calculation() {
        // Cp = (USL - LSL) / (6σ)
        // Setup: USL=2.0, LSL=0.0, tolerance band=0.6 => σ = 0.6/6 = 0.1
        // Expected Cp = (2.0 - 0.0) / (6 × 0.1) = 2.0 / 0.6 = 3.33

        let mut stackup = Stackup::new("Test", "Gap", 1.0, 2.0, 0.0, "Author");
        stackup.add_contributor(Contributor {
            name: "Part A".to_string(),
            feature: None,
            direction: Direction::Positive,
            nominal: 1.0,
            plus_tol: 0.3,
            minus_tol: 0.3, // 0.6 band, σ = 0.1
            distribution: Distribution::Normal,
            source: None,
            gdt_position: None,
        });

        let rss = stackup.calculate_rss();

        // Cp = (USL - LSL) / (6σ) = 2.0 / 0.6 = 3.33
        assert!(
            (rss.cp - 3.33).abs() < 0.01,
            "Cp should be ~3.33, got {}",
            rss.cp
        );
    }

    #[test]
    fn test_cp_equals_cpk_centered() {
        // For a centered process (mean at center of spec), Cp ≈ Cpk
        let mut stackup = Stackup::new("Test", "Gap", 1.0, 2.0, 0.0, "Author");

        // Mean = 1.0, which is exactly center of [0.0, 2.0]
        stackup.add_contributor(Contributor {
            name: "Part A".to_string(),
            feature: None,
            direction: Direction::Positive,
            nominal: 1.0,
            plus_tol: 0.3,
            minus_tol: 0.3, // Symmetric tolerance
            distribution: Distribution::Normal,
            source: None,
            gdt_position: None,
        });

        let rss = stackup.calculate_rss();

        // For centered process, Cp should equal Cpk
        assert!(
            (rss.cp - rss.cpk).abs() < 0.01,
            "For centered process, Cp ({}) should equal Cpk ({})",
            rss.cp,
            rss.cpk
        );
    }

    #[test]
    fn test_cp_greater_than_cpk_off_center() {
        // For off-center process, Cp > Cpk
        // Setup: target center at 1.0, but process mean at 0.7 (off-center toward LSL)
        let mut stackup = Stackup::new("Test", "Gap", 1.0, 2.0, 0.0, "Author");

        stackup.add_contributor(Contributor {
            name: "Part A".to_string(),
            feature: None,
            direction: Direction::Positive,
            nominal: 0.7, // Off-center toward LSL
            plus_tol: 0.3,
            minus_tol: 0.3,
            distribution: Distribution::Normal,
            source: None,
            gdt_position: None,
        });

        let rss = stackup.calculate_rss();

        // Cp ignores centering, Cpk accounts for it
        // Off-center process: Cp > Cpk
        assert!(
            rss.cp > rss.cpk,
            "For off-center process, Cp ({}) should be > Cpk ({})",
            rss.cp,
            rss.cpk
        );
    }

    #[test]
    fn test_monte_carlo_pp_ppk() {
        // Pp/Ppk use Monte Carlo std_dev instead of RSS calculated σ
        // Pp = (USL - LSL) / (6s), Ppk = min(USL-μ, μ-LSL) / (3s)
        // where s = sample std_dev from simulation

        let mut stackup = Stackup::new("Test", "Gap", 1.0, 2.0, 0.0, "Author");
        stackup.add_contributor(Contributor {
            name: "Part A".to_string(),
            feature: None,
            direction: Direction::Positive,
            nominal: 1.0,
            plus_tol: 0.3,
            minus_tol: 0.3,
            distribution: Distribution::Normal,
            source: None,
            gdt_position: None,
        });

        let mc = stackup.calculate_monte_carlo(10000);

        // Pp and Ppk should be present
        assert!(
            mc.pp.is_some(),
            "Monte Carlo should calculate Pp"
        );
        assert!(
            mc.ppk.is_some(),
            "Monte Carlo should calculate Ppk"
        );

        let pp = mc.pp.unwrap();
        let ppk = mc.ppk.unwrap();

        // Both should be positive for a capable process
        assert!(pp > 0.0, "Pp should be positive, got {}", pp);
        assert!(ppk > 0.0, "Ppk should be positive, got {}", ppk);

        // Pp = (USL - LSL) / (6 × std_dev)
        let expected_pp = (2.0 - 0.0) / (6.0 * mc.std_dev);
        assert!(
            (pp - expected_pp).abs() < 0.01,
            "Pp should be (USL-LSL)/(6σ) = {:.3}, got {:.3}",
            expected_pp,
            pp
        );
    }

    #[test]
    fn test_pp_ppk_centered_equals() {
        // For centered process, Pp ≈ Ppk (like Cp/Cpk)
        let mut stackup = Stackup::new("Test", "Gap", 1.0, 2.0, 0.0, "Author");
        stackup.add_contributor(Contributor {
            name: "Part A".to_string(),
            feature: None,
            direction: Direction::Positive,
            nominal: 1.0, // Centered
            plus_tol: 0.3,
            minus_tol: 0.3,
            distribution: Distribution::Normal,
            source: None,
            gdt_position: None,
        });

        let mc = stackup.calculate_monte_carlo(50000);

        let pp = mc.pp.expect("Pp should be present");
        let ppk = mc.ppk.expect("Ppk should be present");

        // For centered process, Pp ≈ Ppk (within Monte Carlo variance)
        assert!(
            (pp - ppk).abs() < 0.3,
            "For centered process, Pp ({:.2}) should be close to Ppk ({:.2})",
            pp,
            ppk
        );
    }
}
