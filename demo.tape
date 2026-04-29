Output assets/demo.gif

Set FontSize 14
Set Width 1200
Set Height 700
Set Theme "Dracula"
Set TypingSpeed 80ms
Set Shell "zsh"

Env PATH "./target/release:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin"

Sleep 800ms

# Launch quiz with 4 questions
Type "hamrs quiz -c 4"
Sleep 400ms
Enter
Sleep 700ms

# Section picker — pick section 5 (Electrical Principles)
Type "5"
Sleep 300ms
Enter
Sleep 1000ms

# Q1: jump to answer 2, confirm
Type "2"
Sleep 500ms
Enter
Sleep 1500ms
Enter
Sleep 700ms

# Q2: navigate down to answer 3, confirm
Down
Sleep 300ms
Down
Sleep 300ms
Down
Sleep 400ms
Enter
Sleep 1500ms
Enter
Sleep 700ms

# Q3: jump to answer 1, confirm
Type "1"
Sleep 500ms
Enter
Sleep 1500ms
Enter
Sleep 700ms

# Q4: navigate down once, confirm
Down
Sleep 500ms
Enter
Sleep 1500ms
Enter
Sleep 700ms

# Score screen — hold so it's readable
Sleep 3500ms

Type "q"
Sleep 800ms
