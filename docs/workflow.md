# TDT Workflow & Collaboration

This document describes the git-integrated workflow features in TDT (Tessera Design Toolkit).

## Overview

TDT provides opt-in workflow commands that help teams collaborate on product data using git and GitHub/GitLab. These features are designed for users who may not be familiar with git, providing a guided experience for common review and approval workflows.

**Key Principle**: These features are completely optional. Git-savvy users can continue using standard git commands directly.

## Status Workflow

All TDT entities follow a common status progression:

```
Draft → Review → Approved → Released
```

| Status | Description |
|--------|-------------|
| `draft` | Initial state, work in progress |
| `review` | Submitted for review (PR created) |
| `approved` | Approved by authorized reviewer |
| `released` | Officially released for use |

## Configuration

Enable workflow features in `.tdt/config.yaml`:

```yaml
workflow:
  enabled: true
  provider: github    # github, gitlab, or none
  auto_commit: true   # Auto-commit status changes
  auto_merge: false   # Merge PR on approval
  base_branch: main   # Target branch for PRs
```

### Configuration Keys

| Key | Description | Default |
|-----|-------------|---------|
| `workflow.enabled` | Enable workflow commands | `false` |
| `workflow.provider` | Git provider: `github`, `gitlab`, `none` | `none` |
| `workflow.auto_commit` | Auto-commit on status changes | `true` |
| `workflow.auto_merge` | Merge PR automatically on approval | `false` |
| `workflow.base_branch` | Target branch for PRs | `main` |

### Setting via CLI

```bash
# Enable workflow with GitHub
tdt config set workflow.enabled true
tdt config set workflow.provider github

# Enable workflow with GitLab
tdt config set workflow.enabled true
tdt config set workflow.provider gitlab

# Manual mode (no PR integration)
tdt config set workflow.enabled true
tdt config set workflow.provider none
```

## Team Roster

Define team members and their approval permissions in `.tdt/team.yaml`:

```yaml
version: 1
members:
  - name: "Jane Smith"
    email: "jane@example.com"
    username: "jsmith"
    roles: [engineering, quality]
    active: true

  - name: "Bob Wilson"
    email: "bob@example.com"
    username: "bwilson"
    roles: [quality, management]
    active: true

approval_matrix:
  REQ: [engineering, quality]
  RISK: [quality, management]
  NCR: [quality]
  _release: [management]
```

### Roles

| Role | Description |
|------|-------------|
| `engineering` | Can approve technical entities (requirements, components) |
| `quality` | Can approve quality-related entities (risks, NCRs, CAPAs) |
| `management` | Can approve releases and high-level decisions |
| `admin` | Full access to all operations |

### Team Commands

```bash
# Initialize team roster
tdt team init

# Add a team member
tdt team add --name "Jane Smith" --email jane@co.com --username jsmith --roles engineering,quality

# Remove a team member
tdt team remove jsmith

# List team members
tdt team list

# Check your own role
tdt team whoami
```

## Submit Command

Submit entities for review (creates a PR if provider configured):

```bash
# Single entity
tdt submit REQ@1

# Multiple entities
tdt submit REQ@1 REQ@2 RISK@3

# With a message
tdt submit REQ@1 -m "Ready for review"

# Pipe from list command
tdt req list -s draft -f short-id | tdt submit -

# All draft entities of a type
tdt submit --type req --status draft

# Create as draft PR
tdt submit REQ@1 --draft

# Skip PR creation (git only)
tdt submit REQ@1 --no-pr
```

### Options

| Option | Short | Description |
|--------|-------|-------------|
| `--message` | `-m` | Submission message |
| `--type` | `-t` | Filter by entity type |
| `--status` | `-s` | Filter by status (default: draft) |
| `--all` | | Submit all matching entities |
| `--no-pr` | | Skip PR creation |
| `--draft` | | Create as draft PR |
| `--yes` | `-y` | Skip confirmation prompt |
| `--dry-run` | | Show what would be done |
| `--verbose` | `-v` | Print commands as they run |

### What Submit Does

1. Validates entities are in Draft status
2. Creates a feature branch (e.g., `review/REQ-01KCWY20`)
3. Changes status to Review in entity files
4. Commits and pushes changes
5. Creates a PR (if provider configured)
6. Prints the PR URL

## Approve Command

Approve entities under review:

```bash
# Single entity
tdt approve REQ@1

# Multiple entities
tdt approve REQ@1 REQ@2 RISK@3

# Approve all entities in a PR
tdt approve --pr 42

# Approve and merge
tdt approve REQ@1 --merge

# With approval message
tdt approve REQ@1 -m "Looks good"

# Skip authorization check (admin)
tdt approve REQ@1 --force
```

### Options

| Option | Short | Description |
|--------|-------|-------------|
| `--pr` | | Approve all entities in a PR by number |
| `--message` | `-m` | Approval comment |
| `--merge` | | Merge PR after approval |
| `--no-merge` | | Skip merge even if auto_merge enabled |
| `--force` | | Skip authorization check |
| `--yes` | `-y` | Skip confirmation prompt |
| `--dry-run` | | Show what would be done |
| `--verbose` | `-v` | Print commands as they run |

### What Approve Does

1. Validates entities are in Review status
2. Verifies user has approval authorization
3. Changes status to Approved
4. Records approval metadata (who, when, role)
5. Commits changes
6. Adds approval to PR (if provider configured)
7. Optionally merges PR

## Reject Command

Reject entities back to draft status:

