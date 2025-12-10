# PDT Feature Entity (Tolerances)

This document describes the Feature entity type in PDT (Plain-text Product Development Toolkit).

## Overview

Features represent dimensional characteristics on components that have tolerances. They are the building blocks for tolerance analysis - features can be used in mates (1:1 fits) and stackups (tolerance chains). Each feature must belong to a parent component.

## Entity Type

- **Prefix**: `FEAT`
- **File extension**: `.pdt.yaml`
- **Directory**: `tolerances/features/`

## Schema

### Required Fields

| Field | Type | Description |
|-------|------|-------------|
| `id` | string | Unique identifier (FEAT-[26-char ULID]) |
| `component` | string | Parent component ID (CMP-...) - **REQUIRED** |
| `title` | string | Short descriptive title (1-200 chars) |
| `status` | enum | `draft`, `review`, `approved`, `released`, `obsolete` |
| `created` | datetime | Creation timestamp (ISO 8601) |
| `author` | string | Author name |

### Optional Fields

| Field | Type | Description |
|-------|------|-------------|
| `feature_type` | enum | `hole`, `shaft`, `planar_surface`, `slot`, `thread`, `counterbore`, `countersink` |
| `description` | string | Detailed description |
| `dimensions` | array[Dimension] | Dimensional characteristics |
| `gdt` | array[GdtControl] | GD&T controls |
| `drawing` | DrawingRef | Drawing reference |
| `tags` | array[string] | Tags for filtering |
| `entity_revision` | integer | Entity revision number (default: 1) |

### Dimension Object

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | Dimension name (e.g., "diameter", "length") |
| `nominal` | number | Nominal value |
| `plus_tol` | number | Plus tolerance (positive number) |
| `minus_tol` | number | Minus tolerance (positive number) |
| `units` | string | Units (default: "mm") |
| `distribution` | enum | Statistical distribution: `normal` (default), `uniform`, `triangular` |

### GdtControl Object

| Field | Type | Description |
|-------|------|-------------|
| `symbol` | enum | `position`, `flatness`, `perpendicularity`, `parallelism`, `concentricity`, `runout`, `profile_surface`, `profile_line` |
| `value` | number | Tolerance value |
| `units` | string | Units |
| `datum_refs` | array[string] | Datum references (e.g., ["A", "B", "C"]) |
| `material_condition` | enum | `mmc`, `lmc`, `rfs` |

### DrawingRef Object

| Field | Type | Description |
|-------|------|-------------|
| `number` | string | Drawing number |
| `revision` | string | Drawing revision |
| `zone` | string | Drawing zone (e.g., "B3") |

### Links

| Field | Type | Description |
|-------|------|-------------|
| `links.used_in_mates` | array[EntityId] | Mates using this feature |
| `links.used_in_stackups` | array[EntityId] | Stackups using this feature |

## Tolerance Format

PDT uses `plus_tol` and `minus_tol` fields instead of the `±` symbol:

```yaml
# Represents: 10.0 +0.1/-0.05
dimensions:
  - name: "diameter"
    nominal: 10.0
    plus_tol: 0.1     # Maximum: 10.1
    minus_tol: 0.05   # Minimum: 9.95
    units: "mm"
    distribution: normal  # For tolerance stackup analysis
```

**Important**: Both `plus_tol` and `minus_tol` are stored as **positive numbers**.

The `distribution` field specifies the statistical distribution used when this feature is added to a tolerance stackup for Monte Carlo analysis.

## Example

```yaml
# Feature: Mounting Hole A
# Created by PDT - Plain-text Product Development Toolkit

id: FEAT-01HC2JB7SMQX7RS1Y0GFKBHPTE
component: CMP-01HC2JB7SMQX7RS1Y0GFKBHPTD
feature_type: hole
title: "Mounting Hole A"

description: |
  Primary mounting hole for locating the bracket.
  Reamed for precision fit.

dimensions:
  - name: "diameter"
    nominal: 10.0
    plus_tol: 0.1
    minus_tol: 0.05
    units: "mm"
    distribution: normal
  - name: "depth"
    nominal: 15.0
    plus_tol: 0.5
    minus_tol: 0.0
    units: "mm"
    distribution: normal

gdt:
  - symbol: position
    value: 0.25
    units: "mm"
    datum_refs: ["A", "B", "C"]
    material_condition: mmc
  - symbol: perpendicularity
    value: 0.1
    units: "mm"
    datum_refs: ["A"]
    material_condition: rfs

drawing:
  number: "DWG-001"
  revision: "A"
  zone: "B3"

tags: [mounting, precision]
status: approved

links:
  used_in_mates:
    - MATE-01HC2JB7SMQX7RS1Y0GFKBHPTF
  used_in_stackups:
    - TOL-01HC2JB7SMQX7RS1Y0GFKBHPTG

# Auto-managed metadata
created: 2024-01-15T10:30:00Z
author: Jack Hale
entity_revision: 1
```

