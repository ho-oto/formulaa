# ^B + ^F together on the DPO loss, where the same policy ratio shows
# up four times. ^B takes a structure whole — the numerator, then the
# entire fraction — and ^F flies to the letters that have to differ.
#   cargo build --release && vhs demo/dpo.tape
# (needs JuliaMono for the math italics and the tall delimiters.)
Output demo/dpo.gif

Set Shell "bash"
Set FontFamily "JuliaMono"
Set FontSize 20
Set Width 1300
Set Height 460
Set Padding 20
Set TypingSpeed 70ms

Hide
Type "export PATH=$PWD/target/release:$PATH"
Enter
Type "cd $(mktemp -d)"
Enter
Type "printf '\\mathcal{L}(\\pi_\\theta) = -\\mathbb{E}_{(x, y_w, y_l) \\sim \\mathcal{D}} \\left[ \\log \\sigma \\left( \\beta \\log \\frac{\\pi_\\theta(y_w \\mid x)}{} \\right) \\right]' | formulaa --latex2aa > dpo.aa"
Enter
Type "clear"
Enter
Sleep 300ms
# The command is typed off-camera: the recording opens on it.
Type "formulaa dpo.aa"
Show

Sleep 900ms
Enter
Sleep 1500ms

# ^F to the numerator, ^B to take it whole: the slot the cursor stands
# in is the first step of the ring.
Ctrl+F
Sleep 400ms
Up 2
Sleep 800ms
Enter
Sleep 400ms
Ctrl+B
Sleep 900ms
Enter
Ctrl+C
Sleep 800ms

# the denominator is the same ratio with one letter changed
Ctrl+F
Sleep 300ms
Down 3
Sleep 700ms
Enter
Sleep 300ms
Ctrl+V
Sleep 1200ms
Ctrl+F
Sleep 300ms
Down
Left 8
Sleep 800ms
Enter
Sleep 300ms
Ctrl+D
Type "\rmref"
Enter
Sleep 1400ms

# now the whole ratio, again, after a minus
Tab
Ctrl+B
Sleep 700ms
Up
Sleep 800ms
Enter
Ctrl+C
Sleep 500ms
Right
Type "-"
Type "\beta"
Enter
Type "\log"
Enter
Sleep 300ms
Ctrl+V
Sleep 1500ms

# the second ratio is the rejected answer: two letters to change
Ctrl+F
Sleep 300ms
Up
Left 6
Sleep 700ms
Enter
Ctrl+D
Type "l"
Sleep 900ms
Ctrl+F
Sleep 300ms
Down 3
Sleep 700ms
Enter
Ctrl+D
Type "l"
Sleep 2s
