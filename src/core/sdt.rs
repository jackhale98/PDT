//! Small Displacement Torsor (SDT) Engine for 3D Tolerance Analysis
//!
//! This module implements torsor-based 3D tolerance analysis using the Jacobian method.
//! Key concepts:
//! - Torsor: 6-DOF deviation vector [u, v, w, α, β, γ] (3 translations + 3 rotations)
//! - Jacobian: 6×6 transformation matrix for propagating torsors through kinematic chains
//! - Invariance class: DOF constraints based on feature geometry type

use nalgebra::{Matrix6, Vector6};
use rand::Rng;

use crate::entities::feature::{GeometryClass, TorsorBounds};
use crate::entities::stackup::{Distribution, ResultTorsor, TorsorStats};

/// A 6-DOF deviation torsor: [u, v, w, α, β, γ]
/// - u, v, w: translational deviations (mm)
/// - α, β, γ: rotational deviations (radians)
pub type Torsor = Vector6<f64>;

/// 6×6 Jacobian matrix for torsor transformation
pub type JacobianMatrix = Matrix6<f64>;

/// DOF indices for clarity
pub const DOF_U: usize = 0;
pub const DOF_V: usize = 1;
pub const DOF_W: usize = 2;
pub const DOF_ALPHA: usize = 3;
pub const DOF_BETA: usize = 4;
pub const DOF_GAMMA: usize = 5;

/// DOF names for display
pub const DOF_NAMES: [&str; 6] = ["u", "v", "w", "α", "β", "γ"];

/// Get the constrained DOFs for a geometry class (invariance class)
///
/// Returns indices of DOFs that are constrained by the feature type.
/// Based on TTRS (Technologically and Topologically Related Surfaces) theory.
pub fn get_constrained_dof(geometry_class: GeometryClass) -> Vec<usize> {
    match geometry_class {
        // Plane: constrains w (normal translation), α, β (tilts)
        GeometryClass::Plane => vec![DOF_W, DOF_ALPHA, DOF_BETA],
        // Cylinder: constrains u, v (radial), α, β (tilts)
        GeometryClass::Cylinder => vec![DOF_U, DOF_V, DOF_ALPHA, DOF_BETA],
        // Sphere: constrains u, v, w (all translations)
        GeometryClass::Sphere => vec![DOF_U, DOF_V, DOF_W],
        // Cone: constrains u, v, w (apex position), α, β (tilts)
        GeometryClass::Cone => vec![DOF_U, DOF_V, DOF_W, DOF_ALPHA, DOF_BETA],
        // Point: constrains u, v, w (position only)
        GeometryClass::Point => vec![DOF_U, DOF_V, DOF_W],
        // Line: constrains u, v (perpendicular translations)
        GeometryClass::Line => vec![DOF_U, DOF_V],
        // Complex: no default constraints (user-defined)
        GeometryClass::Complex => vec![],
    }
}

/// Get the free (unconstrained) DOFs for a geometry class
pub fn get_free_dof(geometry_class: GeometryClass) -> Vec<usize> {
    let constrained = get_constrained_dof(geometry_class);
    (0..6).filter(|dof| !constrained.contains(dof)).collect()
}

/// Build a Jacobian matrix for a contributor at position r
///
/// The Jacobian transforms a local torsor to its contribution at the assembly origin.
/// For a feature at position r = [rx, ry, rz]:
///
/// ```text
/// J = | I₃   [r]× |
///     | 0₃    I₃  |
/// ```
///
/// where [r]× is the skew-symmetric cross-product matrix:
/// ```text
/// [r]× = |  0   -rz   ry |
///        |  rz   0   -rx |
///        | -ry   rx   0  |
/// ```
pub fn build_jacobian(position: [f64; 3]) -> JacobianMatrix {
    let [rx, ry, rz] = position;

    // Start with identity
    let mut j = Matrix6::identity();

    // Add skew-symmetric contribution to upper-right 3×3 block
    // This captures the effect of rotations producing translations at a distance
    // J[0,4] = rz  (rotation about Y produces translation in X at distance rz)
    // J[0,5] = -ry (rotation about Z produces translation in X at distance -ry)
    // etc.
    j[(0, 4)] = rz;
    j[(0, 5)] = -ry;
    j[(1, 3)] = -rz;
    j[(1, 5)] = rx;
    j[(2, 3)] = ry;
    j[(2, 4)] = -rx;

    j
}

