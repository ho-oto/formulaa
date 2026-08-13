# Architecture decision records

The running log of design decisions, condensed one entry per decision.
Numbering is stable — code and older notes cite entries as `§N`.
Entries marked *superseded* are kept because the reasoning still
explains the shape of the code. The roadmap lives at the end.

## Vision

Make math a first-class citizen of plain text. A human-readable 2D
picture (Unicode-first ASCII art) and a math AST convert
deterministically in both directions, so you can think in AA, convert
mechanically to LaTeX, and re-edit any picture structurally. The
internal representation is the AST (`src/ast.rs`); rendering, LaTeX
output and the structure editor all derive from it. `render` and
`parse` are the two sides of one spec ([aa-spec.md](aa-spec.md)) and
must change together; correctness is the roundtrip contract, enforced
by a corpus plus randomized property tests.

## Decisions

1. **AST as the internal representation** — never TeX strings.
   Structural editing needs the tree anyway, and AA⇄AST bijection is
   the end goal.

2. **Aggressive Unicode** — inline scripts (`x²`), math italics (`𝑥`),
   distinct code points remove ambiguity at the root; parsing gets
   easier than pure-ASCII art, not harder.

3. **The band `┈┈∑┈┈` for limits** — unmarked above/below limits are
   provably ambiguous (`∑_{n=1}^{∫}` vs `∫_{∑_{n=1}}` draw the same
   picture; `examples/ambig.rs`). The band marks the range and carries
   the baseline, like a fraction bar.

4. **Compact (script-style) typesetting — removed** (2026-07).
   Automatic operator spacing and two-tier layout were dropped once
   formatting spacers (§21) covered manual spacing; LaTeX does the
   typesetting anyway.

5. **Explicit `⬚` base** — a leading script renders as `⬚ˣ`, keeping
   it distinct from an empty row; empty required slots are `⬚` too.

6. **Accents are 2D marks, not combining characters** — combining
   characters break the one-cell grid. The cells straight above/below a
   base are otherwise unused, so a column walk reads stacks; over- and
   under-mark glyph sets are disjoint.

7. **Matrices are bracket pairs + blank-band separators** — *superseded
   by §21–23 (lattice markers)*. Round-paren matrices were rejected
   (`(x)` would collide with plain parens).

8. **Upright = function, italic = variable** — render-time
   italicization makes upright ASCII runs self-identifying; canonical
   form is always italic.

9. **No unconditional margins** — a full-height blank column may only
   mean "sibling separator"; property tests showed padding inside
   script arguments breaks the outer chunking. Spacers are inserted
   only where bars would fuse.

10. **normalize** — adjacent same-kind scripts merge (`Sup(a);Sup(b)` ≡
    `Sup(a++b)`, same picture); merging can create new adjacency, so
    normalize is re-applied to a fixed point (idempotence bug found by
    property test).

