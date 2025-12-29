# TDT Stackup Entity (Tolerance Analysis)

This document describes the Stackup entity type in TDT (Tessera Design Toolkit).

## Overview

Stackups represent tolerance chain analyses with multiple dimensional contributors. They calculate whether a target dimension (like a gap or clearance) will meet specification limits given the tolerances of all contributing features. TDT supports three analysis methods: worst-case, RSS (statistical), and Monte Carlo simulation.

## Entity Type

- **Prefix**: `TOL`
- **File extension**: `.tdt.yaml`
- **Directory**: `tolerances/stackups/`

## Schema

### Required Fields

| Field | Type | Description |
|-------|------|-------------|
| `id` | string | Unique identifier (TOL-[26-char ULID]) |
| `title` | string | Short descriptive title (1-200 chars) |
| `target` | Target | Target dimension specification |
| `status` | enum | `draft`, `review`, `approved`, `released`, `obsolete` |
| `created` | datetime | Creation timestamp (ISO 8601) |
| `author` | string | Author name |

### Target Object

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | Target dimension name (e.g., "Gap") |
| `nominal` | number | Nominal target value |
| `upper_limit` | number | Upper specification limit (USL) |
| `lower_limit` | number | Lower specification limit (LSL) |
| `units` | string | Units (default: "mm") |
| `critical` | boolean | Is this a critical dimension? |

### Optional Fields

| Field | Type | Description |
|-------|------|-------------|
| `description` | string | Detailed description |
| `contributors` | array[Contributor] | Dimensional contributors |
| `sigma_level` | number | Sigma level for statistical analysis (default: 6.0, meaning ±3σ) |
| `mean_shift_k` | number | Bender k-factor for process drift modeling (default: 0.0) |
| `include_gdt` | boolean | Include GD&T position tolerances in calculations (default: false) |
| `functional_direction` | array[number] | Functional direction [dx, dy, dz] for 3D analysis |
| `analysis_3d` | Analysis3DConfig | 3D analysis configuration |
| `analysis_results` | AnalysisResults | Auto-calculated results (1D) |
| `analysis_results_3d` | Analysis3DResults | Auto-calculated 3D results |
| `disposition` | enum | `under_review`, `approved`, `rejected` |
| `tags` | array[string] | Tags for filtering |
| `entity_revision` | integer | Entity revision number (default: 1) |

### Contributor Object

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | Contributor name/description |
| `feature` | FeatureRef | Optional reference to FEAT entity with cached info |
| `direction` | enum | `positive` or `negative` |
| `nominal` | number | Nominal value |
| `plus_tol` | number | Plus tolerance (positive number) |
| `minus_tol` | number | Minus tolerance (positive number) |
| `distribution` | enum | `normal`, `uniform`, `triangular` |
| `source` | string | Source reference (drawing, etc.) |
| `gdt_position` | GdtContribution | Optional GD&T position tolerance contribution |

### GdtContribution Object

| Field | Type | Description |
|-------|------|-------------|
| `position_tolerance` | number | Position tolerance value (diameter of cylindrical zone) - **Required** |
| `actual_size` | number | Actual feature size for bonus tolerance calculation (MMC/LMC) |
| `bonus` | number | Calculated bonus tolerance (departure from MMC/LMC) |
| `effective_tolerance` | number | Effective position tolerance (position_tolerance + bonus) |

### FeatureRef Object (Cached Feature Reference)

| Field | Type | Description |
|-------|------|-------------|
| `id` | EntityId | Feature entity ID (FEAT-...) - **Required** |
| `name` | string | Feature name (cached from feature entity) |
| `component_id` | string | Component ID that owns this feature (cached) |
| `component_name` | string | Component name/title (cached for readability) |

**Feature Linking**: When a contributor has a `feature` reference, its `nominal`, `plus_tol`, and `minus_tol` values should match the linked feature's primary dimension. The cached fields (`name`, `component_id`, `component_name`) improve readability and are validated against the actual feature during `tdt validate`. TDT can automatically sync values when they drift out of sync.

### AnalysisResults Object (Auto-calculated)