/// Build a Jacobian that projects result onto a functional direction
///
/// Returns a 1×6 row vector that extracts the deviation along the functional direction.
/// The translation components [u,v,w] are projected onto the direction vector.
pub fn build_projection_jacobian(functional_direction: [f64; 3]) -> Vector6<f64> {
    let [dx, dy, dz] = functional_direction;
    // Normalize the direction
    let len = (dx * dx + dy * dy + dz * dz).sqrt();
    let (dx, dy, dz) = if len > 1e-10 {
        (dx / len, dy / len, dz / len)
    } else {
        (1.0, 0.0, 0.0) // Default to X if zero vector
    };

    Vector6::new(dx, dy, dz, 0.0, 0.0, 0.0)
}

/// Contributor data for 3D chain analysis
#[derive(Debug, Clone)]
pub struct ChainContributor3D {
    /// Contributor name
    pub name: String,

    /// Feature ID if linked
    pub feature_id: Option<String>,

    /// Geometry class (determines invariance)
    pub geometry_class: GeometryClass,

    /// Position in assembly coordinates
    pub position: [f64; 3],

    /// Torsor bounds from tolerances
    pub bounds: TorsorBounds,

    /// Distribution type for Monte Carlo
    pub distribution: Distribution,

    /// Sigma level for variance calculation
    pub sigma_level: f64,
}

/// Result of 3D chain propagation
#[derive(Debug, Clone)]
pub struct Chain3DResult {
    /// Worst-case torsor bounds at result
    pub wc_bounds: TorsorBounds,

    /// RSS (statistical) result torsor stats
    pub rss_stats: ResultTorsor,

    /// Monte Carlo result (if run)
    pub mc_stats: Option<ResultTorsor>,

    /// Variance contribution per contributor per DOF
    pub sensitivity: Vec<[f64; 6]>,
}

/// Get bounds value as [min, max], defaulting to [0, 0] for free DOF
fn bounds_or_zero(bounds: &Option<[f64; 2]>) -> [f64; 2] {
    bounds.unwrap_or([0.0, 0.0])
}

/// Propagate torsors through chain using worst-case analysis
///
/// For each DOF j in the result:
/// ```text
/// result_min[j] = Σ min(J[j,k] * bounds[k].min, J[j,k] * bounds[k].max)
/// result_max[j] = Σ max(J[j,k] * bounds[k].min, J[j,k] * bounds[k].max)
/// ```
pub fn propagate_worst_case(contributors: &[ChainContributor3D]) -> TorsorBounds {
    let mut result_min = [0.0f64; 6];
    let mut result_max = [0.0f64; 6];

    for contrib in contributors {
        let j = build_jacobian(contrib.position);
        let bounds_array = [
            bounds_or_zero(&contrib.bounds.u),
            bounds_or_zero(&contrib.bounds.v),
            bounds_or_zero(&contrib.bounds.w),
            bounds_or_zero(&contrib.bounds.alpha),
            bounds_or_zero(&contrib.bounds.beta),
            bounds_or_zero(&contrib.bounds.gamma),
        ];

        // For each output DOF
        for out_dof in 0..6 {
            // Sum contributions from all input DOFs
            for in_dof in 0..6 {
                let j_val = j[(out_dof, in_dof)];
                let [b_min, b_max] = bounds_array[in_dof];

                // Worst case: take min/max considering sign of Jacobian element
                let contrib_1 = j_val * b_min;
                let contrib_2 = j_val * b_max;

                result_min[out_dof] += contrib_1.min(contrib_2);
                result_max[out_dof] += contrib_1.max(contrib_2);
            }
        }
    }

    TorsorBounds {
        u: Some([result_min[0], result_max[0]]),
        v: Some([result_min[1], result_max[1]]),
        w: Some([result_min[2], result_max[2]]),
        alpha: Some([result_min[3], result_max[3]]),
        beta: Some([result_min[4], result_max[4]]),
        gamma: Some([result_min[5], result_max[5]]),
    }
}

