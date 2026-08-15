//! AA -> AST parser: the inverse of `render.rs` on canonical output.
//!
//! Model: a rectangular region of the character grid plus a baseline row.
//! The baseline is scanned left to right; every structural glyph anchors a
//! sub-rectangle that is parsed recursively (see docs/aa-spec.md for the
//! full rules and the reasoning behind them).
//!
//! The parser is slightly lenient beyond the canonical form: plain ASCII
//! letters are accepted as math-italic equivalents (so hand-typed input
//! like `x+1` works), and known function names in upright ASCII ("sin")
//! parse as `Node::Func`.

use crate::ast::{Node, Row};
use crate::glyphs::{
    ARM_FALL, ARM_RISE, BRACE_BL, BRACE_TL, COL_MARK_BOT, COL_MARK_TOP, CROSSING, DOUBLE_BODY,
    FRAC_BAR, HEAD_LEFT, HEAD_RIGHT, LATTICE, LATTICE_LEFT, LATTICE_RIGHT, LATTICE_TOP, MID, NORM,
    OP_BAND, OVERLINE_CORNER, PLACEHOLDER, ROW_JUNCTION_L, ROW_JUNCTION_R, STEM, is_stem_glyph,
    lattice_char,
};
use crate::render::unstyle_char;
use crate::symbols::{ColDelim, Delim};
use crate::symbols::{unsubscript_char, unsuperscript_char};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub msg: String,
    /// 0-based (line, column) in the input text.
    pub at: (usize, usize),
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "parse error at {}:{}: {}",
            self.at.0 + 1,
            self.at.1 + 1,
            self.msg
        )
    }
}

impl std::error::Error for ParseError {}

type Result<T> = std::result::Result<T, ParseError>;

fn err<T>(msg: impl Into<String>, r: usize, c: usize) -> Result<T> {
    Err(ParseError {
        msg: msg.into(),
        at: (r, c),
    })
}

pub struct Grid {
    g: Vec<Vec<char>>,
}

impl Grid {
    fn at(&self, r: usize, c: usize) -> char {
        self.g[r][c]
    }
}

/// Inclusive rectangle of grid cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Rect {
    t: usize,
    b: usize,
    l: usize,
    r: usize,
}

impl Rect {
    fn rows(&self) -> std::ops::RangeInclusive<usize> {
        self.t..=self.b
    }
    fn cols(&self) -> std::ops::RangeInclusive<usize> {
        self.l..=self.r
    }
}

fn col_blank(g: &Grid, rect: Rect, c: usize) -> bool {
    rect.rows().all(|r| g.at(r, c) == ' ')
}

fn row_blank(g: &Grid, rect: Rect, r: usize) -> bool {
    rect.cols().all(|c| g.at(r, c) == ' ')
}

/// Shrink to the bounding box of non-blank cells. None if fully blank.
fn trim(g: &Grid, mut rect: Rect) -> Option<Rect> {
    while rect.t <= rect.b && row_blank(g, rect, rect.t) {
        if rect.t == rect.b {
            return None;
        }
        rect.t += 1;
    }
    while rect.b > rect.t && row_blank(g, rect, rect.b) {
        rect.b -= 1;
    }
    while rect.l <= rect.r && col_blank(g, rect, rect.l) {
        if rect.l == rect.r {
            return None;
        }
        rect.l += 1;
    }
    while rect.r > rect.l && col_blank(g, rect, rect.r) {
        rect.r -= 1;
    }
    Some(rect)
}

/// Side-distinct delimiter glyphs, derived from the `Delim` table so a
/// new pair needs no edit here. The shared extension ⎪ and the angle
/// arms ╱ ╲ are deliberately absent (they name no side of their own);
/// the lattice edges (┌├└ / ┐┤┘) are included so a lattice interior is
/// bracket-protected against cell/script splitting like any other pair.
fn side_glyphs(left: bool) -> &'static [char] {
    use std::sync::OnceLock;
    static SIDES: OnceLock<[Vec<char>; 2]> = OnceLock::new();
    let sides = SIDES.get_or_init(|| {
        let build = |left: bool| {
            let mut v: Vec<char> = Delim::ALL
                .iter()
                .flat_map(|d| d.glyphs(left).iter().copied())
                .filter(|&c| !ColDelim::side_shared_pieces().contains(&c))
                .chain(if left { LATTICE_LEFT } else { LATTICE_RIGHT })
                .collect();
            v.sort_unstable();
            v.dedup();
            v
        };
        [build(true), build(false)]
    });
    &sides[usize::from(!left)]
}

/// What a delimiter column resolves to: a pair side, or the norm —
/// which shares the whole scanning machinery but is its own node.
#[derive(Clone, Copy, PartialEq, Eq)]
enum SideKind {
    Pair(Delim),
    Norm,
}

/// Delimiter spec char for a glyph that can appear on the *baseline row*
/// of a left delimiter column (symbols::Delim answers; the norm ‖ is
/// the one non-pair). Brace/angle columns always show their vertex
/// (⎨ / ⟨) on the baseline, so ⎧ ⎪ ⎩ ╱ ╲ never occur here — and a
/// standalone ├ is a lattice edge (fused junctions resolve via the
/// column walk in open_spec_at).
fn open_spec(c: char) -> Option<SideKind> {
    if c == NORM {
        return Some(SideKind::Norm);
    }
    Delim::of_baseline_piece(c, true).map(SideKind::Pair)
}

/// Fused-grid markers of a delimiter block, if its interior is one.
/// Canonical shapes: pure ┼-rows (multi-row × multi-col), ┬/┴ edge rows
/// (one row), ├/┤ junctions in the delimiter columns (one column) — and
/// any mix of these is accepted as input. A "pure" row contains nothing
/// but blanks and its marker (a nested structure always shows some other
/// glyph on that row). Returns (marker_cols, marker_rows, t, b) with the
/// cell area rows t..=b.
fn fused_grid_markers(
    g: &Grid,
    top: usize,
    bot: usize,
    col: usize,
    close: usize,
) -> Option<(Vec<usize>, Vec<usize>, usize, usize)> {
    if close <= col + 1 || bot <= top {
        return None;
    }
    let pure = |r: usize, mark: char| {
        let mut seen = false;
        for c2 in col + 1..close {
            let ch = g.at(r, c2);
            if ch == mark {
                seen = true;
            } else if ch != ' ' {
                return None;
            }
        }
        seen.then(|| {
            (col + 1..close)
                .filter(|&c2| g.at(r, c2) == mark)
                .collect::<Vec<_>>()
        })
    };
    let (mut t, mut b) = (top, bot);
    let mut cols_marks: Option<Vec<usize>> = None;
    if let Some(cs) = pure(top, COL_MARK_TOP) {
        cols_marks = Some(cs);
        t = top + 1;
    }
    if let Some(cs) = pure(bot, COL_MARK_BOT) {
        if let Some(prev) = &cols_marks {
            if *prev != cs {
                return None;
            }
        } else {
            cols_marks = Some(cs);
        }
        b = bot - 1;
    }
    let mut marker_rows: Vec<usize> = Vec::new();
    for r in t..=b {
        if let Some(cs) = pure(r, CROSSING) {
            match &cols_marks {
                Some(prev) if *prev != cs => return None,
                _ => cols_marks = Some(cs),
            }
            marker_rows.push(r);
        } else if g.at(r, col) == ROW_JUNCTION_L || g.at(r, close) == ROW_JUNCTION_R {
            marker_rows.push(r);
        }
    }
    let marker_cols = cols_marks.unwrap_or_default();
    if marker_cols.is_empty() && marker_rows.is_empty() {
        return None;
    }
    Some((marker_cols, marker_rows, t, b))
}

/// Resolve the delimiter family of a glyph on the baseline row.
/// Bracket pieces are shared with ceil/floor (⎡+⎢ = ceil, ⎢+⎣ = floor,
/// both corners = bracket) and ├ junctions belong to any family, so the
/// contiguous column run's glyph set decides.
fn open_spec_at(g: &Grid, row: usize, col: usize) -> Option<SideKind> {
    let ch = g.at(row, col);
    if angle_open_turn(g, row, col) {
        return Some(SideKind::Pair(Delim::Angle));
    }
    if !ColDelim::is_shared_piece(ch, true)
        && !(ch == ROW_JUNCTION_L && fused_junction(g, row, col))
    {
        return open_spec(ch);
    }
    let h = g.g.len();
    let in_run = |c: char| c == ROW_JUNCTION_L || ColDelim::run_glyphs(true).contains(&c);
    let mut top = row;
    while top > 0 && in_run(g.at(top - 1, col)) {
        top -= 1;
    }
    let mut bot = row;
    while bot + 1 < h && in_run(g.at(bot + 1, col)) {
        bot += 1;
    }
    let has = |c: char| (top..=bot).any(|r| g.at(r, col) == c);
    Some(SideKind::Pair(Delim::Col(ColDelim::of_run(has, true))))
}

fn close_spec(c: char) -> Option<SideKind> {
    if c == NORM {
        return Some(SideKind::Norm);
    }
    Delim::of_baseline_piece(c, false).map(SideKind::Pair)
}

