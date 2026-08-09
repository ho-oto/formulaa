---
name: mascii-math
description: >
  Read and write mascii AA math — 2D plain-text formulas (Unicode-first
  ASCII art) that map one-to-one to a math AST and convert losslessly to
  LaTeX. Use when asked to write, edit or interpret formulas in
  mascii AA form, or to embed re-editable math in plain-text documents.
---

# Reading and writing mascii AA math

The mascii AA format looks like the formula itself while converting
mechanically to LaTeX. This document alone should let you write correct
AA.

## Verification commands (always use them when available)

```sh
echo '<AA>' | mascii fmt      # parse and canonicalize (an error means invalid)
echo '<AA>' | mascii aa2tex   # convert to LaTeX to confirm the meaning
```

Your AA is correct when `mascii fmt` accepts it and the output matches
your intent. Hand-written input is **lenient**: a lone ASCII letter is
an italic variable (`x+1` → 𝑥+1), but a **run of 2+ letters is
upright** (`asiny` → \operatorname{asiny}, `sin` → \sin), so write a
product of variables as `a b` (spaced) or `𝑎𝑏` (italic code points).

## Principles

1. A formula is a character grid. **Every subexpression owns a
   rectangle plus a baseline row**; siblings occupy disjoint column
   ranges.
2. Vertical relationships (numerator/denominator, limits, scripts) are
   marked by **structural glyphs** (bars, bands, delimiter columns),
   never by whitespace alone.
3. When unsure, write on one line: `x²+1`-style one-liners are almost
   always safe.

## Structures

### Fractions — the bar `─` (U+2500)

The bar is wider than either half (max width + 2); the bar row is the
baseline.

```plain
 1        a+b
───   ─────────
 2      c + d
```

Minus stays ASCII `-` (a different character from the bar).

### Scripts — inline characters when possible

`x²` `aᵢ` `e⁻ⁱ` (available: ⁰¹²³⁴⁵⁶⁷⁸⁹⁺⁻⁼⁽⁾ⁿⁱ /
₀₁₂₃₄₅₆₇₈₉₊₋₌₍₎ₐₑₕᵢⱼₖₗₘₙₒₚᵣₛₜᵤᵥₓ). Anything else becomes a 2D block at
the base's **upper/lower right**:

```plain
 α+1
𝑥        ← x^(α+1)     𝑥      ← x_(α+1)
                        α+1
```

A script at the start of a row needs the explicit base `⬚`: `⬚²`.

### Radicals — `√ ∛ ∜` plus the overline `┌──`

```plain
┌────       ┌───
√x+12       │ 1
            │───    ← the stem │ covers every content row; the bottom is the root glyph
            ∛ 2
```