/// Propagate torsors through chain using RSS (Root Sum Square) method
///
/// For each DOF j:
/// ```text
/// mean[j] = Σ J[j,k] * mean[k]
/// σ²[j] = Σ J[j,k]² * σ²[k]
/// ```
/// where σ[k] = (bounds[k].max - bounds[k].min) / sigma_level
pub fn propagate_rss(contributors: &[ChainContributor3D]) -> (ResultTorsor, Vec<[f64; 6]>) {
    let mut mean = [0.0f64; 6];
    let mut variance = [0.0f64; 6];
    let mut individual_variances: Vec<[f64; 6]> = Vec::with_capacity(contributors.len());

    for contrib in contributors {
        let j = build_jacobian(contrib.position);
        let bounds_array = [
            bounds_or_zero(&contrib.bounds.u),
            bounds_or_zero(&contrib.bounds.v),
            bounds_or_zero(&contrib.bounds.w),
            bounds_or_zero(&contrib.bounds.alpha),
            bounds_or_zero(&contrib.bounds.beta),
            bounds_or_zero(&contrib.bounds.gamma),
        ];

        let mut contrib_variance = [0.0f64; 6];

        for out_dof in 0..6 {
            for in_dof in 0..6 {
                let j_val = j[(out_dof, in_dof)];
                let [b_min, b_max] = bounds_array[in_dof];

                // Mean is center of bounds
                let b_mean = (b_min + b_max) / 2.0;
                mean[out_dof] += j_val * b_mean;

                // Variance: σ = range / sigma_level, then J² * σ²
                let range = b_max - b_min;
                let sigma = range / contrib.sigma_level;
                let var_contrib = j_val * j_val * sigma * sigma;
                variance[out_dof] += var_contrib;
                contrib_variance[out_dof] += var_contrib;
            }
        }

        individual_variances.push(contrib_variance);
    }

    // Convert variance to standard deviation and 3-sigma
    let result = ResultTorsor {
        u: TorsorStats {
            wc_min: 0.0,
            wc_max: 0.0,
            rss_mean: mean[0],
            rss_3sigma: 3.0 * variance[0].sqrt(),
            mc_mean: None,
            mc_std_dev: None,
        },
        v: TorsorStats {
            wc_min: 0.0,
            wc_max: 0.0,
            rss_mean: mean[1],
            rss_3sigma: 3.0 * variance[1].sqrt(),
            mc_mean: None,
            mc_std_dev: None,
        },
        w: TorsorStats {
            wc_min: 0.0,
            wc_max: 0.0,
            rss_mean: mean[2],
            rss_3sigma: 3.0 * variance[2].sqrt(),
            mc_mean: None,
            mc_std_dev: None,
        },
        alpha: TorsorStats {
            wc_min: 0.0,
            wc_max: 0.0,
            rss_mean: mean[3],
            rss_3sigma: 3.0 * variance[3].sqrt(),
            mc_mean: None,
            mc_std_dev: None,
        },
        beta: TorsorStats {
            wc_min: 0.0,
            wc_max: 0.0,
            rss_mean: mean[4],
            rss_3sigma: 3.0 * variance[4].sqrt(),
            mc_mean: None,
            mc_std_dev: None,
        },
        gamma: TorsorStats {
            wc_min: 0.0,
            wc_max: 0.0,
            rss_mean: mean[5],
            rss_3sigma: 3.0 * variance[5].sqrt(),
            mc_mean: None,
            mc_std_dev: None,
        },
    };

    // Calculate sensitivity (variance contribution percentage)
    let total_variance = variance;
    let sensitivity: Vec<[f64; 6]> = individual_variances
        .iter()
        .map(|iv| {
            let mut pct = [0.0f64; 6];
            for dof in 0..6 {
                pct[dof] = if total_variance[dof] > 0.0 {
                    (iv[dof] / total_variance[dof]) * 100.0
                } else {
                    0.0
                };
            }
            pct
        })
        .collect();

    (result, sensitivity)
}

