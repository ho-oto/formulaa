# formulAA

**Plain-text math that round-trips.** formulAA is a 2D formula format —
Unicode-first "ASCII art" that reads like typeset math — with one unusual
property: every picture parses back to exactly the syntax tree that drew
it. It ships with a WYSIWYG structure editor (TUI and browser) and a
converter to LaTeX.

```plain
          ┌────────
     -𝑏 ± √𝑏² - 4𝑎𝑐        ∞     1     π²         ⎡ cos θ   -sin θ ⎤
𝑥 = ────────────────     ┈┈∑┈┈ ──── = ────    𝑅 = ⎢        ┼       ⎥
            2𝑎            𝑛=1    𝑛²     6         ⎣ sin θ   cos θ  ⎦
```

Everything above is plain text. Pipe it into `formulaa aa2tex` and you get
compilable LaTeX; open it in the editor and you can keep editing it
structurally.

## Why another math-in-text tool?

Tools that *render* math as text art have existed for decades — pretty
printers in CAS systems, Unicode formula generators, diagram tools. They
all share the same dead end: **the output is a picture.** You cannot edit
it without redrawing it from the original source, and you cannot feed it
to anything else, because nothing can read it back.

formulAA turns the picture itself into the source code:

- **The AA is the single source of truth.** There is no hidden markup the
  picture was generated from. What you see in the file is what gets
  parsed, edited and converted.
- **Round-tripping is a tested contract, not a best effort.** For every
  syntax tree `x`, `parse(render(x)) == x` (up to normalization). The
  editor re-checks this after every keystroke and refuses any edit whose
  picture would stop parsing.
- **Re-editing is first-class.** Paste a formula from a month-old design
  doc into the editor, move a factor out of a square root with the
  arrow keys, paste it back. No LaTeX source to dig up.

In other words: like source code, the format has a grammar, a canonical
style, and a formatter (`formulaa fmt`) that normalizes anything it
accepts. Hand-typed input can be sloppy (`x+1`, `E=m c²`); the canonical
form is what the tools emit.

## The core idea: what makes a picture parseable

Free-form ASCII art is ambiguous — `a` above `b` could be a fraction, a
limit, or a coincidence of layout. formulAA's format is designed backwards
from the parser:

1. **Every subexpression owns a rectangle and a baseline row.** Siblings
   sit side by side in disjoint column ranges; vertical structure exists
   only inside a rectangle. Parsing is: find the baseline, scan it left
   to right, recurse into the rectangles that structural glyphs claim.

2. **Structure is drawn with reserved glyphs that can never be atoms.**
   The fraction bar `─`, the operator band `┈`, delimiter columns
   `⎛ ⎜ ⎝`, the radical `√` and its overline — these characters are
   banned from ordinary content, so their appearance is always a
   structural claim, never a coincidence. Conversely, atoms come from an
   allow-list (every character is exactly one cell wide), so a stray
   wide or combining character cannot silently shear the grid.

3. **Extent is spanned, never counted.** A fraction bar is wider than
   both its arguments; a `┈` band sandwiches the operator and spans its
   limits; a brace's vertex sits on the baseline row. No rule ever
   depends on *how many* spaces separate two things — whitespace only
   separates siblings, which is exactly what makes hand-editing
   survivable.

4. **One canonical spelling per tree.** The renderer's output is the
   normal form; the parser accepts a superset (lenient ASCII input,
   plain letters for math italics) and `fmt` closes the loop. Ambiguity
   that cannot be drawn cannot be represented: e.g. accents stack as
   flat lists, because the picture cannot distinguish
   `\hat{\underline{x}}` from `\underline{\hat{x}}`.

The result: a formula is a picture *and* a syntax tree at all times.

## Examples

All of these are taken from the round-trip test corpus
([more here](docs/examples.md)) — each parses back to its exact tree and
converts to the LaTeX shown.

Gaussian integral:

```plain
 ∞    -𝑥²      ┌─
┈∫┈┈ 𝑒    𝑑𝑥 = √π
 -∞
```

```latex
\int_{-\infty }^{\infty }e^{-x^{2}}dx=\sqrt{\pi }
```

Cauchy–Schwarz:

