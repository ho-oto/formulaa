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

/// The arrow heads capping a body run (`──>` `<══`). The same ASCII
/// chars are ordinary atoms elsewhere — a head is one that touches
/// its body, which is why the renderer spaces the fusion pairs.
pub const HEAD_RIGHT: char = '>';
pub const HEAD_LEFT: char = '<';

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

// ----- display markers (never part of the format) -----

/// A display-time decoration, carried through the render as a
/// zero-width private-use atom and read back from `Block.marks`. The
/// wire form is a `char` so the marker can ride the ordinary AST; this
/// enum is the only place that mapping is written, so a new kind
/// cannot be handled in one host and silently dropped in the other.
///
/// Never let a wire char reach a screen: fonts map the private-use
/// page to logos, so every drawing path resolves through `decode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mark {
    /// Selection range ends (drawn as a background-colored box).
    Sel { open: bool },
    /// Start of a ^B ancestor box: `rank` is the ancestry depth
    /// (0 = innermost), which picks the shade and the highlight. No
    /// letter is drawn — the arrows move the highlight.
    BlockOpen { rank: usize },
    /// End of a ^B ancestor box, paired with the BlockOpen before it.
    BlockClose,
    /// Keeps an empty slot materialized (⬚) while the free cursor is
    /// near it, so approaching it does not shift the layout. No box.
    SlotGhost,
    /// Grid cell-rectangle ends (contents, not structure).
    Cells { open: bool },
    /// Grid lane ends — the lane *itself*, filled to the matrix edge
    /// along its axis, which is why the axis is part of the mark.
    Lane { open: bool, cols: bool },
    /// The edited Array's frame corners: the grid-mode signal.
    Frame { open: bool },
    /// A delimiter pair armed for unwrapping (Backspace/^D against it):
    /// same corner geometry as `Frame`, but only the delimiter columns
    /// light up — they are what the next press removes.
    Delims { open: bool },
    /// The ghost lane previewing an insertion at a lane gap. The one
    /// decoration with real width.
    Gap { cols: bool },
    /// Coordinate probe: never drawn, read straight off `Block.marks`
    /// to build the position → cell table.
    Probe { index: usize },
}

const SEL_BASE: u32 = 0xE0F0;
/// ^B ancestor-box opens (rank = depth) fill the page below SEL_BASE.
const BLOCK_BASE: u32 = 0xE000;
/// Coordinate probes.
pub const PROBE_BASE: u32 = 0xF000;
/// Capacity of the probe block.
pub const PROBE_MAX: usize = 0x800;

impl Mark {
    /// The wire char this mark rides as.
    pub fn ch(self) -> char {
        let u = match self {
            Mark::Sel { open } => SEL_BASE + u32::from(!open),
            Mark::BlockOpen { rank } => BLOCK_BASE + rank as u32,
            Mark::BlockClose => SEL_BASE + 2,
            Mark::SlotGhost => SEL_BASE + 3,
            Mark::Gap { cols: true } => SEL_BASE + 4,
            Mark::Lane { open, cols: true } => SEL_BASE + 5 + u32::from(!open),
            Mark::Cells { open } => SEL_BASE + 7 + u32::from(!open),
            Mark::Frame { open } => SEL_BASE + 9 + u32::from(!open),
            Mark::Delims { open } => SEL_BASE + 11 + u32::from(!open),
            Mark::Lane { open, cols: false } => SEL_BASE + 13 + u32::from(!open),
            Mark::Gap { cols: false } => SEL_BASE + 15,
            Mark::Probe { index } => PROBE_BASE + index as u32,
        };
        char::from_u32(u).expect("marker chars are private-use scalars")
    }

    /// The mark a wire char carries, if any.
    pub fn decode(c: char) -> Option<Mark> {
        let u = c as u32;
        let open = |n: u32| n.is_multiple_of(2);
        match u {
            _ if (SEL_BASE..SEL_BASE + 16).contains(&u) => Some(match u - SEL_BASE {
                n @ (0 | 1) => Mark::Sel { open: open(n) },
                2 => Mark::BlockClose,
                3 => Mark::SlotGhost,
                4 => Mark::Gap { cols: true },
                n @ (5 | 6) => Mark::Lane {
                    open: open(n - 5),
                    cols: true,
                },
                n @ (7 | 8) => Mark::Cells { open: open(n - 7) },
                n @ (9 | 10) => Mark::Frame { open: open(n - 9) },
                n @ (11 | 12) => Mark::Delims { open: open(n - 11) },
                n @ (13 | 14) => Mark::Lane {
                    open: open(n - 13),
                    cols: false,
                },
                15 => Mark::Gap { cols: false },
                _ => return None,
            }),
            _ if (BLOCK_BASE..SEL_BASE).contains(&u) => Some(Mark::BlockOpen {
                rank: (u - BLOCK_BASE) as usize,
            }),
            _ if (PROBE_BASE..PROBE_BASE + PROBE_MAX as u32).contains(&u) => Some(Mark::Probe {
                index: (u - PROBE_BASE) as usize,
            }),
            _ => None,
        }
    }

    /// The mark that opens the box this one closes — the display's
    /// pairing rule, in one place.
    pub fn opener(self) -> Option<Mark> {
        match self {
            Mark::Sel { open: false } => Some(Mark::Sel { open: true }),
            Mark::Cells { open: false } => Some(Mark::Cells { open: true }),
            Mark::Lane { open: false, cols } => Some(Mark::Lane { open: true, cols }),
            Mark::Frame { open: false } => Some(Mark::Frame { open: true }),
            Mark::Delims { open: false } => Some(Mark::Delims { open: true }),
            // A BlockOpen of any rank opens the box BlockClose ends;
            // which rank it is only matters to the paint.
            Mark::BlockClose => Some(Mark::BlockOpen { rank: 0 }),
            _ => None,
        }
    }
}

/// Every private-use codepoint is a display decoration: no marker char
/// may ever reach a screen as a glyph.
pub fn is_display_marker(c: char) -> bool {
    (0xE000..=0xF8FF).contains(&(c as u32))
}

#[cfg(test)]
mod tests {
    use super::*;
    /// The wire mapping is a bijection over every kind, and no mark
    /// escapes the private-use page (where fonts keep their logos).
    #[test]
    fn marks_round_trip_through_their_wire_chars() {
        let mut all: Vec<Mark> = vec![
            Mark::SlotGhost,
            Mark::BlockClose,
            Mark::BlockOpen { rank: 0 },
            Mark::BlockOpen { rank: 0xEF },
        ];
        for open in [true, false] {
            all.push(Mark::Sel { open });
            all.push(Mark::Cells { open });
            all.push(Mark::Frame { open });
            all.push(Mark::Delims { open });
            for cols in [true, false] {
                all.push(Mark::Lane { open, cols });
            }
        }
        for cols in [true, false] {
            all.push(Mark::Gap { cols });
        }
        all.extend([
            Mark::Probe { index: 0 },
            Mark::Probe {
                index: PROBE_MAX - 1,
            },
        ]);
        let mut seen = std::collections::HashSet::new();
        for m in all {
            let c = m.ch();
            assert!(is_display_marker(c), "{:?} escapes the marker page", m);
            assert!(seen.insert(c), "{:?} shares a wire char", m);
            assert_eq!(Mark::decode(c), Some(m));
        }
    }
}
