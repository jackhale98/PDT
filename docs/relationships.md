# TDT Entity Relationships

This document explains how entities are connected in TDT (Tessera Design Toolkit).

## Overview

TDT uses two types of relationships between entities:

| Type | Purpose | Managed By |
|------|---------|------------|
| **Structural** | Parent-child or composition relationships with attributes (quantity, direction, etc.) | Entity-specific commands or YAML editing |
| **Links** | Simple cross-references for traceability | `tdt link` command |

Understanding the difference helps you model your product data correctly.

## Structural Relationships

Structural relationships are embedded directly in entity data because they carry additional attributes beyond just "A relates to B."

### Assembly → Components (BOM)

Assemblies contain a Bill of Materials with quantities and reference designators:

```yaml
# assembly.yaml
id: ASM-01J...
title: "Main PCB Assembly"
bom:
  - component_id: CMP-01J...    # Resistor
    quantity: 4
    reference_designators: ["R1", "R2", "R3", "R4"]
    notes: "0603 footprint"
  - component_id: CMP-01K...    # Capacitor
    quantity: 2
    reference_designators: ["C1", "C2"]
```

The `quantity` and `reference_designators` attributes are why this is structural, not a link.

### Assembly → Subassemblies

Assemblies can contain other assemblies:

```yaml
# top-level-assembly.yaml
subassemblies:
  - ASM-01J...  # Main PCB
  - ASM-01K...  # Enclosure Assembly
```

### Feature → Component (Parent)

Features belong to a component (required field, not optional):

```yaml
# feature.yaml
id: FEAT-01J...
component: CMP-01J...   # Parent component (required)
title: "Mounting Hole A"
dimensions:
  - name: diameter
    nominal: 10.0
    plus_tol: 0.1
    minus_tol: 0.05
```

### Mate → Features

Mates connect exactly two features with fit analysis:

```yaml
# mate.yaml
id: MATE-01J...
feature_a: FEAT-01J...  # Hole feature
feature_b: FEAT-01K...  # Shaft feature
mate_type: clearance
```

### Stackup → Contributors

Tolerance stackups reference features with direction sense:

```yaml
# stackup.yaml
id: TOL-01J...
contributors:
  - feature: FEAT-01J...
    dimension: diameter
    direction: positive
    distribution: normal
  - feature: FEAT-01K...
    dimension: length
    direction: negative
```

### Quote → Component/Assembly

Quotes reference what they're pricing:

```yaml
# quote.yaml
id: QUOT-01J...
component: CMP-01J...    # OR
assembly: ASM-01J...     # (one or the other)
supplier: SUP-01J...
```

## Link Relationships

Links are simple cross-references stored in the `links` section of entities. They're used for traceability and don't carry additional attributes.

### All Link Types

#### Requirements (REQ)

| Link Type | Target | Reciprocal | Description |
|-----------|--------|------------|-------------|
| `satisfied_by` | REQ | `satisfied_by` | REQ that satisfies this one (symmetric) |
| `verified_by` | TEST, CTRL | `verifies` | Test/control that verifies this requirement |
| `derives_from` | REQ | `derived_by` | Parent requirement this derives from |
| `derived_by` | REQ | `derives_from` | Child requirements derived from this |
| `allocated_to` | FEAT | `allocated_from` | Feature this requirement is allocated to |

#### Tests (TEST) / Controls (CTRL)

| Link Type | Target | Reciprocal | Description |
|-----------|--------|------------|-------------|
| `verifies` | REQ | `verified_by` | Requirements this test verifies |
| `validates` | REQ | - | User needs this test validates |
| `mitigates` | RISK | - | Risks whose mitigation this verifies |
| `depends_on` | TEST | - | Tests that must pass before this one |

#### Risks (RISK)

| Link Type | Target | Reciprocal | Description |
|-----------|--------|------------|-------------|
| `affects` | FEAT, CMP, ASM, PROC | `risks` | Entities affected by this risk |
| `mitigated_by` | REQ | - | Design outputs that mitigate this risk |
| `verified_by` | TEST | - | Tests that verify mitigation |
| `related_to` | Any | `related_to` | Generic related entity (symmetric) |