/// Like `close_spec`, resolving a fused-grid junction (┤) by walking its
/// column to a family-distinct glyph (through ⎪ and further junctions).
fn close_spec_at(g: &Grid, row: usize, col: usize) -> Option<SideKind> {
    let ch = g.at(row, col);
    if angle_close_turn(g, row, col) {
        return Some(SideKind::Pair(Delim::Angle));
    }
    let h = g.g.len();
    if !ColDelim::is_shared_piece(ch, false)
        && !(ch == ROW_JUNCTION_R && fused_junction(g, row, col))
    {
        return close_spec(ch);
    }
    let in_run = |c: char| c == ROW_JUNCTION_R || ColDelim::run_glyphs(false).contains(&c);
    let mut top = row;
    while top > 0 && in_run(g.at(top - 1, col)) {
        top -= 1;
    }
    let mut bot = row;
    while bot + 1 < h && in_run(g.at(bot + 1, col)) {
        bot += 1;
    }
    let has = |c: char| (top..=bot).any(|r| g.at(r, col) == c);
    Some(SideKind::Pair(Delim::Col(ColDelim::of_run(has, false))))
}

/// Every glyph a left delimiter column can contain (for the
/// vertical-extent scan).
fn left_family(side: SideKind) -> Vec<char> {
    match side {
        SideKind::Norm => vec![NORM],
        SideKind::Pair(d) => {
            let mut v: Vec<char> = d.glyphs(true).to_vec();
            // A fused-grid junction can sit anywhere in the column…
            v.push(ROW_JUNCTION_L);
            // …and a pair that spells both sides alike (the bar) owns
            // either side's pieces.
            if d.spec(true) == d.spec(false)
                && let Delim::Col(cd) = d
            {
                v.extend(cd.glyphs(false));
            }
            v
        }
    }
}

/// A ├ / ┤ (U+251C/2524) dug into a delimiter column as a fused-grid
/// row junction: the contiguous column run continues with delimiter
/// glyphs above/below. A bare-lattice edge marker has blank gaps
/// instead, so it never classifies.
fn fused_junction(g: &Grid, row: usize, col: usize) -> bool {
    let (junction, left) = match g.at(row, col) {
        ROW_JUNCTION_L => (ROW_JUNCTION_L, true),
        ROW_JUNCTION_R => (ROW_JUNCTION_R, false),
        _ => return false,
    };
    let family = ColDelim::run_glyphs(left);
    let in_run = |c: char| c == junction || family.contains(&c);
    let mut r = row;
    while r > 0 && in_run(g.at(r - 1, col)) {
        r -= 1;
    }
    let top = r;
    let mut r = row;
    while r + 1 < g.g.len() && in_run(g.at(r + 1, col)) {
        r += 1;
    }
    (top..=r).any(|rr| family.contains(&g.at(rr, col)))
}

/// Vertical extent (top, bottom) of the contiguous ‖ run through
/// (row, col) — nested norm pairs differ exactly here.
fn norm_extent(g: &Grid, row: usize, col: usize) -> (usize, usize) {
    let mut top = row;
    while top > 0 && g.at(top - 1, col) == NORM {
        top -= 1;
    }
    let mut bot = row;
    while bot + 1 < g.g.len() && g.at(bot + 1, col) == NORM {
        bot += 1;
    }
    (top, bot)
}

/// Tall angles are pure diagonals; the turn is a same-column vertical
/// pair. Left angle: ╱ directly above ╲; right angle: ╲ directly above
/// ╱. The upper turn row is the baseline.
fn angle_open_turn(g: &Grid, row: usize, col: usize) -> bool {
    let width = |r: usize| g.g[r].len();
    g.at(row, col) == ARM_RISE
        && row + 1 < g.g.len()
        && col < width(row + 1)
        && g.at(row + 1, col) == ARM_FALL
}

fn angle_close_turn(g: &Grid, row: usize, col: usize) -> bool {
    let width = |r: usize| g.g[r].len();
    g.at(row, col) == ARM_FALL
        && row + 1 < g.g.len()
        && col < width(row + 1)
        && g.at(row + 1, col) == ARM_RISE
}

/// Resolve which angle an arm glyph belongs to by walking its diagonal
/// toward the turn, testing the turn pattern at every step — adjacent
/// angles can chain their arms into one diagonal run, so a walk must
/// stop at the first turn it meets rather than at the end of the run.
/// Some(true) = left/open, Some(false) = right/close.
fn angle_arm_side(g: &Grid, row: usize, col: usize) -> Option<bool> {
    let ch = g.at(row, col);
    let h = g.g.len();
    let at = |r: usize, c: usize| -> char {
        if r < h && c < g.g[r].len() {
            g.at(r, c)
        } else {
            ' '
        }
    };
    match ch {
        ARM_RISE => {
            // Left upper arm: follow ╱ down-left; the turn shows as
            // ╱-over-╲ at some cell of the run.
            let (mut r, mut c) = (row, col);
            loop {
                if angle_open_turn(g, r, c) {
                    return Some(true);
                }
                if c > 0 && at(r + 1, c - 1) == ARM_RISE {
                    r += 1;
                    c -= 1;
                } else {
                    break;
                }
            }
            // Right lower arm: follow ╱ up-right; the turn's ╲ sits
            // directly above a run cell.
            let (mut r, mut c) = (row, col);
            loop {
                if r > 0 && angle_close_turn(g, r - 1, c) {
                    return Some(false);
                }
                if r > 0 && at(r - 1, c + 1) == ARM_RISE {
                    r -= 1;
                    c += 1;
                } else {
                    break;
                }
            }
            None
        }
        ARM_FALL => {
            // Right upper arm: follow ╲ down-right to a ╲-over-╱ turn.
            let (mut r, mut c) = (row, col);
            loop {
                if angle_close_turn(g, r, c) {
                    return Some(false);
                }
                if at(r + 1, c + 1) == ARM_FALL {
                    r += 1;
                    c += 1;
                } else {
                    break;
                }
            }
            // Left lower arm: follow ╲ up-left; the turn's ╱ sits
            // directly above a run cell.
            let (mut r, mut c) = (row, col);
            loop {
                if r > 0 && angle_open_turn(g, r - 1, c) {
                    return Some(true);
                }
                if r > 0 && c > 0 && at(r - 1, c - 1) == ARM_FALL {
                    r -= 1;
                    c -= 1;
                } else {
                    break;
                }
            }
            None
        }
        _ => None,
    }
}

/// Which side of a delimiter pair the glyph at (row, col) belongs to:
/// Some(true) = left/open, Some(false) = right/close, None = neither.
/// The shared glyphs (brace extension ⎪, angle arms ╱ ╲) resolve their
/// side by walking the contiguous column run to a side-distinct glyph —
/// without this, depth counting desyncs on rows that cut through a
/// mismatched pair (e.g. a { … ┆ cases block).
fn delim_side(g: &Grid, row: usize, col: usize) -> Option<bool> {
    let ch = g.at(row, col);
    // The sqrt overline corner (┌ directly above the radical stem) is
    // not a delimiter: its row has no matching close, and counting it
    // would desync the depth scan.
    if ch == OVERLINE_CORNER && row + 1 < g.g.len() && is_stem_glyph(g.at(row + 1, col)) {
        return None;
    }
    if ch == NORM {
        // Norm columns use the same ‖ on both sides: parity along the
        // row decides, but only among columns with the same vertical
        // extent — nested norms render the outer pair two rows taller,
        // which is what tells the pairs apart.
        let ext = norm_extent(g, row, col);
        let before = (0..col)
            .filter(|&c2| g.at(row, c2) == NORM && norm_extent(g, row, c2) == ext)
            .count();
        return Some(before % 2 == 0);
    }
    if side_glyphs(true).contains(&ch) {
        return Some(true);
    }
    if side_glyphs(false).contains(&ch) {
        return Some(false);
    }
    if let s @ Some(_) = angle_arm_side(g, row, col) {
        return s;
    }
    // A side-shared piece (the brace extension): walk the column to a
    // side-distinct piece of the families that own it.
    if !ColDelim::side_shared_pieces().contains(&ch) {
        return None;
    }
    let family: Vec<char> = ColDelim::ALL
        .iter()
        .filter(|d| [true, false].iter().any(|&l| d.run_pieces(l).contains(&ch)))
        .flat_map(|d| [true, false].map(|l| d.run_pieces(l)))
        .flatten()
        .copied()
        .collect();
    let mut r = row;
    while r > 0 && family.contains(&g.at(r - 1, col)) {
        r -= 1;
    }
    while r < g.g.len() && family.contains(&g.at(r, col)) {
        let c2 = g.at(r, col);
        if side_glyphs(true).contains(&c2) {
            return Some(true);
        }
        if side_glyphs(false).contains(&c2) {
            return Some(false);
        }
        r += 1;
    }
    None
}

/// Columns lying strictly inside a bracket pair on any row of `rect`
/// (relative to rect.l). Such columns are never treated as matrix cell
/// separators or script-chunk boundaries even when fully blank — this is
/// what keeps a nested matrix's internal gaps from splitting its parent.
fn protected_cols(g: &Grid, rect: Rect) -> Vec<bool> {
    let w = rect.r - rect.l + 1;
    let mut protected = vec![false; w];
    for r in rect.rows() {
        let mut depth: i32 = 0;
        for c in rect.cols() {
            let side = delim_side(g, r, c);
            if side == Some(false) {
                depth -= 1;
            }
            if depth > 0 {
                protected[c - rect.l] = true;
            }
            if side == Some(true) {
                depth += 1;
            }
        }
    }
    protected
}

