Output assets/demo-morse.gif

Set FontSize 14
Set Width 1000
Set Height 700
Set Theme "Dracula"
Set TypingSpeed 80ms
Set Shell "zsh"

Env PATH "./target/release:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin"

Sleep 800ms

Type "hamrs morse"
Enter
Sleep 700ms

# Mode: Receive (1)
Type "1"
Sleep 300ms
Enter
Sleep 500ms

# WPM: 5 WPM — slow enough that the animation is clearly visible in the GIF
Type "1"
Sleep 300ms
Enter
Sleep 500ms

# Characters: Letters only
Type "1"
Sleep 300ms
Enter
Sleep 500ms

# 4 items
Type "4"
Sleep 300ms
Enter
Sleep 900ms

# TUI launches — first item animates.
# At 5 WPM: dit=240ms, dah=720ms. Longest realistic char (~6 elements) ≈ 3s.
Sleep 3500ms

# Answer first item — type a letter, hit Enter
Type "n"
Sleep 400ms
Enter
Sleep 1500ms

# Advance
Enter
Sleep 3500ms

# Answer second item
Type "t"
Sleep 400ms
Enter
Sleep 1500ms
Enter
Sleep 3500ms

# Third item — press r to replay, then answer
Type "r"
Sleep 3500ms
Type "s"
Sleep 400ms
Enter
Sleep 1500ms
Enter
Sleep 3500ms

# Fourth item
Type "e"
Sleep 400ms
Enter
Sleep 1500ms
Enter

# Score screen
Sleep 3000ms

Type "q"
Sleep 800ms
