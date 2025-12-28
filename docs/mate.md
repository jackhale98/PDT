# TDT Mate Entity (Tolerances)

This document describes the Mate entity type in TDT (Tessera Design Toolkit).

## Overview

Mates represent 1:1 contact relationships between two features, such as a pin fitting into a hole. TDT automatically calculates worst-case fit analysis when you create or recalculate a mate, determining whether it's a clearance, interference, or transition fit.

**Auto-detection**: TDT automatically determines which feature is the hole and which is the shaft based on their `internal` field - no need to remember which order to link them!

## Entity Type

- **Prefix**: `MATE`
- **File extension**: `.tdt.yaml`
- **Directory**: `tolerances/mates/`

## Schema

### Required Fields

| Field | Type | Description |
|-------|------|-------------|
| `id` | string | Unique identifier (MATE-[26-char ULID]) |
| `feature_a` | MateFeatureRef | First feature reference with cached info |
| `feature_b` | MateFeatureRef | Second feature reference with cached info |
| `mate_type` | enum | `clearance`, `transition`, `interference` |
| `title` | string | Short descriptive title (1-200 chars) |
| `status` | enum | `draft`, `review`, `approved`, `released`, `obsolete` |
| `created` | datetime | Creation timestamp (ISO 8601) |
| `author` | string | Author name |

### MateFeatureRef Object (Cached Feature Reference)

| Field | Type | Description |
|-------|------|-------------|
| `id` | EntityId | Feature entity ID (FEAT-...) - **Required** |
| `name` | string | Feature name (cached from feature entity) |
| `component_id` | string | Component ID that owns this feature (cached) |
| `component_name` | string | Component name/title (cached for readability) |

**Note**: The order of `feature_a` and `feature_b` doesn't matter - TDT auto-detects which is the hole (internal) and which is the shaft (external) based on their `internal` field. The cached fields improve readability and are validated during `tdt validate`.

### Optional Fields

| Field | Type | Description |
|-------|------|-------------|
| `description` | string | Detailed description |
| `fit_analysis` | FitAnalysis | Auto-calculated fit results |
| `notes` | string | Additional notes |
| `tags` | array[string] | Tags for filtering |
| `entity_revision` | integer | Entity revision number (default: 1) |

### FitAnalysis Object (Auto-calculated)

| Field | Type | Description |
|-------|------|-------------|
| `worst_case_min_clearance` | number | Minimum clearance (or max interference if negative) |
| `worst_case_max_clearance` | number | Maximum clearance (or min interference if negative) |
| `fit_result` | enum | `clearance`, `interference`, `transition` |
| `statistical` | StatisticalFit | Optional statistical (RSS) fit analysis |

### StatisticalFit Object (Optional)

| Field | Type | Description |
|-------|------|-------------|
| `mean_clearance` | number | Mean clearance (hole_mean - shaft_mean) |
| `sigma_clearance` | number | Standard deviation of clearance (RSS of hole and shaft σ) |
| `clearance_3sigma_min` | number | Minimum clearance at 3σ (mean - 3σ) |
| `clearance_3sigma_max` | number | Maximum clearance at 3σ (mean + 3σ) |
| `probability_interference` | number | Probability of interference (clearance < 0) as percentage |
| `fit_result_3sigma` | enum | Fit classification at 3σ limits |

### Links

| Field | Type | Description |
|-------|------|-------------|
| `links.used_in_stackups` | array[EntityId] | Stackups using this mate |
| `links.verifies` | array[EntityId] | Requirements verified by this mate |

## Fit Calculation

TDT automatically calculates worst-case fit from the primary dimensions of both features. The `internal` field on each feature's dimension determines which is treated as the hole and which as the shaft:

- **Internal feature** (`internal: true`): Treated as the hole
- **External feature** (`internal: false`): Treated as the shaft

### Worst-Case Analysis

```
Auto-detection from internal field:
  if feature_a.internal AND NOT feature_b.internal:
    hole = feature_a, shaft = feature_b
  else if NOT feature_a.internal AND feature_b.internal:
    hole = feature_b, shaft = feature_a
  else:
    ERROR: Both features must have opposite internal/external designation

Calculation:
  hole_max = hole.nominal + hole.plus_tol      # LMC for hole
  hole_min = hole.nominal - hole.minus_tol     # MMC for hole
  shaft_max = shaft.nominal + shaft.plus_tol   # MMC for shaft
  shaft_min = shaft.nominal - shaft.minus_tol  # LMC for shaft

  min_clearance = hole_min - shaft_max   # MMC hole - MMC shaft
  max_clearance = hole_max - shaft_min   # LMC hole - LMC shaft

  fit_result =
    if min_clearance > 0: clearance
    else if max_clearance < 0: interference
    else: transition
```

### Statistical (RSS) Analysis

Optional statistical analysis calculates the probability of interference:

