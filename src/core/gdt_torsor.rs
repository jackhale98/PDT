//! GD&T to Torsor Bounds Conversion
//!
//! This module converts GD&T (Geometric Dimensioning and Tolerancing) controls
//! to torsor bounds for 3D tolerance analysis.
//!
//! The conversion follows ASME Y14.5 principles:
//! - Different GD&T symbols affect different DOFs (Degrees of Freedom)
//! - Geometry class determines which DOFs are relevant
//! - Material modifiers (MMC/LMC) can add bonus tolerance
//! - Datum references establish the reference frame

use crate::entities::feature::{
    Dimension, Feature, GdtControl, GdtSymbol, GeometryClass, Geometry3D, MaterialCondition,
    TorsorBounds,
};

/// Result of computing torsor bounds from GD&T
#[derive(Debug, Clone)]
pub struct GdtTorsorResult {
    /// Computed torsor bounds
    pub bounds: TorsorBounds,
    /// Warnings generated during computation
    pub warnings: Vec<String>,
    /// Whether the result includes bonus tolerance
    pub has_bonus: bool,
}

/// Compute torsor bounds from a feature's GD&T controls and geometry
///
/// # Arguments
/// * `feature` - The feature with GD&T controls and geometry info
/// * `actual_size` - Optional actual size for bonus tolerance calculation
///
/// # Returns
/// A `GdtTorsorResult` with computed bounds and any warnings
pub fn compute_torsor_bounds(feature: &Feature, actual_size: Option<f64>) -> GdtTorsorResult {
    let mut bounds = TorsorBounds::default();
    let mut warnings = Vec::new();
    let mut has_bonus = false;

    // Get geometry class, default to Complex (all DOFs active) if not specified
    let geometry_class = feature.geometry_class.unwrap_or(GeometryClass::Complex);

    // Get geometry 3D for computing angular bounds from linear tolerances
    let geometry_3d = feature.geometry_3d.as_ref();

    // Get primary dimension for MMC/LMC calculations
    let primary_dim = feature.primary_dimension();

    // Process each GD&T control and accumulate bounds
    for gdt in &feature.gdt {
        let gdt_bounds = compute_bounds_for_control(
            gdt,
            geometry_class,
            geometry_3d,
            primary_dim,
            actual_size,
        );

        // Merge bounds (take worst case - widest bounds)
        bounds = merge_bounds(bounds, gdt_bounds.bounds);

        if gdt_bounds.has_bonus {
            has_bonus = true;
        }

        warnings.extend(gdt_bounds.warnings);
    }

    // If no GD&T controls, try to compute bounds from dimensional tolerances
    if feature.gdt.is_empty() && !feature.dimensions.is_empty() {
        if let Some(dim) = primary_dim {
            let dim_bounds = compute_bounds_from_dimension(dim, geometry_class, geometry_3d);
            bounds = merge_bounds(bounds, dim_bounds);
            warnings.push("Torsor bounds computed from dimensional tolerance (no GD&T)".to_string());
        }
    }

    // Validate that we have bounds for expected DOFs based on geometry class
    let validation_warnings = validate_bounds_for_geometry(&bounds, geometry_class);
    warnings.extend(validation_warnings);

    GdtTorsorResult {
        bounds,
        warnings,
        has_bonus,
    }
}

