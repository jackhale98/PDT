# TDT Recent Activity

This document describes the recent activity command in TDT (Tessera Design Toolkit).

## Overview

The `tdt recent` command shows recently modified entities across all types, sorted by file modification time. This is useful for tracking project activity and finding entities you were just working on.

## CLI Commands

### Basic Usage

```bash
# Show 20 most recently modified entities (default)
tdt recent

# Show more results
tdt recent -n 50

# Show fewer results
tdt recent -n 5
```

### Filter by Entity Type

```bash
# Show only recent requirements
tdt recent -t req

# Show only recent risks
tdt recent -t risk

# Show multiple types (comma-separated)
tdt recent -t req,risk,test

# Available type prefixes:
#   req, risk, test, rslt, cmp, asm, feat, mate,
#   tol, proc, ctrl, work, lot, dev, ncr, capa, quote, sup
```

### Output Options

```bash
# Show count only
tdt recent --count

# Output as JSON
tdt recent -f json

# Output as YAML
tdt recent -f yaml

# Output as CSV
tdt recent -f csv

# Output as Markdown table
tdt recent -f md

# Output full IDs only (for piping)
tdt recent -f id

# Output short IDs only (for piping)
tdt recent -f short-id
```

## Example Output

### Default Output (TSV)

```
$ tdt recent -n 5
5 recently modified entities:

SHORT      ID                TYPE   TITLE                               STATUS
-------------------------------------------------------------------------------------
REQ@9      REQ-01KDC5WA...   REQ    New Temperature Requirement         draft
TOL@2      TOL-01KD1VKA...   TOL    Pin in Hole Stack                   draft
MATE@3     MATE-01KD1VH...   MATE   Pin in Hole                         draft
FEAT@8     FEAT-01KD1VG...   FEAT   OD                                  draft
FEAT@7     FEAT-01KD1VE...   FEAT   Hole                                draft

Use <TYPE> show <ID> to show entity details.
```

### JSON Output

```bash
$ tdt recent -n 2 -f json
```

```json
[
  {
    "id": "REQ-01KDC5WA2M12JF8DW7RN7TME1C",
    "entity_type": "REQ",
    "title": "New Temperature Requirement",
    "status": "draft",
    "author": "Jack"
  },
  {
    "id": "TOL-01KD1VKABCD1234EFGH5678JKL",
    "entity_type": "TOL",
    "title": "Pin in Hole Stack",
    "status": "draft",
    "author": "Jack"
  }
]
```

### Short ID Output (for piping)

```bash
$ tdt recent -n 3 -f short-id
REQ@9
TOL@2
MATE@3
```

## Use Cases

### Quick Navigation

```bash
# Find the entity you were just working on
tdt recent -n 1

# Show recent with details
tdt recent -n 1 -f id | xargs tdt show
```

### Activity Tracking

```bash
# Count entities modified today (combine with shell)
tdt recent -n 100 --count

# Track recent changes by type
tdt recent -t req -n 10
tdt recent -t risk -n 10
```

### Pipeline Integration

```bash
# Show details of recent requirements
tdt recent -t req -f short-id | xargs -I {} tdt req show {}

# Add tag to recently modified items
tdt recent -n 5 -f id | tdt bulk add-tag "in-progress"
```

## Performance

The `recent` command uses TDT's SQLite cache for fast lookups. It queries by `file_mtime` (file modification timestamp) which is indexed for performance.

The cache auto-syncs on each command, so results reflect the current state of your project files.

## Tips

1. **Default is 20**: Without `-n`, you get the 20 most recent entities

2. **Filter by type for focus**: Use `-t req` when you know what you're looking for

3. **Use with show**: Quick way to see what you just edited
   ```bash
   tdt recent -n 1 -f id | xargs tdt show
   ```

4. **Track team activity**: Combined with git, see what's changed recently
   ```bash
   tdt recent -n 20 -f csv > recent-activity.csv
   ```