## CLI Commands

### Create a new feature

```bash
# Create feature (--component is REQUIRED)
pdt feat new --component CMP@1 --type hole --title "Mounting Hole A"

# Create shaft feature
pdt feat new --component CMP@1 --type shaft --title "Locating Pin"

# Create with interactive wizard
pdt feat new --component CMP@1 -i

# Create and immediately edit
pdt feat new --component CMP@1 --title "New Feature" --edit
```

**Note**: The `--component` flag is required. Features cannot exist without a parent component.

### List features

```bash
# List all features
pdt feat list

# Filter by component
pdt feat list --component CMP@1

# Filter by type
pdt feat list --type hole
pdt feat list --type shaft

# Filter by status
pdt feat list --status approved

# Search in title/description
pdt feat list --search "mounting"

# Sort and limit
pdt feat list --sort title
pdt feat list --limit 10

# Count only
pdt feat list --count

# Output formats
pdt feat list -f json
pdt feat list -f csv
```

### Show feature details

```bash
# Show by ID (partial match supported)
pdt feat show FEAT-01HC2

# Show using short ID
pdt feat show FEAT@1

# Output as JSON
pdt feat show FEAT@1 -f json
```

### Edit a feature

```bash
# Open in editor
pdt feat edit FEAT-01HC2

# Using short ID
pdt feat edit FEAT@1
```

## Feature Types

| Type | Description | Typical Dimensions |
|------|-------------|-------------------|
| **hole** | Cylindrical hole | diameter, depth |
| **shaft** | Cylindrical shaft | diameter, length |
| **planar_surface** | Flat surface | flatness, parallelism |
| **slot** | Linear slot | width, length, depth |
| **thread** | Threaded feature | major diameter, pitch |
| **counterbore** | Counterbored hole | bore diameter, depth |
| **countersink** | Countersunk hole | cone angle, depth |

## GD&T Symbols

| Symbol | Description | Use |
|--------|-------------|-----|
| **position** | True position | Hole/pin location |
| **flatness** | Flatness | Surface form |
| **perpendicularity** | Perpendicularity | Angular orientation |
| **parallelism** | Parallelism | Angular orientation |
| **concentricity** | Concentricity | Axis alignment |
| **runout** | Runout | Rotation about axis |
| **profile_surface** | Profile of surface | 3D surface form |
| **profile_line** | Profile of line | 2D cross-section |

## Material Conditions

| Condition | Symbol | Description |
|-----------|--------|-------------|
| **mmc** | Ⓜ | Maximum Material Condition |
| **lmc** | Ⓛ | Least Material Condition |
| **rfs** | (none) | Regardless of Feature Size |

## Best Practices

### Defining Features

1. **One feature per characteristic** - Don't combine multiple features
2. **Complete dimensions** - Include all relevant dimensions
3. **Reference drawings** - Link to the source drawing
4. **Use GD&T** - Add geometric controls where applicable

### Tolerance Specification

1. **Realistic tolerances** - Don't over-specify
2. **Process capability** - Match tolerances to process capability
3. **Functional requirements** - Derive tolerances from function
4. **Inspection capability** - Consider how tolerances will be verified

### Organizing Features

1. **Group by component** - Features belong to components
2. **Use meaningful names** - "Mounting Hole A" vs "Hole 1"
3. **Use tags** - Enable filtering across features
4. **Track usage** - Monitor which mates/stackups use each feature

## Validation

Features are validated against a JSON Schema:

```bash
# Validate all project files
pdt validate

# Validate specific file
pdt validate tolerances/features/FEAT-01HC2JB7SMQX7RS1Y0GFKBHPTE.pdt.yaml
```

### Validation Rules

1. **ID Format**: Must match `FEAT-[A-Z0-9]{26}` pattern
2. **Component**: Required, must be valid CMP ID
3. **Title**: Required, 1-200 characters
4. **Feature Type**: If specified, must be valid enum
5. **Tolerances**: `plus_tol` and `minus_tol` must be >= 0
6. **Status**: Must be one of: `draft`, `review`, `approved`, `released`, `obsolete`
7. **No Additional Properties**: Unknown fields are not allowed

## JSON Schema

The full JSON Schema for features is available at:

```
pdt/schemas/feat.schema.json
```
