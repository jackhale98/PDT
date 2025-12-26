# TDT Interactive Mode Demo
# Demonstrates: schema-driven wizard for creating entities

Output demos/interactive.gif
Output docs/interactive.gif
Set Theme "Dracula"
Set FontSize 16
Set Width 1000
Set Height 700
Set Padding 20

# Setup - use temp directory
Hide
Type "cd $(mktemp -d) && tdt init -q"
Enter
Sleep 500ms
Show

# Title
Type "# Interactive Mode"
Enter
Sleep 1s

# Start interactive requirement creation
Type "tdt req new -i"
Enter
Sleep 1s

# Fill in the wizard prompts
# Type selection
Type "input"
Enter
Sleep 500ms

# Title
Type "Motor Operating Temperature Range"
Enter
Sleep 500ms

# Priority - use arrow key to select
Down
Enter
Sleep 500ms

# Status
Enter
Sleep 500ms

# Category
Type "thermal"
Enter
Sleep 500ms

# Tags
Type "thermal,critical,motor"
Enter
Sleep 500ms

# Source document
Type "SRS-2024-001"
Enter
Sleep 500ms

# Source revision
Type "A"
Enter
Sleep 500ms

# Source section
Type "3.2.1"
Enter
Sleep 500ms

# Text - the main requirement
Type "The motor shall operate continuously within the temperature range of -20°C to +85°C ambient."
Enter
Sleep 500ms

# Rationale
Type "Required for automotive applications per ISO 16750-4"
Enter
Sleep 500ms

# Acceptance criteria
Type "Temperature chamber test from -25°C to +90°C for 4 hours"
Enter
Sleep 1s

# Wait for file creation
Sleep 2s

# Show the created requirement
Type "tdt req show REQ@1"
Enter
Sleep 4s
