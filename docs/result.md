# TDT Result Entity

This document describes the Result entity type in TDT (Tessera Design Toolkit).

## Overview

Results are execution records of test protocols. They capture the outcome of running a test, including step-by-step results, measurements, deviations, and failures. Results provide objective evidence that testing was performed and document the verdict.

## Entity Type

- **Prefix**: `RSLT`
- **File extension**: `.tdt.yaml`
- **Directories**:
  - `verification/results/` - Results of verification tests
  - `validation/results/` - Results of validation tests

## Schema

### Required Fields

| Field | Type | Description |
|-------|------|-------------|
| `id` | string | Unique identifier (RSLT-[26-char ULID]) |
| `test_id` | string | ID of the test protocol executed (TEST-[26-char ULID]) |
| `verdict` | enum | `pass`, `fail`, `conditional`, `incomplete`, `not_applicable` |
| `executed_date` | datetime | When the test was executed |
| `executed_by` | string | Person who executed the test |
| `status` | enum | `draft`, `review`, `approved`, `released`, `obsolete` |
| `created` | datetime | Creation timestamp (ISO 8601) |
| `author` | string | Author name (who created this record) |

### Optional Fields

| Field | Type | Description |
|-------|------|-------------|
| `test_revision` | integer | Revision of the test protocol used |
| `title` | string | Optional title (defaults to test title + date) |
| `verdict_rationale` | string | Explanation for the verdict |
| `category` | string | User-defined category |
| `tags` | array[string] | Tags for filtering and organization |
| `reviewed_by` | string | Person who reviewed the results |
| `reviewed_date` | datetime | When the results were reviewed |
| `sample_info` | SampleInfo | Information about the tested sample |
| `environment` | ResultEnvironment | Actual environmental conditions |
| `equipment_used` | array[EquipmentUsed] | Equipment used with calibration info |
| `step_results` | array[StepResultRecord] | Results for each procedure step |
| `deviations` | array[Deviation] | Any deviations from the procedure |
| `failures` | array[Failure] | Details of any failures |
| `attachments` | array[Attachment] | Supporting files (photos, data, etc.) |
| `duration` | string | Actual time to complete the test |
| `notes` | string | General observations |
| `revision` | integer | Revision number (default: 1) |

### SampleInfo Object

| Field | Type | Description |
|-------|------|-------------|
| `sample_id` | string | Identifier for the sample |
| `serial_number` | string | Serial number of the unit |
| `lot_number` | string | Lot or batch number |
| `configuration` | string | Configuration or build version |

### EquipmentUsed Object

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | Equipment name |
| `asset_id` | string | Asset or equipment ID |
| `calibration_date` | string | Date of last calibration |
| `calibration_due` | string | Date calibration expires |

### StepResultRecord Object

| Field | Type | Description |
|-------|------|-------------|
| `step` | integer | Step number from the procedure |
| `result` | enum | `pass`, `fail`, `skip`, `not_applicable` |
| `observed` | string | What was actually observed |
| `measurement` | Measurement | Quantitative measurement data |
| `notes` | string | Additional notes for this step |

### Measurement Object

| Field | Type | Description |
|-------|------|-------------|
| `value` | number | Measured value |
| `unit` | string | Unit of measurement |
| `min` | number | Minimum acceptable value |
| `max` | number | Maximum acceptable value |

### Deviation Object

| Field | Type | Description |
|-------|------|-------------|
| `description` | string | Description of the deviation |
| `impact` | string | Impact on results |
| `justification` | string | Justification for accepting |

### Failure Object

| Field | Type | Description |
|-------|------|-------------|
| `description` | string | Description of the failure |
| `step` | integer | Step where failure occurred |
| `root_cause` | string | Root cause analysis |
| `corrective_action` | string | Corrective action required |
| `action_id` | string | ID of related action item |

### Links

| Field | Type | Description |
|-------|------|-------------|
| `links.test` | EntityId | ID of the test protocol (same as test_id) |
| `links.actions` | array[EntityId] | IDs of action items from this result |

## Verdicts

| Verdict | Meaning | Required Actions |
|---------|---------|------------------|
| **pass** | All acceptance criteria met | None |
| **fail** | One or more criteria not met | Action items required |
| **conditional** | Passed with documented deviations | Document justification |
| **incomplete** | Could not complete the test | Reschedule test |
| **not_applicable** | Test not applicable to this sample | Document rationale |

## Example

