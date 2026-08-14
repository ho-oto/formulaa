# Commands

Everything typed after `\` in the minibuffer. The overlay is drawn at
the cursor and its color says what the spelling currently means:
**green** — runs as an edit; **purple** — a mode command (below);
**red** — not yet anything (a half-typed name, or `\lr`/`\matrix`
still waiting for arguments). Typing `\bar` walks red (`\`) → purple
(`\b`, block select) → red (`\ba`) → green (`\bar`). A known name
previews its result in place; `Enter`/`Space` commits, `Esc` cancels.

## Completion

`↑`/`↓` under a partial name opens the list; each row shows the
inserted symbol beside its spellings:

```plain
\al
 α    al[p[ha]]
 ∀    A, all, forall
 ℵ    aleph
```

- `Tab` commits the highlighted row, or the typed name if it is already
  a command; otherwise it opens the list. `Enter`/`Space` commit the
  highlighted row.
- Spellings that produce the same symbol share a row; `al[p[ha]]` means
  `\al`, `\alp` and `\alpha` all give α (brackets show where you may
  stop typing).
- Matching is prefix, then substring, then subsequence: `in` finds
  `\int` by prefix and `\!in` (∉) by substring.
- Commands whose argument is part of the spelling (`\rmx`, `\matrix34`,
  `\lr(]`, `\^z`) can't live in a table; when the typed spelling itself
  resolves, its own row leads the list.
- *Step rows* continue the spelling instead of running: alphabet
  families show their range (`𝔞…𝔷  frak{a…z}` — accepting types `\frak`
  and waits for the letter), grids ask for their size
  (`matrix{1…9}{1…9}`), and `\lr` offers its next tokens one at a time.
  A row that can't advance any further closes the list when accepted
  again ("your turn to type").
- Rows whose result depends on the cursor (`\mid`, `\delrow`) show a
  short gloss instead of a symbol.
- A mouse click on a row accepts it, exactly as Enter on that row
  would.
- Accents preview as their spacing form (`ˆ`, `˙`, `→`) — the mark on
  a `⬚` would be two lines tall.

## Mode commands

The ctrl chords `^F` `^B` `^G` `^Y` `^O` `^Q` may be stolen by the
terminal or host. The same operations have spellings:

| Spelling | Action |
| --- | --- |
| `\f` `\F` `\free` | free cursor mode (`^F`) |
| `\b` `\B` `\block` `\blockselect` | block select mode (`^B`) |
| `\g` `\G` `\grid` | grid edit (`^G`) |
| `\clipboard` | canonical AA to the **system** clipboard (`^Y`) — no one-letter form, it would read like the internal `^C` |
| `\stdout` | canonical AA to stdout, then quit (`^O`) — spelled out for the same reason as `\quit` |
| `\quit` | quit (`^Q`) — no one-letter form, one typo shouldn't quit |

Mode commands show apart: purple in the minibuffer, and a bold chord
marker in the completion's symbol column (`[^F]  f[ree], F (free
cursor)`). They commit only on an explicit `Enter`/`Space`, never on
`Tab`. Short spellings never shadow symbol names (`\b` is a mode, `\bar`
is still the accent — a test enforces this).

## Structure

| Command | Action |
| --- | --- |
| `\frac` | fraction (a selection becomes the numerator) |
| `\sqrt` `\cbrt` `\qdrt` | √ ∛ ∜ |
| `\matrix34` (= `\array34`) | bare grid, 3 rows × 4 columns — **the size is part of the spelling**; bare `\matrix` doesn't run, the completion asks `matrix{1…9}{1…9}`. (No `\smallmatrix` — the AA cannot keep the smallness, so the spelling would lie) |
| `\pmatrix22` `\bmatrix..` `\Bmatrix..` `\vmatrix..` `\Vmatrix..` `\cases..` `\rcases..` | delimited matrices, same size suffix |
| `\overbrace` `\underbrace` | `╭──╮` / `╰──╯` with a label |
| `\xto` `\xfrom` / `\xTo` `\xFrom` | labelled stretchy arrows → ← / ⇒ ⇐ |
| `\latex` | paste-LaTeX box (best effort, KaTeX/MathJax dialect). No `\tex` — one letter from `\text`, a different feature |

## Delimiters

| Command | Action |
| --- | --- |
| `\lr(]` `\lr{\|}` `\lr\lceil\rfloor` … | delimiter spec **in visual order**: left, middles, right. Tokens are one spec character or a `\name` (`\lparen` `\lceil` `\langle` `\vert` `\none` and the `r…` twins) — those names work only inside `\lr` |
| `\ceil` `\floor` `\abs` `\norm` | ⌈ ⌉, ⌊ ⌋, ⎢ ⎥, ‖ ‖ (no `\Vert` — it read like `\vert`'s sibling while doing something else) |
| `\bra` `\ket` `\braket` `\set` | ⟨·│, │·⟩, ⟨·│·⟩, {·│·} — a lone ⟨ ⟩ pair is `\lr<>` (`\langle` is an `\lr` token only, like `\lceil`) |
| `\mid` | inside a pair: add a `│` segment (remove one by Shift-touching it and deleting); elsewhere: the ∣ atom. `\divides` is the atom by name, everywhere |

## Operators and functions

| Command | Action |
| --- | --- |
| `\sum` `\int` `\prod` … | big-operator atoms (`↑`/`↓` promote to the band with limits) |
| `\lim` `\max` `\det` `\argmax` `\limsup` … | `┈band┈` operators, cursor enters the lower limit |
| `\op` | name box: type a name, confirm → upright run / function |
| `\op*` (`\limits`, `\operatorname*`) | same, but becomes a `┈band┈` and enters the lower limit. Space commits (a band name is one piece) |

## Text, spaces, accents, symbols

| Command | Action |
| --- | --- |
| `\rm`, `\text` | upright run / prose boxes (`\rm<chars>`, `\text<chars>` attached forms work too) |
| `\space` | the semantic space ␣ |
| `\^z` `\_10` `\^gamma` | insert an explicit script node |
| `\hat` `\vec` `\bar` `\dot` `\tilde` … | accent the previous character (repeat to stack); with a selection, the wide form (`┈┈˰┈┈`) |
| `\alpha` `\infty` `\to` `\in` `\bbR` … | ~450 symbol spellings plus styled alphabets, ASCII forms included (`\->`, `\+-`, `\oo`) |
| `\!name` / `\name!` | the slashed negation (`\!in` → ∉, `\subset!` → ⊄); bare `\!` (= `\negate`) toggles the symbol left of the cursor |
