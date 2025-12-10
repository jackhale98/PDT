# PDT Requirements Entity

This document describes the Requirements entity type in PDT (Plain-text Product Development Toolkit).

## Overview

Requirements are the foundation of product development in PDT. They capture design inputs (customer needs, regulations, standards) and design outputs (specifications, derived requirements).

## Entity Type

- **Prefix**: `REQ`
- **File extension**: `.pdt.yaml`
- **Directories**:
  - `requirements/inputs/` - Design inputs (customer requirements, regulations)
  - `requirements/outputs/` - Design outputs (specifications, derived requirements)

## Schema

### Required Fields

| Field | Type | Description |
|-------|------|-------------|
| `id` | string | Unique identifier (REQ-[26-char ULID]) |
| `type` | enum | `input` or `output` |
| `title` | string | Short descriptive title (1-200 chars) |
| `text` | string | Full requirement text |
| `status` | enum | `draft`, `review`, `approved`, `obsolete` |
| `priority` | enum | `low`, `medium`, `high`, `critical` |
| `created` | datetime | Creation timestamp (ISO 8601) |
| `author` | string | Author name |

### Optional Fields

| Field | Type | Description |
|-------|------|-------------|
| `source` | object | Reference to source document |
| `source.document` | string | Source document name |
| `source.revision` | string | Document revision |
| `source.section` | string | Section reference |
| `source.date` | date | Date of source document |
| `category` | string | User-defined category |
| `tags` | array[string] | Tags for filtering and organization |
| `rationale` | string | Why this requirement exists |
| `acceptance_criteria` | array[string] | Criteria for verification |
| `revision` | integer | Revision number (default: 1) |

### Links

| Field | Type | Description |
|-------|------|-------------|
| `links.satisfied_by` | array[EntityId] | Design outputs that satisfy this input |
| `links.verified_by` | array[EntityId] | Tests that verify this requirement |

## Example

```yaml
# Requirement: Operating Temperature Range
# Created by PDT - Plain-text Product Development Toolkit

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
  Temperature range based on IEC 60068-2-1 and IEC 60068-2-2.

acceptance_criteria:
  - "Unit powers on at -20C after 4h cold soak"
  - "Unit powers on at +50C after 4h hot soak"
  - "All functions operational across temperature range"

priority: high
status: approved

links:
  satisfied_by:
    - REQ-01HC2JB7SMQX7RS1Y0GFKBHPTE  # Thermal design output
  verified_by:
    - TEST-01HC2JB7SMQX7RS1Y0GFKBHPTF  # Temperature cycling test

# Auto-managed metadata (do not edit manually)
created: 2024-01-15T10:30:00Z
author: Jack Hale
revision: 3
```

## CLI Commands

### Create a new requirement

```bash
# Create with default template
pdt req new

# Create with title and type
pdt req new --title "Operating Temperature Range" --type input

# Create with interactive wizard
pdt req new -i

# Create output requirement with high priority
pdt req new --type output --priority high --title "Thermal Management Spec"
```

### List requirements

```bash
# List all requirements
pdt req list

# Filter by type
pdt req list --type input
pdt req list --type output

# Filter by status
pdt req list --status draft
pdt req list --status approved

# Filter by priority
pdt req list --priority high
pdt req list --priority urgent  # high + critical

# Show orphaned requirements (no links)
pdt req list --orphans

# Show requirements needing review
pdt req list --needs-review

# Show recently created
pdt req list --recent 7  # last 7 days

# Search in title/text
pdt req list --search "temperature"

# Custom columns
pdt req list --columns id,title,status
```

### Show requirement details

```bash
# Show by ID (partial match supported)
pdt req show REQ-01HC2

# Show by title search
pdt req show "temperature"

# Show with linked entities
pdt req show REQ-01HC2 --with-links
```

### Edit a requirement

```bash
# Open in editor
pdt req edit REQ-01HC2
```

## Validation

Requirements are validated against a JSON Schema. Run validation with:

```bash
# Validate all project files
pdt validate

# Validate specific file
pdt validate requirements/inputs/REQ-01HC2JB7SMQX7RS1Y0GFKBHPTD.pdt.yaml

# Validate only requirements
pdt validate --entity-type req

# Continue after first error
pdt validate --keep-going
```

### Validation Rules

1. **ID Format**: Must match `REQ-[A-Z0-9]{26}` pattern
2. **Type**: Must be `input` or `output`
3. **Title**: Required, 1-200 characters
4. **Text**: Required, non-empty
5. **Status**: Must be one of: `draft`, `review`, `approved`, `obsolete`
6. **Priority**: Must be one of: `low`, `medium`, `high`, `critical`
7. **No Additional Properties**: Unknown fields are not allowed

## Link Management

```bash
# Add a link
pdt link add REQ-01HC2 --type satisfied_by REQ-01HC3

# Remove a link
pdt link remove REQ-01HC2 --type satisfied_by REQ-01HC3

# Show all links for a requirement
pdt link show REQ-01HC2

# Check for broken links
pdt link check
```

## Traceability

```bash
# Show traceability matrix
pdt trace matrix

# Export as GraphViz DOT
pdt trace matrix --output dot > trace.dot

# Export as CSV
pdt trace matrix --output csv > trace.csv

# Trace what depends on a requirement
pdt trace from REQ-01HC2

# Trace what a requirement depends on
pdt trace to REQ-01HC2

# Find orphaned requirements
pdt trace orphans

# Verification coverage report
pdt trace coverage

# Show uncovered requirements
pdt trace coverage --uncovered
```

## Best Practices

### Writing Good Requirements

1. **Use "shall"** for mandatory requirements
2. **Use "should"** for recommended requirements
3. **Use "may"** for optional requirements
4. **Be specific and testable** - avoid vague language
5. **One requirement per entity** - don't combine multiple requirements

### Organizing Requirements

1. **Use categories** to group related requirements
2. **Use tags** for cross-cutting concerns
3. **Separate inputs from outputs** in different directories
4. **Link related requirements** with satisfied_by relationships

### Status Workflow

```
draft → review → approved
                    ↓
                 obsolete
```

1. **draft**: Initial creation, still being written
2. **review**: Ready for stakeholder review
3. **approved**: Signed off and baselined
4. **obsolete**: No longer applicable (keep for history)

### Priority Guidelines

- **critical**: Safety, regulatory, or blocking requirements
- **high**: Core functionality, key differentiators
- **medium**: Standard features, quality of life
- **low**: Nice to have, future considerations

## JSON Schema

The full JSON Schema for requirements is embedded in PDT and available at:

```
pdt/schemas/req.schema.json
```

Key schema properties:

- `additionalProperties: false` - No unknown fields allowed
- `minLength` constraints on title and text
- `maxLength: 200` on title
- `format: date-time` on created
- `pattern` validation on ID field
