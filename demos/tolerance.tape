# TDT Tolerance Analysis Demo
# Demonstrates: features, mates, stackup analysis

Output demos/tolerance.gif
Output docs/tolerance.gif
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
Type "# Tolerance Stackup Analysis"
Enter
Sleep 1s

# Create components
Type "tdt cmp new -p 'PN-001' --title 'Housing' --no-edit -q"
Enter
Sleep 1s

Type "tdt cmp new -p 'PN-002' --title 'Shaft' --no-edit -q"
Enter
Sleep 1s

# Create features with tolerances
Type "tdt feat new -c CMP@1 --title 'Bore ID' -t hole --nominal 10.0 --plus 0.025 --minus 0 --no-edit"
Enter
Sleep 2s

Type "tdt feat new -c CMP@2 --title 'Shaft OD' -t pin --nominal 9.95 --plus 0 --minus 0.015 --no-edit"
Enter
Sleep 2s

# Create a mate
Type "tdt mate new -a FEAT@1 -b FEAT@2 --title 'Shaft-Bore Fit'"
Enter
Sleep 3s

# Create tolerance stackup
Type "tdt tol new --title 'Clearance Analysis' --target-nominal 0.05 --target-upper 0.1 --target-lower 0.01 --no-edit"
Enter
Sleep 2s

# Add contributors
Type "tdt tol add TOL@1 FEAT@1 FEAT@2"
Enter
Sleep 2s

# Run analysis
Type "tdt tol analyze TOL@1"
Enter
Sleep 5s
