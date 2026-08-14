# formulAA — guide for AI developers

WYSIWYG TUI math editor plus AA⇄AST bidirectional converter.
Rust / ratatui. The project is spelled **formulAA** in prose and titles;
the crate, binary, and identifiers are `formulaa`.

## Commands

```sh
cargo test                      # all tests (unit + roundtrip + UI)
cargo check --no-default-features --target wasm32-unknown-unknown  # wasm build of the lib (extensions depend on it)
cargo clippy --all-targets      # keep zero warnings
cargo fmt                       # always format before committing
cargo run                       # the TUI editor
cargo run --example demo        # render samples without the TUI
cargo run --example ambig       # regression demo for the ambiguity the band solved
cargo run -q --example catalog > docs/examples.md   # regenerate the corpus catalog
echo '...' | cargo run -q -- aa2tex    # AA → LaTeX (fmt likewise)
npx markdownlint-cli2 "**/*.md" "!target"   # docs lint (config: .markdownlint.yaml)
```

The full verification battery for a substantive change: `cargo test`,
the property tests at scale on two seeds
(`FORMULAA_PROP_N=20000 FORMULAA_UI_PROP_N=3000 cargo test --release`,
then again with `FORMULAA_PROP_SEED`/`FORMULAA_UI_PROP_SEED` set),
clippy, the wasm check, `cargo fmt --check`, and markdownlint when docs
changed. After fixing a bug, revert the fix once to confirm the new
test fails — vacuous tests have slipped through more than once.

## Cardinal rules

1. **`src/render` and `src/parse.rs` are the two sides of the spec
   (docs/aa-spec.md).** Never change one without the other; verify with
   `cargo test` against the contract:
   `parse(render(normalize(x))) == normalize(strip_spacers(normalize(x)))`.
   (`render(parse(aa)) == aa` is *not* required — AA is source code,
   acceptance is wider than canonical, `fmt` normalizes.)
2. A new drawing glyph goes into docs/aa-spec.md's reserved table *and*
   into `is_reserved_glyph` in `symbols/atoms.rs` (the test
   `reserved_glyphs_stay_out_of_the_tables` catches collisions with the
   atom tables).
3. `normalize` (ast.rs) must be **idempotent**.
4. New features start with a real formula in `tests/roundtrip.rs`; key
   behavior changes start in `tests/ui.rs` (key-script DSL). Key
   meanings live only in `src/input.rs` — never add key branches in
   main.rs or a wasm host (drift).
5. The TUI re-checks the roundtrip after every edit: a breaking edit is
   **refused** (undone, with an error message naming the failure —
   parse error, AST mismatch, re-render mismatch). Nothing is written
   to disk, so when the user says "it broke", ask for the picture on
   screen and the keys they pressed; reproduce it in `tests/ui.rs` or
   `tests/roundtrip.rs`, and keep that test as the regression.
6. **Never commit or push without an explicit instruction** — each
   authorization covers that one batch only, and never extends to the
   next piece of work.

## Module map