/// Compute torsor bounds for a single GD&T control
fn compute_bounds_for_control(
    gdt: &GdtControl,
    geometry_class: GeometryClass,
    geometry_3d: Option<&Geometry3D>,
    primary_dim: Option<&Dimension>,
    actual_size: Option<f64>,
) -> GdtTorsorResult {
    let mut bounds = TorsorBounds::default();
    let mut warnings = Vec::new();
    let mut has_bonus = false;

    // Calculate effective tolerance (base + bonus if applicable)
    let effective_tol = if let (Some(dim), Some(actual)) = (primary_dim, actual_size) {
        match gdt.material_condition {
            MaterialCondition::Mmc => {
                let mmc = dim.mmc();
                let bonus = (actual - mmc).abs();
                if bonus > 0.0 {
                    has_bonus = true;
                }
                gdt.value + bonus
            }
            MaterialCondition::Lmc => {
                let lmc = dim.lmc();
                let bonus = (actual - lmc).abs();
                if bonus > 0.0 {
                    has_bonus = true;
                }
                gdt.value + bonus
            }
            MaterialCondition::Rfs => gdt.value,
        }
    } else {
        gdt.value
    };

    // Map GD&T symbol to affected DOFs based on geometry class
    match gdt.symbol {
        GdtSymbol::Position => {
            // Position affects translational DOFs based on geometry class
            match geometry_class {
                GeometryClass::Cylinder | GeometryClass::Cone => {
                    // Cylindrical position zone: u, v (radial position)
                    // Position tolerance is diameter, so radius = tol/2
                    let radial_bound = effective_tol / 2.0;
                    bounds.u = Some([-radial_bound, radial_bound]);
                    bounds.v = Some([-radial_bound, radial_bound]);
                }
                GeometryClass::Sphere | GeometryClass::Point => {
                    // Spherical position zone: u, v, w
                    let bound = effective_tol / 2.0;
                    bounds.u = Some([-bound, bound]);
                    bounds.v = Some([-bound, bound]);
                    bounds.w = Some([-bound, bound]);
                }
                GeometryClass::Plane => {
                    // Planar feature position: depends on datum setup
                    // For now, apply to u, v (in-plane)
                    let bound = effective_tol / 2.0;
                    bounds.u = Some([-bound, bound]);
                    bounds.v = Some([-bound, bound]);
                }
                GeometryClass::Line => {
                    // Line position: u, v (perpendicular to line)
                    let bound = effective_tol / 2.0;
                    bounds.u = Some([-bound, bound]);
                    bounds.v = Some([-bound, bound]);
                }
                GeometryClass::Complex => {
                    // Apply to all translational DOFs
                    let bound = effective_tol / 2.0;
                    bounds.u = Some([-bound, bound]);
                    bounds.v = Some([-bound, bound]);
                    bounds.w = Some([-bound, bound]);
                }
            }
        }

        GdtSymbol::Perpendicularity => {
            // Perpendicularity affects angular DOFs
            if let Some(geo) = geometry_3d {
                let length = geo.length.unwrap_or(10.0); // Default to 10mm if not specified
                // Angular deviation = tolerance / length (small angle approximation)
                let angular_bound = effective_tol / length;
                bounds.alpha = Some([-angular_bound, angular_bound]);
                bounds.beta = Some([-angular_bound, angular_bound]);
            } else {
                warnings.push(format!(
                    "Perpendicularity GD&T requires geometry_3d.length for angular bound calculation"
                ));
            }
        }

        GdtSymbol::Parallelism => {
            // Parallelism affects angular DOFs (same as perpendicularity calculation)
            if let Some(geo) = geometry_3d {
                let length = geo.length.unwrap_or(10.0);
                let angular_bound = effective_tol / length;
                bounds.alpha = Some([-angular_bound, angular_bound]);
                bounds.beta = Some([-angular_bound, angular_bound]);
            } else {
                warnings.push(format!(
                    "Parallelism GD&T requires geometry_3d.length for angular bound calculation"
                ));
            }
        }

        GdtSymbol::Angularity => {
            // Angularity affects angular DOFs
            if let Some(geo) = geometry_3d {
                let length = geo.length.unwrap_or(10.0);
                let angular_bound = effective_tol / length;
                bounds.alpha = Some([-angular_bound, angular_bound]);
                bounds.beta = Some([-angular_bound, angular_bound]);
            } else {
                warnings.push(format!(
                    "Angularity GD&T requires geometry_3d.length for angular bound calculation"
                ));
            }
        }

        GdtSymbol::Flatness => {
            // Flatness affects w (out-of-plane) for planar features
            let bound = effective_tol / 2.0;
            bounds.w = Some([-bound, bound]);
        }

        GdtSymbol::Concentricity => {
            // Concentricity affects radial position (u, v)
            let bound = effective_tol / 2.0;
            bounds.u = Some([-bound, bound]);
            bounds.v = Some([-bound, bound]);
        }

        GdtSymbol::Runout => {
            // Runout affects radial position and some angular
            let bound = effective_tol / 2.0;
            bounds.u = Some([-bound, bound]);
            bounds.v = Some([-bound, bound]);
            // Also affects angular DOFs for axial features
            if let Some(geo) = geometry_3d {
                let length = geo.length.unwrap_or(10.0);
                let angular_bound = effective_tol / length;
                bounds.alpha = Some([-angular_bound, angular_bound]);
                bounds.beta = Some([-angular_bound, angular_bound]);
            }
        }

        GdtSymbol::TotalRunout => {
            // Total runout is more comprehensive than runout
            let bound = effective_tol / 2.0;
            bounds.u = Some([-bound, bound]);
            bounds.v = Some([-bound, bound]);
            bounds.w = Some([-bound, bound]);
            if let Some(geo) = geometry_3d {
                let length = geo.length.unwrap_or(10.0);
                let angular_bound = effective_tol / length;
                bounds.alpha = Some([-angular_bound, angular_bound]);
                bounds.beta = Some([-angular_bound, angular_bound]);
            }
        }

        GdtSymbol::ProfileSurface => {
            // Profile of surface affects all relevant DOFs for the geometry
            let bound = effective_tol / 2.0;
            match geometry_class {
                GeometryClass::Plane => {
                    bounds.w = Some([-bound, bound]);
                }
                GeometryClass::Cylinder | GeometryClass::Cone => {
                    bounds.u = Some([-bound, bound]);
                    bounds.v = Some([-bound, bound]);
                }
                _ => {
                    bounds.u = Some([-bound, bound]);
                    bounds.v = Some([-bound, bound]);
                    bounds.w = Some([-bound, bound]);
                }
            }
        }

        GdtSymbol::ProfileLine => {
            // Profile of line - 2D cross-section deviation
            let bound = effective_tol / 2.0;
            bounds.u = Some([-bound, bound]);
            bounds.v = Some([-bound, bound]);
        }

        GdtSymbol::Circularity => {
            // Circularity affects radial uniformity (u, v)
            let bound = effective_tol / 2.0;
            bounds.u = Some([-bound, bound]);
            bounds.v = Some([-bound, bound]);
        }

        GdtSymbol::Cylindricity => {
            // Cylindricity affects radial uniformity and straightness
            let bound = effective_tol / 2.0;
            bounds.u = Some([-bound, bound]);
            bounds.v = Some([-bound, bound]);
            // Also affects angular for axial straightness
            if let Some(geo) = geometry_3d {
                let length = geo.length.unwrap_or(10.0);
                let angular_bound = effective_tol / length;
                bounds.alpha = Some([-angular_bound, angular_bound]);
                bounds.beta = Some([-angular_bound, angular_bound]);
            }
        }

        GdtSymbol::Straightness => {
            // Straightness depends on application
            match geometry_class {
                GeometryClass::Cylinder | GeometryClass::Line => {
                    // Straightness of axis
                    if let Some(geo) = geometry_3d {
                        let length = geo.length.unwrap_or(10.0);
                        let angular_bound = effective_tol / length;
                        bounds.alpha = Some([-angular_bound, angular_bound]);
                        bounds.beta = Some([-angular_bound, angular_bound]);
                    }
                }
                _ => {
                    // Straightness of surface elements
                    let bound = effective_tol / 2.0;
                    bounds.w = Some([-bound, bound]);
                }
            }
        }

        GdtSymbol::Symmetry => {
            // Symmetry affects centering (translation in one direction)
            let bound = effective_tol / 2.0;
            bounds.u = Some([-bound, bound]);
        }
    }

    GdtTorsorResult {
        bounds,
        warnings,
        has_bonus,
    }
}

