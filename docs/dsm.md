# TDT Design Structure Matrix (DSM)

This document describes the Design Structure Matrix command in TDT (Tessera Design Toolkit).

## Overview

The Design Structure Matrix (DSM) is a systems engineering tool that visualizes relationships between components in your product. It helps identify:

- **Physical interfaces** (mates between components)
- **Tolerance chains** (components linked in tolerance stackups)
- **Process dependencies** (components sharing manufacturing processes)
- **Requirement allocation** (components linked to the same requirements)

DSMs are essential for understanding system architecture, identifying integration risks, and optimizing product modularization.

## Command

```bash
tdt dsm [OPTIONS] [ASSEMBLY]
```

### Arguments

| Argument | Description |
|----------|-------------|
| `ASSEMBLY` | Optional assembly ID to scope the DSM (e.g., `ASM@1`). If omitted, shows all project components. |

### Options

| Option | Short | Description |
|--------|-------|-------------|
| `--rel-type` | `-t` | Relationship types to include: `all`, `mate`, `tolerance`, `process`, `requirement` (default: `all`) |
| `--output` | `-o` | Output format: `table`, `csv`, `dot`, `json` (default: `table`) |
| `--cluster` | `-c` | Apply clustering optimization to group related components |
| `--weighted` | `-w` | Show numeric dependency strength instead of relationship type symbols |
| `--metrics` | `-m` | Show coupling metrics (fan-in, fan-out, coupling coefficient) |
| `--cycles` | | Highlight and report dependency cycles |
| `--full-ids` | | Show full entity IDs instead of short IDs |

## Usage Examples

### Basic DSM

Show all component relationships:

```bash
tdt dsm
```

Output:
```
Design Structure Matrix

        CMP@1  CMP@2  CMP@3
------ ---------------------
CMP@1     ■      M      ·
CMP@2     M      ■      P
CMP@3     ·      P      ■

Legend: M = Mate T = Tolerance P = Process R = Requirement

Summary: 3 components, 1 mate, 0 tolerance, 1 process, 0 requirement
```

### Filter by Relationship Type

Show only mate relationships:

```bash
tdt dsm -t mate
```

Show only tolerance stackup relationships:

```bash
tdt dsm -t tolerance
```

Show only process relationships:

```bash
tdt dsm -t process
```

### Scope to Assembly

Analyze only components within a specific assembly:

```bash
tdt dsm ASM@1
```

### Clustering Optimization

Apply clustering to group related components together:

```bash
tdt dsm -c
```

Output:
```
Design Structure Matrix (Clustered)

        CMP@1  CMP@3  CMP@2
------ ---------------------
CMP@1     ■      M      ·
CMP@3     M      ■      P
CMP@2     ·      P      ■

Legend: M = Mate T = Tolerance P = Process R = Requirement

Summary: 3 components, 1 mate, 0 tolerance, 1 process, 0 requirement

Identified Clusters:
  Cluster 1: CMP@1, CMP@3
  Cluster 2: CMP@2
```

### Export to Spreadsheet (CSV)

Export for analysis in Excel or Google Sheets:

```bash
tdt dsm -o csv > dsm.csv
```

Output:
```csv
Component,CMP@1,CMP@2,CMP@3
CMP@1,X,M,
CMP@2,M,X,P
CMP@3,,P,X
```

### Export for Visualization (Graphviz DOT)

Generate a graph for visualization:

```bash
tdt dsm -o dot > dsm.dot
dot -Tpng dsm.dot -o dsm.png
```

### JSON Output

Export structured data for programmatic use:

```bash
tdt dsm -o json
```

Output:
```json
{
  "components": [
    {"id": "CMP-01ABC...", "short_id": "CMP@1", "title": "Housing", "part_number": "PN-001"}
  ],
  "relationships": [
    {"source": "CMP-01ABC...", "target": "CMP-01DEF...", "types": ["Mate"], "weight": 1}
  ],
  "matrix_size": 3
}
```

### Weighted View

Show numeric dependency strength (count of relationship types) instead of symbols:

```bash
tdt dsm --weighted
```

Output:
```
Design Structure Matrix

        CMP@3  CMP@1  CMP@2  CMP@4
------ ----------------------------
CMP@3     ■      2      1      3
CMP@1     2      ■      2      3
CMP@2     1      2      ■      1
CMP@4     3      3      1      ■

Legend: Numbers show dependency strength (count of relationship types)
```

Values indicate how many relationship types connect each pair:
- `1` = single relationship type (e.g., just Mate)
- `2` = two relationship types (e.g., Mate + Tolerance)
- `3` = three relationship types (e.g., Mate + Tolerance + Process)
- `4` = all four types (Mate + Tolerance + Process + Requirement)

### Coupling Metrics

Analyze coupling statistics for each component:

```bash
tdt dsm --metrics
```

Output:
```
Design Structure Matrix
...

Coupling Metrics

  Component      Fan-in  Fan-out    Total   Coupling %
  ------------ -------- -------- -------- ------------
  CMP@3               6        6        6        66.7% ★
  CMP@1               7        7        7        77.8% ★
  CMP@2               4        4        4        44.4% ★
  CMP@4               7        7        7        77.8% ★

  Total connections: 12 | Avg coupling: 66.7%
  Hubs: CMP@3, CMP@1, CMP@4 (high connectivity)
```

Metrics explained:
- **Fan-in/Fan-out**: Number of incoming/outgoing relationships (equal for symmetric DSM)
- **Total**: Sum of relationship counts across all connections
- **Coupling %**: Percentage of maximum possible connections
- **★ Hub**: Components with high connectivity that propagate changes

### Cycle Detection

Identify components with bidirectional dependencies:

