# TDT Traceability Demo
# Demonstrates: linking entities, tracing, coverage

Output demos/traceability.gif
Output docs/traceability.gif
Set Theme "Dracula"
Set FontSize 16
Set Width 1000
Set Height 650
Set Padding 20

# Setup - use temp directory
Hide
Type "cd $(mktemp -d) && tdt init -q"
Enter
Sleep 500ms
Show

# Title
Type "# Requirements Traceability"
Enter
Sleep 1s

# Create requirement
Type "tdt req new --title 'System shall operate at -20°C to 85°C' -t input --no-edit -q"
Enter
Sleep 1s

# Create test
Type "tdt test new --title 'Temperature Chamber Test' -t verification -m test --no-edit -q"
Enter
Sleep 1s

# Create component
Type "tdt cmp new -p 'PN-001' --title 'Thermal Controller' --no-edit -q"
Enter
Sleep 1s

# Link requirement to test
Type "tdt link add REQ@1 --type verified_by TEST@1"
Enter
Sleep 2s

# Link requirement to component
Type "tdt link add REQ@1 --type allocated_to CMP@1"
Enter
Sleep 2s

# Trace from requirement
Type "tdt trace from REQ@1"
Enter
Sleep 3s

# Check for orphans
Type "tdt trace orphans"
Enter
Sleep 3s

# Show where component is used
Type "tdt where-used CMP@1"
Enter
Sleep 3s