```
# Feature statistics (assuming ±3σ process, sigma_level = 6.0)
hole_mean = hole.nominal + (hole.plus_tol - hole.minus_tol) / 2
hole_sigma = (hole.plus_tol + hole.minus_tol) / sigma_level

shaft_mean = shaft.nominal + (shaft.plus_tol - shaft.minus_tol) / 2
shaft_sigma = (shaft.plus_tol + shaft.minus_tol) / sigma_level

# Clearance distribution
mean_clearance = hole_mean - shaft_mean
sigma_clearance = sqrt(hole_sigma^2 + shaft_sigma^2)

# Interference probability (using normal CDF)
P(interference) = Φ(-mean_clearance / sigma_clearance) × 100%

# 3σ limits
clearance_3sigma_min = mean_clearance - 3 * sigma_clearance
clearance_3sigma_max = mean_clearance + 3 * sigma_clearance
```

**Use when**: Need to understand interference probability for transition fits, or when worst-case analysis is too conservative.

**Important**: A mate requires one internal feature (hole) and one external feature (shaft). TDT will report an error during validation if both features have the same `internal` value.

## Example

```yaml
# Mate: Pin-Hole Mate
# Created by TDT - Tessera Design Toolkit

id: MATE-01HC2JB7SMQX7RS1Y0GFKBHPTF
title: "Pin-Hole Mate"

description: |
  Locating pin engagement with mounting hole.
  Critical for alignment accuracy.

# Features with cached info for readability
feature_a:
  id: FEAT-01HC2JB7SMQX7RS1Y0GFKBHPTE     # Hole: 10.0 +0.1/-0.05
  name: "Mounting Hole"                     # Cached from feature
  component_id: CMP-01HC2JB7SMQX7RS1Y0GFKBHPTA
  component_name: "Housing"                 # Cached for readability

feature_b:
  id: FEAT-01HC2JB7SMQX7RS1Y0GFKBHPTG     # Shaft: 9.95 +0.02/-0.02
  name: "Locating Pin"
  component_id: CMP-01HC2JB7SMQX7RS1Y0GFKBHPTB
  component_name: "Pin Assembly"

mate_type: clearance

# Auto-calculated from feature dimensions
fit_analysis:
  worst_case_min_clearance: 0.03
  worst_case_max_clearance: 0.17
  fit_result: clearance
  # Optional statistical analysis (added with --statistical flag)
  statistical:
    mean_clearance: 0.10
    sigma_clearance: 0.023
    clearance_3sigma_min: 0.031
    clearance_3sigma_max: 0.169
    probability_interference: 0.001
    fit_result_3sigma: clearance

notes: |
  Clearance fit provides easy assembly while maintaining
  adequate positional accuracy for the application.

tags: [locating, precision, alignment]
status: approved

links:
  used_in_stackups:
    - TOL-01HC2JB7SMQX7RS1Y0GFKBHPTH
  verifies:
    - REQ-01HC2JB7SMQX7RS1Y0GFKBHPTI

# Auto-managed metadata
created: 2024-01-15T10:30:00Z
author: John Smith
entity_revision: 1
```

## CLI Commands

### Create a new mate

```bash
# Create mate (--feature-a and --feature-b are REQUIRED)
tdt mate new --feature-a FEAT@1 --feature-b FEAT@2 --title "Pin-Hole Fit"

# Specify mate type
tdt mate new --feature-a FEAT@1 --feature-b FEAT@2 --type clearance

# Create with interactive wizard
tdt mate new --feature-a FEAT@1 --feature-b FEAT@2 -i

# Create and immediately edit
tdt mate new --feature-a FEAT@1 --feature-b FEAT@2 --title "New Mate" --edit
```

**Note**: Both `--feature-a` and `--feature-b` are required.

### List mates

```bash
# List all mates
tdt mate list

# Filter by mate type
tdt mate list --type clearance
tdt mate list --type interference
tdt mate list --type transition

# Filter by status
tdt mate list --status approved

# Search in title/description
tdt mate list --search "pin"

# Sort and limit
tdt mate list --sort title
tdt mate list --limit 10

# Count only
tdt mate list --count

# Output formats
tdt mate list -f json
tdt mate list -f csv
```

### Show mate details

```bash
# Show by ID (partial match supported)
tdt mate show MATE-01HC2

# Show using short ID (includes fit calculation)
tdt mate show MATE@1

# Output as JSON
tdt mate show MATE@1 -f json
```

### Recalculate fit

```bash
# Recalculate fit if feature dimensions changed
tdt mate recalc MATE@1

# Output shows updated fit analysis
# ✓ Recalculated fit for mate MATE@1
#    Result: clearance (0.0300 to 0.1700)

# Include statistical (RSS) analysis with interference probability
tdt mate recalc MATE@1 --statistical

# Output includes statistical analysis:
# ✓ Recalculated fit for mate MATE@1
#    Worst-case: clearance (0.0300 to 0.1700)
#    Statistical: mean=0.100, σ=0.023, P(interference)=0.00%

# Custom sigma level for statistical analysis
tdt mate recalc MATE@1 --statistical --sigma 4.0
```

