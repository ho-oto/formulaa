# ^F free move, the wide case: a Vandermonde determinant with three
# wrong entries in three different corners. The free cursor flies over
# the picture — top-left, bottom-left, then out to the product's
# condition — where walking the tree would be a long trip each time.
#   cargo build --release && vhs demo/free-move-matrix.tape
# (needs JuliaMono for the math italics and the lattice glyphs.)
Output demo/free-move-matrix.gif

Set Shell "bash"
Set FontFamily "JuliaMono"
Set FontSize 20
Set Width 1250
Set Height 460
Set Padding 20
Set TypingSpeed 70ms

Hide
Type "export PATH=$PWD/target/release:$PATH"
Enter
Type "cd $(mktemp -d)"
Enter
Type "printf '\\begin{bmatrix} 2 & x_1 & x_1^2 & \\cdots & x_1^{n-1} \\\\ 1 & x_2 & x_2^2 & \\cdots & x_2^{n-1} \\\\ \\vdots & \\vdots & \\vdots & \\ddots & \\vdots \\\\ 0 & x_n & x_n^2 & \\cdots & x_n^{n-1} \\end{bmatrix} = \\prod_{1\\le i<j\\le m}(x_j-x_i)' | formulaa --latex2aa > vandermonde.aa"
Enter
Type "clear"
Enter
Sleep 300ms
# The command is typed off-camera: the recording opens on it.
Type "formulaa vandermonde.aa"
Show

Sleep 900ms
Enter
Sleep 1500ms

# 1. the top-left entry should be 1
Ctrl+F
Sleep 400ms
Up 3
Sleep 300ms
Left 45
Sleep 800ms
Enter
Ctrl+D
Type "1"
Sleep 1000ms

# 2. straight down to the bottom-left entry
Ctrl+F
Sleep 300ms
Down 6
Sleep 800ms
Enter
Backspace
Type "1"
Sleep 1000ms

# 3. and out to the product's condition, far right
Ctrl+F
Sleep 300ms
Up 2
Sleep 300ms
Right 35
Sleep 800ms
Enter
Backspace
Type "n"
Sleep 2s