| Field | Type | Description |
|-------|------|-------------|
| `worst_case` | WorstCaseResult | Worst-case analysis |
| `rss` | RssResult | RSS statistical analysis |
| `monte_carlo` | MonteCarloResult | Monte Carlo simulation |

### RssResult Object

| Field | Type | Description |
|-------|------|-------------|
| `mean` | number | Mean (expected) value |
| `sigma_3` | number | 3-sigma spread (±3σ) |
| `margin` | number | Margin to specification limits at 3σ |
| `cp` | number | Process capability (Cp) - ignores centering: (USL - LSL) / (6σ) |
| `cpk` | number | Process capability (Cpk) - accounts for centering: min(USL-μ, μ-LSL) / (3σ) |
| `yield_percent` | number | Estimated yield percentage |
| `sensitivity` | array[number] | Variance contribution percentage for each contributor (sum = 100%) |
| `shifted_mean` | number | Shifted mean when mean_shift_k > 0 (Bender method) |

### MonteCarloResult Object

| Field | Type | Description |
|-------|------|-------------|
| `iterations` | integer | Number of Monte Carlo iterations |
| `mean` | number | Sample mean |
| `std_dev` | number | Sample standard deviation |
| `min` | number | Minimum observed value |
| `max` | number | Maximum observed value |
| `yield_percent` | number | Percentage of samples within spec |
| `percentile_2_5` | number | 2.5th percentile (95% CI lower) |
| `percentile_97_5` | number | 97.5th percentile (95% CI upper) |
| `pp` | number | Process performance (Pp) - uses sample std_dev: (USL - LSL) / (6s) |
| `ppk` | number | Process performance (Ppk) - uses sample std_dev, accounts for centering |

### Analysis3DConfig Object

| Field | Type | Description |
|-------|------|-------------|
| `enabled` | boolean | Enable 3D torsor-based analysis |
| `method` | string | `jacobian_torsor` or `monte_carlo_3d` |
| `monte_carlo_iterations` | integer | Iterations for Monte Carlo 3D (default: 10000) |

### TorsorStats Object

| Field | Type | Description |
|-------|------|-------------|
| `wc_min` | number | Worst-case minimum value |
| `wc_max` | number | Worst-case maximum value |
| `rss_mean` | number | Statistical mean |
| `rss_3sigma` | number | Statistical 3-sigma spread |
| `mc_mean` | number | Monte Carlo mean |
| `mc_std_dev` | number | Monte Carlo standard deviation |

### ResultTorsor Object

| Field | Type | Description |
|-------|------|-------------|
| `u` | TorsorStats | Translation along X |
| `v` | TorsorStats | Translation along Y |
| `w` | TorsorStats | Translation along Z |
| `alpha` | TorsorStats | Rotation about X |
| `beta` | TorsorStats | Rotation about Y |
| `gamma` | TorsorStats | Rotation about Z |

### Analysis3DResults Object

| Field | Type | Description |
|-------|------|-------------|
| `result_torsor` | ResultTorsor | 6-DOF result torsor with statistics |
| `sensitivity_3d` | array | Per-contributor sensitivity by DOF |
| `jacobian_summary` | object | Chain length and DOF summary |
| `analyzed_at` | datetime | Analysis timestamp |

### Links

| Field | Type | Description |
|-------|------|-------------|
| `links.verifies` | array[EntityId] | Requirements verified by this stackup |
| `links.mates_used` | array[EntityId] | Mates included in this stackup |

## Tolerance Format

Contributors use `plus_tol` and `minus_tol` fields:

```yaml
contributors:
  - name: "Part A Length"
    direction: positive
    nominal: 10.0
    plus_tol: 0.1     # +0.1
    minus_tol: 0.05   # -0.05
    distribution: normal
```

**Important**: Both values are stored as **positive numbers**.

## Example

