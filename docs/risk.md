# PDT Risk Entity (FMEA)

This document describes the Risk entity type in PDT (Plain-text Product Development Toolkit).

## Overview

Risks capture potential failure modes and their analysis using FMEA (Failure Mode and Effects Analysis) methodology. They help identify, assess, and mitigate risks throughout product development.

## Entity Type

- **Prefix**: `RISK`
- **File extension**: `.pdt.yaml`
- **Directories**:
  - `risks/design/` - Design risks (product/component failures)
  - `risks/process/` - Process risks (manufacturing/operational failures)

## Schema

### Required Fields

| Field | Type | Description |
|-------|------|-------------|
| `id` | string | Unique identifier (RISK-[26-char ULID]) |
| `type` | enum | `design` or `process` |
| `title` | string | Short descriptive title (1-200 chars) |
| `description` | string | Detailed description of the risk |
| `status` | enum | `draft`, `review`, `approved`, `released`, `obsolete` |
| `created` | datetime | Creation timestamp (ISO 8601) |
| `author` | string | Author name |

### FMEA Fields

| Field | Type | Description |
|-------|------|-------------|
| `failure_mode` | string | How the failure manifests |
| `cause` | string | Root cause or mechanism of failure |
| `effect` | string | Impact or consequence of the failure |
| `severity` | integer | Severity rating 1-10 (S) |
| `occurrence` | integer | Occurrence/probability rating 1-10 (O) |
| `detection` | integer | Detection difficulty rating 1-10 (D) |
| `rpn` | integer | Risk Priority Number = S × O × D (1-1000) |
| `risk_level` | enum | `low`, `medium`, `high`, `critical` |

### Optional Fields

| Field | Type | Description |
|-------|------|-------------|
| `category` | string | User-defined category |
| `tags` | array[string] | Tags for filtering and organization |
| `initial_risk` | object | Initial risk assessment before mitigations |
| `mitigations` | array[object] | List of mitigation actions |
| `revision` | integer | Revision number (default: 1) |

### Mitigation Object

| Field | Type | Description |
|-------|------|-------------|
| `action` | string | Mitigation action description (required) |
| `type` | enum | `prevention` or `detection` |
| `status` | enum | `proposed`, `in_progress`, `completed`, `verified` |
| `owner` | string | Person responsible for implementing |
| `due_date` | date | Target completion date |

### Links

| Field | Type | Description |
|-------|------|-------------|
| `links.related_to` | array[EntityId] | Related requirements or entities |
| `links.mitigated_by` | array[EntityId] | Design outputs that mitigate this risk |
| `links.verified_by` | array[EntityId] | Tests that verify mitigation effectiveness |

## Example

```yaml
# Risk: Battery Thermal Runaway
# Created by PDT - Plain-text Product Development Toolkit

id: RISK-01HC2JB7SMQX7RS1Y0GFKBHPTD
type: design
title: "Battery Thermal Runaway"

category: "Electrical Safety"
tags: [battery, thermal, safety, critical]

description: |
  Risk of thermal runaway in lithium-ion battery pack during
  charging or high-temperature operation. This could lead to
  fire, explosion, or toxic gas release.

failure_mode: |
  Battery cells exceed thermal limits causing cascading
  thermal runaway across the pack.

cause: |
  Internal short circuit, overcharging, or external heat source
  causing cell temperature to exceed safe limits (>60°C).

effect: |
  Fire, explosion, or toxic gas release endangering users
  and damaging equipment. Potential for serious injury.

# FMEA Risk Assessment
severity: 9        # High - potential for serious injury
occurrence: 3      # Low - good quality cells, proper BMS
detection: 4       # Moderate - temperature monitoring in place
rpn: 108           # 9 × 3 × 4 = 108 (Medium risk)

# Initial assessment (before mitigations)
initial_risk:
  severity: 9
  occurrence: 5
  detection: 6
  rpn: 270

mitigations:
  - action: "Add thermal cutoff protection circuit"
    type: prevention
    status: completed
    owner: "John Smith"
  - action: "Add temperature monitoring with alerts"
    type: detection
    status: completed
    owner: "Jane Doe"
  - action: "Implement cell balancing algorithm"
    type: prevention
    status: in_progress
    owner: "Bob Wilson"
    due_date: 2024-06-30

status: review
risk_level: medium

links:
  related_to:
    - REQ-01HC2JB7SMQX7RS1Y0GFKBHPTE  # Battery safety requirement
  mitigated_by:
    - REQ-01HC2JB7SMQX7RS1Y0GFKBHPTF  # Thermal protection spec
  verified_by:
    - TEST-01HC2JB7SMQX7RS1Y0GFKBHPTG  # Thermal abuse test

# Auto-managed metadata (do not edit manually)
created: 2024-01-15T10:30:00Z
author: Jane Doe
revision: 2
```

