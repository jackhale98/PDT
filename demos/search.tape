# TDT Global Search Demo
# Demonstrates: search across entities, filters, output formats

Output demos/search.gif
Output docs/search.gif
Set Theme "Dracula"
Set FontSize 16
Set Width 1000
Set Height 600
Set Padding 20

# Setup - use temp directory with entities
Hide
Type "cd $(mktemp -d) && tdt init -q"
Enter
Sleep 300ms
Type "tdt req new --title 'Thermal Management System' -t input --no-edit -q"
Enter
Sleep 300ms
Type "tdt req new --title 'Motor Thermal Protection' -t output --no-edit -q"
Enter
Sleep 300ms
Type "tdt risk new --title 'Thermal Runaway Risk' -t design --no-edit -q"
Enter
Sleep 300ms
Type "tdt test new --title 'Thermal Cycling Test' -t verification --no-edit -q"
Enter
Sleep 300ms
Type "tdt cmp new -p 'HS-001' --title 'Thermal Interface Material' --no-edit -q"
Enter
Sleep 300ms
Show

# Title
Type "# Global Search"
Enter
Sleep 1s

# Basic search
Type "tdt search thermal"
Enter
Sleep 3s

# Filter by type
Type "tdt search thermal -t req,risk"
Enter
Sleep 2s

# Output as JSON
Type "tdt search thermal -f json"
Enter
Sleep 3s

# Count only
Type "tdt search thermal --count"
Enter
Sleep 2s
