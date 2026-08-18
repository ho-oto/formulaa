# formulAA

[![ci](https://github.com/ho-oto/formulaa/actions/workflows/ci.yml/badge.svg)](https://github.com/ho-oto/formulaa/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/formulaa.svg)](https://crates.io/crates/formulaa)
[![docs.rs](https://docs.rs/formulaa/badge.svg)](https://docs.rs/formulaa)

**Math as plain text you can actually edit.** Three pieces:

- a **2D text format** for formulas — Unicode "ASCII art" with a formal
  grammar ([docs/aa-spec.md](docs/aa-spec.md)): every picture has exactly
  one reading and parses back to the syntax tree it was rendered from;
- a **WYSIWYG structure editor** in the terminal for writing it;
- **converters** to and from LaTeX, and a formatter.

![Typing the quadratic formula in the editor, saving it, converting it to LaTeX, and reopening it to change a sign](demo/quadratic.gif)

```sh
$ formulaa gauss.aa              # edit it in the terminal; ^W writes and quits
$ cat gauss.aa                   # the file is just the picture
 ∞    -𝑥²   ┌─
┈∫┈┈ 𝑒   𝑑𝑥=√π
 -∞
$ formulaa --aa2latex gauss.aa   # the picture converts to LaTeX
\int_{-\infty }^{\infty }e^{-x^{2}}dx=\sqrt{\pi }
$ formulaa --aa2latex gauss.aa | formulaa --latex2aa   # and converts back
 ∞    -𝑥²   ┌─
┈∫┈┈ 𝑒   𝑑𝑥=√π
 -∞
```

There is no separate source file behind the picture: the AA text is
what gets stored, edited and converted. Put it anywhere plain text goes.

## Why a picture as the source

Terminals, git, Markdown and prompts all work on plain text, but math
fits it badly. LaTeX is machine-readable, but a human has to picture
`\frac{-b\pm\sqrt{b^2-4ac}}{2a}` in their head. ASCII art is readable at
a glance, but no program can interpret it.

Diagram tools take the drawing itself as the source and render it from
there — [ditaa](https://github.com/stathissideris/ditaa)
(2004), [aafigure](https://pypi.org/project/aafigure/),
[ASCIIToSVG](https://github.com/dhobsd/asciitosvg),
[Markdeep](https://casual-effects.com/markdeep/) (2015),
[svgbob](https://github.com/ivanceras/svgbob),
[GoAT](https://github.com/blampe/goat). formulAA does the same for
math, with LaTeX as the output instead of SVG.

## One picture, one reading

Those tools interpret free-form drawings as best they can. That works
for boxes and arrows, but not for math: `a` above `b` could be a
fraction, a limit, or two unrelated lines, and a wrong guess silently
changes the meaning.

So formulAA does not interpret arbitrary drawings. It defines a format
([docs/aa-spec.md](docs/aa-spec.md)) in which every accepted picture has
exactly one reading; the rules are summarized in [what makes a picture
parseable](#the-core-idea-what-makes-a-picture-parseable) below.

This requires a few **reserved structural glyphs** — the fraction bar
`─`, the operator band `┈`, delimiter columns `⎛ ⎜ ⎝`, grid junctions
`┼` — which never appear as ordinary content and have to line up
correctly. Keeping them aligned by hand would be tedious, so the editor
does it: it edits the syntax tree and redraws the picture. The files
themselves are still plain text — you can edit them in vim, paste them
into a document, or have a language model write them
([`SKILL.md`](SKILL.md) documents the format for that purpose).

## The editor

`formulaa formula.aa` opens the editor on a file; `^O` saves and `^W`
saves and quits. Full reference: [keys](docs/keys.md) ·
[commands](docs/commands.md).

- Type naturally: letters become math italics, `//` makes a fraction,
  `^`/`_` open scripts, `(` `[` `{` auto-size, and a `\` minibuffer with
  Tab completion covers the rest (`\frac`, `\sum`, `\alpha`, `\bbR`,
  aliases like `\->` and `\oo`).
- Arrows move *through* structure; `↑`/`↓` enter limits. `Shift+←/→`
  selects, and a structure key wraps the selection.

Three modes help once a formula grows past one line.

### `^F` — the free cursor

Arrows move over the picture instead of through the tree; Enter lands
on the nearest edit position.

![Correcting entries of a Vandermonde determinant](demo/free-move-matrix.gif)

### `^B` — block select

`^B` highlights the enclosing structures of the cursor, `↑`/`↓` widen
and narrow the selection, so a whole subexpression can be copied in a
few keys.

![Building the DPO loss by copying the policy ratio](demo/dpo.gif)

### `^G` — grid edit

Inside a matrix, `^G` gives a cell cursor; `c` and `r` switch to column
and row lanes, where Enter on a gap inserts one and Backspace on a lane
removes it.

![Growing a 2x2 rotation matrix into a 3x3 one](demo/grid-edit.gif)

## The core idea: what makes a picture parseable

The format is designed around four rules that make parsing
deterministic:

1. **Every subexpression owns a rectangle and a baseline row.** Siblings
   sit in disjoint column ranges; vertical structure exists only inside
   a rectangle. Parsing is: find the baseline, scan left to right,
   recurse into the rectangles that structural glyphs claim.

2. **Structure is drawn with glyphs that can never be atoms.** The bar
   `─`, the band `┈`, delimiter columns `⎛ ⎜ ⎝`, the radical `√` are
   banned from ordinary content, so when one appears it always marks
   structure. Atoms come from an allow-list of one-cell characters, so
   a wide or combining character cannot break the grid.

3. **Extent is spanned, never counted.** A bar is wider than both its
   arguments; a band sandwiches its operator. No rule depends on *how
   many* spaces separate two things, so shifting something sideways
   while hand-editing does not change the reading.

4. **One canonical spelling per tree.** The renderer's output is the
   normal form and the parser accepts a superset. What the picture
   cannot distinguish, the AST does not represent: accents stack as
   flat lists, because the picture cannot tell `\hat{\underline{x}}`
   from `\underline{\hat{x}}`.

The result is that a formula is a picture and a syntax tree at the same
time.

## Examples

Taken from the test corpus ([more examples](docs/examples.md)); each
parses back to its exact tree and converts to the LaTeX shown.

The quadratic formula:

```plain
      ┌──────
   -𝑏±√𝑏²-4𝑎𝑐
𝑥=────────────
       2𝑎
```

```latex
x=\frac{-b\pm \sqrt{b^{2}-4ac}}{2a}
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

A Vandermonde determinant — grids carry explicit lattice markers, so
rows and columns stay unambiguous even with empty cells:

```plain
⎡ 1   𝑥₁   𝑥₁²   ⋯   𝑥₁ⁿ⁻¹ ⎤
⎢   ┼    ┼     ┼   ┼       ⎥
⎢ 1   𝑥₂   𝑥₂²   ⋯   𝑥₂ⁿ⁻¹ ⎥
⎢   ┼    ┼     ┼   ┼       ⎥ = ┈┈┈┈∏┈┈┈┈ (𝑥ⱼ-𝑥ᵢ)
⎢ ⋮   ⋮     ⋮    ⋱     ⋮   ⎥    1≤𝑖<𝑗≤𝑛
⎢   ┼    ┼     ┼   ┼       ⎥
⎣ 1   𝑥ₙ   𝑥ₙ²   ⋯   𝑥ₙⁿ⁻¹ ⎦
```

## Install

```sh
cargo install formulaa
```

## CLI

```sh
formulaa formula.aa            # edit a formula file (^O saves, ^W saves and quits,
                               #   ^Y copies the AA; a missing file is created)
cat formula.aa | formulaa -    # …or take it from stdin
formulaa --aa2latex formula.aa # AA → LaTeX (stdin works too)
formulaa --latex2aa formula.tex # LaTeX → AA, best effort (KaTeX/MathJax dialect)
formulaa --format formula.aa   # normalize hand-written AA to canonical form
```

Everything `--aa2latex` emits reads back to the same tree, and `\latex`
in the editor opens a box to paste LaTeX into (unknown commands are
skipped, never an error).

## Fonts

The format leans on Unicode math symbols — mathematical alphanumerics
(`𝑥`, `𝒟`, `𝔼`), big operators, bracket pieces, box drawing — which most
coding fonts cover only in part.
[JuliaMono](https://juliamono.netlify.app/) has all of them at a
monospace width and is the recommended font for the editor;
[`tools/merge_math_font.py`](tools/merge_math_font.py) ports just those
glyphs into another coding font if you would rather keep yours.

## For AI agents

Language models can read and write the format directly.
[`SKILL.md`](SKILL.md) is a self-contained guide for them, including
the verification loop (`--format` to check the syntax, `--aa2latex` to
confirm the meaning). This makes AA a practical way to embed
re-editable math in documents that humans and agents both maintain.

MIT licensed.
