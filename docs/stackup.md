# PDT Stackup Entity (Tolerance Analysis)

This document describes the Stackup entity type in PDT (Plain-text Product Development Toolkit).

## Overview

Stackups represent tolerance chain analyses with multiple dimensional contributors. They calculate whether a target dimension (like a gap or clearance) will meet specification limits given the tolerances of all contributing features. PDT supports three analysis methods: worst-case, RSS (statistical), and Monte Carlo simulation.

## Entity Type

- **Prefix**: `TOL`
- **File extension**: `.pdt.yaml`
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
| `analysis_results` | AnalysisResults | Auto-calculated results |
| `disposition` | enum | `under_review`, `approved`, `rejected` |
| `tags` | array[string] | Tags for filtering |
| `entity_revision` | integer | Entity revision number (default: 1) |

### Contributor Object

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | Contributor name/description |
| `feature_id` | string | Optional reference to FEAT entity |
| `direction` | enum | `positive` or `negative` |
| `nominal` | number | Nominal value |
| `plus_tol` | number | Plus tolerance (positive number) |
| `minus_tol` | number | Minus tolerance (positive number) |
| `distribution` | enum | `normal`, `uniform`, `triangular` |
| `source` | string | Source reference (drawing, etc.) |

### AnalysisResults Object (Auto-calculated)

| Field | Type | Description |
|-------|------|-------------|
| `worst_case` | WorstCaseResult | Worst-case analysis |
| `rss` | RssResult | RSS statistical analysis |
| `monte_carlo` | MonteCarloResult | Monte Carlo simulation |

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
# Created by PDT - Plain-text Product Development Toolkit

id: TOL-01HC2JB7SMQX7RS1Y0GFKBHPTH
title: "Gap Analysis"

description: |
  Analysis of the gap between the housing and cover.
  Gap must be maintained for thermal expansion allowance.

target:
  name: "Gap"
  nominal: 1.0
  upper_limit: 1.5
  lower_limit: 0.5
  units: "mm"
  critical: true

contributors:
  - name: "Housing Depth"
    feature_id: FEAT-01HC2JB7SMQX7RS1Y0GFKBHPTE
    direction: positive
    nominal: 50.0
    plus_tol: 0.1
    minus_tol: 0.1
    distribution: normal
    source: "DWG-001 Rev A"

  - name: "Cover Height"
    feature_id: FEAT-01HC2JB7SMQX7RS1Y0GFKBHPTF
    direction: negative
    nominal: 45.0
    plus_tol: 0.08
    minus_tol: 0.08
    distribution: normal
    source: "DWG-002 Rev A"

  - name: "Gasket Thickness"
    direction: negative
    nominal: 2.0
    plus_tol: 0.15
    minus_tol: 0.10
    distribution: uniform
    source: "Vendor Spec"

  - name: "Bracket Height"
    direction: negative
    nominal: 2.0
    plus_tol: 0.05
    minus_tol: 0.05
    distribution: normal
    source: "DWG-003 Rev A"

# Auto-calculated by 'pdt tol analyze'
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
    cpk: 1.59
    yield_percent: 99.9997
  monte_carlo:
    iterations: 10000
    mean: 1.0
    std_dev: 0.07
    min: 0.71
    max: 1.28
    yield_percent: 100.0
    percentile_2_5: 0.86
    percentile_97_5: 1.14

disposition: approved
tags: [critical, thermal, assembly]
status: approved

links:
  verifies:
    - REQ-01HC2JB7SMQX7RS1Y0GFKBHPTI
  mates_used: []

# Auto-managed metadata
created: 2024-01-15T10:30:00Z
author: Jack Hale
entity_revision: 1
```

## CLI Commands

### Create a new stackup

```bash
# Create with target specification
pdt tol new --title "Gap Analysis" --target-nominal 1.0 --target-upper 1.5 --target-lower 0.5

# Specify target name
pdt tol new --title "Gap Analysis" --target-name "Gap" \
    --target-nominal 1.0 --target-upper 1.5 --target-lower 0.5

# Mark as critical
pdt tol new --title "Critical Gap" --target-nominal 1.0 \
    --target-upper 1.5 --target-lower 0.5 --critical

# Create with interactive wizard
pdt tol new -i