### Edit a mate

```bash
# Open in editor
tdt mate edit MATE-01HC2

# Using short ID
tdt mate edit MATE@1
```

### Delete or archive a mate

```bash
# Permanently delete (checks for incoming links first)
tdt mate delete MATE@1

# Force delete even if referenced
tdt mate delete MATE@1 --force

# Archive instead of delete (moves to .tdt/archive/)
tdt mate archive MATE@1
```

## Fit Types

### Clearance Fit

Both min and max clearances are positive - shaft always fits freely in hole.

```
min_clearance > 0 AND max_clearance > 0
```

**Applications**: Easy assembly, sliding fits, running clearance

### Interference Fit (Press Fit)

Both min and max clearances are negative - shaft is always larger than hole.

```
min_clearance < 0 AND max_clearance < 0
```

**Applications**: Permanent assembly, torque transmission, press-fit pins

### Transition Fit

Min clearance is negative but max clearance is positive - may be either clearance or interference depending on actual dimensions.

```
min_clearance < 0 AND max_clearance > 0
```

**Applications**: Locating fits, accurate positioning with some assembly force

## ISO Fit Classifications

| Fit Type | ISO Symbol | Description |
|----------|------------|-------------|
| Loose running | H11/c11 | Large clearance for free movement |
| Free running | H9/d9 | Light running with minimal friction |
| Close running | H8/f7 | Accurate location with free movement |
| Sliding | H7/g6 | Accurate location, can slide |
| Locational clearance | H7/h6 | Accurate location, snug fit |
| Locational transition | H7/k6 | Accurate location, light press |
| Locational interference | H7/n6 | Accurate location, press fit |
| Medium drive | H7/p6 | Permanent assembly |
| Force fit | H7/s6 | High interference |

## Best Practices

### Creating Mates

1. **Set internal field** - Ensure each feature has the correct `internal` field (true for holes, false for shafts)
2. **Feature order doesn't matter** - TDT auto-detects hole vs shaft from the `internal` field
3. **Complete features first** - Ensure both features have dimensions before creating mate
4. **Verify fit type** - Check that calculated fit matches your intent
5. **Document rationale** - Explain why this fit was chosen

### Managing Mates

1. **Recalculate after changes** - Run `tdt mate recalc` after modifying features
2. **Link to requirements** - Connect to requirements that specify fit
3. **Use in stackups** - Reference mates in tolerance stackups
4. **Track status** - Update status as design matures

### Fit Selection Guidelines

| Application | Recommended Fit |
|-------------|-----------------|
| High-speed rotation | Clearance |
| Sliding/reciprocating | Clearance |
| Accurate positioning | Transition |
| Light press assembly | Transition |
| Permanent assembly | Interference |
| Torque transmission | Interference |

## Validation

Mates are validated against a JSON Schema:

```bash
# Validate all project files
tdt validate

# Validate specific file
tdt validate tolerances/mates/MATE-01HC2JB7SMQX7RS1Y0GFKBHPTF.tdt.yaml
```

### Validation Rules

1. **ID Format**: Must match `MATE-[A-Z0-9]{26}` pattern
2. **Feature A**: Required, `feature_a.id` must reference a valid FEAT entity
3. **Feature B**: Required, `feature_b.id` must reference a valid FEAT entity
4. **Feature Pairing**: One feature must be internal (hole), one must be external (shaft)
5. **Cached Info Sync**: Cached `name` and `component_id` must match actual feature values
6. **Title**: Required, 1-200 characters
7. **Mate Type**: If specified, must be valid enum
8. **Fit Result**: If specified, must be `clearance`, `interference`, or `transition`
9. **Fit Analysis Sync**: Stored `fit_analysis` must match calculated values from features
10. **Status**: Must be one of: `draft`, `review`, `approved`, `released`, `obsolete`
11. **No Additional Properties**: Unknown fields are not allowed

### Fixing Out-of-Sync Mates

If feature dimensions or metadata have changed, mates may be out of sync:

```bash
# Check for validation issues
tdt validate

# Example warnings:
# ! MATE-01HC2... - validation warning(s)
#     fit_analysis mismatch: stored (0.03 to 0.17) vs calculated (0.02 to 0.18)
#     feature_a has stale cached name 'Old Name' (feature is 'Mounting Hole')

# Auto-fix fit_analysis and cached values
tdt validate --fix
```

The `--fix` flag will:
- Recalculate and update `fit_analysis` values
- Update cached `name` and `component_id` fields to match actual features

## JSON Schema

The full JSON Schema for mates is available at:

```
tdt/schemas/mate.schema.json
```
