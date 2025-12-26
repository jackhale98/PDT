# TDT Control Entity (Control Plan Items)

This document describes the Control entity type in TDT (Tessera Design Toolkit).

## Overview

Controls define control plan items - how to monitor and control manufacturing processes. They capture inspection methods, SPC parameters, sampling plans, and reaction procedures. Controls link back to processes and features, and forward to requirements they verify.

## Entity Type

- **Prefix**: `CTRL`
- **File extension**: `.tdt.yaml`
- **Directory**: `manufacturing/controls/`

## Schema

### Required Fields

| Field | Type | Description |
|-------|------|-------------|
| `id` | string | Unique identifier (CTRL-[26-char ULID]) |
| `title` | string | Short descriptive title (1-200 chars) |
| `status` | enum | `draft`, `review`, `approved`, `released`, `obsolete` |
| `created` | datetime | Creation timestamp (ISO 8601) |
| `author` | string | Author name |

### Optional Fields

| Field | Type | Description |
|-------|------|-------------|
| `description` | string | Detailed description |
| `control_type` | enum | Type of control (see below) |
| `control_category` | enum | `variable` or `attribute` |
| `characteristic` | Characteristic | Characteristic being controlled |
| `measurement` | Measurement | Measurement method |
| `sampling` | Sampling | Sampling plan |
| `control_limits` | ControlLimits | SPC control limits |
| `reaction_plan` | string | Out-of-control reaction plan |
| `tags` | array[string] | Tags for filtering |
| `entity_revision` | integer | Entity revision number (default: 1) |

### Control Types

| Type | Description |
|------|-------------|
| `spc` | Statistical Process Control |
| `inspection` | Dimensional/attribute inspection |
| `poka_yoke` | Error-proofing device |
| `visual` | Visual inspection |
| `functional_test` | Functional test |
| `attribute` | Attribute check (pass/fail) |

### Control Categories

| Category | Description |
|----------|-------------|
| `variable` | Continuous/measured data (dimensions, weight) |
| `attribute` | Discrete/counted data (pass/fail, defect count) |

### Characteristic Object

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | Characteristic name (e.g., "Bore Diameter") |
| `nominal` | number | Nominal value |
| `upper_limit` | number | Upper specification limit (USL) |
| `lower_limit` | number | Lower specification limit (LSL) |
| `units` | string | Units of measurement |
| `critical` | boolean | Critical to quality (CTQ) flag |

### Measurement Object

| Field | Type | Description |
|-------|------|-------------|
| `method` | string | Measurement method description |
| `equipment` | string | Gage/equipment used |
| `gage_rr_percent` | number | Gage R&R percentage from MSA |

### Sampling Object

| Field | Type | Description |
|-------|------|-------------|
| `type` | enum | `continuous`, `periodic`, `lot`, `first_article` |
| `frequency` | string | Sampling frequency (e.g., "5 parts", "every 2 hours") |
| `sample_size` | integer | Sample size per check |

### ControlLimits Object (for SPC)

| Field | Type | Description |
|-------|------|-------------|
| `ucl` | number | Upper control limit |
| `lcl` | number | Lower control limit |
| `target` | number | Target/centerline value |

### Links

| Field | Type | Description |
|-------|------|-------------|
| `links.process` | EntityId | Parent process |
| `links.component` | EntityId | Component being controlled |
| `links.feature` | EntityId | Feature being controlled |
| `links.risks` | array[EntityId] | Risks this control mitigates |
| `links.verifies` | array[EntityId] | Requirements verified |

## Example

```yaml
id: CTRL-01KC5B5M87QMYVJT048X27TJ5S
title: "Bore Diameter SPC"
description: |
  Statistical process control for critical bore diameter.
  Monitors tool wear and process stability.

control_type: spc
control_category: variable

characteristic:
  name: "Bore Diameter"
  nominal: 25.0
  upper_limit: 25.025
  lower_limit: 25.000
  units: "mm"
  critical: true

measurement:
  method: "Bore gauge measurement at 3 depths"
  equipment: "Mitutoyo Bore Gauge GA-045"
  gage_rr_percent: 12.5

sampling:
  type: continuous
  frequency: "5 parts"
  sample_size: 1

control_limits:
  ucl: 25.018
  lcl: 25.007
  target: 25.0125

reaction_plan: |
  1. STOP production
  2. Quarantine last 5 parts
  3. Notify supervisor immediately
  4. Measure quarantined parts
  5. Adjust tool offset per SOP-123
  6. Verify with 3 consecutive good parts
  7. Document on control chart

tags: [spc, bore, critical, ctq]
status: approved

links:
  process: PROC-01KC5B2GDDQ0JAXFVXYYZ9DWDZ
  feature: FEAT-01HC2JB7SMQX7RS1Y0GFKBHPTE
  verifies:
    - REQ-01HC2JB7SMQX7RS1Y0GFKBHPTD

created: 2024-01-15T10:30:00Z
author: John Smith
entity_revision: 1
```

