# TDT Feature Entity (Tolerances)

This document describes the Feature entity type in TDT (Tessera Design Toolkit).

## Overview

Features represent dimensional characteristics on components that have tolerances. They are the building blocks for tolerance analysis - features can be used in mates (1:1 fits) and stackups (tolerance chains). Each feature must belong to a parent component.

## Entity Type

- **Prefix**: `FEAT`
- **File extension**: `.tdt.yaml`
- **Directory**: `tolerances/features/`

## Schema

### Required Fields

| Field | Type | Description |
|-------|------|-------------|
| `id` | string | Unique identifier (FEAT-[26-char ULID]) |
| `component` | string | Parent component ID (CMP-...) |
| `feature_type` | enum | `internal` or `external` - determines MMC/LMC calculation |
| `title` | string | Short descriptive title (1-200 chars) |
| `status` | enum | `draft`, `review`, `approved`, `released`, `obsolete` |
| `created` | datetime | Creation timestamp (ISO 8601) |
| `author` | string | Author name |

### Optional Fields

| Field | Type | Description |
|-------|------|-------------|
| `description` | string | Detailed description |
| `dimensions` | array[Dimension] | Dimensional characteristics |
| `gdt` | array[GdtControl] | GD&T controls |
| `geometry_class` | enum | Geometry class for 3D analysis (see below) |
| `datum_label` | string | Datum label (A, B, or C) if this feature is a datum |
| `geometry_3d` | Geometry3D | 3D geometry definition for kinematic chain analysis |
| `torsor_bounds` | TorsorBounds | Auto-calculated torsor bounds from tolerances |
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
| `internal` | boolean | Whether this is an internal feature (default: `false`) |
| `distribution` | enum | Statistical distribution: `normal` (default), `uniform`, `triangular` |

#### Internal vs External Features

The `internal` field determines how MMC (Maximum Material Condition) and LMC (Least Material Condition) are calculated:

| Feature Type | `internal` | MMC | LMC |
|--------------|------------|-----|-----|
| **Internal** (holes, slots, pockets) | `true` | Smallest size (`nominal - minus_tol`) | Largest size (`nominal + plus_tol`) |
| **External** (shafts, bosses) | `false` | Largest size (`nominal + plus_tol`) | Smallest size (`nominal - minus_tol`) |

This is critical for mate calculations - when validating mates, TDT uses the `internal` flag to auto-detect which feature is the hole and which is the shaft.

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

### Geometry3D Object (3D Tolerance Analysis)

| Field | Type | Description |
|-------|------|-------------|
| `origin` | array[number] | Origin point [x, y, z] in component coordinate system |
| `axis` | array[number] | Axis direction [dx, dy, dz] - unit vector for feature orientation |
| `length` | number | Optional length for axial features (cylinders, cones) |

### TorsorBounds Object (Auto-calculated)

| Field | Type | Description |
|-------|------|-------------|
| `u` | array[number] | Translation along local X [min, max] |
| `v` | array[number] | Translation along local Y [min, max] |
| `w` | array[number] | Translation along local Z [min, max] |
| `alpha` | array[number] | Rotation about local X [min, max] in radians |
| `beta` | array[number] | Rotation about local Y [min, max] in radians |
| `gamma` | array[number] | Rotation about local Z [min, max] in radians |

### GeometryClass Enum

| Value | Description | Constrained DOF |
|-------|-------------|-----------------|
| `plane` | Planar surface | w, α, β |
| `cylinder` | Cylindrical feature | u, v, α, β |
| `sphere` | Spherical feature | u, v, w |
| `cone` | Conical feature | u, v, α, β |
| `point` | Point feature | u, v, w |
| `line` | Linear feature | u, v |
| `complex` | Complex geometry | None (all free) |

## GD&T Integration with 3D Analysis

TDT's 3D tolerance analysis is designed to work seamlessly with ASME Y14.5 GD&T controls. The system automatically converts GD&T callouts to torsor bounds for chain analysis:

### How GD&T Controls Map to Torsors

