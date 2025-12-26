# TDT Demo Recordings

This folder contains [VHS](https://github.com/charmbracelet/vhs) tape scripts for generating terminal demo GIFs.

## Prerequisites

Install VHS:

```bash
# macOS
brew install vhs

# Linux (via Go)
go install github.com/charmbracelet/vhs@latest

# Or download from releases
# https://github.com/charmbracelet/vhs/releases
```

VHS also requires `ffmpeg` and `ttyd` for recording.

**Note**: On Linux in sandboxed environments, you may need to set:
```bash
export VHS_NO_SANDBOX=true
```

## Available Demos

| Demo | Description | Output |
|------|-------------|--------|
| `quickstart.tape` | Init, create requirements, list, show | `quickstart.gif` |
| `tolerance.tape` | Features, mates, stackup analysis | `tolerance.gif` |
| `fmea.tape` | FMEA risks, ratings, matrix | `fmea.gif` |
| `traceability.tape` | Links, trace, where-used | `traceability.gif` |
| `recent-tags.tape` | Recent activity, tag management | `recent-tags.gif` |
| `search.tape` | Global search with filters | `search.gif` |

## Generating GIFs

```bash
# Generate a single demo
cd pdt/demos
vhs quickstart.tape

# Generate all demos
for tape in *.tape; do
  vhs "$tape"
done
```

## Customization

Edit the tape files to customize:

- `Set Theme "Dracula"` - Terminal theme (try: `Catppuccin Mocha`, `Nord`, `GitHub Dark`)
- `Set FontSize 16` - Font size in pixels
- `Set Width 1000` - Terminal width
- `Set Height 600` - Terminal height
- `Sleep 2s` - Pause duration for readability

## Output Formats

VHS can output multiple formats:

```tape
Output demos/quickstart.gif
Output demos/quickstart.mp4
Output demos/quickstart.webm
```

## Usage in Documentation

Reference the generated GIFs in markdown:

```markdown
![TDT Quick Start](demos/quickstart.gif)
```

Or in HTML:

```html
<img src="demos/quickstart.gif" alt="TDT Quick Start Demo" />
```
