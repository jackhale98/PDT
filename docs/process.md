# TDT Process Entity (Manufacturing)

This document describes the Process entity type in TDT (Tessera Design Toolkit).

## Overview

Processes define manufacturing operations - the engineering specification of *what* needs to be done. They capture equipment requirements, process parameters, cycle times, capability data, and safety requirements. Processes link to control plans and work instructions.

## Entity Type

- **Prefix**: `PROC`
- **File extension**: `.tdt.yaml`
- **Directory**: `manufacturing/processes/`

## Schema

### Required Fields

| Field | Type | Description |
|-------|------|-------------|
| `id` | string | Unique identifier (PROC-[26-char ULID]) |
| `title` | string | Short descriptive title (1-200 chars) |
| `status` | enum | `draft`, `review`, `approved`, `released`, `obsolete` |
| `created` | datetime | Creation timestamp (ISO 8601) |
| `author` | string | Author name |

### Optional Fields

| Field | Type | Description |
|-------|------|-------------|
| `description` | string | Detailed description |
| `process_type` | enum | Type of process (see below) |
| `operation_number` | string | Operation number (e.g., "OP-010") |
| `equipment` | array[Equipment] | Equipment used |
| `parameters` | array[ProcessParameter] | Process parameters |
| `cycle_time_minutes` | number | Cycle time in minutes |
| `setup_time_minutes` | number | Setup time in minutes |
| `capability` | ProcessCapability | Process capability data (Cpk, Ppk) |
| `operator_skill` | enum | Required skill level |
| `safety` | ProcessSafety | Safety requirements |
| `require_signature` | boolean | Require operator signature when completing (DHR compliance) |
| `step_approval` | StepApprovalConfig | PR-based approval configuration |
| `tags` | array[string] | Tags for filtering |
| `entity_revision` | integer | Entity revision number (default: 1) |

### Process Types

| Type | Description |
|------|-------------|
| `machining` | Material removal operations (milling, turning, drilling) |
| `assembly` | Component assembly operations |
| `inspection` | Quality inspection operations |
| `test` | Functional or performance testing |
| `finishing` | Surface finishing (painting, plating, anodizing) |
| `packaging` | Packaging operations |
| `handling` | Material handling operations |
| `heat_treat` | Heat treatment processes |
| `welding` | Welding and joining operations |
| `coating` | Coating application |

### Operator Skill Levels

| Level | Description |
|-------|-------------|
| `entry` | New operators with basic training |
| `intermediate` | Trained operators (default) |
| `advanced` | Experienced operators |
| `expert` | Highly skilled specialists |

### Equipment Object

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | Equipment name |
| `equipment_id` | string | Asset number / equipment ID |
| `capability` | string | Required capability or specification |

### ProcessParameter Object

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | Parameter name (e.g., "Spindle Speed") |
| `value` | number | Nominal value |
| `units` | string | Units (e.g., "RPM", "mm/min") |
| `min` | number | Minimum allowed value |
| `max` | number | Maximum allowed value |

### ProcessCapability Object

| Field | Type | Description |
|-------|------|-------------|
| `cpk` | number | Process capability index |
| `ppk` | number | Process performance index |
| `sample_size` | integer | Sample size for study |
| `study_date` | date | Date of capability study |

### ProcessSafety Object

| Field | Type | Description |
|-------|------|-------------|
| `ppe` | array[string] | Required PPE items |
| `hazards` | array[string] | Hazards present |

### StepApprovalConfig Object

| Field | Type | Description |
|-------|------|-------------|
| `require_approval` | boolean | Whether PR approval is required after step completion |
| `min_approvals` | integer | Minimum number of approvals required (default: 1) |
| `required_roles` | array[string] | Roles required to approve (e.g., ["quality", "engineering"]) |

### Links

