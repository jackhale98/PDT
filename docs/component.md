# TDT Component Entity (BOM)

This document describes the Component entity type in TDT (Tessera Design Toolkit).

## Overview

Components represent individual parts in your Bill of Materials (BOM). They can be either manufactured internally (make) or purchased from suppliers (buy). Components track part numbers, suppliers, materials, and costs.

## Entity Type

- **Prefix**: `CMP`
- **File extension**: `.tdt.yaml`
- **Directory**: `bom/components/`

## Schema

### Required Fields

| Field | Type | Description |
|-------|------|-------------|
| `id` | string | Unique identifier (CMP-[26-char ULID]) |
| `title` | string | Short descriptive title (1-200 chars) |
| `status` | enum | `draft`, `review`, `approved`, `released`, `obsolete` |
| `created` | datetime | Creation timestamp (ISO 8601) |
| `author` | string | Author name |

### Optional Fields

| Field | Type | Description |
|-------|------|-------------|
| `part_number` | string | Company part number |
| `revision` | string | Part revision (e.g., "A", "B") |
| `description` | string | Detailed description |
| `make_buy` | enum | `make` or `buy` |
| `category` | enum | `mechanical`, `electrical`, `software`, `fastener`, `consumable` |
| `material` | string | Material specification |
| `mass_kg` | number | Mass in kilograms |
| `unit_cost` | number | Cost per unit |
| `suppliers` | array[Supplier] | List of approved suppliers |
| `manufacturing` | ManufacturingConfig | Manufacturing routing and settings (for "make" items) |
| `documents` | array[Document] | Related documents (drawings, specs) |
| `coordinate_system` | CoordinateSystem | Component coordinate system for 3D analysis |
| `datum_frame` | DatumFrame | Datum reference frame (auto-populated from features) |
| `tags` | array[string] | Tags for filtering |
| `entity_revision` | integer | Entity revision number (default: 1) |

### Supplier Object

| Field | Type | Description |
|-------|------|-------------|
| `supplier_id` | string | Supplier entity ID (SUP-... or SUP@N) - links to SUP entity |
| `name` | string | Supplier name (fallback if supplier_id not set) |
| `supplier_pn` | string | Supplier's part number |
| `lead_time_days` | integer | Lead time in days |
| `moq` | integer | Minimum order quantity |
| `unit_cost` | number | Cost per unit from this supplier |

> **Note:** Use `supplier_id` to link to SUP entities for full traceability. The `name` field is optional and can be used as a display name or fallback.

### Document Object

| Field | Type | Description |
|-------|------|-------------|
| `type` | string | Document type (drawing, spec, datasheet) |
| `path` | string | Path to document file |
| `revision` | string | Document revision |

### ManufacturingConfig Object

| Field | Type | Description |
|-------|------|-------------|
| `routing` | array[EntityId] | Ordered list of PROC IDs for manufacturing |
| `work_cell` | string | Default work cell/location |

### CoordinateSystem Object (3D Tolerance Analysis)

Defines the component's local coordinate system for 3D tolerance analysis. Used to transform feature positions into assembly coordinates.

| Field | Type | Description |
|-------|------|-------------|
| `origin` | array[number] | Origin point [x, y, z] in assembly coordinates |
| `x_axis` | array[number] | X-axis direction [dx, dy, dz] - unit vector |
| `z_axis` | array[number] | Z-axis direction [dx, dy, dz] - unit vector |

**Note**: The Y-axis is computed as the cross product of Z and X axes (right-hand rule).

**Example**:
```yaml
coordinate_system:
  origin: [0.0, 0.0, 0.0]       # Component origin in assembly
  x_axis: [1.0, 0.0, 0.0]       # Standard X direction
  z_axis: [0.0, 0.0, 1.0]       # Standard Z direction (up)
```

### DatumFrame Object (Auto-populated)

The datum reference frame is automatically populated from features that have `datum_label` set (A, B, or C). This follows ASME Y14.5 datum hierarchy.