/// Determine the baseline row of a region from its leftmost column.
/// See docs/aa-spec.md §baseline-recovery.
fn find_baseline(g: &Grid, rect: Rect) -> Result<usize> {
    let rect = match trim(g, rect) {
        Some(r) => r,
        None => return err("empty region has no baseline", rect.t, rect.l),
    };
    let c = rect.l;
    let mut occupied: Vec<usize> = rect.rows().filter(|&r| g.at(r, c) != ' ').collect();

    // Bars sit exactly on the baseline of their block, and the leftmost
    // column of a fraction/big-op block contains only the bar (contents
    // are centered with >= 1 column of slack on each side).
    if let Some(&r) = occupied.iter().find(|&&r| {
        matches!(g.at(r, c), FRAC_BAR | OP_BAND | DOUBLE_BODY)
            && !(g.at(r, c) == OP_BAND
                && (accent_band_run(g, rect, r, c, true).is_some()
                    || accent_band_run(g, rect, r, c, false).is_some()))
    }) {
        return Ok(r);
    }

    // Accent marks stack directly above/below their base; over-marks and
    // under-marks are reserved chars disjoint from atoms, so stripping them
    // leaves the base row.
    while occupied.len() > 1 && over_mark_at(g.at(occupied[0], c)).is_some() {
        occupied.remove(0);
    }
    while occupied.len() > 1 && under_mark_at(g.at(*occupied.last().unwrap(), c)).is_some() {
        occupied.pop();
    }

    let first = *occupied.first().unwrap();
    let last = *occupied.last().unwrap();
    let dive = |t, b, l, r| find_baseline(g, Rect { t, b, l, r });
    match g.at(first, c) {
        // Delimiter columns (any baseline-capable head, or the norm):
        // a fused grid (├ junctions in this column or ┬ markers on the
        // top row) centers on the extent; otherwise the baseline is
        // that of the inner region (a lattice inside answers with its
        // own ┌├└ center rule, a one-line pair with its only row).
        ch if ch == NORM || Delim::of_baseline_piece(ch, true).is_some() => {
            if let Ok(close) = match_delim(g, first, c, rect.r)
                && fused_grid_markers(g, first, last, c, close).is_some()
            {
                return Ok((first + last) / 2);
            }
            dive(first, last, c + 1, rect.r)
        }
        // Brace columns carry their vertex on the baseline row.
        _ if ColDelim::ALL
            .iter()
            .any(|d| d.info().vertex.is_some() && d.run_pieces(true).contains(&g.at(first, c))) =>
        {
            let d = ColDelim::ALL
                .iter()
                .find(|d| d.info().vertex.is_some() && d.run_pieces(true).contains(&g.at(first, c)))
                .unwrap();
            let vertex = d.info().vertex.unwrap().0;
            occupied
                .iter()
                .find(|&&r| g.at(r, c) == vertex)
                .copied()
                .ok_or(())
                .or_else(|_| err("brace column without ⎨", first, c))
        }
        // Angle: the diagonal turn pair — ╱ directly above ╲ in the
        // leftmost column. The upper turn row is the baseline.
        ARM_RISE | ARM_FALL => occupied
            .iter()
            .find(|&&r| angle_open_turn(g, r, c))
            .copied()
            .ok_or(())
            .or_else(|_| err("angle column without a turn", first, c)),
        // Lattice left-edge column: the grid centers on its extent.
        // A ┌ directly above a radical stem is the sqrt overline corner
        // (a lattice ┌ has a blank gap below instead): dive into the
        // radicand right of the stem.
        OVERLINE_CORNER if first < last && is_stem_glyph(g.at(first + 1, c)) => {
            dive(first + 1, last, c + 1, rect.r)
        }
        _ if LATTICE_LEFT.contains(&g.at(first, c)) => Ok((first + last) / 2),
        // Accent band: the base owns the baseline on the other side
        // (over marks ride above their base, under marks below).
        _ if accent_band_run(g, rect, first, c, true).is_some() => {
            dive(first + 1, rect.b, c, rect.r)
        }
        _ if first > rect.t && accent_band_run(g, rect, first, c, false).is_some() => {
            dive(rect.t, first - 1, c, rect.r)
        }
        // Over/under brace corner: the argument owns the baseline on the
        // other side of the brace row.
        BRACE_TL => dive(first + 1, rect.b, c, rect.r),
        BRACE_BL if first > rect.t => dive(rect.t, first - 1, c, rect.r),
        // Sqrt: stem covers exactly the content rows; recurse into content.
        STEM => dive(first, last, c + 1, rect.r),
        ch if crate::symbols::Radical::of_glyph(ch).is_some() => dive(first, last, c + 1, rect.r),
        _ => {
            if occupied.len() == 1 {
                Ok(first)
            } else {
                err(
                    format!(
                        "cannot determine baseline (ambiguous leftmost column; region rows {}..{} cols {}..{})",
                        rect.t, rect.b, rect.l, rect.r
                    ),
                    first,
                    c,
                )
            }
        }
    }
}