/// Compute basic torsor bounds from a dimensional tolerance
fn compute_bounds_from_dimension(
    dim: &Dimension,
    geometry_class: GeometryClass,
    geometry_3d: Option<&Geometry3D>,
) -> TorsorBounds {
    let mut bounds = TorsorBounds::default();
    let half_band = (dim.plus_tol + dim.minus_tol) / 2.0;

    match geometry_class {
        GeometryClass::Cylinder | GeometryClass::Cone => {
            // Radial variation from diameter tolerance
            let radial_bound = half_band / 2.0; // Diameter -> radius
            bounds.u = Some([-radial_bound, radial_bound]);
            bounds.v = Some([-radial_bound, radial_bound]);
        }
        GeometryClass::Sphere | GeometryClass::Point => {
            let bound = half_band / 2.0;
            bounds.u = Some([-bound, bound]);
            bounds.v = Some([-bound, bound]);
            bounds.w = Some([-bound, bound]);
        }
        GeometryClass::Plane => {
            // Planar feature - affects w (thickness/position)
            bounds.w = Some([-half_band, half_band]);
        }
        GeometryClass::Line => {
            let bound = half_band / 2.0;
            bounds.u = Some([-bound, bound]);
            bounds.v = Some([-bound, bound]);
        }
        GeometryClass::Complex => {
            // Apply to length dimension along axis
            if let Some(_geo) = geometry_3d {
                bounds.w = Some([-half_band, half_band]);
            } else {
                // Default to all translational
                bounds.u = Some([-half_band, half_band]);
                bounds.v = Some([-half_band, half_band]);
                bounds.w = Some([-half_band, half_band]);
            }
        }
    }

    bounds
}