/// Sample a torsor based on distribution type
fn sample_torsor<R: Rng>(
    bounds: &TorsorBounds,
    distribution: Distribution,
    sigma_level: f64,
    rng: &mut R,
) -> Torsor {
    let bounds_array = [
        bounds_or_zero(&bounds.u),
        bounds_or_zero(&bounds.v),
        bounds_or_zero(&bounds.w),
        bounds_or_zero(&bounds.alpha),
        bounds_or_zero(&bounds.beta),
        bounds_or_zero(&bounds.gamma),
    ];

    let mut result = Torsor::zeros();

    for (dof, [b_min, b_max]) in bounds_array.iter().enumerate() {
        let range = b_max - b_min;
        let center = (b_min + b_max) / 2.0;

        result[dof] = match distribution {
            Distribution::Normal => {
                // Box-Muller transform
                let sigma = range / sigma_level;
                let u1: f64 = rng.random();
                let u2: f64 = rng.random();
                let z = (-2.0_f64 * u1.ln()).sqrt() * (2.0_f64 * std::f64::consts::PI * u2).cos();
                center + sigma * z
            }
            Distribution::Uniform => {
                let half_range = range / 2.0;
                rng.random_range((center - half_range)..=(center + half_range))
            }
            Distribution::Triangular => {
                let min = *b_min;
                let max = *b_max;
                let u: f64 = rng.random();
                let fc = (center - min) / (max - min);
                if u < fc {
                    min + (u * (max - min) * (center - min)).sqrt()
                } else {
                    max - ((1.0 - u) * (max - min) * (max - center)).sqrt()
                }
            }
        };
    }

    result
}

/// Run Monte Carlo 3D simulation
pub fn monte_carlo_3d(contributors: &[ChainContributor3D], iterations: u32) -> ResultTorsor {
    let mut rng = rand::rng();

    // Collect samples for each DOF
    let mut samples: [Vec<f64>; 6] = Default::default();
    for s in &mut samples {
        s.reserve(iterations as usize);
    }

    for _ in 0..iterations {
        let mut result_torsor = Torsor::zeros();

        for contrib in contributors {
            let j = build_jacobian(contrib.position);
            let sample = sample_torsor(
                &contrib.bounds,
                contrib.distribution,
                contrib.sigma_level,
                &mut rng,
            );

            // Transform through Jacobian
            result_torsor += j * sample;
        }

        for dof in 0..6 {
            samples[dof].push(result_torsor[dof]);
        }
    }

    // Calculate statistics for each DOF
    fn calc_stats(samples: &[f64]) -> (f64, f64) {
        let n = samples.len() as f64;
        let mean: f64 = samples.iter().sum::<f64>() / n;
        let variance: f64 = samples.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n;
        (mean, variance.sqrt())
    }

    let (u_mean, u_std) = calc_stats(&samples[0]);
    let (v_mean, v_std) = calc_stats(&samples[1]);
    let (w_mean, w_std) = calc_stats(&samples[2]);
    let (alpha_mean, alpha_std) = calc_stats(&samples[3]);
    let (beta_mean, beta_std) = calc_stats(&samples[4]);
    let (gamma_mean, gamma_std) = calc_stats(&samples[5]);

    ResultTorsor {
        u: TorsorStats {
            wc_min: 0.0,
            wc_max: 0.0,
            rss_mean: 0.0,
            rss_3sigma: 0.0,
            mc_mean: Some(u_mean),
            mc_std_dev: Some(u_std),
        },
        v: TorsorStats {
            wc_min: 0.0,
            wc_max: 0.0,
            rss_mean: 0.0,
            rss_3sigma: 0.0,
            mc_mean: Some(v_mean),
            mc_std_dev: Some(v_std),
        },
        w: TorsorStats {
            wc_min: 0.0,
            wc_max: 0.0,
            rss_mean: 0.0,
            rss_3sigma: 0.0,
            mc_mean: Some(w_mean),
            mc_std_dev: Some(w_std),
        },
        alpha: TorsorStats {
            wc_min: 0.0,
            wc_max: 0.0,
            rss_mean: 0.0,
            rss_3sigma: 0.0,
            mc_mean: Some(alpha_mean),
            mc_std_dev: Some(alpha_std),
        },
        beta: TorsorStats {
            wc_min: 0.0,
            wc_max: 0.0,
            rss_mean: 0.0,
            rss_3sigma: 0.0,
            mc_mean: Some(beta_mean),
            mc_std_dev: Some(beta_std),
        },
        gamma: TorsorStats {
            wc_min: 0.0,
            wc_max: 0.0,
            rss_mean: 0.0,
            rss_3sigma: 0.0,
            mc_mean: Some(gamma_mean),
            mc_std_dev: Some(gamma_std),
        },
    }
}

