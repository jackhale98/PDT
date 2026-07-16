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
    Dimension, DimensionRef, Feature, GdtControl, GdtSymbol, Geometry3D, GeometryClass,
    MaterialCondition, TorsorBounds,
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

/// Resolved length with tolerance information for angular bound calculation
///
/// When computing angular bounds from linear tolerances, the length of the feature
/// affects the result. If the length itself has a tolerance, this affects the
/// angular bounds:
/// - Shorter length = larger angular deviation for same linear tolerance
/// - Longer length = smaller angular deviation for same linear tolerance
#[derive(Debug, Clone)]
pub struct ResolvedLength {
    /// Nominal length value (mm)
    pub nominal: f64,
    /// Plus tolerance (positive value, mm)
    pub plus_tol: f64,
    /// Minus tolerance (positive value, mm)
    pub minus_tol: f64,
    /// Source of the length (for diagnostics), e.g., "FEAT-xxx:depth" or "fixed"
    pub source: String,
}

impl ResolvedLength {
    /// Create a ResolvedLength from a fixed nominal value (no tolerance)
    pub fn fixed(nominal: f64) -> Self {
        Self {
            nominal,
            plus_tol: 0.0,
            minus_tol: 0.0,
            source: "fixed".to_string(),
        }
    }

    /// Get minimum length (nominal - minus_tol)
    /// This produces the maximum angular deviation
    pub fn min_length(&self) -> f64 {
        (self.nominal - self.minus_tol).max(0.001) // Avoid division by zero
    }

    /// Get maximum length (nominal + plus_tol)
    /// This produces the minimum angular deviation
    pub fn max_length(&self) -> f64 {
        self.nominal + self.plus_tol
    }

    /// Check if this length has any tolerance variation
    pub fn has_tolerance(&self) -> bool {
        self.plus_tol > 0.0 || self.minus_tol > 0.0
    }

    /// Get variance for RSS calculation (sigma squared)
    /// Assumes tolerances represent ±3σ (or specified sigma_level)
    pub fn variance(&self, sigma_level: f64) -> f64 {
        let sigma = (self.plus_tol + self.minus_tol) / sigma_level;
        sigma * sigma
    }
}

/// Resolve length with tolerance from geometry_3d, optionally following length_ref
///
/// This function resolves the length for angular bound calculations by:
/// 1. Trying to follow length_ref if present and feature_lookup is provided
/// 2. Falling back to the cached length value (nominal only)
/// 3. Using a default value if nothing is available
///
/// # Arguments
/// * `geometry_3d` - The geometry definition with length/length_ref
/// * `feature_lookup` - Optional function to look up features by ID for length_ref resolution
///
/// # Returns
/// A ResolvedLength with nominal and tolerance values
pub fn resolve_length_with_tolerance<F>(
    geometry_3d: &Geometry3D,
    feature_lookup: Option<&F>,
) -> ResolvedLength
where
    F: Fn(&str) -> Option<Feature>,
{
    // First try length_ref if available and we have a lookup function
    if let Some(ref length_ref_str) = geometry_3d.length_ref {
        if let Some(dim_ref) = DimensionRef::parse(length_ref_str) {
            if let Some(lookup) = feature_lookup {
                if let Some(target_feat) = lookup(&dim_ref.feature_id) {
                    // Find the dimension by name
                    if let Some(dim) = target_feat
                        .dimensions
                        .iter()
                        .find(|d| d.name == dim_ref.dimension_name)
                    {
                        return ResolvedLength {
                            nominal: dim.nominal,
                            plus_tol: dim.plus_tol,
                            minus_tol: dim.minus_tol,
                            source: length_ref_str.clone(),
                        };
                    }
                }
            }
        }
    }

    // Fall back to cached length (no tolerance info available)
    let nominal = geometry_3d.length.unwrap_or(10.0); // Default 10mm if not specified
    ResolvedLength::fixed(nominal)
}

/// Compute worst-case angular bound considering length tolerance
///
/// For a linear tolerance `t` applied over a length `L` with tolerance:
/// - Max angular deviation = t / L_min (at minimum length)
/// - Min angular deviation = t / L_max (at maximum length)
///
/// Returns the larger (more conservative) bound
fn compute_angular_bound_with_length_tolerance(
    linear_tolerance: f64,
    length: &ResolvedLength,
) -> f64 {
    let angular_at_min_length = linear_tolerance / length.min_length();
    let angular_at_max_length = linear_tolerance / length.max_length();
    angular_at_min_length.max(angular_at_max_length)
}

