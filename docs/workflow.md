# TDT Workflow & Collaboration

This document describes the workflow features in TDT (Tessera Design Toolkit).

## Overview

TDT provides workflow commands that help teams collaborate on product data with git-based version control. The commands are designed to be **git-transparent** - users run simple TDT commands and git operations happen automatically in the background.

**For non-git users**: You just run `tdt approve REQ@1` - TDT handles all the git operations for you.

**For git users**: All git operations are visible with `--verbose`, and you can always use git commands directly.

## Status Workflow

All TDT entities follow a common status progression:

```
Draft → Review → Approved → Released
```

| Status | Description |
|--------|-------------|
| `draft` | Initial state, work in progress |
| `review` | Submitted for review |
| `approved` | Approved by authorized reviewers |
| `released` | Officially released for use |

## Configuration

Enable workflow features in `.tdt/config.yaml`:

```yaml
workflow:
  enabled: true
  provider: github    # github, gitlab, or none
  auto_commit: true   # Auto-commit status changes (recommended)
  auto_merge: false   # Merge PR on approval
  base_branch: main   # Target branch for PRs

  # Default approval requirements (applies to all entity types)
  default_approvals:
    min_approvals: 1
    require_unique_approvers: true

  # Per-entity-type approval requirements
  approvals:
    RISK:
      min_approvals: 2
      required_roles: [engineering, quality]
    REQ:
      min_approvals: 2
    NCR:
      min_approvals: 2
      required_roles: [quality]
```

### Configuration Keys

| Key | Description | Default |
|-----|-------------|---------|
| `workflow.enabled` | Enable workflow commands | `false` |
| `workflow.provider` | Git provider: `github`, `gitlab`, `none` | `none` |
| `workflow.auto_commit` | Auto-commit on status changes | `true` |
| `workflow.auto_merge` | Merge PR automatically on approval | `false` |
| `workflow.base_branch` | Target branch for PRs | `main` |
| `workflow.default_approvals` | Default approval requirements | See below |
| `workflow.approvals.<TYPE>` | Per-entity-type approval requirements | See below |

### Approval Requirements

| Key | Description | Default |
|-----|-------------|---------|
| `min_approvals` | Minimum number of approvals required | `1` |
| `required_roles` | List of roles that must provide at least one approval | `[]` |
| `require_unique_approvers` | Prevent same person from approving twice | `true` |
| `require_signature` | Require GPG-signed commits (for 21 CFR Part 11) | `false` |

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

# Set default approval requirements
tdt config set workflow.default_approvals.min_approvals 2
tdt config set workflow.default_approvals.require_unique_approvers true

# Set per-entity requirements (e.g., for RISK entities)
tdt config set workflow.approvals.RISK.min_approvals 2
tdt config set workflow.approvals.RISK.require_signature true

# View all available config keys
tdt config keys
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

Approve entities under review. This is designed to be simple - just run `tdt approve` and everything is handled automatically:

```bash
# Simple approval
tdt approve REQ@1

# Multiple entities
tdt approve REQ@1 REQ@2 RISK@3

# Approve all entities in a PR
tdt approve --pr 42

# Approve and merge
tdt approve REQ@1 --merge

# With approval message
tdt approve REQ@1 -m "Looks good"

# Check approval status without adding approval
tdt approve REQ@1 --status

# Skip git commit (just update YAML files)
tdt approve REQ@1 --no-commit

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
| `--no-commit` | | Skip git commit (just update YAML files) |
| `--sign` | `-S` | GPG-sign the approval commit (for Part 11 compliance) |
| `--status` | | Show approval status without adding an approval |
| `--force` | | Skip authorization check |
| `--yes` | `-y` | Skip confirmation prompt |
| `--dry-run` | | Show what would be done |
| `--verbose` | `-v` | Print commands as they run |

### Multi-Signature Approvals

When `min_approvals > 1` is configured for an entity type, approvals are accumulated until the requirements are met:

```bash
# First approval (entity stays in Review status)
$ tdt approve RISK@1 -m "Engineering approval"
  Recorded approval by jsmith for 1 entities
  Committed: "Approve RISK-01KCWY20: Motor failure"
1 entities need more approvals before transitioning to 'approved' status.

# Check current approval status
$ tdt approve RISK@1 --status
RISK-01KCWY20  Motor failure analysis
  Status: review
  Approvals: 1/2
  Approvers: jsmith
  Missing roles: quality
  Need 1 more approval(s)

