# Tessera

**Plain-text engineering artifact management with full traceability**

A CLI and desktop application for managing requirements, risks, tests, BOMs, tolerance analysis, manufacturing, and quality data as human-readable YAML files under git version control.

🌐 **Website:** [tessera-engineering.com](https://tessera-engineering.com)

![Tessera Quick Start Demo](docs/quickstart.gif)

---

## Table of Contents

- [Features](#features)
- [Installation](#installation)
  - [CLI Tool](#cli-tool)
  - [Desktop App (Beta)](#desktop-app-beta)
- [Quick Start](#quick-start)
- [Short IDs](#short-ids)
- [Quick Start Workflows](#quick-start-workflows)
- [Project Structure](#project-structure)
- [Entity Types](#entity-types)
- [Output Formats](#output-formats)
- [Commands](#commands)
  - [Project Management](#project-management)
  - [Configuration](#configuration)
  - [Global Search](#global-search)
  - [Requirements](#requirements)
  - [Risks (FMEA)](#risks-fmea)
  - [Tests](#tests-verificationvalidation)
  - [Test Results](#test-results)
  - [Components (BOM)](#components-bom)
  - [Assemblies (BOM)](#assemblies-bom)
  - [Suppliers](#suppliers-approved-vendors)
  - [Quotes](#quotes-supplier-quotations)
  - [Features (Tolerances)](#features-tolerances)
  - [Mates (Tolerances)](#mates-tolerances)
  - [Stackups (Tolerance Analysis)](#stackups-tolerance-analysis)
  - [Manufacturing Processes](#manufacturing-processes)
  - [Control Plan Items](#control-plan-items-spc-inspection)
  - [Work Instructions](#work-instructions)
  - [Non-Conformance Reports](#non-conformance-reports-ncrs)
  - [Corrective/Preventive Actions](#correctivepreventive-actions-capas)
  - [Link Management](#link-management)
  - [Traceability](#traceability)
  - [Where-Used Queries](#where-used-queries)
  - [Reports](#reports)
  - [Version Control](#version-control-git-wrappers)
  - [Baselines](#baselines-git-tags)
  - [Bulk Operations](#bulk-operations)
  - [CSV Import](#csv-import)
  - [SysML v2 Interchange](#sysml-v2-interchange)
- [Example Workflows](#example-workflows)
- [Manufacturing Quality Loop](#manufacturing-quality-loop)
- [Tolerance Format](#tolerance-format)
- [Validation](#validation)
- [Status Workflow](#status-workflow)
- [Priority Levels](#priority-levels)
- [Risk Assessment (FMEA)](#risk-assessment-fmea)
- [Test Engineering](#test-engineering)
- [Tolerance Analysis](#tolerance-analysis)
- [Best Practices](#best-practices)
- [License](#license)
- [Disclaimer](#disclaimer)

---

## Features

- **Plain-text YAML files** - Human-readable, git-friendly, diff-able
- **Schema validation** - JSON Schema validation with helpful error messages
- **Traceability** - Link entities together and generate traceability matrices
- **ULID-based IDs** - Unique, sortable identifiers for all entities
- **Short ID aliases** - Use `REQ@1`, `RISK@2`, etc. instead of typing long IDs
- **Delete & Archive** - Safe deletion with link checking, or archive entities for later reference
- **Beautiful error messages** - Line numbers, context, and actionable suggestions
- **FMEA Risk Management** - Built-in support for Failure Mode and Effects Analysis
- **BOM Management** - Components and assemblies with supplier tracking
- **Tolerance Analysis** - Features, mates, and stackups with worst-case, RSS, Monte Carlo, and 3D SDT analysis
- **SysML v2 Interchange** - Export and import SysML v2 textual notation for MBSE integration
- **Desktop App (Beta)** - Full-featured GUI for visual entity management

---

## Installation

### CLI Tool

The command-line tool is called `tdt` (Tessera Design Toolkit).

#### Pre-built Binaries (Recommended)

Download the latest release for your platform from [GitHub Releases](https://github.com/jackhale98/Tessera/releases):

```bash
# Linux (x64)
curl -sSL https://github.com/jackhale98/Tessera/releases/latest/download/tdt-linux-x64.tar.gz | tar xz
sudo mv tdt /usr/local/bin/

# macOS (Apple Silicon)
curl -sSL https://github.com/jackhale98/Tessera/releases/latest/download/tdt-macos-arm64.tar.gz | tar xz
sudo mv tdt /usr/local/bin/

# macOS (Intel)
curl -sSL https://github.com/jackhale98/Tessera/releases/latest/download/tdt-macos-x64.tar.gz | tar xz
sudo mv tdt /usr/local/bin/

# Windows - download tdt-windows-x64.zip and extract to your PATH
```

#### From crates.io

```bash
cargo install tessera-design-toolkit
```

This installs the `tdt` binary. Note: compilation takes a few minutes due to dependencies.

#### From Source

```bash
git clone https://github.com/jackhale98/Tessera.git
cd Tessera
cargo build -p tessera-design-toolkit --release
# Binary will be at target/release/tdt
```

### Desktop App (Beta)

Tessera Desktop provides a full-featured graphical interface for managing your engineering artifacts. The desktop app is currently in **beta**.

Download the latest desktop release for your platform from [GitHub Releases](https://github.com/jackhale98/Tessera/releases):

| Platform | Download |
|----------|----------|
| Windows (x64) | `tessera-desktop-windows-x64.msi` or `.exe` |
| macOS (Apple Silicon) | `tessera-desktop-macos-arm64.dmg` |
| macOS (Intel) | `tessera-desktop-macos-x64.dmg` |
| Linux (x64) | `tessera-desktop-linux-x64.AppImage` or `.deb` |

**Features:**
- Visual entity management with forms and tables
- Risk analytics dashboard with FMEA worksheet
- Traceability matrix visualization
- BOM cost and mass rollup
- Git integration for version control

> **Note:** The desktop app works with the same project structure as the CLI. You can use both interchangeably.

---

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

---

## Short IDs

After running `list` commands, Tessera assigns entity-prefixed short IDs (`REQ@1`, `RISK@1`, etc.) to entities:

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
tdt link add REQ@1 TEST@1
tdt trace from REQ@1
```

Short IDs are persistent per entity type - the same entity keeps its short ID across list commands.
This enables cross-entity linking (e.g., linking `REQ@1` to `TEST@1`).

---

## Quick Start Workflows

Common multi-step tasks made easy:

### Record a Test Result
```bash
tdt test list                          # Find test to execute
tdt test run TEST@1 --verdict pass     # Record pass result (creates RSLT)
tdt rslt summary                       # Check overall test status
```

### Close an NCR
```bash
tdt ncr list --ncr-status open         # Find open NCRs
tdt ncr show NCR@1                     # Review details
tdt ncr close NCR@1 -d rework --rationale "Re-machined to spec"
```

### Verify a CAPA
```bash
tdt capa list --capa-status implementation  # Find CAPAs ready for verification
tdt capa verify CAPA@1 --result effective --evidence "30-day audit"
```

### Analyze Component Interactions
```bash
tdt dsm                                # View design structure matrix
tdt tol analyze TOL@1                  # Run tolerance stackup analysis
```

### Risk Assessment Review
```bash
tdt risk matrix                        # Visual severity × occurrence grid
tdt risk list --by-rpn -n 5            # Top 5 risks by RPN
```

### Manufacturing Process Review
```bash
tdt proc flow --controls               # Process flow with control points
tdt ctrl list --critical               # Review CTQ controls
```

---

## Project Structure

After `tdt init`, your project will have:

```
.tdt/
├── config.yaml              # Project configuration
├── cache.db                 # SQLite cache (auto-generated on first query)
├── schema/                  # JSON schemas (auto-generated)
└── archive/                 # Archived entities (from `tdt <entity> archive`)

requirements/
├── inputs/                  # Design inputs (customer requirements)
└── outputs/                 # Design outputs (specifications)

risks/
├── hazards/                 # Hazard definitions
├── design/                  # Design risks
├── process/                 # Process risks
├── use/                     # Use risks
└── software/                # Software risks

bom/
├── assemblies/              # Assembly definitions
├── components/              # Component definitions
├── suppliers/               # Approved supplier list
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
├── lots/                    # Production lots/batches
├── deviations/              # Process deviations
├── ncrs/                    # Non-conformance reports
└── capas/                   # Corrective/preventive actions
```

---

## Entity Types

| Prefix | Entity | Description |
|--------|--------|-------------|
| REQ | Requirement | Design inputs and outputs |
| HAZ | Hazard | Hazard definition |
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
| LOT | Lot | Production lot / batch |
| DEV | Deviation | Process deviation |
| NCR | Non-Conformance | Non-conformance report |
| CAPA | CAPA | Corrective/preventive action |
| QUOT | Quote | Quote / cost record |
| SUP | Supplier | Approved supplier |

---

## Output Formats

Use `-o/--output` to control output format:

```bash
tdt req list -o json        # JSON output (for scripting)
tdt req list -o yaml        # YAML output
tdt req list -o csv         # CSV output (for spreadsheets)
tdt req list -o tsv         # Tab-separated (default for lists)
tdt req list -o md          # Markdown table
tdt req list -o id          # Just IDs, one per line

tdt req show REQ-01 -o json # Full entity as JSON
tdt req show REQ-01 -o yaml # Full entity as YAML
```

---

## Commands

### Project Management

```bash
tdt init                    # Initialize a new project
tdt init --git              # Initialize with git repository
tdt validate                # Validate all project files
tdt validate --fail-fast    # Stop on first error (default: validate all)
tdt validate --summary      # Show summary only
tdt validate --fix          # Auto-fix calculated values (RPN, risk level)
tdt validate --strict       # Treat warnings as errors
```

### Configuration

```bash
tdt config show                       # Show effective (merged) configuration
tdt config show author                # Show specific key's value
tdt config show --project-only        # Show only project-level config
tdt config show --global-only         # Show only user-level config
tdt config set author "Jane Doe"      # Set in project config
tdt config set author "Jane Doe" -g   # Set in global (user) config
tdt config unset author               # Remove from project config
tdt config unset author -g            # Remove from global config
tdt config path                       # Show config file paths
tdt config keys                       # List all available config keys
```

Configuration is layered (highest priority first):
1. Environment variables (`TDT_AUTHOR`, `TDT_EDITOR`)
2. Project config (`.tdt/config.yaml`)
3. Global user config (`~/.config/tdt/config.yaml`)

### Global Search

```bash
tdt search "temperature"              # Search all entities
tdt search "motor" -t req,risk        # Search specific entity types
tdt search "draft" --status draft     # Filter by status
tdt search "keyword" --author "Jane"  # Filter by author
tdt search "v2" --tag "release"       # Filter by tag
tdt search "term" --count             # Show count only
tdt search "term" -o json             # Output as JSON
```

### Recent Activity

```bash
tdt recent                            # Show 20 most recently modified entities
tdt recent -n 50                      # Show 50 most recent
tdt recent -t req,risk                # Filter by entity type
tdt recent -o short-id                # Output short IDs (for piping)
tdt recent --count                    # Show count only
```

### Tags

```bash
tdt tags list                         # List all tags with usage counts
tdt tags list -n 10                   # Show top 10 tags
tdt tags list -o json                 # Output as JSON
tdt tags show precision               # Show entities with tag "precision"
tdt tags show thermal -o id           # Output full IDs (for piping)
tdt tags show urgent --count          # Count entities with tag
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
tdt req list --linked-to CMP@1       # Requirements linked to a component
tdt req show REQ-01HC2                # Show details (partial ID match)
tdt req edit REQ-01HC2                # Open in editor
tdt req delete REQ@1                  # Permanently delete (checks for links)
tdt req delete REQ@1 --force          # Delete even if referenced
tdt req archive REQ@1                 # Move to .tdt/archive/ instead of deleting
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
tdt risk list --linked-to CMP@1       # Risks affecting a component
tdt risk show RISK-01HC2               # Show details
tdt risk edit RISK-01HC2               # Open in editor
tdt risk delete RISK@1                 # Permanently delete
tdt risk archive RISK@1                # Move to archive
tdt risk matrix                        # Severity × Occurrence risk matrix
tdt risk matrix --show-ids             # Show risk IDs in cells
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
tdt test list --linked-to REQ@1 --via verified_by  # Tests verifying a requirement
tdt test show TEST-01HC2                      # Show details
tdt test edit TEST-01HC2                      # Open in editor
tdt test delete TEST@1                        # Permanently delete
tdt test archive TEST@1                       # Move to archive
tdt test run TEST@1 --verdict pass            # Execute test and record result
tdt test run TEST@1 --verdict fail --notes "See NCR@1"  # Record failure with notes
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
tdt rslt delete RSLT@1                        # Permanently delete
tdt rslt archive RSLT@1                       # Move to archive
tdt rslt summary                              # Test execution statistics dashboard
tdt rslt summary --detailed                   # Breakdown by test type
```

### Components (BOM)

```bash
tdt cmp new                                   # Create with template
tdt cmp new --title "Motor Assembly" --part-number "PN-001"
tdt cmp new --make-buy buy --category mechanical
tdt cmp list                                  # List all components
tdt cmp list --make-buy buy                   # Filter by make/buy
tdt cmp list --category electrical            # Filter by category
tdt cmp list --assembly ASM@1                 # Components in assembly's BOM
tdt cmp show CMP@1                            # Show details
tdt cmp edit CMP@1                            # Open in editor
tdt cmp delete CMP@1                          # Permanently delete
tdt cmp archive CMP@1                         # Move to archive
tdt dsm                                       # Design structure matrix (component interactions)
tdt dsm --full-ids                            # Show full component IDs in cells
```

### Assemblies (BOM)

```bash
tdt asm new                                   # Create with template
tdt asm new --title "Main Assembly" --part-number "ASM-001"
tdt asm list                                  # List all assemblies
tdt asm list --assembly ASM@1                 # Sub-assemblies within ASM@1
tdt asm show ASM@1                            # Show details
tdt asm bom ASM@1                             # Show flattened BOM
tdt asm cost ASM@1                            # Calculate recursive BOM cost
tdt asm mass ASM@1                            # Calculate recursive BOM mass
tdt asm edit ASM@1                            # Open in editor
tdt asm delete ASM@1                          # Permanently delete
tdt asm archive ASM@1                         # Move to archive
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
tdt sup delete SUP@1                          # Permanently delete
tdt sup archive SUP@1                         # Move to archive
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
tdt quote delete QUOT@1                       # Permanently delete
tdt quote archive QUOT@1                      # Move to archive
```

### Features (Tolerances)

```bash
tdt feat new --component CMP@1 --feature-type hole --title "Mounting Hole"
tdt feat new --component CMP@1 --feature-type shaft   # Feature requires parent component
tdt feat list                                 # List all features
tdt feat list --component CMP@1               # Filter by component
tdt feat list --feature-type hole                     # Filter by type
tdt feat show FEAT@1                          # Show details
tdt feat edit FEAT@1                          # Open in editor
tdt feat delete FEAT@1                        # Permanently delete
tdt feat archive FEAT@1                       # Move to archive
```

### Mates (Tolerances)

```bash
tdt mate new --feature-a FEAT@1 --feature-b FEAT@2 --title "Pin-Hole Fit"
tdt mate list                                 # List all mates
tdt mate list --mate-type clearance                # Filter by mate type
tdt mate show MATE@1                          # Show details with fit calculation
tdt mate recalc MATE@1                        # Recalculate fit from features
tdt mate edit MATE@1                          # Open in editor
tdt mate delete MATE@1                        # Permanently delete
tdt mate archive MATE@1                       # Move to archive
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
tdt tol analyze TOL@1 --3d                    # 3D SDT torsor-based analysis
tdt tol analyze TOL@1 --3d --visualize        # 3D with braille visualization
tdt tol edit TOL@1                            # Open in editor
tdt tol delete TOL@1                          # Permanently delete
tdt tol archive TOL@1                         # Move to archive
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
tdt proc delete PROC@1                        # Permanently delete
tdt proc archive PROC@1                       # Move to archive
tdt proc flow                                 # Visualize process flow with controls
tdt proc flow --controls                      # Show linked control points
tdt proc flow --work-instructions             # Show linked work instructions
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
tdt ctrl delete CTRL@1                        # Permanently delete
tdt ctrl archive CTRL@1                       # Move to archive
```

Control types: `spc`, `inspection`, `poka-yoke`, `visual`, `functional-test`, `attribute`

### Work Instructions

```bash
tdt work new --title "CNC Mill Setup" --process PROC@1 --doc-number "WI-MACH-001"
tdt work list                                 # List all work instructions
tdt work list --process PROC@1                # Filter by process
tdt work list --search "setup"                # Search in title
tdt work show WORK@1                          # Show details
tdt work edit WORK@1                          # Open in editor
tdt work delete WORK@1                        # Permanently delete
tdt work archive WORK@1                       # Move to archive
```

### Non-Conformance Reports (NCRs)

```bash
tdt ncr new --title "Bore Diameter Out of Tolerance" --type internal --severity major
tdt ncr new --title "Supplier Material Issue" --type supplier --severity critical --category material
tdt ncr list                                  # List all NCRs
tdt ncr list --type internal                  # Filter by NCR type
tdt ncr list --severity critical              # Filter by severity
tdt ncr list --ncr-status open                # Filter by workflow status
tdt ncr list --linked-to CMP@1               # NCRs linked to a component
tdt cmp list -o short-id | tdt ncr list --linked-to -  # NCRs for piped components
tdt ncr show NCR@1                            # Show details
tdt ncr edit NCR@1                            # Open in editor
tdt ncr delete NCR@1                          # Permanently delete
tdt ncr archive NCR@1                         # Move to archive
tdt ncr close NCR@1 --disposition rework      # Close with disposition
tdt ncr close NCR@1 -d use-as-is --rationale "Within tolerance"
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
tdt capa list --linked-to NCR@1              # CAPAs linked to an NCR
tdt capa show CAPA@1                          # Show details
tdt capa edit CAPA@1                          # Open in editor
tdt capa delete CAPA@1                        # Permanently delete
tdt capa archive CAPA@1                       # Move to archive
tdt capa verify CAPA@1 --result effective     # Record effectiveness verification
tdt capa verify CAPA@1 -r partial --evidence "Process audit"
```

CAPA types: `corrective`, `preventive`
Source types: `ncr`, `audit`, `customer_complaint`, `trend_analysis`, `risk`

### Link Management

```bash
tdt link add REQ@1 REQ@2 -t satisfied_by    # Add link
tdt link remove REQ@1 REQ@2 -t satisfied_by # Remove link
tdt link show REQ@1                          # Show all links
tdt link check                               # Check for broken links
```

### Traceability

```bash
tdt trace matrix                  # Show traceability matrix
tdt trace matrix -o csv           # Export as CSV
tdt trace matrix -o dot           # Export as GraphViz DOT
tdt trace from REQ@1              # What depends on this?
tdt trace from REQ@1 REQ@2        # Trace from multiple entities
tdt req list -o short-id | tdt trace from -  # Trace from piped IDs
tdt trace to REQ@1                # What does this depend on?
tdt trace orphans                 # Find unlinked entities
```

### Where-Used Queries

```bash
tdt where-used CMP@1              # Find where a component is used
tdt where-used FEAT@3             # Find stackups/mates using a feature
tdt where-used REQ@5              # Find what references a requirement
tdt where-used CMP@1 --direct-only  # Show only direct references
```

### Reports

```bash
tdt report rvm                    # Requirements Verification Matrix
tdt report rvm -f report.csv      # Export RVM to file
tdt report fmea                   # FMEA report sorted by RPN
tdt report fmea --min-rpn 100     # Show risks above RPN threshold
tdt report bom ASM@1              # Indented BOM with costs/masses
tdt report test-status            # Test execution summary
tdt report open-issues            # All open NCRs, CAPAs, failed tests
```

### Risk Summary

```bash
tdt risk summary                  # Risk statistics overview
tdt risk summary -n 5             # Show top N risks by RPN
tdt risk summary -o json          # Output as JSON (for CI/CD)
```

### Project Status Dashboard

```bash
tdt status                        # Full project status dashboard
tdt status --section requirements # Show only requirements metrics
tdt status --section risks        # Show only risk metrics
tdt status --detailed             # Show detailed breakdown
tdt status -o json                # Output as JSON (for CI/CD)
```

### Version Control (Git Wrappers)

```bash
# History - view git history for an entity
tdt history REQ@1                 # Show commit history
tdt history REQ@1 -n 10           # Last 10 commits
tdt history REQ@1 --since 2024-01-01  # Since date
tdt history REQ@1 --full          # Show full commit messages

# Blame - view git blame for an entity
tdt blame REQ@1                   # Show who changed each line
tdt blame REQ@1 -L 10-20          # Blame specific line range

# Diff - view git diff for an entity
tdt diff REQ@1                    # Show working changes
tdt diff REQ@1 HEAD~1             # Diff vs previous commit
tdt diff REQ@1 v1.0..v2.0         # Diff between tags
```

### Baselines (Git Tags)

```bash
tdt baseline create v1.0          # Validate, then create git tag
tdt baseline create v1.0 -m "Release 1.0"  # With message
tdt baseline list                 # List all Tessera baselines
tdt baseline compare v1.0 v2.0    # Show what changed between versions
tdt baseline changed v1.0         # List entities changed since baseline
```

### Bulk Operations

```bash
# Set status on multiple entities
tdt bulk set-status approved REQ@1 REQ@2 REQ@3
tdt bulk set-status review -t req --dry-run

# Add/remove tags
tdt bulk add-tag "v2.0" CMP@1 CMP@2 CMP@3
tdt bulk remove-tag "deprecated" -t cmp --all

# Set author
tdt bulk set-author "Jane Doe" REQ@1 REQ@2

# Dry run (preview changes without modifying)
tdt bulk set-status approved -t req --dry-run
```

#### Unix Pipeline Support

Bulk commands read entity IDs from stdin, enabling Unix-style pipelines:

```bash
# Pipe from list commands
tdt req list --output id | tdt bulk set-status approved
tdt req list --unverified --output id | tdt bulk add-tag needs-review
tdt risk list --level high --output id | tdt bulk add-tag urgent

# Combine with standard Unix tools
tdt req list --output id | grep "input" | tdt bulk set-status review
tdt cmp list --output id | head -5 | tdt bulk add-tag "prototype"

# Chain multiple operations
tdt req list --status draft --output id | tdt bulk set-status review
```

This works with all entity types and all bulk commands (`set-status`, `add-tag`, `remove-tag`, `set-author`).

#### Cross-Entity Filtering (`--linked-to`)

Every `list` command supports `--linked-to` to filter entities by their links to other entities. Combine with `-` to read IDs from stdin for powerful pipe composition:

```bash
# Show all NCRs linked to a specific component
tdt ncr list --linked-to CMP@1

# Pipe component IDs to find all linked NCRs
tdt cmp list -o short-id | tdt ncr list --linked-to -

# Find tests linked to approved requirements
tdt req list --status approved -o short-id | tdt test list --linked-to -

# Show risks linked to components in an assembly's BOM
tdt asm bom ASM@1 -o short-id | tdt risk list --linked-to -

# Filter by link type with --via
tdt test list --linked-to REQ@1 --via verified_by
tdt req list --linked-to CMP@1 --via satisfied_by

# Pipe multiple IDs to trace commands
echo -e "REQ@1\nREQ@2" | tdt trace from -
tdt req list -o short-id | tdt trace from -
```

The `--linked-to` flag accepts comma-separated IDs, short IDs, or `-` for stdin. The optional `--via` flag filters by a specific link type (e.g., `verified_by`, `satisfied_by`, `mitigated_by`).

### CSV Import

Bulk-import entities from CSV files. Supports all major entity types with flexible headers, default values, and dry-run previews.

```bash
# Generate a CSV template for any entity type
tdt import --template req > requirements.csv
tdt import --template cmp > components.csv
tdt import --template risk > risks.csv

# Import entities from CSV
tdt import req requirements.csv              # Import requirements
tdt import cmp components.csv                # Import components
tdt import risk risks.csv                    # Import risks/FMEA items

# Preview without creating files
tdt import req requirements.csv --dry-run

# Continue past errors instead of stopping on first failure
tdt import req requirements.csv --skip-errors

# Set default parent references for child entities
tdt import feat features.csv --component CMP@1    # Features for a component
tdt import ctrl controls.csv --process PROC@1      # Controls for a process
tdt import rslt results.csv --test TEST@1          # Results for a test
tdt import quote quotes.csv --supplier SUP@1       # Quotes from a supplier
tdt import cmp components.csv --assembly ASM@1     # Components in an assembly
```

**Supported entity types:** `req`, `risk`, `cmp`, `asm`, `sup`, `quote`, `test`, `rslt`, `proc`, `ctrl`, `ncr`, `capa`, `feat`

CSV headers are case-insensitive and flexible. Required fields vary by entity type (typically just `title`). Use `--template` to see all available columns for a given type.

### SysML v2 Interchange

Export project data to SysML v2 textual notation for use with MBSE tools, or import SysML v2 files to create Tessera entities.

```bash
# Export all requirements, tests, components, and results to SysML v2
tdt export sysml                          # Output to stdout
tdt export sysml -f model.sysml           # Write to file
tdt export sysml --package MyProject      # Custom package name

# Import from a SysML v2 file
tdt import sysml model.sysml             # Create entities from SysML
tdt import sysml model.sysml --dry-run   # Preview without creating files
tdt import sysml model.sysml --update    # Update existing entities
```

**Export** generates SysML v2 textual notation with:
- `requirement def` blocks for requirements with `doc` and `@TdtMetadata` annotations
- `verification def` blocks for tests with `@VerificationMethod` and `objective` linking
- `part def` blocks for components
- `satisfy requirement : X by Y;` relationships from `satisfied_by` links
- Test results as structured comments with verdict and execution date

**Import** parses SysML v2 and creates Tessera entity YAML files with:
- Round-trip ID preservation (TDT entity IDs in SysML short IDs are reused)
- Bidirectional link restoration (`verifies`/`verified_by`, `satisfied_by`)
- Metadata recovery from `@TdtMetadata` annotations (status, author, category, tags)

---

## Example Workflows

Tessera is more than requirements tracking. Here are some powerful workflows:

### Tolerance Stackup Analysis

Define features on components, create mates, and run worst-case, RSS, and Monte Carlo analysis:

```bash
# Create a component
$ tdt cmp new -p "PN-001" -t "Housing"
✓ Created component CMP@1
   Part: PN-001 | Housing

# Add features with tolerances
$ tdt feat new -c CMP@1 -t hole --title "Bore"
✓ Created feature FEAT@1
   Parent: CMP-01KC5W... | Type: hole | Bore

# Create a mate to analyze fit
$ tdt mate new -a FEAT@1 -b FEAT@2 --title "Pin-Hole Fit"
✓ Created mate MATE@1
   Fit Analysis:
     Result: transition (-0.1500 to 0.1500)

# Create a tolerance stackup
$ tdt tol new -t "Gap Analysis" --target-nominal 1.0 --target-upper 1.5 --target-lower 0.5
✓ Created stackup TOL@1
   Target: Target = 1.000 (LSL: 0.500, USL: 1.500)

# Run Monte Carlo analysis
$ tdt tol analyze TOL@1 --iterations 10000
⚙ Analyzing stackup TOL@1...
✓ Analysis complete

   Worst-Case Analysis:
     Range: 0.8700 to 1.1800
     Result: pass

   RSS (Statistical) Analysis:
     Mean: 1.0000 | ±3σ: 0.0750
     Cpk: 4.56 | Yield: 99.99%

   Monte Carlo (10000 iterations):
     Mean: 1.0001 | Std Dev: 0.0249
     Yield: 100.00%
```

### FMEA Risk Management

Track design and process risks with full FMEA methodology:

```bash
# Create a risk with FMEA ratings
$ tdt risk new --title "Motor Overheating" -t design \
    --severity 8 --occurrence 4 --detection 5
✓ Created risk RISK@1
   RPN: 160 (high)

# View risks sorted by RPN
$ tdt risk list --by-rpn
SHORT    ID                TYPE      TITLE                  STATUS  LEVEL   RPN
--------------------------------------------------------------------------------
RISK@1   RISK-01KC5W...    design    Motor Overheating      draft   high    160
RISK@2   RISK-01KC5W...    process   Seal Degradation       draft   medium   72

2 risk(s) found. Use RISK@N to reference by short ID.
```

### Manufacturing Quality Loop

Define processes, controls, and track quality issues through to resolution:

```bash
# Define a manufacturing process
$ tdt proc new -t "CNC Milling" -T machining -n "OP-010"
✓ Created process PROC@1
   Type: machining | CNC Milling

# Add an SPC control point
$ tdt ctrl new -t "Bore Diameter SPC" -T spc -p PROC@1 --critical
✓ Created control CTRL@1
   Type: spc | Bore Diameter SPC [CTQ]

# Create work instructions
$ tdt work new -t "Mill Setup" -p PROC@1 -d "WI-MACH-001"
✓ Created work instruction WORK@1
   Mill Setup
   Doc: WI-MACH-001

# Log a non-conformance
$ tdt ncr new -t "Bore Out of Tolerance" -S major -T internal
✓ Created NCR NCR@1
   internal | major | Bore Out of Tolerance

# Create corrective action linked to NCR
$ tdt capa new -t "Tool Wear Detection" -T corrective --ncr NCR@1
✓ Created CAPA CAPA@1
   corrective | Tool Wear Detection
   Source: NCR-01KC5W...
```

### Full Traceability

Link requirements to tests, risks to mitigations, and generate coverage reports:

```bash
# Link a requirement to a test
$ tdt link add REQ@1 TEST@1 -t verified_by
✓ Added link: REQ-01KC5W... --[verified_by]--> TEST-01KC5W...

# Check verification coverage
$ tdt report rvm
Requirements Verification Matrix
════════════════════════════════════════════════════════════
Total requirements:     24
Verified:               22
Unverified:             2

Coverage: 92%

# Find orphaned entities
$ tdt trace orphans
Orphaned Entities
────────────────────────────────────────────────────────────
○ RISK-01KC5W... - Motor Overheating (no links)
○ NCR-01KC5W...  - Bore Out of Tolerance (no links)

Found 2 orphaned entity(ies)
```

### Supply Chain Management

Track suppliers, quotes, and compare pricing:

```bash
# Create a supplier
$ tdt sup new -n "Acme Manufacturing" --short-name "Acme"
✓ Created supplier SUP@1
   Name: Acme Manufacturing

# Add a quote for a component
$ tdt quote new -c CMP@1 -s SUP@1 --price 12.50 --lead-time 14
✓ Created quote QUOT@1
   Supplier: SUP-01KC5W... | Component: CMP-01KC5W...
   Price: 12.50 | Lead: 14d

# Compare quotes for a component
$ tdt quote compare CMP@1
Comparing 3 quotes for CMP@1

SHORT    SUPPLIER   PRICE      MOQ   LEAD     STATUS
------------------------------------------------------
QUOT@1   SUP@1      12.50      100   14d      pending
QUOT@2   SUP@2      13.75       50   21d      pending
QUOT@3   SUP@3      10.00      500   45d      pending

★ Lowest price: 10.00 from SUP@3
```

### SysML v2 Round-Trip

Export your project to SysML v2 for review in MBSE tools, then import changes back:

```bash
# Export current project state
$ tdt export sysml -f project.sysml
✓ Exported 7 requirements, 4 tests, 4 results, 3 components

# Share with SysML tools, make edits, then re-import
$ tdt import sysml updated_model.sysml
✓ Imported 7 requirements, 4 tests, 3 components
  Links: 4 verified_by, 3 satisfied_by

# Verify imported entities
$ tdt req list
$ tdt report rvm
```

> **Note:** For complete YAML schema documentation and field references, see the individual entity docs in the [docs/](docs/) directory.

---

## Manufacturing Quality Loop

Tessera supports the complete manufacturing quality loop:

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

---

## Tolerance Format

Tessera uses `plus_tol` and `minus_tol` fields instead of the `±` symbol (which is hard to type):

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

---

## Validation

Tessera validates files against JSON Schema with detailed error messages:

```
error[tessera::schema::validation]: Schema validation failed
  --> requirements/inputs/REQ-01HC2.tdt.yaml:8:1
   |
 8 | status: pending
   | ^^^^^^^^^^^^^^^ Invalid enum value
   |
  help: Valid values: draft, review, approved, released, obsolete
```

---

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

---

## Priority Levels

| Priority | Use For |
|----------|---------|
| critical | Safety, regulatory, blocking requirements |
| high | Core functionality, key differentiators |
| medium | Standard features, quality of life |
| low | Nice to have, future considerations |

---

## Risk Assessment (FMEA)

Tessera uses FMEA (Failure Mode and Effects Analysis) methodology:

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

---

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

---

## Tolerance Analysis

Tessera supports four analysis methods for tolerance stackups:

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

### 3D SDT Analysis (Small Displacement Torsor)

Full 3D tolerance analysis using torsor theory:
- **6-DOF Torsors**: 3 translations (u, v, w) + 3 rotations (α, β, γ) per feature
- **Jacobian Propagation**: Transform deviations through kinematic chains with 6×6 matrices
- **Geometry Classes**: Plane, Cylinder, Sphere, Cone - each with invariance constraints
- **ASME Y14.5 Integration**: Works with standard GD&T controls (position, perpendicularity, etc.)
- **Datum Reference Frames**: Supports A, B, C datum hierarchy
- **Braille Visualization**: Terminal-based chain schematics and deviation ellipses

```bash
# Run 1D analysis with default iterations
tdt tol analyze TOL@1

# Run with more iterations for higher confidence
tdt tol analyze TOL@1 --iterations 100000

# Run 3D SDT analysis (requires features with geometry_3d)
tdt tol analyze TOL@1 --3d

# 3D analysis with braille visualization
tdt tol analyze TOL@1 --3d --visualize

# 3D Monte Carlo simulation
tdt tol analyze TOL@1 --3d --iterations 50000
```

### Test Verdicts

| Verdict | Meaning | Follow-up |
|---------|---------|-----------|
| **pass** | All criteria met | None required |
| **fail** | One or more criteria not met | Action items required |
| **conditional** | Passed with deviations | Document justification |
| **incomplete** | Could not complete test | Reschedule |
| **not_applicable** | Test not applicable | Document rationale |

---

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

---

## License

MIT License

Copyright (c) 2024 Tessera Contributors

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.

---

## Disclaimer

This software is provided for informational and educational purposes. While Tessera
can help organize engineering documentation, **it is not a substitute for
professional engineering judgment, certified quality management systems, or
regulatory compliance tools**.

Users are responsible for:
- Validating that outputs meet their specific requirements
- Ensuring compliance with applicable regulations and standards
- Maintaining appropriate backup and version control practices
- Performing independent verification of safety-critical calculations

**Tessera is not certified for use in regulated industries** (medical devices,
aerospace, automotive safety systems, etc.) without independent validation
by qualified professionals.
