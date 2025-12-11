# TDT - Tessera Engineering Toolkit

A CLI tool for managing engineering artifacts as plain-text YAML files. TDT provides structured tracking of requirements, risks, tests, and other entities with full traceability and validation.

## Features

- **Plain-text YAML files** - Human-readable, git-friendly, diff-able
- **Schema validation** - JSON Schema validation with helpful error messages
- **Traceability** - Link entities together and generate traceability matrices
- **ULID-based IDs** - Unique, sortable identifiers for all entities
- **Short ID aliases** - Use `REQ@1`, `RISK@2`, etc. instead of typing long IDs
- **Beautiful error messages** - Line numbers, context, and actionable suggestions
- **FMEA Risk Management** - Built-in support for Failure Mode and Effects Analysis
- **BOM Management** - Components and assemblies with supplier tracking
- **Tolerance Analysis** - Features, mates, and stackups with worst-case, RSS, and Monte Carlo analysis

## Installation

```bash
cargo install tdt
```

Or build from source:

```bash
git clone https://github.com/yourorg/tdt.git
cd tdt
cargo build --release
```

## Quick Start

```bash
# Initialize a new project
tdt init

# Create a requirement
tdt req new --title "Operating Temperature Range" --type input

# List all requirements (shows REQ@N short IDs)
tdt req list

# Show a specific requirement using short ID
tdt req show REQ@1                 # Use prefixed short ID from list
tdt req show REQ-01HC2             # Or partial ID match

# Create a risk
tdt risk new --title "Battery Overheating" -t design

# Validate all project files
tdt validate
```

## Short IDs

After running `list` commands, TDT assigns entity-prefixed short IDs (`REQ@1`, `RISK@1`, etc.) to entities:

```bash
$ tdt req list
@       ID               TYPE     TITLE                                STATUS     PRIORITY
--------------------------------------------------------------------------------------------
REQ@1   REQ-01HC2JB7...  input    Operating Temperature Range          approved   high
REQ@2   REQ-01HC2JB8...  output   Thermal Management Specification     draft      high

$ tdt risk list
@        ID                TYPE      TITLE                            STATUS     LEVEL    RPN
----------------------------------------------------------------------------------------------
RISK@1   RISK-01HC2JB7...  design    Battery Overheating              review     medium   108

# Use prefixed short IDs instead of full IDs
tdt req show REQ@1
tdt risk show RISK@1
tdt link add REQ@1 --type verified_by TEST@1
tdt trace from REQ@1
```

Short IDs are persistent per entity type - the same entity keeps its short ID across list commands.
This enables cross-entity linking (e.g., linking `REQ@1` to `TEST@1`).

## Project Structure

After `tdt init`, your project will have:

```
.tdt/
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
├── processes/               # Manufacturing process definitions
├── controls/                # Control plan items (SPC, inspection)
├── work_instructions/       # Operator procedures
├── ncrs/                    # Non-conformance reports
└── capas/                   # Corrective/preventive actions
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
| PROC | Process | Manufacturing process definition |
| CTRL | Control | Control plan item (SPC, inspection) |
| WORK | Work Instruction | Operator procedures |
| NCR | Non-Conformance | Non-conformance report |
| CAPA | CAPA | Corrective/preventive action |
| QUOT | Quote | Quote / cost record |
| SUP | Supplier | Approved supplier |

## Output Formats

Use `-f/--format` to control output format:

```bash
tdt req list -f json        # JSON output (for scripting)
tdt req list -f yaml        # YAML output
tdt req list -f csv         # CSV output (for spreadsheets)
tdt req list -f tsv         # Tab-separated (default for lists)
tdt req list -f md          # Markdown table
tdt req list -f id          # Just IDs, one per line