#### Results (RSLT)

| Link Type | Target | Reciprocal | Description |
|-----------|--------|------------|-------------|
| `created_ncr` | NCR | `from_result` | NCR created from this failed result |

#### NCRs (NCR)

| Link Type | Target | Reciprocal | Description |
|-----------|--------|------------|-------------|
| `from_result` | RSLT | `created_ncr` | Test result that created this NCR |
| `component` | CMP | - | Affected component |
| `process` | PROC | - | Process where defect was found |
| `control` | CTRL | - | Control that detected the issue |
| `capa` | CAPA | `ncrs` | Linked CAPA if opened |

#### CAPAs (CAPA)

| Link Type | Target | Reciprocal | Description |
|-----------|--------|------------|-------------|
| `ncrs` | NCR | `capa` | Source NCRs for this CAPA |
| `processes_modified` | PROC | `modified_by_capa` | Processes modified by this CAPA |
| `controls_added` | CTRL | `added_by_capa` | Controls added by this CAPA |
| `risks` | RISK | - | Related risks |

#### Components (CMP)

| Link Type | Target | Reciprocal | Description |
|-----------|--------|------------|-------------|
| `replaces` | CMP | `replaced_by` | Component this supersedes |
| `replaced_by` | CMP | `replaces` | Component that supersedes this |
| `interchangeable_with` | CMP | `interchangeable_with` | Alternate components (symmetric) |
| `used_in` | ASM | - | Assemblies using this component |
| `risks` | RISK | `affects` | Risks affecting this component |
| `related_to` | Any | `related_to` | Generic related entity |

#### Processes (PROC)

| Link Type | Target | Reciprocal | Description |
|-----------|--------|------------|-------------|
| `produces` | CMP | - | Components produced by this process |
| `controls` | CTRL | `process` | Control plan items |
| `work_instructions` | WORK | - | Work instructions for this process |
| `risks` | RISK | `affects` | Risks affecting this process |
| `modified_by_capa` | CAPA | `processes_modified` | CAPAs that modified this process |
| `related_to` | Any | `related_to` | Generic related entity |

#### Features (FEAT)

| Link Type | Target | Reciprocal | Description |
|-----------|--------|------------|-------------|
| `allocated_from` | REQ | `allocated_to` | Requirements allocated to this feature |
| `risks` | RISK | `affects` | Risks affecting this feature |

#### Controls (CTRL)

| Link Type | Target | Reciprocal | Description |
|-----------|--------|------------|-------------|
| `process` | PROC | `controls` | Parent process |
| `feature` | FEAT | - | Feature being controlled |
| `verifies` | REQ | `verified_by` | Requirements this control verifies |
| `added_by_capa` | CAPA | `controls_added` | CAPA that added this control |

## Managing Links

### Add a link

```bash
# Basic syntax
tdt link add SOURCE TARGET LINK_TYPE

# Examples
tdt link add REQ@1 TEST@1 verified_by
tdt link add RISK@1 CMP@1 affects
tdt link add CAPA@1 PROC@1 processes_modified

# Add reciprocal automatically with -r
tdt link add REQ@1 TEST@1 verified_by -r
# This adds: REQ@1.verified_by → TEST@1
#       and: TEST@1.verifies → REQ@1
```

### Remove a link

```bash
tdt link remove REQ@1 TEST@1 verified_by
```

Note: To remove reciprocal links, run the command twice with swapped source/target.

### Show links for an entity

```bash
tdt link show REQ@1

# Show only outgoing links
tdt link show REQ@1 --outgoing

# Show only incoming links
tdt link show REQ@1 --incoming
```

### Check for broken links

```bash
tdt link check
```

### View all link types

```bash
tdt link add --help
```

## Suspect Links

When an entity is modified (revision increment, status regression, or content changes), its incoming links may need to be reviewed to ensure they're still valid. TDT tracks these as "suspect" links.

### Why Links Become Suspect