## CLI Commands

### Create a new risk

```bash
# Create with default template
pdt risk new

# Create with title and type
pdt risk new --title "Battery Overheating" --type design

# Create with FMEA ratings
pdt risk new --title "Motor Failure" --severity 7 --occurrence 4 --detection 5

# Create process risk
pdt risk new --title "Assembly Error" --type process

# Create with interactive wizard
pdt risk new -i

# Create and open in editor
pdt risk new --title "New Risk" --edit
```

### List risks

```bash
# List all risks
pdt risk list

# Filter by type
pdt risk list --type design
pdt risk list --type process

# Filter by status
pdt risk list --status draft
pdt risk list --status approved

# Filter by risk level
pdt risk list --level high
pdt risk list --level critical

# Filter by urgency (high + critical)
pdt risk list --level urgent

# Filter by RPN range
pdt risk list --min-rpn 100
pdt risk list --max-rpn 200
pdt risk list --min-rpn 100 --max-rpn 300

# Sort by RPN (highest first)
pdt risk list --by-rpn

# Show unmitigated risks
pdt risk list --unmitigated

# Search in title/description
pdt risk list --search "thermal"

# Combine filters
pdt risk list --type design --level high --by-rpn

# Output formats
pdt risk list -f json
pdt risk list -f csv
pdt risk list -f md
```

### Show risk details

```bash
# Show by ID (partial match supported)
pdt risk show RISK-01HC2

# Show by short ID (after running list)
pdt risk show RISK@1

# Show by title search
pdt risk show "thermal"

# Output as JSON
pdt risk show RISK@1 -f json

# Output as YAML
pdt risk show RISK@1 -f yaml
```

### Edit a risk

```bash
# Open in editor by ID
pdt risk edit RISK-01HC2

# Open by short ID
pdt risk edit RISK@1
```

## FMEA Methodology

### Severity Rating (S)

| Rating | Description | Criteria |
|--------|-------------|----------|
| 1 | None | No effect |
| 2-3 | Minor | Slight inconvenience, no safety impact |
| 4-5 | Moderate | Customer dissatisfaction, minor injury possible |
| 6-7 | High | Significant impact, injury possible |
| 8-9 | Very High | Serious injury possible, regulatory non-compliance |
| 10 | Hazardous | Life-threatening, regulatory violation |

### Occurrence Rating (O)

| Rating | Description | Probability |
|--------|-------------|-------------|
| 1 | Remote | < 1 in 1,500,000 |
| 2-3 | Low | 1 in 150,000 - 1 in 15,000 |
| 4-5 | Moderate | 1 in 2,000 - 1 in 400 |
| 6-7 | High | 1 in 80 - 1 in 20 |
| 8-9 | Very High | 1 in 8 - 1 in 3 |
| 10 | Almost Certain | > 1 in 2 |

### Detection Rating (D)

| Rating | Description | Criteria |
|--------|-------------|----------|
| 1 | Almost Certain | Controls will almost certainly detect |
| 2-3 | High | High likelihood of detection |
| 4-5 | Moderate | Moderate likelihood of detection |
| 6-7 | Low | Low likelihood of detection |
| 8-9 | Very Low | Very low likelihood of detection |
| 10 | Undetectable | No known controls can detect |

### Risk Priority Number (RPN)

RPN = Severity × Occurrence × Detection

| RPN Range | Risk Level | Action Required |
|-----------|------------|-----------------|
| 1-50 | Low | Monitor, document |
| 51-150 | Medium | Plan mitigations, track progress |
| 151-400 | High | Prioritize mitigations, management review |
| 401-1000 | Critical | Immediate action, escalate to leadership |

## Risk Mitigation

### Mitigation Types

1. **Prevention** - Reduces occurrence probability
   - Design changes
   - Material selection
   - Process controls
   - Training

2. **Detection** - Improves ability to detect before failure
   - Inspection methods
   - Testing procedures
   - Monitoring systems
   - Alarms/alerts

### Mitigation Workflow

```
proposed → in_progress → completed → verified
```

