# Tessera DEV Entity (Process Deviation)

This document describes the DEV entity type in Tessera.

## Overview

DEVs track pre-approved departures from standard processes. Unlike NCRs (which are reactive - something went wrong), deviations are proactive - intentionally doing something different, with proper approval.

Common use cases:
- **Material substitution** - Using equivalent material due to supply issues
- **Process modification** - Temporary parameter changes
- **Equipment substitution** - Using alternate equipment
- **Specification deviation** - Exceeding or relaxing specification limits

## Entity Type

- **Prefix**: `DEV`
- **File extension**: `.tdt.yaml`
- **Directory**: `manufacturing/deviations/`

## Schema

### Required Fields

| Field | Type | Description |
|-------|------|-------------|
| `id` | string | Unique identifier (DEV-[26-char ULID]) |
| `title` | string | Short descriptive title (1-200 chars) |
| `status` | enum | `draft`, `review`, `approved`, `released`, `obsolete` |
| `created` | datetime | Creation timestamp (ISO 8601) |
| `author` | string | Author name |

### Optional Fields

| Field | Type | Description |
|-------|------|-------------|
| `deviation_number` | string | User-defined deviation number (e.g., "DEV-2024-042") |
| `deviation_type` | enum | Type of deviation (see below) |
| `category` | enum | Category of deviation (see below) |
| `description` | string | Detailed description of the deviation |
| `risk` | DevRisk | Risk assessment object (see below) |
| `approval` | DevApproval | Approval information (see below) |
| `effective_date` | date | Date the deviation becomes effective |
| `expiration_date` | date | Date the deviation expires (null for permanent) |
| `dev_status` | enum | Deviation workflow status (see below) |
| `notes` | string | Markdown notes |
| `tags` | array[string] | Tags for filtering |
| `entity_revision` | integer | Entity revision number (default: 1) |

### Deviation Type

| Type | Description |
|------|-------------|
| `temporary` | Time-limited deviation with expiration date |
| `permanent` | Requires formal change control (links to ECO) |
| `emergency` | Immediate action needed, expedited approval |

### Deviation Category

| Category | Description |
|----------|-------------|
| `material` | Material substitution |
| `process` | Process parameter change |
| `equipment` | Equipment substitution |
| `tooling` | Tooling modification |
| `specification` | Specification deviation |
| `documentation` | Documentation deviation |

### Deviation Status (dev_status)

| Status | Description |
|--------|-------------|
| `pending` | Awaiting approval |
| `approved` | Approved but not yet effective |
| `active` | Currently in effect |
| `expired` | Past expiration date |
| `closed` | Manually closed before expiration |
| `rejected` | Deviation request rejected |

### DevRisk Object

| Field | Type | Description |
|-------|------|-------------|
| `level` | enum | Risk level: `low`, `medium`, `high` |
| `assessment` | string | Risk assessment text |
| `mitigations` | array[string] | Mitigation measures |

### DevApproval Object

| Field | Type | Description |
|-------|------|-------------|
| `approved_by` | string | Name of approver |
| `approval_date` | date | Date of approval |
| `authorization_level` | enum | `engineering`, `quality`, `management` |

### Links

| Field | Type | Description |
|-------|------|-------------|
| `links.processes` | array[EntityId] | Affected PROC entities |
| `links.lots` | array[EntityId] | LOT entities this applies to |
| `links.components` | array[EntityId] | Affected CMP entities |
| `links.requirements` | array[EntityId] | REQ entities being deviated from |
| `links.ncrs` | array[EntityId] | Related NCRs (if deviation arose from NCR) |
| `links.change_order` | string | ECO/DCN reference for permanent deviations |

## Example

```yaml
id: DEV-01KC5B6E1RKCPKGACCH569FX5R
title: "316L Material Substitution for Lot 2024-001"
deviation_number: "DEV-2024-042"

deviation_type: temporary
category: material

description: |
  Substituting 316L stainless steel for 304 stainless steel
  due to supply chain shortage. Engineering analysis shows
  316L meets or exceeds all requirements.

risk:
  level: low
  assessment: "316L has equal or better corrosion resistance than 304"
  mitigations:
    - "First article inspection required"
    - "Material cert review by QE"
    - "Update BOM upon permanent change approval"

approval:
  approved_by: "R. Williams"
  approval_date: 2024-01-15
  authorization_level: engineering

effective_date: 2024-01-16
expiration_date: 2024-03-15

dev_status: active

notes: |
  # Notes
  - Supplier ABC confirmed 304 supply delayed 6 weeks
  - 316L procurement confirmed at same price point
  - ECO pending for permanent change

tags: [material, supply-chain, 2024-q1]
status: approved

links:
  processes:
    - PROC-01KC5B2GDDQ0JAXFVXYYZ9DWDZ
  lots:
    - LOT-01KC5B6E1RKCPKGACCH569FX5R
  components:
    - CMP-01HC2JB7SMQX7RS1Y0GFKBHPTD
  requirements:
    - REQ-01HC2JA3K1QX7RS1Y0GFKBHPTC
  ncrs: []
  change_order: ~

created: 2024-01-15T10:00:00Z
author: J. Smith
entity_revision: 1
```

## CLI Commands

### Create a new DEV

```bash
# Create with default template
tdt dev new

# Create with title and type
tdt dev new --title "Material Substitution" --dev-type temporary --category material

# Create emergency deviation
tdt dev new --title "Speed Override" --dev-type emergency --category process --risk high

# Create with deviation number
tdt dev new --title "Equipment Change" --deviation-number "DEV-2024-043"

# Create and link to entities
tdt dev new --title "Process Change" --link PROC@1 --link LOT@2

# Create and immediately edit
tdt dev new --title "New Deviation" --edit

# Non-interactive (skip editor)
tdt dev new --title "Quick Deviation" --no-edit
```

