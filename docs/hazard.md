# TDT Hazard Entity (Safety Analysis)

This document describes the Hazard entity type in TDT (Tessera Design Toolkit).

## Overview

Hazards represent potential sources of harm in a product or system. They are distinct from risks - a **hazard** is the source of potential harm (e.g., "high voltage"), while a **risk** quantifies the probability and severity of harm occurring from that hazard.

Hazard identification is a foundational step in safety analysis methodologies across multiple industries.

### Standards Alignment

- **ISO 14971** - Medical devices risk management
- **ISO 26262** - Automotive functional safety (HARA)
- **IEC 61508** - Functional safety of electrical/electronic systems
- **DO-178C** - Aerospace software considerations

## Entity Type

- **Prefix**: `HAZ`
- **File extension**: `.tdt.yaml`
- **Directory**: `risks/hazards/`

## Schema

### Required Fields

| Field | Type | Description |
|-------|------|-------------|
| `id` | string | Unique identifier (HAZ-[26-char ULID]) |
| `category` | enum | Hazard category by energy type |
| `title` | string | Short descriptive title (1-200 chars) |
| `description` | string | Detailed description of the hazard source |
| `status` | enum | `draft`, `review`, `approved`, `released`, `obsolete` |
| `created` | datetime | Creation timestamp (ISO 8601) |
| `author` | string | Author who identified the hazard |

### Optional Fields

| Field | Type | Description |
|-------|------|-------------|
| `potential_harms` | array[string] | List of potential harms from this hazard |
| `energy_level` | string | Energy level or magnitude (e.g., "300V DC", "500 psi") |
| `severity` | enum | Maximum severity of potential harm |
| `exposure_scenario` | string | How someone could be exposed |
| `affected_populations` | array[string] | Populations at risk (operators, patients, etc.) |
| `tags` | array[string] | Tags for filtering |
| `revision` | integer | Revision number (default: 1) |

### Hazard Categories

| Category | Description | Examples |
|----------|-------------|----------|
| **electrical** | Electrical hazards | Shock, burns, fire from electrical energy |
| **mechanical** | Mechanical hazards | Crushing, cutting, entanglement |
| **thermal** | Thermal hazards | Burns, frostbite from heat/cold |
| **chemical** | Chemical hazards | Toxicity, corrosion, irritation |
| **biological** | Biological hazards | Infection, allergens, pathogens |
| **radiation** | Radiation hazards | Ionizing and non-ionizing radiation |
| **ergonomic** | Ergonomic hazards | Repetitive strain, posture issues |
| **software** | Software/cyber hazards | Malfunction, security breaches |
| **environmental** | Environmental hazards | Noise, vibration, pressure |

### Hazard Severity

| Severity | Description |
|----------|-------------|
| **negligible** | No injury or minor discomfort |
| **minor** | Temporary injury, first aid only |
| **serious** | Injury requiring medical attention |
| **severe** | Permanent injury or disability |
| **catastrophic** | Death or multiple fatalities |

### Links

| Field | Type | Description |
|-------|------|-------------|
| `links.originates_from` | array[EntityId] | Components/assemblies where hazard originates |
| `links.causes` | array[EntityId] | Risks caused by this hazard |
| `links.controlled_by` | array[EntityId] | Controls that address this hazard |
| `links.verified_by` | array[EntityId] | Tests that verify hazard controls |
| `links.related_to` | array[EntityId] | Related safety requirements |

## Example

```yaml
# Hazard: High Voltage in Motor Controller
# Created by TDT - Tessera Design Toolkit

id: HAZ-01HC2JB7SMQX7RS1Y0GFKBHPTD
category: electrical
title: "High Voltage in Motor Controller"

description: |
  The motor controller contains 300V DC bus voltage during
  operation. This voltage is present on internal PCB traces
  and power terminals.

potential_harms:
  - Electric shock
  - Burns from arc flash
  - Cardiac arrest

energy_level: "300V DC, 50A peak"

severity: catastrophic

exposure_scenario: |
  Maintenance personnel could be exposed when opening the
  controller enclosure for service or repair. Users could
  be exposed if enclosure is damaged.

affected_populations:
  - Service technicians
  - End users (if enclosure breached)

tags: [electrical, safety-critical, high-voltage]
status: approved

links:
  originates_from:
    - CMP-01HC2JB7SMQX7RS1Y0GFKBHPTE  # Motor controller assembly
  causes:
    - RISK-01HC2JB7SMQX7RS1Y0GFKBHPTF  # Electric shock risk
    - RISK-01HC2JB7SMQX7RS1Y0GFKBHPTG  # Arc flash risk
  controlled_by:
    - CTRL-01HC2JB7SMQX7RS1Y0GFKBHPTH  # Enclosure interlock
    - CTRL-01HC2JB7SMQX7RS1Y0GFKBHPTI  # Discharge circuit
  verified_by:
    - TEST-01HC2JB7SMQX7RS1Y0GFKBHPTJ  # Interlock test
  related_to:
    - REQ-01HC2JB7SMQX7RS1Y0GFKBHPTK  # Safety requirement

# Auto-managed metadata
created: 2024-01-15T10:30:00Z
author: Jane Doe
revision: 1
```

## CLI Commands

### Create a new hazard

