# The formulAA AA format

This document is the format specification, written for someone who has
never seen this implementation and wants to build a parser (or renderer)
from scratch. It defines the grid model, the reserved glyph vocabulary,
and — construct by construct — what a picture must look like to mean
what it means. `src/render` and `src/parse.rs` implement the two sides
of this spec and must always change together.

Atoms (the ~500 ordinary symbols: `α`, `≤`, `∈`, `ℝ` …) are *not*
enumerated here; they live in the tables under `src/symbols/`. What this
spec fixes is the rule that governs them (§2).

## 0. The contract

A formula is simultaneously a picture and a syntax tree. For every tree
`x`:

```plain
parse(render(normalize(x))) == normalize(strip_spacers(normalize(x)))
```

`render(parse(aa)) == aa` is **not** required. The AA is source code:
the parser accepts more than the canonical form (lenient hand-written
input, §9), every accepted picture has exactly one reading, and `fmt`
rewrites any accepted picture into the canonical form. Two further
requirements:

- `normalize` is idempotent (§8).
- No rule may depend on *how many* spaces separate two things. Space
  carries meaning only by presence or absence.

## 1. The grid model

- A formula is a character grid; every character is exactly one cell
  wide (this is why the atom set is an allow-list, §2).
- Every subexpression owns a **rectangle plus a baseline row**. Siblings
  sit side by side in disjoint column ranges, with baselines aligned.
- Parsing is recursive descent over regions: given a rectangle and its
  baseline, scan the baseline left to right; each structural glyph
  claims a column range and dictates which sub-rectangles to recurse
  into (§5). Sub-regions either inherit the baseline (delimiter
  segments, radical contents, brace arguments) or re-derive it (§4).

## 2. Vocabulary: three glyph classes

1. **Atoms** — the ordinary symbols. An allow-list derived from the
   symbol tables: a character is an atom iff some `\name` produces it or
   it is directly typeable ASCII. Everything on the list is one cell
   wide and non-combining. ASCII `^ ~ \` and the backtick are excluded
   (they collide with LaTeX syntax; `\sim`, `\backslash` are the
   spellings); `# $ % &` are atoms that the LaTeX serializer escapes.
   Any character outside the list is a parse error — one double-width
   character would silently shear every column to its right.

2. **Structural glyphs** — reserved, never atoms, so their appearance is
   always a structural claim. The roles:

   | Glyphs | Role |
   | --- | --- |
   | `─` | fraction bar; also the body of stretchy arrows (`──>`) |
   | `═` | double-arrow body (`══>`) |
   | `┈` | the band: big-operator limits, wide-accent mark rows, and (alone on a line) the multi-line formula separator |
   | `╭ ╮ ╰ ╯` | overbrace / underbrace range rows |
   | `√ ∛ ∜ │ ┌` | radicals: root glyph, stem column, overline corner (`┌` + `─` run) |
   | `( ) [ ] { } ⟨ ⟩` + `⎛⎜⎝ ⎞⎟⎠ ⎡⎢⎣ ⎤⎥⎦ ⎧⎪⎨⎩ ⎫⎪⎬⎭ ╱ ╲` | delimiter columns (§5.4). `⌈ ⌉ ⌊ ⌋` one-line ceil/floor; `‖` the norm; `┆ ┊` the null delimiters (`\left.`/`\right.`); mid-height `│` the segment separator |
   | `┌┬┐ ├┼┤ └┴┘` | grid lattice markers (§5.5) |
   | `⬚` | the explicit empty: empty slots, the base of a leading script |
   | `' "` | roman/text quoting (§5.8) — the prime atom is `′` U+2032, a different character |
   | `_ ¯ ˜ ˷ ˰ ˯ ˳ ․ ￫` | accent drawing forms (§5.7) — `․` U+2024 and `￫` U+FFEB are distinct from the atoms `.` and `→` |
   | math-italic letters, `⁰¹²…`, `₀₁₂…` | the *rendered* faces of plain letters and inline scripts (§5.6) |

3. **Display-only characters** — the editor's view layer uses `▌` and
   the private-use block U+E000–F8FF for cursors and selection markers.
   They never appear in canonical AA and a parser may reject them.