```yaml
# Stackup: Gap Analysis
# Created by TDT - Tessera Design Toolkit

id: TOL-01HC2JB7SMQX7RS1Y0GFKBHPTH
title: "Gap Analysis"

description: |
  Analysis of the gap between the housing and cover.
  Gap must be maintained for proper assembly clearance.

target:
  name: "Gap"
  nominal: 1.0
  upper_limit: 1.5
  lower_limit: 0.5
  units: "mm"
  critical: true

contributors:
  - name: "Housing Depth"
    feature:
      id: FEAT-01HC2JB7SMQX7RS1Y0GFKBHPTE
      name: "Depth"                         # Cached from feature
      component_id: CMP-01HC2JB7SMQX7RS1Y0GFKBHPTA
      component_name: "Housing"             # Cached for readability
    direction: positive
    nominal: 50.0
    plus_tol: 0.1
    minus_tol: 0.1
    distribution: normal
    source: "DWG-001 Rev A"

  - name: "Cover Height"
    feature:
      id: FEAT-01HC2JB7SMQX7RS1Y0GFKBHPTF
      name: "Height"
      component_id: CMP-01HC2JB7SMQX7RS1Y0GFKBHPTB
      component_name: "Cover"
    direction: negative
    nominal: 45.0
    plus_tol: 0.08
    minus_tol: 0.08
    distribution: normal
    source: "DWG-002 Rev A"

  - name: "Gasket Thickness"
    # No feature link - manually entered contributor
    direction: negative
    nominal: 2.0
    plus_tol: 0.15
    minus_tol: 0.10
    distribution: uniform
    source: "Vendor Spec"

  - name: "Bracket Height"
    feature:
      id: FEAT-01HC2JB7SMQX7RS1Y0GFKBHPTG
      name: "Height"
      component_id: CMP-01HC2JB7SMQX7RS1Y0GFKBHPTC
      component_name: "Bracket"
    direction: negative
    nominal: 2.0
    plus_tol: 0.05
    minus_tol: 0.05
    distribution: normal
    source: "DWG-003 Rev A"

# Statistical analysis parameters
sigma_level: 6.0       # 6σ means tolerance = ±3σ (default)
mean_shift_k: 0.0      # Bender k-factor (0=none, 1.5=automotive)
include_gdt: false     # Include GD&T position in calculations

# Auto-calculated by 'tdt tol analyze'
analysis_results:
  worst_case:
    min: 0.62
    max: 1.38
    margin: 0.12
    result: pass
  rss:
    mean: 1.0
    sigma_3: 0.21
    margin: 0.29
    cp: 1.59
    cpk: 1.59
    yield_percent: 99.9997
    sensitivity: [40.0, 30.0, 20.0, 10.0]  # % contribution per contributor
  monte_carlo:
    iterations: 10000
    mean: 1.0
    std_dev: 0.07
    min: 0.71
    max: 1.28
    yield_percent: 100.0
    percentile_2_5: 0.86
    percentile_97_5: 1.14
    pp: 1.19
    ppk: 1.19

disposition: approved
tags: [critical, thermal, assembly]
status: approved

links:
  verifies:
    - REQ-01HC2JB7SMQX7RS1Y0GFKBHPTI
  mates_used: []

# Auto-managed metadata
created: 2024-01-15T10:30:00Z
author: John Smith
entity_revision: 1
```

## CLI Commands

### Create a new stackup

```bash
# Create with target specification
tdt tol new --title "Gap Analysis" --target-nominal 1.0 --target-upper 1.5 --target-lower 0.5

# Specify target name
tdt tol new --title "Gap Analysis" --target-name "Gap" \
    --target-nominal 1.0 --target-upper 1.5 --target-lower 0.5

# Mark as critical
tdt tol new --title "Critical Gap" --target-nominal 1.0 \
    --target-upper 1.5 --target-lower 0.5 --critical

# Create with interactive wizard
tdt tol new -i

# Create and immediately edit
tdt tol new --title "New Stackup" --edit
```

### List stackups

```bash
# List all stackups
tdt tol list

# Filter by worst-case result
tdt tol list --result pass
tdt tol list --result fail
tdt tol list --result marginal

# Filter by disposition
tdt tol list --disposition approved
tdt tol list --disposition rejected

# Show only critical stackups
tdt tol list --critical

# Filter by status
tdt tol list --status approved

# Search in title/description
tdt tol list --search "gap"

# Sort and limit
tdt tol list --sort title
tdt tol list --limit 10

# Count only
tdt tol list --count

# Output formats
tdt tol list -f json
tdt tol list -f csv
```