/// Parse a region. `baseline` may be passed down when the caller already
/// knows it (paren/sqrt interiors share the caller's baseline row).
fn parse_region(g: &Grid, rect: Rect, baseline: Option<usize>) -> Result<Row> {
    let rect = match trim(g, rect) {
        Some(r) => r,
        None => return Ok(vec![]),
    };
    let bl = match baseline {
        Some(b) => b,
        None => find_baseline(g, rect)?,
    };

    let mut out: Row = Vec::new();
    let mut col = rect.l;
    while col <= rect.r {
        let ch = g.at(bl, col);
        match ch {
            ' ' => {
                let run_end = scan_while(g, bl, col, rect.r, |c| c == ' ');
                // Lattice edge glyphs mark a grid; one whose extent centers
                // on *this* baseline is an inline sibling even though its
                // center row may be blank — carve it out of the run before
                // script handling. (A lattice fully above/below is script
                // content and is left to parse_script_run.)
                let lattice_start = (col..=run_end).find(|&c| {
                    let rows: Vec<usize> = rect
                        .rows()
                        .filter(|&r| LATTICE_LEFT.contains(&g.at(r, c)))
                        .collect();
                    rows.len() >= 2 && (rows[0] + rows[rows.len() - 1]) / 2 == bl
                });
                // A ╭ above (or ╰ below) the baseline in the run marks an
                // over/under brace whose argument owns this baseline; an
                // accent band (┈ run holding only a mark) the same way.
                let brace_start = (col..=run_end).find_map(|c| {
                    brace_at(g, rect, bl, c).map(|(r, over, right)| (c, r, over, right))
                });
                let accent_start = (col..=run_end).find_map(|c| {
                    wide_accent_at(g, rect, bl, c).map(|(r, over, right)| (c, r, over, right))
                });
                #[derive(Clone, Copy)]
                enum Special {
                    Lattice,
                    Brace(usize, bool, usize),
                    Accent(usize, bool, usize),
                }
                let mut special: Option<(usize, Special)> = None;
                if let Some(l) = lattice_start {
                    special = Some((l, Special::Lattice));
                }
                if let Some((c, r, over, right)) = brace_start
                    && special.is_none_or(|(s, _)| c < s)
                {
                    special = Some((c, Special::Brace(r, over, right)));
                }
                if let Some((c, r, over, right)) = accent_start
                    && special.is_none_or(|(s, _)| c < s)
                {
                    special = Some((c, Special::Accent(r, over, right)));
                }
                let run_end = match special {
                    Some((start, kind)) => {
                        if start > col {
                            parse_script_run(g, rect, bl, col, start - 1, &mut out)?;
                        }
                        let (node, right) = match kind {
                            Special::Lattice => parse_lattice(g, rect, start)?,
                            Special::Brace(r, over, right) => {
                                parse_brace(g, rect, bl, start, r, over, right)?
                            }
                            Special::Accent(r, over, right) => {
                                parse_wide_accent(g, rect, bl, start, r, over, right)?
                            }
                        };
                        out.push(node);
                        col = right + 1;
                        continue;
                    }
                    None => run_end,
                };
                parse_script_run(g, rect, bl, col, run_end, &mut out)?;
                col = run_end + 1;
            }
            // A ├ inside a delimiter column run is a fused-grid row
            // junction, not a bare-lattice edge — the delimiter arm
            // below handles it.
            _ if LATTICE_LEFT.contains(&ch) && !fused_junction(g, bl, col) => {
                let (node, right) = parse_lattice(g, rect, col)?;
                out.push(node);
                col = right + 1;
            }
            _ if ch == FRAC_BAR => {
                // Maximal munch: a ─ run capped by > is a labeled arrow;
                // a fraction next to a > atom renders with a space
                // between (presence of the space is the disambiguator).
                let run_end = scan_while(g, bl, col, rect.r, |c| c == FRAC_BAR);
                if run_end < rect.r && g.at(bl, run_end + 1) == HEAD_RIGHT {
                    let span = Rect {
                        t: rect.t,
                        b: rect.b,
                        l: col,
                        r: run_end + 1,
                    };
                    let over =
                        region_above(span, bl).map_or(Ok(vec![]), |r| parse_region(g, r, None))?;
                    let under =
                        region_below(span, bl).map_or(Ok(vec![]), |r| parse_region(g, r, None))?;
                    out.push(Node::Arrow {
                        op: crate::symbols::Arrow::of_body(FRAC_BAR, true).unwrap(),
                        over,
                        under,
                    });
                    col = run_end + 2;
                    continue;
                }
                let span = Rect {
                    t: rect.t,
                    b: rect.b,
                    l: col,
                    r: run_end,
                };
                let num =
                    region_above(span, bl).map_or(Ok(vec![]), |r| parse_region(g, r, None))?;
                let den =
                    region_below(span, bl).map_or(Ok(vec![]), |r| parse_region(g, r, None))?;
                out.push(Node::Frac { num, den });
                col = run_end + 1;
            }
            _ if ch == OP_BAND => {
                // General band: ┈+ piece ┈+ — whatever is sandwiched in
                // ┈ without spaces takes over/under limits (`┈∑┈`,
                // `┈lim┈`, `┈argmax┈`). The piece is exactly one run, so
                // the bare picture reads back as exactly one node.
                let (pieces, end) = scan_band(g, rect, bl, col)?;
                let [(l0, r0)] = pieces[..] else {
                    return err(
                        if pieces.is_empty() {
                            "band without content"
                        } else {
                            "a band holds one piece (an operator name has no blanks)"
                        },
                        bl,
                        col,
                    );
                };
                let base: String = (l0..=r0).map(|c| unstyle_char(g.at(bl, c))).collect();
                // A named band is an upright run, and with empty
                // limits it normalizes to that run's bare form — which
                // the quoted '…' spelling must be able to carry, or
                // fmt would emit a picture its own parser rejects. So
                // the name is held to the quoted charset, not just to
                // atom-hood.
                if base.chars().count() > 1
                    && let Some(bad) = base
                        .chars()
                        .find(|&c| !(c.is_ascii_alphanumeric() || c == '.'))
                {
                    return err(
                        format!("{:?} cannot appear in an operator name", bad),
                        bl,
                        col,
                    );
                }
                let span = Rect {
                    t: rect.t,
                    b: rect.b,
                    l: col,
                    r: end,
                };
                // A limit region holding nothing but one (repeated)
                // accent mark is a stretchy accent, not a limit — marks
                // are reserved and cannot be atoms, so this reading is
                // free. Mixing a mark row with a content row is an
                // error rather than a guess.
                let upper =
                    region_above(span, bl).map_or(Ok(vec![]), |r| parse_region(g, r, None))?;
                let lower =
                    region_below(span, bl).map_or(Ok(vec![]), |r| parse_region(g, r, None))?;
                let one = base.chars().count() == 1;
                let c0 = base.chars().next().unwrap_or(' ');
                out.push(if one && crate::symbols::is_bigop(c0) {
                    Node::BigOpSym {
                        op: c0,
                        lower,
                        upper,
                    }
                } else if one {
                    return err("a one-character band must be a ∑-class operator", bl, col);
                } else {
                    Node::BigOp {
                        name: base,
                        lower,
                        upper,
                    }
                });
                col = end + 1;
            }
            _ if ch == DOUBLE_BODY => {
                // Double-arrow body: ═ run capped by > (═ has no other use).
                let run_end = scan_while(g, bl, col, rect.r, |c| c == DOUBLE_BODY);
                if run_end == rect.r || g.at(bl, run_end + 1) != HEAD_RIGHT {
                    return err("═ run without a > head", bl, col);
                }
                let span = Rect {
                    t: rect.t,
                    b: rect.b,
                    l: col,
                    r: run_end + 1,
                };
                let over =
                    region_above(span, bl).map_or(Ok(vec![]), |r| parse_region(g, r, None))?;
                let under =
                    region_below(span, bl).map_or(Ok(vec![]), |r| parse_region(g, r, None))?;
                out.push(Node::Arrow {
                    op: crate::symbols::Arrow::of_body(DOUBLE_BODY, true).unwrap(),
                    over,
                    under,
                });
                col = run_end + 2;
            }
            HEAD_LEFT if col < rect.r && matches!(g.at(bl, col + 1), FRAC_BAR | DOUBLE_BODY) => {
                // Left-pointing labeled arrow: head, then the body run.
                let body = g.at(bl, col + 1);
                let run_end = scan_while(g, bl, col + 1, rect.r, |c| c == body);
                let span = Rect {
                    t: rect.t,
                    b: rect.b,
                    l: col,
                    r: run_end,
                };
                let over =
                    region_above(span, bl).map_or(Ok(vec![]), |r| parse_region(g, r, None))?;
                let under =
                    region_below(span, bl).map_or(Ok(vec![]), |r| parse_region(g, r, None))?;
                let op = crate::symbols::Arrow::of_body(body, false).unwrap();
                out.push(Node::Arrow { op, over, under });
                col = run_end + 1;
            }
            '"' => {
                // "double-quoted" \text run: flat baseline chars up to
                // the closing quote. A backslash escapes the next char
                // (so a literal " or \ can live inside); ␣ maps back
                // to a space.
                let mut close = None;
                let mut c2 = col + 1;
                while c2 <= rect.r {
                    match g.at(bl, c2) {
                        '\\' => c2 += 2,
                        '"' => {
                            close = Some(c2);
                            break;
                        }
                        _ => c2 += 1,
                    }
                }
                let Some(close) = close else {
                    return err("unclosed \"", bl, col);
                };
                check_flat_columns(g, rect, bl, col, close, 0, 0)?;
                let mut t = String::new();
                let mut c2 = col + 1;
                while c2 < close {
                    match g.at(bl, c2) {
                        '\\' if c2 + 1 < close => {
                            t.push(g.at(bl, c2 + 1));
                            c2 += 2;
                        }
                        '␣' => {
                            t.push(' ');
                            c2 += 1;
                        }
                        ch => {
                            t.push(ch);
                            c2 += 1;
                        }
                    }
                }
                out.push(Node::Text(t));
                col = close + 1;
            }
            '\'' => {
                // 'single-quoted' \mathrm run. The quote is reserved:
                // the prime atom is ′ U+2032, so a ' is always a
                // delimiter and an unclosed one is an error.
                let close = (col + 1..=rect.r)
                    .take_while(|&c2| g.at(bl, c2) != ' ')
                    .find(|&c2| g.at(bl, c2) == '\'')
                    .filter(|&close| {
                        close > col + 1
                            && (col + 1..close).all(|c2| {
                                let ch2 = g.at(bl, c2);
                                ch2.is_ascii_alphanumeric() || ch2 == '␣' || ch2 == '.'
                            })
                    });
                let Some(close) = close else {
                    return err("unclosed ' (the prime atom is ′)", bl, col);
                };
                check_flat_columns(g, rect, bl, col, close, 0, 0)?;
                let t: String = (col + 1..close)
                    .map(|c2| match g.at(bl, c2) {
                        '␣' => ' ',
                        c2 => c2,
                    })
                    .collect();
                // An all-space run has no bare form (glued next to a
                // letter it would draw as a plain gap and read back as
                // a formatting spacer), so it cannot round-trip.
                if t.chars().all(|c| c == ' ') {
                    return err("an upright run cannot be only spaces", bl, col);
                }
                out.push(if t.chars().count() == 1 {
                    Node::Roman(t.chars().next().unwrap())
                } else {
                    Node::Func(t)
                });
                col = close + 1;
            }
            // Closing delimiters are never atoms: a `)` atom inside
            // any delimiter would be captured by the mismatched-pair
            // depth scan (`{y)}` would close at `)`), so a stray
            // close is an error rather than a silently shifted read.
            _ if matches!(Delim::of_spec(ch), Some((_, Some(false)))) => {
                return err(format!("unmatched {}", ch), bl, col);
            }
            _ if open_spec_at(g, bl, col).is_some() => {
                let (node, close_col) = parse_delim(g, rect, bl, col)?;
                out.push(node);
                col = close_col + 1;
            }
            _ if is_stem_glyph(ch) => {
                // Stem covers exactly the content rows; the radical glyph
                // (√ ∛ ∜) is its bottom cell.
                let mut top = bl;
                while top > rect.t && g.at(top - 1, col) == STEM {
                    top -= 1;
                }
                let mut bot = bl;
                while crate::symbols::Radical::of_glyph(g.at(bot, col)).is_none() {
                    if bot == rect.b {
                        return err("radical stem without √", bl, col);
                    }
                    bot += 1;
                }
                if top == 0 || g.at(top - 1, col) != OVERLINE_CORNER {
                    return err("radical without its ┌─ overline", top, col);
                }
                let index = crate::symbols::Radical::of_glyph(g.at(bot, col)).unwrap();
                // The overline run measures the radicand's width. A stem
                // in the region's last column has no room for one, so
                // there is nothing to cover — an empty radicand, not a
                // scan past the edge (scan_while returns its start when
                // the range is empty).
                let w = if col < rect.r {
                    scan_while(g, top - 1, col + 1, rect.r, |c| c == FRAC_BAR) - col
                } else {
                    0
                };
                let arg = if w == 0 {
                    vec![]
                } else {
                    let inner = Rect {
                        t: top,
                        b: bot,
                        l: col + 1,
                        r: col + w,
                    };
                    parse_region(g, inner, Some(bl))?
                };
                out.push(Node::Sqrt { arg, index });
                col += w + 1;
            }
            _ if ch == PLACEHOLDER => {
                check_flat_columns(g, rect, bl, col, col, 0, 0)?;
                col += 1;
            }
            _ if unsuperscript_char(ch).is_some() => {
                let run_end = scan_while(g, bl, col, rect.r, |c| unsuperscript_char(c).is_some());
                check_flat_columns(g, rect, bl, col, run_end, 0, 0)?;
                let arg = (col..=run_end)
                    .map(|c| Node::Sym(unsuperscript_char(g.at(bl, c)).unwrap()))
                    .collect();
                out.push(Node::Sup { arg });
                col = run_end + 1;
            }
            _ if unsubscript_char(ch).is_some() => {
                let run_end = scan_while(g, bl, col, rect.r, |c| unsubscript_char(c).is_some());
                check_flat_columns(g, rect, bl, col, run_end, 0, 0)?;
                let arg = (col..=run_end)
                    .map(|c| Node::Sym(unsubscript_char(g.at(bl, c)).unwrap()))
                    .collect();
                out.push(Node::Sub { arg });
                col = run_end + 1;
            }
            _ if ch.is_ascii_alphabetic() => {
                // Upright ASCII letter run: a dictionary word is a
                // function, anything longer is a \mathrm text run, and a
                // *lone* single letter is an italic variable — unless it
                // is glued to another letter (d𝑦 = roman differential).
                // Dots join the run for abbreviations (i.i.d., w.r.t.):
                // an interior dot must be followed by a letter, and one
                // trailing dot joins iff the run already has a dot — so
                // `sin.` stays Func + period while `i.i.d.` is one run.
                let mut run_end = scan_while(g, bl, col, rect.r, |c| c.is_ascii_alphabetic());
                loop {
                    let dot = run_end + 1;
                    if dot > rect.r || g.at(bl, dot) != '.' {
                        break;
                    }
                    let next = dot + 1;
                    if next <= rect.r && g.at(bl, next).is_ascii_alphabetic() {
                        run_end = scan_while(g, bl, next, rect.r, |c| c.is_ascii_alphabetic());
                    } else if (col..=run_end).any(|c2| g.at(bl, c2) == '.') {
                        run_end = dot;
                        break;
                    } else {
                        break;
                    }
                }
                check_flat_columns(g, rect, bl, col, run_end, 0, 0)?;
                let word: String = (col..=run_end).map(|c| g.at(bl, c)).collect();
                out.push(if word.chars().count() == 1 {
                    let prev_letter = col > rect.l && g.at(bl, col - 1).is_alphabetic();
                    let next_letter = run_end < rect.r && g.at(bl, run_end + 1).is_alphabetic();
                    if prev_letter || next_letter {
                        Node::Roman(word.chars().next().unwrap())
                    } else {
                        Node::Sym(ch)
                    }
                } else {
                    // Any upright multi-letter run is a Func; the
                    // dictionary only decides limits and lexing.
                    Node::Func(word)
                });
                col = run_end + 1;
            }
            _ => {
                // Marks in the cells directly above/below an atom are an
                // accent stack; those cells are otherwise always blank.
                let (overs, unders, extra) = accent_stacks(g, rect, bl, col);
                check_flat_columns(g, rect, bl, col, col, overs.len(), unders.len())?;
                let base = unstyle_char(ch);
                // Atoms are an allow-list (symbols::is_atom): the layout
                // is a grid of one-cell chars, so anything wider would
                // shift every column, and a char no `\name` produces has
                // no LaTeX spelling either.
                if !crate::symbols::is_atom(base) {
                    return err(format!("{:?} is not a valid atom", base), bl, col);
                }
                if !overs.is_empty() || !unders.is_empty() {
                    out.push(Node::Accent {
                        overs,
                        unders,
                        base,
                    });
                } else {
                    out.push(Node::Sym(base));
                }
                col += 1 + extra;
            }
        }
    }
    crate::render::absorb_row(&mut out);
    Ok(out)
}