```plain
⎛  𝑛     _ ⎞    ⎛  𝑛      ⎞ ⎛  𝑛      ⎞
⎜┈┈∑┈┈ 𝑢ₖ𝑣ₖ⎟² ≤ ⎜┈┈∑┈┈ 𝑢ₖ²⎟ ⎜┈┈∑┈┈ 𝑣ₖ²⎟
⎝ 𝑘=1      ⎠    ⎝ 𝑘=1     ⎠ ⎝ 𝑘=1     ⎠
```

```latex
\left(\sum_{k=1}^{n}u_{k}\bar{v}_{k}\right)^{2}\le \left(\sum_{k=1}^{n}u_{k}^{2}\right)\left(\sum_{k=1}^{n}v_{k}^{2}\right)
```

A Vandermonde determinant — matrices are grids with explicit lattice
markers, so rows and columns are unambiguous even with empty cells:

```plain
⎡ 1   𝑥₁   𝑥₁²   ⋯   𝑥₁ⁿ⁻¹ ⎤
⎢   ┼    ┼     ┼   ┼       ⎥
⎢ 1   𝑥₂   𝑥₂²   ⋯   𝑥₂ⁿ⁻¹ ⎥
⎢   ┼    ┼     ┼   ┼       ⎥ = ┈┈┈┈∏┈┈┈┈ (𝑥ⱼ-𝑥ᵢ)
⎢ ⋮   ⋮     ⋮    ⋱     ⋮   ⎥    1≤𝑖<𝑗≤𝑛
⎢   ┼    ┼     ┼   ┼       ⎥
⎣ 1   𝑥ₙ   𝑥ₙ²   ⋯   𝑥ₙⁿ⁻¹ ⎦
```

## The editor

`cargo run` opens a WYSIWYG structure editor in the terminal: you
edit the tree, and the picture follows.

- Type naturally: letters become math italics, `//` makes a fraction,
  `^`/`_` open scripts, `(` `[` `{` auto-size, `\frac` `\sum` `\alpha`
  `\bbR` and friends via a `\` minibuffer with Tab completion (aliases
  included — `\leq`, `\rightarrow`, ASCII spellings like `\->` and
  `\oo` all work).
- Move *through* structure with the arrow keys; `↑`/`↓` enter limits and
  promote a bare `∑`/`lim` to its band form. Select with `Shift+←/→`,
  wrap the selection in `\frac`, `\sqrt`, parentheses, a norm…
- Free 2D cursor (`^F`), block selection (`^B`), grid editing for
  matrices (`^G`), undo/redo, mouse support.
- After every edit the editor re-parses its own output; an edit that
  would break the round trip is refused on the spot (run with `--debug`
  to capture a report in `formulaa_debug/` instead).

Key reference: [docs/keys.md](docs/keys.md) · command reference:
[docs/commands.md](docs/commands.md).

The same editor core compiles to WebAssembly; prototype integrations for
VS Code and Obsidian live in their own repositories
([formulaa-vscode](https://github.com/ho-oto/formulaa-vscode),
[formulaa-obsidian](https://github.com/ho-oto/formulaa-obsidian)); Zed is
CLI-task based (see the roadmap in [docs/adr.md](docs/adr.md)).

## CLI

```sh
formulaa                       # the TUI editor (^Y copies the AA;
                             #   --print writes it to stdout on exit)
formulaa aa2tex formula.txt    # AA → LaTeX (stdin works too)
formulaa tex2aa formula.tex    # LaTeX → AA, best effort (KaTeX/MathJax dialect)
formulaa fmt    formula.txt    # normalize hand-written AA to canonical form
```

LaTeX round-trips: everything `aa2tex` emits reads back to the exact
same tree, and `\tex` in the editor opens a box you can paste LaTeX
into (unknown commands are skipped, never an error).

## For AI agents

Because the format is plain text with a strict grammar, language models
can read and write it directly. [`SKILL.md`](SKILL.md) is a
self-contained guide that teaches an agent the format, including the
verification loop (`formulaa fmt` to check, `formulaa aa2tex` to confirm the
meaning) — useful for embedding re-editable math in Markdown documents
that both humans and agents maintain.

## Status

Working and tested (a corpus of real formulas plus randomized
property tests over trees and key sequences), but young — the AA
format may still evolve. LaTeX conversion runs in both directions;
MathML output is on the roadmap ([docs/adr.md](docs/adr.md)).

MIT licensed.

<!-- TODO before release: add the install line once published to crates.io. -->
