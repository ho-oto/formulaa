# ^B + ^F together on the DPO loss, where the same policy ratio shows
# up four times. ^B takes a structure whole (the ⟨y|x⟩ pair, then the
# entire fraction) and ^F flies to the two letters that have to differ.
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

# the argument pair in the numerator: ^F lands in it, ^B takes it whole
Ctrl+F
Sleep 400ms
Up 2
Left 7
Sleep 800ms
Enter
Sleep 400ms
Ctrl+B
Sleep 900ms
Enter
Ctrl+C
Sleep 700ms

# the denominator is the reference policy with the same argument
Ctrl+F
Sleep 300ms
Down 3
Sleep 700ms
Enter
Sleep 400ms
Type "\pi"
Enter
Type "_"
Type "\rmref"
Enter
Tab
Sleep 400ms
Ctrl+V
Sleep 1200ms

# now the whole ratio, again, after a minus
Ctrl+B
Sleep 900ms
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