```yaml
# Result: Temperature Cycling Test - Run 1
# Created by TDT - Tessera Design Toolkit

id: RSLT-01HC2JB7SMQX7RS1Y0GFKBHPTG
test_id: TEST-01HC2JB7SMQX7RS1Y0GFKBHPTF
test_revision: 1
title: "Temperature Cycling Test - Run 1"

verdict: pass
verdict_rationale: |
  All steps completed successfully. Device operated within
  specification at both temperature extremes. Minor temperature
  overshoot during cold ramp was within acceptable limits.

category: "Environmental"
tags: [thermal, production-lot-2024-001]

executed_date: 2024-02-01T09:00:00Z
executed_by: "John Smith"

reviewed_by: "Jane Doe"
reviewed_date: 2024-02-02T14:00:00Z

sample_info:
  sample_id: "SN-001234"
  serial_number: "001234"
  lot_number: "LOT-2024-001"
  configuration: "Rev B hardware, v1.2.0 firmware"

environment:
  temperature: "-20C to +50C per procedure"
  humidity: "45% RH"
  location: "Lab A, Environmental Chamber #3"

equipment_used:
  - name: "Temperature Chamber"
    asset_id: "ENV-CHAM-003"
    calibration_date: "2024-01-15"
    calibration_due: "2025-01-15"
  - name: "Multimeter"
    asset_id: "MM-042"
    calibration_date: "2024-01-10"
    calibration_due: "2024-07-10"

step_results:
  - step: 1
    result: pass
    observed: "Unit booted in 12 seconds, all LEDs nominal"
    notes: "Slightly faster than previous units"

  - step: 2
    result: pass
    observed: "No anomalies during temperature ramp"
    measurement:
      value: -20.5
      unit: "C"
      min: -21
      max: -19
    notes: "Slight overshoot to -20.5C, recovered within 30 seconds"

  - step: 3
    result: pass
    observed: "Self-test passed at 1h, 2h, 3h, 4h intervals"
    measurement:
      value: -20.1
      unit: "C"
      min: -21
      max: -19

  - step: 4
    result: pass
    observed: "No anomalies during temperature ramp"
    measurement:
      value: 50.2
      unit: "C"
      min: 49
      max: 51

  - step: 5
    result: pass
    observed: "Self-test passed at all intervals"
    measurement:
      value: 50.0
      unit: "C"
      min: 49
      max: 51

  - step: 6
    result: pass
    observed: "Unit powered off and on cleanly, all functions verified"

deviations: []

failures: []

attachments:
  - filename: "temperature_log_20240201.csv"
    path: "attachments/temperature_log_20240201.csv"
    type: data
    description: "Chamber temperature log for entire test duration"
  - filename: "unit_photo_cold_soak.jpg"
    path: "attachments/unit_photo_cold_soak.jpg"
    type: photo
    description: "Photo of unit at -20C showing frost formation"

duration: "8h 15m"

notes: |
  Test completed without significant issues. Minor temperature
  overshoot during cold ramp (reached -20.5C briefly) was within
  acceptable chamber control limits. Unit #001234 meets all
  temperature requirements.

  Recommend proceeding to next production lot testing.

status: approved

links:
  test: TEST-01HC2JB7SMQX7RS1Y0GFKBHPTF
  actions: []

# Auto-managed metadata
created: 2024-02-01T17:30:00Z
author: John Smith
revision: 2
```

## Example with Failure

```yaml
id: RSLT-01HC2JB7SMQX7RS1Y0GFKBHPTH
test_id: TEST-01HC2JB7SMQX7RS1Y0GFKBHPTF
test_revision: 1
title: "Temperature Cycling Test - Run 2 (Failed)"

verdict: fail
verdict_rationale: |
  Unit failed during cold soak phase. Communication was lost
  at -15C and did not recover. Root cause analysis indicates
  potential component failure.

executed_date: 2024-02-05T09:00:00Z
executed_by: "John Smith"

step_results:
  - step: 1
    result: pass
    observed: "Unit booted normally"

  - step: 2
    result: fail
    observed: "Communication lost at -15C"
    measurement:
      value: -15.2
      unit: "C"
      min: -21
      max: -19

  - step: 3
    result: skip
    notes: "Skipped due to failure in step 2"

failures:
  - description: "Communication loss at -15C"
    step: 2
    root_cause: |
      Preliminary analysis indicates solder joint failure
      on communication IC. Cold temperature caused thermal
      contraction exposing marginal joint.
    corrective_action: |
      1. Return unit to manufacturing for failure analysis
      2. Review solder process parameters
      3. Inspect other units from same lot
    action_id: "ACT-01HC2JB7SMQX7RS1Y0GFKBHPTI"

status: approved

links:
  test: TEST-01HC2JB7SMQX7RS1Y0GFKBHPTF
  actions:
    - ACT-01HC2JB7SMQX7RS1Y0GFKBHPTI

created: 2024-02-05T15:00:00Z
author: John Smith
revision: 1
```

## CLI Commands

### Create a new result

```bash
# Create result for a specific test
tdt rslt new --test TEST-01HC2

# Create using short ID
tdt rslt new --test TEST@1 --verdict pass

# Create with verdict
tdt rslt new --test TEST-01HC2 --verdict fail

# Specify executor
tdt rslt new --test TEST-01HC2 -e "John Smith"

# Create with interactive wizard
tdt rslt new -i

# Create and immediately edit
tdt rslt new --test TEST-01HC2 --edit
```

### List results

