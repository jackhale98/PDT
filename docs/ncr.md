# Tessera NCR Entity (Non-Conformance Report)

This document describes the NCR entity type in Tessera.

## Overview

NCRs document quality issues - when products or processes don't meet specifications. They capture what went wrong, containment actions, disposition decisions, and cost impact. NCRs can trigger CAPAs for systemic issues.

## Entity Type

- **Prefix**: `NCR`
- **File extension**: `.tdt.yaml`
- **Directory**: `manufacturing/ncrs/`

## Schema

### Required Fields

| Field | Type | Description |
|-------|------|-------------|
| `id` | string | Unique identifier (NCR-[26-char ULID]) |
| `title` | string | Short descriptive title (1-200 chars) |
| `status` | enum | `draft`, `review`, `approved`, `released`, `obsolete` |
| `created` | datetime | Creation timestamp (ISO 8601) |
| `author` | string | Author name |

### Optional Fields

| Field | Type | Description |
|-------|------|-------------|
| `ncr_number` | string | Company NCR number (e.g., "NCR-2024-0042") |
| `report_date` | date | Date issue was reported |
| `ncr_type` | enum | `internal`, `supplier`, `customer` |
| `severity` | enum | `minor`, `major`, `critical` |
| `category` | enum | Category of non-conformance (see below) |
| `detection` | Detection | Where/when/how found |
| `affected_items` | AffectedItems | Parts/lots affected |
| `defect` | Defect | Defect details |
| `containment` | array[ContainmentAction] | Containment actions taken |
| `disposition` | Disposition | Final disposition decision |
| `cost_impact` | CostImpact | Cost of non-conformance |
| `ncr_status` | enum | NCR workflow status |
| `tags` | array[string] | Tags for filtering |
| `entity_revision` | integer | Entity revision number (default: 1) |

### NCR Types

| Type | Description |
|------|-------------|
| `internal` | Issue found internally during production |
| `supplier` | Issue with supplier-provided material/parts |
| `customer` | Issue reported by customer |

### Severity Levels

| Severity | Description |
|----------|-------------|
| `minor` | Cosmetic or minor deviation, usable with concession |
| `major` | Significant deviation requiring rework or scrap |
| `critical` | Safety, regulatory, or complete failure to meet requirements |

### Categories

| Category | Description |
|----------|-------------|
| `dimensional` | Out of tolerance dimensions |
| `cosmetic` | Surface finish, appearance defects |
| `material` | Wrong material, material defects |
| `functional` | Doesn't function as required |
| `documentation` | Missing or incorrect documentation |
| `process` | Process deviation or error |
| `packaging` | Packaging or labeling issues |

### NCR Status

| Status | Description |
|--------|-------------|
| `open` | Newly created, awaiting action |
| `containment` | Containment actions in progress |
| `investigation` | Root cause investigation |
| `disposition` | Awaiting disposition decision |
| `closed` | NCR resolved and closed |

### Detection Object

| Field | Type | Description |
|-------|------|-------------|
| `found_at` | enum | `incoming`, `in_process`, `final`, `customer`, `field` |
| `found_by` | string | Person who found the issue |
| `found_date` | date | Date issue was found |
| `operation` | string | Operation/stage where found |

### AffectedItems Object

| Field | Type | Description |
|-------|------|-------------|
| `part_number` | string | Part number affected |
| `lot_number` | string | Lot/batch number |
| `serial_numbers` | array[string] | Specific serial numbers |
| `quantity_affected` | integer | Number of units affected |

### Defect Object

| Field | Type | Description |
|-------|------|-------------|
| `characteristic` | string | What characteristic failed |
| `specification` | string | Required specification |
| `actual` | string | Actual measured value |
| `deviation` | number | Amount of deviation |

### ContainmentAction Object

| Field | Type | Description |
|-------|------|-------------|
| `action` | string | Containment action taken |
| `date` | date | Date action taken |
| `completed_by` | string | Person who completed action |
| `status` | enum | `open`, `completed` |

### Disposition Object

| Field | Type | Description |
|-------|------|-------------|
| `decision` | enum | `use_as_is`, `rework`, `scrap`, `return_to_supplier` |
| `decision_date` | date | Date of decision |
| `decision_by` | string | Person who made decision |
| `justification` | string | Rationale for decision |
| `mrb_required` | boolean | Material Review Board required |