| Field | Type | Description |
|-------|------|-------------|
| `links.produces` | array[EntityId] | Components produced |
| `links.requirements` | array[EntityId] | Requirements this process implements |
| `links.controls` | array[EntityId] | Control plan items |
| `links.work_instructions` | array[EntityId] | Work instructions |
| `links.risks` | array[EntityId] | Related risks |
| `links.supplier` | EntityId | Supplier ID if process is outsourced |
| `links.related_to` | array[EntityId] | Related entities |

## Example

```yaml
id: PROC-01KC5B2GDDQ0JAXFVXYYZ9DWDZ
title: "CNC Milling - Housing"
description: |
  Precision CNC milling of main housing from aluminum billet.
  Critical features include mounting bores and sealing surfaces.

process_type: machining
operation_number: "OP-010"

equipment:
  - name: "Haas VF-2 CNC Mill"
    equipment_id: "EQ-001"
    capability: "3-axis, 30x16x20 travel"
  - name: "Renishaw Probe"
    equipment_id: "EQ-002"

parameters:
  - name: "Spindle Speed"
    value: 8000
    units: "RPM"
    min: 7500
    max: 8500
  - name: "Feed Rate"
    value: 500
    units: "mm/min"
    min: 400
    max: 600
  - name: "Depth of Cut"
    value: 2.0
    units: "mm"
    max: 3.0

cycle_time_minutes: 15.5
setup_time_minutes: 30

capability:
  cpk: 1.45
  ppk: 1.38
  sample_size: 50
  study_date: 2024-01-15

operator_skill: intermediate

safety:
  ppe:
    - safety_glasses
    - hearing_protection
    - steel_toe_boots
  hazards:
    - "rotating machinery"
    - "sharp edges"
    - "coolant splash"

tags: [machining, housing, critical]
status: approved

links:
  produces:
    - CMP-01HC2JB7SMQX7RS1Y0GFKBHPTD
  controls:
    - CTRL-01KC5B5M87QMYVJT048X27TJ5S
  work_instructions:
    - WORK-01KC5B5XKGWKFTTA9YWTGJB9GE
  risks: []
  related_to: []

created: 2024-01-15T10:30:00Z
author: John Smith
entity_revision: 1
```

## CLI Commands

### Create a new process

```bash
# Create with default template
tdt proc new

# Create with title and type
tdt proc new --title "CNC Milling" --type machining

# Create with operation number
tdt proc new --title "Final Assembly" --type assembly --op-number "OP-020"

# Create with cycle/setup times
tdt proc new --title "Inspection" --type inspection --cycle-time 5 --setup-time 10

# Create with interactive wizard
tdt proc new -i

# Create and immediately edit
tdt proc new --title "New Process" --edit
```

### List processes

```bash
# List all processes
tdt proc list

# Filter by type
tdt proc list --type machining
tdt proc list --type assembly

# Filter by status
tdt proc list --status approved
tdt proc list --status draft

# Search in title/description
tdt proc list --search "milling"

# Sort and limit
tdt proc list --sort title
tdt proc list --limit 10

# Count only
tdt proc list --count

# Output formats
tdt proc list -f json
tdt proc list -f csv
tdt proc list -f md
```

### Show process details

```bash
# Show by ID (partial match supported)
tdt proc show PROC-01KC5

# Show using short ID
tdt proc show PROC@1

# Output as JSON
tdt proc show PROC@1 -f json
```

### Edit a process

```bash
# Open in editor
tdt proc edit PROC-01KC5

# Using short ID
tdt proc edit PROC@1
```

### Delete or archive a process

```bash
# Permanently delete (checks for incoming links first)
tdt proc delete PROC@1

# Force delete even if referenced
tdt proc delete PROC@1 --force

# Archive instead of delete (moves to .tdt/archive/)
tdt proc archive PROC@1
```

### Visualize process flow

```bash
# Display process flow diagram
tdt proc flow

# Show controls for each process
tdt proc flow --controls

# Show work instructions
tdt proc flow --work-instructions

# Show both controls and work instructions
tdt proc flow -c -w

# Filter to specific process
tdt proc flow --process PROC@1

# Output as JSON
tdt proc flow -f json
```

