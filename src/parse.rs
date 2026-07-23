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
use crate::render::{
    DOUBLE_BODY, FRAC_BAR, OP_BAND, PLACEHOLDER, lattice_char, unstyle_char, unsubscript_char,
    unsuperscript_char,
};

/// Left-edge glyphs of a grid lattice column (see render::LATTICE_CHARS).
const LATTICE_LEFT: &[char] = &['┌', '├', '└']; // U+250C/251C/2514
const LATTICE_TOP: &[char] = &['┌', '┬', '┐'];

const RADICALS: &[(char, u8)] = &[('√', 2), ('∛', 3), ('∜', 4)];

fn radical_index(c: char) -> Option<u8> {
    RADICALS.iter().find(|&&(r, _)| r == c).map(|&(_, i)| i)
}

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

/// A structural region discovered during parsing: (top, bottom, left,
/// right) grid rows/cols (inclusive) and its nesting depth. Used by the
/// TUI structure view to paint blocks by depth.
pub type RegionSpan = ((usize, usize, usize, usize), usize);

pub struct Grid {
    g: Vec<Vec<char>>,
    /// Cells carrying a combining long solidus (U+0338) = \cancel strike.
    cancel: Vec<Vec<bool>>,
}

impl Grid {
    fn at(&self, r: usize, c: usize) -> char {
        self.g[r][c]
    }

