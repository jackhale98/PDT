# TDT Test Entity

This document describes the Test entity type in TDT (Tessera Design Toolkit).

## Overview

Tests are verification and validation protocols that ensure your product meets its requirements. TDT supports both verification tests (did we build it right?) and validation tests (did we build the right thing?).

## Entity Type

- **Prefix**: `TEST`
- **File extension**: `.tdt.yaml`
- **Directories**:
  - `verification/protocols/` - Verification test protocols
  - `validation/protocols/` - Validation test protocols

## Schema

### Required Fields

| Field | Type | Description |
|-------|------|-------------|
| `id` | string | Unique identifier (TEST-[26-char ULID]) |
| `type` | enum | `verification` or `validation` |
| `title` | string | Short descriptive title (1-200 chars) |
| `objective` | string | What this test verifies or validates |
| `status` | enum | `draft`, `review`, `approved`, `released`, `obsolete` |
| `created` | datetime | Creation timestamp (ISO 8601) |
| `author` | string | Author name |

### Optional Fields

| Field | Type | Description |
|-------|------|-------------|
| `test_level` | enum | `unit`, `integration`, `system`, `acceptance` |
| `test_method` | enum | `inspection`, `analysis`, `demonstration`, `test` |
| `category` | string | User-defined category |
| `tags` | array[string] | Tags for filtering and organization |
| `description` | string | Detailed description of the test |
| `preconditions` | array[string] | Conditions that must be met before testing |
| `equipment` | array[Equipment] | Required test equipment |
| `procedure` | array[ProcedureStep] | Step-by-step test procedure |
| `acceptance_criteria` | array[string] | Overall pass/fail criteria |
| `sample_size` | SampleSize | Sample size and selection method |
| `environment` | Environment | Environmental conditions |
| `estimated_duration` | string | Expected time to complete |
| `priority` | enum | `low`, `medium`, `high`, `critical` |
| `revision` | integer | Revision number (default: 1) |

### Equipment Object

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | Equipment name or description |
| `specification` | string | Required specification |
| `calibration_required` | boolean | Whether calibration is required |

### ProcedureStep Object

| Field | Type | Description |
|-------|------|-------------|
| `step` | integer | Step number |
| `action` | string | Action to perform |
| `expected` | string | Expected outcome |
| `acceptance` | string | Pass/fail criteria for this step |

### Links

| Field | Type | Description |
|-------|------|-------------|
| `links.verifies` | array[EntityId] | Requirements this test verifies |
| `links.validates` | array[EntityId] | User needs this test validates |
| `links.mitigates` | array[EntityId] | Risks whose mitigation this test verifies |
| `links.depends_on` | array[EntityId] | Tests that must pass before this one |

## Test Types

### Verification vs Validation

| Type | Purpose | Question Answered |
|------|---------|-------------------|
| **Verification** | Confirms design outputs meet design inputs | Did we build it right? |
| **Validation** | Confirms product meets user needs | Did we build the right thing? |

### V-Model Test Levels

Tests follow the V-model hierarchy:

```
User Needs              ←→  Acceptance Testing (validation)
    ↓                           ↑
System Requirements     ←→  System Testing
    ↓                           ↑
Architecture Design     ←→  Integration Testing
    ↓                           ↑
Detailed Design         ←→  Unit Testing
```

| Level | Tests Against | Scope |
|-------|---------------|-------|
| **Unit** | Detailed design | Individual components |
| **Integration** | Architecture design | Component interactions |
| **System** | System requirements | Complete system |
| **Acceptance** | User needs | End-user scenarios |

### IADT Methods

Choose the appropriate verification method:

| Method | Description | When to Use |
|--------|-------------|-------------|
| **Inspection** | Visual examination of product/documentation | Workmanship, labeling, documentation |
| **Analysis** | Calculation, modeling, or simulation | Complex systems, safety-critical |
| **Demonstration** | Show functionality works | User interface, simple operations |
| **Test** | Measured execution under controlled conditions | Performance, environmental, stress |

## Example

