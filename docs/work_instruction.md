# TDT Work Instruction Entity

This document describes the Work Instruction entity type in TDT (Tessera Design Toolkit).

## Overview

Work Instructions provide step-by-step procedures for operators. While processes define *what* to do, work instructions define *how* to do it. They capture safety requirements, tools, materials, detailed procedures, and in-process quality checks.

## Entity Type

- **Prefix**: `WORK`
- **File extension**: `.tdt.yaml`
- **Directory**: `manufacturing/work_instructions/`

## Schema

### Required Fields

| Field | Type | Description |
|-------|------|-------------|
| `id` | string | Unique identifier (WORK-[26-char ULID]) |
| `title` | string | Short descriptive title (1-200 chars) |
| `status` | enum | `draft`, `review`, `approved`, `released`, `obsolete` |
| `created` | datetime | Creation timestamp (ISO 8601) |
| `author` | string | Author name |

### Optional Fields

| Field | Type | Description |
|-------|------|-------------|
| `document_number` | string | Document number (e.g., "WI-MACH-015") |
| `revision` | string | Document revision |
| `description` | string | Purpose/description |
| `safety` | WorkSafety | Safety requirements |
| `tools_required` | array[Tool] | Tools needed |
| `materials_required` | array[Material] | Materials needed |
| `procedure` | array[ProcedureStep] | Step-by-step procedure |
| `quality_checks` | array[QualityCheck] | In-process checks |
| `estimated_duration_minutes` | number | Total estimated time |
| `tags` | array[string] | Tags for filtering |
| `entity_revision` | integer | Entity revision number (default: 1) |

### WorkSafety Object

| Field | Type | Description |
|-------|------|-------------|
| `ppe_required` | array[PpeItem] | Required PPE items |
| `hazards` | array[Hazard] | Hazards and controls |

### PpeItem Object

| Field | Type | Description |
|-------|------|-------------|
| `item` | string | PPE item name |
| `standard` | string | Standard/specification (e.g., "ANSI Z87.1") |

### Hazard Object

| Field | Type | Description |
|-------|------|-------------|
| `hazard` | string | Hazard description |
| `control` | string | Control measure |

### Tool Object

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | Tool name |
| `part_number` | string | Part number or specification |

### Material Object

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | Material name |
| `specification` | string | Specification or part number |

### ProcedureStep Object

| Field | Type | Description |
|-------|------|-------------|
| `step` | integer | Step number |
| `action` | string | Action to perform |
| `verification` | string | Verification point |
| `caution` | string | Caution/warning |
| `image` | string | Image reference path |
| `estimated_time_minutes` | number | Time for this step |

### QualityCheck Object

| Field | Type | Description |
|-------|------|-------------|
| `at_step` | integer | Step number where check occurs |
| `characteristic` | string | What to check |
| `specification` | string | Specification/tolerance |

### Links

| Field | Type | Description |
|-------|------|-------------|
| `links.process` | EntityId | Parent process |
| `links.component` | EntityId | Component this work instruction is for |
| `links.assembly` | EntityId | Assembly this work instruction is for |
| `links.controls` | array[EntityId] | Related control plan items |
| `links.risks` | array[EntityId] | Risks addressed by following this instruction |

## Example