1. **proposed**: Mitigation identified but not started
2. **in_progress**: Implementation underway
3. **completed**: Implementation finished
4. **verified**: Effectiveness confirmed through testing

## Link Management

```bash
# Link risk to a requirement
pdt link add RISK-01HC2 --type related_to REQ-01HC3

# Link risk to mitigation design output
pdt link add RISK-01HC2 --type mitigated_by REQ-01HC4

# Link risk to verification test
pdt link add RISK-01HC2 --type verified_by TEST-01HC5

# Show all links for a risk
pdt link show RISK-01HC2

# Check for broken links
pdt link check
```

## Traceability

```bash
# Show what depends on a risk
pdt trace from RISK-01HC2

# Show what a risk depends on
pdt trace to RISK-01HC2

# Find unlinked risks
pdt trace orphans --type risk

# Risk coverage report
pdt trace coverage --type risk
```

## Validation

Risks are validated against a JSON Schema. Run validation with:

```bash
# Validate all project files
pdt validate

# Validate specific file
pdt validate risks/design/RISK-01HC2JB7SMQX7RS1Y0GFKBHPTD.pdt.yaml

# Validate only risks
pdt validate --entity-type risk

# Check for RPN/risk_level calculation mismatches (shown as warnings)
pdt validate --strict    # Treat calculation warnings as errors

# Auto-fix calculated values (RPN, risk_level)
pdt validate --fix
```

### Calculated Value Validation

PDT validates that calculated values (RPN and risk_level) are consistent:

- **RPN**: Must equal `severity × occurrence × detection`
- **risk_level**: Must match the expected level based on RPN:
  - 1-50: low
  - 51-150: medium
  - 151-400: high
  - 401+: critical

If these values are incorrect, PDT shows warnings:

```
! risks/design/RISK-01HC2.pdt.yaml - 2 calculation warning(s)
    RPN mismatch: stored 999 but calculated 125 (5×5×5)
    risk_level mismatch: stored 'low' but calculated 'medium'
```

Use `--fix` to automatically correct these values:

```bash
pdt validate --fix
# Files fixed:    1
```

### Validation Rules

1. **ID Format**: Must match `RISK-[A-Z0-9]{26}` pattern
2. **Type**: Must be `design` or `process`
3. **Title**: Required, 1-200 characters
4. **Description**: Required, non-empty
5. **Status**: Must be one of: `draft`, `review`, `approved`, `released`, `obsolete`
6. **Severity/Occurrence/Detection**: Must be 1-10 if provided
7. **RPN**: Must be 1-1000 if provided
8. **Risk Level**: Must be one of: `low`, `medium`, `high`, `critical`
9. **No Additional Properties**: Unknown fields are not allowed

## Best Practices

### Writing Good Risk Descriptions

1. **Be specific** - Describe the exact failure scenario
2. **Include context** - When/where does this risk apply?
3. **Quantify when possible** - Use numbers, ranges, thresholds
4. **Separate cause from effect** - Don't conflate root cause with impact

### Organizing Risks

1. **Use categories** to group related risks (electrical, mechanical, software, etc.)
2. **Use tags** for cross-cutting concerns (safety, regulatory, customer-facing)
3. **Separate design from process** risks in different directories
4. **Link to requirements** to maintain traceability

### Risk Review Process

1. **Initial Assessment**: Create risk with preliminary FMEA ratings
2. **Team Review**: Validate ratings with cross-functional team
3. **Mitigation Planning**: Identify and assign mitigation actions
4. **Implementation**: Track mitigation progress
5. **Re-assessment**: Update ratings after mitigations are verified
6. **Approval**: Move to approved status when acceptable

### RPN Reduction Strategies

To reduce RPN, focus on:

1. **Severity** - Often fixed by design; may require fundamental changes
2. **Occurrence** - Add prevention controls, improve design robustness
3. **Detection** - Add inspection, testing, monitoring systems

Priority: Focus on high-severity risks first, then high-RPN risks.

## JSON Schema

The full JSON Schema for risks is embedded in PDT and available at:

```
pdt/schemas/risk.schema.json
```

Key schema properties:

- `additionalProperties: false` - No unknown fields allowed
- `minLength` constraints on title and description
- `maxLength: 200` on title
- `minimum: 1, maximum: 10` on severity, occurrence, detection
- `minimum: 1, maximum: 1000` on rpn
- `format: date-time` on created
- `pattern` validation on ID field