/// Merge two TorsorBounds, taking the wider bounds for each DOF
fn merge_bounds(a: TorsorBounds, b: TorsorBounds) -> TorsorBounds {
    TorsorBounds {
        u: merge_dof(a.u, b.u),
        v: merge_dof(a.v, b.v),
        w: merge_dof(a.w, b.w),
        alpha: merge_dof(a.alpha, b.alpha),
        beta: merge_dof(a.beta, b.beta),
        gamma: merge_dof(a.gamma, b.gamma),
    }
}

/// Merge two DOF bounds, taking the wider bounds
fn merge_dof(a: Option<[f64; 2]>, b: Option<[f64; 2]>) -> Option<[f64; 2]> {
    match (a, b) {
        (Some([a_min, a_max]), Some([b_min, b_max])) => {
            Some([a_min.min(b_min), a_max.max(b_max)])
        }
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

/// Validate that bounds cover expected DOFs for geometry class
fn validate_bounds_for_geometry(bounds: &TorsorBounds, geometry_class: GeometryClass) -> Vec<String> {
    let mut warnings = Vec::new();

    match geometry_class {
        GeometryClass::Cylinder => {
            if bounds.u.is_none() || bounds.v.is_none() {
                warnings.push("Cylinder feature missing radial (u, v) bounds".to_string());
            }
        }
        GeometryClass::Plane => {
            if bounds.w.is_none() && bounds.alpha.is_none() && bounds.beta.is_none() {
                warnings.push("Plane feature has no bounds - expected w, alpha, or beta".to_string());
            }
        }
        GeometryClass::Sphere | GeometryClass::Point => {
            if bounds.u.is_none() && bounds.v.is_none() && bounds.w.is_none() {
                warnings.push("Point/Sphere feature missing positional (u, v, w) bounds".to_string());
            }
        }
        _ => {}
    }

    warnings
}

/// Check if torsor bounds are approximately equal (within tolerance)
pub fn bounds_approx_equal(a: &TorsorBounds, b: &TorsorBounds, epsilon: f64) -> bool {
    dof_approx_equal(&a.u, &b.u, epsilon)
        && dof_approx_equal(&a.v, &b.v, epsilon)
        && dof_approx_equal(&a.w, &b.w, epsilon)
        && dof_approx_equal(&a.alpha, &b.alpha, epsilon)
        && dof_approx_equal(&a.beta, &b.beta, epsilon)
        && dof_approx_equal(&a.gamma, &b.gamma, epsilon)
}

fn dof_approx_equal(a: &Option<[f64; 2]>, b: &Option<[f64; 2]>, epsilon: f64) -> bool {
    match (a, b) {
        (Some([a_min, a_max]), Some([b_min, b_max])) => {
            (a_min - b_min).abs() < epsilon && (a_max - b_max).abs() < epsilon
        }
        (None, None) => true,
        _ => false,
    }
}

/// Compute stale bounds check - returns diff description if bounds are stale
pub fn check_stale_bounds(
    stored: &Option<TorsorBounds>,
    computed: &TorsorBounds,
    epsilon: f64,
) -> Option<String> {
    match stored {
        Some(stored_bounds) => {
            if !bounds_approx_equal(stored_bounds, computed, epsilon) {
                Some(format!(
                    "stored torsor_bounds differs from computed (use 'tdt feat compute-bounds' to update)"
                ))
            } else {
                None
            }
        }
        None => {
            // Check if computed has any bounds
            if computed.u.is_some()
                || computed.v.is_some()
                || computed.w.is_some()
                || computed.alpha.is_some()
                || computed.beta.is_some()
                || computed.gamma.is_some()
            {
                Some("torsor_bounds not set but can be computed from GD&T".to_string())
            } else {
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::feature::FeatureType;
    use crate::entities::stackup::Distribution;

    fn create_test_feature() -> Feature {
        Feature::new("CMP-TEST", FeatureType::Internal, "Test Feature", "Test Author")
    }

    // ===== Position Tolerance Tests =====

    #[test]
    fn test_position_cylinder_basic() {
        let mut feat = create_test_feature();
        feat.geometry_class = Some(GeometryClass::Cylinder);
        feat.gdt.push(GdtControl {
            symbol: GdtSymbol::Position,
            value: 0.25,
            units: "mm".to_string(),
            datum_refs: vec!["A".to_string(), "B".to_string()],
            material_condition: MaterialCondition::Rfs,
        });

        let result = compute_torsor_bounds(&feat, None);

        // Position 0.25 diameter -> ±0.125 radius
        assert!(result.bounds.u.is_some());
        assert!(result.bounds.v.is_some());
        let [u_min, u_max] = result.bounds.u.unwrap();
        assert!((u_min - (-0.125)).abs() < 1e-10, "u_min should be -0.125");
        assert!((u_max - 0.125).abs() < 1e-10, "u_max should be 0.125");
    }

    #[test]
    fn test_position_with_mmc_bonus() {
        let mut feat = create_test_feature();
        feat.geometry_class = Some(GeometryClass::Cylinder);
        // Hole: 10.0 +0.1/-0.0 -> MMC = 10.0
        feat.dimensions.push(Dimension {
            name: "diameter".to_string(),
            nominal: 10.0,
            plus_tol: 0.1,
            minus_tol: 0.0,
            units: "mm".to_string(),
            internal: true, // hole
            distribution: Distribution::Normal,
        });
        feat.gdt.push(GdtControl {
            symbol: GdtSymbol::Position,
            value: 0.25,
            units: "mm".to_string(),
            datum_refs: vec!["A".to_string()],
            material_condition: MaterialCondition::Mmc,
        });

        // At actual size 10.05 (departure from MMC = 0.05)
        // Effective position = 0.25 + 0.05 = 0.30 diameter
        let result = compute_torsor_bounds(&feat, Some(10.05));

        assert!(result.has_bonus);
        let [u_min, u_max] = result.bounds.u.unwrap();
        // 0.30 / 2 = 0.15 radius
        assert!((u_min - (-0.15)).abs() < 1e-10, "u_min should be -0.15, got {}", u_min);
        assert!((u_max - 0.15).abs() < 1e-10, "u_max should be 0.15, got {}", u_max);
    }

    #[test]
    fn test_position_sphere() {
        let mut feat = create_test_feature();
        feat.geometry_class = Some(GeometryClass::Sphere);
        feat.gdt.push(GdtControl {
            symbol: GdtSymbol::Position,
            value: 0.50,
            units: "mm".to_string(),
            datum_refs: vec!["A".to_string()],
            material_condition: MaterialCondition::Rfs,
        });

        let result = compute_torsor_bounds(&feat, None);

        // Sphere: u, v, w all constrained
        assert!(result.bounds.u.is_some());
        assert!(result.bounds.v.is_some());
        assert!(result.bounds.w.is_some());
        let [w_min, w_max] = result.bounds.w.unwrap();
        assert!((w_min - (-0.25)).abs() < 1e-10);
        assert!((w_max - 0.25).abs() < 1e-10);
    }

    // ===== Perpendicularity Tests =====

    #[test]
    fn test_perpendicularity_cylinder() {
        let mut feat = create_test_feature();
        feat.geometry_class = Some(GeometryClass::Cylinder);
        feat.geometry_3d = Some(Geometry3D {
            origin: [0.0, 0.0, 0.0],
            axis: [0.0, 0.0, 1.0],
            length: Some(20.0),
        });
        feat.gdt.push(GdtControl {
            symbol: GdtSymbol::Perpendicularity,
            value: 0.10,
            units: "mm".to_string(),
            datum_refs: vec!["A".to_string()],
            material_condition: MaterialCondition::Rfs,
        });

        let result = compute_torsor_bounds(&feat, None);

        // Angular deviation = 0.10 / 20.0 = 0.005 radians
        assert!(result.bounds.alpha.is_some());
        assert!(result.bounds.beta.is_some());
        let [alpha_min, alpha_max] = result.bounds.alpha.unwrap();
        assert!((alpha_min - (-0.005)).abs() < 1e-10);
        assert!((alpha_max - 0.005).abs() < 1e-10);
    }

    #[test]
    fn test_perpendicularity_without_length_warns() {
        let mut feat = create_test_feature();
        feat.geometry_class = Some(GeometryClass::Cylinder);
        // No geometry_3d
        feat.gdt.push(GdtControl {
            symbol: GdtSymbol::Perpendicularity,
            value: 0.10,
            units: "mm".to_string(),
            datum_refs: vec!["A".to_string()],
            material_condition: MaterialCondition::Rfs,
        });

        let result = compute_torsor_bounds(&feat, None);

        assert!(!result.warnings.is_empty());
        assert!(result.warnings.iter().any(|w| w.contains("geometry_3d.length")));
    }

    // ===== Flatness Tests =====

    #[test]
    fn test_flatness_plane() {
        let mut feat = create_test_feature();
        feat.geometry_class = Some(GeometryClass::Plane);
        feat.gdt.push(GdtControl {
            symbol: GdtSymbol::Flatness,
            value: 0.05,
            units: "mm".to_string(),
            datum_refs: vec![],
            material_condition: MaterialCondition::Rfs,
        });

        let result = compute_torsor_bounds(&feat, None);

        // Flatness affects w (out-of-plane)
        assert!(result.bounds.w.is_some());
        let [w_min, w_max] = result.bounds.w.unwrap();
        assert!((w_min - (-0.025)).abs() < 1e-10);
        assert!((w_max - 0.025).abs() < 1e-10);
    }

    // ===== Concentricity Tests =====

    #[test]
    fn test_concentricity_cylinder() {
        let mut feat = create_test_feature();
        feat.geometry_class = Some(GeometryClass::Cylinder);
        feat.gdt.push(GdtControl {
            symbol: GdtSymbol::Concentricity,
            value: 0.08,
            units: "mm".to_string(),
            datum_refs: vec!["A".to_string()],
            material_condition: MaterialCondition::Rfs,
        });

        let result = compute_torsor_bounds(&feat, None);

        // Concentricity affects u, v (radial offset)
        let [u_min, u_max] = result.bounds.u.unwrap();
        assert!((u_min - (-0.04)).abs() < 1e-10);
        assert!((u_max - 0.04).abs() < 1e-10);
    }

    // ===== Runout Tests =====

    #[test]
    fn test_runout_cylinder() {
        let mut feat = create_test_feature();
        feat.geometry_class = Some(GeometryClass::Cylinder);
        feat.geometry_3d = Some(Geometry3D {
            origin: [0.0, 0.0, 0.0],
            axis: [0.0, 0.0, 1.0],
            length: Some(50.0),
        });
        feat.gdt.push(GdtControl {
            symbol: GdtSymbol::Runout,
            value: 0.10,
            units: "mm".to_string(),
            datum_refs: vec!["A".to_string()],
            material_condition: MaterialCondition::Rfs,
        });

        let result = compute_torsor_bounds(&feat, None);

        // Runout affects u, v and angular
        assert!(result.bounds.u.is_some());
        assert!(result.bounds.v.is_some());
        assert!(result.bounds.alpha.is_some());
        assert!(result.bounds.beta.is_some());
    }

    // ===== Combined GD&T Tests =====

    #[test]
    fn test_multiple_gdt_controls() {
        let mut feat = create_test_feature();
        feat.geometry_class = Some(GeometryClass::Cylinder);
        feat.geometry_3d = Some(Geometry3D {
            origin: [50.0, 25.0, 0.0],
            axis: [0.0, 0.0, 1.0],
            length: Some(15.0),
        });
        feat.dimensions.push(Dimension {
            name: "diameter".to_string(),
            nominal: 10.0,
            plus_tol: 0.1,
            minus_tol: 0.05,
            units: "mm".to_string(),
            internal: true,
            distribution: Distribution::Normal,
        });

        // Position tolerance
        feat.gdt.push(GdtControl {
            symbol: GdtSymbol::Position,
            value: 0.25,
            units: "mm".to_string(),
            datum_refs: vec!["A".to_string(), "B".to_string(), "C".to_string()],
            material_condition: MaterialCondition::Mmc,
        });

        // Perpendicularity tolerance
        feat.gdt.push(GdtControl {
            symbol: GdtSymbol::Perpendicularity,
            value: 0.10,
            units: "mm".to_string(),
            datum_refs: vec!["A".to_string()],
            material_condition: MaterialCondition::Rfs,
        });

        let result = compute_torsor_bounds(&feat, None);

        // Should have both position (u, v) and perpendicularity (alpha, beta)
        assert!(result.bounds.u.is_some());
        assert!(result.bounds.v.is_some());
        assert!(result.bounds.alpha.is_some());
        assert!(result.bounds.beta.is_some());

        // Position: 0.25 / 2 = 0.125
        let [u_min, u_max] = result.bounds.u.unwrap();
        assert!((u_min - (-0.125)).abs() < 1e-10);
        assert!((u_max - 0.125).abs() < 1e-10);

        // Perpendicularity: 0.10 / 15.0 ≈ 0.00667
        let [alpha_min, alpha_max] = result.bounds.alpha.unwrap();
        assert!((alpha_min - (-0.10 / 15.0)).abs() < 1e-10);
        assert!((alpha_max - (0.10 / 15.0)).abs() < 1e-10);
    }

    // ===== Dimension-Only Tests =====

    #[test]
    fn test_bounds_from_dimension_only() {
        let mut feat = create_test_feature();
        feat.geometry_class = Some(GeometryClass::Cylinder);
        feat.dimensions.push(Dimension {
            name: "diameter".to_string(),
            nominal: 10.0,
            plus_tol: 0.1,
            minus_tol: 0.1,
            units: "mm".to_string(),
            internal: true,
            distribution: Distribution::Normal,
        });
        // No GD&T controls

        let result = compute_torsor_bounds(&feat, None);

        // Should compute from dimensional tolerance
        assert!(result.bounds.u.is_some());
        assert!(result.warnings.iter().any(|w| w.contains("no GD&T")));

        // Diameter tolerance 0.2 total -> radius variation 0.1 / 2 = 0.05
        let [u_min, u_max] = result.bounds.u.unwrap();
        assert!((u_min - (-0.05)).abs() < 1e-10);
        assert!((u_max - 0.05).abs() < 1e-10);
    }

    // ===== Bounds Comparison Tests =====

    #[test]
    fn test_bounds_approx_equal() {
        let a = TorsorBounds {
            u: Some([-0.125, 0.125]),
            v: Some([-0.125, 0.125]),
            ..Default::default()
        };
        let b = TorsorBounds {
            u: Some([-0.125, 0.125]),
            v: Some([-0.125, 0.125]),
            ..Default::default()
        };

        assert!(bounds_approx_equal(&a, &b, 1e-10));
    }

    #[test]
    fn test_bounds_not_equal() {
        let a = TorsorBounds {
            u: Some([-0.125, 0.125]),
            ..Default::default()
        };
        let b = TorsorBounds {
            u: Some([-0.15, 0.15]), // Different
            ..Default::default()
        };

        assert!(!bounds_approx_equal(&a, &b, 1e-10));
    }

    #[test]
    fn test_check_stale_bounds_stale() {
        let stored = Some(TorsorBounds {
            u: Some([-0.1, 0.1]),
            ..Default::default()
        });
        let computed = TorsorBounds {
            u: Some([-0.125, 0.125]), // Different
            ..Default::default()
        };

        let result = check_stale_bounds(&stored, &computed, 1e-10);
        assert!(result.is_some());
        assert!(result.unwrap().contains("differs"));
    }

    #[test]
    fn test_check_stale_bounds_missing() {
        let stored = None;
        let computed = TorsorBounds {
            u: Some([-0.125, 0.125]),
            ..Default::default()
        };

        let result = check_stale_bounds(&stored, &computed, 1e-10);
        assert!(result.is_some());
        assert!(result.unwrap().contains("not set"));
    }

    // ===== Merge Bounds Tests =====

    #[test]
    fn test_merge_bounds_takes_wider() {
        let a = TorsorBounds {
            u: Some([-0.1, 0.1]),
            v: Some([-0.05, 0.05]),
            ..Default::default()
        };
        let b = TorsorBounds {
            u: Some([-0.05, 0.15]), // Asymmetric, wider on positive side
            w: Some([-0.02, 0.02]),
            ..Default::default()
        };

        let merged = merge_bounds(a, b);

        // u: takes min(-0.1, -0.05) = -0.1, max(0.1, 0.15) = 0.15
        let [u_min, u_max] = merged.u.unwrap();
        assert!((u_min - (-0.1)).abs() < 1e-10);
        assert!((u_max - 0.15).abs() < 1e-10);

        // v: only from a
        assert!(merged.v.is_some());

        // w: only from b
        assert!(merged.w.is_some());
    }

    // ===== Validation Tests =====

    #[test]
    fn test_validate_cylinder_missing_radial() {
        let bounds = TorsorBounds {
            w: Some([-0.1, 0.1]), // Only w, missing u, v
            ..Default::default()
        };

        let warnings = validate_bounds_for_geometry(&bounds, GeometryClass::Cylinder);
        assert!(!warnings.is_empty());
        assert!(warnings.iter().any(|w| w.contains("radial")));
    }

    #[test]
    fn test_validate_plane_ok() {
        let bounds = TorsorBounds {
            w: Some([-0.05, 0.05]),
            alpha: Some([-0.001, 0.001]),
            beta: Some([-0.001, 0.001]),
            ..Default::default()
        };

        let warnings = validate_bounds_for_geometry(&bounds, GeometryClass::Plane);
        assert!(warnings.is_empty());
    }
}