/// Extends right from `from` while `pred` holds, never past `max`.
/// `from` itself is assumed to qualify — it is returned untested.
fn scan_while(g: &Grid, row: usize, from: usize, max: usize, pred: impl Fn(char) -> bool) -> usize {
    let mut c = from;
    while c < max && pred(g.at(row, c + 1)) {
        c += 1;
    }
    c
}

fn region_above(span: Rect, bl: usize) -> Option<Rect> {
    (bl > span.t).then(|| Rect { b: bl - 1, ..span })
}

fn region_below(span: Rect, bl: usize) -> Option<Rect> {
    (bl < span.b).then(|| Rect { t: bl + 1, ..span })
}

/// A run of blank-baseline columns: contains superscript blocks (above the
/// baseline) and subscript blocks (below). The two sides are segmented
/// independently into column runs (bracket-protected gaps bridge a run, so
/// a matrix inside a script stays whole), then all script blocks are
/// emitted ordered by their leftmost column.
#[allow(clippy::too_many_arguments)]
fn parse_script_run(
    g: &Grid,
    rect: Rect,
    bl: usize,
    from: usize,
    to: usize,
    out: &mut Row,
) -> Result<()> {
    let mut parts: Vec<(usize, bool, Rect)> = Vec::new();
    let run_span = Rect {
        t: rect.t,
        b: rect.b,
        l: from,
        r: to,
    };
    for (side_rect, opposite, is_sup) in [
        (region_above(run_span, bl), region_below(run_span, bl), true),
        (
            region_below(run_span, bl),
            region_above(run_span, bl),
            false,
        ),
    ] {
        let Some(side) = side_rect else { continue };
        let protected = protected_cols(g, side);
        // A segment boundary is a blank column that belongs to an
        // opposite-side script. Columns blank on BOTH sides are internal
        // spacers/padding of a single script argument and must not split
        // it: in normal form two same-side scripts are always separated
        // by an opposite-side one (adjacent same-side scripts merge).
        let occupied: Vec<bool> = side
            .cols()
            .map(|c| {
                let boundary = col_blank(g, side, c)
                    && opposite.is_some_and(|o| !col_blank(g, o, c))
                    && !protected[c - from];
                !boundary
            })
            .collect();
        let mut i = 0;
        while i < occupied.len() {
            if !occupied[i] {
                i += 1;
                continue;
            }
            let start = i;
            while i < occupied.len() && occupied[i] {
                i += 1;
            }
            let seg = Rect {
                t: side.t,
                b: side.b,
                l: from + start,
                r: from + i - 1,
            };
            if let Some(seg) = trim(g, seg) {
                parts.push((seg.l, is_sup, seg));
            }
        }
    }
    parts.sort_by_key(|&(l, _, _)| l);
    // A column with nothing on either side is the writer's own blank:
    // one `Spacer` each, in place among the script parts. (Blanks the
    // reading needs anyway are taken back out by `absorb_spacers`.)
    let mut items: Vec<(usize, Option<(bool, Rect)>)> =
        parts.iter().map(|&(l, s, r)| (l, Some((s, r)))).collect();
    for c in from..=to {
        let inside_part = parts.iter().any(|&(_, _, r)| r.l <= c && c <= r.r);
        if !inside_part && col_blank(g, rect, c) {
            items.push((c, None));
        }
    }
    items.sort_by_key(|&(c, _)| c);
    for (_, part) in items {
        match part {
            Some((is_sup, r)) => {
                let arg = parse_region(g, r, None)?;
                out.push(if is_sup {
                    Node::Sup { arg }
                } else {
                    Node::Sub { arg }
                });
            }
            None => out.push(Node::Spacer),
        }
    }
    Ok(())
}

/// Scan a band starting at (bl, col): `B+ (piece B+)*` where B is the
/// band char. Returns the piece spans and the column of the final band
/// char. A piece not closed by another band char is an error (canonical
/// bands always are).
fn scan_band(g: &Grid, rect: Rect, bl: usize, col: usize) -> Result<(Vec<(usize, usize)>, usize)> {
    let mut pieces = Vec::new();
    let mut end = scan_while(g, bl, col, rect.r, |c| c == OP_BAND);
    loop {
        let pstart = end + 1;
        if pstart > rect.r || g.at(bl, pstart) == ' ' {
            break;
        }
        let mut pend = pstart;
        while pend < rect.r && g.at(bl, pend + 1) != ' ' && g.at(bl, pend + 1) != OP_BAND {
            pend += 1;
        }
        if pend == rect.r || g.at(bl, pend + 1) != OP_BAND {
            return err("band piece without a closing band char", bl, pstart);
        }
        pieces.push((pstart, pend));
        end = scan_while(g, bl, pend + 1, rect.r, |c| c == OP_BAND);
    }
    Ok((pieces, end))
}

/// Locate a brace anchored at column `c` for the caller's baseline:
/// (brace row, over?, right col). Requires a well-formed ╭──╮ / ╰──╯ row
/// AND baseline coverage (some nonblank cell of the brace's columns on
/// the baseline) — a brace living entirely inside a script region has no
/// baseline content and is left for the script parser.
fn brace_at(g: &Grid, rect: Rect, bl: usize, c: usize) -> Option<(usize, bool, usize)> {
    let cand = (rect.t..bl)
        .find(|&r| g.at(r, c) == BRACE_TL)
        .map(|r| (r, true))
        .or_else(|| {
            (bl + 1..=rect.b)
                .find(|&r| g.at(r, c) == BRACE_BL)
                .map(|r| (r, false))
        });
    let (brow, over) = cand?;
    let run_end = scan_while(g, brow, c, rect.r, |c2| c2 == FRAC_BAR);
    let closer = if over {
        crate::glyphs::BRACE_TR
    } else {
        crate::glyphs::BRACE_BR
    };
    if run_end == rect.r || g.at(brow, run_end + 1) != closer {
        return None;
    }
    let right = run_end + 1;
    (c..=right)
        .any(|c2| g.at(bl, c2) != ' ')
        .then_some((brow, over, right))
}