### Show stackup details

```bash
# Show by ID (includes analysis results)
tdt tol show TOL-01HC2

# Show using short ID
tdt tol show TOL@1

# Output as JSON
tdt tol show TOL@1 -f json
```

### Run analysis

```bash
# Run all analyses (worst-case, RSS, Monte Carlo)
tdt tol analyze TOL@1

# Custom Monte Carlo iterations
tdt tol analyze TOL@1 --iterations 50000

# Verbose output (shows Cp, Cpk, Pp, Ppk, sensitivity)
tdt tol analyze TOL@1 --verbose

# Custom sigma level (4σ = ±2σ process capability)
tdt tol analyze TOL@1 --sigma 4.0

# Mean shift factor (Bender method for process drift)
tdt tol analyze TOL@1 --mean-shift 1.5

# Include GD&T position tolerances in calculations
tdt tol analyze TOL@1 --with-gdt

# Show sensitivity analysis (% contribution per contributor)
tdt tol analyze TOL@1 --sensitivity

# 3D torsor-based analysis (requires features with geometry_3d)
tdt tol analyze TOL@1 --3d

# 3D analysis with braille visualization
tdt tol analyze TOL@1 --3d --visualize

# 3D Monte Carlo analysis
tdt tol analyze TOL@1 --3d --method-3d monte-carlo
```

### Add features as contributors

```bash
# Add features with direction prefix
# Use + for positive direction, ~ for negative
# Distribution is pulled from the feature's dimension
tdt tol add TOL@1 +FEAT@1 ~FEAT@2 +FEAT@3

# Specify which dimension to use (default: first dimension)
tdt tol add TOL@1 --dimension length +FEAT@1

# Run analysis after adding
tdt tol add TOL@1 --analyze +FEAT@1 ~FEAT@2
```

### Remove contributors

```bash
# Remove contributor(s) by feature ID
tdt tol rm TOL@1 FEAT@1

# Remove multiple contributors
tdt tol rm TOL@1 FEAT@1 FEAT@2
```

### Edit a stackup

```bash
# Open in editor
tdt tol edit TOL-01HC2

# Using short ID
tdt tol edit TOL@1
```

### Delete or archive a stackup

```bash
# Permanently delete (checks for incoming links first)
tdt tol delete TOL@1

# Force delete even if referenced
tdt tol delete TOL@1 --force

# Archive instead of delete (moves to .tdt/archive/)
tdt tol archive TOL@1
```

## Analysis Methods

### Worst-Case Analysis

Assumes all dimensions are simultaneously at their worst-case limits:

```
For each contributor:
  if positive:
    min_result += (nominal - minus_tol)
    max_result += (nominal + plus_tol)
  if negative:
    min_result -= (nominal + plus_tol)
    max_result -= (nominal - minus_tol)

margin = min(USL - max_result, min_result - LSL)

result:
  pass:     margin > 10% of tolerance band
  marginal: margin > 0 but < 10% of tolerance band
  fail:     margin < 0
```

**Use when**: 100% conformance is required, safety-critical applications

### RSS (Root Sum Square) Analysis

Statistical analysis assuming normal distributions with configurable sigma level:

```
mean = sum of (sign * nominal)

For each contributor:
  sigma = (plus_tol + minus_tol) / sigma_level  # Default: 6.0 (±3σ process)
  variance += sigma^2

sigma_total = sqrt(variance)
sigma_3 = 3 * sigma_total

# Process Capability Indices
Cp  = (USL - LSL) / (6 * sigma_total)           # Ignores centering
Cpk = min(USL - mean, mean - LSL) / (3 * sigma_total)  # Accounts for centering

# Sensitivity Analysis (% variance contribution)
sensitivity[i] = (sigma_i^2 / sum(sigma^2)) * 100%
```

**Process Capability Indices**:

