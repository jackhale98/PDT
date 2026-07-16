# Tessera Global Search

This document describes the global search command in Tessera.

## Overview

The `tdt search` command allows you to search across all entity types in your project. It searches entity titles and descriptions, with optional filtering by type, status, author, and tags.

## CLI Commands

### Basic Search

```bash
# Search all entities for a term
tdt search "temperature"

# Search is case-insensitive by default
tdt search "TEMPERATURE"

# Case-sensitive search
tdt search "Temperature" --case-sensitive
```

### Filter by Entity Type

```bash
# Search only requirements
tdt search "motor" --entity-type req

# Search only risks
tdt search "failure" --entity-type risk

# Search multiple types (comma-separated)
tdt search "thermal" --entity-type req,risk,test

# Available type prefixes:
#   req, risk, test, rslt, cmp, asm, feat, mate,
#   tol, proc, ctrl, work, ncr, capa, quote, sup
```

### Filter by Status

```bash
# Search only draft entities
tdt search "prototype" --status draft

# Search only approved entities
tdt search "released" --status approved
```

### Filter by Author

```bash
# Search entities by a specific author
tdt search "design" --author "Jane Doe"

# Partial author name match
tdt search "spec" --author "Jane"
```

### Filter by Tag

```bash
# Search entities with a specific tag
tdt search "v2" --tag "release"

# Combined with type filter
tdt search "feature" --entity-type req --tag "phase1"
```

### Combine Filters

```bash
# Complex search with multiple filters
tdt search "thermal" --entity-type req,risk --status approved --author "Jane"

# Search for high-priority items by a specific author
tdt search "critical" --entity-type risk --author "Bob"
```

### Output Options

```bash
# Show count only
tdt search "motor" --count

# Limit results
tdt search "test" --limit 10

# Output as JSON
tdt search "thermal" -f json

# Output as YAML
tdt search "thermal" -f yaml

# Output as CSV
tdt search "thermal" -f csv
```

## Example Output

### Default Output (TSV)

```
$ tdt search "thermal"
TYPE   SHORT    ID               TITLE                              STATUS
---------------------------------------------------------------------------------
REQ    REQ@3    REQ-01HC2J...    Thermal Management Requirements    approved
RISK   RISK@1   RISK-01HC2...   Battery Thermal Runaway            review
TEST   TEST@5   TEST-01HC2...   Thermal Cycling Test Protocol      draft
CMP    CMP@2    CMP-01HC2...    Thermal Interface Material         approved

4 result(s) found
```

### JSON Output

```bash
$ tdt search "thermal" -f json
```

```json
{
  "results": [
    {
      "entity_type": "REQ",
      "short_id": "REQ@3",
      "id": "REQ-01HC2JB7SMQX7RS1Y0GFKBHPTD",
      "title": "Thermal Management Requirements",
      "status": "approved"
    },
    {
      "entity_type": "RISK",
      "short_id": "RISK@1",
      "id": "RISK-01HC2JB7SMQX7RS1Y0GFKBHPTE",
      "title": "Battery Thermal Runaway",
      "status": "review"
    }
  ],
  "count": 2
}
```

### Count Only

```bash
$ tdt search "thermal" --count
4
```

## Search Behavior

### What Gets Searched

The search looks in:
- Entity titles
- Entity descriptions/text fields

### Case Sensitivity

- By default, search is **case-insensitive**
- Use `--case-sensitive` for exact case matching

### Partial Matching

- Search terms match partial strings
- `"therm"` will match "thermal", "thermometer", etc.

## Use Cases

### Find Related Entities

```bash
# Find everything related to a component
tdt search "motor"

# Find all thermal-related items
tdt search "thermal"
```

### Audit by Author

```bash
# Find all entities created by a team member
tdt search "" --author "Jane Doe"

# Find draft items by author
tdt search "" --author "Bob" --status draft
```

### Release Planning

```bash
# Find all items tagged for a release
tdt search "" --tag "v2.0"

# Find approved items for release
tdt search "" --tag "release" --status approved
```

### Quality Review

```bash
# Find all open NCRs and CAPAs
tdt search "" --entity-type ncr,capa --status draft

# Find high-risk items
tdt search "critical" --entity-type risk
```

## Performance

The search command uses Tessera's SQLite cache for fast lookups. The cache is automatically maintained and updated when entities change.

If search results seem stale, rebuild the cache:

```bash
tdt cache rebuild
```

## Tips

1. **Use short type names**: `--entity-type req` instead of `--entity-type requirement`

2. **Combine with other commands**: Pipe search results to other tools
   ```bash
   tdt search "thermal" -f id | xargs -I {} tdt show {}
   ```

3. **Empty search with filters**: Use `""` to search all entities with filters
   ```bash
   tdt search "" --author "Jane" --status draft
   ```

4. **Quick counts**: Use `--count` for dashboards or scripts
   ```bash
   echo "Draft items: $(tdt search '' --status draft --count)"
   ```
