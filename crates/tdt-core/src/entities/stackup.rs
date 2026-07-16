//! Stackup entity - Tolerance chain analysis with multiple contributors
//!
//! A stackup represents a tolerance chain with multiple dimensional contributors.
//! Supports worst-case, RSS (statistical), and Monte Carlo analysis methods.

use chrono::{DateTime, Utc};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::core::entity::{Entity, Status};
use crate::core::identity::{EntityId, EntityPrefix};
use crate::core::stats::{box_muller, normal_cdf};
use crate::entities::feature::Feature;

/// Errors raised by stackup analysis when prerequisites aren't met.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum AnalysisError {
    /// The stackup has no contributors — analysis would be meaningless.
    #[error("stackup has no contributors; add contributors before running analysis")]
    NoContributors,

    /// Monte Carlo needs at least 2 iterations to compute sample statistics.
    #[error("Monte Carlo requires at least 2 iterations (got {0})")]
    TooFewIterations(u32),
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

    /// PRNG seed used for this run. Recorded so the simulation is reproducible.
    /// Required for regulated-environment audit trails (FDA, ISO 13485).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
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

    /// Wall-clock time when these results were generated. Together with
    /// `input_hash` lets a reader detect stale results when contributors change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub analyzed_at: Option<DateTime<Utc>>,

    /// SHA-256 hash of the contributor chain at analysis time. Stable across
    /// stackups with the same contributors — purely a function of the inputs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_hash: Option<String>,
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

/// Functional projection result - scalar deviation along functional direction
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FunctionalProjection {
    /// Functional direction used [dx, dy, dz]
    pub direction: [f64; 3],

    /// Worst-case range [min, max]
    pub wc_range: [f64; 2],

    /// RSS mean deviation
    pub rss_mean: f64,

    /// RSS 3-sigma deviation
    pub rss_3sigma: f64,

    /// Monte Carlo mean (if run)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mc_mean: Option<f64>,

    /// Monte Carlo std dev (if run)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mc_std_dev: Option<f64>,

    /// Capability index Cp (from 3D analysis)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cp: Option<f64>,

    /// Capability index Cpk (from 3D analysis)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpk: Option<f64>,

    /// Estimated yield percentage (from 3D analysis)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub yield_percent: Option<f64>,

    /// Pass/fail result based on worst-case
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wc_result: Option<String>,
}