# Create and immediately edit
pdt tol new --title "New Stackup" --edit
```

### List stackups

```bash
# List all stackups
pdt tol list

# Filter by worst-case result
pdt tol list --result pass
pdt tol list --result fail
pdt tol list --result marginal

# Filter by disposition
pdt tol list --disposition approved
pdt tol list --disposition rejected

# Show only critical stackups
pdt tol list --critical

# Filter by status
pdt tol list --status approved

# Search in title/description
pdt tol list --search "gap"

# Sort and limit
pdt tol list --sort title
pdt tol list --limit 10

# Count only
pdt tol list --count

# Output formats
pdt tol list -f json
pdt tol list -f csv
```

### Show stackup details

```bash
# Show by ID (includes analysis results)
pdt tol show TOL-01HC2

# Show using short ID
pdt tol show TOL@1

# Output as JSON
pdt tol show TOL@1 -f json
```

### Run analysis

```bash
# Run all analyses (worst-case, RSS, Monte Carlo)
pdt tol analyze TOL@1

# Custom Monte Carlo iterations
pdt tol analyze TOL@1 --iterations 50000

# Verbose output
pdt tol analyze TOL@1 --verbose
```

### Add features as contributors

```bash
# Add features with direction prefix
# Use + for positive direction, ~ for negative
# Distribution is pulled from the feature's dimension
pdt tol add TOL@1 +FEAT@1 ~FEAT@2 +FEAT@3

# Specify which dimension to use (default: first dimension)
pdt tol add TOL@1 --dimension length +FEAT@1

# Run analysis after adding
pdt tol add TOL@1 --analyze +FEAT@1 ~FEAT@2
```

### Remove contributors

```bash
# Remove contributor(s) by feature ID
pdt tol rm TOL@1 FEAT@1

# Remove multiple contributors
pdt tol rm TOL@1 FEAT@1 FEAT@2
```

### Edit a stackup

```bash
# Open in editor
pdt tol edit TOL-01HC2

# Using short ID
pdt tol edit TOL@1
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

Statistical analysis assuming normal distributions with 3-sigma process:

```
mean = sum of (sign * nominal)

For each contributor:
  sigma = (plus_tol + minus_tol) / 6  # Assume 3-sigma process
  variance += sigma^2

sigma_total = sqrt(variance)
sigma_3 = 3 * sigma_total

Cpk = min(USL - mean, mean - LSL) / (3 * sigma_total)
```

**Cpk Guidelines**:

| Cpk | Sigma Level | Yield | Quality |
|-----|-------------|-------|---------|
| 0.33 | 1σ | 68.27% | Poor |
| 0.67 | 2σ | 95.45% | Marginal |
| 1.0 | 3σ | 99.73% | Capable |
| 1.33 | 4σ | 99.99% | Good |
| 1.67 | 5σ | 99.9997% | Excellent |
| 2.0 | 6σ | 99.9999% | Six Sigma |

**Use when**: Statistical process control is in place, many contributors

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
```

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
2. **Track disposition** - Document approval/rejection decisions
3. **Link to requirements** - Connect to requirements being verified
4. **Mark critical dimensions** - Flag safety/function-critical stackups

## Validation

Stackups are validated against a JSON Schema:

```bash
# Validate all project files
pdt validate

# Validate specific file
pdt validate tolerances/stackups/TOL-01HC2JB7SMQX7RS1Y0GFKBHPTH.pdt.yaml
```

### Validation Rules

1. **ID Format**: Must match `TOL-[A-Z0-9]{26}` pattern
2. **Title**: Required, 1-200 characters
3. **Target**: Required with name, nominal, upper_limit, lower_limit
4. **Contributors**: Must have name, nominal, plus_tol, minus_tol
5. **Direction**: Must be `positive` or `negative`
6. **Distribution**: Must be `normal`, `uniform`, or `triangular`
7. **Disposition**: Must be `under_review`, `approved`, or `rejected`
8. **Status**: Must be one of: `draft`, `review`, `approved`, `released`, `obsolete`
9. **No Additional Properties**: Unknown fields are not allowed

## JSON Schema

The full JSON Schema for stackups is available at:

```
pdt/schemas/tol.schema.json
```
