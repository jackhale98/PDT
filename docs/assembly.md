# TDT Assembly Entity (BOM)

This document describes the Assembly entity type in TDT (Tessera Design Toolkit).

## Overview

Assemblies represent collections of components and sub-assemblies in your Bill of Materials (BOM). They track part numbers, BOM quantities, and hierarchical structure. Assemblies can contain other assemblies (sub-assemblies) to create a multi-level BOM.

## Entity Type

- **Prefix**: `ASM`
- **File extension**: `.tdt.yaml`
- **Directory**: `bom/assemblies/`

## Schema

### Required Fields

| Field | Type | Description |
|-------|------|-------------|
| `id` | string | Unique identifier (ASM-[26-char ULID]) |
| `title` | string | Short descriptive title (1-200 chars) |
| `status` | enum | `draft`, `review`, `approved`, `released`, `obsolete` |
| `created` | datetime | Creation timestamp (ISO 8601) |
| `author` | string | Author name |

### Optional Fields

| Field | Type | Description |
|-------|------|-------------|
| `part_number` | string | Assembly part number |
| `revision` | string | Assembly revision (e.g., "A", "B") |
| `description` | string | Detailed description |
| `bom` | array[BomItem] | List of components in this assembly |
| `subassemblies` | array[string] | IDs of sub-assemblies |
| `documents` | array[Document] | Related documents |
| `tags` | array[string] | Tags for filtering |
| `entity_revision` | integer | Entity revision number (default: 1) |

### BomItem Object

| Field | Type | Description |
|-------|------|-------------|
| `component_id` | string | Component ID (CMP-...) |
| `quantity` | integer | Quantity used in this assembly |
| `reference_designators` | array[string] | Reference designators (e.g., R1, R2) |
| `notes` | string | Assembly notes (e.g., "Use thread locker") |

### Document Object

| Field | Type | Description |
|-------|------|-------------|
| `type` | string | Document type (drawing, spec) |
| `path` | string | Path to document file |
| `revision` | string | Document revision |

### Links

| Field | Type | Description |
|-------|------|-------------|
| `links.related_to` | array[EntityId] | Related entities |
| `links.parent` | EntityId | Parent assembly (if sub-assembly) |

## Example

```yaml
# Assembly: Main Assembly
# Created by TDT - Tessera Design Toolkit

id: ASM-01HC2JB7SMQX7RS1Y0GFKBHPTE
part_number: "ASM-001"
revision: "A"
title: "Main Assembly"

description: |
  Top-level product assembly containing all major sub-systems.
  Assembly weight: 2.5 kg

bom:
  - component_id: CMP-01HC2JB7SMQX7RS1Y0GFKBHPTD
    quantity: 4
    reference_designators: ["BRK1", "BRK2", "BRK3", "BRK4"]
    notes: "Use thread locker on mounting screws"
  - component_id: CMP-01HC2JB7SMQX7RS1Y0GFKBHPTF
    quantity: 8
    reference_designators: []
    notes: "M4x10 screws"
  - component_id: CMP-01HC2JB7SMQX7RS1Y0GFKBHPTG
    quantity: 1
    reference_designators: ["PCB1"]
    notes: "Handle with ESD precautions"

subassemblies:
  - ASM-01HC2JB7SMQX7RS1Y0GFKBHPTH  # Power sub-assembly
  - ASM-01HC2JB7SMQX7RS1Y0GFKBHPTI  # Sensor sub-assembly

documents:
  - type: "drawing"
    path: "drawings/ASM-001.pdf"
    revision: "A"
  - type: "assembly_instructions"
    path: "docs/ASM-001-instructions.pdf"
    revision: "A"

tags: [top-level, product]
status: approved

links:
  related_to:
    - REQ-01HC2JB7SMQX7RS1Y0GFKBHPTJ
  parent: null

# Auto-managed metadata
created: 2024-01-15T10:30:00Z
author: John Smith
entity_revision: 1
```

## CLI Commands

### Create a new assembly

```bash
# Create with default template
tdt asm new

# Create with title and part number
tdt asm new --title "Main Assembly" --part-number "ASM-001"

# Create with interactive wizard
tdt asm new -i

# Create and immediately edit
tdt asm new --title "New Assembly" --edit
```

### List assemblies

```bash
# List all assemblies
tdt asm list

# Filter by status
tdt asm list --status approved
tdt asm list --status draft

# Search in title/description
tdt asm list --search "power"

# Filter to sub-assemblies within a parent assembly (recursive)
tdt asm list --assembly ASM@1

# Sort and limit
tdt asm list --sort title
tdt asm list --limit 10

# Count only
tdt asm list --count

# Output formats
tdt asm list -f json
tdt asm list -f csv
tdt asm list -f md
```

### Show assembly details

```bash
# Show by ID (partial match supported)
tdt asm show ASM-01HC2

# Show using short ID
tdt asm show ASM@1

# Output as JSON
tdt asm show ASM@1 -f json

# Output as YAML
tdt asm show ASM@1 -f yaml
```

### Show flattened BOM

```bash
# Show all components with quantities
tdt asm bom ASM@1

# Recursively expand sub-assemblies
tdt asm bom ASM@1 --recursive

# Output as CSV for import
tdt asm bom ASM@1 -f csv
```

