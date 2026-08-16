# ^B block select: the Ising Hamiltonian's field term is the coupling
# term's sum again, with another index. From inside the ⟨i,j⟩ pair the
# ring walks out — the pair, the whole ∑ with its limit, the line —
# and the clipboard takes the structure whole.
#   cargo build --release && vhs demo/block-select.tape
# (needs JuliaMono for the math italics and the ┈ band.)
Output demo/block-select.gif

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
Type "printf 'H = -J \\sum_{\\left\\langle i,j \\right\\rangle} \\sigma_i \\sigma_j' | formulaa --latex2aa > ising.aa"
Enter
Type "clear"
Enter
Sleep 300ms
# The command is typed off-camera: the recording opens on it, so
# the first frame says what is about to run.
Type "formulaa ising.aa"
Show

Sleep 900ms
Enter
Sleep 1200ms

# down into the sum's limit, inside the ⟨i,j⟩ pair
Ctrl+A
Right 6
Sleep 700ms

# the ring: the pair, the sum, the whole line, and back to the sum
Ctrl+B
Sleep 900ms
Up
Sleep 900ms
Up
Sleep 900ms
Down
Sleep 700ms
Enter
Sleep 500ms
Ctrl+C
Sleep 500ms

# the field term is the same sum with a plain index
Ctrl+E
Type "-h"
Sleep 300ms
Ctrl+V
Sleep 1200ms
Left
Left
Sleep 400ms
# Backspace arms the ⟨ ⟩ pair, then lifts it — the index replaces it
Backspace
Sleep 500ms
Backspace
Sleep 500ms
Type "i"
Tab
Sleep 500ms
Type "\sigma"
Enter
Type "_"
Type "i"
Tab
Sleep 1500ms

Ctrl+W
Sleep 500ms
Type "cat ising.aa"
Enter
Sleep 1800ms