11. **Symbol coverage** — 4000+ entries auto-extracted from
    [ho-oto/mathematical-symbols](https://github.com/ho-oto/mathematical-symbols);
    later curated down (§55, §60).

12. **Compatibility display mode — removed** (2026-07). A display-only
    ASCII fallback double-managed the canonical form; font problems are
    better solved by the merged font (§13) or an external filter.

13. **Merged-font generator** (`tools/merge_math_font.py`) — copies
    missing math glyphs from JuliaMono into any monospace font with
    advances forced to the base cell width.

14. **Marker-atom display decorations** — jump labels and block
    highlights insert private-use `Sym`s into a display-only clone of
    the AST, decoded after rendering; the layout engine needs no
    coordinate tracking. (14b: the Ctrl+O structure view built on this
    was removed once ^B block-select covered it.)

15. **Cancel via combining overlays** — *superseded by §67*.

16. **`▶` baseline marker — removed** (2026-07). Lenient parsing made
    it unnecessary; ambiguous input errors instead.

17. **Space is an explicit atom `␣`; the Space key is input; Tab
    exits** — real spaces in canonical form would collide with the
    structural meaning of blank columns. Visible spacing is the atom
    `␣` (a LaTeX control space); automatic readability spacers were removed.

18. **`~` as a lenient band — removed** (§25).

19. **Paren/Matrix unified into Delim/Array** —
    `Delim{left,right,mids,segs}` is the only bracket node; a matrix is
    a Delim around a bare-grid Array. Lessons: grid baselines need a
    "is the interior a grid" branch; mismatched pairs require resolving
    shared glyphs by column walk; glyph sides are fixed (`\left)` is
    not expressible — a variable side would make `[a]b]` ambiguous).

20. **Requirements reframed: AA is source code** — the invariants are
    (a) parse is deterministic over everything it accepts, and (b)
    `parse(render(normalize(x))) == normalize(x)`. Canonical form is a
    formatter's output, not the only accepted spelling; input
    whitespace is free.

21. **Lattice markers and formatting spacers** — bare arrays
    self-delimit with junction glyphs (`┌┬┐ ├┼┤ └┴┘`), erasing all
    meaning from whitespace *width*; the Space key became
    `Node::Spacer` (renders one column, vanishes on reparse), relaxing
    the contract to `parse∘render == strip_spacers∘normalize`.

22. **Grids are lattices everywhere; whitespace-count rules abolished**
    — blank-separated grids removed; `[ ]` lost every special case.
    Arrows use the same maximal-munch principle (`───>` is an arrow,
    `─── >` a fraction and an atom).

23. **Fused grids use minimal markers** — a delimiter whose sole
    segment is an Array absorbs the lattice edge; only `┼` separator
    rows (or `┬┴` / `├┤` for single row/column) remain. 1×1 fusions
    normalize away; angles and braces don't fuse.

24. **The band generalizes to any material** (`┈lim┈`, `┈argmax┈`) —
    one consistent rule replaced per-operator special cases; empty-limit
    bands normalize to bare material, so the promotion rule for bare
    `∑` disappeared.

25. **Lenient `~` band removed; "never drop silently"** — the lenient
    form was one misread away from data loss. Principle: accept more
    only where the reading is unique; anything overlapping a baseline
    token errors instead of vanishing.

26. **One keymap** — host-independent `Key`/`Effect` and
    `Editor::input` (`src/input.rs`); TUI and wasm only translate.
    Random key-sequence property tests (with a per-keystroke roundtrip
    check) found real bugs on day one.

27. **Editing UX round 1** — `[` became an ordinary auto-pair; `//`
    makes fractions; internal clipboard (^C/^X/^V); Shift+↑ selects the
    parent; delimiter specs unified as Typst-style `\lr` in visual
    order; the help line is context-dependent.

28. **Block select (^B) and `\op`/`\op*`** — ^B selects structural
    ancestors; `\op` is `\operatorname` (an upright run), `\op*` a
    Text-based band.

29. **Overlay labels, not inserted ones** — display markers are
    stripped from layout and overdrawn, so decorations never shift the
    picture.

30. **Mouse cursor placement** — click maps to the nearest edit
    position via probe rendering.

31. **↑/↓ promote bare big operators** back into bands (empty-limit
    bands don't exist in canonical form, so this is the only way in).

32. **Zero-width caret** — the cursor is `Block.caret` metadata; the
    displayed geometry always equals the cursor-free render. TUI shows
    reverse video.

33. **All display decoration is zero-width** — markers propagate as
    `Block.marks` through every composition; mode displays share the
    edit view's geometry exactly. Invisible slots materialize as ghost
    `⬚` only while needed.

34. **Jump v2** — *superseded by §68 (jump removed)*.

35. **^F free cursor mode** — free 2D movement with a live snap target;
    auto-expands unexpanded scripts near the cursor with hysteresis and
    re-anchoring.

36. **Multi-line formulas** — `Node::Break`; canonical AA stacks lines
    with a lone-`┈` separator row; segments parse as independent
    formulas. No align.

37. **Null delimiter glyphs `┆ ┊`** — dashed "no wall here" glyphs in
    the same ghost family as `┈`.

38. **`\op` name box** — an in-place input box; words become dictionary
    functions or upright runs; `\op*` makes each word a band piece.
    Free-form band-base editing rejected: delete and retype.

39. **Grid edit mode** — a key layer for matrix surgery; input dispatch
    became a chain of mode layers, one method each.

40. **Undo/redo** — snapshot `(root, path, col)` before every
    tree-changing key; cursor restores too; redo clears on new edits.

41. **Context-sensitive quoting for 1-letter romans** (`d𝑦` bare, `'d'`
    isolated); session files spell spacers `␠` — later removed with
    sessions (§52).

42. **Tall angle brackets are diagonal arms only** — mixing `⟨` with
    `╱╲` kinks; even height, fold = vertical pair in one column, upper
    row is the baseline.

43. **In-place minibuffer** — the typed `\command` overlays the cursor
    position with zero layout shift; the dedicated status line is gone.

44. **^F/marker integration** — *superseded by §68*; lessons about
    preview-not-move selection sweeps are recorded in the git history.

45. **Dotted roman runs** (`i.i.d.`), `\rm`/`\text` boxes, `\rcases`.

46. **`\text` real spaces; KaTeX function dictionary; ceil/floor/norm**
    — ceil/floor reuse bracket pieces with one corner dropped; family
    resolution by "which corners does the column run contain"; the norm
    `‖` resolves side by parity and cannot nest directly inside itself.

47. **Wide accents** — mark-side band rows hugging a bare base
    (`WideAccent{overs, unders, base}`); the base may be any block.
    Four fuzz findings folded into the scan rules.

48. **Mark fill glyphs** — every accent draws in a base-hugging form
    (`_` bar, `˰` hat, `˷`/`˜` tilde pair, `․` dot, `￫` vec, `․․` ddot
    with a one-column overhang); fill glyphs are side-exclusive so
    baseline recovery knows which way to dive. Breve deleted.

49. **Glyph consolidation** — band is `┈` everywhere; null delimiters
    `┆ ┊`; norm stacks the same `‖`; fused-grid junctions are light
    `├ ┤`; braces don't fuse and have minimum height 3; legacy forms
    dropped from the parser; `FUNCS` became a single `FuncSpec` table;
    `symbols/` and `output/` directories.

50. **`┈￫┈` vec, `┌─` radical overline, `\^z` script commands.**

51. **^B ancestor chain; `\abs` = `⎢⎥`; word operators; CLI cleanups.**

52. **Prime `′` vs quotes; text mode `"`; phf tables** — `'` is always
    a quote delimiter; upright runs are `\operatorname` (2+ letters) or
    `\mathrm` (1); alphabet-family spellings collapsed into rule +
    exception tables; all lookup tables moved to phf (duplicate keys
    become build errors). Typst-era `also` spellings unified as
    aliases.

53. **Typst output removed** — double maintenance for little value;
    the AST stays target-neutral, so it can return as a module if
    needed.

54. **AST reshape** — LaTeX always uses `\operatorname` (so the
    dictionary only decides limits and lexing); `Text{t, math}` split
    into `Func`/`Roman`/`Text`; bands hold one piece (`┈argmax┈`),
    making band⇄bare a 1:1 unconditional rule; `BigOpSym`/`BigOp`
    split.

55. **Atom allow-list; total LaTeX spellings** — `is_atom` derives from
    the tables; every accepted atom has a LaTeX spelling (gap=0 is a
    test); `^ ~ \` and backtick excluded from atoms; `# $ % &`
    escaped on output.

56. **WideAccent stacks** — `overs`/`unders` vectors, same as compact
    accents; one picture, one AST.

57. **Table roles: pinning, not duplication** — the curated symbol
    table pins meanings against regeneration of the extracted one;
    intentional overrides are enumerated and tested. Aliases live in
    one table.

58. **Table consolidation and file splits** — delimiters became one
    `DELIMS` row per family (was 8 scattered sites); sup/sub bijection
    is one paired-string table; `editor.rs` and `main.rs` split by
    role.

59. **`symbols/` one-concern-per-file** — atoms/funcs/accents/delims/
    arrows/scripts/alphabets/ext; `mod.rs` only wires and re-exports.

60. **Atoms keyed by char** — the character itself is the command;
    input spellings are aliases; `latex` is the single output spelling.

61. **The `Edit` enum** — `resolve(cmd) -> Option<Edit>` (pure) +
    `apply(Edit)` replaced the monolithic `execute`; most commands
    collapse into `Insert{node, wrap}` with one selection rule. Mode
    switches stay out of `Edit` (the boundary: does it mutate the
    undo-tracked tree?).

62. **Arrow/Accent enums** — mark characters became enum variants with
    `drawn()`; four parallel lookup functions disappeared.

63. **render/parse node-level unification — rejected** — render is
    compositional per node but parse's essence is the *recognition*
    problem before any node is known; acceptance is wider than the
    canonical form, so recognizers aren't derivable from the renderer.
    Share the glyph vocabulary (tables), not the control flow; the
    roundtrip property tests are the real "one rule per node" enforcer.

64. **`symbols`/`glyphs` split; `Delim = Col(ColDelim) | Angle`** —
    spelling tables and structural glyph constants are different kinds
    of thing; angles differ from column delimiters in type, not in a
    `None` field. Angle stays inside the pair node (bra-kets mix
    `⟨x│`).

65. **Cancel restricted to atom forms** — *superseded by §67*.

66. **Cancel restricted to `Sym`** — *superseded by §67*.

67. **Cancel removed entirely** — combining-overlay strikes are
    font-unstable (double-width cells, ratatui ghosting). Negation uses
    precomposed slashed atoms via one `negated` table (`\!=`, `\in!` …
    resolve through it); U+0338 input is an explicit parse error.

68. **Jump labels removed** — ^G and label-key selection deleted
    everywhere; ^B stays as an arrows-only mode; `\!` became the
    negation toggle command.

69. **Italic display toggle removed; grid edit moved to ^T.**

70. **^B paints only the current step** — with label jumps gone, the
    arrows walk one step at a time, so only the highlighted ancestor
    and its two neighbors are shown; the purple palette shrank to two
    shades.

71. **Review-round lessons** (2026-08) — two multi-agent review rounds
    over the completion work; recurring root causes: canvas vs screen
    coordinates confused in three separate places (fix: placement
    centralized in `Viewport`/`place_below`), and one-directional
    tables (spellings `resolve` accepted but completion didn't know —
    fix: independent-list tests in both directions).

72. **Unwrap generalized; deletes announce** (2026-08) — the staged
    unwrap (arm → lift contents → delete) extends to `\sqrt` (from its
    root's side) and `\norm`; Shift-selecting *just* a bracket arms the
    pair from either side, and an armed delete unwraps without
    selecting (the gesture already said what it wanted). Deleting into
    a non-empty structure from outside selects it whole first; entering
    is the arrows' job. Containers (`\norm`, `Edit::Delim` commands)
    wrap the selection like `(` does.

73. **Mode commands** (2026-08) — minibuffer spellings for the ctrl
    chords (`\free`, `\blockselect`, `\gridedit`, `\clipboard`,
    `\quit`), dispatched in the input layer (they move modes or the
    app, so they are not `Edit`s and `resolve` never sees them). Shown
    apart: purple minibuffer, bold `[^F]` chord markers in the
    completion; Enter-only commit (never Tab); no one-letter `\q`/`\c`.
    The minibuffer color is three-valued: green = runs, purple = mode,
    red = not yet anything.

74. **Roundtrip failures refuse the edit** (2026-08) — by default the
    guard undoes an edit whose picture stops parsing (the state before
    it is the last one that survives its own file format); `--debug`
    restores the old behavior (keep the state, dump a report to
    `formulaa_debug/`) for fixing the toolchain.

75. **Docs restructured in English** (2026-08) — README from the
    English draft; `aa-spec.md` rewritten as a self-contained spec for
    third-party parser authors (absorbing `parse-model.md`);
    `design.md` renamed to this ADR log; `keys.md` slimmed and split
    from `commands.md`; `editors.md` folded into the README and the
    roadmap.

76. **Renamed: mascii → formulAA** (2026-08) — "mascii" claimed ASCII
    while the format is Unicode-first, and read ambiguously. The new
    name keeps the AA (ASCII-art) identity where it is true — the
    *picture* — and reads as the word it contains (crate and binary:
    `formulaa`). Debug artifacts moved to `formulaa_debug/`, property
    test env vars to `FORMULAA_*`. The GitHub repositories and the
    editor extensions followed suit (formulaa, formulaa-vscode,
    formulaa-obsidian — old URLs redirect, and the legacy ```mascii
    fence stays accepted in the extensions).

## Test strategy

- `tests/roundtrip.rs`: a corpus of real formulas (Cardano,
  Cauchy–Schwarz, Vandermonde, Gaussian integral, Schrödinger, Bayes,
  rotation matrices, continued fractions, nested limits …) plus
  randomized ASTs (2000 per run, scalable via `FORMULAA_PROP_N`).
- `tests/ui.rs`: key-script DSL plus random key sequences with a
  per-keystroke roundtrip check (`FORMULAA_UI_PROP_N`).
- Counterexample hunting: render two candidate ASTs and compare
  pictures (`examples/ambig.rs`).
- After fixing a bug, revert the fix once to confirm the new test
  fails (several vacuous tests were caught this way).

## Roadmap

### Short term

- [x] **ANSI-16 palette audit** — portability: keep the theme within
  the standard + bright ANSI colors. Current state: named ANSI colors
  only (green = OK, maroon = errors, purple = selection, white =
  selection-secondary with forced black text, grey = popup ground);
  the caret is blinking reverse video; the ^F free cursor and ^B's
  provisional selection stay selection-purple and blink (the linear
  Shift selection blinks too); the secondary marks — ^B's
  one-step-outward ring and the ^F snap preview — are reverse video
  with no color of their own (the snap preview blinks); ^B shows only
  the selection and its outward step (the
  inner step is where you just came from); glyphs on any themed ground
  take a fixed foreground for light-terminal safety. Verified
  hands-on, 2026-08.

- [ ] **Command spelling cleanups** — the `\command` vocabulary:
  canonical `\delim` over `\lr`, drop `\tex` (one letter from
  `\text`, different feature), drop the one-letter mode spellings,
  `\grid` as the mode's canonical name, `\negate` alias for `\!`,
  `\sqrt3`/`\sqrt4` as the shown spellings, `\roman` alias,
  `\xRightarrow` shown over `\xTo`.
- [ ] **Paste behavior** — ^V only reads the internal clipboard;
  terminal paste arrives as keystrokes and multi-line LaTeX into the
  `\tex` box commits at the first newline (no bracketed paste).
  Decide how external AA / LaTeX / plain text should be received.

### Editor integrations (own repositories)

[formulaa-vscode](https://github.com/ho-oto/formulaa-vscode) and
[formulaa-obsidian](https://github.com/ho-oto/formulaa-obsidian) each carry
their own wasm crate and depend on this repo by git; bindings are
duplicated deliberately (repo independence over sharing). Both are
prototypes (code-reviewed, not yet field-tested). Zed has no extension
UI API yet — use CLI tasks (`formulaa aa2tex` / `formulaa fmt` over
`$ZED_SELECTED_TEXT`) or the TUI in its terminal. Staging toward the
inline ideal:

1. Now: fenced ```math blocks edited in a panel/modal (implemented).
2. Obsidian: replace the fenced block in Live Preview with the editor
   via a CodeMirror 6 `ReplaceDecoration` widget (technically the
   shortest path).
3. VS Code: no inline-webview API — consider a keystroke-applied
   "virtual structure edit" mode or a Notebook/Custom Editor route.
4. Zed: wait for the extension UI API, then mirror the VS Code shape.

### Mid term

- [ ] MathML output (the `<math>` tree is nearly isomorphic to the AST).
- [ ] AsciiMath-style one-line input (`sum_(i=1)^n 1/i^2` → AST).
- [ ] Multi-line big-operator glyphs (⎲⎳ / ⌠⌡) as an option.
- [ ] East Asian Width handling (currently every glyph is width 1).

### Long term

- [ ] Equation numbers.
- [ ] Templates (theorem environments), search and replace.
- [ ] crates.io release.