```yaml
# Test: Temperature Cycling Test
# Created by TDT - Tessera Design Toolkit

id: TEST-01HC2JB7SMQX7RS1Y0GFKBHPTF
type: verification
test_level: system
test_method: test
title: "Temperature Cycling Test"

category: "Environmental"
tags: [thermal, environmental, reliability]

objective: |
  Verify the device operates within specified temperature range
  from -20C to +50C as required by REQ-01HC2JB7SMQX7RS1Y0GFKBHPTD.

description: |
  This test verifies device operation at temperature extremes using
  a controlled temperature chamber. The unit will be subjected to
  cold and hot soak conditions while monitoring functionality.

preconditions:
  - "Unit at room temperature (23C +/- 2C)"
  - "All test equipment calibrated"
  - "Power supply connected and verified"
  - "Test unit passed incoming inspection"

equipment:
  - name: "Temperature Chamber"
    specification: "-40C to +100C range, 0.5C accuracy"
    calibration_required: true
  - name: "Multimeter"
    specification: "DC voltage measurement, 0.1% accuracy"
    calibration_required: true
  - name: "Data Logger"
    specification: "8-channel temperature logging"
    calibration_required: true

procedure:
  - step: 1
    action: |
      Place unit in chamber at 23C, power on.
      Verify boot sequence completes.
    expected: "Unit boots successfully within 30 seconds"
    acceptance: "All LEDs illuminate correctly, no errors logged"

  - step: 2
    action: |
      Ramp chamber temperature to -20C at 2C/min.
      Monitor unit status continuously.
    expected: "Unit remains operational during ramp"
    acceptance: "No errors logged, heartbeat maintained"

  - step: 3
    action: |
      Hold at -20C for 4 hours.
      Run self-test at 1h intervals.
    expected: "Continuous operation maintained"
    acceptance: "All self-tests pass, no errors logged"

  - step: 4
    action: |
      Ramp chamber temperature to +50C at 2C/min.
      Monitor unit status continuously.
    expected: "Unit remains operational during ramp"
    acceptance: "No errors logged, heartbeat maintained"

  - step: 5
    action: |
      Hold at +50C for 4 hours.
      Run self-test at 1h intervals.
    expected: "Continuous operation maintained"
    acceptance: "All self-tests pass, no errors logged"

  - step: 6
    action: |
      Return to 23C. Power cycle unit.
      Verify normal operation.
    expected: "Unit recovers normally"
    acceptance: "Full functionality verified"

acceptance_criteria:
  - "All procedure steps pass"
  - "No errors in system log throughout test"
  - "All functions operational at temperature extremes"
  - "No physical damage to unit"

sample_size:
  quantity: 3
  rationale: "Statistical significance with 95% confidence"
  sampling_method: "Random selection from production lot"

environment:
  temperature: "Per procedure (-20C to +50C)"
  humidity: "< 80% RH (non-condensing)"
  other: "No vibration during test"

estimated_duration: "12 hours"

priority: high
status: approved

links:
  verifies:
    - REQ-01HC2JB7SMQX7RS1Y0GFKBHPTD  # Temperature requirement
  mitigates:
    - RISK-01HC2JB7SMQX7RS1Y0GFKBHPTE  # Thermal failure risk

# Auto-managed metadata
created: 2024-01-15T10:30:00Z
author: Jane Doe
revision: 1
```

## CLI Commands

### Create a new test

```bash
# Create with default template
tdt test new

# Create verification test with title
tdt test new --title "Temperature Test" -t verification

# Create validation test at acceptance level
tdt test new -t validation -l acceptance

# Create with specific method (IADT)
tdt test new -t verification -m inspection

# Create with interactive wizard
tdt test new -i

# Create and immediately open in editor
tdt test new --title "New Test" --edit
```

### List tests

```bash
# List all tests
tdt test list

# Filter by type
tdt test list --type verification
tdt test list --type validation

# Filter by test level
tdt test list --level unit
tdt test list --level system

# Filter by method
tdt test list --method inspection
tdt test list --method test

# Filter by status
tdt test list --status approved
tdt test list --status active    # All except obsolete

# Filter by priority
tdt test list --priority high

# Show orphaned tests (no linked requirements)
tdt test list --orphans

# Show recently created
tdt test list --recent 7  # Last 7 days

# Search in title/objective
tdt test list --search "temperature"

# Sort and limit
tdt test list --sort title
tdt test list --sort created --reverse
tdt test list --limit 10

# Count only
tdt test list --count
```