### CostImpact Object

| Field | Type | Description |
|-------|------|-------------|
| `rework_cost` | number | Cost of rework |
| `scrap_cost` | number | Cost of scrapped material |
| `currency` | string | Currency code (e.g., "USD") |

### Links

| Field | Type | Description |
|-------|------|-------------|
| `links.component` | EntityId | Affected component |
| `links.supplier` | EntityId | Supplier ID for supplier-related NCRs |
| `links.process` | EntityId | Process where found |
| `links.control` | EntityId | Control that detected |
| `links.capa` | EntityId | Linked CAPA if opened |
| `links.from_result` | EntityId | Test result ID that created this NCR |
| `links.lot` | array[EntityId] | Production lot(s) where non-conformance was found |

## Example

```yaml
id: NCR-01KC5B6E1RKCPKGACCH569FX5R
title: "Bore Diameter Out of Tolerance"
ncr_number: "NCR-2024-0042"
report_date: 2024-01-20

ncr_type: internal
severity: major
category: dimensional

detection:
  found_at: in_process
  found_by: "J. Smith"
  found_date: 2024-01-20
  operation: "CNC Milling - Op 20"

affected_items:
  part_number: "PN-12345"
  lot_number: "LOT-2024-01-20A"
  serial_numbers:
    - "SN-001"
    - "SN-002"
    - "SN-003"
  quantity_affected: 3

defect:
  characteristic: "Bore Diameter"
  specification: "25.00 +0.025/-0.000 mm"
  actual: "24.985 mm"
  deviation: -0.015

containment:
  - action: "Quarantine affected lot"
    date: 2024-01-20
    completed_by: "J. Smith"
    status: completed
  - action: "100% inspection of in-process inventory"
    date: 2024-01-20
    completed_by: "Q. Inspector"
    status: completed

disposition:
  decision: rework
  decision_date: 2024-01-21
  decision_by: "R. Williams"
  justification: "Can re-bore to oversized tolerance per ECN-456"
  mrb_required: true

cost_impact:
  rework_cost: 150.00
  scrap_cost: 0.00
  currency: "USD"

ncr_status: closed

tags: [machining, bore, rework]
status: approved

links:
  component: CMP-01HC2JB7SMQX7RS1Y0GFKBHPTD
  process: PROC-01KC5B2GDDQ0JAXFVXYYZ9DWDZ
  control: CTRL-01KC5B5M87QMYVJT048X27TJ5S
  capa: CAPA-01KC5B6P6PSHZ6TMCSDJQQ6HG3

created: 2024-01-20T08:30:00Z
author: J. Smith
entity_revision: 2
```

## CLI Commands

### Create a new NCR

```bash
# Create with default template
tdt ncr new

# Create with title
tdt ncr new --title "Bore Out of Tolerance"

# Create with type and severity
tdt ncr new --title "Supplier Material Defect" --type supplier --severity major

# Create with category
tdt ncr new --title "Surface Scratch" --type internal --severity minor --category cosmetic

# Interactive wizard
tdt ncr new -i

# Create and immediately edit
tdt ncr new --title "New NCR" --edit

# Create linked to a production lot
tdt ncr new --title "Coating defect" --type internal --severity minor -L LOT@1

# Create and add bidirectional link to lot
tdt ncr new --title "Thin anodize coating" --type internal --severity minor
tdt link add NCR@1 LOT@1   # Creates NCR→LOT and LOT→NCR links
```

### List NCRs

```bash
# List all NCRs
tdt ncr list

# Filter by type
tdt ncr list --type internal
tdt ncr list --type supplier
tdt ncr list --type customer

# Filter by severity
tdt ncr list --severity critical
tdt ncr list --severity major

# Filter by category
tdt ncr list --type dimensional
tdt ncr list --type material

# Filter by NCR status
tdt ncr list --ncr-status open
tdt ncr list --ncr-status closed

# Search in title/description
tdt ncr list --search "bore"

# Filter by linked entities
tdt ncr list --linked-to CMP@1               # NCRs linked to a component
tdt ncr list --linked-to PROC@1              # NCRs linked to a process
tdt ncr list --linked-to CMP@1 --via component  # Filter by link type

# Cross-entity piping
tdt cmp list -f short-id | tdt ncr list --linked-to -   # NCRs for all components
tdt proc list -f short-id | tdt ncr list --linked-to -  # NCRs for all processes

# Output formats
tdt ncr list -f json
tdt ncr list -f csv
tdt ncr list -f md
```