/// Is the ┈ run through (row, col) an accent band — pieces holding one
/// repeated accent mark and nothing else? Returns (mark, right end).
/// `over` selects which mark set applies (the band above the base wears
/// over marks, the one below wears under marks).
fn accent_band_run(
    g: &Grid,
    rect: Rect,
    row: usize,
    col: usize,
    over: bool,
) -> Option<(crate::symbols::Accent, usize)> {
    if g.at(row, col) != OP_BAND {
        return None;
    }
    // Phased scan — leading ┈s, one material group, trailing ┈s — and
    // STOP there: material appearing after the trailing ┈s belongs to a
    // neighbour (a sibling's raised superscript can sit flush against
    // the band when an ancestor block crops tightly). The run also
    // self-terminates on any non-material char (inside a delimiter the
    // band hugs the delimiter column with no gap).
    let mut piece: Vec<char> = Vec::new();
    let mut end = col;
    let mut trailed = false;
    let mut c = col;
    while c <= rect.r {
        let ch = g.at(row, c);
        if ch == OP_BAND {
            end = c;
            if !piece.is_empty() {
                trailed = true;
            }
        } else if !trailed
            // The full material alphabet: every mark's drawn glyph
            // (symbols::Accent answers).
            && crate::symbols::Accent::is_mark_material(ch)
        {
            piece.push(ch);
        } else {
            break;
        }
        c += 1;
    }
    if !trailed || piece.is_empty() {
        return None;
    }
    // Classify the material. Every fill belongs to exactly one side
    // (over bar _ vs under bar ¯, over tilde ˷ vs under tilde ˜): the
    // baseline dive relies on the over/under classification being
    // positionally unambiguous.
    let all = |m: char| piece.iter().all(|&c| c == m);
    let single = |m: char| piece.len() == 1 && piece[0] == m;
    // The drawn-form table names each glyph's (mark, side); a
    // multi-cell material (the ddot's ․․) matches its cells whole,
    // which is what tells it from the single dot.
    let mark = crate::symbols::Accent::ALL.into_iter().find(|a| {
        a.under() != over
            && match a.drawn() {
                crate::symbols::DrawnForm::Center(g) => single(g),
                crate::symbols::DrawnForm::Fill(g) => all(g),
                crate::symbols::DrawnForm::Dots => piece == a.cells(),
            }
    });
    mark.map(|m| (m, end))
}

/// Locate a wide accent anchored at column `c` for the caller's
/// baseline: an accent band above (over marks) or below (under marks),
/// with some baseline content under its span (a band fully inside a
/// script region is the script parser's business).
fn wide_accent_at(g: &Grid, rect: Rect, bl: usize, c: usize) -> Option<(usize, bool, usize)> {
    let cand = (rect.t..bl)
        .find_map(|r| accent_band_run(g, rect, r, c, true).map(|(_, end)| (r, true, end)))
        .or_else(|| {
            (bl + 1..=rect.b)
                .find_map(|r| accent_band_run(g, rect, r, c, false).map(|(_, end)| (r, false, end)))
        });
    let (brow, over, right) = cand?;
    (c..=right)
        .any(|c2| g.at(bl, c2) != ' ')
        .then_some((brow, over, right))
}

/// Parse a wide accent: the base region owns the caller's baseline,
/// bands (over above / under below, either or both) carry the marks.
#[allow(clippy::too_many_arguments)]
fn parse_wide_accent(
    g: &Grid,
    rect: Rect,
    bl: usize,
    col: usize,
    brow: usize,
    over_first: bool,
    right: usize,
) -> Result<(Node, usize)> {
    // Bands stack: walk away from the baseline while they keep coming,
    // so `overs` comes out innermost-first like a compact Accent's.
    let (overs, top) = if over_first {
        // `brow` is the *outermost* over band (the search runs top
        // down), so collect downward to the baseline and reverse.
        let mut marks = Vec::new();
        let mut r = brow;
        while r < bl
            && let Some((m, _)) = accent_band_run(g, rect, r, col, true)
        {
            marks.push(m);
            r += 1;
        }
        marks.reverse();
        (marks, r)
    } else {
        (Vec::new(), rect.t)
    };
    // The first under band below the baseline (when we started from an
    // over band) — or the anchoring band itself (under-only).
    let under_row = if over_first {
        (bl + 1..=rect.b).find(|&r| accent_band_run(g, rect, r, col, false).is_some())
    } else {
        Some(brow)
    };
    let (unders, bot) = match under_row {
        Some(first) => {
            let mut marks = Vec::new();
            let mut r = first;
            while let Some((m, _)) = accent_band_run(g, rect, r, col, false) {
                marks.push(m);
                if r == rect.b {
                    break;
                }
                r += 1;
            }
            (marks, first - 1)
        }
        None => (Vec::new(), rect.b),
    };
    let base_rect = Rect {
        t: top,
        b: bot,
        l: col,
        r: right,
    };
    let base = parse_region(g, base_rect, Some(bl))?;
    let node = Node::WideAccent {
        overs,
        unders,
        base,
    };
    Ok((node, right))
}

/// An over/under brace: a ╭──╮ (or ╰──╯) row at `brow`, argument block
/// owning the caller's baseline on the other side, label beyond.
#[allow(clippy::too_many_arguments)]
fn parse_brace(
    g: &Grid,
    rect: Rect,
    bl: usize,
    col: usize,
    brow: usize,
    over: bool,
    right: usize,
) -> Result<(Node, usize)> {
    let cols = (col, right);
    let (arg_rect, label_rect) = if over {
        (
            Rect {
                t: brow + 1,
                b: rect.b,
                l: cols.0,
                r: cols.1,
            },
            (rect.t < brow).then(|| Rect {
                t: rect.t,
                b: brow - 1,
                l: cols.0,
                r: cols.1,
            }),
        )
    } else {
        (
            Rect {
                t: rect.t,
                b: brow - 1,
                l: cols.0,
                r: cols.1,
            },
            (brow < rect.b).then(|| Rect {
                t: brow + 1,
                b: rect.b,
                l: cols.0,
                r: cols.1,
            }),
        )
    };
    let arg = parse_region(g, arg_rect, Some(bl))?;
    let label = label_rect.map_or(Ok(vec![]), |r| parse_region(g, r, None))?;
    let node = Node::Brace { over, arg, label };
    Ok((node, right))
}

/// Accent-mark stacks in the cells directly above/below (bl, col),
/// innermost first. Returns (overs, unders).
/// A flat baseline token (atom, letter/text run, inline script chars…)
/// owns its columns entirely: apart from the accent marks it consumed,
/// the cells above and below it must be blank. Hand-written input that
/// stacks anything else there would otherwise be dropped silently.
fn check_flat_columns(
    g: &Grid,
    rect: Rect,
    bl: usize,
    l: usize,
    r: usize,
    skip_over: usize,
    skip_under: usize,
) -> Result<()> {
    for c in l..=r {
        for row in rect.t..bl - skip_over {
            if g.at(row, c) != ' ' {
                return err(
                    "content stacked above a baseline token (not an accent)",
                    row,
                    c,
                );
            }
        }
        for row in bl + skip_under + 1..=rect.b {
            if g.at(row, c) != ' ' {
                return err(
                    "content stacked below a baseline token (not an accent)",
                    row,
                    c,
                );
            }
        }
    }
    Ok(())
}

/// Compact accents draw the same hugging glyphs as the wide bands
/// (`Accent::drawn`), so the column readers are just that table read
/// backwards.
fn over_mark_at(c: char) -> Option<crate::symbols::Accent> {
    crate::symbols::Accent::of_over_glyph(c)
}

fn under_mark_at(c: char) -> Option<crate::symbols::Accent> {
    crate::symbols::Accent::of_under_glyph(c)
}

/// Returns (overs, unders, extra columns consumed). A ddot draws as
/// `․․` overhanging one column to the right of its base; the pair is
/// only taken when that spill column holds nothing else (otherwise the
/// second ․ is the neighbour's own dot and this row stays a single ˙).
fn accent_stacks(
    g: &Grid,
    rect: Rect,
    bl: usize,
    col: usize,
) -> (
    Vec<crate::symbols::Accent>,
    Vec<crate::symbols::Accent>,
    usize,
) {
    let mut overs = Vec::new();
    let mut pair_rows: Vec<usize> = Vec::new();
    let mut r = bl;
    while r > rect.t
        && let Some(m) = over_mark_at(g.at(r - 1, col))
    {
        // A length-sensitive pair (the ddot's ․․): the neighbor cell
        // widens the mark, remembered until the spill test confirms
        // the whole overhang column is free.
        if col < rect.r && m.widen(g.at(r - 1, col + 1)).is_some() {
            pair_rows.push(r - 1);
        }
        overs.push(m);
        r -= 1;
    }
    let mut unders = Vec::new();
    let mut r = bl;
    while r < rect.b
        && let Some(m) = under_mark_at(g.at(r + 1, col))
    {
        unders.push(m);
        r += 1;
    }
    let spill = !pair_rows.is_empty()
        && rect
            .rows()
            .all(|rr| pair_rows.contains(&rr) || g.at(rr, col + 1) == ' ');
    if spill {
        let top = bl - overs.len();
        for &pr in &pair_rows {
            let m = overs[bl - 1 - pr];
            overs[bl - 1 - pr] = m.widen(g.at(pr, col + 1)).unwrap_or(m);
            debug_assert!(pr >= top);
        }
    }
    (overs, unders, spill as usize)
}

