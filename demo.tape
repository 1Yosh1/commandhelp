# VHS Recording Script for CommandHelp (`chelp`)
# Render crisp terminal demo GIF/video via: vhs demo.tape

Output demo.gif

Set Shell "bash"
Set FontSize 17
Set Width 1100
Set Height 550
Set WindowBar Colorful
Set Theme "Catppuccin Mocha"
Set Padding 20
Set TypingSpeed 45ms

# Scenario 1: Natural Language Query & Interactive Verification
Type "chelp query 'find all mp4 files larger than 50MB modified in last 7 days'"
Sleep 600ms
Enter

# Pause to showcase the interactive TUI card with safety verification
Sleep 2.5s

# Hit Tab / e to drop command directly onto prompt for review
Type "e"
Sleep 1.2s

# Clear line
Ctrl+C
Sleep 500ms

# Scenario 2: Zero-Latency Universal Dynamic Autocomplete
Type "docker run --det"
Sleep 500ms
Tab
Sleep 1.5s

Type " --nam"
Sleep 500ms
Tab
Sleep 2s
