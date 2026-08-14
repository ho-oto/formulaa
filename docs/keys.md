# Keys

The TUI (`formulAA`) is a WYSIWYG structure editor: the cursor is always
an edit position inside the formula tree. The same keymap runs
in the wasm-based editor extensions. `\commands` (including completion
and the mode spellings for stolen ctrl chords) are in
[commands.md](commands.md).

## Typing

| Key | Action |
| --- | --- |
| letters, digits, symbols | insert atoms (letters become math italics) |
| `^` `_` | enter a super/subscript (inlined as `x²` when possible) |
| `(` `[` `{` | enter an auto-sizing pair (wraps the selection) |
| `)` `]` `}` | close the pair and step out (`]` also exits a matrix) |
| `//` | fraction (type `/` twice) |
| `Space` | formatting space (not in LaTeX output; `\space` is the semantic `␣`) |
| `"` | text mode: type prose, close with `"` (`\text`) |
| `'` | the prime atom ′ |
| `\` | command minibuffer — see [commands.md](commands.md) |

## Moving

| Key | Action |
| --- | --- |
| `←` `→` | move through structure |
| `↑` `↓` | numerator ⇄ denominator, limits, matrix rows; multi-line rows otherwise |
| `Tab` | leave the current inset |
| `Home` `End` | start / end of the current line |
| `Ctrl+A` `Ctrl+E` | start / end of the formula |
| mouse click | nearest edit position |
| `Enter` (top level) | formula line break |

## Selecting

| Key | Action |
| --- | --- |
| `Shift+←` `Shift+→` | grow / shrink the selection |
| `Shift+↑` | select the parent block |
| `Ctrl+B` | block-select mode: walk the ancestor chain with the arrows — outward it ends on the whole formula — `Enter` selects |
| `Ctrl+C` / `Ctrl+X` / `Ctrl+V` | copy / cut / paste (the internal buffer — the system clipboard is `Ctrl+Y`) |
| structure keys, with a selection | containers (`(`, `\frac`, `\sqrt`, `\norm`, `\abs`, `\lr…`) wrap it; content inserts (symbols, functions, matrices) replace it |

## Deleting

| Key | Action |
| --- | --- |
| `Backspace` / `Delete` (`Ctrl+D`) | delete left / right |
| …with a selection | delete the selection |
| …behind an accented atom | peels the outermost mark first (the inverse of typing); the bare atom deletes last |
| …touching a non-empty structure from outside | first press selects it whole, second deletes it (entering to edit is what the arrows are for) |
| …just inside a bracket, `\sqrt`, `\norm`, or a wide accent | first press lights it up (arms it), second lifts the contents out, third deletes them |
| `Shift+←/→` touching just the bracket | arms the same way — the next delete unwraps without selecting anything |
| `Shift+←/→` touching a `│` middle (from inside a segment edge) | arms that one separator; the next delete removes it, merging its two segments |

Pairs with `│` mids and brackets whose sole content is a matrix don't
unwrap (there is no single "contents"); empty pairs delete in one
press.

## Undo, output, quitting

| Key | Action |
| --- | --- |
| `Ctrl+Z` / `Ctrl+R` | undo / redo |
| `Ctrl+Y` | copy the canonical AA to the system clipboard |
| `Esc` | dismiss mode → clear selection → quit |

## Free cursor (`Ctrl+F`)

Move anywhere on the grid with the arrows; the nearest edit position is
shown as a snap target. `Enter` lands there; `Esc` cancels.

## Grid edit (`Ctrl+G`, inside a matrix)

The edited matrix's frame turns green.

| Key | Action |
| --- | --- |
| arrows | move the cell cursor |
| `Shift`+arrows | select a cell rectangle; pushing past a full axis promotes to a row/column lane |
| `Ctrl+C/X/V` | copy / cut / paste the cell rectangle (paste overwrites, growing the grid as needed) |
| `Backspace` / `Delete` | clear the selected cells |
| `c` / `\|`, `r` / `-` | column / row lane mode: walk gaps and lanes, `Enter` on a gap inserts a lane, `Backspace` on a lane deletes it |
| `Enter` | leave the mode with the cell's contents selected |
| `Esc` / `Tab` / `\` / `Ctrl+G` | leave the mode |
