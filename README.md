# PDT - Plain-text Product Development Toolkit

A CLI tool for managing product development artifacts as plain-text YAML files. PDT provides structured tracking of requirements, risks, tests, and other entities with full traceability and validation.

## Features

- **Plain-text YAML files** - Human-readable, git-friendly, diff-able
- **Schema validation** - JSON Schema validation with helpful error messages
- **Traceability** - Link entities together and generate traceability matrices
- **ULID-based IDs** - Unique, sortable identifiers for all entities
- **Short ID aliases** - Use `@1`, `@2`, etc. instead of typing long IDs
- **Beautiful error messages** - Line numbers, context, and actionable suggestions
- **FMEA Risk Management** - Built-in support for Failure Mode and Effects Analysis

## Installation

```bash
cargo install pdt
```

Or build from source:

```bash
git clone https://github.com/yourorg/pdt.git
cd pdt
cargo build --release
```

## Quick Start

```bash
# Initialize a new project
pdt init

# Create a requirement
pdt req new --title "Operating Temperature Range" --type input

# List all requirements (shows @N short IDs)
pdt req list

# Show a specific requirement using short ID
pdt req show @1                    # Use short ID from list
pdt req show REQ-01HC2             # Or partial ID match

# Create a risk
pdt risk new --title "Battery Overheating" -t design

# Validate all project files
pdt validate
```

## Short IDs

After running `list` commands, PDT assigns short IDs (`@1`, `@2`, etc.) to entities:

```bash
$ pdt req list
@     ID               TYPE     TITLE                                STATUS     PRIORITY
------------------------------------------------------------------------------------------
@1    REQ-01HC2JB7...  input    Operating Temperature Range          approved   high
@2    REQ-01HC2JB8...  output   Thermal Management Specification     draft      high

# Now use @N instead of full ID
pdt req show @1
pdt req edit @2
pdt link add @1 --type satisfied_by @2
```

Short IDs are session-local and regenerated each time you run `list`.

## Project Structure

After `pdt init`, your project will have:

```
.pdt/
└── config.yaml              # Project configuration

requirements/
├── inputs/                  # Design inputs (customer requirements)
└── outputs/                 # Design outputs (specifications)

risks/
├── design/                  # Design risks
└── process/                 # Process risks

bom/
├── assemblies/              # Assembly definitions
├── components/              # Component definitions
└── quotes/                  # Supplier quotes

tolerances/
├── features/                # Feature tolerances
├── mates/                   # Mating features
└── stackups/                # Tolerance stackups

verification/
├── protocols/               # Verification test protocols
└── results/                 # Test results

validation/
├── protocols/               # Validation protocols
└── results/                 # Validation results

manufacturing/
├── processes/               # Manufacturing processes
└── controls/                # Process controls
```

## Entity Types

| Prefix | Entity | Description |
|--------|--------|-------------|
| REQ | Requirement | Design inputs and outputs |
| RISK | Risk | Risk / FMEA item |
| TEST | Test | Verification or validation protocol |
| RSLT | Result | Test result / execution record |
| TOL | Tolerance | Tolerance stackup |
| MATE | Mate | Feature mate (for stackups) |
| ASM | Assembly | Assembly definition |
| CMP | Component | Component definition |
| FEAT | Feature | Feature (on a component) |
| PROC | Process | Manufacturing process |
| CTRL | Control | Control plan item |
| QUOT | Quote | Quote / cost record |
| ACT | Action | Action item |

## Output Formats

Use `-f/--format` to control output format:

```bash
pdt req list -f json        # JSON output (for scripting)
pdt req list -f yaml        # YAML output
pdt req list -f csv         # CSV output (for spreadsheets)
pdt req list -f tsv         # Tab-separated (default for lists)
pdt req list -f md          # Markdown table
pdt req list -f id          # Just IDs, one per line

pdt req show REQ-01 -f json # Full entity as JSON
pdt req show REQ-01 -f yaml # Full entity as YAML
```

## Commands

### Project Management

```bash
pdt init                    # Initialize a new project
pdt init --git              # Initialize with git repository
pdt validate                # Validate all project files
pdt validate --keep-going   # Continue after errors
pdt validate --summary      # Show summary only
```

### Requirements