```yaml
id: WORK-01KC5B5XKGWKFTTA9YWTGJB9GE
title: "CNC Mill Setup and Operation"
document_number: "WI-MACH-015"
revision: "B"

description: |
  Step-by-step instructions for setting up and operating the
  Haas VF-2 CNC mill for housing machining operation OP-010.

safety:
  ppe_required:
    - item: "Safety Glasses"
      standard: "ANSI Z87.1"
    - item: "Hearing Protection"
      standard: "NRR 25dB minimum"
    - item: "Steel Toe Boots"
      standard: "ASTM F2413"
  hazards:
    - hazard: "Rotating machinery"
      control: "Keep hands clear during operation, use chip brush"
    - hazard: "Sharp edges"
      control: "Wear cut-resistant gloves when handling parts"
    - hazard: "Coolant splash"
      control: "Keep machine doors closed during operation"

tools_required:
  - name: "3/4 inch End Mill"
    part_number: "TL-EM-750"
  - name: "Edge Finder"
    part_number: "TL-EF-001"
  - name: "Torque Wrench"
    part_number: "TL-TW-25"

materials_required:
  - name: "Cutting Coolant"
    specification: "Coolant-500 mixed 8:1"
  - name: "Deburring Tool"
    specification: "Standard"

procedure:
  - step: 1
    action: "Verify correct CNC program loaded: PRG-1234"
    verification: "Program number matches router sheet"
    estimated_time_minutes: 1

  - step: 2
    action: "Load raw material in vise, torque jaw bolts to 25 ft-lbs"
    verification: "Part seated firmly against parallels"
    caution: "Do not over-torque - risk of part distortion"
    image: "images/step2-fixturing.png"
    estimated_time_minutes: 3

  - step: 3
    action: "Touch off work coordinates using edge finder"
    verification: "X0, Y0, Z0 set correctly"
    estimated_time_minutes: 2

  - step: 4
    action: "Verify tool lengths in tool table"
    verification: "All tools measured within 0.001\""
    estimated_time_minutes: 2

  - step: 5
    action: "Run program in single block mode for first part"
    verification: "Observe proper tool paths, no collisions"
    caution: "Keep hand on feed hold button"
    estimated_time_minutes: 20

  - step: 6
    action: "Measure critical dimensions per control plan"
    verification: "All dimensions within specification"
    estimated_time_minutes: 5

  - step: 7
    action: "If acceptable, run production at full speed"
    estimated_time_minutes: 15

  - step: 8
    action: "Deburr all edges"
    verification: "No sharp edges remaining"
    estimated_time_minutes: 2

quality_checks:
  - at_step: 6
    characteristic: "Bore Diameter"
    specification: "25.00 +0.025/-0.000 mm"
  - at_step: 6
    characteristic: "Overall Length"
    specification: "100.0 ±0.1 mm"
  - at_step: 8
    characteristic: "Surface Finish"
    specification: "Ra 1.6 μm max"

estimated_duration_minutes: 50

tags: [cnc, milling, housing]
status: released

links:
  process: PROC-01KC5B2GDDQ0JAXFVXYYZ9DWDZ
  controls:
    - CTRL-01KC5B5M87QMYVJT048X27TJ5S

created: 2024-01-15T10:30:00Z
author: John Smith
entity_revision: 2
```

## CLI Commands

### Create a new work instruction

```bash
# Create with default template
tdt work new

# Create with title
tdt work new --title "CNC Mill Setup"

# Create with document number
tdt work new --title "CNC Mill Setup" --doc-number "WI-MACH-015"

# Create linked to a process
tdt work new --title "Mill Setup" --process PROC@1

# Interactive wizard
tdt work new -i

# Create and immediately edit
tdt work new --title "New Work Instruction" --edit
```

### List work instructions

```bash
# List all work instructions
tdt work list

# Filter by process
tdt work list --process PROC@1

# Filter by status
tdt work list --status released

# Search in title/description
tdt work list --search "setup"

# Sort options
tdt work list --sort title
tdt work list --sort doc-number

# Output formats
tdt work list -f json
tdt work list -f csv
tdt work list -f md
```

### Show work instruction details

```bash
# Show by ID
tdt work show WORK-01KC5

# Show using short ID
tdt work show WORK@1

# Output as JSON
tdt work show WORK@1 -f json
```

### Edit a work instruction

```bash
# Open in editor
tdt work edit WORK-01KC5

# Using short ID
tdt work edit WORK@1
```

### Delete or archive a work instruction

```bash
# Permanently delete (checks for incoming links first)
tdt work delete WORK@1

# Force delete even if referenced
tdt work delete WORK@1 --force

# Archive instead of delete (moves to .tdt/archive/)
tdt work archive WORK@1
```

## Best Practices

### Writing Effective Work Instructions

1. **Use active voice** - "Torque the bolt" not "The bolt should be torqued"
2. **One action per step** - Keep steps atomic
3. **Include verification points** - How to know step is complete
4. **Add images** - Reference photos for complex setups
5. **Estimate times** - Help with capacity planning
6. **Safety first** - Document hazards and controls

### Document Numbering

Use a consistent scheme:

```
WI-MACH-001  Machining work instruction #1
WI-ASSY-001  Assembly work instruction #1
WI-INSP-001  Inspection work instruction #1
```

### Cautions and Warnings

Use consistent language:

- **CAUTION**: Risk of equipment damage or minor injury
- **WARNING**: Risk of serious injury
- **DANGER**: Risk of death or severe injury

## Validation

```bash
# Validate all project files
tdt validate

# Validate specific file
tdt validate manufacturing/work_instructions/WORK-01KC5B5XKGWKFTTA9YWTGJB9GE.tdt.yaml
```