| Field | Type | Description |
|-------|------|-------------|
| `a` | EntityId | Primary datum feature (A) - constrains 3 DOF |
| `b` | EntityId | Secondary datum feature (B) - constrains 2 DOF |
| `c` | EntityId | Tertiary datum feature (C) - constrains 1 DOF |

**Example**:
```yaml
# Auto-populated from features with datum_label: A, B, C
datum_frame:
  a: FEAT-01HC2JB7SMQX7RS1Y0GFKBHPTA  # Bottom surface (plane)
  b: FEAT-01HC2JB7SMQX7RS1Y0GFKBHPTB  # Locating hole (cylinder)
  c: FEAT-01HC2JB7SMQX7RS1Y0GFKBHPTC  # Orientation slot (plane)
```

### Links

| Field | Type | Description |
|-------|------|-------------|
| `links.related_to` | array[EntityId] | Related entities |
| `links.used_in` | array[EntityId] | Assemblies using this component |

## Example

```yaml
# Component: Widget Bracket
# Created by TDT - Tessera Design Toolkit

id: CMP-01HC2JB7SMQX7RS1Y0GFKBHPTD
part_number: "PN-001"
revision: "A"
title: "Widget Bracket"

description: |
  Aluminum bracket for mounting the main widget assembly.
  Heat treated for increased strength.

make_buy: buy
category: mechanical
material: "6061-T6 Aluminum"
mass_kg: 0.125
unit_cost: 12.50

suppliers:
  - supplier_id: SUP-01HC2JB7SMQX7RS1Y0GFKBHPTS  # Links to Acme Corp supplier entity
    supplier_pn: "ACM-123"
    lead_time_days: 14
    moq: 100
    unit_cost: 11.00
  - supplier_id: SUP-01HC2JB7SMQX7RS1Y0GFKBHPTT  # Links to Quality Parts supplier entity
    name: "Quality Parts Inc"  # Optional display name
    supplier_pn: "QP-456"
    lead_time_days: 21
    moq: 50
    unit_cost: 13.50

documents:
  - type: "drawing"
    path: "drawings/PN-001.pdf"
    revision: "A"
  - type: "spec"
    path: "specs/material-6061-T6.pdf"
    revision: "B"

tags: [mechanical, bracket, aluminum]
status: approved

links:
  related_to: []
  used_in:
    - ASM-01HC2JB7SMQX7RS1Y0GFKBHPTE

# Auto-managed metadata
created: 2024-01-15T10:30:00Z
author: John Smith
entity_revision: 1
```

## CLI Commands

### Create a new component

```bash
# Create with default template
tdt cmp new

# Create with title and part number
tdt cmp new --title "Widget Bracket" --part-number "PN-001"

# Create buy part with category
tdt cmp new --title "Resistor 10K" --make-buy buy --category electrical

# Create make part
tdt cmp new --title "Custom Housing" --make-buy make --category mechanical

# Create with interactive wizard
tdt cmp new -i

# Create and immediately edit
tdt cmp new --title "New Part" --edit
```

### List components

```bash
# List all components
tdt cmp list

# Filter by make/buy
tdt cmp list --make-buy buy
tdt cmp list --make-buy make

# Filter by category
tdt cmp list --category mechanical
tdt cmp list --category electrical

# Filter by status
tdt cmp list --status approved
tdt cmp list --status draft

# Search in title/description
tdt cmp list --search "bracket"

# Filter to components within an assembly's BOM (recursive)
tdt cmp list --assembly ASM@1

# Sort and limit
tdt cmp list --sort title
tdt cmp list --limit 10

# Count only
tdt cmp list --count

# Output formats
tdt cmp list -f json
tdt cmp list -f csv
tdt cmp list -f md
```

### Show component details

```bash
# Show by ID (partial match supported)
tdt cmp show CMP-01HC2

# Show using short ID
tdt cmp show CMP@1

# Output as JSON
tdt cmp show CMP@1 -f json

# Output as YAML
tdt cmp show CMP@1 -f yaml
```

