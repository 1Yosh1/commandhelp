# VHS Recording Script for CommandHelp (`chelp`)
# Render crisp terminal demo GIF/video via: vhs demo.tape

Output demo.gif

Set Shell "powershell"
Set FontSize 16
Set Width 1100
Set Height 620
Set WindowBar Colorful
Set Theme "Catppuccin Mocha"
Set Padding 20
Set TypingSpeed 45ms

# Scene 1: Version check
Type ".\target\release\chelp.exe --version"
Sleep 400ms
Enter
Sleep 1.2s

# Scene 2: Index workspace targets (Makefiles, Justfiles, scripts)
Type ".\target\release\chelp.exe index-repo"
Sleep 500ms
Enter
Sleep 2s

# Scene 3: Team Runbook Recipes with Safety Ratings
Type ".\target\release\chelp.exe recipe list"
Sleep 600ms
Enter
Sleep 3.5s

# Scene 4: Instant Shell Complete for Discovered Targets
Type ".\target\release\chelp.exe complete 'make '"
Sleep 600ms
Enter
Sleep 2.5s

# Scene 5: Fast Universal Flag Autocompletion
Type ".\target\release\chelp.exe complete 'docker run --'"
Sleep 600ms
Enter
Sleep 3s
