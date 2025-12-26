# TDT Quick Start Demo
# Demonstrates: init, create requirements, list, validate, status

Output demos/quickstart.gif
Output docs/quickstart.gif
Set Theme "Dracula"
Set FontSize 16
Set Width 1000
Set Height 600
Set Padding 20

# Setup - use temp directory
Hide
Type "cd $(mktemp -d)"
Enter
Sleep 500ms
Show

# Title
Type "# TDT Quick Start"
Enter
Sleep 1s

# Initialize project
Type "tdt init -q"
Enter
Sleep 1s

# Set author
Type "tdt config set author 'John Smith'"
Enter
Sleep 500ms

# Create requirements with proper engineering titles
Type "tdt req new --title 'Operating Temperature: -20°C to 85°C' -t input --priority high --no-edit -q"
Enter
Sleep 1.5s

Type "tdt req new --title 'Maximum Power Consumption: 500mW' -t input --priority critical --no-edit -q"
Enter
Sleep 1.5s

Type "tdt req new --title 'Minimum Operating Lifetime: 10,000 hours' -t input --no-edit -q"
Enter
Sleep 1.5s

# List requirements
Type "tdt req list"
Enter
Sleep 3s

# Validate project
Type "tdt validate"
Enter
Sleep 2s

# Show project status
Type "tdt status"
Enter
Sleep 4s
