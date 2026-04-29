Output assets/demo-concept.gif

Set FontSize 13
Set Width 1200
Set Height 1300
Set Theme "Dracula"
Set TypingSpeed 80ms
Set Shell "zsh"

Env PATH "/Users/gcurrie/dev/hamrs-ca/target/release:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin"

Sleep 800ms

Type "hamrs concept"
Enter
Sleep 700ms

# Section picker — pick section 1 (Regulations & Licensing)
Type "1"
Sleep 300ms
Enter
Sleep 600ms

# Subsection picker — pick subsection 18 (Repeaters and Special Transmission Types)
# Short content (24 lines) + only 3 exam questions — fits on one screen
Type "18"
Sleep 300ms
Enter
Sleep 1200ms

# Pause so viewer can read the narrative and questions
Sleep 3500ms

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