Top row: `┌` (the stem's column) + a `─` run as long as the argument.
One-line content sits directly right of `√`.

### Limits — the band `┈` (U+2508)

**Anything sandwiched by `┈` without spaces** takes limits above and
below (∑ and lim use the same notation). One space separates a band
from its neighbors. No limits → the bare `∫` is fine.

```plain
  ∞
┈┈∑┈┈ aₙ      ┈lim┈ f(x)      ┈argmax┈ f(x)   ← one word, no spaces
 n=1           x→0                x∈S
```

### Delimiters — `( ) [ ] { } ⟨ ⟩ ⎢ ⎥ ⌈ ⌉ ⌊ ⌋ ‖ ┆ ┊` on one line, columns when tall

```plain
⎛    1 ⎞    ⎧ 1 ⎫     ╱    1 ╲
⎜1 + ─ ⎟    ⎨───⎬    ╱ 𝑥+─── ╲    ⎢𝑥⎥ = |x|
⎝    x ⎠    ⎩ 2 ⎭    ╲    2  ╱
                      ╲     ╱
```

- Parens `⎛⎜⎝`, brackets `⎡⎢⎣`, braces `⎧⎪⎨⎩` (**the vertex ⎨ is
  always the baseline row**); tall angles are **diagonal arms `╱ ╲`
  only** (even height, `⟨ ⟩` on one line only). The fold is a vertical
  pair in one column — left: `╱` directly above `╲` — and the upper
  row of the pair is the baseline.
- Absolute value: left `⎢` / right `⎥` (bracket extension pieces; a
  column with no corners reads as a bar; `|` is an atom, `│` is the
  radical stem / segment separator).
- **Middles** (bra-kets, set-builder): full-height `│` columns split
  segments: `⟨ψ│H│ψ⟩`, `{𝑥│𝑥 > 0}` → \{x \mid x>0\}.
- Mismatched pairs are fine: `(0,1]`.
- Ceil/floor: `⌈ ⌉ ⌊ ⌋` (tall: bracket pieces with one corner dropped —
  ceil has no foot, floor has no head). Norm: a full-height `‖` column
  (tall: the same `‖` stacked; a norm directly inside a norm cannot be
  written — use `⎢ ⎥` inside).
- **Null delimiters** left `┆` U+2506 / right `┊` U+250A ("no wall
  here", `\left.`/`\right.`). `cases` is `⎧` + a grid + a right `┊`.

### Matrices and grids — the lattice (the same picture everywhere)

Junction glyphs `┌┬┐ ├┼┤ └┴┘` sit at every separator intersection,
outer border included; a matrix just wraps the grid in a delimiter
pair. Whitespace inside cells is free — markers decide the boundaries:

```plain
┌   ┬   ┐      ⎡ a   b ⎤     ⎡   ┬   ⎤     ⎛ a ⎞
  a   b        ⎢   ┼   ⎥     ⎢ a   b ⎥     ├   ┤
├   ┼   ┤      ⎣ c   d ⎦     ⎣   ┴   ⎦     ⎝ b ⎠
  c   d        ← bmatrix      one row       one column (the ├┤ junctions
└   ┴   ┘       (separator     (┬┴ rows)     bite into the delimiter column)
                rows: spaces + ┼)
```

### Accents — marks directly above/below a one-character base (stackable)

For multi-character bases, a **marked band row** hugs the base:

```plain
┈┈˰┈┈     ┈┈￫┈     ┈___┈
 𝑎𝑏𝑐       𝐴𝐵       𝑧+1   ← \widehat / \vec / \overline
                           (below: ┈¯¯¯┈ = \underline, ┈˜˜˜┈ = \utilde;
                            the base stays bare)
```

All marks draw in base-hugging forms: bar above is `_`, hat `˰`, tilde
`˷`, check `˯`, ring `˳`, dot `․` U+2024 (not the atom `.`), vec `￫`
U+FFEB (not the atom `→`), ddot `․․` (overhanging one column right,
that column's baseline stays blank), underline is `¯` below, utilde `˜`
below (the tildes swap between AST and drawing).

```plain
￫       ￫        ˰
E ⋅ d A          ￫    ← stacks grow outward: \hat{\vec{a}}
                 𝑎
```

### Negation — precomposed slashed atoms only

Combining overlays like U+0338 are rejected (explicit error). Write
≠ ∉ ⊄ ≢ … directly.

### Spaces

Real spaces are formatting and vanish on parse. To put a space into the
LaTeX output use the visible atom `␣` U+2423 (`\space`; a LaTeX control space).

### Upright text — bare runs, `'…'`, `"…"`

Letter runs like `dx` `asiny` are upright (`\operatorname`);
abbreviations with dots (`i.i.d.`, `w.r.t.`) are one run. A single
letter is roman only when glued to a letter (`d𝑦`, the differential);
isolate it as `'d'`. `"if x"` → `\text{if x}` (real spaces allowed,
`\"` `\\` escape). `'` is **always** a quote delimiter — the prime is
the atom `′` U+2032.

### Labelled stretchy arrows — body `─` (`═` for ⇒⇐) + head `>` `<`

```plain
   f
A────>B     ← A \xrightarrow{f} B (labels centered above/below)
```

A head glued to the run makes an arrow; a space makes a fraction bar
plus an atom (`─── >`). Double arrows: `══>` / `<══`.

### overbrace / underbrace — `╭──╮` / `╰──╯`

```plain
  n
╭───╮
 a+b + c      ← \overbrace{a+b}^{n} + \underbrace{c}_{m}
      ╰─╯
       m
```

### Function names — upright ASCII

`sin cos tan log ln exp lim det …` are written upright (variables
italicize to `𝑥`, so they stay distinct). Write `\sin x` as `sin x` or
`sin𝑥` (`sinx` is not a dictionary word and becomes
`\operatorname{sinx}`).

## Reserved characters (never atoms)

`─ ┈ ═ │ √∛∜ ( ) [ ] ⎛⎜⎝⎞⎟⎠ ⎡⎢⎣⎤⎥⎦ { } ⟨ ⟩ ⎧⎪⎨⎩⎫⎬⎭ ╱ ╲ ┆ ┊ ⬚ ▌
┌ ┬ ┐ ├ ┼ ┤ └ ┴ ┘ ╭ ╮ ╰ ╯ ¯ ˜ ˷ _ ˰ ˯ ˳ ․ ￫ ' "`,
math-italic letters, inline script characters. Beyond those, only
characters in the symbol tables are usable (α ≤ ∈ → ℝ ⊗ …); anything
else — full-width characters, emoji — is a parse error (it would shear
the one-cell grid and has no LaTeX spelling). In ASCII, `^ ~ \` and the
backtick are also unusable — write `\sim`, `\backslash`.

## Multi-line formulas

Stack line blocks with a **lone `┈` line** between them:

```plain
𝑦=(𝑥+1)²
┈
=𝑥²+2𝑥+1
```

LaTeX `\\`. There is no alignment.

## Avoiding ambiguity

- Never write an empty script (a `⬚`-only exponent) — it doesn't exist
  in canonical form.
- Use `␣` for visible spacing (real spaces are eaten by `fmt`).
- Always band (`┈`) the limits of ∑/∫ — bare stacking is ambiguous with
  nesting and errors out.
- Never overlap content directly above/below a baseline token (other
  than accent marks) — it errors rather than being dropped.

## Examples

(Real spaces added for readability; `mascii fmt` tightens them.)

Quadratic formula:

```plain
    ┌──────
 -𝑏±√𝑏²-4𝑎𝑐
────────────
     2𝑎
```

Gaussian integral:

```plain
 ∞    -𝑥²   ┌─
┈∫┈┈ 𝑒   𝑑𝑥=√π
 -∞
```

Bayes (a one-liner is enough):

```plain
𝑃(𝐴|𝐵) = 𝑃(𝐵|𝐴)𝑃(𝐴)/𝑃(𝐵)
```

Rotation matrix:

```plain
  ⎡ cosθ   -sinθ ⎤
𝑅=⎢      ┼       ⎥
  ⎣ sinθ   cosθ  ⎦
```

## Full specification

See `docs/aa-spec.md` (the format spec) and `docs/adr.md` (decision
records). For any structure you are unsure of, generate a reference
picture first with the `mascii` TUI or the library
(`mascii::render::render_root` + `RenderCtx::canonical()`).
