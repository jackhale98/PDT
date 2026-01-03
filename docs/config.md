# TDT Configuration

This document describes the configuration system in TDT (Tessera Design Toolkit).

## Overview

TDT uses a layered configuration system that allows settings at multiple levels. Configuration values are merged with higher-priority sources overriding lower ones.

## Configuration Hierarchy

Configuration is loaded from multiple sources (highest priority first):

1. **Environment variables** - `TDT_AUTHOR`, `TDT_EDITOR`
2. **Project config** - `.tdt/config.yaml` in the project root
3. **Global user config** - `~/.config/tdt/config.yaml`

## Configuration Keys

| Key | Description | Example |
|-----|-------------|---------|
| `author` | Default author for new entities | `"Jane Doe"` |
| `editor` | Editor command for `tdt edit` | `"code --wait"` |
| `pager` | Pager command for long output | `"less"` |
| `default_format` | Default output format | `"yaml"` |
| `workflow.enabled` | Enable workflow commands | `true` |
| `workflow.provider` | Git provider: github, gitlab, or none | `"github"` |
| `workflow.auto_commit` | Auto-commit on status changes | `true` |
| `workflow.auto_merge` | Merge PR automatically on approval | `false` |
| `workflow.base_branch` | Target branch for PRs | `"main"` |
| `manufacturing.lot_branch_enabled` | Auto-create git branch per lot | `true` |
| `manufacturing.base_branch` | Branch to create lot branches from | `"main"` |
| `manufacturing.branch_pattern` | Lot branch naming pattern | `"lot/{lot_number}"` |
| `manufacturing.create_tags` | Create tags at lot lifecycle events | `true` |
| `manufacturing.sign_commits` | Require signed commits for lots | `false` |

## CLI Commands

### Show configuration

```bash
# Show effective (merged) configuration
tdt config show

# Show a specific key's value
tdt config show author

# Show only project-level config
tdt config show --project-only

# Show only global (user) config
tdt config show --global-only
```

**Example output:**

```
Effective Configuration

  author: Jane Doe
  editor: code --wait
  pager: (not set)
  default_format: (not set)

Config Sources (in priority order):
  1. Environment variables (TDT_AUTHOR, TDT_EDITOR)
  2. Project config (.tdt/config.yaml)
  3. Global config (~/.config/tdt/config.yaml)
```

### Set configuration

```bash
# Set in project config (default)
tdt config set author "Jane Doe"

# Set in global (user) config
tdt config set author "Jane Doe" --global
tdt config set author "Jane Doe" -g

# Set editor command
tdt config set editor "code --wait"

# Set nested values (dot notation)
tdt config set some.nested.key "value"
```

### Unset configuration

```bash
# Remove from project config
tdt config unset author

# Remove from global config
tdt config unset author --global
tdt config unset author -g
```

### Show config file paths

```bash
# Show all config file paths
tdt config path

# Show only project config path
tdt config path --project-only

# Show only global config path
tdt config path --global-only
```

**Example output:**

```
Configuration file paths:

  Global: /home/user/.config/tdt/config.yaml
         (exists)

  Project: /home/user/myproject/.tdt/config.yaml
          (not created)
```

### List available keys

```bash
tdt config keys
```

**Example output:**

```
Available configuration keys:

  author               Default author for new entities
  editor               Editor command for `tdt edit`
  pager                Pager command for long output
  default_format       Default output format (yaml, json, tsv, etc.)

Use 'tdt config set <key> <value>' to set a value.
```

## Configuration File Format

Configuration files use YAML format:

```yaml
# .tdt/config.yaml or ~/.config/tdt/config.yaml
author: "Jane Doe"
editor: "code --wait"
pager: "less"
default_format: "yaml"

# Workflow configuration (optional)
workflow:
  enabled: true
  provider: github    # github, gitlab, or none
  auto_commit: true
  auto_merge: false
  base_branch: main

# Manufacturing workflow configuration (optional)
manufacturing:
  lot_branch_enabled: true    # Auto-create git branch per lot
  base_branch: main           # Branch to create lot branches from
  branch_pattern: "lot/{lot_number}"  # Lot branch naming pattern
  create_tags: true           # Create tags at lot lifecycle events
  sign_commits: false         # Require signed commits for lots
```

## Environment Variables

Environment variables take highest priority:

| Variable | Overrides |
|----------|-----------|
| `TDT_AUTHOR` | `author` setting |
| `TDT_EDITOR` | `editor` setting |

```bash
# Set author via environment
export TDT_AUTHOR="Jane Doe"

# Set editor via environment
export TDT_EDITOR="vim"
```

## Editor Configuration

The `editor` setting supports commands with arguments:

```bash
# VS Code (wait for file to close)
tdt config set editor "code --wait"

# Emacs client
tdt config set editor "emacsclient -nw"

# Vim
tdt config set editor "vim"

# Nano
tdt config set editor "nano"
```

If not configured, TDT falls back to:
1. `$EDITOR` environment variable
2. `$VISUAL` environment variable
3. `vi` (default)

## Best Practices

### Project vs Global Config

- **Global config**: Settings that apply to all your projects
  - Your default author name
  - Your preferred editor

- **Project config**: Settings specific to a project
  - Project-specific author (team name)
  - Project-specific output format preferences

### Team Projects

For team projects, consider:

1. **Don't commit personal settings** - Add `.tdt/config.yaml` to `.gitignore` if it contains personal preferences
2. **Use environment variables** - Let each team member set their own `TDT_AUTHOR`
3. **Document conventions** - Document expected configuration in project README

### Example Setup

```bash
# Set your global author name
tdt config set author "Jane Doe" --global

# Set your preferred editor globally
tdt config set editor "code --wait" --global

# For a specific project, override author
cd myproject
tdt config set author "Project Team"
```
