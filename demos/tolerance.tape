# TDT Tolerance Stackup Analysis Demo
# Demonstrates: features, mates, stackup analysis

Output demos/tolerance.gif
Output docs/tolerance.gif
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
Type "# Tolerance Stackup Analysis"
Enter
Sleep 1s

# Create components
Type "tdt cmp new -p 'HSG-001' --title 'Housing' --no-edit -q"
Enter
Sleep 1s

Type "tdt cmp new -p 'SFT-001' --title 'Shaft' --no-edit -q"
Enter
Sleep 1s

# Create features (uses default dimensions 10.0 +0.1/-0.05)
Type "tdt feat new -c CMP@1 --title 'Bore ID' -t internal --no-edit -q"
Enter
Sleep 1.5s

Type "tdt feat new -c CMP@2 --title 'Shaft OD' -t external --no-edit -q"
Enter
Sleep 1.5s

# Create mate between features
Type "tdt mate new -a FEAT@1 -b FEAT@2 --title 'Shaft-Bore Clearance Fit' --no-edit"
Enter
Sleep 2s

# Create tolerance stackup
Type "tdt tol new --title 'Clearance Analysis' --target-nominal 0.05 --target-upper 0.15 --target-lower 0.01 --no-edit"
Enter
Sleep 2s

# Add features to stackup
Type "tdt tol add TOL@1 FEAT@1 FEAT@2"
Enter
Sleep 2s

# Run full analysis (worst-case, RSS, Monte Carlo)
Type "tdt tol analyze TOL@1"
Enter
Sleep 5s