**Example Output:**

```
Process Flow
────────────────────────────────────────────────────────────────

[OP-010] Rough Machining (PROC@1)
  │ Type: Machining | Cycle: 45 min | Setup: 30 min
  │ Equipment: CNC Mill #3
  │ Controls: CTRL@1 "First article inspection"
  │           CTRL@2 "In-process dimensional check"
  ▼
[OP-020] Heat Treatment (PROC@2)
  │ Type: HeatTreat | Cycle: 180 min | Setup: 15 min
  │ Equipment: Furnace #1
  │ Controls: CTRL@3 "Temperature monitoring"
  ▼
[OP-030] Finish Machining (PROC@3)
  │ Type: Machining | Cycle: 60 min | Setup: 20 min
  │ Equipment: CNC Mill #5
  │ Controls: CTRL@4 "Final dimensional inspection"
  ▼
[OP-040] Assembly (PROC@4)
  │ Type: Assembly | Cycle: 30 min | Setup: 10 min
  │ Work Inst: WORK@1 "Bearing installation procedure"

4 processes in flow
```

## DHR Compliance Features

### Operator Signatures

For FDA 21 CFR 820 / ISO 13485 compliance, processes can require operator signatures:

```yaml
# In process YAML
require_signature: true
```

When a process requires signature:
- Operators must use `--sign` flag when completing the step
- Step records `signature_verified: true` and captures the signing key
- Unsigned completion attempts are rejected

### Step Approval Workflow

For critical processes requiring QA or team review:

```yaml
# In process YAML
require_signature: true
step_approval:
  require_approval: true
  min_approvals: 1
  required_roles:
    - quality
    - engineering
```

This enables PR-based approval:
1. Operator completes step with `tdt lot step LOT@1 --sign`
2. Submit for approval with `tdt lot submit-step LOT@1`
3. Reviewer approves with `tdt lot approve LOT@1 --step 1`

## Process vs Work Instruction

| Aspect | Process (PROC) | Work Instruction (WORK) |
|--------|----------------|-------------------------|
| **Audience** | Engineers | Operators |
| **Purpose** | Define *what* to do | Define *how* to do it |
| **Detail** | Parameters, equipment | Step-by-step actions |
| **Focus** | Engineering spec | Execution guidance |

## Best Practices

### Process Definition

1. **One operation per process** - Keep processes focused
2. **Include capability data** - Track Cpk/Ppk from process studies
3. **Document parameters** - Capture critical process parameters with limits
4. **Link to controls** - Connect to control plan items
5. **Safety first** - Always document hazards and PPE requirements

### Operation Numbering

Use a consistent scheme for operation numbers:

```
OP-010  First operation
OP-020  Second operation
OP-030  Third operation
```

Leave gaps (10s) to allow inserting operations later.

## Validation

Processes are validated against a JSON Schema:

```bash
# Validate all project files
tdt validate

# Validate specific file
tdt validate manufacturing/processes/PROC-01KC5B2GDDQ0JAXFVXYYZ9DWDZ.tdt.yaml
```

### Validation Rules

1. **ID Format**: Must match `PROC-[A-Z0-9]{26}` pattern
2. **Title**: Required, 1-200 characters
3. **Process Type**: If specified, must be valid enum value
4. **Status**: Must be one of: `draft`, `review`, `approved`, `released`, `obsolete`

## Process Analysis

### Process Flow Visualization

```bash
# Show process flow with operation sequence
tdt proc flow

# Include control plan items
tdt proc flow --controls

# Include work instructions
tdt proc flow --work-instructions
```

### Domain Mapping Matrix

Analyze process-to-component relationships:

```bash
# Show which processes produce which components
tdt dmm proc cmp

# Show which controls apply to which processes
tdt dmm ctrl proc --stats
```

### Design Structure Matrix

See component relationships including shared processes:

```bash
# Full DSM showing mate, process, and requirement relationships
tdt dsm

# Process relationships only
tdt dsm -t process
```