# Second approval (meets requirements, transitions to Approved)
$ tdt approve RISK@1 -m "Quality approval"
  Recorded approval by bwilson for 1 entities
  Committed: "Approve RISK-01KCWY20: Motor failure"
1 entities fully approved.
```

### What Approve Does

1. Validates entities are in Review status
2. Verifies user has approval authorization (if team roster configured)
3. Checks for duplicate approvals (if require_unique_approvers is enabled)
4. Records approval as an "electronic signature" (approver name, email, role, timestamp)
5. Checks if approval requirements are met for the entity type
6. If requirements met: Changes status to Approved
7. If requirements not met: Entity stays in Review status
8. Commits changes (if git available and auto_commit enabled)
9. Adds approval to PR (if provider configured)
10. Optionally merges PR (only if all entities are fully approved)

### Approval Records (Electronic Signatures)

Each approval is recorded in the entity YAML file as an "electronic signature":

```yaml
approvals:
  - approver: "Jane Smith"
    email: "jane@example.com"
    role: engineering
    timestamp: 2024-01-15T10:30:00Z
    comment: "Reviewed, looks good"
  - approver: "Bob Wilson"
    email: "bob@example.com"
    role: quality
    timestamp: 2024-01-15T14:22:00Z
    comment: "Quality approved"
    signature_verified: true        # GPG signature verified
    signing_key: "ABC123DEF456"     # GPG key ID
```

This provides a complete audit trail of who approved what and when.

## 21 CFR Part 11 Compliance

For FDA-regulated industries (medical devices, pharmaceuticals, biotech), TDT supports [21 CFR Part 11](https://www.fda.gov/regulatory-information/search-fda-guidance-documents/part-11-electronic-records-electronic-signatures-scope-and-application) compliance through GPG-signed approvals.

### Why GPG Signatures?

Part 11 requires electronic signatures to be:
- **Unique to an individual** - GPG keys are personal and password-protected
- **Verifiable** - Signatures can be cryptographically verified
- **Inextricably linked to the record** - Git commits bind the signature to exact content

Combined with Git's immutable audit trail, this satisfies Part 11's requirements for electronic records and signatures.

### Enabling Part 11 Mode

```yaml
# .tdt/config.yaml
workflow:
  enabled: true
  provider: github

  # Require GPG signatures for all approvals
  default_approvals:
    min_approvals: 2
    require_signature: true

  # Or require signatures only for specific entity types
  approvals:
    RISK:
      min_approvals: 2
      required_roles: [engineering, quality]
      require_signature: true
    REQ:
      min_approvals: 2
      require_signature: true
```

### Using GPG-Signed Approvals

```bash
# Approve with GPG signature
tdt approve REQ@1 --sign

# If require_signature is enabled, --sign is required
tdt approve RISK@1 --sign -m "Quality review complete"
```

### Setting Up GPG Signing

Before using GPG-signed approvals, each team member needs to configure GPG signing:

1. **Generate a GPG key** (if you don't have one):
   ```bash
   gpg --full-generate-key
   ```

2. **Configure Git to use your key**:
   ```bash
   # List your keys to find the key ID
   gpg --list-secret-keys --keyid-format=long

   # Configure Git
   git config --global user.signingkey YOUR_KEY_ID
   git config --global commit.gpgsign true
   ```

3. **Add your public key to GitHub/GitLab** for verification badges

For detailed instructions, see:
- [GitHub: Managing commit signature verification](https://docs.github.com/en/authentication/managing-commit-signature-verification)
- [GitLab: Signing commits with GPG](https://docs.gitlab.com/ee/user/project/repository/signed_commits/gpg.html)

### Part 11 Compliance Checklist

Using TDT with GPG signing satisfies several Part 11 requirements:

| Part 11 Requirement | TDT Feature |
|---------------------|-------------|
| Audit trail (§11.10(e)) | Git commit history with timestamps |
| Unique user identification (§11.100) | GPG keys + team roster |
| Signature meaning (§11.50) | Approval comments and role |
| Non-repudiation (§11.200) | GPG cryptographic signatures |
| Record integrity (§11.10(c)) | Git hash-linked commits |

**Note**: Technology alone doesn't ensure compliance. You also need:
- System validation documentation (IQ/OQ/PQ)
- Standard Operating Procedures (SOPs)
- Training records
- Periodic audits

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

### For Teams New to Git

1. **Just use TDT commands** - Run `tdt approve`, `tdt submit`, etc. Git happens automatically
2. **Use `--verbose`** - See what git commands are running to learn
3. **Use `--dry-run`** - Preview what will happen before executing

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
