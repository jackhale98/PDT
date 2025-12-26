# TDT Tags

This document describes tag management in TDT (Tessera Design Toolkit).

## Overview

Tags provide a flexible way to categorize and group entities across your project. Unlike fixed fields like status or priority, tags can be any string and entities can have multiple tags.

TDT provides commands for:
- Viewing all tags in use (`tdt tags list`)
- Finding entities with a specific tag (`tdt tags show`)
- Adding/removing tags in bulk (`tdt bulk add-tag/remove-tag`)

## CLI Commands

### List All Tags

```bash
# List all tags with usage counts
tdt tags list

# Show only top 10 most-used tags
tdt tags list -n 10

# Show count of unique tags only
tdt tags list --count

# Output as JSON
tdt tags list -f json

# Output as CSV
tdt tags list -f csv

# Output tag names only
tdt tags list -f id
```

### Show Entities with Tag

```bash
# Show all entities with a specific tag
tdt tags show precision

# Limit results
tdt tags show thermal -n 10

# Show count only
tdt tags show precision --count

# Output as JSON
tdt tags show mechanical -f json

# Output IDs for piping
tdt tags show urgent -f id
tdt tags show critical -f short-id
```

### Adding Tags

```bash
# Add tag to specific entities
tdt bulk add-tag "v2.0" REQ@1 REQ@2 REQ@3

# Add tag to all entities of a type
tdt bulk add-tag "reviewed" -t req

# Add tag to entities with specific status
tdt bulk add-tag "needs-review" --status draft

# Preview without making changes
tdt bulk add-tag "release" REQ@1 REQ@2 --dry-run
```

### Removing Tags

```bash
# Remove tag from specific entities
tdt bulk remove-tag "deprecated" CMP@1 CMP@2

# Remove tag from all entities of a type
tdt bulk remove-tag "old" -t cmp

# Preview changes
tdt bulk remove-tag "draft" -t req --dry-run
```

## Example Output

### List Tags

```
$ tdt tags list
18 unique tags in project:

TAG                            COUNT
------------------------------------------
precision                      5
mechanical                     4
thermal                        3
motor                          2
press-fit                      2
sliding-fit                    2
vendor-part                    2
bearing                        1
clearance                      1
...

Use tdt tags show <tag> to see entities with a tag.
```

### Show Tag

```
$ tdt tags show precision
5 entities with tag 'precision':

SHORT      ID                TYPE   TITLE                               STATUS
-------------------------------------------------------------------------------------
FEAT@1     FEAT-01KC8FN...   FEAT   Bushing OD                          approved
FEAT@5     FEAT-01KC8FN...   FEAT   Bushing Bore                        approved
FEAT@2     FEAT-01KC8FN...   FEAT   Shaft Diameter                      approved
FEAT@6     FEAT-01KC8FM...   FEAT   Housing Bore                        approved
REQ@6      REQ-01KC8FFD...   REQ    Shaft-Bushing Clearance             approved
```

### JSON Output

```bash
$ tdt tags list -f json
```

```json
[
  {"tag": "precision", "count": 5},
  {"tag": "mechanical", "count": 4},
  {"tag": "thermal", "count": 3}
]
```

## Use Cases

### Release Management

```bash
# Tag items for a release
tdt req list --status approved -f id | tdt bulk add-tag "v2.0"

# Find all items in a release
tdt tags show v2.0

# Count items by release
tdt tags show v2.0 --count
```

### Review Workflow

```bash
# Tag items needing review
tdt req list --status draft -f id | tdt bulk add-tag "needs-review"

# Find items to review
tdt tags show needs-review

# After review, remove tag and update status
tdt bulk remove-tag "needs-review" REQ@1 REQ@2
tdt bulk set-status review REQ@1 REQ@2
```

### Categorization

```bash
# List items by category tag
tdt tags show mechanical
tdt tags show electrical
tdt tags show software

# See tag distribution
tdt tags list -n 20
```

### Reporting

```bash
# Export tagged items to CSV
tdt tags show critical -f csv > critical-items.csv

# Count items by tag for metrics
echo "Precision items: $(tdt tags show precision --count)"
echo "Thermal items: $(tdt tags show thermal --count)"
```

## Pipeline Integration

### Combined with Search

```bash
# Search within tagged items
tdt search "motor" --tag thermal

# Find items with multiple criteria
tdt search "" --tag precision --status approved
```

### Combined with Bulk Operations

```bash
# Add multiple tags to search results
tdt search "thermal" -f id | tdt bulk add-tag "heat-related"

# Tag recent items
tdt recent -n 10 -f id | tdt bulk add-tag "in-progress"
```

## Performance

Tag queries use TDT's SQLite cache:
- `tags list` aggregates tags from the cache (O(n) where n = entities)
- `tags show` filters by tag using indexed lookups
- Both are fast even with thousands of entities

## Tips

1. **Use consistent naming**: Pick a tag convention (kebab-case, camelCase) and stick to it

2. **Don't over-tag**: Tags work best when they're meaningful categories, not duplicates of other fields

3. **Use for cross-cutting concerns**: Tags are great for things that span entity types
   - `v2.0` - release version
   - `needs-review` - workflow state
   - `thermal` - domain category

4. **Combine with status**: Tags complement status, they don't replace it
   ```bash
   # Wrong: using tags for status
   tdt bulk add-tag "draft" REQ@1

   # Right: using status for status
   tdt bulk set-status draft REQ@1

   # Right: using tags for categories
   tdt bulk add-tag "phase1" REQ@1
   ```

5. **Check existing tags first**: Before adding a new tag, see what's already in use
   ```bash
   tdt tags list | grep -i thermal
   ```