### List DEVs

```bash
# List all deviations
tdt dev list

# Filter by deviation status
tdt dev list --status pending
tdt dev list --status active
tdt dev list --status expired

# Filter by type
tdt dev list --dev-type temporary
tdt dev list --dev-type emergency

# Filter by category
tdt dev list --category material
tdt dev list --category process

# Filter by risk level
tdt dev list --risk high

# Show only active deviations
tdt dev list --active

# Search in title
tdt dev list --search "material"

# Output formats
tdt dev list -f json
tdt dev list -f csv
tdt dev list -f md
```

### Show DEV details

```bash
# Show by ID
tdt dev show DEV-01KC5

# Show using short ID
tdt dev show DEV@1

# Output as JSON
tdt dev show DEV@1 -f json

# Output as YAML
tdt dev show DEV@1 -f yaml
```

### Edit a DEV

```bash
# Open in editor
tdt dev edit DEV-01KC5

# Using short ID
tdt dev edit DEV@1
```

### Approve a DEV

```bash
# Approve with approver name
tdt dev approve DEV@1 --approved-by "R. Williams"

# Approve with authorization level
tdt dev approve DEV@1 --approved-by "M. Jones" --authorization quality

# Skip confirmation
tdt dev approve DEV@1 --approved-by "J. Smith" -y
```

### Expire/close a DEV

```bash
# Close/expire a deviation
tdt dev expire DEV@1

# Expire with reason
tdt dev expire DEV@1 --reason "Production run complete"

# Skip confirmation
tdt dev expire DEV@1 -y
```

### Delete or archive a DEV

```bash
# Permanently delete (checks for incoming links first)
tdt dev delete DEV@1

# Force delete even if referenced
tdt dev delete DEV@1 --force

# Archive instead of delete (moves to .tdt/archive/)
tdt dev archive DEV@1
```

## Deviation Workflow

```
┌─────────┐     ┌──────────┐     ┌────────┐     ┌─────────┐
│ PENDING │────▶│ APPROVED │────▶│ ACTIVE │────▶│ EXPIRED │
└─────────┘     └──────────┘     └────────┘     └─────────┘
     │                                │               ▲
     │                                │               │
     ▼                                └───────────────┘
┌──────────┐                          (auto or manual)
│ REJECTED │
└──────────┘                    ┌────────┐
                                │ CLOSED │
                                └────────┘
                                    ▲
                                    │ (manual)
```

### Typical Flow

1. **Request** - `tdt dev new --title "Material Change" --category material`
2. **Risk assess** - Edit to add risk assessment and mitigations
3. **Review** - Set status to `review`
4. **Approve** - `tdt dev approve DEV@1 --approved-by "Manager"`
5. **Activate** - Becomes `active` on effective date
6. **Execute** - Work proceeds under deviation
7. **Expire** - Auto-expires on expiration date, or manual `tdt dev expire`

## Use with Lot Step Execution

Approved deviations can be used to bypass step order enforcement during lot execution. When a LOT's electronic router enforces sequential step execution, an approved deviation allows out-of-order step completion:

```bash
# Bypass step order enforcement with an approved deviation
tdt lot wi-step LOT@1 --process 2 --wi WORK@2 --step 1 --complete --deviation DEV@1
```

The `--deviation` flag validates that the referenced DEV entity exists and has `dev_status: approved` before allowing the bypass.

## DEV vs NCR

| Aspect | DEV (Deviation) | NCR (Non-Conformance) |
|--------|-----------------|----------------------|
| **Timing** | Pre-approved (proactive) | After discovery (reactive) |
| **Purpose** | Planned departure from standard | Documenting something wrong |
| **Approval** | Required before work | Disposition after discovery |
| **Duration** | Defined scope/time | Until closure |
| **Examples** | Material substitution, process change | Failed inspection, wrong part used |

## Best Practices

### Risk Assessment

1. **Assess thoroughly** - Consider all failure modes
2. **Document mitigations** - List specific controls
3. **Match authorization** - Higher risk = higher approval level
4. **Review periodically** - Monitor active deviations

### Scope Control

1. **Be specific** - Define exact scope (lots, dates, quantities)
2. **Link entities** - Connect to affected PROC, LOT, CMP
3. **Set expiration** - Temporary deviations must expire
4. **Track permanent** - Link ECO for permanent changes

### Traceability

1. **Record linkages** - Connect to requirements being deviated
2. **Document NCRs** - Link if deviation arose from NCR
3. **Track lots** - Link all affected production lots
4. **Archive expired** - Keep records for audit trail

### Regulatory Compliance

For regulated industries (FDA, ISO 13485):
- Maintain deviation register
- Document risk assessments
- Track approval authorities
- Archive closed deviations
- Link to change control for permanent changes

## Git Integration

```bash
# Create deviation request
tdt dev new --title "Material Sub" --category material
git commit -m "DEV@1: Requested material substitution"

# After approval
tdt dev approve DEV@1 --approved-by "R. Williams"
git commit -m "DEV@1: Approved by R. Williams"

# Link to production lot
tdt lot edit LOT@1  # Add DEV@1 to links
git commit -m "LOT@1: Working under DEV@1"

# Close deviation
tdt dev expire DEV@1 --reason "Lot complete"
git commit -m "DEV@1: Closed - lot complete"
```

## Validation

```bash
# Validate all project files
tdt validate

# Validate specific file
tdt validate manufacturing/deviations/DEV-01KC5B6E1RKCPKGACCH569FX5R.tdt.yaml
```
