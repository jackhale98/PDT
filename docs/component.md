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
| `documents` | array[Document] | Related documents (drawings, specs) |
| `tags` | array[string] | Tags for filtering |
| `entity_revision` | integer | Entity revision number (default: 1) |

### Supplier Object

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | Supplier name |
| `supplier_pn` | string | Supplier's part number |
| `lead_time_days` | integer | Lead time in days |
| `moq` | integer | Minimum order quantity |
| `unit_cost` | number | Cost per unit from this supplier |

### Document Object

| Field | Type | Description |
|-------|------|-------------|
| `type` | string | Document type (drawing, spec, datasheet) |
| `path` | string | Path to document file |
| `revision` | string | Document revision |

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
  - name: "Acme Corp"
    supplier_pn: "ACM-123"
    lead_time_days: 14
    moq: 100
    unit_cost: 11.00
  - name: "Quality Parts Inc"
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
author: Jack Hale
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

### Show component interaction matrix (DSM)

Display a Design Structure Matrix showing how components interact through mates and tolerance stackups.

```bash
# Show all component interactions
tdt cmp matrix

# Filter by interaction type
tdt cmp matrix --interaction-type mate       # Only mate interactions
tdt cmp matrix --interaction-type tolerance  # Only tolerance stackup interactions

# Filter to show interactions for specific component
tdt cmp matrix --component CMP@1

# Export as CSV (recommended for large matrices)
tdt cmp matrix --csv

# Output as JSON for programmatic use
tdt cmp matrix -f json
```

**Example Output:**

```
Component Interaction Matrix
5 components, 8 interactions

              1   2   3   4   5
             ──────────────────────
Housing       ·   M   MT  T   ·
Shaft         M   ·   M   ·   ·
Bearing       MT  M   ·   T   M
Cover         T   ·   T   ·   T
Bracket       ·   ·   M   T   ·

Legend:
  M = Mate interaction
  T = Tolerance stackup
  MT = Both mate and tolerance

Components:
   1. CMP@1 Housing
   2. CMP@2 Shaft
   3. CMP@3 Bearing
   4. CMP@4 Cover
   5. CMP@5 Bracket
```

**CSV Output Example:**

```csv
"Component","Housing","Shaft","Bearing","Cover","Bracket"
"Housing","","M","MT","T",""
"Shaft","M","","M","",""
"Bearing","MT","M","","T","M"
"Cover","T","","T","","T"
"Bracket","","","M","T",""
```

**Notes:**
- The matrix is symmetric (A interacts with B = B interacts with A)
- Interactions are derived from mate definitions (`tdt mate`) and tolerance stackups (`tdt tol`)
- Use `--csv` for large matrices that don't fit in terminal width
- JSON output includes full component IDs for integration with other tools

> **See Also:** For more comprehensive component analysis, use:
> - `tdt dsm` - Design Structure Matrix including mates, processes, and requirements with clustering, metrics, and cycle detection
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