```bash
# Single entity with reason
tdt reject REQ@1 -r "Needs more detail"

# Multiple entities
tdt reject REQ@1 REQ@2 -r "Incomplete specifications"

# Reject all entities in a PR
tdt reject --pr 42 -r "Does not meet requirements"
```

### Options

| Option | Short | Description |
|--------|-------|-------------|
| `--reason` | `-r` | Rejection reason (required) |
| `--pr` | | Reject all entities in a PR |
| `--yes` | `-y` | Skip confirmation prompt |
| `--dry-run` | | Show what would be done |
| `--verbose` | `-v` | Print commands as they run |

### What Reject Does

1. Validates entities are in Review status
2. Changes status back to Draft
3. Records rejection (who, when, reason)
4. Commits changes
5. Closes PR with comment (if provider configured)

## Release Command

Release approved entities:

```bash
# Single entity
tdt release REQ@1

# Multiple entities
tdt release REQ@1 REQ@2 REQ@3

# All approved requirements
tdt release --type req

# All approved entities
tdt release --all

# Pipe from list
tdt req list -s approved -f short-id | tdt release -
```

### Options

| Option | Short | Description |
|--------|-------|-------------|
| `--entity-type` | `-t` | Filter by entity type |
| `--all` | | Release all approved entities |
| `--message` | `-m` | Release message |
| `--force` | | Skip authorization check |
| `--yes` | `-y` | Skip confirmation prompt |
| `--dry-run` | | Show what would be done |
| `--verbose` | `-v` | Print commands as they run |

### What Release Does

1. Validates entities are in Approved status
2. Verifies user has release authorization (Management role)
3. Changes status to Released
4. Commits with release message

## Review Command

View pending reviews:

```bash
# List items pending your review
tdt review list

# Filter by entity type
tdt review list -t req

# Show all pending reviews (not just yours)
tdt review list --all

# Summary of review queue
tdt review summary
```

### Example Output

```
Pending reviews for jsmith:

SHORT   TYPE   TITLE                        AUTHOR      PR
REQ@1   REQ    Pump GPM requirement         alice       #42
RISK@3  RISK   Motor overheating failure    bob         #45

2 items pending your review. Run `tdt approve <id>` to approve.
```

## Provider Integration

### GitHub

TDT uses the `gh` CLI for GitHub integration. Install it from https://cli.github.com and authenticate:

```bash
gh auth login
```

Commands used:
- `gh pr create` - Create pull request
- `gh pr review --approve` - Add approval
- `gh pr merge` - Merge PR
- `gh pr list --search "review-requested:@me"` - List pending reviews

### GitLab

TDT uses the `glab` CLI for GitLab integration. Install it from https://gitlab.com/gitlab-org/cli and authenticate:

```bash
glab auth login
```

Commands used:
- `glab mr create` - Create merge request
- `glab mr approve` - Approve MR
- `glab mr merge` - Merge MR
- `glab mr list --reviewer=@me` - List pending reviews

### Manual Mode (No Provider)

Set `provider: none` to use workflow commands without GitHub/GitLab integration:

```bash
tdt submit REQ@1
# → Creates branch, commits, pushes
# → Prints: "Create PR manually at your git provider"
```

## Transparency

All workflow commands support transparency flags:

```bash
# Show what would be done without executing
tdt submit REQ@1 --dry-run

# Print commands as they run
tdt submit REQ@1 --verbose
```

### Example Dry Run

```
$ tdt submit REQ@1 --dry-run

Would execute:
  git checkout -b review/REQ-01KCWY20
  git add requirements/inputs/REQ-01KCWY20.tdt.yaml
  git commit -m "Submit REQ@1: Pump GPM requirement"
  git push -u origin review/REQ-01KCWY20
  gh pr create --title "Submit REQ@1: Pump GPM requirement" --base main

No changes made (dry run).
```

## Full Workflow Example

### Setup

```bash
# Initialize a TDT project with workflow
tdt init myproject
cd myproject

# Enable workflow with GitHub
tdt config set workflow.enabled true
tdt config set workflow.provider github

# Create team roster
tdt team init
tdt team add --name "Jane Smith" --username jsmith --roles engineering,quality
tdt team add --name "Bob Wilson" --username bwilson --roles quality,management
```

### Author Creates and Submits

```bash
# Create a requirement
tdt req new --title "Pump GPM requirement"

# Submit for review
tdt submit REQ@1 -m "Initial pump requirement"
# → Creates branch, commits, pushes, opens PR #42
```

### Reviewer Approves

```bash
# Check pending reviews
tdt review list
# → Shows REQ@1 pending review

# Approve (with merge)
tdt approve REQ@1 --merge
# → Adds approval to PR #42, merges to main
```

### Manager Releases

```bash
# Release the approved requirement
tdt release REQ@1
# → Status: approved → released
```

## Best Practices

### For Teams

1. **Define clear roles** - Set up team roster with appropriate role assignments
2. **Use PRs for visibility** - Keep provider configured for audit trail
3. **Review before release** - Require approval before releasing entities

### For Solo Developers

1. **Minimal config** - Just enable workflow, skip team roster
2. **Use provider: none** - No GitHub/GitLab dependency needed
3. **Status tracking** - Still get status progression benefits

### Git-Savvy Users

Workflow commands are optional. You can always:

```bash
# Use git directly
git add requirements/inputs/REQ-*.yaml
git commit -m "Add requirements"
git push

# Edit status manually in YAML files
# Create PRs through web UI
```
