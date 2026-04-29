Output assets/demo-concept.gif

Set FontSize 14
Set Width 1200
Set Height 700
Set Theme "Dracula"
Set TypingSpeed 80ms
Set Shell "zsh"

Env PATH "/Users/gcurrie/dev/hamrs-ca/target/release:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin"

Sleep 800ms

Type "hamrs concept"
Enter
Sleep 700ms

# Section picker — pick section 7 (Propagation)
Type "7"
Sleep 300ms
Enter
Sleep 600ms

# Subsection picker — pick subsection 1
Type "1"
Sleep 300ms
Enter
Sleep 1200ms

# Pause so viewer can read the content
Sleep 5000ms

# Press n to return to subsection picker
Type "n"
Enter
Sleep 600ms

# Press b to go back to section picker
Type "b"
Enter
Sleep 600ms

# Press q to quit
Type "q"
Sleep 800ms
