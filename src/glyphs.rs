//! The structural glyph constants — the drawing vocabulary that is
//! *not* a spelling table: nothing here maps a `\name` to a meaning
//! or classifies families; each constant is one canonical glyph with
//! one structural role. The spelling/classification layer lives under
//! `symbols`; render and parse spell their pictures with these names.

// ----- cross-cutting baseline marks -----

/// Editing caret glyph (view-only; never part of the format).
pub const CURSOR_CHAR: char = '▌'; // U+258C LEFT HALF BLOCK

/// Placeholder for an empty mandatory slot, and explicit base of a script
/// that starts a row (so `[Sup(x)]` is distinguishable from `[Sym(x)]`).
pub const PLACEHOLDER: char = '⬚'; // U+2B1A DOTTED SQUARE

/// Fraction bar. Distinct from the '-' atom and the big-op band. Also
/// the single-arrow body (a bar run directly capped by a `>` head *is*
/// the arrow), the radical overline run and the over/underbrace body.
pub const FRAC_BAR: char = '─'; // U+2500 BOX DRAWINGS LIGHT HORIZONTAL

/// Big-operator band: marks the horizontal extent of over/under limits.
/// A lone band row is also the top-level formula-line separator
/// (`Node::Break`).
pub const OP_BAND: char = '┈'; // U+2508 BOX DRAWINGS LIGHT QUADRUPLE DASH HORIZONTAL

/// Double-arrow body (`══>`); ═ has no other use. Heads render as
/// ASCII < > (never the Unicode arrows — those stay ordinary atoms).
pub const DOUBLE_BODY: char = '═'; // U+2550 BOX DRAWINGS DOUBLE HORIZONTAL

// ----- delimiter-layer glyphs outside the pair tables -----

/// The norm `‖` — not a pair (both sides are the same glyph, told
/// apart by extent), so it is its own node and its own glyph.
pub const NORM: char = '‖'; // U+2016 DOUBLE VERTICAL LINE

/// The `\lr` mid separator column (│ between segments).
pub const MID: char = '│'; // U+2502 BOX DRAWINGS LIGHT VERTICAL

/// The tall angles' diagonal arms (drawn by the renderer; resolved
/// contextually by the parser — they name no side of their own).
pub const ARM_RISE: char = '╱'; // U+2571 BOX DRAWINGS LIGHT DIAGONAL
pub const ARM_FALL: char = '╲'; // U+2572

// ----- radicals -----

/// The radical stem column: │ runs from the overline down to the root
/// glyph, which is its bottom cell.
pub const STEM: char = '│'; // U+2502 (shared glyph with the \lr mid separator)

/// The overline's corner, sitting directly above the stem (the lattice
/// ┌ has a blank gap below instead — that adjacency is the
/// disambiguator).
pub const OVERLINE_CORNER: char = '┌'; // U+250C

/// A glyph the stem column may show: the stem or a root sign.
pub fn is_stem_glyph(c: char) -> bool {
    c == STEM || crate::symbols::Radical::of_glyph(c).is_some()
}

// ----- the over/underbrace row (`Node::Brace`) -----

pub const BRACE_TL: char = '╭'; // U+256D
pub const BRACE_TR: char = '╮'; // U+256E
pub const BRACE_BL: char = '╰'; // U+2570
pub const BRACE_BR: char = '╯'; // U+256F

/// The corner pair of the brace row: (left, right) for the given
/// direction.
pub const fn brace_corners(over: bool) -> (char, char) {
    if over {
        (BRACE_TL, BRACE_TR)
    } else {
        (BRACE_BL, BRACE_BR)
    }
}

/// Any of the four brace corners.
pub fn is_brace_corner(c: char) -> bool {
    matches!(c, BRACE_TL | BRACE_TR | BRACE_BL | BRACE_BR)
}

// ----- the grid lattice -----

/// The junction table, indexed by (row kind, col kind):
/// 0 = first, 1 = internal, 2 = last.
pub const LATTICE: [[char; 3]; 3] = [['┌', '┬', '┐'], ['├', '┼', '┤'], ['└', '┴', '┘']];

/// Junction glyph for a lattice crossing at (row kind, col kind).
pub fn lattice_char(row_kind: usize, col_kind: usize) -> char {
    LATTICE[row_kind][col_kind]
}

/// Any of the nine junction glyphs.
pub fn is_lattice_glyph(c: char) -> bool {
    LATTICE.iter().flatten().any(|&l| l == c)
}

/// The left-edge column of a lattice (┌ ├ └), top to bottom kinds.
pub const LATTICE_LEFT: [char; 3] = [LATTICE[0][0], LATTICE[1][0], LATTICE[2][0]];
/// The right-edge column (┐ ┤ ┘).
pub const LATTICE_RIGHT: [char; 3] = [LATTICE[0][2], LATTICE[1][2], LATTICE[2][2]];
/// The top-edge row of a lattice (┌ ┬ ┐), left to right kinds.
pub const LATTICE_TOP: [char; 3] = [LATTICE[0][0], LATTICE[0][1], LATTICE[0][2]];

/// The interior markers by name — what a *fused* grid keeps of the
/// lattice: ┬ ┴ column markers riding the delimiter's top/bottom rows,
/// ├ ┤ row junctions dug into the delimiter columns, ┼ separator rows.
pub const COL_MARK_TOP: char = LATTICE[0][1]; // ┬
pub const COL_MARK_BOT: char = LATTICE[2][1]; // ┴
pub const ROW_JUNCTION_L: char = LATTICE[1][0]; // ├
pub const ROW_JUNCTION_R: char = LATTICE[1][2]; // ┤
pub const CROSSING: char = LATTICE[1][1]; // ┼
