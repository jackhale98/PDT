# TDT Recent Activity & Tags Demo
# Demonstrates: recent command, tags list, tags show

Output demos/recent-tags.gif
Output docs/recent-tags.gif
Set Theme "Dracula"
Set FontSize 16
Set Width 1000
Set Height 650
Set Padding 20

# Setup - use temp directory with some entities
Hide
Type "cd $(mktemp -d) && tdt init -q"
Enter
Sleep 300ms
Type "tdt req new --title 'Temperature Range: -20°C to 85°C' -t input --no-edit -q"
Enter
Sleep 300ms
Type "tdt req new --title 'Power Budget: 500mW max' -t input --no-edit -q"
Enter
Sleep 300ms
Type "tdt risk new --title 'Motor Overheating Risk' -t design --no-edit -q"
Enter
Sleep 300ms
Type "tdt cmp new -p 'TH-001' --title 'Aluminum Heatsink' --no-edit -q"
Enter
Sleep 300ms
# Add tags via bulk command
Type "tdt bulk add-tag thermal REQ@1 RISK@1 CMP@1"
Enter
Sleep 300ms
Type "tdt bulk add-tag critical REQ@1"
Enter
Sleep 300ms
Type "tdt bulk add-tag electrical REQ@2"
Enter
Sleep 300ms
Show

# Title
Type "# Recent Activity & Tags"
Enter
Sleep 1s

# Show recent activity
Type "tdt recent"
Enter
Sleep 3s

# Show recent with type filter
Type "tdt recent -t req -n 5"
Enter
Sleep 2s

# List all tags
Type "tdt tags list"
Enter
Sleep 3s

# Show entities with specific tag
Type "tdt tags show thermal"
Enter
Sleep 3s