`␣` U+2423 is an ordinary atom (a *semantic* space; LaTeX renders it as a control space). A real
space column is either a sibling separator or a formatting spacer that
vanishes on reparse.

## 3. Multi-line formulas

A line whose only glyph is a single `┈` splits the picture into
formulas joined by line breaks (LaTeX `\\`). Bands always sandwich
their material without spaces, so a lone `┈` has no other reading.
Blank lines adjacent to a separator are formatting; a blank line
splitting a formula *without* a separator is an error. Two consecutive
separators mean an empty line. There is no alignment (no `align`
column syncing).

## 4. Baseline recovery

When a region's baseline is not inherited, derive it from the **leftmost
column after trimming**:

1. If the column contains `─`, `┈`, or `═`, that row is the baseline
   (bars and bands sit on the baseline and stick out past their
   contents).
2. Strip accent marks growing down from the top and up from the bottom;
   what remains is the base's row.
3. Otherwise branch on the first glyph: brace/angle verticals with a
   vertex (`⎨`, `⟨`) → the vertex row; `╱` with `╲` directly below it →
   an angle fold, the upper row; paren/bracket/bar/null columns
   (`⎛ ⎡ ⎢ ┆`) → the extent's center row if the interior is a fused
   grid, else recurse into the interior; lattice edges (`┌ ├ └`) → the
   lattice extent's center row; `╭` → recurse below (the argument),
   `╰` → recurse above; `(` on one line → that row.
4. If exactly one non-blank row remains, that is the baseline. More
   than one → ambiguity error (the writer must use a form that shows
   its baseline).

## 5. The baseline scan

Scan the baseline left to right. Each construct below consumes its
column range and recurses into the rectangles it defines.

### 5.1 Blank baseline run → scripts

A run of blank baseline columns holds the *scripts of the element to
its left*: content above is superscript, below is subscript. Chunk the
upper and lower halves **independently** into column groups; columns
that are blank on both sides separate groups; columns protected by a
bracket pair (§7) never separate. Before treating a blank run as
scripts, look for two structures whose interiors the baseline happens
to pass through blankly: a lattice edge (`┌ ├ └`) whose extent centers
on this baseline (an inline bare grid), and a `╭`/`╰` above/below whose
argument side contains this baseline (an over/underbrace).

### 5.2 `───` → fraction; `──>` → arrow

Read the maximal `─` run. If a `>` is glued to its end (or a `<` to its
start), it is a labelled stretchy arrow: labels sit centered above and
below the body's column range. Otherwise it is a fraction: numerator
and denominator are the rectangles above and below the bar's columns
(centered; the bar is 2 wider than either, so the region's left column
holds only the bar). `═` runs are double arrows. Maximal munch: glued
means arrow, one space means "fraction (or bar) then an atom `>`".
Arrowheads are ASCII `<` `>` only; Unicode arrows are always atoms.

### 5.3 `┈material┈` → big operator with limits

Material sandwiched by `┈` with no spaces takes limits above and below,
centered within the band's columns: `┈∑┈`, `┈lim┈`, `┈argmax┈`. The
material is one piece (no spaces); a single ∑-class symbol is a symbol
operator, anything else is a named operator. The band is what makes
limit attachment unambiguous (without it, `∑` above `∫` cannot say
which owns which). A band with both limits empty never appears in
canonical AA — it normalizes to the bare atom or upright run (§8).
Bands read greedily, so canonical output always separates a band from
its neighbors with one space.

Floating *above or below* the baseline, a row of `┈` plus accent
material is a **wide accent** range row (§5.7).

### 5.4 Vertical delimiter columns

A delimiter pair is two columns of pair glyphs around one or more
segments, `│` full-height columns separating segments (the "mids", as
in `⟨ψ│H│ψ⟩` or `{x│x>0}`):