    fn cancelled(&self, r: usize, c: usize) -> bool {
        self.cancel[r][c]
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

/// Side-distinct delimiter glyphs (each char appears on exactly one side;
/// the shared extension ⎪ and the angle arms ╱ ╲ are deliberately absent).
/// Lattice edges (┌├└ / ┐┤┘) are included so a lattice interior is
/// bracket-protected against cell/script splitting like any other pair.
const OPEN_BRACKETS: &[char] = &[
    '(', '⎛', '⎜', '⎝', '[', '⎡', '⎢', '⎣', '{', '⎧', '⎨', '⎩', '⟨', '┆', '┌', '├', '└', '⌈', '⌊',
];
const CLOSE_BRACKETS: &[char] = &[
    ')', '⎞', '⎟', '⎠', ']', '⎤', '⎥', '⎦', '}', '⎫', '⎬', '⎭', '⟩', '┊', '┐', '┤', '┘', '⌉', '⌋',
];

/// Delimiter spec char for a glyph that can appear on the *baseline row*
/// of a left delimiter column. Brace/angle columns always show their
/// vertex (⎨ / ⟨) on the baseline, so ⎧ ⎪ ⎩ ╱ ╲ never occur here.
fn open_spec(c: char) -> Option<char> {
    Some(match c {
        '(' | '⎛' | '⎜' | '⎝' => '(',
        '[' | '⎡' | '⎢' | '⎣' => '[',
        '⌈' => '⌈',
        '⌊' => '⌊',
        '‖' => '‖',
        '{' | '⎨' => '{',
        '⟨' => '⟨',
        '┆' => '.',
        // ├ (fused-grid junction) resolves its family via the column walk
        // in open_spec_at; standalone it is a lattice edge.
        '├' => return None,
        _ => return None,
    })
}

/// Fused-grid markers of a delimiter block, if its interior is one.
/// Canonical shapes: pure ┼-rows (multi-row × multi-col), ┬/┴ edge rows
/// (one row), ├/┤ junctions in the delimiter columns (one column) — and
/// any mix of these is accepted as input. A "pure" row contains nothing
/// but blanks and its marker (a nested structure always shows some other
/// glyph on that row). Returns (marker_cols, marker_rows, t, b) with the
/// cell area rows t..=b.
#[allow(clippy::type_complexity)]
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
    if let Some(cs) = pure(top, '┬') {
        cols_marks = Some(cs);
        t = top + 1;
    }
    if let Some(cs) = pure(bot, '┴') {
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
        if let Some(cs) = pure(r, '┼') {
            match &cols_marks {
                Some(prev) if *prev != cs => return None,
                _ => cols_marks = Some(cs),
            }
            marker_rows.push(r);
        } else if g.at(r, col) == '├' || g.at(r, close) == '┤' {
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
fn open_spec_at(g: &Grid, row: usize, col: usize) -> Option<char> {
    let ch = g.at(row, col);
    if angle_open_turn(g, row, col) {
        return Some('⟨');
    }
    if !matches!(ch, '⎡' | '⎢' | '⎣') && !(ch == '├' && fused_junction(g, row, col)) {
        return open_spec(ch);
    }
    let h = g.g.len();
    let in_run = |c: char| {
        matches!(
            c,
            '⎡' | '⎢' | '⎣' | '├' | '⎪' | '⎛' | '⎜' | '⎝' | '⎧' | '⎨' | '⎩' | '┆'
        )
    };
    let mut top = row;
    while top > 0 && in_run(g.at(top - 1, col)) {
        top -= 1;
    }
    let mut bot = row;
    while bot + 1 < h && in_run(g.at(bot + 1, col)) {
        bot += 1;
    }
    let has = |c: char| (top..=bot).any(|r| g.at(r, col) == c);
    if has('⎛') || has('⎜') || has('⎝') {
        return Some('(');
    }
    if has('⎧') || has('⎨') || has('⎩') {
        return Some('{');
    }
    if has('┆') {
        return Some('.');
    }
    Some(match (has('⎡'), has('⎣')) {
        (true, true) => '[',
        (true, false) => '⌈',
        (false, true) => '⌊',
        // A cornerless extension run is the vertical bar (\abs).
        (false, false) => '|',
    })
}

fn close_spec(c: char) -> Option<char> {
    Some(match c {
        ')' | '⎞' | '⎟' | '⎠' => ')',
        ']' | '⎤' | '⎥' | '⎦' => ']',
        '⌉' => '⌉',
        '⌋' => '⌋',
        '‖' => '‖',
        '}' | '⎬' => '}',
        '⟩' => '⟩',
        '┊' => '.',
        _ => return None,
    })
}

/// Like `close_spec`, resolving a fused-grid junction (┤) by walking its
/// column to a family-distinct glyph (through ⎪ and further junctions).
fn close_spec_at(g: &Grid, row: usize, col: usize) -> Option<char> {
    let ch = g.at(row, col);
    if angle_close_turn(g, row, col) {
        return Some('⟩');
    }
    let h = g.g.len();
    if !matches!(ch, '⎤' | '⎥' | '⎦') && !(ch == '┤' && fused_junction(g, row, col)) {
        return close_spec(ch);
    }
    let in_run = |c: char| {
        matches!(
            c,
            '⎤' | '⎥' | '⎦' | '┤' | '⎪' | '⎞' | '⎟' | '⎠' | '⎫' | '⎬' | '⎭' | '┊'
        )
    };
    let mut top = row;
    while top > 0 && in_run(g.at(top - 1, col)) {
        top -= 1;
    }
    let mut bot = row;
    while bot + 1 < h && in_run(g.at(bot + 1, col)) {
        bot += 1;
    }
    let has = |c: char| (top..=bot).any(|r| g.at(r, col) == c);
    if has('⎞') || has('⎟') || has('⎠') {
        return Some(')');
    }
    if has('⎫') || has('⎬') || has('⎭') {
        return Some('}');
    }
    if has('┊') {
        return Some('.');
    }
    Some(match (has('⎤'), has('⎦')) {
        (true, true) => ']',
        (true, false) => '⌉',
        (false, true) => '⌋',
        (false, false) => '|',
    })
}

/// Every glyph a left delimiter column of `spec` can contain (for the
/// vertical-extent scan).
fn left_family(spec: char) -> &'static [char] {
    match spec {
        '(' => &['(', '⎛', '⎜', '⎝', '├'],
        '[' => &['[', '⎡', '⎢', '⎣', '├'],
        '⌈' => &['⌈', '⎡', '⎢', '├'],
        '⌊' => &['⌊', '⎢', '⎣', '├'],
        '‖' => &['‖'],
        '{' => &['{', '⎧', '⎪', '⎨', '⎩'],
        '⟨' => &['⟨'],
        '|' => &['⎢', '⎥', '├'],
        _ => &['┆', '├'],
    }
}

/// A ├ / ┤ (U+251C/2524) dug into a delimiter column as a fused-grid
/// row junction: the contiguous column run continues with delimiter
/// glyphs above/below. A bare-lattice edge marker has blank gaps
/// instead, so it never classifies.
fn fused_junction(g: &Grid, row: usize, col: usize) -> bool {
    let (junction, family): (char, &str) = match g.at(row, col) {
        '├' => ('├', "⎛⎜⎝⎡⎢⎣⎧⎪⎨⎩⎸┆"),
        '┤' => ('┤', "⎞⎟⎠⎤⎥⎦⎫⎬⎭⎹┊"),
        _ => return false,
    };
    let in_run = |c: char| c == junction || family.contains(c);
    let mut r = row;
    while r > 0 && in_run(g.at(r - 1, col)) {
        r -= 1;
    }
    let top = r;
    let mut r = row;
    while r + 1 < g.g.len() && in_run(g.at(r + 1, col)) {
        r += 1;
    }
    (top..=r).any(|rr| family.contains(g.at(rr, col)))
}

/// Which side of a delimiter pair the glyph at (row, col) belongs to:
/// Some(true) = left/open, Some(false) = right/close, None = neither.
/// The shared glyphs (brace extension ⎪, angle arms ╱ ╲) resolve their
/// side by walking the contiguous column run to a side-distinct glyph —
/// without this, depth counting desyncs on rows that cut through a
/// mismatched pair (e.g. a { … ┆ cases block).
/// Tall angles are pure diagonals; the turn is a same-column vertical
/// pair. Left angle: ╱ directly above ╲; right angle: ╲ directly above
/// ╱. The upper turn row is the baseline.
fn angle_open_turn(g: &Grid, row: usize, col: usize) -> bool {
    let width = |r: usize| g.g[r].len();
    g.at(row, col) == '╱'
        && row + 1 < g.g.len()
        && col < width(row + 1)
        && g.at(row + 1, col) == '╲'
        // A hat accent band (┈╱╲┈) fakes this pair when its ╱ sits right
        // above another band's ╲. The hat's ╱ always has its partner ╲
        // directly to its right; a real turn's interior never starts
        // with ╲ (a ┈ band may hug the turn, so ┈ stays allowed).
        && (col + 1 >= width(row) || g.at(row, col + 1) != '╲')
}

fn angle_close_turn(g: &Grid, row: usize, col: usize) -> bool {
    let width = |r: usize| g.g[r].len();
    g.at(row, col) == '╲'
        && row + 1 < g.g.len()
        && col < width(row + 1)
        && g.at(row + 1, col) == '╱'
        // Mirror of the open-turn guard: a stacked hat's ╲ has its
        // partner ╱ directly to its left; a real turn's interior never
        // ends with ╱ (a ┈ band may hug the turn, so ┈ stays allowed).
        && (col == 0 || g.at(row, col - 1) != '╱')
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
        '╱' => {
            // Left upper arm: follow ╱ down-left; the turn shows as
            // ╱-over-╲ at some cell of the run.
            let (mut r, mut c) = (row, col);
            loop {
                if angle_open_turn(g, r, c) {
                    return Some(true);
                }
                if c > 0 && at(r + 1, c - 1) == '╱' {
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
                if r > 0 && at(r - 1, c + 1) == '╱' {
                    r -= 1;
                    c += 1;
                } else {
                    break;
                }
            }
            None
        }
        '╲' => {
            // Right upper arm: follow ╲ down-right to a ╲-over-╱ turn.
            let (mut r, mut c) = (row, col);
            loop {
                if angle_close_turn(g, r, c) {
                    return Some(false);
                }
                if at(r + 1, c + 1) == '╲' {
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
                if r > 0 && c > 0 && at(r - 1, c - 1) == '╲' {
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

fn delim_side(g: &Grid, row: usize, col: usize) -> Option<bool> {
    let ch = g.at(row, col);
    // The sqrt overline corner (┌ directly above the radical stem) is
    // not a delimiter: its row has no matching close, and counting it
    // would desync the depth scan.
    if ch == '┌' && row + 1 < g.g.len() && matches!(g.at(row + 1, col), '│' | '√' | '∛' | '∜')
    {
        return None;
    }
    // Norm columns use the same ‖ on both sides: parity along the row
    // decides (full-height columns keep the parity consistent per row).
    // Direct norm-in-norm is therefore unsupported.
    if ch == '‖' {
        let before = (0..col).filter(|&c2| g.at(row, c2) == '‖').count();
        return Some(before % 2 == 0);
    }
    if OPEN_BRACKETS.contains(&ch) {
        return Some(true);
    }
    if CLOSE_BRACKETS.contains(&ch) {
        return Some(false);
    }
    if let s @ Some(_) = angle_arm_side(g, row, col) {
        return s;
    }
    let family: &[char] = match ch {
        '⎪' => &['⎧', '⎨', '⎩', '⎫', '⎬', '⎭', '⎪'],
        _ => return None,
    };
    let mut r = row;
    while r > 0 && family.contains(&g.at(r - 1, col)) {
        r -= 1;
    }
    while r < g.g.len() && family.contains(&g.at(r, col)) {
        let c2 = g.at(r, col);
        if OPEN_BRACKETS.contains(&c2) {
            return Some(true);
        }
        if CLOSE_BRACKETS.contains(&c2) {
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
fn protected_cols(g: &Grid, rect: Rect, skip_row: Option<usize>) -> Vec<bool> {
    let w = rect.r - rect.l + 1;
    let mut protected = vec![false; w];
    for r in rect.rows() {
        if Some(r) == skip_row {
            continue;
        }
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
    match g.at(first, c) {
        // Delimiter columns: a fused grid (├ junctions in this column or
        // ┬ markers on the top row) centers on the extent; otherwise the
        // baseline is that of the inner region (a lattice inside answers
        // with its own ┌├└ center rule).
        '⎡' | '[' | '⎛' | '┆' | '⎢' | '‖' => {
            if let Ok(close) = match_delim(g, first, c, rect.r)
                && fused_grid_markers(g, first, last, c, close).is_some()
            {
                return Ok((first + last) / 2);
            }
            find_baseline(
                g,
                Rect {
                    t: first,
                    b: last,
                    l: c + 1,
                    r: rect.r,
                },
            )
        }
        '(' => Ok(first),
        // Brace columns carry their vertex on the baseline row.
        '⎧' | '⎪' | '⎨' | '⎩' => occupied
            .iter()
            .find(|&&r| g.at(r, c) == '⎨')
            .copied()
            .ok_or(())
            .or_else(|_| err("brace column without ⎨", first, c)),
        // Angle: the ⟨ vertex (one-line form) or the diagonal turn
        // pair — ╱ directly above ╲ in the leftmost column. The upper
        // turn row is the baseline.
        '⟨' | '╱' | '╲' => occupied
            .iter()
            .find(|&&r| g.at(r, c) == '⟨' || angle_open_turn(g, r, c))
            .copied()
            .ok_or(())
            .or_else(|_| err("angle column without a vertex or turn", first, c)),
        // Lattice left-edge column: the grid centers on its extent.
        // A ┌ directly above a radical stem is the sqrt overline corner
        // (a lattice ┌ has a blank gap below instead): dive into the
        // radicand right of the stem.
        '┌' if first < last && matches!(g.at(first + 1, c), '│' | '√' | '∛' | '∜') => {
            find_baseline(
                g,
                Rect {
                    t: first + 1,
                    b: last,
                    l: c + 1,
                    r: rect.r,
                },
            )
        }
        '┌' | '├' | '└' => Ok((first + last) / 2),
        // Accent band: the base owns the baseline on the other side
        // (over marks ride above their base, under marks below).
        _ if accent_band_run(g, rect, first, c, true).is_some() => find_baseline(
            g,
            Rect {
                t: first + 1,
                b: rect.b,
                l: c,
                r: rect.r,
            },
        ),
        _ if first > rect.t && accent_band_run(g, rect, first, c, false).is_some() => {
            find_baseline(
                g,
                Rect {
                    t: rect.t,
                    b: first - 1,
                    l: c,
                    r: rect.r,
                },
            )
        }
        // Over/under brace corner: the argument owns the baseline on the
        // other side of the brace row.
        '╭' => find_baseline(
            g,
            Rect {
                t: first + 1,
                b: rect.b,
                l: c,
                r: rect.r,
            },
        ),
        '╰' if first > rect.t => find_baseline(
            g,
            Rect {
                t: rect.t,
                b: first - 1,
                l: c,
                r: rect.r,
            },
        ),
        // Sqrt: stem covers exactly the content rows; recurse into content.
        '│' => find_baseline(
            g,
            Rect {
                t: first,
                b: last,
                l: c + 1,
                r: rect.r,
            },
        ),
        ch if radical_index(ch).is_some() => find_baseline(
            g,
            Rect {
                t: first,
                b: last,
                l: c + 1,
                r: rect.r,
            },
        ),
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
#[allow(clippy::too_many_arguments)]
fn parse_region(
    g: &Grid,
    rect: Rect,
    baseline: Option<usize>,
    depth: usize,
    trace: &mut Vec<RegionSpan>,
    in_cancel: bool,
) -> Result<Row> {
    let rect = match trim(g, rect) {
        Some(r) => r,
        None => return Ok(vec![]),
    };
    trace.push(((rect.t, rect.b, rect.l, rect.r), depth));
    let bl = match baseline {
        Some(b) => b,
        None => find_baseline(g, rect)?,
    };

    let mut out: Row = Vec::new();
    let mut col = rect.l;
    while col <= rect.r {
        // Struck-through block. The extent follows the canonical "maximal
        // cancel" form: runs of flagged baseline cells, extended across
        // blank-baseline stretches only when everything there is struck
        // too (a partially struck script belongs to a sibling, and its own
        // Cancel is found when the script argument is parsed).
        if !in_cancel && g.cancelled(bl, col) {
            let fully_struck = |l: usize, r: usize| {
                (l..=r).all(|c| {
                    rect.rows()
                        .all(|row| g.at(row, c) == ' ' || g.cancelled(row, c))
                })
            };
            let mut end = col;
            loop {
                while end < rect.r && g.cancelled(bl, end + 1) {
                    end += 1;
                }
                // Next baseline content after the blank-baseline stretch.
                let mut j = end + 1;
                while j <= rect.r && g.at(bl, j) == ' ' {
                    j += 1;
                }
                // Struck baseline continues past a fully struck stretch
                // (spaced operators or struck scripts inside the argument).
                if j <= rect.r && g.cancelled(bl, j) && fully_struck(end + 1, j - 1) {
                    end = j;
                    continue;
                }
                // Otherwise absorb trailing fully struck script segments.
                let mut c = end + 1;
                let mut extended = false;
                loop {
                    while c < j.min(rect.r + 1) && col_blank(g, rect, c) {
                        c += 1;
                    }
                    if c >= j || c > rect.r {
                        break;
                    }
                    let s0 = c;
                    let mut s1 = c;
                    while s1 + 1 < j && !col_blank(g, rect, s1 + 1) {
                        s1 += 1;
                    }
                    if fully_struck(s0, s1) {
                        end = s1;
                        extended = true;
                        c = s1 + 1;
                    } else {
                        break;
                    }
                }
                if !extended {
                    break;
                }
            }
            let inner = Rect {
                l: col,
                r: end,
                ..rect
            };
            let arg = parse_region(g, inner, Some(bl), depth + 1, trace, true)?;
            out.push(Node::Cancel { arg });
            col = end + 1;
            continue;
        }
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
                            parse_script_run(
                                g,
                                rect,
                                bl,
                                col,
                                start - 1,
                                depth,
                                trace,
                                in_cancel,
                                &mut out,
                            )?;
                        }
                        let (node, right) = match kind {
                            Special::Lattice => {
                                parse_lattice(g, rect, start, depth, trace, in_cancel)?
                            }
                            Special::Brace(r, over, right) => parse_brace(
                                g, rect, bl, start, r, over, right, depth, trace, in_cancel,
                            )?,
                            Special::Accent(r, over, right) => parse_wide_accent(
                                g, rect, bl, start, r, over, right, depth, trace, in_cancel,
                            )?,
                        };
                        out.push(node);
                        col = right + 1;
                        continue;
                    }
                    None => run_end,
                };
                parse_script_run(g, rect, bl, col, run_end, depth, trace, in_cancel, &mut out)?;
                col = run_end + 1;
            }
            // A ├ inside a delimiter column run is a fused-grid row
            // junction, not a bare-lattice edge — the delimiter arm
            // below handles it.
            '┌' | '├' | '└' if !fused_junction(g, bl, col) => {
                let (node, right) = parse_lattice(g, rect, col, depth, trace, in_cancel)?;
                out.push(node);
                col = right + 1;
            }
            _ if ch == FRAC_BAR => {
                // Maximal munch: a ─ run capped by → is a labeled arrow;
                // a fraction next to an arrow *atom* renders with a space
                // between (presence of the space is the disambiguator).
                let run_end = scan_while(g, bl, col, rect.r, |c| c == FRAC_BAR);
                if run_end < rect.r && matches!(g.at(bl, run_end + 1), '>' | '→') {
                    let span = Rect {
                        t: rect.t,
                        b: rect.b,
                        l: col,
                        r: run_end + 1,
                    };
                    let over = region_above(span, bl).map_or(Ok(vec![]), |r| {
                        parse_region(g, r, None, depth + 1, trace, in_cancel)
                    })?;
                    let under = region_below(span, bl).map_or(Ok(vec![]), |r| {
                        parse_region(g, r, None, depth + 1, trace, in_cancel)
                    })?;
                    out.push(Node::Arrow {
                        op: '→',
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
                let num = region_above(span, bl).map_or(Ok(vec![]), |r| {
                    parse_region(g, r, None, depth + 1, trace, in_cancel)
                })?;
                let den = region_below(span, bl).map_or(Ok(vec![]), |r| {
                    parse_region(g, r, None, depth + 1, trace, in_cancel)
                })?;
                out.push(Node::Frac { num, den });
                col = run_end + 1;
            }
            _ if ch == OP_BAND => {
                // General band: ┈+ (piece ┈+)+ — anything sandwiched in ┈
                // without spaces takes over/under limits (∑, lim, arg┈max
                // …). Pieces are flat one-line rows joined into the base.
                let (pieces, end) = scan_band(g, rect, bl, col, OP_BAND)?;
                if pieces.is_empty() {
                    return err("band without content", bl, col);
                }
                let mut base: Row = Vec::new();
                for (l0, r0) in pieces {
                    let prect = Rect {
                        t: bl,
                        b: bl,
                        l: l0,
                        r: r0,
                    };
                    base.extend(parse_region(
                        g,
                        prect,
                        Some(bl),
                        depth + 1,
                        trace,
                        in_cancel,
                    )?);
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
                let upper = region_above(span, bl).map_or(Ok(vec![]), |r| {
                    parse_region(g, r, None, depth + 1, trace, in_cancel)
                })?;
                let lower = region_below(span, bl).map_or(Ok(vec![]), |r| {
                    parse_region(g, r, None, depth + 1, trace, in_cancel)
                })?;
                out.push(Node::BigOp { base, lower, upper });
                col = end + 1;
            }
            _ if ch == DOUBLE_BODY => {
                // Double-arrow body: ═ run capped by ⇒ (═ has no other use).
                let run_end = scan_while(g, bl, col, rect.r, |c| c == DOUBLE_BODY);
                if run_end == rect.r || !matches!(g.at(bl, run_end + 1), '>' | '⇒') {
                    return err("═ run without a > head", bl, col);
                }
                let span = Rect {
                    t: rect.t,
                    b: rect.b,
                    l: col,
                    r: run_end + 1,
                };
                let over = region_above(span, bl).map_or(Ok(vec![]), |r| {
                    parse_region(g, r, None, depth + 1, trace, in_cancel)
                })?;
                let under = region_below(span, bl).map_or(Ok(vec![]), |r| {
                    parse_region(g, r, None, depth + 1, trace, in_cancel)
                })?;
                out.push(Node::Arrow {
                    op: '⇒',
                    over,
                    under,
                });
                col = run_end + 2;
            }
            '<' | '←' | '⇐'
                if col < rect.r
                    && ((matches!(ch, '<' | '←') && g.at(bl, col + 1) == FRAC_BAR)
                        || (matches!(ch, '<' | '⇐') && g.at(bl, col + 1) == DOUBLE_BODY)) =>
            {
                // Left-pointing labeled arrow: head, then the body run.
                let body = g.at(bl, col + 1);
                let run_end = scan_while(g, bl, col + 1, rect.r, |c| c == body);
                let span = Rect {
                    t: rect.t,
                    b: rect.b,
                    l: col,
                    r: run_end,
                };
                let over = region_above(span, bl).map_or(Ok(vec![]), |r| {
                    parse_region(g, r, None, depth + 1, trace, in_cancel)
                })?;
                let under = region_below(span, bl).map_or(Ok(vec![]), |r| {
                    parse_region(g, r, None, depth + 1, trace, in_cancel)
                })?;
                let op = if body == DOUBLE_BODY { '⇐' } else { '←' };
                out.push(Node::Arrow { op, over, under });
                col = run_end + 1;
            }
            '"' => {
                // "double-quoted" \text run: flat baseline chars up to
                // the closing quote; ␣ maps back to a space.
                let mut close = None;
                for c2 in col + 1..=rect.r {
                    if g.at(bl, c2) == '"' {
                        close = Some(c2);
                        break;
                    }
                }
                let Some(close) = close else {
                    return err("unclosed \"", bl, col);
                };
                check_flat_columns(g, rect, bl, col, close, 0, 0)?;
                let t: String = (col + 1..close)
                    .map(|c2| match g.at(bl, c2) {
                        '␣' => ' ',
                        c2 => c2,
                    })
                    .collect();
                out.push(Node::Text { t, math: false });
                col = close + 1;
            }
            '\'' => {
                // 'single-quoted' \mathrm run — but only when a closing
                // quote exists and the content is nonempty ASCII
                // alphanumerics/␣; anything else keeps ' as the prime
                // atom (canonical output spaces primes away from letter
                // runs and quotes, so the two never collide).
                let quoted = (col + 1..=rect.r)
                    .take_while(|&c2| g.at(bl, c2) != ' ')
                    .find(|&c2| g.at(bl, c2) == '\'')
                    .filter(|&close| {
                        close > col + 1
                            && (col + 1..close).all(|c2| {
                                let ch2 = g.at(bl, c2);
                                ch2.is_ascii_alphanumeric() || ch2 == '␣' || ch2 == '.'
                            })
                    });
                match quoted {
                    Some(close) => {
                        check_flat_columns(g, rect, bl, col, close, 0, 0)?;
                        let t: String = (col + 1..close)
                            .map(|c2| match g.at(bl, c2) {
                                '␣' => ' ',
                                c2 => c2,
                            })
                            .collect();
                        out.push(Node::Text { t, math: true });
                        col = close + 1;
                    }
                    None => {
                        // A prime is an ordinary atom — including under
                        // an accent stack (a wide accent collapsing onto
                        // a one-char base can put marks on anything).
                        let (overs, unders, extra) = accent_stacks(g, rect, bl, col);
                        check_flat_columns(g, rect, bl, col, col, overs.len(), unders.len())?;
                        if overs.is_empty() && unders.is_empty() {
                            out.push(Node::Sym('\''));
                        } else {
                            out.push(Node::Accent {
                                overs,
                                unders,
                                base: '\'',
                            });
                        }
                        col += 1 + extra;
                    }
                }
            }
            ')' | ']' | '}' | '⟩' => {
                // Closing delimiters are never atoms: a `)` atom inside
                // any delimiter would be captured by the mismatched-pair
                // depth scan (`{y)}` would close at `)`), so a stray
                // close is an error rather than a silently shifted read.
                return err(format!("unmatched {}", ch), bl, col);
            }
            _ if open_spec_at(g, bl, col).is_some() => {
                let (node, close_col) = parse_delim(g, rect, bl, col, depth, trace, in_cancel)?;
                out.push(node);
                col = close_col + 1;
            }
            '│' | '√' | '∛' | '∜' => {
                // Stem covers exactly the content rows; the radical glyph
                // (√ ∛ ∜) is its bottom cell.
                let mut top = bl;
                while top > rect.t && g.at(top - 1, col) == '│' {
                    top -= 1;
                }
                let mut bot = bl;
                while radical_index(g.at(bot, col)).is_none() {
                    if bot == rect.b {
                        return err("radical stem without √", bl, col);
                    }
                    bot += 1;
                }
                if top == 0 || g.at(top - 1, col) != '┌' {
                    return err("radical without its ┌─ overline", top, col);
                }
                let index = radical_index(g.at(bot, col)).unwrap();
                let w = scan_while(g, top - 1, col + 1, rect.r, |c| c == FRAC_BAR) - col;
                let inner = Rect {
                    t: top,
                    b: bot,
                    l: col + 1,
                    r: col + w,
                };
                out.push(Node::Sqrt {
                    arg: parse_region(g, inner, Some(bl), depth + 1, trace, in_cancel)?,
                    index,
                });
                col += w + 1;
            }
            _ if ch == PLACEHOLDER => {
                check_flat_columns(g, rect, bl, col, col, 0, 0)?;
                col += 1;
            }
            _ if unsuperscript_char(ch).is_some() => {
                let run_end =
                    scan_while_same_flag(g, bl, col, rect.r, |c| unsuperscript_char(c).is_some());
                check_flat_columns(g, rect, bl, col, run_end, 0, 0)?;
                let arg = (col..=run_end)
                    .map(|c| Node::Sym(unsuperscript_char(g.at(bl, c)).unwrap()))
                    .collect();
                out.push(Node::Sup { arg });
                col = run_end + 1;
            }
            _ if unsubscript_char(ch).is_some() => {
                let run_end =
                    scan_while_same_flag(g, bl, col, rect.r, |c| unsubscript_char(c).is_some());
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
                let mut run_end =
                    scan_while_same_flag(g, bl, col, rect.r, |c| c.is_ascii_alphabetic());
                let flag = g.cancelled(bl, col);
                loop {
                    let dot = run_end + 1;
                    if dot > rect.r || g.at(bl, dot) != '.' || g.cancelled(bl, dot) != flag {
                        break;
                    }
                    let next = dot + 1;
                    if next <= rect.r
                        && g.at(bl, next).is_ascii_alphabetic()
                        && g.cancelled(bl, next) == flag
                    {
                        run_end =
                            scan_while_same_flag(g, bl, next, rect.r, |c| c.is_ascii_alphabetic());
                    } else if (col..=run_end).any(|c2| g.at(bl, c2) == '.') {
                        run_end = dot;
                        break;
                    } else {
                        break;
                    }
                }
                check_flat_columns(g, rect, bl, col, run_end, 0, 0)?;
                let word: String = (col..=run_end).map(|c| g.at(bl, c)).collect();
                if word.chars().count() == 1 {
                    let prev_letter = col > rect.l && g.at(bl, col - 1).is_alphabetic();
                    let next_letter = run_end < rect.r && g.at(bl, run_end + 1).is_alphabetic();
                    if prev_letter || next_letter {
                        out.push(Node::Text {
                            t: word,
                            math: true,
                        });
                    } else {
                        out.push(Node::Sym(ch));
                    }
                } else if !word.contains('.') && crate::symbols::is_func_name(&word) {
                    out.push(Node::Func(word));
                } else {
                    out.push(Node::Text {
                        t: word,
                        math: true,
                    });
                }
                col = run_end + 1;
            }
            _ => {
                // Marks in the cells directly above/below an atom are an
                // accent stack; those cells are otherwise always blank.
                let (overs, unders, extra) = accent_stacks(g, rect, bl, col);
                check_flat_columns(g, rect, bl, col, col, overs.len(), unders.len())?;
                let base = unstyle_char(ch);
                // ␠ is the explicit form of a formatting Spacer (the
                // session file uses it; fmt turns it back into a blank).
                if base == '␠' && overs.is_empty() && unders.is_empty() {
                    out.push(Node::Spacer);
                    col += 1;
                    continue;
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
    Ok(out)
}

/// Last column <= `max` such that all of `from..=result` satisfy `pred`.
fn scan_while(g: &Grid, row: usize, from: usize, max: usize, pred: impl Fn(char) -> bool) -> usize {
    let mut c = from;
    while c < max && pred(g.at(row, c + 1)) {
        c += 1;
    }
    c
}

/// Like `scan_while`, but the run must also keep the cancel flag of its
/// first cell (a struck token never merges with an unstruck neighbour).
fn scan_while_same_flag(
    g: &Grid,
    row: usize,
    from: usize,
    max: usize,
    pred: impl Fn(char) -> bool,
) -> usize {
    let flag = g.cancelled(row, from);
    let mut c = from;
    while c < max && pred(g.at(row, c + 1)) && g.cancelled(row, c + 1) == flag {
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
    depth: usize,
    trace: &mut Vec<RegionSpan>,
    in_cancel: bool,
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
        let protected = protected_cols(g, side, None);
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
    for (_, is_sup, r) in parts {
        let arg = parse_region(g, r, None, depth + 1, trace, in_cancel)?;
        out.push(if is_sup {
            Node::Sup { arg }
        } else {
            Node::Sub { arg }
        });
    }
    Ok(())
}

/// Scan a band starting at (bl, col): `B+ (piece B+)*` where B is the
/// band char. Returns the piece spans and the column of the final band
/// char. A piece not closed by another band char is an error (canonical
/// bands always are).
fn scan_band(
    g: &Grid,
    rect: Rect,
    bl: usize,
    col: usize,
    band: char,
) -> Result<(Vec<(usize, usize)>, usize)> {
    let mut pieces = Vec::new();
    let mut end = scan_while(g, bl, col, rect.r, |c| c == band);
    loop {
        let pstart = end + 1;
        if pstart > rect.r || g.at(bl, pstart) == ' ' {
            break;
        }
        let mut pend = pstart;
        while pend < rect.r && g.at(bl, pend + 1) != ' ' && g.at(bl, pend + 1) != band {
            pend += 1;
        }
        if pend == rect.r || g.at(bl, pend + 1) != band {
            return err("band piece without a closing band char", bl, pstart);
        }
        pieces.push((pstart, pend));
        end = scan_while(g, bl, pend + 1, rect.r, |c| c == band);
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
        .find(|&r| g.at(r, c) == '╭')
        .map(|r| (r, true))
        .or_else(|| {
            (bl + 1..=rect.b)
                .find(|&r| g.at(r, c) == '╰')
                .map(|r| (r, false))
        });
    let (brow, over) = cand?;
    let run_end = scan_while(g, brow, c, rect.r, |c2| c2 == FRAC_BAR);
    let closer = if over { '╮' } else { '╯' };
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
) -> Option<(char, usize)> {
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
            // The full material alphabet: ─ and > for the vec arrow,
            // the fill/centered drawn marks for everything else.
            && matches!(ch, '￫' | '_' | '¯' | '˜' | '˷' | '˰' | '˯' | '˳' | '․')
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
    let mark = if over && all('￫') {
        Some('⇀') // centered halfwidth arrow: \overrightarrow
    } else if over && all('˰') {
        Some('^') // centered low arrowhead: \widehat
    } else if over && all('˯') {
        Some('ˇ') // \widecheck
    } else if over && all('_') {
        Some('¯') // low fill hugging the base: \overline
    } else if !over && all('¯') {
        Some('‗') // high fill hugging from below: \underline
    } else if over && all('˷') {
        Some('˜') // low tilde fill: \widetilde
    } else if !over && all('˜') {
        Some('˷') // high tilde fill: \utilde
    } else if over && all('˳') {
        Some('˚') // centered low ring: \mathring
    } else if over && all('․') {
        // Leader dots: one is \dot, two are \ddot.
        match piece.len() {
            1 => Some('˙'),
            2 => Some('¨'),
            _ => None,
        }
    } else {
        None
    };
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
    depth: usize,
    trace: &mut Vec<RegionSpan>,
    in_cancel: bool,
) -> Result<(Node, usize)> {
    // A fully struck accent is \cancel content (the strike scan cannot
    // anchor on the band) — wrap it like a struck brace.
    let struck = !in_cancel
        && rect
            .rows()
            .all(|r| (col..=right).all(|c2| g.at(r, c2) == ' ' || g.cancelled(r, c2)));
    let in_cancel = in_cancel || struck;
    let (over, top) = if over_first {
        let (m, _) = accent_band_run(g, rect, brow, col, true).unwrap();
        (Some(m), brow + 1)
    } else {
        (None, rect.t)
    };
    // A matching under band below the baseline (when we started from an
    // over band) — or the anchoring band itself (under-only).
    let under_row = if over_first {
        (bl + 1..=rect.b).find(|&r| accent_band_run(g, rect, r, col, false).is_some())
    } else {
        Some(brow)
    };
    let (under, bot) = match under_row {
        Some(r) => {
            let (m, _) = accent_band_run(g, rect, r, col, false).unwrap();
            (Some(m), r - 1)
        }
        None => (None, rect.b),
    };
    let base_rect = Rect {
        t: top,
        b: bot,
        l: col,
        r: right,
    };
    let base = parse_region(g, base_rect, Some(bl), depth + 1, trace, in_cancel)?;
    let node = Node::WideAccent { over, under, base };
    let node = if struck {
        Node::Cancel { arg: vec![node] }
    } else {
        node
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
    depth: usize,
    trace: &mut Vec<RegionSpan>,
    in_cancel: bool,
) -> Result<(Node, usize)> {
    let cols = (col, right);
    // A fully struck brace is \cancel content (the cancel-extent scan
    // cannot always anchor on it); wrap and let normalize merge.
    let struck = !in_cancel
        && rect
            .rows()
            .all(|r| (col..=right).all(|c2| g.at(r, c2) == ' ' || g.cancelled(r, c2)));
    let in_cancel = in_cancel || struck;
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
    let arg = parse_region(g, arg_rect, Some(bl), depth + 1, trace, in_cancel)?;
    let label = label_rect.map_or(Ok(vec![]), |r| {
        parse_region(g, r, None, depth + 1, trace, in_cancel)
    })?;
    let node = Node::Brace { over, arg, label };
    let node = if struck {
        Node::Cancel { arg: vec![node] }
    } else {
        node
    };
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

/// Compact accents draw marks with a low variant hugging the base —
/// the over bar as '_' (low in its cell), tilde as '˷', hat as '˰',
/// check as '˯', ring as '˳', dot as '․' U+2024 (a reserved leader dot,
/// distinct from the '.' atom), the under bar as '¯' (high in its cell)
/// — so the drawn glyph maps back to the AST mark here. The old
/// floating glyphs (¯ ˜ ^ ˇ ˚ ˙ above, ‗ below) still read leniently.
fn over_mark_at(c: char) -> Option<char> {
    match c {
        '_' => Some('¯'), // U+005F LOW LINE -> bar
        '˷' => Some('˜'), // U+02F7 LOW TILDE -> tilde
        '˰' => Some('^'), // U+02F0 LOW UP ARROWHEAD -> hat
        '˯' => Some('ˇ'), // U+02EF LOW DOWN ARROWHEAD -> check
        '˳' => Some('˚'), // U+02F3 LOW RING -> ring
        '․' => Some('˙'), // U+2024 ONE DOT LEADER -> dot (.. -> ddot)
        '￫' => Some('⇀'), // U+FFEB HALFWIDTH RIGHTWARDS ARROW -> vec
        _ => None,
    }
}

fn under_mark_at(c: char) -> Option<char> {
    match c {
        '¯' => Some('‗'), // U+00AF MACRON -> underline
        // The drawn under tilde is the high ˜ (hugging from below); the
        // AST mark is ˷ — the tilde pair swaps between the two roles.
        '˜' => Some('˷'), // U+02DC SMALL TILDE -> utilde
        _ => None,
    }
}

/// Returns (overs, unders, extra columns consumed). A ddot draws as
/// `․․` overhanging one column to the right of its base; the pair is
/// only taken when that spill column holds nothing else (otherwise the
/// second ․ is the neighbour's own dot and this row stays a single ˙).
fn accent_stacks(g: &Grid, rect: Rect, bl: usize, col: usize) -> (Vec<char>, Vec<char>, usize) {
    let mut overs = Vec::new();
    let mut pair_rows: Vec<usize> = Vec::new();
    let mut r = bl;
    while r > rect.t
        && let Some(m) = over_mark_at(g.at(r - 1, col))
    {
        if m == '˙' && col < rect.r && g.at(r - 1, col + 1) == '․' {
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
            overs[bl - 1 - pr] = '¨';
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
fn parse_lattice(
    g: &Grid,
    rect: Rect,
    col: usize,
    depth: usize,
    trace: &mut Vec<RegionSpan>,
    in_cancel: bool,
) -> Result<(Node, usize)> {
    let marker_rows: Vec<usize> = rect
        .rows()
        .filter(|&r| LATTICE_LEFT.contains(&g.at(r, col)))
        .collect();
    if marker_rows.len() < 2 || g.at(marker_rows[0], col) != '┌' {
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
            if ch == '┐' {
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
    let bot = *marker_rows.last().unwrap();
    let right = *marker_cols.last().unwrap();
    trace.push(((top, bot, col, right), depth));
    // A fully struck lattice is \cancel content: wrap it so the adjacent-
    // Cancel merge in normalize reassembles the whole struck run (the
    // cancel-extent scan cannot anchor on the lattice's blank baseline).
    let struck = !in_cancel
        && (top..=bot).all(|r| (col..=right).all(|c| g.at(r, c) == ' ' || g.cancelled(r, c)));
    let in_cancel = in_cancel || struck;
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
                parse_region(g, cell, None, depth + 1, trace, in_cancel)?
            };
            cells.push(row);
        }
    }
    let node = Node::Array { rows, cols, cells };
    let node = if struck {
        Node::Cancel { arg: vec![node] }
    } else {
        node
    };
    Ok((node, right))
}

/// A delimiter block starting at (bl, col): left column, optional
/// full-height │ middles, matching right column (possibly of a different
/// family — mismatched pairs are legal). Returns the node and the close
/// column.
#[allow(clippy::too_many_arguments)]
fn parse_delim(
    g: &Grid,
    rect: Rect,
    bl: usize,
    col: usize,
    depth: usize,
    trace: &mut Vec<RegionSpan>,
    in_cancel: bool,
) -> Result<(Node, usize)> {
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
        while bl + 1 > k && bl >= k && col + k <= rect.r && g.at(bl - k, col + k) == '╱' {
            k += 1;
        }
        (bl + 1 - k, (bl + k).min(rect.b), col + k)
    } else {
        let (t, b) = vertical_extent(g, rect, col, bl, left_family(left));
        (t, b, col + 1)
    };
    let interior_r = if angle_close_turn(g, bl, close_col) {
        let mut k = 1usize;
        while bl >= k && close_col > k && g.at(bl - k, close_col - k) == '╲' {
            k += 1;
        }
        close_col - k
    } else {
        close_col - 1
    };

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
                    let cell = Rect {
                        t: (row_edges[ri] + 1) as usize,
                        b: (row_edges[ri + 1] - 1) as usize,
                        l: col_edges[ci] + 1,
                        r: col_edges[ci + 1] - 1,
                    };
                    let row = if cell.t > cell.b || cell.l > cell.r {
                        vec![]
                    } else {
                        parse_region(g, cell, None, depth + 1, trace, in_cancel)?
                    };
                    cells.push(row);
                }
            }
            let array = Node::Array {
                rows: rows_n,
                cols: cols_n,
                cells,
            };
            let node = Node::Delim {
                left,
                right,
                mids: vec![],
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
            parse_region(g, rect, Some(bl), depth + 1, trace, in_cancel)?
        };
        segs.push(seg);
        start = m + 1;
    }
    let node = Node::Delim {
        left,
        right,
        mids: vec!['|'; mid_cols.len()],
        segs,
    };
    Ok((node, close_col))
}

/// Middle-separator columns of a delimiter interior: │ over the full
/// extent (a √ stem never spans the extent — its top row is the overline
/// row — and a nested delimiter's middles are bracket-protected).
fn mid_columns(g: &Grid, interior: Rect) -> Vec<usize> {
    let protected = protected_cols(g, interior, None);
    interior
        .cols()
        .filter(|&c| interior.rows().all(|r| g.at(r, c) == '│') && !protected[c - interior.l])
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
    parse_with_regions(text).map(|(row, _)| row)
}

/// Like `parse`, but also return every structural region's rectangle and
/// nesting depth (for the TUI structure view).
pub fn parse_with_regions(text: &str) -> Result<(Row, Vec<RegionSpan>)> {
    // Fold combining long solidus overlays (\cancel strikes) into a
    // parallel flag grid so they do not occupy cells of their own.
    let mut lines: Vec<Vec<char>> = Vec::new();
    let mut flags: Vec<Vec<bool>> = Vec::new();
    for raw in text.lines() {
        let mut line = Vec::new();
        let mut flag = Vec::new();
        for c in raw.trim_end().chars() {
            match c {
                '\u{338}' | '\u{336}' => {
                    if let Some(f) = flag.last_mut() {
                        *f = true;
                    }
                }
                '\t' => {
                    line.push(' ');
                    flag.push(false);
                }
                c => {
                    line.push(c);
                    flag.push(false);
                }
            }
        }
        lines.push(line);
        flags.push(flag);
    }
    if lines.iter().all(|l| l.is_empty()) {
        return Ok((vec![], vec![]));
    }
    let width = lines.iter().map(|l| l.len()).max().unwrap();
    for l in &mut lines {
        l.resize(width, ' ');
    }
    for f in &mut flags {
        f.resize(width, false);
    }
    let g = Grid {
        g: lines,
        cancel: flags,
    };
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
    let mut trace = Vec::new();
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
            out.extend(parse_region(&g, rect, None, 0, &mut trace, false)?)
        }
    }
    // Normalize: a script arg that mixes padded structures with atoms
    // parses as adjacent script chunks; merging them restores the
    // canonical single node (render is only defined on normal forms).
    Ok((crate::ast::normalize(&out), trace))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::normalize;
    use crate::latex::row_to_latex;
    use crate::render::{RenderCtx, render_row};

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
            index: 2,
        }]);
        roundtrip(&vec![Node::Sqrt {
            arg: syms("x+1"),
            index: 3,
        }]);
        roundtrip(&vec![Node::Accent {
            overs: vec!['^'],
            unders: vec![],
            base: 'x',
        }]);
        roundtrip(&vec![Node::Accent {
            overs: vec![],
            unders: vec!['‗'],
            base: 'y',
        }]);
        roundtrip(&vec![Node::Func("sin".into()), Node::Sym('x')]);
        roundtrip(&vec![Node::Delim {
            left: '[',
            right: ']',
            mids: vec![],
            segs: vec![vec![Node::Array {
                rows: 2,
                cols: 2,
                cells: vec![syms("a"), syms("b+1"), vec![], syms("d")],
            }]],
        }]);
        roundtrip(&vec![Node::Delim {
            left: '(',
            right: ')',
            mids: vec![],
            segs: vec![syms("a+b")],
        }]);
        roundtrip(&vec![Node::BigOp {
            base: vec![Node::Sym('∑')],
            lower: syms("i=0"),
            upper: syms("n"),
        }]);
        roundtrip(&vec![Node::BigOp {
            base: vec![Node::Sym('∫')],
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
        assert_eq!(row_to_latex(&row), "a\\sin y");
        // …but letter *runs* are \mathrm unless they are dictionary words,
        // and a single letter glued to another letter is roman too (d𝑦).
        let row = parse("asiny").unwrap();
        assert_eq!(row_to_latex(&row), "\\mathrm{asiny}");
        let row = parse("E=mc²").unwrap();
        assert_eq!(row_to_latex(&row), "E=\\mathrm{mc}^{2}");
        let row = parse("d𝑦").unwrap();
        assert_eq!(row_to_latex(&row), "\\mathrm{d}y");
        // 'single quotes' force \mathrm; primes survive elsewhere.
        let row = parse("'d'x").unwrap();
        assert_eq!(row_to_latex(&row), "\\mathrm{d}x");
        let row = parse("𝑥''").unwrap();
        assert_eq!(row_to_latex(&row), "x''");
    }

    #[test]
    fn tilde_is_a_plain_atom() {
        assert_eq!(parse("a~b").unwrap(), syms("a~b"));
        assert_eq!(parse("a ~ b").unwrap(), syms("a~b"));
        // A literal ~ still takes accents (drawn low-hat form).
        let row = parse("˰\n~").unwrap();
        assert_eq!(
            row,
            vec![Node::Accent {
                overs: vec!['^'],
                unders: vec![],
                base: '~'
            }]
        );
    }

    #[test]
    fn ambiguity_counterexample_is_now_distinguishable() {
        // The two ASTs that rendered identically before the band notation.
        let ast1 = vec![Node::BigOp {
            base: vec![Node::Sym('∑')],
            lower: syms("n=1"),
            upper: vec![Node::BigOp {
                base: vec![Node::Sym('∫')],
                lower: vec![],
                upper: vec![],
            }],
        }];
        let ast2 = vec![Node::BigOp {
            base: vec![Node::Sym('∫')],
            lower: vec![Node::BigOp {
                base: vec![Node::Sym('∑')],
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