```bash
pdt req new                           # Create with template
pdt req new --title "Title" -t input  # Create with options
pdt req new -i                        # Interactive wizard (schema-driven)
pdt req list                          # List all
pdt req list --status draft           # Filter by status
pdt req list --priority high          # Filter by priority
pdt req list --type input             # Filter by type
pdt req list --search "temperature"   # Search in title/text
pdt req list --orphans                # Show unlinked requirements
pdt req show REQ-01HC2                # Show details (partial ID match)
pdt req edit REQ-01HC2                # Open in editor
```

### Risks (FMEA)

```bash
pdt risk new                           # Create with template
pdt risk new --title "Overheating"     # Create with title
pdt risk new -t process                # Create process risk
pdt risk new --severity 8 --occurrence 5 --detection 3  # Set FMEA ratings
pdt risk new -i                        # Interactive wizard
pdt risk list                          # List all risks
pdt risk list --level high             # Filter by risk level
pdt risk list --by-rpn                 # Sort by RPN (highest first)
pdt risk list --min-rpn 100            # Filter by minimum RPN
pdt risk list --unmitigated            # Show risks without mitigations
pdt risk show RISK-01HC2               # Show details
pdt risk edit RISK-01HC2               # Open in editor
```

### Tests (Verification/Validation)

```bash
pdt test new                                  # Create with template
pdt test new --title "Temperature Test"       # Create with title
pdt test new -t verification -l system        # Create verification test at system level
pdt test new -m analysis                      # Create with analysis method (IADT)
pdt test new -i                               # Interactive wizard
pdt test list                                 # List all tests
pdt test list --type verification             # Filter by test type
pdt test list --level unit                    # Filter by test level
pdt test list --method inspection             # Filter by IADT method
pdt test list --orphans                       # Show tests without linked requirements
pdt test show TEST-01HC2                      # Show details
pdt test edit TEST-01HC2                      # Open in editor
```

### Test Results

```bash
pdt rslt new --test TEST-01HC2                # Create result for a test
pdt rslt new --test @1 --verdict pass         # Use short ID, set verdict
pdt rslt new -i                               # Interactive wizard
pdt rslt list                                 # List all results
pdt rslt list --verdict fail                  # Filter by verdict
pdt rslt list --verdict issues                # Show fail/conditional/incomplete
pdt rslt list --test TEST-01HC2               # Show results for a specific test
pdt rslt list --with-failures                 # Show only results with failures
pdt rslt list --recent 7                      # Show results from last 7 days
pdt rslt show RSLT-01HC2                      # Show details
pdt rslt edit RSLT-01HC2                      # Open in editor
```

### Link Management

```bash
pdt link add REQ-01 --type satisfied_by REQ-02    # Add link
pdt link remove REQ-01 --type satisfied_by REQ-02 # Remove link
pdt link show REQ-01                               # Show all links
pdt link check                                     # Check for broken links
```

### Traceability

```bash
pdt trace matrix                  # Show traceability matrix
pdt trace matrix --output csv     # Export as CSV
pdt trace matrix --output dot     # Export as GraphViz DOT
pdt trace from REQ-01             # What depends on this?
pdt trace to REQ-01               # What does this depend on?
pdt trace orphans                 # Find unlinked entities
pdt trace coverage                # Verification coverage report
pdt trace coverage --uncovered    # Show uncovered requirements
```

## Requirement Example

```yaml
id: REQ-01HC2JB7SMQX7RS1Y0GFKBHPTD
type: input
title: "Operating Temperature Range"

source:
  document: "Customer Requirements Spec"
  revision: "A"
  section: "3.2.1"
  date: 2024-01-15

category: "Environmental"
tags: [thermal, environmental, reliability]

text: |
  The device shall operate continuously in ambient temperatures
  from -20C to +50C without degradation of performance.

rationale: |
  Required for outdoor deployment in various climates.

acceptance_criteria:
  - "Unit powers on at -20C after 4h cold soak"
  - "Unit powers on at +50C after 4h hot soak"

priority: high
status: approved

links:
  satisfied_by:
    - REQ-01HC2JB7SMQX7RS1Y0GFKBHPTE
  verified_by:
    - TEST-01HC2JB7SMQX7RS1Y0GFKBHPTF

created: 2024-01-15T10:30:00Z
author: Jane Doe
revision: 1
```