| Spec | One line | Multi-line column (top/extension/bottom) |
| --- | --- | --- |
| `( )` | `( )` | `⎛⎜⎝ ⎞⎟⎠` |
| `[ ]` | `[ ]` | `⎡⎢⎣ ⎤⎥⎦` |
| `{ }` | `{ }` | `⎧⎪⎨⎩ ⎫⎪⎬⎭` — the vertex `⎨ ⎬` is always the baseline; minimum height 3 |
| `⟨ ⟩` | `⟨ ⟩` | diagonal arms `╱ ╲` only, even height; the fold is a vertical pair in one column (left: `╱` above `╲`) and the upper row of the pair is the baseline |
| `⌈ ⌉ ⌊ ⌋` | those glyphs | bracket pieces with one corner dropped: ceil = `⎡`+`⎢` (no foot), floor = `⎢`+`⎣` (no head) |
| `\|` (abs) | `⎢ ⎥` | all-rows `⎢` / `⎥` — a cornerless extension column |
| `.` (null) | `┆ ┊` | all-rows `┆` (left) / `┊` (right) |

- Which bracket family a column belongs to is decided by **which
  corners the column run contains** (`⎡`+`⎣` = bracket, `⎡` alone =
  ceil, corner-free = absolute-value bar).
- Matching: walk the baseline counting depth; left and right glyphs
  are distinct characters, so mismatched pairs like `(0,1]` count
  correctly. The side-shared glyphs `⎪ ╱ ╲` are resolved by walking
  their column. The norm `‖` is the one pair whose two sides share a
  glyph: its side is resolved by parity among same-extent `‖` columns,
  it takes no mids, and a norm directly inside a norm is inexpressible
  (write the inner pair as `⎢ ⎥`).
- Unmatched closing atoms `) ] } ⟩` are a parse error; closing-bracket
  atoms cannot exist (they would desynchronize the depth count).

### 5.5 Grids

An `Array` is drawn the same way everywhere: box-drawing junction
markers at every separator intersection, outer border included —
`┌┬┐ ├┼┤ └┴┘`. A matrix is just a grid wrapped in a delimiter pair.
Cell boundaries are read from marker positions; whitespace inside or
around cells is meaningless in any quantity.

When a grid is a delimiter's sole segment it **fuses**: the delimiter
columns absorb the lattice edge and only minimal markers remain —
multi-row × multi-column: separator rows of spaces and `┼` only;
one row: a `┬`…`┴` marker row above/below; one column: `├ ┤` junctions
biting into the delimiter column (told from a bare lattice edge by the
`⎛/⎡`-family glyphs continuing in the same column). A "pure marker row"
is the detection condition — nested structure always drops other glyphs
into such a row, so there are no false positives. Braces and angles do
not fuse (no junction glyph coexists with the `⎨` vertex / the diagonal
arms); they wrap a bare lattice instead. A fused 1×1 is
indistinguishable from a plain cell, so it normalizes away. The corner
markers are what keep two adjacent lattices from merging into one.
The grid's baseline is the center row of its extent.

### 5.6 Letters, scripts, functions

- Plain letters render as math italics (`a` → `𝑎`); an upright ASCII
  run is therefore self-identifying. Per maximal run: a run of 2+
  letters is an upright name (`\operatorname`, or the function `\sin`
  when it is a dictionary word); a single letter is an italic variable,
  unless glued to a letter neighbor — then it is roman (the
  differential `d` in `d𝑦`). Dots may join a run (`i.i.d.`): an inner
  `.` joins when a letter follows; a trailing `.` joins when the run
  already contains one. Adjacent upright runs are separated by one
  space.
- Inline scripts use the dedicated Unicode script characters
  (`x²`, `aᵢ`) — distinct code points, hence unambiguous. Content that
  cannot be inlined becomes a 2D block above/below-right of its base
  (read by §5.1). A script at the start of a row gets an explicit `⬚`
  base.

### 5.7 Accents

Nothing but accent marks may occupy the cells directly above and below
a baseline token (anything else there is an error — content is never
silently dropped). So accents are read by walking straight up (and
down) from the base while marks continue, innermost first. The AST
keeps flat over/under lists — over/under nesting order cannot be drawn,
so it must not be representable.

