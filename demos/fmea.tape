# TDT FMEA Risk Management Demo
# Demonstrates: create risks, set FMEA ratings, view matrix

Output demos/fmea.gif
Output docs/fmea.gif
Set Theme "Dracula"
Set FontSize 16
Set Width 1000
Set Height 650
Set Padding 20

# Setup - use temp directory
Hide
Type "cd $(mktemp -d) && tdt init -q && tdt config set author 'John Smith'"
Enter
Sleep 500ms
Show

# Title
Type "# FMEA Risk Management"
Enter
Sleep 1s

# Create risks with FMEA ratings
Type "tdt risk new --title 'Motor Overheating' -t design --severity 8 --occurrence 4 --detection 3 --no-edit"
Enter
Sleep 2s

Type "tdt risk new --title 'Bearing Wear' -t design --severity 6 --occurrence 5 --detection 4 --no-edit"
Enter
Sleep 2s

Type "tdt risk new --title 'Power Supply Failure' -t design --severity 9 --occurrence 2 --detection 2 --no-edit"
Enter
Sleep 2s

# List risks sorted by RPN
Type "tdt risk list --by-rpn"
Enter
Sleep 3s

# Show risk matrix
Type "tdt risk matrix"
Enter
Sleep 4s

# Show summary
Type "tdt risk summary"
Enter
Sleep 4s