tdt req show REQ-01 -f json # Full entity as JSON
tdt req show REQ-01 -f yaml # Full entity as YAML
```

## Commands

### Project Management

```bash
tdt init                    # Initialize a new project
tdt init --git              # Initialize with git repository
tdt validate                # Validate all project files
tdt validate --keep-going   # Continue after errors
tdt validate --summary      # Show summary only
tdt validate --fix          # Auto-fix calculated values (RPN, risk level)
tdt validate --strict       # Treat warnings as errors
```

### Requirements

```bash
tdt req new                           # Create with template
tdt req new --title "Title" -t input  # Create with options
tdt req new -i                        # Interactive wizard (schema-driven)
tdt req list                          # List all
tdt req list --status draft           # Filter by status
tdt req list --priority high          # Filter by priority
tdt req list --type input             # Filter by type
tdt req list --search "temperature"   # Search in title/text
tdt req list --orphans                # Show unlinked requirements
tdt req show REQ-01HC2                # Show details (partial ID match)
tdt req edit REQ-01HC2                # Open in editor
```

### Risks (FMEA)

```bash
tdt risk new                           # Create with template
tdt risk new --title "Overheating"     # Create with title
tdt risk new -t process                # Create process risk
tdt risk new --severity 8 --occurrence 5 --detection 3  # Set FMEA ratings
tdt risk new -i                        # Interactive wizard
tdt risk list                          # List all risks
tdt risk list --level high             # Filter by risk level
tdt risk list --by-rpn                 # Sort by RPN (highest first)
tdt risk list --min-rpn 100            # Filter by minimum RPN
tdt risk list --unmitigated            # Show risks without mitigations
tdt risk show RISK-01HC2               # Show details
tdt risk edit RISK-01HC2               # Open in editor
```

### Tests (Verification/Validation)

```bash
tdt test new                                  # Create with template
tdt test new --title "Temperature Test"       # Create with title
tdt test new -t verification -l system        # Create verification test at system level
tdt test new -m analysis                      # Create with analysis method (IADT)
tdt test new -i                               # Interactive wizard
tdt test list                                 # List all tests
tdt test list --type verification             # Filter by test type
tdt test list --level unit                    # Filter by test level
tdt test list --method inspection             # Filter by IADT method
tdt test list --orphans                       # Show tests without linked requirements
tdt test show TEST-01HC2                      # Show details
tdt test edit TEST-01HC2                      # Open in editor
```

### Test Results

```bash
tdt rslt new --test TEST-01HC2                # Create result for a test
tdt rslt new --test @1 --verdict pass         # Use short ID, set verdict
tdt rslt new -i                               # Interactive wizard
tdt rslt list                                 # List all results
tdt rslt list --verdict fail                  # Filter by verdict
tdt rslt list --verdict issues                # Show fail/conditional/incomplete
tdt rslt list --test TEST-01HC2               # Show results for a specific test
tdt rslt list --with-failures                 # Show only results with failures
tdt rslt list --recent 7                      # Show results from last 7 days
tdt rslt show RSLT-01HC2                      # Show details
tdt rslt edit RSLT-01HC2                      # Open in editor
```

### Components (BOM)

```bash
tdt cmp new                                   # Create with template
tdt cmp new --title "Motor Assembly" --part-number "PN-001"
tdt cmp new --make-buy buy --category mechanical
tdt cmp list                                  # List all components
tdt cmp list --make-buy buy                   # Filter by make/buy
tdt cmp list --category electrical            # Filter by category
tdt cmp show CMP@1                            # Show details
tdt cmp edit CMP@1                            # Open in editor
```

### Assemblies (BOM)

```bash
tdt asm new                                   # Create with template
tdt asm new --title "Main Assembly" --part-number "ASM-001"
tdt asm list                                  # List all assemblies
tdt asm show ASM@1                            # Show details
tdt asm bom ASM@1                             # Show flattened BOM
tdt asm edit ASM@1                            # Open in editor
```

### Suppliers (Approved Vendors)

```bash
tdt sup new --name "Acme Manufacturing Corp"  # Create supplier
tdt sup new -n "Acme Corp" --short-name "Acme" --website "https://acme.com"
tdt sup new -i                                # Interactive mode
tdt sup list                                  # List all suppliers
tdt sup list -c machining                     # Filter by capability
tdt sup list --search "acme"                  # Search in name
tdt sup show SUP@1                            # Show details
tdt sup edit SUP@1                            # Open in editor
```

### Quotes (Supplier Quotations)

```bash
tdt quote new --component CMP@1 --supplier SUP@1        # Quote for component
tdt quote new --assembly ASM@1 --supplier SUP@1         # Quote for assembly
tdt quote new -c CMP@1 -s SUP@1 --price 12.50 --lead-time 14
tdt quote new -i                              # Interactive mode
tdt quote list                                # List all quotes
tdt quote list -Q pending                     # Filter by quote status
tdt quote list --component CMP@1              # Filter by component
tdt quote list --supplier SUP@1               # Filter by supplier
tdt quote show QUOT@1                         # Show details
tdt quote compare CMP@1                       # Compare quotes for item
tdt quote edit QUOT@1                         # Open in editor
```

### Features (Tolerances)

```bash
tdt feat new --component CMP@1 --type hole --title "Mounting Hole"
tdt feat new --component CMP@1 --type shaft   # Feature requires parent component
tdt feat list                                 # List all features
tdt feat list --component CMP@1               # Filter by component
tdt feat list --type hole                     # Filter by type
tdt feat show FEAT@1                          # Show details
tdt feat edit FEAT@1                          # Open in editor
```

### Mates (Tolerances)

```bash
tdt mate new --feature-a FEAT@1 --feature-b FEAT@2 --title "Pin-Hole Fit"
tdt mate list                                 # List all mates
tdt mate list --type clearance_fit            # Filter by mate type
tdt mate show MATE@1                          # Show details with fit calculation
tdt mate recalc MATE@1                        # Recalculate fit from features
tdt mate edit MATE@1                          # Open in editor
```

### Stackups (Tolerance Analysis)

```bash
tdt tol new --title "Gap Analysis" --target-nominal 1.0 --target-upper 1.5 --target-lower 0.5
tdt tol list                                  # List all stackups
tdt tol list --result pass                    # Filter by worst-case result
tdt tol list --critical                       # Show only critical stackups
tdt tol show TOL@1                            # Show details with analysis
tdt tol analyze TOL@1                         # Run worst-case, RSS, Monte Carlo
tdt tol analyze TOL@1 --iterations 50000      # Custom Monte Carlo iterations
tdt tol edit TOL@1                            # Open in editor
```

### Manufacturing Processes

```bash
tdt proc new --title "CNC Milling" --type machining
tdt proc new --title "Final Assembly" --type assembly --op-number "OP-020"
tdt proc list                                 # List all processes
tdt proc list --type machining                # Filter by process type
tdt proc list --status approved               # Filter by status
tdt proc show PROC@1                          # Show details
tdt proc edit PROC@1                          # Open in editor
```

Process types: `machining`, `assembly`, `inspection`, `test`, `finishing`, `packaging`, `handling`, `heat_treat`, `welding`, `coating`

### Control Plan Items (SPC, Inspection)

```bash
tdt ctrl new --title "Bore Diameter SPC" --type spc --process PROC@1
tdt ctrl new --title "Visual Check" --type visual --critical
tdt ctrl list                                 # List all controls
tdt ctrl list --type spc                      # Filter by control type
tdt ctrl list --process PROC@1                # Filter by process
tdt ctrl list --critical                      # Show only CTQ controls
tdt ctrl show CTRL@1                          # Show details
tdt ctrl edit CTRL@1                          # Open in editor
```

Control types: `spc`, `inspection`, `poka_yoke`, `visual`, `functional_test`, `attribute`

### Work Instructions

```bash
tdt work new --title "CNC Mill Setup" --process PROC@1 --doc-number "WI-MACH-001"
tdt work list                                 # List all work instructions
tdt work list --process PROC@1                # Filter by process
tdt work list --search "setup"                # Search in title
tdt work show WORK@1                          # Show details
tdt work edit WORK@1                          # Open in editor
```

### Non-Conformance Reports (NCRs)

```bash
tdt ncr new --title "Bore Diameter Out of Tolerance" --type internal --severity major
tdt ncr new --title "Supplier Material Issue" --type supplier --severity critical --category material
tdt ncr list                                  # List all NCRs
tdt ncr list --type internal                  # Filter by NCR type
tdt ncr list --severity critical              # Filter by severity
tdt ncr list --ncr-status open                # Filter by workflow status
tdt ncr show NCR@1                            # Show details
tdt ncr edit NCR@1                            # Open in editor
```

NCR types: `internal`, `supplier`, `customer`
Severity levels: `minor`, `major`, `critical`
Categories: `dimensional`, `cosmetic`, `material`, `functional`, `documentation`, `process`, `packaging`

### Corrective/Preventive Actions (CAPAs)

```bash
tdt capa new --title "Tool Wear Detection" --type corrective --ncr NCR@1
tdt capa new --title "Process Improvement" --type preventive --source trend_analysis
tdt capa list                                 # List all CAPAs
tdt capa list --type corrective               # Filter by CAPA type
tdt capa list --capa-status implementation    # Filter by workflow status
tdt capa list --overdue                       # Show overdue CAPAs
tdt capa show CAPA@1                          # Show details
tdt capa edit CAPA@1                          # Open in editor
```

CAPA types: `corrective`, `preventive`
Source types: `ncr`, `audit`, `customer_complaint`, `trend_analysis`, `risk`

### Link Management

```bash
tdt link add REQ-01 --type satisfied_by REQ-02    # Add link
tdt link remove REQ-01 --type satisfied_by REQ-02 # Remove link
tdt link show REQ-01                               # Show all links
tdt link check                                     # Check for broken links
```

### Traceability

```bash
tdt trace matrix                  # Show traceability matrix
tdt trace matrix --output csv     # Export as CSV
tdt trace matrix --output dot     # Export as GraphViz DOT
tdt trace from REQ-01             # What depends on this?
tdt trace to REQ-01               # What does this depend on?
tdt trace orphans                 # Find unlinked entities
tdt trace coverage                # Verification coverage report
tdt trace coverage --uncovered    # Show uncovered requirements
```

## Example Workflows

TDT is more than requirements tracking. Here are some powerful workflows:

### Tolerance Stackup Analysis

Define features on components, create mates, and run worst-case, RSS, and Monte Carlo analysis:

```bash
# Create a component
$ tdt cmp new --title "Housing" --part-number "PN-001"
✓ Created component CMP@1