/// Merge worst-case bounds into a ResultTorsor
pub fn merge_wc_into_result(result: &mut ResultTorsor, wc: &TorsorBounds) {
    if let Some([min, max]) = wc.u {
        result.u.wc_min = min;
        result.u.wc_max = max;
    }
    if let Some([min, max]) = wc.v {
        result.v.wc_min = min;
        result.v.wc_max = max;
    }
    if let Some([min, max]) = wc.w {
        result.w.wc_min = min;
        result.w.wc_max = max;
    }
    if let Some([min, max]) = wc.alpha {
        result.alpha.wc_min = min;
        result.alpha.wc_max = max;
    }
    if let Some([min, max]) = wc.beta {
        result.beta.wc_min = min;
        result.beta.wc_max = max;
    }
    if let Some([min, max]) = wc.gamma {
        result.gamma.wc_min = min;
        result.gamma.wc_max = max;
    }
}

/// Merge Monte Carlo stats into a ResultTorsor
pub fn merge_mc_into_result(result: &mut ResultTorsor, mc: &ResultTorsor) {
    result.u.mc_mean = mc.u.mc_mean;
    result.u.mc_std_dev = mc.u.mc_std_dev;
    result.v.mc_mean = mc.v.mc_mean;
    result.v.mc_std_dev = mc.v.mc_std_dev;
    result.w.mc_mean = mc.w.mc_mean;
    result.w.mc_std_dev = mc.w.mc_std_dev;
    result.alpha.mc_mean = mc.alpha.mc_mean;
    result.alpha.mc_std_dev = mc.alpha.mc_std_dev;
    result.beta.mc_mean = mc.beta.mc_mean;
    result.beta.mc_std_dev = mc.beta.mc_std_dev;
    result.gamma.mc_mean = mc.gamma.mc_mean;
    result.gamma.mc_std_dev = mc.gamma.mc_std_dev;
}