```bash
# Create with title and category
tdt haz new --title "High Voltage" --category electrical

# Create with severity and energy level
tdt haz new --title "Pinch Point" --category mechanical \
    --severity serious --energy "500N force"

# Create with potential harms
tdt haz new --title "Hot Surface" --category thermal \
    --harms "Burns,Blisters"

# Create with source component
tdt haz new --title "Battery Fire" --category thermal \
    --source CMP@1

# Create with interactive wizard
tdt haz new -i

# Create without opening editor
tdt haz new --title "New Hazard" --category electrical --no-edit
```

### List hazards

```bash
# List all hazards
tdt haz list

# Filter by category
tdt haz list --category electrical
tdt haz list -c mechanical

# Filter by severity
tdt haz list --severity catastrophic
tdt haz list --severity severe

# Filter by status
tdt haz list --status approved
tdt haz list -s draft

# Filter by tag
tdt haz list --tag safety-critical

# Show uncontrolled hazards (no controls linked)
tdt haz list --uncontrolled

# Show hazards without linked risks
tdt haz list --no-risks

# Limit results
tdt haz list -n 10

# Output formats
tdt haz list -o json
tdt haz list -o yaml
tdt haz list -o id
```

### Show hazard details

```bash
# Show by ID (partial match supported)
tdt haz show HAZ-01HC2

# Show by short ID (after running list)
tdt haz show HAZ@1

# Output as JSON
tdt haz show HAZ@1 -o json

# Output as YAML
tdt haz show HAZ@1 -o yaml
```

### Edit a hazard

```bash
# Open in editor by ID
tdt haz edit HAZ-01HC2

# Open by short ID
tdt haz edit HAZ@1
```

### Delete or archive a hazard

```bash
# Permanently delete
tdt haz delete HAZ@1

# Delete without confirmation
tdt haz delete HAZ@1 -y

# Archive instead of delete (moves to .archive/)
tdt haz archive HAZ@1
```

## Hazard vs Risk

Understanding the distinction between hazards and risks is crucial:

| Aspect | Hazard (HAZ) | Risk (RISK) |
|--------|--------------|-------------|
| **Definition** | Source of potential harm | Combination of probability and severity |
| **Example** | "300V DC voltage" | "Electric shock during maintenance" |
| **Focus** | What could cause harm | How likely harm occurs and impact |
| **Quantification** | Energy level, severity | RPN (S × O × D) |
| **Mitigation** | Controls, barriers | Prevention, detection |

### Typical Flow

```
Hazard Identification → Risk Assessment → Mitigation → Verification
       (HAZ)                (RISK)          (CTRL)        (TEST)
```

1. **Identify hazards** in your system (HAZ entities)
2. **Analyze risks** that could result from each hazard (RISK entities)
3. **Implement controls** to reduce risk (CTRL entities)
4. **Verify controls** are effective (TEST entities)

## Link Management

```bash
# Link hazard to source component
tdt link add HAZ@1 CMP@1 originates_from

# Link hazard to risk it causes
tdt link add HAZ@1 RISK@1 causes

# Link hazard to control
tdt link add HAZ@1 CTRL@1 controlled_by

# Link hazard to verification test
tdt link add HAZ@1 TEST@1 verified_by

# Show all links for a hazard
tdt link show HAZ@1

# Check for broken links
tdt link check
```

## Traceability

```bash
# Show what depends on a hazard
tdt trace from HAZ@1

# Show what a hazard depends on
tdt trace to HAZ@1

# Find unlinked hazards (orphans)
tdt trace orphans --type haz

# Coverage report
tdt trace coverage --type haz
```

## Best Practices

### Hazard Identification

1. **Be systematic** - Use structured methods (HAZOP, FMEA, FTA)
2. **Consider all energy types** - Electrical, mechanical, thermal, etc.
3. **Identify sources** - Link hazards to originating components
4. **Quantify energy** - Document energy levels where applicable

### Hazard Documentation

1. **Clear titles** - Use descriptive, unambiguous titles
2. **Specific descriptions** - Explain what the hazard is and where it exists
3. **List potential harms** - Document all possible outcomes
4. **Identify affected populations** - Who could be harmed

### Hazard Control

1. **Link to controls** - Every significant hazard should have controls
2. **Verify effectiveness** - Link controls to verification tests
3. **Track coverage** - Use `tdt haz list --uncontrolled` to find gaps
4. **Review regularly** - Update hazards as design evolves

## Validation

Hazards are validated against a JSON Schema:

```bash
# Validate all project files
tdt validate

# Validate specific file
tdt validate risks/hazards/HAZ-01HC2JB7SMQX7RS1Y0GFKBHPTD.tdt.yaml

# Validate only hazards
tdt validate --entity-type haz
```

### Validation Rules

1. **ID Format**: Must match `HAZ-[A-Z0-9]{26}` pattern
2. **Category**: Must be valid enum value
3. **Title**: Required, 1-200 characters
4. **Description**: Required, non-empty
5. **Status**: Must be one of: `draft`, `review`, `approved`, `released`, `obsolete`
6. **Severity**: Must be valid enum value if provided
7. **No Additional Properties**: Unknown fields are not allowed

## JSON Schema

The full JSON Schema for hazards is embedded in TDT and available at:

```
tdt/schemas/haz.schema.json
```