# Add features with tolerances
$ tdt feat new --component CMP@1 --type hole --title "Bore"
✓ Created feature FEAT@1

$ tdt feat new --component CMP@1 --type shaft --title "Locating Pin"
✓ Created feature FEAT@2

# Create a mate to analyze fit
$ tdt mate new --feature-a FEAT@1 --feature-b FEAT@2 --title "Pin-Hole Fit"
✓ Created mate MATE@1

# Check the fit calculation
$ tdt mate show MATE@1
Fit Analysis:
  Worst-case clearance: 0.02 to 0.15 mm
  Result: CLEARANCE FIT ✓

# Create a tolerance stackup
$ tdt tol new --title "Gap Analysis" --target-nominal 1.0 --target-upper 1.5 --target-lower 0.5
✓ Created stackup TOL@1

# Run Monte Carlo analysis
$ tdt tol analyze TOL@1 --iterations 50000
═══════════════════════════════════════════
  Tolerance Stackup: Gap Analysis
═══════════════════════════════════════════

  Worst Case: 0.87 to 1.18 mm      PASS
  RSS (3σ):   0.89 to 1.11 mm      Cpk: 4.56
  Monte Carlo: 99.98% yield (50000 iterations)
```

### FMEA Risk Management

Track design and process risks with full FMEA methodology:

```bash
# Create a risk with FMEA ratings
$ tdt risk new --title "Motor Overheating" -t design \
    --severity 8 --occurrence 4 --detection 5