| GD&T Symbol | Geometry Class | Torsor DOF Affected |
|-------------|----------------|---------------------|
| **Position** (⊕) | Cylinder | u, v (radial position) |
| **Position** (⊕) | Plane | u, v, w (planar position) |
| **Perpendicularity** (⟂) | Cylinder | α, β (angular deviation) |
| **Parallelism** (//) | Plane | α, β (angular deviation) |
| **Flatness** (⏥) | Plane | w (out-of-plane) |
| **Concentricity** (◎) | Cylinder | u, v (radial offset) |
| **Runout** (↗) | Cylinder | u, v, α, β (radial + angular) |

### Datum Reference Frames

The `datum_label` field (A, B, C) establishes the datum reference frame per ASME Y14.5:

- **Primary datum (A)**: Constrains 3 DOF (typically a plane: w, α, β)
- **Secondary datum (B)**: Constrains 2 additional DOF (typically a cylinder: u, v OR a plane: one translation + rotation)
- **Tertiary datum (C)**: Constrains 1 remaining DOF

**Example**: A typical 3-2-1 datum scheme:

```yaml
# Primary datum - bottom surface (constrains w, α, β)
geometry_class: plane
datum_label: A

# Secondary datum - locating hole (constrains u, v)
geometry_class: cylinder
datum_label: B

# Tertiary datum - slot (constrains γ)
geometry_class: plane
datum_label: C
```

### Material Modifiers in 3D Analysis

When GD&T controls specify material modifiers (MMC Ⓜ, LMC Ⓛ), bonus tolerance is automatically calculated:

```yaml
gdt:
  - symbol: position
    value: 0.25
    datum_refs: ["A", "B", "C"]
    material_condition: mmc  # Enables bonus tolerance

dimensions:
  - name: diameter
    nominal: 10.0
    plus_tol: 0.1
    minus_tol: 0.05
    internal: true  # Hole - MMC is smallest (9.95)
```

**Bonus calculation**: When the actual size departs from MMC, the position tolerance zone grows:
- MMC = 9.95mm, Actual = 10.02mm → Bonus = 0.07mm
- Effective position tolerance = 0.25 + 0.07 = 0.32mm

### Links

| Field | Type | Description |
|-------|------|-------------|
| `links.used_in_mates` | array[EntityId] | Mates using this feature |
| `links.used_in_stackups` | array[EntityId] | Stackups using this feature |
| `links.allocated_from` | array[EntityId] | Requirements allocated to this feature |
| `links.risks` | array[EntityId] | Risks affecting this feature |

## Tolerance Format

TDT uses `plus_tol` and `minus_tol` fields instead of the `±` symbol:

```yaml
# Represents: 10.0 +0.1/-0.05 for a hole (internal feature)
dimensions:
  - name: "diameter"
    nominal: 10.0
    plus_tol: 0.1     # Maximum (LMC): 10.1
    minus_tol: 0.05   # Minimum (MMC): 9.95
    units: "mm"
    internal: true    # This is a hole - MMC is smallest
    distribution: normal  # For tolerance stackup analysis
```

**Important**: Both `plus_tol` and `minus_tol` are stored as **positive numbers**.

The `distribution` field specifies the statistical distribution used when this feature is added to a tolerance stackup for Monte Carlo analysis.

## Example

```yaml
# Feature: Mounting Hole A
# Created by TDT - Tessera Design Toolkit

id: FEAT-01HC2JB7SMQX7RS1Y0GFKBHPTE
component: CMP-01HC2JB7SMQX7RS1Y0GFKBHPTD
feature_type: internal
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
    internal: true       # Hole - MMC is smallest (9.95)
    distribution: normal
  - name: "depth"
    nominal: 15.0
    plus_tol: 0.5
    minus_tol: 0.0
    units: "mm"
    internal: true       # Internal dimension
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

# 3D Geometry for SDT Analysis
geometry_class: cylinder
datum_label: B                   # This feature serves as datum B
geometry_3d:
  origin: [50.0, 25.0, 0.0]      # Position in component coordinates (mm)
  axis: [0.0, 0.0, 1.0]          # Z-axis oriented (perpendicular to surface)
  length: 15.0                   # Hole depth

# Auto-calculated torsor bounds (from tolerances + geometry_class)
torsor_bounds:
  u: [-0.125, 0.125]             # Position tol / 2
  v: [-0.125, 0.125]             # Position tol / 2
  alpha: [-0.0067, 0.0067]       # Perpendicularity / length
  beta: [-0.0067, 0.0067]        # Perpendicularity / length

drawing:
  number: "DWG-001"
  revision: "A"
  zone: "B3"

tags: [mounting, precision, datum]
status: approved

links:
  used_in_mates:
    - MATE-01HC2JB7SMQX7RS1Y0GFKBHPTF
  used_in_stackups:
    - TOL-01HC2JB7SMQX7RS1Y0GFKBHPTG

# Auto-managed metadata
created: 2024-01-15T10:30:00Z
author: John Smith
entity_revision: 1
```

## CLI Commands

### Create a new feature

```bash
# Create internal feature (hole, pocket, slot) - component is REQUIRED
tdt feat new --component CMP@1 --feature-type internal --title "Mounting Hole A"

# Create external feature (shaft, boss, pin)
tdt feat new --component CMP@1 --feature-type external --title "Locating Pin"

# Create with interactive wizard
tdt feat new --component CMP@1 -i

# Create and immediately edit
tdt feat new --component CMP@1 --title "New Feature" --edit
```

**Note**: The `--component` flag is required. Features cannot exist without a parent component.

### List features

```bash
# List all features
tdt feat list

# Filter by component
tdt feat list --component CMP@1

# Filter by type
tdt feat list --feature-type internal
tdt feat list --feature-type external

# Filter by status
tdt feat list --status approved

# Search in title/description
tdt feat list --search "mounting"

# Sort and limit
tdt feat list --sort title
tdt feat list --limit 10

# Count only
tdt feat list --count

# Output formats
tdt feat list -f json
tdt feat list -f csv
```

### Show feature details

```bash
# Show by ID (partial match supported)
tdt feat show FEAT-01HC2

# Show using short ID
tdt feat show FEAT@1

# Output as JSON
tdt feat show FEAT@1 -f json
```

### Edit a feature

```bash
# Open in editor
tdt feat edit FEAT-01HC2

# Using short ID
tdt feat edit FEAT@1
```

### Delete or archive a feature

```bash
# Permanently delete (checks for incoming links first)
tdt feat delete FEAT@1

# Force delete even if referenced
tdt feat delete FEAT@1 --force

# Archive instead of delete (moves to .tdt/archive/)
tdt feat archive FEAT@1
```

## Feature Types

Features are classified by their material behavior for tolerance analysis:

| Type | Description | MMC | LMC | Examples |
|------|-------------|-----|-----|----------|
| **internal** | Material is removed | Smallest size | Largest size | Holes, bores, pockets, slots, counterbores |
| **external** | Material remains | Largest size | Smallest size | Shafts, pins, bosses |

This classification determines how Maximum Material Condition (MMC) and Least Material Condition (LMC) are calculated, which is critical for:

- **Fit calculations** (clearance, interference, transition fits)
- **Tolerance stackups** (worst-case and statistical analysis)
- **Bonus tolerance** calculations with GD&T

Specific geometry (counterbore, thread, etc.) can be documented in the feature title or description.

## GD&T Symbols

| Symbol | Description | Use |
|--------|-------------|-----|
| **position** | True position | Hole/pin location |
| **flatness** | Flatness | Surface form |
| **perpendicularity** | Perpendicularity | Angular orientation |
| **parallelism** | Parallelism | Angular orientation |
| **angularity** | Angularity | Angular orientation |
| **concentricity** | Concentricity | Axis alignment |
| **symmetry** | Symmetry | Median plane alignment |
| **runout** | Runout | Rotation about axis |
| **total_runout** | Total runout | Full rotation about axis |
| **circularity** | Circularity | Round cross-section |
| **cylindricity** | Cylindricity | Cylinder form |
| **straightness** | Straightness | Line form |
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
tdt validate

# Validate specific file
tdt validate tolerances/features/FEAT-01HC2JB7SMQX7RS1Y0GFKBHPTE.tdt.yaml
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
tdt/schemas/feat.schema.json
```
