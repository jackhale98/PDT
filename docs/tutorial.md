# TDT Tutorial: Building a Complete Product Development Project

This tutorial walks you through creating a complete TDT project from scratch. We'll build a simple **LED Flashlight** product, covering all entity types and showing how they connect together for full traceability.

## Table of Contents

1. [Project Setup](#1-project-setup)
2. [Requirements](#2-requirements)
3. [Components & Bill of Materials](#3-components--bill-of-materials)
4. [Suppliers & Quotes](#4-suppliers--quotes)
5. [Features & Tolerances](#5-features--tolerances)
6. [Mates & Fit Analysis](#6-mates--fit-analysis)
7. [Tolerance Stackups](#7-tolerance-stackups)
8. [Risk Management](#8-risk-management)
9. [Manufacturing Processes](#9-manufacturing-processes)
10. [Process Controls](#10-process-controls)
11. [Test Protocols](#11-test-protocols)
12. [Test Results](#12-test-results)
13. [Assemblies](#13-assemblies)
14. [Handling Issues (NCRs & CAPAs)](#14-handling-issues-ncrs--capas)
15. [Project Status & Validation](#15-project-status--validation)
16. [Traceability](#16-traceability)

---

## 1. Project Setup

### Initialize Your Project

Create a new directory and initialize TDT:

```bash
mkdir led-flashlight
cd led-flashlight
tdt init
```

This creates:
- A `.tdt/` directory with configuration
- A `.gitignore` file (excludes local short ID mappings)
- A git repository
- Standard directory structure for all entity types

### Configure Your Project

Edit `.tdt/config.yaml` to set your default author:

```yaml
# .tdt/config.yaml
author: "Your Name"
editor: "code"  # or vim, nano, etc.
```

You can also set this globally:
```bash
git config --global user.name "Your Name"
```

---

## 2. Requirements

Requirements define what your product must do. TDT supports **input requirements** (from customers/stakeholders) and **output requirements** (derived/allocated).

### Create Input Requirements

```bash
# Brightness requirement
tdt req new --title "Light Output" --type input --priority critical \
  --category performance --status approved --no-edit

# Battery life requirement
tdt req new --title "Battery Life" --type input --priority high \
  --category performance --status approved --no-edit

# Water resistance requirement
tdt req new --title "Water Resistance" --type input --priority medium \
  --category environmental --status approved --no-edit

# Drop resistance requirement
tdt req new --title "Drop Resistance" --type input --priority high \
  --category durability --status approved --no-edit
```

### View Your Requirements

```bash
tdt req list
```

Output:
```
SHORT    ID               TYPE   TITLE             STATUS    PRIORITY
----------------------------------------------------------------------
REQ@1    REQ-01KCA...     input  Light Output      approved  critical
REQ@2    REQ-01KCA...     input  Battery Life      approved  high
REQ@3    REQ-01KCA...     input  Water Resistance  approved  medium
REQ@4    REQ-01KCA...     input  Drop Resistance   approved  high
```

### Edit Requirements with Full Details

Open a requirement in your editor to add detailed information:

```bash
tdt req edit REQ@1
```

Add acceptance criteria, rationale, and source information:

```yaml
id: REQ-01KCA...
type: input
title: Light Output
source:
  document: PRD-2024-001
  section: "3.1"
  revision: A
category: performance
text: |
  The flashlight shall produce a minimum of 500 lumens
  at maximum brightness setting.
rationale: |
  500 lumens provides adequate illumination for outdoor use
  at distances up to 50 meters.
acceptance_criteria:
  - Light output >= 500 lumens measured at LED surface
  - Beam angle between 15-20 degrees
priority: critical
status: approved
tags:
  - optical
  - performance
links: {}
```

### Create Derived Requirements

Derived requirements flow down from input requirements:

```bash
tdt req new --title "LED Selection" --type derived --priority high \
  --category electrical --status approved --no-edit
```

Then link it to the parent requirement:

```bash
tdt link add REQ@5 REQ@1 --link-type derives_from
```

---

## 3. Components & Bill of Materials

Components are the parts that make up your product. They can be **make** (manufactured) or **buy** (purchased).

### Create Components

```bash
# Housing (make part)
tdt cmp new --title "Flashlight Housing" --part-number "FL-HSG-001" \
  --make-buy make --category mechanical --no-edit

# LED module (buy part)
tdt cmp new --title "LED Module" --part-number "LED-500LM" \
  --make-buy buy --category electrical --no-edit

# Battery holder (buy part)
tdt cmp new --title "Battery Holder" --part-number "BH-2xAA" \
  --make-buy buy --category electrical --no-edit

# Lens (buy part)
tdt cmp new --title "Optical Lens" --part-number "LENS-20DEG" \
  --make-buy buy --category optical --no-edit

# O-ring seal (buy part)
tdt cmp new --title "Housing O-Ring" --part-number "OR-25x2" \
  --make-buy buy --category mechanical --no-edit

# End cap (make part)
tdt cmp new --title "End Cap" --part-number "FL-CAP-001" \
  --make-buy make --category mechanical --no-edit
```

### View Components

```bash
tdt cmp list
```

### Add Component Details

Edit a component to add specifications:

```bash
tdt cmp edit CMP@1
```

```yaml
id: CMP-01KCA...
part_number: FL-HSG-001
title: Flashlight Housing
description: |
  Main housing body, CNC machined from 6061-T6 aluminum.
  Provides structural support and heat dissipation for LED.
make_buy: make
category: mechanical
material: "6061-T6 Aluminum"
mass_kg: 0.085
unit_cost: 12.50
specifications:
  finish: "Type III Hard Anodize, Black"
  wall_thickness: "2.0mm minimum"
status: draft
```

---

## 4. Suppliers & Quotes

For buy parts, track suppliers and quotes.

### Create Suppliers

```bash
tdt sup new --name "BrightLED Inc" --short-name "BrightLED" \
  --contact-email "sales@brightled.example" --no-edit

tdt sup new --name "PowerCell Supply" --short-name "PowerCell" \
  --contact-email "orders@powercell.example" --no-edit
```

### Create Quotes

```bash
# Quote for LED module
tdt quote new --title "LED Module Quote" --supplier SUP@1 \
  --component CMP@2 --unit-price 3.50 --moq 100 --lead-time 14 --no-edit

# Quote for battery holder
tdt quote new --title "Battery Holder Quote" --supplier SUP@2 \
  --component CMP@3 --unit-price 0.85 --moq 500 --lead-time 7 --no-edit
```

### View Quotes

```bash
tdt quote list
```

---

## 5. Features & Tolerances

Features are the geometric characteristics of components that matter for fit and function.

### Create Features

```bash
# Housing bore (where LED mounts)
tdt feat new --title "LED Mounting Bore" --component CMP@1 \
  --feature-type hole --no-edit

# Housing thread (for end cap)
tdt feat new --title "End Cap Thread" --component CMP@1 \
  --feature-type thread --no-edit

# LED module OD
tdt feat new --title "LED Module OD" --component CMP@2 \
  --feature-type shaft --no-edit

# End cap thread
tdt feat new --title "Cap Thread" --component CMP@6 \
  --feature-type thread --no-edit

# O-ring groove
tdt feat new --title "O-Ring Groove" --component CMP@6 \
  --feature-type slot --no-edit
```

### Add Tolerances to Features

Edit a feature to add dimensional tolerances:

```bash
tdt feat edit FEAT@1
```

```yaml
id: FEAT-01KCA...
title: LED Mounting Bore
component: CMP-01KCA...  # Housing
feature_type: hole
description: |
  Bore in housing front face for LED module press-fit.
  Critical for thermal contact and alignment.
nominal: 20.00
plus_tolerance: 0.021
minus_tolerance: 0.000
unit: mm
notes: |
  H7 tolerance class for transition/light interference fit.
  Surface finish Ra 1.6 or better for thermal contact.
critical: true
status: approved
```

### View Features

```bash
# All features
tdt feat list

# Features for a specific component
tdt feat list -c CMP@1

# With descriptions
tdt feat list --columns title,description,component
```

---

## 6. Mates & Fit Analysis

Mates define how features from different components interact.

### Create Mates

```bash
# LED to Housing fit
tdt mate new --title "LED-Housing Press Fit" \
  --feature-a FEAT@1 --feature-b FEAT@3 \
  --mate-type interference_fit --no-edit

# End cap thread engagement
tdt mate new --title "End Cap Thread Engagement" \
  --feature-a FEAT@2 --feature-b FEAT@4 \
  --mate-type thread_engagement --no-edit
```

### Analyze Fit

```bash
tdt mate list
```

The fit analysis shows:
- **Worst-case clearance/interference** calculated from tolerances
- **Fit result**: clearance, interference, or transition
- **Match indicator**: ✓ if result matches intended type, ⚠ if mismatch

---

## 7. Tolerance Stackups

Stackups analyze how tolerances accumulate across multiple features.

### Create a Stackup

```bash
tdt tol new --title "LED to Lens Air Gap" --no-edit
```

Then edit to add contributors:

```bash
tdt tol edit TOL@1
```

```yaml
id: TOL-01KCA...
title: LED to Lens Air Gap
description: |
  Critical air gap between LED emitter surface and lens.
  Affects beam focus and light output efficiency.
target:
  nominal: 2.5
  lower_limit: 2.0
  upper_limit: 3.0
  unit: mm
contributors:
  - feature_id: FEAT-01KCA...  # LED Mounting Bore
    description: "Housing bore depth"
    direction: positive
    nominal: 15.0
    tolerance: 0.1
    distribution: normal
    sigma: 3.0
  - feature_id: FEAT-01KCA...  # LED Module
    description: "LED module height"
    direction: negative
    nominal: 12.5
    tolerance: 0.15
    distribution: normal
    sigma: 3.0
analysis_results: {}
status: draft
```

### Run Analysis

```bash
tdt tol analyze TOL@1
```

This calculates:
- **Worst-case** min/max values
- **RSS (Root Sum Square)** statistical analysis
- **Monte Carlo** simulation with yield prediction
- **Cpk** process capability index

### View Results

```bash
tdt tol list
```

```
SHORT    TITLE                  RESULT     CPK     YIELD    STATUS
------------------------------------------------------------------
TOL@1    LED to Lens Air Gap    acceptable 1.45    99.8%    draft
```

---

## 8. Risk Management

Identify and mitigate risks to your design and manufacturing processes.

### Create Design Risks

```bash
tdt risk new --title "LED Overheating" --type design \
  --category thermal --no-edit
```

Edit to add FMEA details:

```bash
tdt risk edit RISK@1
```

```yaml
id: RISK-01KCA...
type: design
title: LED Overheating
category: thermal
failure_mode: |
  LED junction temperature exceeds maximum rating during
  continuous operation at maximum brightness.
cause: |
  Insufficient thermal path from LED to housing.
  Inadequate heat sink mass.
effect: |
  Reduced LED life, color shift, potential thermal shutdown.
severity: 8
occurrence: 4
detection: 6
# RPN = 8 × 4 × 6 = 192
mitigations:
  - action: "Add thermal interface material between LED and housing"
    owner: "Thermal Engineer"
    status: planned
  - action: "Increase housing wall thickness at LED mount"
    owner: "Mechanical Engineer"
    status: completed
status: draft
```

### Create Process Risks

```bash
tdt risk new --title "Housing Machining Defects" --type process \
  --category manufacturing --no-edit
```

### View Risks

```bash
tdt risk list
```

Shows risk level (Critical/High/Medium/Low) and RPN scores.

---

## 9. Manufacturing Processes

Define how components are manufactured.

### Create Process Steps

```bash
# Housing machining
tdt proc new --title "Housing CNC Machining" --type machining \
  --operation-number "OP-010" --no-edit

# Housing anodizing
tdt proc new --title "Housing Anodizing" --type finishing \
  --operation-number "OP-020" --no-edit

# Final assembly
tdt proc new --title "Final Assembly" --type assembly \
  --operation-number "OP-030" --no-edit
```

### Link Processes to Components and Risks

```bash
tdt link add PROC@1 CMP@1 --link-type produces
tdt link add PROC@1 RISK@2 --link-type risks
```

---

## 10. Process Controls

Controls ensure processes produce conforming product.

### Create Controls

```bash
# Bore diameter inspection
tdt ctrl new --title "LED Bore Diameter Check" --type inspection \
  --process PROC@1 --feature FEAT@1 --no-edit
```

Edit to add control details:

```bash
tdt ctrl edit CTRL@1
```

```yaml
id: CTRL-01KCA...
title: LED Bore Diameter Check
control_type: inspection
process: PROC-01KCA...
feature: FEAT-01KCA...
description: |
  100% inspection of LED mounting bore diameter.
characteristic: "Bore diameter 20.000 - 20.021 mm"
measurement_method: "Bore gauge, calibrated"
frequency: "Every part"
acceptance_criteria: "20.000 - 20.021 mm"
reaction_plan: |
  Out of tolerance: Quarantine part, notify quality.
  Trend toward limit: Adjust tool offset.
critical: true
status: draft
```

### View Controls

```bash
tdt ctrl list
```

---

## 11. Test Protocols

Define how requirements will be verified.

### Create Test Protocols

```bash
# Light output test
tdt test new --title "Light Output Verification" \
  --type verification --level system --method test \
  --priority critical --verifies REQ@1 --no-edit

# Battery life test
tdt test new --title "Battery Life Test" \
  --type verification --level system --method test \
  --priority high --verifies REQ@2 --no-edit

# Water resistance test (IP rating)
tdt test new --title "Water Resistance Test" \
  --type verification --level system --method test \
  --priority medium --verifies REQ@3 --no-edit

# Drop test
tdt test new --title "Drop Test" \
  --type verification --level system --method test \
  --priority high --verifies REQ@4 --mitigates RISK@1 --no-edit
```

The `--verifies` flag links the test to requirements automatically!

### Add Test Procedure Details

```bash
tdt test edit TEST@1
```

```yaml
id: TEST-01KCA...
type: verification
test_level: system
test_method: test
title: Light Output Verification
objective: |
  Verify flashlight meets minimum 500 lumen output requirement.
preconditions:
  - Fresh batteries installed
  - Unit at room temperature (23°C ± 2°C)
  - Integrating sphere calibrated
equipment:
  - name: "Integrating Sphere"
    specification: "12-inch diameter, calibrated"
    calibration_required: true
  - name: "Spectrometer"
    specification: "350-750nm range"
    calibration_required: true
procedure:
  - step: 1
    action: "Install fresh AA batteries"
    expected: "Power indicator lights"
  - step: 2
    action: "Place flashlight in integrating sphere"
    expected: "Centered in sphere aperture"
  - step: 3
    action: "Turn on at maximum brightness"
    expected: "Stable light output"
  - step: 4
    action: "Record lumen reading after 30 second stabilization"
    expected: ">= 500 lumens"
    acceptance: "Pass if >= 500 lumens"
acceptance_criteria:
  - "Light output >= 500 lumens"
  - "No visible flicker"
priority: critical
status: approved
links:
  verifies:
    - REQ-01KCA...
```

### View Tests

```bash
tdt test list
```

---

## 12. Test Results

Record the outcomes of test execution.

### Create Test Results

```bash
# Passing result
tdt rslt new --test TEST@1 --verdict pass \
  --title "Light Output Test - Unit SN001" --no-edit

# Another passing result
tdt rslt new --test TEST@2 --verdict pass \
  --title "Battery Life Test - Unit SN001" --no-edit

# Failing result (for demonstration)
tdt rslt new --test TEST@3 --verdict fail \
  --title "Water Resistance Test - Unit SN001" --no-edit
```

### Add Result Details

```bash
tdt rslt edit RSLT@1
```

```yaml
id: RSLT-01KCA...
test_id: TEST-01KCA...
title: Light Output Test - Unit SN001
verdict: pass
executed_by: "Test Engineer"
executed_date: "2024-01-15T10:30:00Z"
measurements:
  - parameter: "Light Output"
    value: 523
    unit: "lumens"
    pass: true
  - parameter: "Color Temperature"
    value: 5800
    unit: "K"
    pass: true
notes: |
  Unit exceeded minimum requirement by 4.6%.
  Consistent with expected LED module performance.
status: approved
```

### View Results

```bash
tdt rslt list
```

```
SHORT    TEST     VERDICT      STATUS     AUTHOR
------------------------------------------------
RSLT@1   TEST@1   pass         approved   Test Engineer
RSLT@2   TEST@2   pass         draft      Test Engineer
RSLT@3   TEST@3   fail         draft      Test Engineer
```

---

## 13. Assemblies

Define how components come together.

### Create Assembly

```bash
tdt asm new --title "LED Flashlight Assembly" \
  --part-number "FL-ASM-001" --no-edit
```

### Add Bill of Materials

Edit the assembly to add BOM:

```bash
tdt asm edit ASM@1
```

```yaml
id: ASM-01KCA...
part_number: FL-ASM-001
title: LED Flashlight Assembly
description: |
  Complete LED flashlight assembly, ready for packaging.
bom:
  - component_id: CMP-01KCA...  # Housing
    quantity: 1
    reference: "1"
  - component_id: CMP-01KCA...  # LED Module
    quantity: 1
    reference: "2"
  - component_id: CMP-01KCA...  # Battery Holder
    quantity: 1
    reference: "3"
  - component_id: CMP-01KCA...  # Lens
    quantity: 1
    reference: "4"
  - component_id: CMP-01KCA...  # O-Ring
    quantity: 1
    reference: "5"
  - component_id: CMP-01KCA...  # End Cap
    quantity: 1
    reference: "6"
revision: "A"
status: draft
```

### View BOM

```bash
tdt asm bom ASM@1
```

Shows indented BOM with costs and masses rolled up.

### Calculate Assembly Cost/Mass

```bash
tdt asm cost ASM@1
tdt asm mass ASM@1
```

---

## 14. Handling Issues (NCRs & CAPAs)

When things go wrong, track nonconformances and corrective actions.

### Create an NCR

When the water resistance test failed:

```bash
tdt ncr new --title "Water Ingress at O-Ring Seal" \
  --type internal --severity major --no-edit
```

Edit to add details:

```bash
tdt ncr edit NCR@1
```

```yaml
id: NCR-01KCA...
title: Water Ingress at O-Ring Seal
type: internal
severity: major
description: |
  During water resistance testing (TEST@3), water ingress
  observed at end cap O-ring seal after 30 minutes
  submersion at 1 meter depth.
immediate_action: |
  Quarantined test unit SN001.
  Halted further water resistance testing.
root_cause: |
  O-ring groove depth undersized by 0.15mm, preventing
  proper O-ring compression. Caused by tool wear on
  CNC lathe.
disposition: rework
status: open
links:
  from_result:
    - RSLT-01KCA...  # The failing test result
  component:
    - CMP-01KCA...   # End Cap
```

### Link NCR to Test Result

```bash
tdt link add NCR@1 RSLT@3 --link-type from_result
```

### Create a CAPA

```bash
tdt capa new --title "O-Ring Groove Tool Wear Control" \
  --type corrective --no-edit
```

Edit to add action plan:

```bash
tdt capa edit CAPA@1
```

```yaml
id: CAPA-01KCA...
title: O-Ring Groove Tool Wear Control
type: corrective
description: |
  Implement controls to prevent O-ring groove dimension
  drift due to tool wear.
root_cause_analysis: |
  5-Why Analysis:
  1. Why water ingress? O-ring didn't seal.
  2. Why didn't O-ring seal? Groove too shallow.
  3. Why groove too shallow? Tool worn beyond limit.
  4. Why tool worn? No scheduled replacement.
  5. Why no schedule? Tool life not characterized.
actions:
  - description: "Characterize groove tool life"
    owner: "Manufacturing Engineer"
    due_date: "2024-02-01"
    status: completed
  - description: "Add tool change to PM schedule at 500 parts"
    owner: "Production Supervisor"
    due_date: "2024-02-15"
    status: in_progress
  - description: "Add groove depth to in-process inspection"
    owner: "Quality Engineer"
    due_date: "2024-02-15"
    status: planned
effectiveness_criteria: |
  Zero O-ring seal failures in next 100 units.
capa_status: in_progress
links:
  ncrs:
    - NCR-01KCA...
```

### Link CAPA to NCR

```bash
tdt link add CAPA@1 NCR@1 --link-type ncrs
```

---

## 15. Project Status & Validation

### Check Project Status

```bash
tdt status
```

Shows a dashboard with:
- Requirements coverage (verified vs unverified)
- Risk summary (by level, average RPN)
- Test status (pass rate, pending tests)
- Quality metrics (open NCRs, CAPAs)
- BOM summary
- Tolerance analysis results

### Validate Project

```bash
tdt validate
```

Checks:
- YAML syntax validity
- Schema compliance
- Required field presence
- Link integrity (referenced entities exist)

### Check Verification Coverage

```bash
tdt trace coverage
```

```
Verification Coverage Report
════════════════════════════════════════════════════════════

Total requirements:     5
With verification:      4
Without verification:   1

Coverage: 80%

Uncovered Requirements:
────────────────────────────────────────────────────────────
  ○ REQ@5 - LED Selection
```

---

## 16. Traceability

TDT maintains full traceability across all entities.

### View Traceability Matrix

```bash
# Full matrix
tdt trace matrix

# With short ID aliases for readability
tdt trace matrix --aliases

# Filter by entity type
tdt trace matrix --source-type TEST --target-type REQ
```

### Trace From an Entity

See what depends on a specific entity:

```bash
tdt trace from REQ@1
```

Shows all tests, components, and other entities linked to this requirement.

### Trace To an Entity

See what an entity depends on:

```bash
tdt trace to TEST@1
```

### Find Orphaned Entities

Entities with no links (potential gaps):

```bash
tdt trace orphans
```

---

## Summary

You've now created a complete TDT project with:

| Entity Type | Purpose | Commands |
|-------------|---------|----------|
| **Requirements** | What the product must do | `tdt req` |
| **Components** | Parts (make/buy) | `tdt cmp` |
| **Suppliers** | Vendor information | `tdt sup` |
| **Quotes** | Pricing and lead times | `tdt quote` |
| **Features** | Geometric characteristics | `tdt feat` |
| **Mates** | Feature interactions | `tdt mate` |
| **Stackups** | Tolerance accumulation | `tdt tol` |
| **Risks** | FMEA (design/process) | `tdt risk` |
| **Processes** | Manufacturing steps | `tdt proc` |
| **Controls** | Process controls | `tdt ctrl` |
| **Tests** | Verification protocols | `tdt test` |
| **Results** | Test outcomes | `tdt rslt` |
| **Assemblies** | BOM structure | `tdt asm` |
| **NCRs** | Nonconformances | `tdt ncr` |
| **CAPAs** | Corrective actions | `tdt capa` |

### Key Workflows

1. **Requirements → Tests → Results**: Verify requirements are met
2. **Components → Features → Mates → Stackups**: Ensure parts fit
3. **Risks → Mitigations → Controls**: Manage product/process risks
4. **NCRs → CAPAs**: Handle quality issues systematically

### Tips

- Use `--no-edit` for scripted creation, omit it to open editor immediately
- Use short IDs (`REQ@1`, `CMP@2`) for quick reference
- Use `tdt link add` to create traceability between entities
- Run `tdt validate` regularly to catch errors early
- Use `tdt status` for a project health overview

---

## Next Steps

- Explore individual entity documentation in the docs folder
- Set up your own project structure
- Integrate with your existing PLM/QMS systems via JSON/YAML export
- Use `tdt report` commands for generating documentation

For more information, visit [tessera.dev](https://tessera.dev)
