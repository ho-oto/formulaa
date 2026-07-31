//! The grid lattice glyphs: a bare Array frames itself with
//! box-drawing junctions at every crossing of its separator rows and
//! columns, outer edges included (┌ ┬ ┐ / ├ ┼ ┤ / └ ┴ ┘), so it needs
//! no delimiter to have a parseable extent, and the explicit corners
//! make adjacent lattices unambiguous. A delimiter fusing with a sole
//! grid absorbs the edges and shows only the interior markers (┬ ┴ on
//! its top/bottom rows, ├ ┤ junctions in its own columns, ┼ rows in
//! between — see `Delim::fuses`).

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