### Calculate BOM cost

Calculate total cost for an assembly, optionally including NRE/tooling:

```bash
# Basic cost calculation
tdt asm cost ASM@1

# Cost at production quantity (uses price breaks from selected quotes)
tdt asm cost ASM@1 --qty 1000

# Show cost breakdown by component
tdt asm cost ASM@1 --breakdown

# Include NRE/tooling amortized over production run
tdt asm cost ASM@1 --qty 500 --amortize 2000 --breakdown

# Exclude NRE/tooling from calculation
tdt asm cost ASM@1 --no-nre

# Disable expired quote warnings
tdt asm cost ASM@1 --warn-expired=false
```

**Cost Calculation Logic:**

1. For each component in the BOM:
   - If `selected_quote` is set: use `quote.price_for_qty(bom_qty × production_qty)`
   - Else if `unit_cost` is set: use that value
   - Otherwise: $0.00 (warning shown)

2. When `--amortize N` is specified:
   - NRE costs from selected quotes are summed
   - NRE per unit = Total NRE ÷ N
   - Effective unit cost = Piece cost + NRE per unit

3. Warnings are shown for:
   - Components with available quotes but no `selected_quote` set
   - Quotes that have expired (past `valid_until` date)

**Setting a Selected Quote:**

```bash
# Set a quote as the selected price source for a component
tdt cmp set-quote CMP@1 QUOT@1

# Clear the selected quote
tdt cmp clear-quote CMP@1
```

### Edit an assembly

```bash
# Open in editor
tdt asm edit ASM-01HC2

# Using short ID
tdt asm edit ASM@1
```

### Delete or archive an assembly

```bash
# Permanently delete (checks for incoming links first)
tdt asm delete ASM@1

# Force delete even if referenced
tdt asm delete ASM@1 --force

# Archive instead of delete (moves to .tdt/archive/)
tdt asm archive ASM@1
```

## BOM Structure

### Single-Level BOM

Lists only direct children of the assembly:

```
ASM-001 Main Assembly
├── CMP-001 Bracket (qty: 4)
├── CMP-002 Screw M4x10 (qty: 8)
├── ASM-002 Power Sub-assembly (qty: 1)
└── ASM-003 Sensor Sub-assembly (qty: 1)
```

### Multi-Level BOM (Indented)

Expands sub-assemblies recursively:

```
ASM-001 Main Assembly
├── CMP-001 Bracket (qty: 4)
├── CMP-002 Screw M4x10 (qty: 8)
├── ASM-002 Power Sub-assembly (qty: 1)
│   ├── CMP-003 Power Board (qty: 1)
│   ├── CMP-004 Connector (qty: 2)
│   └── CMP-005 Cable (qty: 1)
└── ASM-003 Sensor Sub-assembly (qty: 1)
    ├── CMP-006 Sensor PCB (qty: 1)
    └── CMP-007 Housing (qty: 1)
```

### Flattened BOM

Sums quantities across all levels:

| Part Number | Description | Total Qty |
|-------------|-------------|-----------|
| CMP-001 | Bracket | 4 |
| CMP-002 | Screw M4x10 | 8 |
| CMP-003 | Power Board | 1 |
| CMP-004 | Connector | 2 |
| CMP-005 | Cable | 1 |
| CMP-006 | Sensor PCB | 1 |
| CMP-007 | Housing | 1 |

## Reference Designators

Reference designators identify specific instances of components:

| Convention | Use | Examples |
|------------|-----|----------|
| R1, R2 | Resistors | R1, R2, R101 |
| C1, C2 | Capacitors | C1, C2, C101 |
| U1, U2 | ICs | U1, U2, U101 |
| J1, J2 | Connectors | J1, J2 |
| BRK1 | Custom | BRK1, MTR1 |

## Best Practices

### Assembly Structure

1. **Logical grouping** - Group components that are assembled together
2. **Sub-assemblies** - Create sub-assemblies for reusable modules
3. **Flat vs deep** - Balance between flat BOMs and deep nesting
4. **Track quantities** - Verify quantities match drawings

### Documentation

1. **Assembly drawings** - Link to assembly drawings
2. **Work instructions** - Document assembly sequence
3. **Notes** - Add notes for special handling

### Configuration Management

1. **Revision control** - Track assembly revisions
2. **Where-used** - Track which higher assemblies use this one
3. **Effectivity** - Document when changes take effect

## Validation

Assemblies are validated against a JSON Schema:

```bash
# Validate all project files
tdt validate

# Validate specific file
tdt validate bom/assemblies/ASM-01HC2JB7SMQX7RS1Y0GFKBHPTE.tdt.yaml
```

### Validation Rules

1. **ID Format**: Must match `ASM-[A-Z0-9]{26}` pattern
2. **Title**: Required, 1-200 characters
3. **BOM Items**: Must have `component_id` and `quantity`
4. **Status**: Must be one of: `draft`, `review`, `approved`, `released`, `obsolete`
5. **No Additional Properties**: Unknown fields are not allowed

## JSON Schema

The full JSON Schema for assemblies is available at:

```
tdt/schemas/asm.schema.json
```