/// A grid lattice whose leftmost marker column is `col`: box-drawing
/// junctions (┌ ┬ ┐ / ├ ┼ ┤ / └ ┴ ┘) frame every crossing of the separator
/// rows/columns including the outer edges; the explicit corners terminate
/// the scan, so adjacent lattices can never merge. Returns the Array and
/// the rightmost marker column.
fn parse_lattice(g: &Grid, rect: Rect, col: usize) -> Result<(Node, usize)> {
    let marker_rows: Vec<usize> = rect
        .rows()
        .filter(|&r| LATTICE_LEFT.contains(&g.at(r, col)))
        .collect();
    if marker_rows.len() < 2 || g.at(marker_rows[0], col) != LATTICE[0][0] {
        return err("broken lattice edge column", rect.t, col);
    }
    let top = marker_rows[0];
    // Marker columns: junctions on the top row, through the closing ┐.
    let mut marker_cols = vec![col];
    let mut c = col + 1;
    loop {
        if c > rect.r {
            return err("lattice without a closing ┐", top, col);
        }
        let ch = g.at(top, c);
        if LATTICE_TOP.contains(&ch) {
            marker_cols.push(c);
            if ch == LATTICE[0][2] {
                break;
            }
        }
        c += 1;
    }
    let (rows_n, cols_n) = (marker_rows.len() - 1, marker_cols.len() - 1);
    let kind = |i: usize, n: usize| {
        if i == 0 {
            0
        } else if i == n {
            2
        } else {
            1
        }
    };
    for (ri, &r) in marker_rows.iter().enumerate() {
        for (ci, &mc) in marker_cols.iter().enumerate() {
            let want = lattice_char(kind(ri, rows_n), kind(ci, cols_n));
            if g.at(r, mc) != want {
                return err(
                    format!("broken lattice (expected {} at a crossing)", want),
                    r,
                    mc,
                );
            }
        }
    }
    let right = *marker_cols.last().unwrap();
    let (rows, cols) = (rows_n, cols_n);
    let mut cells = Vec::with_capacity(rows * cols);
    for ri in 0..rows {
        for ci in 0..cols {
            let cell = Rect {
                t: marker_rows[ri] + 1,
                b: marker_rows[ri + 1] - 1,
                l: marker_cols[ci] + 1,
                r: marker_cols[ci + 1] - 1,
            };
            let row = if cell.t > cell.b || cell.l > cell.r {
                vec![]
            } else {
                parse_region(g, cell, None)?
            };
            cells.push(row);
        }
    }
    let node = Node::Array { rows, cols, cells };
    Ok((node, right))
}

/// A delimiter block starting at (bl, col): left column, optional
/// full-height │ middles, matching right column (possibly of a different
/// family — mismatched pairs are legal). Returns the node and the close
/// column.
fn parse_delim(g: &Grid, rect: Rect, bl: usize, col: usize) -> Result<(Node, usize)> {
    let left = match open_spec_at(g, bl, col) {
        Some(sp) => sp,
        None => return err("cannot resolve delimiter family", bl, col),
    };
    let close_col = match_delim(g, bl, col, rect.r)?;
    let right = match close_spec_at(g, bl, close_col) {
        Some(sp) => sp,
        None => return err("cannot resolve delimiter family", bl, close_col),
    };
    // Tall diagonal angles: the arms slant one column per row away from
    // the turn, so the extent comes from the diagonal and the interior
    // starts after the widest arm cell.
    let (top, bot, interior_l) = if angle_open_turn(g, bl, col) {
        let mut k = 1usize;
        while bl + 1 > k && bl >= k && col + k <= rect.r && g.at(bl - k, col + k) == ARM_RISE {
            k += 1;
        }
        (bl + 1 - k, (bl + k).min(rect.b), col + k)
    } else {
        let (t, b) = vertical_extent(g, rect, col, bl, &left_family(left));
        (t, b, col + 1)
    };
    let interior_r = if angle_close_turn(g, bl, close_col) {
        let mut k = 1usize;
        while bl >= k && close_col > k && g.at(bl - k, close_col - k) == ARM_FALL {
            k += 1;
        }
        close_col - k
    } else {
        close_col - 1
    };
    // The pair claims its interior columns for the full region height:
    // content stacked above/below the delimiter's own extent would be
    // silently skipped by the segment parse (which only walks
    // top..=bot), so it is an error — never a quiet drop. A canonical
    // delimiter always spans its content, so only a hand-drawn pair
    // shorter than its interior can trip this.
    for r in rect.rows() {
        if (top..=bot).contains(&r) {
            continue;
        }
        if let Some(c) = (interior_l..=interior_r).find(|&c| g.at(r, c) != ' ') {
            return err("content outside the delimiter's vertical extent", r, c);
        }
    }

    // Fused grid: ┬ markers on the delimiter's top row and/or ├ junction
    // rows in the left column mean the interior is one grid whose edges
    // are the delimiter columns themselves.
    {
        if let Some((marker_cols, marker_rows, t, b)) =
            fused_grid_markers(g, top, bot, col, close_col)
        {
            let (rows_n, cols_n) = (marker_rows.len() + 1, marker_cols.len() + 1);
            let row_edges: Vec<i64> = std::iter::once(t as i64 - 1)
                .chain(marker_rows.iter().map(|&r| r as i64))
                .chain(std::iter::once(b as i64 + 1))
                .collect();
            let col_edges: Vec<usize> = std::iter::once(col)
                .chain(marker_cols.iter().copied())
                .chain(std::iter::once(close_col))
                .collect();
            let mut cells = Vec::with_capacity(rows_n * cols_n);
            for ri in 0..rows_n {
                for ci in 0..cols_n {
                    // Edges are signed because the first one sits one
                    // row above the cell area; an empty cell (a junction
                    // on the very first row) must stay empty rather
                    // than wrap through zero.
                    let (t, b) = (row_edges[ri] + 1, row_edges[ri + 1] - 1);
                    let (l, r) = (col_edges[ci] + 1, col_edges[ci + 1]);
                    let row = if t > b || l + 1 > r {
                        vec![]
                    } else {
                        let cell = Rect {
                            t: t as usize,
                            b: b as usize,
                            l,
                            r: r - 1,
                        };
                        parse_region(g, cell, None)?
                    };
                    cells.push(row);
                }
            }
            let array = Node::Array {
                rows: rows_n,
                cols: cols_n,
                cells,
            };
            let (SideKind::Pair(left), SideKind::Pair(right)) = (left, right) else {
                return err("a norm cannot fuse with a grid", bl, col);
            };
            let node = Node::Delim {
                left,
                right,
                mids: 0,
                segs: vec![vec![array]],
            };
            return Ok((node, close_col));
        }
    }

    let mid_cols = if interior_r >= interior_l {
        mid_columns(
            g,
            Rect {
                t: top,
                b: bot,
                l: interior_l,
                r: interior_r,
            },
        )
    } else {
        vec![]
    };

    let mut segs: Vec<Row> = Vec::new();
    let mut start = interior_l;
    let end_sentinel = interior_r + 1;
    for &m in mid_cols.iter().chain(std::iter::once(&end_sentinel)) {
        let seg = if start >= m {
            vec![]
        } else {
            let rect = Rect {
                t: top,
                b: bot,
                l: start,
                r: m - 1,
            };
            parse_region(g, rect, Some(bl))?
        };
        segs.push(seg);
        start = m + 1;
    }
    // Both norm sides are the same ‖, so it has no middles to speak of
    // (a `│` inside one would have no side to belong to).
    let node = match (left, right) {
        (SideKind::Norm, SideKind::Norm) => {
            if segs.len() != 1 {
                return err("a norm takes no │ middle", bl, col);
            }
            Node::Norm {
                arg: segs.into_iter().next().unwrap(),
            }
        }
        (SideKind::Pair(left), SideKind::Pair(right)) => Node::Delim {
            left,
            right,
            mids: mid_cols.len(),
            segs,
        },
        _ => return err("a ‖ pairs only with ‖", bl, col),
    };
    Ok((node, close_col))
}

/// Middle-separator columns of a delimiter interior: │ over the full
/// extent (a √ stem never spans the extent — its top row is the overline
/// row — and a nested delimiter's middles are bracket-protected).
fn mid_columns(g: &Grid, interior: Rect) -> Vec<usize> {
    let protected = protected_cols(g, interior);
    interior
        .cols()
        .filter(|&c| interior.rows().all(|r| g.at(r, c) == MID) && !protected[c - interior.l])
        .collect()
}

/// Column of the structurally matching close delimiter for the open one at
/// (row, col): depth counting over every side-distinct delimiter glyph, so
/// mismatched pairs like ( … ] pair up too.
fn match_delim(g: &Grid, row: usize, col: usize, max: usize) -> Result<usize> {
    let mut depth = 0;
    for c in col..=max {
        match delim_side(g, row, c) {
            Some(false) => {
                depth -= 1;
                if depth == 0 {
                    return Ok(c);
                }
            }
            Some(true) => depth += 1,
            None => {}
        }
    }
    err(format!("unmatched {}", g.at(row, col)), row, col)
}

/// Contiguous vertical run of `chars` through (bl, col).
fn vertical_extent(g: &Grid, rect: Rect, col: usize, bl: usize, chars: &[char]) -> (usize, usize) {
    let mut top = bl;
    while top > rect.t && chars.contains(&g.at(top - 1, col)) {
        top -= 1;
    }
    let mut bot = bl;
    while bot < rect.b && chars.contains(&g.at(bot + 1, col)) {
        bot += 1;
    }
    (top, bot)
}