## Risk Example (FMEA)

```yaml
id: RISK-01HC2JB7SMQX7RS1Y0GFKBHPTD
type: design
title: "Battery Thermal Runaway"

category: "Electrical Safety"
tags: [battery, thermal, safety]

description: |
  Risk of thermal runaway in lithium-ion battery pack during
  charging or high-temperature operation.

failure_mode: |
  Battery cells exceed thermal limits causing cascading
  thermal runaway across the pack.

cause: |
  Internal short circuit, overcharging, or external heat source
  causing cell temperature to exceed safe limits.

effect: |
  Fire, explosion, or toxic gas release endangering users
  and damaging equipment.

# FMEA Risk Assessment (1-10 scale)
severity: 9      # Impact if failure occurs
occurrence: 3    # Likelihood of occurrence
detection: 4     # Ability to detect before failure
rpn: 108         # Risk Priority Number (S x O x D)

mitigations:
  - action: "Add thermal cutoff protection circuit"
    type: prevention
    status: completed
    owner: "John Smith"
  - action: "Add temperature monitoring sensors"
    type: detection
    status: in_progress
    owner: "Jane Doe"

status: review
risk_level: medium

links:
  related_to:
    - REQ-01HC2JB7SMQX7RS1Y0GFKBHPTE
  mitigated_by:
    - REQ-01HC2JB7SMQX7RS1Y0GFKBHPTF
  verified_by:
    - TEST-01HC2JB7SMQX7RS1Y0GFKBHPTG

created: 2024-01-15T10:30:00Z
author: Jane Doe
revision: 2
```

## Test Example (Verification/Validation Protocol)

```yaml
id: TEST-01HC2JB7SMQX7RS1Y0GFKBHPTF
type: verification
test_level: system
test_method: test
title: "Temperature Cycling Test"

category: "Environmental"
tags: [thermal, environmental, reliability]

objective: |
  Verify the device operates within specified temperature range
  as required by REQ-01HC2JB7SMQX7RS1Y0GFKBHPTD.

preconditions:
  - "Unit at room temperature (23C +/- 2C)"
  - "All test equipment calibrated"
  - "Power supply connected"

equipment:
  - name: "Temperature Chamber"
    specification: "-40C to +100C range, 0.5C accuracy"
    calibration_required: true
  - name: "Multimeter"
    specification: "DC voltage measurement"
    calibration_required: true

procedure:
  - step: 1
    action: "Place unit in chamber at 23C, power on"
    expected: "Unit boots successfully"
    acceptance: "All LEDs illuminate correctly"
  - step: 2
    action: "Ramp chamber to -20C at 2C/min"
    expected: "Unit remains operational"
    acceptance: "No errors logged"
  - step: 3
    action: "Hold at -20C for 4 hours"
    expected: "Continuous operation"
    acceptance: "All functions pass self-test"
  - step: 4
    action: "Ramp chamber to +50C at 2C/min"
    expected: "Unit remains operational"
    acceptance: "No errors logged"

acceptance_criteria:
  - "All steps pass"
  - "No errors in system log"
  - "All functions operational at temperature extremes"

environment:
  temperature: "Per procedure"
  humidity: "< 80% RH (non-condensing)"

estimated_duration: "8 hours"

priority: high
status: approved

links:
  verifies:
    - REQ-01HC2JB7SMQX7RS1Y0GFKBHPTD
  mitigates:
    - RISK-01HC2JB7SMQX7RS1Y0GFKBHPTE

created: 2024-01-15T10:30:00Z
author: Jane Doe
revision: 1
```

## Result Example

```yaml
id: RSLT-01HC2JB7SMQX7RS1Y0GFKBHPTG
test_id: TEST-01HC2JB7SMQX7RS1Y0GFKBHPTF
test_revision: 1
title: "Temperature Cycling Test - Run 1"

verdict: pass
verdict_rationale: |
  All steps completed successfully. Device operated within
  specification at both temperature extremes.

category: "Environmental"

executed_date: 2024-02-01T09:00:00Z
executed_by: "John Smith"

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

step_results:
  - step: 1
    result: pass
    observed: "Unit booted in 12 seconds"
  - step: 2
    result: pass
    observed: "No anomalies during ramp"
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
    observed: "No anomalies during ramp"

deviations: []
failures: []

duration: "8h 15m"
notes: |
  Test completed without incident. Minor temperature overshoot
  observed during cold ramp (reached -20.5C briefly).

status: approved

links:
  test: TEST-01HC2JB7SMQX7RS1Y0GFKBHPTF

created: 2024-02-01T17:30:00Z
author: John Smith
revision: 1
```