| Index | Formula | Description |
|-------|---------|-------------|
| **Cp** | (USL - LSL) / (6σ) | Potential capability (ignores centering) |
| **Cpk** | min(USL-μ, μ-LSL) / (3σ) | Actual capability (accounts for centering) |

When Cp ≈ Cpk, the process is well-centered. When Cpk < Cp, there's centering loss.

**Cpk Guidelines**:

| Cpk | Sigma Level | Yield | Quality |
|-----|-------------|-------|---------|
| 0.33 | 1σ | 68.27% | Poor |
| 0.67 | 2σ | 95.45% | Marginal |
| 1.0 | 3σ | 99.73% | Capable |
| 1.33 | 4σ | 99.99% | Good |
| 1.67 | 5σ | 99.9997% | Excellent |
| 2.0 | 6σ | 99.9999% | Six Sigma |

**Sensitivity Analysis**: Shows which contributors dominate the total variance. Use this to prioritize tolerance tightening efforts on the highest-contributing dimensions.

**Use when**: Statistical process control is in place, many contributors

### Mean Shift Factor (Bender Method)

The Bender method models process drift by shifting the mean toward the nearest specification limit:

```
if mean_shift_k > 0:
  if (USL - mean) < (mean - LSL):
    shifted_mean = mean + k * sigma_total  # Shift toward USL
  else:
    shifted_mean = mean - k * sigma_total  # Shift toward LSL

  Cpk uses shifted_mean for more conservative estimate
```

Common k-factor values:
- **0.0**: No shift (default)
- **1.5**: Automotive/Bender method (accounts for typical process drift)

**Use when**: Long-term process variation is a concern, conservative estimates needed

### GD&T Position Integration

When `include_gdt: true`, GD&T position tolerances are added to dimensional tolerances:

```
total_tolerance_band = (plus_tol + minus_tol) + effective_position_tolerance

effective_position_tolerance = position_tolerance + bonus (if MMC/LMC)
```

This provides more realistic tolerance analysis when position tolerances significantly contribute to the tolerance chain.

**Use when**: Position tolerances are significant contributors to the stackup

### 3D SDT Analysis (Small Displacement Torsor)

3D tolerance analysis uses Small Displacement Torsor (SDT) theory to propagate geometric deviations through kinematic chains. Each feature contributes a 6-DOF torsor (3 translations + 3 rotations).

**Torsor Components**:

| Component | Symbol | Description |
|-----------|--------|-------------|
| `u` | u | Translation along local X |
| `v` | v | Translation along local Y |
| `w` | w | Translation along local Z |
| `alpha` | α | Rotation about local X |
| `beta` | β | Rotation about local Y |
| `gamma` | γ | Rotation about local Z |

**Invariance Classes** (DOF constraints by geometry type):

| Geometry | Constrained DOF | Free DOF |
|----------|-----------------|----------|
| **Plane** | w, α, β | u, v, γ |
| **Cylinder** | u, v, α, β | w, γ |
| **Sphere** | u, v, w | α, β, γ |
| **Cone** | u, v, α, β | w, γ |
| **Point** | u, v, w | α, β, γ |
| **Line** | u, v | w, α, β, γ |

**Jacobian Propagation**:

The Jacobian matrix transforms torsors through the kinematic chain:

```
J = | I₃   [r]× |
    | 0₃    I₃  |

where [r]× is the skew-symmetric matrix of position vector r
```

**Use when**: Multi-DOF geometric analysis, complex kinematic chains, 3D position/orientation tolerances

**Requirements**:
- Features must have `geometry_3d` defined (origin, axis)
- Features must have `geometry_class` specified
- Stackup must have `functional_direction` for result projection

### Monte Carlo Simulation

Runs thousands of random samples with configurable distributions:

```
For each iteration (default: 10,000):
  For each contributor:
    Sample value from distribution (normal, uniform, triangular)
    Apply direction (positive or negative)
    Sum to get result

Calculate statistics:
  mean, std_dev, min, max
  yield = (samples in spec) / (total samples) * 100%
  percentile_2_5, percentile_97_5 (95% confidence interval)

# Process Performance Indices (using sample statistics)
Pp  = (USL - LSL) / (6 * std_dev)           # Ignores centering
Ppk = min(USL - mean, mean - LSL) / (3 * std_dev)  # Accounts for centering
```

