Output assets/demo-drill.gif

Set FontSize 14
Set Width 1200
Set Height 700
Set Theme "Dracula"
Set TypingSpeed 80ms
Set Shell "zsh"

Env PATH "./target/release:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin"

Sleep 800ms

Type "hamrs drill"
Enter
Sleep 900ms

# Category picker — select Q Codes
Type "q"
Sleep 400ms
Enter
Sleep 900ms

# Card 1: prompt visible — pause so viewer can read it
Sleep 1500ms

# Reveal answer
Space
Sleep 2000ms

# Got it
Type "y"
Sleep 700ms

# Card 2: prompt visible
Sleep 1500ms

# Reveal
Space
Sleep 2000ms

# Missed
Type "n"
Sleep 700ms

# Card 3: prompt visible
Sleep 1500ms

# Reveal
Space
Sleep 2000ms

# Got it
Type "y"
Sleep 700ms

# Quit to summary early — shows missed card list
Type "q"
Sleep 3000ms

# Quit
Type "q"
Sleep 800ms
