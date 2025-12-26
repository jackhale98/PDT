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
Type "cd $(mktemp -d) && tdt init -q && tdt config set author 'John Smith'"
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

# Wizard prompts in order: type, title, priority, category, tags, revision, text, rationale, acceptance_criteria

# 1. Type selection (enum - input/output)
Type "input"
Enter
Sleep 500ms

# 2. Title
Type "Motor Operating Temperature Range"
Enter
Sleep 500ms

# 3. Priority (enum - use arrow to select 'high')
Down
Enter
Sleep 500ms

# 4. Category
Type "thermal"
Enter
Sleep 500ms

# 5. Tags
Type "thermal,critical,motor"
Enter
Sleep 500ms

# 6. Revision (integer, default 1 - just press Enter)
Enter
Sleep 500ms

# 7. Text (requirement statement)
Type "The motor shall operate continuously within -20C to +85C ambient temperature."
Enter
Sleep 500ms

# 8. Rationale
Type "Required for automotive applications per ISO 16750-4"
Enter
Sleep 500ms

# 9. Acceptance criteria (can enter multiple, empty to finish)
Type "Temperature chamber test from -25C to +90C for 4 hours"
Enter
Sleep 500ms

# Empty entry to finish acceptance criteria
Enter
Sleep 1s

# Wait for file creation
Sleep 2s

# Show the created requirement
Type "tdt req show REQ@1"
Enter
Sleep 4s
