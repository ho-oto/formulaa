# ^F free move: three mistakes in a normal distribution, in three
# different corners of the picture. The free cursor goes straight at
# them — arrows over the *picture*, Enter to land on the nearest edit
# position — instead of walking the tree.
#   cargo build --release && vhs demo/free-move.tape
# (needs JuliaMono for the math italics and the radical.)
Output demo/free-move.gif

Set Shell "bash"
Set FontFamily "JuliaMono"
Set FontSize 22
Set Width 1100
Set Height 460
Set Padding 20
Set TypingSpeed 70ms

Hide
Type "export PATH=$PWD/target/release:$PATH"
Enter
Type "cd $(mktemp -d)"
Enter
Type "printf 'f(x) = \\frac{2}{\\sigma\\sqrt{2\\pi}} e^{\\frac{-(x+\\mu)^2}{2\\sigma}}' | formulaa --latex2aa > gaussian.aa"
Enter
Type "clear"
Enter
Sleep 300ms
# The command is typed off-camera: the recording opens on it, so the
# first frame says what is about to run without spelling it out.
Type "formulaa gaussian.aa"
Show

Sleep 900ms
Enter
Sleep 1200ms

# 1. the sign in the exponent's numerator
Ctrl+F
Sleep 500ms
Up 3
Sleep 400ms
Left 5
Sleep 600ms
Enter
Sleep 400ms
Ctrl+D
Type "-"
Sleep 800ms

# 2. the numerator of the leading fraction, two rows down
Ctrl+F
Sleep 400ms
Down 2
Sleep 400ms
Left 10
Sleep 600ms
Enter
Sleep 400ms
Ctrl+D
Type "1"
Sleep 800ms

# 3. σ is squared in the exponent's denominator
Ctrl+F
Sleep 400ms
Right 10
Sleep 600ms
Enter
Sleep 400ms
Type "^"
Type "2"
Tab
Sleep 1500ms