### Edit a component

```bash
# Open in editor
tdt cmp edit CMP-01HC2

# Using short ID
tdt cmp edit CMP@1
```

### Delete or archive a component

```bash
# Permanently delete (checks for incoming links first)
tdt cmp delete CMP@1

# Force delete even if referenced
tdt cmp delete CMP@1 --force

# Archive instead of delete (moves to .tdt/archive/)
tdt cmp archive CMP@1
```

### Manage manufacturing routing (for "make" items)

Define the sequence of manufacturing processes for a component:

```bash
# Add a process to the routing
tdt cmp routing add CMP@1 PROC@1

# Add at specific position (1-indexed)
tdt cmp routing add CMP@1 PROC@2 --position 1

# Remove a process by position
tdt cmp routing rm CMP@1 1

# Remove by process ID
tdt cmp routing rm CMP@1 PROC@1

# List current routing
tdt cmp routing list CMP@1

# Set complete routing (replaces existing)
tdt cmp routing set CMP@1 PROC@1 PROC@2 PROC@3
```

**Note:** Manufacturing routing is typically used for "make" items. For "buy" items, supplier information is more relevant.

### Analyze component interactions

Use the Design Structure Matrix (DSM) to analyze component relationships:

```bash
# Show all component interactions
tdt dsm

# Filter by interaction type
tdt dsm -t mate       # Only mate interactions
tdt dsm -t tolerance  # Only tolerance stackup interactions

# Apply clustering and show metrics
tdt dsm --cluster --metrics
```

> **See Also:**
> - `tdt dsm` - Full Design Structure Matrix with clustering, metrics, and cycle detection
> - `tdt dmm cmp req` - Domain Mapping Matrix showing component-to-requirement allocation

## Make vs Buy Classification

| Type | Description | Typical Use |
|------|-------------|-------------|
| **make** | Manufactured internally | Custom parts, assemblies |
| **buy** | Purchased from suppliers | Standard parts, COTS |

## Category Classification

| Category | Description | Examples |
|----------|-------------|----------|
| **mechanical** | Mechanical parts | Brackets, housings, shafts |
| **electrical** | Electrical components | Resistors, capacitors, ICs |
| **software** | Software components | Firmware, licenses |
| **fastener** | Fastening hardware | Screws, nuts, bolts |
| **consumable** | Consumable items | Adhesives, lubricants |

## Best Practices

### Part Numbering

1. **Use consistent format** - Establish a part numbering scheme
2. **Include revision** - Track design revisions
3. **Avoid special characters** - Stick to alphanumeric
4. **Be meaningful** - Include category prefix if helpful

### Supplier Management

1. **Multiple suppliers** - Have backup sources for critical parts
2. **Track lead times** - Plan procurement around lead times
3. **Document MOQs** - Consider minimum order quantities
4. **Compare costs** - Track unit costs across suppliers

### Documentation

1. **Link drawings** - Reference 2D drawings with revisions
2. **Include specs** - Link material and process specs
3. **Track revisions** - Keep document revisions in sync

## Validation

Components are validated against a JSON Schema:

```bash
# Validate all project files
tdt validate

# Validate specific file
tdt validate bom/components/CMP-01HC2JB7SMQX7RS1Y0GFKBHPTD.tdt.yaml
```

### Validation Rules

1. **ID Format**: Must match `CMP-[A-Z0-9]{26}` pattern
2. **Title**: Required, 1-200 characters
3. **Make/Buy**: If specified, must be `make` or `buy`
4. **Category**: If specified, must be valid enum value
5. **Status**: Must be one of: `draft`, `review`, `approved`, `released`, `obsolete`
6. **No Additional Properties**: Unknown fields are not allowed

## JSON Schema

The full JSON Schema for components is available at:

```
tdt/schemas/cmp.schema.json
```