/// Compute torsor bounds from a feature's GD&T controls and geometry
///
/// # Arguments
/// * `feature` - The feature with GD&T controls and geometry info
/// * `actual_size` - Optional actual size for bonus tolerance calculation
/// * `feature_lookup` - Optional function to look up features by ID for length_ref resolution
///
/// # Returns
/// A `GdtTorsorResult` with computed bounds and any warnings
pub fn compute_torsor_bounds<F>(
    feature: &Feature,
    actual_size: Option<f64>,
    feature_lookup: Option<&F>,
) -> GdtTorsorResult
where
    F: Fn(&str) -> Option<Feature>,
{
    let mut bounds = TorsorBounds::default();
    let mut warnings = Vec::new();
    let mut has_bonus = false;

    // Get geometry class, default to Complex (all DOFs active) if not specified
    let geometry_class = feature.geometry_class.unwrap_or(GeometryClass::Complex);

    // Get geometry 3D for computing angular bounds from linear tolerances
    let geometry_3d = feature.geometry_3d.as_ref();

    // Get primary dimension for MMC/LMC calculations
    let primary_dim = feature.primary_dimension();

    // Resolve length with tolerance for angular bound calculations
    let resolved_length = geometry_3d.map(|geo| resolve_length_with_tolerance(geo, feature_lookup));

    // Process each GD&T control and accumulate bounds
    for gdt in &feature.gdt {
        let gdt_bounds = compute_bounds_for_control(
            gdt,
            geometry_class,
            geometry_3d,
            primary_dim,
            actual_size,
            resolved_length.as_ref(),
        );

        // Merge bounds (simultaneous satisfaction per Y14.5 - tightest zone governs)
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
            warnings
                .push("Torsor bounds computed from dimensional tolerance (no GD&T)".to_string());
        }
    }

    // Validate that we have bounds for expected DOFs based on geometry class
    let validation_warnings = validate_bounds_for_geometry(&bounds, geometry_class);
    warnings.extend(validation_warnings);

    // Add info about length tolerance if used
    if let Some(ref len) = resolved_length {
        if len.has_tolerance() {
            warnings.push(format!(
                "Angular bounds include length tolerance from {} ({}mm +{:.3}/-{:.3})",
                len.source, len.nominal, len.plus_tol, len.minus_tol
            ));
        }
    }

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
    resolved_length: Option<&ResolvedLength>,
) -> GdtTorsorResult {
    let mut bounds = TorsorBounds::default();
    let mut warnings = Vec::new();
    let mut has_bonus = false;

    // Calculate effective tolerance (base + bonus if applicable)
    //
    // Bonus tolerance is the signed departure from the material condition
    // toward the opposite limit, clamped to >= 0. A size-nonconforming
    // feature (beyond MMC/LMC) gets NO bonus.
    let effective_tol = if let (Some(dim), Some(actual)) = (primary_dim, actual_size) {
        match gdt.material_condition {
            MaterialCondition::Mmc => {
                let mmc = dim.mmc();
                // Internal (hole): MMC = min size, bonus as actual grows.
                // External (shaft): MMC = max size, bonus as actual shrinks.
                let bonus = if dim.internal {
                    (actual - mmc).max(0.0)
                } else {
                    (mmc - actual).max(0.0)
                };
                if bonus > 0.0 {
                    has_bonus = true;
                }
                gdt.value + bonus
            }
            MaterialCondition::Lmc => {
                let lmc = dim.lmc();
                // Internal (hole): LMC = max size, bonus as actual shrinks.
                // External (shaft): LMC = min size, bonus as actual grows.
                let bonus = if dim.internal {
                    (lmc - actual).max(0.0)
                } else {
                    (actual - lmc).max(0.0)
                };
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
                    // Position on a planar feature is a zone of two parallel
                    // planes (total width t) centered on true position and
                    // oriented along the feature normal (ASME Y14.5). It
                    // bounds translation along the normal: w = ±t/2.
                    let bound = effective_tol / 2.0;
                    bounds.w = Some([-bound, bound]);
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
            if let Some(length) = resolved_length {
                // Angular bound computed with length tolerance consideration
                let angular_bound =
                    compute_angular_bound_with_length_tolerance(effective_tol, length);
                bounds.alpha = Some([-angular_bound, angular_bound]);
                bounds.beta = Some([-angular_bound, angular_bound]);
            } else if let Some(g) = geometry_3d {
                // Fallback to simple calculation if no resolved length
                let length = g.length.unwrap_or(10.0);
                let angular_bound = effective_tol / length;
                bounds.alpha = Some([-angular_bound, angular_bound]);
                bounds.beta = Some([-angular_bound, angular_bound]);
            } else {
                warnings.push(
                    "Perpendicularity GD&T requires geometry_3d.length for angular bound calculation"
                        .to_string(),
                );
            }
        }

        GdtSymbol::Parallelism => {
            // Parallelism affects angular DOFs (same as perpendicularity calculation)
            if let Some(length) = resolved_length {
                let angular_bound =
                    compute_angular_bound_with_length_tolerance(effective_tol, length);
                bounds.alpha = Some([-angular_bound, angular_bound]);
                bounds.beta = Some([-angular_bound, angular_bound]);
            } else if let Some(g) = geometry_3d {
                let length = g.length.unwrap_or(10.0);
                let angular_bound = effective_tol / length;
                bounds.alpha = Some([-angular_bound, angular_bound]);
                bounds.beta = Some([-angular_bound, angular_bound]);
            } else {
                warnings.push(
                    "Parallelism GD&T requires geometry_3d.length for angular bound calculation"
                        .to_string(),
                );
            }
        }

        GdtSymbol::Angularity => {
            // Angularity affects angular DOFs
            if let Some(length) = resolved_length {
                let angular_bound =
                    compute_angular_bound_with_length_tolerance(effective_tol, length);
                bounds.alpha = Some([-angular_bound, angular_bound]);
                bounds.beta = Some([-angular_bound, angular_bound]);
            } else if let Some(g) = geometry_3d {
                let length = g.length.unwrap_or(10.0);
                let angular_bound = effective_tol / length;
                bounds.alpha = Some([-angular_bound, angular_bound]);
                bounds.beta = Some([-angular_bound, angular_bound]);
            } else {
                warnings.push(
                    "Angularity GD&T requires geometry_3d.length for angular bound calculation"
                        .to_string(),
                );
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
            if let Some(length) = resolved_length {
                let angular_bound =
                    compute_angular_bound_with_length_tolerance(effective_tol, length);
                bounds.alpha = Some([-angular_bound, angular_bound]);
                bounds.beta = Some([-angular_bound, angular_bound]);
            } else if let Some(geo) = geometry_3d {
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
            if let Some(length) = resolved_length {
                let angular_bound =
                    compute_angular_bound_with_length_tolerance(effective_tol, length);
                bounds.alpha = Some([-angular_bound, angular_bound]);
                bounds.beta = Some([-angular_bound, angular_bound]);
            } else if let Some(geo) = geometry_3d {
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
            if let Some(length) = resolved_length {
                let angular_bound =
                    compute_angular_bound_with_length_tolerance(effective_tol, length);
                bounds.alpha = Some([-angular_bound, angular_bound]);
                bounds.beta = Some([-angular_bound, angular_bound]);
            } else if let Some(geo) = geometry_3d {
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
                    if let Some(length) = resolved_length {
                        let angular_bound =
                            compute_angular_bound_with_length_tolerance(effective_tol, length);
                        bounds.alpha = Some([-angular_bound, angular_bound]);
                        bounds.beta = Some([-angular_bound, angular_bound]);
                    } else if let Some(geo) = geometry_3d {
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

/// Merge two TorsorBounds, taking the tighter (intersected) bounds for each DOF
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

/// Merge two DOF bounds by intersection (tightest-wins).
///
/// Per ASME Y14.5, a feature must satisfy ALL applicable tolerance zones
/// simultaneously, so when multiple controls bound the same DOF the tightest
/// zone governs: [max of mins, min of maxes].
///
/// If the intersection is empty (contradictory controls whose zones do not
/// overlap), we deterministically fall back to the tighter control's bound
/// (smaller width; ties resolved in favor of `a`, the accumulated bound).
/// This keeps the result a valid, conservative interval instead of an
/// inverted one.
fn merge_dof(a: Option<[f64; 2]>, b: Option<[f64; 2]>) -> Option<[f64; 2]> {
    match (a, b) {
        (Some([a_min, a_max]), Some([b_min, b_max])) => {
            let lo = a_min.max(b_min);
            let hi = a_max.min(b_max);
            if lo <= hi {
                Some([lo, hi])
            } else if (a_max - a_min) <= (b_max - b_min) {
                Some([a_min, a_max])
            } else {
                Some([b_min, b_max])
            }
        }
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

/// Validate that bounds cover expected DOFs for geometry class
fn validate_bounds_for_geometry(
    bounds: &TorsorBounds,
    geometry_class: GeometryClass,
) -> Vec<String> {
    let mut warnings = Vec::new();

    match geometry_class {
        GeometryClass::Cylinder => {
            if bounds.u.is_none() || bounds.v.is_none() {
                warnings.push("Cylinder feature missing radial (u, v) bounds".to_string());
            }
        }
        GeometryClass::Plane => {
            if bounds.w.is_none() && bounds.alpha.is_none() && bounds.beta.is_none() {
                warnings
                    .push("Plane feature has no bounds - expected w, alpha, or beta".to_string());
            }
        }
        GeometryClass::Sphere | GeometryClass::Point => {
            if bounds.u.is_none() && bounds.v.is_none() && bounds.w.is_none() {
                warnings
                    .push("Point/Sphere feature missing positional (u, v, w) bounds".to_string());
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
                Some("stored torsor_bounds differs from computed (use 'tdt feat compute-bounds' to update)".to_string())
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
        Feature::new(
            "CMP-TEST",
            FeatureType::Internal,
            "Test Feature",
            "Test Author",
        )
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

        let result = compute_torsor_bounds::<fn(&str) -> Option<Feature>>(&feat, None, None);

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
        let result = compute_torsor_bounds::<fn(&str) -> Option<Feature>>(&feat, Some(10.05), None);

        assert!(result.has_bonus);
        let [u_min, u_max] = result.bounds.u.unwrap();
        // 0.30 / 2 = 0.15 radius
        assert!(
            (u_min - (-0.15)).abs() < 1e-10,
            "u_min should be -0.15, got {}",
            u_min
        );
        assert!(
            (u_max - 0.15).abs() < 1e-10,
            "u_max should be 0.15, got {}",
            u_max
        );
    }

    #[test]
    fn test_mmc_no_bonus_for_nonconforming_size() {
        // Regression: a hole 10.0 +0.1/-0 measured at 9.9 is BELOW MMC (10.0),
        // i.e. size-nonconforming. It must get bonus = 0.0, not a phantom
        // |9.9 - 10.0| = 0.1 bonus.
        let mut feat = create_test_feature();
        feat.geometry_class = Some(GeometryClass::Cylinder);
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

        let result = compute_torsor_bounds::<fn(&str) -> Option<Feature>>(&feat, Some(9.9), None);

        assert!(!result.has_bonus, "nonconforming size must not grant bonus");
        let [u_min, u_max] = result.bounds.u.unwrap();
        // Base position 0.25 diameter -> ±0.125, no bonus
        assert!(
            (u_min - (-0.125)).abs() < 1e-10,
            "u_min should be -0.125, got {}",
            u_min
        );
        assert!(
            (u_max - 0.125).abs() < 1e-10,
            "u_max should be 0.125, got {}",
            u_max
        );
    }

    #[test]
    fn test_lmc_no_bonus_for_nonconforming_size() {
        // Hole 10.0 +0.1/-0 at LMC modifier: LMC = 10.1. Actual 10.2 is
        // beyond LMC (nonconforming) -> bonus 0.0. Actual 10.0 -> bonus 0.1.
        let mut feat = create_test_feature();
        feat.geometry_class = Some(GeometryClass::Cylinder);
        feat.dimensions.push(Dimension {
            name: "diameter".to_string(),
            nominal: 10.0,
            plus_tol: 0.1,
            minus_tol: 0.0,
            units: "mm".to_string(),
            internal: true,
            distribution: Distribution::Normal,
        });
        feat.gdt.push(GdtControl {
            symbol: GdtSymbol::Position,
            value: 0.25,
            units: "mm".to_string(),
            datum_refs: vec!["A".to_string()],
            material_condition: MaterialCondition::Lmc,
        });

        // Nonconforming (beyond LMC): no bonus
        let result = compute_torsor_bounds::<fn(&str) -> Option<Feature>>(&feat, Some(10.2), None);
        assert!(!result.has_bonus);
        let [u_min, u_max] = result.bounds.u.unwrap();
        assert!((u_min - (-0.125)).abs() < 1e-10);
        assert!((u_max - 0.125).abs() < 1e-10);

        // Departure from LMC toward MMC: bonus = 10.1 - 10.0 = 0.1
        let result = compute_torsor_bounds::<fn(&str) -> Option<Feature>>(&feat, Some(10.0), None);
        assert!(result.has_bonus);
        let [u_min, u_max] = result.bounds.u.unwrap();
        // (0.25 + 0.1) / 2 = 0.175
        assert!((u_min - (-0.175)).abs() < 1e-10);
        assert!((u_max - 0.175).abs() < 1e-10);
    }

    #[test]
    fn test_position_plane_bounds_normal() {
        // Regression: position on a planar feature is a two-parallel-planes
        // zone along the feature NORMAL: w = ±t/2. u/v must NOT be bounded
        // by that control.
        let mut feat = create_test_feature();
        feat.geometry_class = Some(GeometryClass::Plane);
        feat.gdt.push(GdtControl {
            symbol: GdtSymbol::Position,
            value: 0.2,
            units: "mm".to_string(),
            datum_refs: vec!["A".to_string()],
            material_condition: MaterialCondition::Rfs,
        });

        let result = compute_torsor_bounds::<fn(&str) -> Option<Feature>>(&feat, None, None);

        assert!(
            result.bounds.u.is_none(),
            "position on plane must not bound u"
        );
        assert!(
            result.bounds.v.is_none(),
            "position on plane must not bound v"
        );
        let [w_min, w_max] = result.bounds.w.unwrap();
        assert!(
            (w_min - (-0.1)).abs() < 1e-10,
            "w_min should be -0.1, got {}",
            w_min
        );
        assert!(
            (w_max - 0.1).abs() < 1e-10,
            "w_max should be 0.1, got {}",
            w_max
        );
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

        let result = compute_torsor_bounds::<fn(&str) -> Option<Feature>>(&feat, None, None);

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
            length_ref: None,
        });
        feat.gdt.push(GdtControl {
            symbol: GdtSymbol::Perpendicularity,
            value: 0.10,
            units: "mm".to_string(),
            datum_refs: vec!["A".to_string()],
            material_condition: MaterialCondition::Rfs,
        });

        let result = compute_torsor_bounds::<fn(&str) -> Option<Feature>>(&feat, None, None);

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

        let result = compute_torsor_bounds::<fn(&str) -> Option<Feature>>(&feat, None, None);

        assert!(!result.warnings.is_empty());
        assert!(result
            .warnings
            .iter()
            .any(|w| w.contains("geometry_3d.length")));
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

        let result = compute_torsor_bounds::<fn(&str) -> Option<Feature>>(&feat, None, None);

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

        let result = compute_torsor_bounds::<fn(&str) -> Option<Feature>>(&feat, None, None);

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
            length_ref: None,
        });
        feat.gdt.push(GdtControl {
            symbol: GdtSymbol::Runout,
            value: 0.10,
            units: "mm".to_string(),
            datum_refs: vec!["A".to_string()],
            material_condition: MaterialCondition::Rfs,
        });

        let result = compute_torsor_bounds::<fn(&str) -> Option<Feature>>(&feat, None, None);

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
            length_ref: None,
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

        let result = compute_torsor_bounds::<fn(&str) -> Option<Feature>>(&feat, None, None);

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

        let result = compute_torsor_bounds::<fn(&str) -> Option<Feature>>(&feat, None, None);

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
    fn test_merge_bounds_takes_tighter() {
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

        // u: intersection = [max(-0.1, -0.05), min(0.1, 0.15)] = [-0.05, 0.1]
        // (simultaneous satisfaction - tightest zone governs)
        let [u_min, u_max] = merged.u.unwrap();
        assert!((u_min - (-0.05)).abs() < 1e-10);
        assert!((u_max - 0.1).abs() < 1e-10);

        // v: only from a
        assert!(merged.v.is_some());

        // w: only from b
        assert!(merged.w.is_some());
    }

    #[test]
    fn test_merge_dof_empty_intersection_falls_back_to_tighter() {
        // Disjoint (contradictory) bounds: fall back to the tighter interval
        let merged = merge_dof(Some([-0.3, -0.2]), Some([0.1, 0.15]));
        let [lo, hi] = merged.unwrap();
        // b has width 0.05 < a's width 0.1, so b wins
        assert!((lo - 0.1).abs() < 1e-10);
        assert!((hi - 0.15).abs() < 1e-10);
    }

    #[test]
    fn test_tightest_wins_position_plus_concentricity() {
        // Regression: position ⌀0.5 + concentricity 0.08 on a cylinder must
        // yield u = ±0.04 (tightest zone governs), not the union ±0.25.
        let mut feat = create_test_feature();
        feat.geometry_class = Some(GeometryClass::Cylinder);
        feat.gdt.push(GdtControl {
            symbol: GdtSymbol::Position,
            value: 0.5,
            units: "mm".to_string(),
            datum_refs: vec!["A".to_string()],
            material_condition: MaterialCondition::Rfs,
        });
        feat.gdt.push(GdtControl {
            symbol: GdtSymbol::Concentricity,
            value: 0.08,
            units: "mm".to_string(),
            datum_refs: vec!["A".to_string()],
            material_condition: MaterialCondition::Rfs,
        });

        let result = compute_torsor_bounds::<fn(&str) -> Option<Feature>>(&feat, None, None);

        let [u_min, u_max] = result.bounds.u.unwrap();
        assert!(
            (u_min - (-0.04)).abs() < 1e-10,
            "u_min should be -0.04, got {}",
            u_min
        );
        assert!(
            (u_max - 0.04).abs() < 1e-10,
            "u_max should be 0.04, got {}",
            u_max
        );
        let [v_min, v_max] = result.bounds.v.unwrap();
        assert!((v_min - (-0.04)).abs() < 1e-10);
        assert!((v_max - 0.04).abs() < 1e-10);
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

    // ===== ResolvedLength Tests =====

    #[test]
    fn test_resolved_length_fixed() {
        let len = ResolvedLength::fixed(50.0);
        assert_eq!(len.nominal, 50.0);
        assert_eq!(len.plus_tol, 0.0);
        assert_eq!(len.minus_tol, 0.0);
        assert_eq!(len.source, "fixed");
        assert!(!len.has_tolerance());
    }

    #[test]
    fn test_resolved_length_with_tolerance() {
        let len = ResolvedLength {
            nominal: 50.0,
            plus_tol: 0.5,
            minus_tol: 0.3,
            source: "FEAT@1:depth".to_string(),
        };

        assert!(len.has_tolerance());
        assert!((len.min_length() - 49.7).abs() < 1e-10);
        assert!((len.max_length() - 50.5).abs() < 1e-10);
    }

    #[test]
    fn test_resolved_length_min_clamp() {
        // Test that min_length is clamped to prevent division by zero
        let len = ResolvedLength {
            nominal: 0.5,
            plus_tol: 0.0,
            minus_tol: 0.5, // Would make min_length = 0
            source: "test".to_string(),
        };

        // Should be clamped to 0.001
        assert!((len.min_length() - 0.001).abs() < 1e-10);
    }

    #[test]
    fn test_resolved_length_variance() {
        let len = ResolvedLength {
            nominal: 50.0,
            plus_tol: 0.3,  // +0.3
            minus_tol: 0.3, // -0.3
            source: "test".to_string(),
        };

        // σ = (0.3 + 0.3) / 6.0 = 0.1
        // variance = σ² = 0.01
        let variance = len.variance(6.0);
        assert!((variance - 0.01).abs() < 1e-10);
    }

    #[test]
    fn test_angular_bound_with_length_tolerance() {
        // With tolerance, angular bound should be larger at min length
        let len_with_tol = ResolvedLength {
            nominal: 50.0,
            plus_tol: 5.0,  // Max length = 55
            minus_tol: 5.0, // Min length = 45
            source: "test".to_string(),
        };

        let linear_tol = 0.1; // 0.1mm perpendicularity

        let angular = compute_angular_bound_with_length_tolerance(linear_tol, &len_with_tol);

        // At min length (45): 0.1/45 = 0.00222...
        // At max length (55): 0.1/55 = 0.00181...
        // Should take the max = 0.1/45
        let expected = linear_tol / 45.0;
        assert!(
            (angular - expected).abs() < 1e-10,
            "Expected {}, got {}",
            expected,
            angular
        );
    }

    #[test]
    fn test_angular_bound_fixed_length() {
        // With no tolerance, should use nominal length
        let len_fixed = ResolvedLength::fixed(50.0);
        let linear_tol = 0.1;

        let angular = compute_angular_bound_with_length_tolerance(linear_tol, &len_fixed);

        let expected = linear_tol / 50.0; // 0.002
        assert!((angular - expected).abs() < 1e-10);
    }
}