✓ Created risk RISK@1 (RPN: 160, Level: high)

# View risks sorted by RPN
$ tdt risk list --by-rpn
@        ID              TYPE      TITLE                    RPN    LEVEL
------------------------------------------------------------------------
RISK@1   RISK-01HC2...   design    Motor Overheating        160    high
RISK@2   RISK-01HC3...   process   Seal Degradation          84    medium
RISK@3   RISK-01HC4...   design    EMI Susceptibility        48    low

# Find unmitigated risks
$ tdt risk list --unmitigated
2 risks without mitigations
```

### Manufacturing Quality Loop

Define processes, controls, and track quality issues through to resolution:

```bash
# Define a manufacturing process
$ tdt proc new --title "CNC Milling" --type machining --op-number "OP-010"
✓ Created process PROC@1

# Add an SPC control point
$ tdt ctrl new --title "Bore Diameter SPC" --type spc --process PROC@1 --critical
✓ Created control CTRL@1

# Create work instructions
$ tdt work new --title "Mill Setup" --process PROC@1 --doc-number "WI-MACH-001"
✓ Created work instruction WORK@1

# Log a non-conformance
$ tdt ncr new --title "Bore Out of Tolerance" --severity major --type internal
✓ Created NCR NCR@1

# Create corrective action linked to NCR
$ tdt capa new --title "Tool Wear Detection" --type corrective --ncr NCR@1
✓ Created CAPA CAPA@1 linked to NCR@1

# Track overdue CAPAs
$ tdt capa list --overdue
1 overdue CAPA(s)
```

### Full Traceability

Link requirements to tests, risks to mitigations, and generate coverage reports:

```bash
# Link a requirement to a test
$ tdt link add REQ@1 --type verified_by TEST@1
✓ Added link: REQ@1 --verified_by--> TEST@1