**Process Performance vs Capability**: Pp/Ppk use actual sample standard deviation from Monte Carlo, while Cp/Cpk use theoretical σ from tolerance assumptions. When distributions are non-normal or processes have drift, Pp/Ppk may differ from Cp/Cpk.

**Distributions**:

| Distribution | Description | When to Use |
|--------------|-------------|-------------|
| **normal** | Bell curve (Gaussian) | Machined parts, stable processes |
| **uniform** | Equal probability | Vendor tolerances, unknown distribution |
| **triangular** | Peak at nominal | Assembly tolerances, skilled processes |

**Use when**: Complex distributions, non-normal processes, high-fidelity analysis

## Contributor Direction

| Direction | Effect on Result | Example |
|-----------|------------------|---------|
| **positive** | Adds to result | Housing depth |
| **negative** | Subtracts from result | Cover height |

## Best Practices

### Building Stackups

1. **Define the loop** - Draw the tolerance chain from datum to target
2. **List all contributors** - Include every dimension in the chain
3. **Set directions correctly** - Positive adds, negative subtracts
4. **Reference sources** - Document where each tolerance comes from
5. **Link to features** - Connect contributors to FEAT entities when possible

### Tolerance Allocation

1. **Start with worst-case** - Ensure feasibility with conservative analysis
2. **Use RSS for cost reduction** - Loosen tolerances where statistical is acceptable
3. **Verify with Monte Carlo** - Confirm yield predictions
4. **Iterate as needed** - Tighten critical contributors if required

### Managing Stackups

1. **Run analysis after changes** - Recalculate when contributors change
2. **Sync from features** - Use `tdt validate --fix` to sync contributors with linked features
3. **Track disposition** - Document approval/rejection decisions
4. **Link to requirements** - Connect to requirements being verified
5. **Mark critical dimensions** - Flag safety/function-critical stackups

## Validation

Stackups are validated against a JSON Schema:

```bash
# Validate all project files
tdt validate

# Validate specific file
tdt validate tolerances/stackups/TOL-01HC2JB7SMQX7RS1Y0GFKBHPTH.tdt.yaml
```

### Validation Rules

1. **ID Format**: Must match `TOL-[A-Z0-9]{26}` pattern
2. **Title**: Required, 1-200 characters
3. **Target**: Required with name, nominal, upper_limit, lower_limit
4. **Contributors**: Must have name, nominal, plus_tol, minus_tol
5. **Feature Reference**: Contributors with `feature.id` must reference valid features
6. **Dimensional Sync**: Contributor dimensions must match linked feature's primary dimension
7. **Cached Info Sync**: Cached `feature.name` and `feature.component_id` must match actual values
8. **Direction**: Must be `positive` or `negative`
9. **Distribution**: Must be `normal`, `uniform`, or `triangular`
10. **Disposition**: Must be `under_review`, `approved`, or `rejected`
11. **Status**: Must be one of: `draft`, `review`, `approved`, `released`, `obsolete`
12. **No Additional Properties**: Unknown fields are not allowed

### Syncing Contributors from Features

When feature dimensions or metadata change, contributors may become out of sync:

```bash
# Check for out-of-sync contributors
tdt validate

# Example warnings:
# ! TOL-01HC2... - calculation warning(s)
#     Contributor 'Housing Depth' out of sync with FEAT-...:
#     stored (50.0000 +0.1000/-0.1000) vs feature (50.0000 +0.1500/-0.1000)
#
#     Contributor 'Housing Depth' has stale cached name 'Old Name' (feature is 'Depth')

# Auto-sync contributor values and cached info from features
tdt validate --fix
```

The `--fix` flag will update:
- Dimensional values (`nominal`, `plus_tol`, `minus_tol`) to match linked features
- Cached feature info (`name`, `component_id`) to match actual values

Note that this does NOT automatically re-run the analysis - use `tdt tol analyze` after fixing to recalculate results.

## JSON Schema

The full JSON Schema for stackups is available at:

```
tdt/schemas/tol.schema.json
```