/// Combined 3D analysis results
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Analysis3DResults {
    /// Result torsor with 6-DOF statistics
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_torsor: Option<ResultTorsor>,

    /// Functional projection (scalar deviation along functional direction)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub functional_result: Option<FunctionalProjection>,

    /// 3D sensitivity analysis per contributor
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sensitivity_3d: Vec<Sensitivity3DEntry>,

    /// Jacobian chain summary
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jacobian_summary: Option<JacobianSummary>,

    /// Analysis timestamp
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub analyzed_at: Option<DateTime<Utc>>,

    /// Seed used for the 3D Monte Carlo run. Re-running `tdt tol analyze
    /// --3d --seed <N>` with the same chain reproduces results bit for bit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mc_seed: Option<u64>,
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

    /// Run all analyses. Errors if the stackup has no contributors.
    pub fn analyze(&mut self) -> Result<(), AnalysisError> {
        let wc = self.calculate_worst_case()?;
        let rss = self.calculate_rss()?;
        let mc = self.calculate_monte_carlo(10000)?;
        self.analysis_results.worst_case = Some(wc);
        self.analysis_results.rss = Some(rss);
        self.analysis_results.monte_carlo = Some(mc);
        self.analysis_results.analyzed_at = Some(Utc::now());
        self.analysis_results.input_hash = Some(self.contributor_input_hash());
        Ok(())
    }

    /// Stable hash of every input that analysis results depend on: the
    /// contributor chain plus the target spec limits, sigma level, mean shift,
    /// and GD&T inclusion flag. Used to detect stale analysis results when any
    /// of these are mutated after analysis.
    pub fn contributor_input_hash(&self) -> String {
        let mut hasher = Sha256::new();
        // Analysis parameters — results are functions of these, not just of
        // the contributor chain.
        hasher.update(self.target.nominal.to_le_bytes());
        hasher.update(self.target.upper_limit.to_le_bytes());
        hasher.update(self.target.lower_limit.to_le_bytes());
        hasher.update(self.sigma_level.to_le_bytes());
        hasher.update(self.mean_shift_k.to_le_bytes());
        hasher.update([u8::from(self.include_gdt)]);
        if let Some(dir) = self.functional_direction {
            hasher.update(b"fdir:");
            for c in dir {
                hasher.update(c.to_le_bytes());
            }
        }
        for c in &self.contributors {
            // Length-prefix the variable-width name so adjacent fields can't
            // alias across record boundaries.
            hasher.update((c.name.len() as u64).to_le_bytes());
            hasher.update(c.name.as_bytes());
            hasher.update(c.nominal.to_le_bytes());
            hasher.update(c.plus_tol.to_le_bytes());
            hasher.update(c.minus_tol.to_le_bytes());
            let dir_byte: u8 = match c.direction {
                Direction::Positive => 0,
                Direction::Negative => 1,
            };
            hasher.update([dir_byte]);
            let dist_byte: u8 = match c.distribution {
                Distribution::Normal => 0,
                Distribution::Uniform => 1,
                Distribution::Triangular => 2,
            };
            hasher.update([dist_byte]);
            if let Some(ref f) = c.feature {
                hasher.update(b"feat:");
                hasher.update(f.id.to_string().as_bytes());
            }
            if let Some(ref g) = c.gdt_position {
                hasher.update(b"gdt:");
                hasher.update(g.position_tolerance.to_le_bytes());
                if let Some(s) = g.actual_size {
                    hasher.update(b"size:");
                    hasher.update(s.to_le_bytes());
                }
            }
        }
        format!("{:x}", hasher.finalize())
    }

    /// Whether stored analysis results match the current contributor chain.
    /// Returns `None` if no analysis has been run; otherwise `Some(true)` if
    /// fresh and `Some(false)` if stale.
    pub fn analysis_is_fresh(&self) -> Option<bool> {
        let stored = self.analysis_results.input_hash.as_ref()?;
        Some(stored == &self.contributor_input_hash())
    }

    /// Calculate worst-case analysis
    pub fn calculate_worst_case(&self) -> Result<WorstCaseResult, AnalysisError> {
        if self.contributors.is_empty() {
            return Err(AnalysisError::NoContributors);
        }
        let mut min_result = 0.0;
        let mut max_result = 0.0;

        for contrib in &self.contributors {
            // When include_gdt is set, GD&T position tolerance widens each side
            // by ±(effective / 2) — the same 1D convention used by RSS and
            // Monte Carlo via total_tolerance_band(). Worst case must be at
            // least as wide as the statistical methods.
            let gdt_half = if self.include_gdt {
                contrib
                    .gdt_position
                    .as_ref()
                    .map_or(0.0, |g| g.effective() / 2.0)
            } else {
                0.0
            };
            let plus_tol = contrib.plus_tol + gdt_half;
            let minus_tol = contrib.minus_tol + gdt_half;
            match contrib.direction {
                Direction::Positive => {
                    min_result += contrib.nominal - minus_tol;
                    max_result += contrib.nominal + plus_tol;
                }
                Direction::Negative => {
                    min_result -= contrib.nominal + plus_tol;
                    max_result -= contrib.nominal - minus_tol;
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

        Ok(WorstCaseResult {
            min: min_result,
            max: max_result,
            margin,
            result,
        })
    }

    /// Calculate RSS (Root Sum Square) statistical analysis
    pub fn calculate_rss(&self) -> Result<RssResult, AnalysisError> {
        if self.contributors.is_empty() {
            return Err(AnalysisError::NoContributors);
        }
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
        } else if upper_margin.min(lower_margin) >= 0.0 {
            // Zero variance, mean inside spec — infinitely capable.
            f64::INFINITY
        } else {
            // Zero variance, mean outside spec — Cpk diverges to -∞, NOT +∞.
            f64::NEG_INFINITY
        };

        // Calculate yield using normal distribution CDF (based on original mean)
        // Φ(z) = probability that a standard normal random variable is ≤ z
        let yield_percent = if sigma > 0.0 {
            let z_upper = (self.target.upper_limit - mean) / sigma;
            let z_lower = -(mean - self.target.lower_limit) / sigma;
            (normal_cdf(z_upper) - normal_cdf(z_lower)) * 100.0
        } else if mean >= self.target.lower_limit && mean <= self.target.upper_limit {
            // Zero variance: every sample lands exactly at the mean.
            100.0
        } else {
            0.0
        };

        // Margin at 3σ
        let margin = (self.target.upper_limit - (mean + sigma_3))
            .min((mean - sigma_3) - self.target.lower_limit);

        Ok(RssResult {
            mean,
            sigma_3,
            margin,
            cp,
            cpk,
            yield_percent,
            sensitivity,
            shifted_mean,
        })
    }

    /// Run Monte Carlo simulation with a freshly drawn random seed (recorded
    /// in the result for reproducibility).
    pub fn calculate_monte_carlo(
        &self,
        iterations: u32,
    ) -> Result<MonteCarloResult, AnalysisError> {
        if self.contributors.is_empty() {
            return Err(AnalysisError::NoContributors);
        }
        if iterations < 2 {
            return Err(AnalysisError::TooFewIterations(iterations));
        }
        let seed: u64 = rand::rng().random();
        let (result, _samples) = self.run_monte_carlo_with_seed_inner(iterations, seed);
        Ok(result)
    }

    /// Run Monte Carlo simulation with a caller-provided seed. Same seed +
    /// same contributor chain → same results, bit for bit.
    pub fn calculate_monte_carlo_with_seed(
        &self,
        iterations: u32,
        seed: u64,
    ) -> Result<MonteCarloResult, AnalysisError> {
        if self.contributors.is_empty() {
            return Err(AnalysisError::NoContributors);
        }
        if iterations < 2 {
            return Err(AnalysisError::TooFewIterations(iterations));
        }
        let (result, _samples) = self.run_monte_carlo_with_seed_inner(iterations, seed);
        Ok(result)
    }

    /// Run Monte Carlo simulation and return both results and raw samples.
    /// Generates a fresh random seed (recorded in the result).
    pub fn calculate_monte_carlo_with_samples(
        &self,
        iterations: u32,
    ) -> Result<(MonteCarloResult, Vec<f64>), AnalysisError> {
        if self.contributors.is_empty() {
            return Err(AnalysisError::NoContributors);
        }
        if iterations < 2 {
            return Err(AnalysisError::TooFewIterations(iterations));
        }
        let seed: u64 = rand::rng().random();
        Ok(self.run_monte_carlo_with_seed_inner(iterations, seed))
    }

    /// Run Monte Carlo simulation with samples and a caller-provided seed.
    pub fn calculate_monte_carlo_with_samples_seeded(
        &self,
        iterations: u32,
        seed: u64,
    ) -> Result<(MonteCarloResult, Vec<f64>), AnalysisError> {
        if self.contributors.is_empty() {
            return Err(AnalysisError::NoContributors);
        }
        if iterations < 2 {
            return Err(AnalysisError::TooFewIterations(iterations));
        }
        Ok(self.run_monte_carlo_with_seed_inner(iterations, seed))
    }

    fn run_monte_carlo_with_seed_inner(
        &self,
        iterations: u32,
        seed: u64,
    ) -> (MonteCarloResult, Vec<f64>) {
        let mut rng = StdRng::seed_from_u64(seed);
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
                        let z = box_muller(&mut rng);
                        mean + sigma * z
                    }
                    Distribution::Uniform => {
                        // For uniform, use the full tolerance band as the range
                        let half_band = tol_band / 2.0;
                        let center = contrib.nominal + (contrib.plus_tol - contrib.minus_tol) / 2.0;
                        rng.random_range((center - half_band)..=(center + half_band))
                    }
                    Distribution::Triangular => {
                        let half_band = tol_band / 2.0;
                        let center = contrib.nominal + (contrib.plus_tol - contrib.minus_tol) / 2.0;
                        let min = center - half_band;
                        let max = center + half_band;
                        if (max - min).abs() < f64::EPSILON {
                            center
                        } else {
                            let mode = center;
                            let u: f64 = rng.random();
                            let fc = (mode - min) / (max - min);
                            if u < fc {
                                min + (u * (max - min) * (mode - min)).sqrt()
                            } else {
                                max - ((1.0 - u) * (max - min) * (max - mode)).sqrt()
                            }
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
        results.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let n = results.len() as f64;
        let mean: f64 = results.iter().sum::<f64>() / n;
        let variance: f64 = results.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1.0);
        let std_dev = variance.sqrt();

        let min = results.first().copied().unwrap_or(0.0);
        let max = results.last().copied().unwrap_or(0.0);

        // Calculate yield (percentage within spec)
        let in_spec = results
            .iter()
            .filter(|&x| *x >= self.target.lower_limit && *x <= self.target.upper_limit)
            .count();
        let yield_percent = (in_spec as f64 / n) * 100.0;

        // Percentiles. Use floor for the lower bound and ceil for the upper
        // bound, then clamp to the valid index range so small iteration counts
        // never panic and never silently fall back to min/max.
        let last_idx = results.len().saturating_sub(1);
        let p2_5_idx = ((iterations as f64 * 0.025).floor() as usize).min(last_idx);
        let p97_5_idx_raw = (iterations as f64 * 0.975).ceil() as usize;
        let p97_5_idx = p97_5_idx_raw.saturating_sub(1).min(last_idx);
        let percentile_2_5 = results[p2_5_idx];
        let percentile_97_5 = results[p97_5_idx];

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
                seed: Some(seed),
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
        let wc = stackup.calculate_worst_case().expect("populated stackup");

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
        let wc = stackup.calculate_worst_case().expect("populated stackup");
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

        let rss = stackup.calculate_rss().expect("populated stackup");

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

        let mc = stackup
            .calculate_monte_carlo(1000)
            .expect("populated stackup");

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

        stackup.analyze().expect("populated stackup");

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
        let wc = stackup.calculate_worst_case().expect("populated stackup");
        let rss = stackup.calculate_rss().expect("populated stackup");
        let mc = stackup
            .calculate_monte_carlo(10000)
            .expect("populated stackup");

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
        let rss = parsed.calculate_rss().expect("populated stackup");

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

        let rss = stackup.calculate_rss().expect("populated stackup");

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

        let rss = stackup.calculate_rss().expect("populated stackup");

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

    // (Empty-stackup behavior is now covered by `test_empty_stackup_rejects_analysis`
    // in the Phase 6 block — analysis errors out instead of returning a degenerate
    // result with an empty sensitivity array.)

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

        let rss = stackup.calculate_rss().expect("populated stackup");

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

        let rss = stackup.calculate_rss().expect("populated stackup");

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

        let rss4 = stackup4.calculate_rss().expect("populated stackup");
        let rss6 = stackup6.calculate_rss().expect("populated stackup");

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

        let rss_no_shift = stackup_no_shift.calculate_rss().expect("populated stackup");
        let rss_with_shift = stackup_with_shift
            .calculate_rss()
            .expect("populated stackup");

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

        let rss = stackup.calculate_rss().expect("populated stackup");
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

        let rss = stackup.calculate_rss().expect("populated stackup");

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

        let rss = stackup.calculate_rss().expect("populated stackup");

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

        let rss = stackup.calculate_rss().expect("populated stackup");

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

        let mc = stackup
            .calculate_monte_carlo(10000)
            .expect("populated stackup");

        // Pp and Ppk should be present
        assert!(mc.pp.is_some(), "Monte Carlo should calculate Pp");
        assert!(mc.ppk.is_some(), "Monte Carlo should calculate Ppk");

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

        let mc = stackup
            .calculate_monte_carlo(50000)
            .expect("populated stackup");

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

    // ===== Phase 6: Negative-direction asymmetric tolerance regression =====

    /// Locks in the worst-case math for negative-direction contributors with
    /// asymmetric (unilateral) tolerances. The contribution range of a value
    /// `v ∈ [nominal - minus_tol, nominal + plus_tol]` flipped negative is
    /// `[-(nominal + plus_tol), -(nominal - minus_tol)]`.
    #[test]
    fn test_worst_case_negative_asymmetric_tolerances() {
        // Positive: 10 +0.10/-0.05 → range [9.95, 10.10]
        // Negative: 5  +0.20/-0.10 → range [4.90, 5.20]
        // Result range = pos − neg
        //   min = pos.min - neg.max = 9.95  - 5.20 = 4.75
        //   max = pos.max - neg.min = 10.10 - 4.90 = 5.20
        let mut stackup = Stackup::new("Asymmetric", "Gap", 5.0, 5.5, 4.5, "Author");
        stackup.add_contributor(Contributor {
            name: "Pos".into(),
            feature: None,
            direction: Direction::Positive,
            nominal: 10.0,
            plus_tol: 0.10,
            minus_tol: 0.05,
            distribution: Distribution::Normal,
            source: None,
            gdt_position: None,
        });
        stackup.add_contributor(Contributor {
            name: "Neg".into(),
            feature: None,
            direction: Direction::Negative,
            nominal: 5.0,
            plus_tol: 0.20,
            minus_tol: 0.10,
            distribution: Distribution::Normal,
            source: None,
            gdt_position: None,
        });

        let wc = stackup.calculate_worst_case().expect("populated stackup");
        assert!(
            (wc.min - 4.75).abs() < 1e-12,
            "min should be 4.75, got {}",
            wc.min
        );
        assert!(
            (wc.max - 5.20).abs() < 1e-12,
            "max should be 5.20, got {}",
            wc.max
        );
    }

    // ===== Phase 6: Empty-stackup rejection =====

    #[test]
    fn test_empty_stackup_rejects_analysis() {
        let stackup = Stackup::new("Empty", "Gap", 1.0, 1.5, 0.5, "Author");
        assert!(matches!(
            stackup.calculate_worst_case(),
            Err(AnalysisError::NoContributors)
        ));
        assert!(matches!(
            stackup.calculate_rss(),
            Err(AnalysisError::NoContributors)
        ));
        assert!(matches!(
            stackup.calculate_monte_carlo(100),
            Err(AnalysisError::NoContributors)
        ));
    }

    // ===== Phase 6: Cpk and yield in zero-variance regimes =====

    #[test]
    fn test_rss_cpk_zero_variance_in_spec() {
        // All-zero tolerances: sigma=0. Mean inside spec → Cpk=+inf, yield=100%.
        let mut stackup = Stackup::new("Tight", "Gap", 1.0, 2.0, 0.0, "Author");
        stackup.add_contributor(Contributor {
            name: "A".into(),
            feature: None,
            direction: Direction::Positive,
            nominal: 1.0,
            plus_tol: 0.0,
            minus_tol: 0.0,
            distribution: Distribution::Normal,
            source: None,
            gdt_position: None,
        });
        let rss = stackup.calculate_rss().expect("populated");
        assert!(rss.cpk.is_infinite() && rss.cpk > 0.0);
        assert!((rss.yield_percent - 100.0).abs() < 1e-9);
    }

    #[test]
    fn test_rss_cpk_zero_variance_out_of_spec() {
        // Zero variance, but mean is outside spec → Cpk should be -inf, yield=0%.
        let mut stackup = Stackup::new("Tight OOS", "Gap", 1.0, 2.0, 0.0, "Author");
        stackup.add_contributor(Contributor {
            name: "A".into(),
            feature: None,
            direction: Direction::Positive,
            nominal: 5.0, // out of [0, 2] spec
            plus_tol: 0.0,
            minus_tol: 0.0,
            distribution: Distribution::Normal,
            source: None,
            gdt_position: None,
        });
        let rss = stackup.calculate_rss().expect("populated");
        assert!(
            rss.cpk.is_infinite() && rss.cpk < 0.0,
            "Out-of-spec zero-variance Cpk should be -inf, got {}",
            rss.cpk
        );
        assert!(
            rss.yield_percent.abs() < 1e-9,
            "Yield must be 0% out of spec, got {}",
            rss.yield_percent
        );
    }

    // ===== Phase 6: Monte Carlo percentile bounds-safety =====

    #[test]
    fn test_monte_carlo_percentile_bounds_safe_small_n() {
        // Tiny iteration count must not panic and indices must be in-range.
        let mut stackup = Stackup::new("Small N", "Gap", 1.0, 2.0, 0.0, "Author");
        stackup.add_contributor(Contributor {
            name: "A".into(),
            feature: None,
            direction: Direction::Positive,
            nominal: 1.0,
            plus_tol: 0.1,
            minus_tol: 0.1,
            distribution: Distribution::Normal,
            source: None,
            gdt_position: None,
        });
        let mc = stackup.calculate_monte_carlo(5).expect("populated");
        // With 5 samples both percentiles should fall on real samples (≥min, ≤max).
        assert!(mc.percentile_2_5 >= mc.min && mc.percentile_2_5 <= mc.max);
        assert!(mc.percentile_97_5 >= mc.min && mc.percentile_97_5 <= mc.max);
        assert!(mc.percentile_2_5 <= mc.percentile_97_5);
    }

    // ===== Phase 6: Monte Carlo seed reproducibility =====

    #[test]
    fn test_monte_carlo_with_seed_is_reproducible() {
        let mut stackup = Stackup::new("Seeded", "Gap", 1.0, 2.0, 0.0, "Author");
        stackup.add_contributor(Contributor {
            name: "A".into(),
            feature: None,
            direction: Direction::Positive,
            nominal: 1.0,
            plus_tol: 0.1,
            minus_tol: 0.1,
            distribution: Distribution::Normal,
            source: None,
            gdt_position: None,
        });
        let r1 = stackup
            .calculate_monte_carlo_with_seed(1000, 42)
            .expect("populated");
        let r2 = stackup
            .calculate_monte_carlo_with_seed(1000, 42)
            .expect("populated");
        assert_eq!(r1.mean.to_bits(), r2.mean.to_bits());
        assert_eq!(r1.std_dev.to_bits(), r2.std_dev.to_bits());
        assert_eq!(r1.percentile_2_5.to_bits(), r2.percentile_2_5.to_bits());
        assert_eq!(r1.percentile_97_5.to_bits(), r2.percentile_97_5.to_bits());
        assert_eq!(r1.seed, Some(42));
    }

    #[test]
    fn test_monte_carlo_default_records_seed() {
        // Even when not seeded explicitly, the result must record the seed used
        // so the run can be reproduced later.
        let mut stackup = Stackup::new("Recorded", "Gap", 1.0, 2.0, 0.0, "Author");
        stackup.add_contributor(Contributor {
            name: "A".into(),
            feature: None,
            direction: Direction::Positive,
            nominal: 1.0,
            plus_tol: 0.1,
            minus_tol: 0.1,
            distribution: Distribution::Normal,
            source: None,
            gdt_position: None,
        });
        let r = stackup.calculate_monte_carlo(500).expect("populated");
        let seed = r.seed.expect("default monte carlo must record its seed");
        // Re-running with the recorded seed reproduces the result.
        let r2 = stackup
            .calculate_monte_carlo_with_seed(500, seed)
            .expect("populated");
        assert_eq!(r.mean.to_bits(), r2.mean.to_bits());
    }

    // ===== Phase 6: Analysis provenance (timestamp + input hash) =====

    #[test]
    fn test_analyze_records_timestamp_and_input_hash() {
        let mut stackup = Stackup::new("Provenance", "Gap", 1.0, 2.0, 0.0, "Author");
        stackup.add_contributor(Contributor {
            name: "A".into(),
            feature: None,
            direction: Direction::Positive,
            nominal: 1.0,
            plus_tol: 0.1,
            minus_tol: 0.1,
            distribution: Distribution::Normal,
            source: None,
            gdt_position: None,
        });
        stackup.analyze().expect("populated");
        assert!(stackup.analysis_results.analyzed_at.is_some());
        let h1 = stackup
            .analysis_results
            .input_hash
            .clone()
            .expect("hash recorded");

        // Mutating a contributor changes the input hash on next analysis.
        stackup.contributors[0].nominal = 1.5;
        stackup.analyze().expect("populated");
        let h2 = stackup
            .analysis_results
            .input_hash
            .clone()
            .expect("hash recorded");
        assert_ne!(h1, h2, "input hash must change when contributors change");
    }

    #[test]
    fn test_input_hash_stable_for_same_contributors() {
        let mut a = Stackup::new("A", "Gap", 1.0, 2.0, 0.0, "Author");
        let mut b = Stackup::new("B", "Gap", 1.0, 2.0, 0.0, "Other");
        let c = Contributor {
            name: "Part".into(),
            feature: None,
            direction: Direction::Positive,
            nominal: 1.0,
            plus_tol: 0.1,
            minus_tol: 0.05,
            distribution: Distribution::Normal,
            source: None,
            gdt_position: None,
        };
        a.add_contributor(c.clone());
        b.add_contributor(c);
        // Hash covers analysis inputs (contributors + target/sigma settings)
        // but not identity fields like title/author, so two stackups with the
        // same chain and spec share an input hash.
        assert_eq!(a.contributor_input_hash(), b.contributor_input_hash());

        // Changing any analysis input — here a spec limit — must change the
        // hash, otherwise stored results would be reported as fresh after the
        // spec they were judged against moved.
        b.target.upper_limit = 1.5;
        assert_ne!(a.contributor_input_hash(), b.contributor_input_hash());
    }

    #[test]
    fn test_worst_case_includes_gdt_when_enabled() {
        let mut stackup = Stackup::new("WC GDT", "Gap", 10.0, 10.35, 9.85, "Author");
        stackup.add_contributor(Contributor {
            name: "Hole position".into(),
            feature: None,
            direction: Direction::Positive,
            nominal: 10.0,
            plus_tol: 0.1,
            minus_tol: 0.1,
            distribution: Distribution::Normal,
            source: None,
            gdt_position: Some(GdtContribution::new(0.4)),
        });

        // Without GD&T inclusion: plain ±0.1.
        stackup.include_gdt = false;
        let wc = stackup.calculate_worst_case().unwrap();
        assert!((wc.min - 9.9).abs() < 1e-9);
        assert!((wc.max - 10.1).abs() < 1e-9);

        // With GD&T inclusion, each side widens by effective/2 = 0.2, the same
        // ±(effective/2) convention RSS and Monte Carlo use — worst case must
        // bound the statistical methods.
        stackup.include_gdt = true;
        let wc = stackup.calculate_worst_case().unwrap();
        assert!((wc.min - 9.7).abs() < 1e-9);
        assert!((wc.max - 10.3).abs() < 1e-9);
        assert_eq!(wc.result, AnalysisResult::Fail);
    }

    #[test]
    fn test_monte_carlo_rejects_too_few_iterations() {
        let mut stackup = Stackup::new("MC", "Gap", 1.0, 2.0, 0.0, "Author");
        stackup.add_contributor(Contributor {
            name: "Part".into(),
            feature: None,
            direction: Direction::Positive,
            nominal: 1.0,
            plus_tol: 0.1,
            minus_tol: 0.1,
            distribution: Distribution::Normal,
            source: None,
            gdt_position: None,
        });
        assert!(matches!(
            stackup.calculate_monte_carlo(0),
            Err(AnalysisError::TooFewIterations(0))
        ));
        assert!(matches!(
            stackup.calculate_monte_carlo(1),
            Err(AnalysisError::TooFewIterations(1))
        ));
        assert!(stackup.calculate_monte_carlo(2).is_ok());
    }
}