| File | Role |
| --- | --- |
| `src/ast.rs` | the math AST (`Node`/`Row`/`Field`), cursor paths, `normalize` |
| `src/render/` | AST → 2D block with baseline. `block.rs` is node-agnostic block algebra; `mod.rs` holds the per-node rules |
| `src/parse.rs` | AA → AST: region+baseline recursive descent; canonical acceptance plus lenient input |
| `src/editor/mod.rs` | the structure editor (the cursor is a path into the tree; insert/move/delete/select; staged unwrap of `Delim`/`Norm`/`Sqrt`) |
| `src/editor/modes.rs` | ^F free cursor · ^B block select · ^G grid edit (`GridSel`) · display decoration (`decorated`) |
| `src/editor/command.rs` | **`Edit` enum + `resolve`/`apply`** (pure spelling resolution split from application), mode commands (`ModeCmd` — spellings for the ctrl chords, dispatched by the input layer), `\op` name box |
| `src/complete.rs` | **Tab completion** (spelling enumeration, scoring, grouping by identical `Edit`, step rows, mode rows). **Touches no data sources** — derives everything from table keys and `resolve`/`preview_row`; removable as a unit |
| `src/input.rs` | **the shared keymap** (`Key`/`Effect`/`Editor::input`, `completion_click`). TUI and wasm only translate. Tree-changing keys build an `Edit` and `apply` it |
| `src/output/latex.rs` | AST → LaTeX (re-exported as `formulaa::latex`) |
| `src/from_latex.rs` | LaTeX → AST (second path): own output round-trips fully; external LaTeX (KaTeX/MathJax dialect) is best-effort |
| `src/symbols/` | **home of every table** (one concern per file, re-exported flat): `atoms` (vocabulary + gates: `ATOMS`, `NAMES`, `NEGATIONS`/`UNNEGATIONS`, `is_atom` — every accepted atom has a LaTeX spelling, gap=0 tested), `funcs`, `accents` (incl. one-line `preview` glyphs), `radicals`, `delims`, `arrows`, `grids`, `scripts`, `alphabets` |
| `src/glyphs.rs` | structural glyph constants + display markers (the `Mark` enum is the only spelling of the private-use area) |
| `src/theme.rs` | TUI colors, two layers: a named-ANSI palette (the only place a `Color` literal may appear) and role constants assigned through it. The ground rule: fills are dark colors with white glyphs, everything else is reverse video |
| `src/main.rs` | main loop + CLI subcommands, mouse → click / completion pick, the `Effect` handlers (clipboard, stdout, quit) |
| `src/tui.rs` | drawing: layout, scrolling, marker/selection painting, popup/preview overlay, help line |
| `src/guard.rs` | per-edit roundtrip check: a breaking edit is undone and reported on the message line |
| `tests/roundtrip.rs` | formula corpus + randomized property tests |
| `tests/ui.rs` | key-driven UI tests (script DSL + random key sequences) |
| `tools/merge_math_font.py` | merged-font generator (fontTools) |
| (wasm) | wasm bindings live in the **extension repositories** (formulaa-vscode / formulaa-obsidian); this repo is lib + CLI only |
| `SKILL.md` | guide for AI agents reading/writing AA directly |
| `docs/examples.md` | corpus catalog (regenerate via `examples/catalog.rs`) |

## Documents

- `docs/aa-spec.md` — the AA format spec (self-contained; written for
  third-party parser authors)
- `docs/adr.md` — architecture decision records and the roadmap.
  **Read before starting work**
- `docs/keys.md` / `docs/commands.md` — user-facing key and command
  reference. **Update when keys or commands change**

## Pitfalls

- `Block.baseline` can equal `height()` for superscript blocks (no
  baseline row exists). Never index `lines[baseline]` unconditionally.
- A full-height blank column may only mean "sibling separator". No
  unconditional margins (adr.md §9).
- The cursor is `Block.caret` (zero-width metadata propagated through
  every composition); geometry never changes with cursor presence.
  New `render_node` arms must propagate `caret` (the ui-test caret
  check fails otherwise).
- `Sqrt.index`, arrow ops, accent marks, delimiter sides are enums
  (`Radical`/`Arrow`/`Accent`/`Delim`); `Accent.base` is one char.
  There is no `\cancel` — negation is precomposed slashed atoms
  (`symbols::negated`).
- Brackets are `Node::Delim{left,right,mids,segs}`; the slot decides
  the side (`]` cannot open). `Node::Array` is a lattice everywhere
  (bare = full frame, fused = minimal markers). No rule depends on
  whitespace counts.
- Space is a formatting spacer (vanishes on reparse), `\space` = ␣,
  Tab exits insets, Enter breaks lines at top level. The top-level
  render entry is `render_root` (Break splitting + vstack) — don't call
  `render_row` on the root row directly.
- ratatui sits behind the `tui` feature (bin only); no TUI dependency
  in the library (breaks wasm). Colors are ratatui's named ANSI values
  (its plain names are the *dark* colors — `Green` is DarkGreen); the
  crossterm enum never appears in this codebase.
- Handle only `KeyEventKind::Press` (Windows duplicates).
- Display decorations insert private-use marker atoms into a display
  clone (`decorated`). **U+E000–F8FF is display-reserved**
  (`glyphs::is_display_marker`); never let a raw marker reach the
  terminal — `decorate_line` resolves them. Markers render zero-width
  and travel in `Block.marks`; the one exception is the grid-edit gap
  cursor, which really inserts a width-1 ghost lane (its insertion
  preview), kept consistent with click probing (`display_coords`).
- Symbol data (`src/symbols/`) is deliberately hand-written and flat —
  derive from it, don't reorganize it; features like completion attach
  from the outside.