| Reason | Description |
|--------|-------------|
| `revision_changed` | Target entity's revision was incremented |
| `status_regressed` | Target entity's status regressed (e.g., approved → draft) |
| `content_modified` | Target entity's content was modified |
| `manually_marked` | User manually marked the link as suspect |

### Suspect Link Format

Suspect links use an extended format in YAML:

```yaml
links:
  verified_by:
    - TEST-01ABC...                  # Simple link (not suspect)
    - id: TEST-02DEF...              # Extended link (suspect)
      suspect: true
      suspect_reason: revision_changed
      suspect_since: 2024-01-15T10:30:00Z
```

### Managing Suspect Links

```bash
# List all suspect links in the project
tdt link suspect list

# Filter by entity type
tdt link suspect list -t RISK

# Review suspect links for a specific entity
tdt link suspect review REQ@1

# Clear suspect status after review
tdt link suspect clear REQ@1 TEST@1 -t verified_by

# Clear all suspect links for an entity
tdt link suspect clear REQ@1

# Manually mark a link as suspect
tdt link suspect mark REQ@1 TEST@1 -r revision_changed
```

### Suspect Link Validation

The `tdt validate` command reports suspect links as warnings:

```bash
$ tdt validate
→ Validating 15 file(s)...

! REQ-01ABC... - 2 suspect link(s)
    verified_by → TEST-02DEF... (revision changed)
    allocated_to → FEAT-03GHI... (status regressed)

────────────────────────────────────────────────────────────
Validation Summary
────────────────────────────────────────────────────────────
  Files checked:  15
  Files passed:   15
  Files failed:   0
  Total errors:   0
  Total warnings: 2
  Suspect links:  2 in 1 file(s)
                  Run 'tdt link suspect list' to review
```

### Best Practices for Suspect Links

1. **Review regularly** - Check for suspect links after significant changes
2. **Clear after verification** - Clear suspect status only after verifying the link is still valid
3. **Use in CI/CD** - Add `tdt link suspect list --count` to your CI pipeline
4. **Document reviews** - Use the `--verified-revision` flag when clearing to record the revision you verified against

## When to Use Each Type

### Use Structural Relationships When:

- The relationship has attributes (quantity, direction, notes)
- It's a parent-child or composition relationship
- The child entity "belongs to" the parent
- The relationship is required (not optional)

**Examples:**
- Components in an assembly (has quantity)
- Features on a component (feature belongs to component)
- Contributors in a stackup (has direction, distribution)

### Use Links When:

- It's a simple cross-reference for traceability
- The relationship is optional
- You need bidirectional navigation
- The relationship type may change

**Examples:**
- Requirement → Test (verification traceability)
- Risk → Component (what's affected)
- NCR → CAPA (escalation)

## Traceability

Links enable traceability analysis:

```bash
# Coverage report (requirements with/without tests)
tdt trace coverage

# Show only uncovered requirements
tdt trace coverage --uncovered

# Generate traceability matrix
tdt trace matrix

# Export as CSV
tdt trace matrix -o csv > trace.csv

# Export as GraphViz DOT
tdt trace matrix -o dot > trace.dot

# Find orphaned requirements (no links)
tdt trace orphans
```

## Best Practices

1. **Use reciprocal links** (`-r` flag) to maintain bidirectional traceability
2. **Run `tdt link check`** periodically to find broken links
3. **Don't duplicate** structural relationships as links
4. **Be consistent** with link types across your project
5. **Document custom relationships** in your project README

## Example: Complete Traceability Chain

```
Customer Need (REQ input)
    │
    │ satisfied_by
    ▼
Design Specification (REQ output)
    │
    ├─── allocated_to ──► Feature (FEAT)
    │                         │
    │                         │ (structural: component field)
    │                         ▼
    │                     Component (CMP) ◄── affects ── Risk (RISK)
    │                         │
    │                         │ (structural: BOM)
    │                         ▼
    │                     Assembly (ASM)
    │
    │ verified_by
    ▼
Test Protocol (TEST) ──► Test Result (RSLT)
                              │
                              │ created_ncr (if failed)
                              ▼
                          NCR ──► CAPA
```