## Validation

PDT validates files against JSON Schema with detailed error messages:

```
error[pdt::schema::validation]: Schema validation failed
  --> requirements/inputs/REQ-01HC2.pdt.yaml:8:1
   |
 8 | status: pending
   | ^^^^^^^^^^^^^^^ Invalid enum value
   |
  help: Valid values: draft, review, approved, released, obsolete
```

## Status Workflow

```
draft → review → approved → released
                    ↓           ↓
                 obsolete ← ← ← ┘
```

| Status | Description |
|--------|-------------|
| draft | Initial creation, still being written |
| review | Ready for stakeholder review |
| approved | Signed off and baselined |
| released | Released to production/manufacturing |
| obsolete | No longer applicable |

## Priority Levels

| Priority | Use For |
|----------|---------|
| critical | Safety, regulatory, blocking requirements |
| high | Core functionality, key differentiators |
| medium | Standard features, quality of life |
| low | Nice to have, future considerations |

## Risk Assessment (FMEA)

PDT uses FMEA (Failure Mode and Effects Analysis) methodology:

### FMEA Ratings (1-10 scale)

| Factor | 1 | 10 |
|--------|---|-----|
| **Severity** | Minimal impact | Catastrophic, safety hazard |
| **Occurrence** | Very unlikely | Almost certain |
| **Detection** | Always detected | Cannot be detected |

### Risk Priority Number (RPN)

RPN = Severity x Occurrence x Detection (range: 1-1000)

| RPN Range | Risk Level | Action |
|-----------|------------|--------|
| 1-50 | Low | Monitor, no immediate action needed |
| 51-150 | Medium | Plan mitigations, track progress |
| 151-400 | High | Prioritize mitigations, escalate |
| 401+ | Critical | Immediate action required |

### Mitigation Types

| Type | Purpose |
|------|---------|
| **prevention** | Reduces occurrence probability |
| **detection** | Improves ability to detect before failure |

## Test Engineering

### Verification vs Validation

| Type | Purpose | Question |
|------|---------|----------|
| **Verification** | Did we build it right? | Confirms design outputs meet inputs |
| **Validation** | Did we build the right thing? | Confirms product meets user needs |

### V-Model Test Levels

| Level | Tests Against | Scope |
|-------|---------------|-------|
| **Unit** | Detailed design | Individual components |
| **Integration** | Architecture design | Component interactions |
| **System** | System requirements | Complete system |
| **Acceptance** | User needs | End-user scenarios |

### IADT Methods

Tests can use different verification methods (Inspection, Analysis, Demonstration, Test):

| Method | Description | When to Use |
|--------|-------------|-------------|
| **Inspection** | Visual examination | Workmanship, labeling, documentation |
| **Analysis** | Calculation/simulation | Complex systems, safety-critical |
| **Demonstration** | Show functionality | User interface, simple operations |
| **Test** | Measured execution | Performance, environmental, stress |

### Test Verdicts

| Verdict | Meaning | Follow-up |
|---------|---------|-----------|
| **pass** | All criteria met | None required |
| **fail** | One or more criteria not met | Action items required |
| **conditional** | Passed with deviations | Document justification |
| **incomplete** | Could not complete test | Reschedule |
| **not_applicable** | Test not applicable | Document rationale |

## Best Practices

### Writing Requirements

- Use **"shall"** for mandatory requirements
- Use **"should"** for recommended requirements
- Use **"may"** for optional requirements
- Be specific and testable
- One requirement per file

### Organizing Requirements

- Use **categories** to group related requirements
- Use **tags** for cross-cutting concerns
- Separate **inputs** from **outputs** in different directories
- Link related requirements with `satisfied_by` relationships

## License

MIT License - See LICENSE file for details.