/// Run full 3D analysis on a chain of contributors
pub fn analyze_chain_3d(
    contributors: &[ChainContributor3D],
    run_monte_carlo: bool,
    monte_carlo_iterations: u32,
) -> Chain3DResult {
    // Worst-case analysis
    let wc_bounds = propagate_worst_case(contributors);

    // RSS analysis with sensitivity
    let (mut rss_stats, sensitivity) = propagate_rss(contributors);

    // Merge worst-case into RSS stats
    merge_wc_into_result(&mut rss_stats, &wc_bounds);

    // Optional Monte Carlo
    let mc_stats = if run_monte_carlo && !contributors.is_empty() {
        let mc = monte_carlo_3d(contributors, monte_carlo_iterations);
        merge_mc_into_result(&mut rss_stats, &mc);
        Some(mc)
    } else {
        None
    };

    Chain3DResult {
        wc_bounds,
        rss_stats,
        mc_stats,
        sensitivity,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constrained_dof_plane() {
        let dof = get_constrained_dof(GeometryClass::Plane);
        assert!(dof.contains(&DOF_W));
        assert!(dof.contains(&DOF_ALPHA));
        assert!(dof.contains(&DOF_BETA));
        assert_eq!(dof.len(), 3);
    }

    #[test]
    fn test_constrained_dof_cylinder() {
        let dof = get_constrained_dof(GeometryClass::Cylinder);
        assert!(dof.contains(&DOF_U));
        assert!(dof.contains(&DOF_V));
        assert!(dof.contains(&DOF_ALPHA));
        assert!(dof.contains(&DOF_BETA));
        assert_eq!(dof.len(), 4);
    }

    #[test]
    fn test_jacobian_at_origin() {
        // At origin, Jacobian should be identity
        let j = build_jacobian([0.0, 0.0, 0.0]);
        assert!((j - Matrix6::identity()).norm() < 1e-10);
    }

    #[test]
    fn test_jacobian_with_offset() {
        // Test that rotation about Y at position [10, 0, 0] produces translation in Z
        let j = build_jacobian([10.0, 0.0, 0.0]);

        // J[2,4] should be -rx = -10 (rotation about Y produces -Z translation at X offset)
        assert!((j[(2, 4)] - (-10.0)).abs() < 1e-10);

        // J[1,5] should be rx = 10 (rotation about Z produces Y translation at X offset)
        assert!((j[(1, 5)] - 10.0).abs() < 1e-10);
    }

    #[test]
    fn test_propagate_worst_case_single() {
        let contrib = ChainContributor3D {
            name: "Test".to_string(),
            feature_id: None,
            geometry_class: GeometryClass::Plane,
            position: [0.0, 0.0, 0.0],
            bounds: TorsorBounds {
                u: Some([-0.1, 0.1]),
                v: Some([-0.1, 0.1]),
                w: Some([-0.05, 0.05]),
                alpha: None,
                beta: None,
                gamma: None,
            },
            distribution: Distribution::Normal,
            sigma_level: 6.0,
        };

        let result = propagate_worst_case(&[contrib]);

        // At origin with identity Jacobian, result should match input
        assert!((result.u.unwrap()[0] - (-0.1)).abs() < 1e-10);
        assert!((result.u.unwrap()[1] - 0.1).abs() < 1e-10);
    }

    #[test]
    fn test_propagate_rss() {
        let contrib = ChainContributor3D {
            name: "Test".to_string(),
            feature_id: None,
            geometry_class: GeometryClass::Plane,
            position: [0.0, 0.0, 0.0],
            bounds: TorsorBounds {
                u: Some([-0.1, 0.1]),
                v: None,
                w: None,
                alpha: None,
                beta: None,
                gamma: None,
            },
            distribution: Distribution::Normal,
            sigma_level: 6.0,
        };

        let (result, sensitivity) = propagate_rss(&[contrib]);

        // Mean should be 0 for symmetric bounds
        assert!(result.u.rss_mean.abs() < 1e-10);

        // σ = 0.2/6, 3σ = 0.1
        assert!((result.u.rss_3sigma - 0.1).abs() < 1e-10);

        // Single contributor should have 100% sensitivity
        assert!((sensitivity[0][0] - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_free_dof() {
        let free = get_free_dof(GeometryClass::Plane);
        // Plane constrains w, α, β, so u, v, γ are free
        assert!(free.contains(&DOF_U));
        assert!(free.contains(&DOF_V));
        assert!(free.contains(&DOF_GAMMA));
        assert_eq!(free.len(), 3);
    }

    #[test]
    fn test_projection_jacobian() {
        // X direction
        let proj = build_projection_jacobian([1.0, 0.0, 0.0]);
        assert!((proj[0] - 1.0).abs() < 1e-10);
        assert!(proj[1].abs() < 1e-10);
        assert!(proj[2].abs() < 1e-10);

        // Z direction
        let proj = build_projection_jacobian([0.0, 0.0, 1.0]);
        assert!(proj[0].abs() < 1e-10);
        assert!(proj[1].abs() < 1e-10);
        assert!((proj[2] - 1.0).abs() < 1e-10);

        // 45 degree in XY
        let proj = build_projection_jacobian([1.0, 1.0, 0.0]);
        let expected = 1.0 / 2.0_f64.sqrt();
        assert!((proj[0] - expected).abs() < 1e-10);
        assert!((proj[1] - expected).abs() < 1e-10);
    }
}