```bash
# List all results
tdt rslt list

# Filter by verdict
tdt rslt list --verdict pass
tdt rslt list --verdict fail
tdt rslt list --verdict issues   # fail + conditional + incomplete

# Filter by test
tdt rslt list --test TEST-01HC2

# Filter by status
tdt rslt list --status approved
tdt rslt list --status active

# Filter by executor
tdt rslt list --executed-by "John"

# Show results with failures
tdt rslt list --with-failures

# Show results with deviations
tdt rslt list --with-deviations

# Show recent results
tdt rslt list --recent 7  # Last 7 days

# Search in title/notes
tdt rslt list --search "temperature"

# Sort and limit
tdt rslt list --sort verdict
tdt rslt list --sort executed-date --reverse
tdt rslt list --limit 10

# Count only
tdt rslt list --count
```

### Show result details

```bash
# Show by ID (partial match supported)
tdt rslt show RSLT-01HC2

# Show using short ID
tdt rslt show RSLT@1

# Show with test protocol details
tdt rslt show RSLT-01HC2 --with-test

# Output as JSON
tdt rslt show RSLT-01HC2 -f json

# Output as YAML
tdt rslt show RSLT-01HC2 -f yaml
```

### Edit a result

```bash
# Open in editor
tdt rslt edit RSLT-01HC2

# Using short ID
tdt rslt edit RSLT@1
```

### Show result summary

```bash
# Show overall test execution statistics
tdt rslt summary

# Filter to specific test
tdt rslt summary --test TEST@1

# Include detailed breakdown by test type
tdt rslt summary --detailed

# Output as JSON for reporting
tdt rslt summary -f json
```

**Example Output:**

```
Test Results Summary
──────────────────────────────────────────────────
Total Results: 45

  Pass:        38 ( 84.4%)
  Fail:         4 (  8.9%)
  Conditional:  2 (  4.4%)
  Incomplete:   1 (  2.2%)

Recent Failures (last 30 days):
  RSLT@12 TEST@5 "Load Test"           2024-01-15
  RSLT@8  TEST@3 "Dimensional Check"   2024-01-10

Requirement Coverage:
  78.5% (45/57 requirements have tests)
```

## Validation

Results are validated against a JSON Schema. Run validation with:

```bash
# Validate all project files
tdt validate

# Validate specific file
tdt validate verification/results/RSLT-01HC2JB7SMQX7RS1Y0GFKBHPTG.tdt.yaml
```

### Validation Rules

1. **ID Format**: Must match `RSLT-[A-Z0-9]{26}` pattern
2. **Test ID Format**: Must match `TEST-[A-Z0-9]{26}` pattern
3. **Verdict**: Must be `pass`, `fail`, `conditional`, `incomplete`, or `not_applicable`
4. **Step Results**: Must have `step` (integer) and `result` (enum)
5. **Status**: Must be one of: `draft`, `review`, `approved`, `released`, `obsolete`
6. **Dates**: Must be ISO 8601 format
7. **No Additional Properties**: Unknown fields are not allowed

## Workflow

### Typical Result Workflow

1. **Execute Test** - Run the test per the protocol
2. **Create Result** - `tdt rslt new --test TEST-xxx`
3. **Document** - Edit result to add step results, observations
4. **Review** - Submit for peer review (status: review)
5. **Approve** - Sign off on results (status: approved)

### Handling Failures

When a test fails:

1. **Document Failure** - Add to `failures` array with description
2. **Root Cause Analysis** - Document root cause in failure record
3. **Corrective Actions** - Create action items
4. **Link Actions** - Add action IDs to `links.actions`
5. **Retest** - Create new result for retest

### Handling Deviations

When a test passes with deviations:

1. **Set Verdict** - Use `conditional`
2. **Document Deviation** - Add to `deviations` array
3. **Justify** - Explain why deviation is acceptable
4. **Assess Impact** - Document impact on results

## Best Practices

### Documenting Results

1. **Be thorough** - Document everything observed
2. **Include measurements** - Record actual values with min/max
3. **Attach evidence** - Photos, data files, logs
4. **Explain verdicts** - Especially for non-pass results
5. **Track equipment** - Record calibration status

### Organizing Results

1. **One result per execution** - Don't combine multiple test runs
2. **Use consistent naming** - Include test name and run number
3. **Use categories and tags** - Enable filtering
4. **Link to actions** - Maintain traceability for failures
5. **Archive attachments** - Keep data files with results

### Verdict Guidelines

| Scenario | Verdict | Action |
|----------|---------|--------|
| All criteria met | pass | None |
| Minor non-conformance, documented justification | conditional | Document deviation |
| Any failure without justification | fail | Create action items |
| Test interrupted, cannot complete | incomplete | Reschedule |
| Test does not apply | not_applicable | Document rationale |

## Metrics

Use TDT to generate testing metrics:

```bash
# Overall pass rate
tdt rslt list --verdict pass --count
tdt rslt list --count

# Failure rate by test
tdt rslt list --test TEST-01HC2 --verdict fail --count

# Recent failures
tdt rslt list --verdict fail --recent 30
```

## JSON Schema

The full JSON Schema for results is embedded in TDT and available at:

```
tdt/schemas/rslt.schema.json
```