# Trace from a requirement
$ tdt trace from REQ@1
REQ@1 Operating Temperature Range
├── satisfied_by → REQ@3 Thermal Management Spec
├── verified_by  → TEST@1 Thermal Chamber Test
└── related_to   → RISK@1 Overheating Risk

# Check verification coverage
$ tdt trace coverage
Requirements: 24 total, 22 verified (92%)
Uncovered: REQ@12, REQ@18

# Find orphaned entities
$ tdt trace orphans
3 requirements with no links
2 tests with no linked requirements
```

### Supply Chain Management

Track suppliers, quotes, and compare pricing:

```bash
# Create a supplier
$ tdt sup new --name "Acme Manufacturing" --short-name "Acme"
✓ Created supplier SUP@1

# Add a quote for a component
$ tdt quote new --component CMP@1 --supplier SUP@1 --price 12.50 --lead-time 14
✓ Created quote QUOT@1

# Compare quotes for a component
$ tdt quote compare CMP@1
Component: Widget Bracket (CMP@1)
───────────────────────────────────────────
Supplier         Price    Lead Time   MOQ
───────────────────────────────────────────
Acme Corp        $12.50   14 days     100
Quality Parts    $13.75   21 days      50
Budget Metals    $10.00   45 days     500
```

> **Note:** For complete YAML schema documentation and field references, see the individual entity docs in the [docs/](docs/) directory.

## Manufacturing Quality Loop

TDT supports the complete manufacturing quality loop:

```
┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│    PROC      │────▶│    CTRL      │────▶│    WORK      │
│  (Process)   │     │  (Control)   │     │ (Work Inst)  │
└──────────────┘     └──────────────┘     └──────────────┘
       │                    │                    │
       │                    ▼                    │
       │             ┌──────────────┐            │
       │             │    NCR       │◀───────────┘
       │             │ (Non-Conf)   │
       │             └──────────────┘
       │                    │
       │                    ▼
       │             ┌──────────────┐
       └────────────▶│    CAPA      │
                     │  (Corrective)│
                     └──────────────┘
```

1. **PROC** defines *what* manufacturing operations to perform
2. **CTRL** defines *how* to monitor/control the process (SPC, inspection)
3. **WORK** provides step-by-step instructions for *operators*
4. **NCR** captures quality issues found during manufacturing
5. **CAPA** drives systematic improvement back to processes

## Tolerance Format

TDT uses `plus_tol` and `minus_tol` fields instead of the `±` symbol (which is hard to type):

```yaml
# Correct: 10.0 +0.1/-0.05
dimensions:
  - name: "diameter"
    nominal: 10.0
    plus_tol: 0.1     # Positive deviation allowed
    minus_tol: 0.05   # Negative deviation allowed (stored as positive number)
```

Both values are stored as **positive numbers**. The actual tolerance range is:
- Maximum: `nominal + plus_tol` = 10.1
- Minimum: `nominal - minus_tol` = 9.95

## Validation

TDT validates files against JSON Schema with detailed error messages:

```
error[tdt::schema::validation]: Schema validation failed
  --> requirements/inputs/REQ-01HC2.tdt.yaml:8:1
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

TDT uses FMEA (Failure Mode and Effects Analysis) methodology:

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

## Tolerance Analysis

TDT supports three analysis methods for tolerance stackups:

### Worst-Case Analysis

Assumes all dimensions are at their worst-case limits simultaneously:
- **Min result**: All positive contributors at minimum, all negative at maximum
- **Max result**: All positive contributors at maximum, all negative at minimum
- **Conservative** but often overly pessimistic

### RSS (Root Sum Square) Analysis

Statistical analysis assuming normal distributions:
- Calculates mean and 3σ spread
- Computes Cpk (process capability index)
- Estimates yield percentage
- More realistic than worst-case for multi-contributor stacks

| Cpk | Yield | Quality Level |
|-----|-------|---------------|
| 0.33 | 68.27% | Poor |
| 0.67 | 95.45% | Marginal |
| 1.0 | 99.73% | Capable |
| 1.33 | 99.99% | Good |
| 1.67 | 99.9997% | Excellent |
| 2.0 | 99.9999% | Six Sigma |

### Monte Carlo Simulation

Runs thousands of random samples:
- Supports normal, uniform, and triangular distributions
- Provides actual yield percentage
- Reports 95% confidence interval (2.5th to 97.5th percentile)
- Default: 10,000 iterations

```bash
# Run analysis with default iterations
tdt tol analyze TOL@1

# Run with more iterations for higher confidence
tdt tol analyze TOL@1 --iterations 100000
```

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