### Show NCR details

```bash
# Show by ID
tdt ncr show NCR-01KC5

# Show using short ID
tdt ncr show NCR@1

# Output as JSON
tdt ncr show NCR@1 -f json
```

### Edit an NCR

```bash
# Open in editor
tdt ncr edit NCR-01KC5

# Using short ID
tdt ncr edit NCR@1
```

### Delete or archive an NCR

```bash
# Permanently delete (checks for incoming links first)
tdt ncr delete NCR@1

# Force delete even if referenced
tdt ncr delete NCR@1 --force

# Archive instead of delete (moves to .tdt/archive/)
tdt ncr archive NCR@1
```

### Advance NCR workflow

Advance an NCR through its workflow stages (open → containment → investigation → disposition):

```bash
# Auto-advance to next stage
tdt ncr advance NCR@1

# Advance to a specific stage
tdt ncr advance NCR@1 --status containment
tdt ncr advance NCR@1 --status investigation
tdt ncr advance NCR@1 --status disposition
```

The `advance` command auto-determines the next workflow stage. To close an NCR from `disposition`, use `ncr close` instead.

### Close an NCR

```bash
# Close with disposition decision
tdt ncr close NCR@1 --disposition rework

# Close with rationale
tdt ncr close NCR@1 --disposition scrap --rationale "Cannot rework to spec"

# Close and link to CAPA
tdt ncr close NCR@1 --disposition use-as-is --capa CAPA@2

# Available dispositions: use-as-is, rework, scrap, return
tdt ncr close NCR@1 --disposition return

# Skip confirmation prompt
tdt ncr close NCR@1 --disposition rework -y
```

**Example Output:**

```
Closing NCR
──────────────────────────────────────────────────
NCR: NCR@3 "Out-of-spec bearing bore diameter"
Current Status: Disposition
Severity: Major

Disposition: Rework
Rationale: Bore can be re-machined to spec

Confirm close? [y/N] y

✓ NCR NCR@3 closed
  Disposition: Rework
  Linked CAPA: CAPA@2
```

## NCR Workflow

```
┌─────────┐     ┌─────────────┐     ┌───────────────┐     ┌─────────────┐     ┌────────┐
│  OPEN   │────▶│ CONTAINMENT │────▶│ INVESTIGATION │────▶│ DISPOSITION │────▶│ CLOSED │
└─────────┘     └─────────────┘     └───────────────┘     └─────────────┘     └────────┘
     │                │                     │                    │
     │                │                     │                    │
     ▼                ▼                     ▼                    ▼
  Create NCR    Quarantine,         Root cause          Use as-is,
                stop the line       analysis            rework, or scrap
```

## Best Practices

### Writing Effective NCRs

1. **Be specific** - Document exactly what was found
2. **Include measurements** - Actual vs. specification
3. **Identify scope** - All affected parts/lots
4. **Act quickly** - Containment prevents spread
5. **Document costs** - Track cost of quality

### Severity Guidelines

| Severity | When to Use |
|----------|-------------|
| **Critical** | Safety risk, regulatory violation, customer ship stop |
| **Major** | Significant rework, potential customer impact, repeat issue |
| **Minor** | Cosmetic, minor deviation, isolated incident |

### Disposition Decisions

| Decision | When Appropriate |
|----------|------------------|
| **Use as-is** | Deviation acceptable, customer approval obtained |
| **Rework** | Can be corrected to meet specification |
| **Scrap** | Cannot be salvaged, cost-prohibitive to rework |
| **Return to supplier** | Supplier-caused defect, within warranty |

### When to Open a CAPA

Open a CAPA when:
- Critical or repeat NCRs
- Systemic issues identified
- Customer complaints
- Audit findings
- Trend analysis shows pattern

## Validation

```bash
# Validate all project files
tdt validate

# Validate specific file
tdt validate manufacturing/ncrs/NCR-01KC5B6E1RKCPKGACCH569FX5R.tdt.yaml
```