## CLI Commands

### Create a new control

```bash
# Create with default template
tdt ctrl new

# Create with title and type
tdt ctrl new --title "Bore Diameter SPC" --type spc

# Create linked to a process
tdt ctrl new --title "Bore SPC" --type spc --process PROC@1

# Create linked to a feature
tdt ctrl new --title "Bore SPC" --type spc --process PROC@1 --feature FEAT@1

# Mark as critical (CTQ)
tdt ctrl new --title "Critical Dimension" --type inspection --critical

# Specify characteristic name
tdt ctrl new --title "Length Check" --type inspection --characteristic "Overall Length"

# Interactive wizard
tdt ctrl new -i

# Create and immediately edit
tdt ctrl new --title "New Control" --edit
```

### List controls

```bash
# List all controls
tdt ctrl list

# Filter by control type
tdt ctrl list --type spc
tdt ctrl list --type inspection
tdt ctrl list --type visual

# Filter by process
tdt ctrl list --process PROC@1

# Show only critical (CTQ) controls
tdt ctrl list --critical

# Filter by status
tdt ctrl list --status approved

# Search in title/description
tdt ctrl list --search "bore"

# Output formats
tdt ctrl list -f json
tdt ctrl list -f csv
tdt ctrl list -f md
```

### Show control details

```bash
# Show by ID
tdt ctrl show CTRL-01KC5

# Show using short ID
tdt ctrl show CTRL@1

# Output as JSON
tdt ctrl show CTRL@1 -f json
```

### Edit a control

```bash
# Open in editor
tdt ctrl edit CTRL-01KC5

# Using short ID
tdt ctrl edit CTRL@1
```

### Delete or archive a control

```bash
# Permanently delete (checks for incoming links first)
tdt ctrl delete CTRL@1

# Force delete even if referenced
tdt ctrl delete CTRL@1 --force

# Archive instead of delete (moves to .tdt/archive/)
tdt ctrl archive CTRL@1
```

## Control Types in Detail

### SPC (Statistical Process Control)

For monitoring process stability with control charts:

- Use for critical dimensions
- Requires control limits (UCL, LCL, target)
- Include Gage R&R results
- Document reaction plan

### Inspection

For dimensional or attribute checks:

- First article inspection
- In-process checks
- Final inspection

### Poka-Yoke

For error-proofing devices:

- Go/no-go fixtures
- Sensors and interlocks
- Automatic verification

### Visual

For visual inspection criteria:

- Cosmetic requirements
- Workmanship standards
- Color/finish checks

## Best Practices

### Control Plan Development

1. **Link to features** - Connect controls to specific features
2. **Document Gage R&R** - Include MSA results
3. **Define reaction plans** - Clear steps for out-of-control conditions
4. **Mark CTQs** - Flag critical characteristics
5. **Appropriate sampling** - Balance detection with cost

### Sampling Considerations

| Risk Level | Sampling Strategy |
|------------|-------------------|
| Critical (CTQ) | 100% or continuous SPC |
| High | Every part or frequent |
| Medium | Periodic sampling |
| Low | First article + audit |

### Control Limits vs Spec Limits

| Limit Type | Source | Purpose |
|------------|--------|---------|
| **Specification Limits** (USL/LSL) | Engineering | Product acceptance |
| **Control Limits** (UCL/LCL) | Process data | Process monitoring |

Control limits should be tighter than specification limits for capable processes (Cpk > 1.33).

## Validation

Controls are validated against a JSON Schema:

```bash
# Validate all project files
tdt validate

# Validate specific file
tdt validate manufacturing/controls/CTRL-01KC5B5M87QMYVJT048X27TJ5S.tdt.yaml
```