Marks are drawn in *base-hugging* forms: bar `_` (above) / `¯` (below),
hat `˰`, tilde `˷` above / `˜` below, check `˯`, ring `˳`, dot `․`,
vec `￫`, ddot `․․` (the second dot overhangs one column to the right;
the overhang column has a blank baseline and holds nothing else).

**Wide accents** (multi-character bases, `\widehat{abc}`,
`\overline{z+1}`): the base stays bare on the baseline, and a band row
`┈` + mark material hugs it above (below), one column wider on each
side, discovered from the blank baseline column it overhangs.
Stretchable marks fill the width (`┈___┈` overline, `┈˷˷˷┈` widetilde);
point marks sit alone in the center (`┈┈˰┈┈` widehat, `┈┈￫┈┈` vec).
Fill glyphs are side-exclusive (`˷` above, `˜` below), so baseline
recovery knows which way to dive past the band. Bands stack for
repeated accents. A one-character base with a single mark normalizes to
the compact form. A wide accent always has one space on each side.

### 5.8 Quoting

- `"…"` is `\text` — real spaces allowed inside; `\"` and `\\` escape;
  bracket characters cannot appear inside (quotes are opaque and would
  break depth counting).
- `'…'` forces a roman run (`'d'` = a lone `\mathrm{d}`). `'` is
  *always* a quote delimiter: it must close, and the contents are ASCII
  alphanumerics/`␣`/`.`. The prime is the separate atom `′`.

### 5.9 Braces (overbrace / underbrace)

`╭──╮` directly above the argument block (`╰──╯` below), the label
beyond it, both 2 wider than max(argument, label) — the fraction rule
displaced off the baseline. Detected from the blank baseline run whose
columns carry the `╭`/`╰` (the argument must cover the caller's
baseline).

### 5.10 Radicals

Top row `┌` + a `─` run whose length is the argument's width; below it
the stem column (`│`, bottoming out in `√ ∛ ∜`) covering every argument
row. The argument's baseline is the outer baseline. The `┌` is told
from a lattice corner by the stem directly beneath it, and it does not
participate in bracket depth counting.

## 6. Recursion

Inherit the baseline into: delimiter segments, radical arguments, brace
arguments. Re-derive it (§4) for: fraction halves, band limits, arrow
and brace labels, grid cells, script chunks.

## 7. The two devices that keep readings unique

**Protection.** A column that lies inside a bracket pair (or lattice
border) on *any* row never acts as a separator — the interior blanks of
a nested matrix cannot split its parent.

**Separation duty.** The renderer must place one space between any two
neighbors that would fuse into a different token: bar runs of the same
glyph (`──`+`──`), a band and anything (both sides), `─`/`═` against an
arrowhead, upright runs, `√`'s greedy overline against a neighboring
bar, a wide accent block on both sides, dotted-run absorption cases.
The parser then simply reads greedily and stops at spaces. No other
whitespace is ever emitted: a full-height blank column means "sibling
separator" and nothing else.

## 8. Normalization

The parser returns only normal forms; the renderer assumes them:

- Merge adjacent same-kind scripts (`x^a^b` ≡ `x^{ab}` — same picture),
  across intervening spacers; re-normalize recursively (idempotence).
- A band with both limits empty unwraps to its bare material.
- Drop empty scripts and empty text; a markless accent is its base.
- Drop leading/trailing formatting spacers on each line (protects the
  `⬚` base of a leading script).
- A fused 1×1 grid inside a delimiter unwraps to its content (to a
  fixed point); a bare 1×1 lattice keeps its frame and survives.

## 9. Lenient input and hard errors

Accepted beyond canonical (and rewritten by `fmt`):

- Plain ASCII letters for italic variables (`x+1`); `*` for `∗`;
  math-italic code points read as their plain letters; tabs as one
  space; free whitespace between siblings.

Rejected loudly, never silently dropped:

- Content overlapping directly above/below a baseline token (other than
  consumed accent marks).
- Unmatched closing brackets.
- Characters outside the atom allow-list; combining overlays
  (U+0338/U+0336 — negation is written with precomposed atoms: ≠ ∉ ⊄).
- A formula split by a blank line without a `┈` separator; an ambiguous
  baseline.