```bash
tdt dsm --cycles
```

Output:
```
Design Structure Matrix
...

Dependency Cycles: 1 cycle group(s) detected
  Cycle 1: CMP@3 <-> CMP@1 <-> CMP@2 <-> CMP@4

  Components in cycles have bidirectional dependencies
```

Cycles indicate tightly coupled subsystems where changes propagate both ways.

## Understanding DSM Output

### Matrix Symbols

| Symbol | Meaning |
|--------|---------|
| `■` | Diagonal (component with itself) |
| `M` | Mate relationship (physical interface) |
| `T` | Tolerance relationship (tolerance stackup) |
| `P` | Process relationship (shared manufacturing) |
| `R` | Requirement relationship (common allocation) |
| `·` | No relationship |
| `M,T` | Multiple relationship types |

### Relationship Types

**Mate (M)**
- Detected from mate entities that connect features on different components
- Indicates physical interfaces requiring tolerance analysis
- Example: A bearing housing mating with a shaft

**Tolerance (T)**
- Components linked through tolerance stackups
- Indicates dimensional chains requiring analysis
- Example: Multiple parts contributing to an assembly gap

**Process (P)**
- Components linked to the same manufacturing process
- Indicates shared tooling or manufacturing dependencies
- Example: Multiple parts machined on the same CNC setup

**Requirement (R)**
- Components allocated to the same requirement
- Indicates functional coupling at the specification level
- Example: Multiple parts contributing to a system-level requirement

## Clustering Algorithm

The `--cluster` option reorders components to minimize off-diagonal distance, grouping tightly coupled components together. This helps identify:

- **Product modules**: Groups of components that interact heavily
- **Integration boundaries**: Natural divisions for parallel development
- **Risk areas**: Clusters with many dependencies

The algorithm uses a greedy approach:
1. Start with the most connected component
2. Add the component most connected to the current cluster
3. Repeat until all components are ordered

## Best Practices

### When to Use DSM

- **Architecture reviews**: Understand system structure early in design
- **Change impact analysis**: Identify components affected by a change
- **Modularization**: Find natural module boundaries
- **Integration planning**: Sequence integration based on dependencies

### Recommended Workflow

1. **Initial analysis**: Run `tdt dsm` to see overall structure
2. **Focus on mates**: Use `tdt dsm -t mate` for physical interface analysis
3. **Cluster analysis**: Use `tdt dsm -c` to identify modules
4. **Export for stakeholders**: Use `tdt dsm -o csv` for spreadsheet analysis

### DSM for Large Systems

For systems with many components:

```bash
# Export to CSV for spreadsheet filtering
tdt dsm -o csv > full_dsm.csv

# Analyze specific assembly
tdt dsm ASM@1 -c

# Focus on specific relationship type
tdt dsm -t mate -c
```

---

# Domain Mapping Matrix (DMM)

The Domain Mapping Matrix extends DSM analysis to show relationships between **different entity types**.

## DMM Command

```bash
tdt dmm <ROW_TYPE> <COL_TYPE> [OPTIONS]
```

### Arguments

| Argument | Description |
|----------|-------------|
| `ROW_TYPE` | Entity type for rows: `cmp`, `req`, `proc`, `test`, `risk`, `ctrl` |
| `COL_TYPE` | Entity type for columns (must be different from row type) |

### Options

| Option | Short | Description |
|--------|-------|-------------|
| `--output` | `-o` | Output format: `table`, `csv`, `json` (default: `table`) |
| `--stats` | `-s` | Show coverage statistics |
| `--full-ids` | | Show full entity IDs instead of short IDs |

## DMM Examples

### Components × Requirements

Show which components satisfy which requirements:

```bash
tdt dmm cmp req --stats
```

Output:
```
Domain Mapping Matrix: Components × Requirements

        REQ@7  REQ@5  REQ@6  REQ@3  REQ@2  REQ@1  REQ@4
------ -------------------------------------------------
CMP@3     ·      ·      ·      X      ·      ·      ·
CMP@1     ·      ·      ·      X      ·      X      ·
CMP@2     ·      ·      ·      ·      ·      X      ·
CMP@4     ·      ·      ·      X      ·      ·      ·

Coverage Statistics
  Components: 4/4 (100.0% coverage)
  Requirements: 2/7 (28.6% coverage)
  Total links: 5
```

### Components × Processes

Show which processes produce which components:

```bash
tdt dmm cmp proc
```

Output:
```
Domain Mapping Matrix: Components × Processes

        PROC@2  PROC@1  PROC@3
------ ------------------------
CMP@3     X       X       X
CMP@1     X       ·       ·
CMP@2     X       ·       ·
CMP@4     X       X       ·

Summary: 4 components × 3 processes (7 links)
```

### Requirements × Tests

Show verification coverage:

```bash
tdt dmm req test --stats
```

This reveals:
- Requirements without test coverage (gaps)
- Tests without linked requirements (orphaned tests)
- Coverage percentage for V&V planning

## DSM vs DMM

| Feature | DSM | DMM |
|---------|-----|-----|
| Entities | Same type (component × component) | Different types (component × requirement) |
| Matrix | Square, symmetric | Rectangular, asymmetric |
| Purpose | Analyze internal coupling | Analyze cross-domain allocation |
| Clustering | Supported | Not applicable |

Use **DSM** for understanding component architecture.
Use **DMM** for traceability and coverage analysis.

## See Also

- [Component Management](component.md) - Managing component entities
- [Mate Management](mate.md) - Physical interfaces between components
- [Feature Management](feature.md) - Dimensional features for tolerance analysis
- [Assembly Management](assembly.md) - Bill of materials and assemblies