/// Parse a formula from its AA text form.
pub fn parse(text: &str) -> Result<Row> {
    // Combining strike overlays would desync every column, so they
    // are refused up front with a pointed message (adr.md §67).
    let mut lines: Vec<Vec<char>> = Vec::new();
    for (r, raw) in text.lines().enumerate() {
        let mut line = Vec::new();
        for c in raw.trim_end().chars() {
            match c {
                '\u{338}' | '\u{336}' => {
                    return err(
                        "strike overlays (\u{338}) are not supported — use the slashed relation atoms (≠ ∉ …)",
                        r,
                        line.len(),
                    );
                }
                '\t' => line.push(' '),
                c => line.push(c),
            }
        }
        lines.push(line);
    }
    if lines.iter().all(|l| l.is_empty()) {
        return Ok(vec![]);
    }
    let width = lines.iter().map(|l| l.len()).max().unwrap();
    for l in &mut lines {
        l.resize(width, ' ');
    }
    let g = Grid { g: lines };
    // Multi-line formulas: a row whose only glyph is a single ┈ is a
    // line separator (a band always sandwiches its pieces, so a lone ┈
    // never occurs inside one formula). Each segment between separators
    // is an ordinary formula, read with the usual baseline inference;
    // segments are joined with Break. Blank rows next to separators are
    // formatting; a blank row splitting a segment without a separator
    // is an error (a canonical block never contains one).
    let h = g.g.len();
    let blank_row = |r: usize| g.g[r].iter().all(|&c| c == ' ');
    let sep_row = |r: usize| {
        let mut glyphs = g.g[r].iter().filter(|&&c| c != ' ');
        glyphs.next() == Some(&OP_BAND) && glyphs.next().is_none()
    };
    // Trimmed inclusive row ranges; None = an empty line.
    let mut segments: Vec<Option<(usize, usize)>> = Vec::new();
    let mut t = 0;
    for r in 0..=h {
        if r < h && !sep_row(r) {
            continue;
        }
        let (mut a, mut b) = (t, r); // rows t..r, b exclusive
        while a < b && blank_row(a) {
            a += 1;
        }
        while b > a && blank_row(b - 1) {
            b -= 1;
        }
        segments.push((a < b).then(|| (a, b - 1)));
        t = r + 1;
    }
    let mut out: Row = Vec::new();
    for (k, seg) in segments.iter().enumerate() {
        if k > 0 {
            out.push(Node::Break);
        }
        let Some(&(t, b)) = seg.as_ref() else {
            continue; // empty line
        };
        if (t..=b).any(blank_row) {
            return err("stacked formula lines need a lone ┈ separator row", t, 0);
        }
        let rect = Rect {
            t,
            b,
            l: 0,
            r: width - 1,
        };
        if let Some(rect) = trim(&g, rect) {
            out.extend(parse_region(&g, rect, None)?)
        }
    }
    // Normalize: a script arg that mixes padded structures with atoms
    // parses as adjacent script chunks; merging them restores the
    // canonical single node (render is only defined on normal forms).
    Ok(crate::ast::normalize(&out))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::normalize;
    use crate::latex::row_to_latex;
    use crate::render::{RenderCtx, render_row};
    use crate::symbols::Accent;

    fn roundtrip(row: &Row) {
        // Canonical AA is defined on normal forms.
        let row = normalize(row);
        let aa = render_row(&row, None, false, &RenderCtx::canonical()).to_text();
        let parsed = parse(&aa).unwrap_or_else(|e| panic!("{}\n---\n{}", e, aa));
        assert_eq!(parsed, row, "AST mismatch for:\n{}", aa);
        let aa2 = render_row(&parsed, None, false, &RenderCtx::canonical()).to_text();
        assert_eq!(aa2, aa, "re-render mismatch");
    }

    fn syms(s: &str) -> Row {
        s.chars().map(Node::Sym).collect()
    }

    #[test]
    fn roundtrips_atoms_and_scripts() {
        roundtrip(&syms("x+1"));
        roundtrip(&vec![Node::Sym('x'), Node::Sup { arg: syms("2") }]);
        roundtrip(&vec![Node::Sym('x'), Node::Sub { arg: syms("i") }]);
        roundtrip(&vec![Node::Sym('e'), Node::Sup { arg: syms("απ") }]);
        roundtrip(&vec![Node::Sup { arg: syms("2") }]); // leading script
    }

    #[test]
    fn roundtrips_structures() {
        roundtrip(&vec![Node::Frac {
            num: syms("1"),
            den: syms("x+1"),
        }]);
        roundtrip(&vec![Node::Sqrt {
            arg: syms("2"),
            index: crate::symbols::Radical::Sqrt,
        }]);
        roundtrip(&vec![Node::Sqrt {
            arg: syms("x+1"),
            index: crate::symbols::Radical::Cbrt,
        }]);
        roundtrip(&vec![Node::Accent {
            overs: vec![Accent::Hat],
            unders: vec![],
            base: 'x',
        }]);
        roundtrip(&vec![Node::Accent {
            overs: vec![],
            unders: vec![Accent::Underline],
            base: 'y',
        }]);
        roundtrip(&vec![Node::Func("sin".into()), Node::Sym('x')]);
        use crate::symbols::ColDelim as C;
        use crate::symbols::Delim as D;
        roundtrip(&vec![Node::Delim {
            left: D::Col(C::Bracket),
            right: D::Col(C::Bracket),
            mids: 0,
            segs: vec![vec![Node::Array {
                rows: 2,
                cols: 2,
                cells: vec![syms("a"), syms("b+1"), vec![], syms("d")],
            }]],
        }]);
        roundtrip(&vec![Node::Delim {
            left: D::Col(C::Paren),
            right: D::Col(C::Paren),
            mids: 0,
            segs: vec![syms("a+b")],
        }]);
        roundtrip(&vec![Node::BigOpSym {
            op: '∑',
            lower: syms("i=0"),
            upper: syms("n"),
        }]);
        roundtrip(&vec![Node::BigOpSym {
            op: '∫',
            lower: vec![],
            upper: vec![],
        }]);
    }

    #[test]
    fn parses_handwritten_ascii() {
        // Lone ASCII letters are italic variables…
        let row = parse("x+1").unwrap();
        assert_eq!(row_to_latex(&row), "x+1");
        let row = parse("a sin y").unwrap();
        assert_eq!(row_to_latex(&row), "a\\operatorname{sin}y");
        // …but letter *runs* are \mathrm unless they are dictionary words,
        // and a single letter glued to another letter is roman too (d𝑦).
        let row = parse("asiny").unwrap();
        assert_eq!(row_to_latex(&row), "\\operatorname{asiny}");
        let row = parse("E=mc²").unwrap();
        assert_eq!(row_to_latex(&row), "E=\\operatorname{mc}^{2}");
        let row = parse("d𝑦").unwrap();
        assert_eq!(row_to_latex(&row), "\\mathrm{d}y");
        // 'single quotes' force the one-letter \mathrm; the prime is
        // its own atom ′ (a bare ' is always a quote delimiter).
        let row = parse("'d'x").unwrap();
        assert_eq!(row_to_latex(&row), "\\mathrm{d}x");
        let row = parse("𝑥′′").unwrap();
        assert_eq!(row_to_latex(&row), "x\\prime \\prime ");
        assert!(parse("𝑥''").is_err(), "an unclosed quote is an error");
    }

    /// An accent's base is always a `Sym` atom. A `Roman` base is not
    /// merely untyped — it has no picture: the canonical upright form
    /// is quoted (`\'d\'`), and a bare `d` reads back as the italic
    /// variable, so such a node could never round-trip.
    #[test]
    fn accent_bases_are_sym_atoms() {
        let row = parse("˰\n𝑑").unwrap();
        assert!(matches!(&row[0], Node::Accent { base: 'd', .. }));
        for unrepresentable in ["˰\nd", " ˰\n'd'"] {
            assert!(parse(unrepresentable).is_err(), "{:?}", unrepresentable);
        }
        // …and whatever base does get built is an accepted atom.
        for aa in ["˰\n𝑑", "․\nα", "˷\n∞"] {
            let Node::Accent { base, .. } = &parse(aa).unwrap()[0] else {
                panic!("{} is not an accent", aa)
            };
            assert!(crate::symbols::is_atom(*base), "{:?}", base);
        }
    }

    #[test]
    fn latex_unsafe_ascii_is_rejected() {
        // `~ ^ ` \` have no atom meaning and would be LaTeX syntax; the
        // symbols are \sim \backslash … `# $ % &` stay atoms and get
        // escaped on the way out.
        for bad in ["a~b", "a^b", "a`b", "a\\b"] {
            assert!(parse(bad).is_err(), "{} must be rejected", bad);
        }
        assert_eq!(row_to_latex(&parse("50%").unwrap()), "50\\%");
        assert_eq!(row_to_latex(&parse("a&b").unwrap()), "a\\&b");
        // The ∼ operator is the atom one actually wants.
        let row = parse("a∼b").unwrap();
        assert_eq!(row_to_latex(&row), "a\\sim b");
    }

    #[test]
    fn ambiguity_counterexample_is_now_distinguishable() {
        // The two ASTs that rendered identically before the band notation.
        let ast1 = vec![Node::BigOpSym {
            op: '∑',
            lower: syms("n=1"),
            upper: vec![Node::BigOpSym {
                op: '∫',
                lower: vec![],
                upper: vec![],
            }],
        }];
        let ast2 = vec![Node::BigOpSym {
            op: '∫',
            lower: vec![Node::BigOpSym {
                op: '∑',
                lower: syms("n=1"),
                upper: vec![],
            }],
            upper: vec![],
        }];
        let ctx = RenderCtx::canonical();
        let a = render_row(&ast1, None, false, &ctx).to_text();
        let b = render_row(&ast2, None, false, &ctx).to_text();
        assert_ne!(a, b);
        roundtrip(&ast1);
        roundtrip(&ast2);
    }
}