### Show test details

```bash
# Show by ID (partial match supported)
tdt test show TEST-01HC2

# Show using short ID
tdt test show TEST@1

# Show by title search
tdt test show "temperature"

# Show with linked entities
tdt test show TEST-01HC2 --with-links

# Output as JSON
tdt test show TEST-01HC2 -f json
```

### Edit a test

```bash
# Open in editor
tdt test edit TEST-01HC2

# Using short ID
tdt test edit TEST@1
```

### Execute a test and record result

```bash
# Execute test and record result (interactive verdict prompt)
tdt test run TEST@1

# Execute with verdict specified
tdt test run TEST@1 --verdict pass
tdt test run TEST@1 --verdict fail
tdt test run TEST@1 --verdict conditional
tdt test run TEST@1 --verdict incomplete

# Specify who executed the test
tdt test run TEST@1 --verdict pass --by "John Smith"

# Add notes
tdt test run TEST@1 --verdict pass --notes "Minor overshoot during ramp"

# Open result in editor for full details
tdt test run TEST@1 --verdict pass --edit

# Output result ID only (for scripting)
tdt test run TEST@1 --verdict pass -f id
```

**Example Output:**

```
✓ Created result RSLT@5 for test TEST@3 "Housing Dimensional Inspection"
   Verdict: pass
   Executed by: John Smith
   verification/results/RSLT-01JEH7QXYZ123456789ABCDEFG.tdt.yaml
```

**Notes:**
- Results are saved to `verification/results/` or `validation/results/` based on the test type
- A new RSLT entity is created and linked to the test
- Use `tdt rslt edit RSLT@N` to add step results, measurements, and attachments

## Validation

Tests are validated against a JSON Schema. Run validation with:

```bash
# Validate all project files
tdt validate

# Validate specific file
tdt validate verification/protocols/TEST-01HC2JB7SMQX7RS1Y0GFKBHPTF.tdt.yaml
```

### Validation Rules

1. **ID Format**: Must match `TEST-[A-Z0-9]{26}` pattern
2. **Type**: Must be `verification` or `validation`
3. **Test Level**: If specified, must be `unit`, `integration`, `system`, or `acceptance`
4. **Test Method**: If specified, must be `inspection`, `analysis`, `demonstration`, or `test`
5. **Status**: Must be one of: `draft`, `review`, `approved`, `released`, `obsolete`
6. **Priority**: Must be one of: `low`, `medium`, `high`, `critical`
7. **Procedure Steps**: Must have `step` (integer) and `action` (string)
8. **No Additional Properties**: Unknown fields are not allowed

## Traceability

```bash
# Show verification coverage
tdt trace coverage

# Show uncovered requirements
tdt trace coverage --uncovered

# Trace what a test verifies
tdt trace from TEST-01HC2

# Find tests that verify a requirement
tdt trace to REQ-01HC2
```

## Best Practices

### Writing Test Procedures

1. **Be specific** - Include exact settings, values, and conditions
2. **Include acceptance criteria** - Define pass/fail for each step
3. **Document equipment** - List required equipment with specifications
4. **Specify preconditions** - Document everything needed before starting
5. **Consider edge cases** - Test boundary conditions

### Organizing Tests

1. **One test per protocol** - Keep tests focused and atomic
2. **Use categories** - Group related tests
3. **Use tags** - Enable cross-cutting filtering
4. **Link to requirements** - Maintain traceability
5. **Separate verification from validation** - Use appropriate directories

### Test Level Guidelines

| Level | When to Use |
|-------|-------------|
| **Unit** | Testing individual components in isolation |
| **Integration** | Testing component interactions |
| **System** | Testing complete system against requirements |
| **Acceptance** | Validating against user needs |

### Method Selection Guidelines

| Method | Best For |
|--------|----------|
| **Inspection** | Physical attributes, labeling, documentation |
| **Analysis** | Mathematical proof, simulation, modeling |
| **Demonstration** | Operational functionality, user workflows |
| **Test** | Quantitative measurement, performance limits |

## JSON Schema

The full JSON Schema for tests is embedded in TDT and available at:

```
tdt/schemas/test.schema.json
```
