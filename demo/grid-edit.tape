# ^G grid edit: the plane rotation matrix becomes the rotation about
# the z axis. `c` and `r` switch to column and row lanes, where Enter
# on a gap inserts one and Backspace on a lane takes it away again.
# The subscript at the end is ^F: the free cursor crosses the whole
# picture in one go.
#   cargo build --release && vhs demo/grid-edit.tape
# (needs JuliaMono for the math italics and the lattice glyphs.)
Output demo/grid-edit.gif

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
Type "printf 'R(\\theta) = \\begin{pmatrix}\\cos\\theta & -\\sin\\theta \\\\ \\sin\\theta & \\cos\\theta\\end{pmatrix}' | formulaa --latex2aa > rotation.aa"
Enter
Type "clear"
Enter
Sleep 300ms
# The command is typed off-camera: the recording opens on it, so
# the first frame says what is about to run.
Type "formulaa rotation.aa"
Show

Sleep 900ms
Enter
Sleep 1200ms

# into a cell, then grid mode
Left
Left
Sleep 300ms
Ctrl+G
Sleep 800ms

# a third column: `c` walks the column gaps, Enter inserts one
Type "c"
Sleep 700ms
Right
Right
Sleep 700ms
Enter
Sleep 800ms

# and a third row
Type "r"
Sleep 500ms
Down
Down
Down
Sleep 700ms
Enter
Sleep 900ms

# one row too many — Backspace on a lane takes it back
Down
Enter
Sleep 900ms
Backspace
Sleep 900ms

# the new cells are plain editing again
Escape
Type "1"
Up
Type "0"
Up
Type "0"
Sleep 700ms

# the bottom row: the cell cursor is how you step sideways
Ctrl+G
Down
Down
Left
Sleep 500ms
Enter
Type "0"
Ctrl+G
Left
Sleep 500ms
Enter
Type "0"
Sleep 800ms

# it is the rotation about z now: ^F flies to the name and says so
Ctrl+F
Sleep 500ms
Up 2
Sleep 400ms
Left 8
Sleep 800ms
Enter
Type "_"
Type "z"
Tab
Sleep 1500ms

Ctrl+W
Sleep 500ms
Type "formulaa --aa2latex rotation.aa"
Enter
Sleep 1500ms
