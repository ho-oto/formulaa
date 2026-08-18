# The round trip in one take: type a formula, look at what landed on
# disk, convert it, then open it again and keep editing. The shell
# comments ride along with the commands.
#   cargo build --release && vhs demo/quadratic.tape
# (needs JuliaMono installed — it is the font that carries the math
# italics and joins the radical to its overline; fallbacks break both.)
Output demo/quadratic.gif

Set Shell "bash"
Set FontFamily "JuliaMono"
Set FontSize 22
Set Width 1250
Set Height 520
Set Padding 20
Set TypingSpeed 55ms

Hide
Type "export PATH=$PWD/target/release:$PATH"
Enter
Type "cd $(mktemp -d)"
Enter
Type "clear"
Enter
Sleep 300ms
Show

Type "formulaa quadratic.aa  # a WYSIWYG math editor, in the terminal"
Sleep 400ms
Enter
Sleep 900ms

# fraction + root + inline script
Type "x="
Sleep 200ms
Type "//"
Sleep 300ms
Type "-b"
Type "\pm"
Sleep 300ms
Enter
Sleep 200ms
# a half-typed name: Tab shows what it could be
Type "\sq"
Sleep 300ms
Tab
Sleep 700ms
Enter
Sleep 200ms
Type "b^2"
Sleep 200ms
Tab
Type "-4ac"
Sleep 300ms
Tab
Sleep 300ms
Down
Sleep 200ms
Type "2a"
Sleep 900ms

# ^W saves and leaves
Ctrl+W
Sleep 600ms
Type "cat quadratic.aa  # it saved plain text: the picture IS the file"
Sleep 300ms
Enter
Sleep 1200ms
Type "formulaa --aa2latex quadratic.aa  # every picture parses back"
Sleep 300ms
Enter
Sleep 1500ms

# and back in: the file is a tree again, so the other root is two keys
Type "formulaa quadratic.aa  # …so it reopens as structure, not as art"
Sleep 300ms
Enter
Sleep 1200ms
Left
Up
Sleep 600ms
Ctrl+D
Type "-"
Sleep 1200ms
Ctrl+W
Sleep 500ms
Type "cat quadratic.aa"
Enter
Sleep 1800ms
