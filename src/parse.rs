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
    lattice_char, unstyle_char, unsubscript_char, unsuperscript_char, DOUBLE_BODY, FRAC_BAR,
    OP_BAND, PLACEHOLDER,
};

/// Left-edge glyphs of a grid lattice column (see render::LATTICE_CHARS).
const LATTICE_LEFT: &[char] = &['┌', '├', '└'];
const LATTICE_TOP: &[char] = &['┌', '┬', '┐'];
use crate::symbols::{bigop_by_char, func_prefix, is_over_mark, is_under_mark};

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
        write!(f, "parse error at {}:{}: {}", self.at.0 + 1, self.at.1 + 1, self.msg)
    }
}

impl std::error::Error for ParseError {}

type Result<T> = std::result::Result<T, ParseError>;

fn err<T>(msg: impl Into<String>, r: usize, c: usize) -> Result<T> {
    Err(ParseError { msg: msg.into(), at: (r, c) })
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
    '(', '⎛', '⎜', '⎝', '[', '⎡', '⎢', '⎣', '{', '⎧', '⎨', '⎩', '⟨', '⎸', '▏', '┌', '├', '└',
    '┠',
];
const CLOSE_BRACKETS: &[char] = &[
    ')', '⎞', '⎟', '⎠', ']', '⎤', '⎥', '⎦', '}', '⎫', '⎬', '⎭', '⟩', '⎹', '▕', '┐', '┤', '┘',
    '┨',
];

/// Delimiter spec char for a glyph that can appear on the *baseline row*
/// of a left delimiter column. Brace/angle columns always show their
/// vertex (⎨ / ⟨) on the baseline, so ⎧ ⎪ ⎩ ╱ ╲ never occur here.
fn open_spec(c: char) -> Option<char> {
    Some(match c {
        '(' | '⎛' | '⎜' | '⎝' => '(',
        '[' | '⎡' | '⎢' | '⎣' => '[',
        '{' | '⎨' => '{',
        '⟨' => '⟨',
        '⎸' => '|',
        '▏' => '.',
        // ┠ (fused-grid junction) resolves its family via the column walk
        // in open_spec_at; standalone it only marks "an open side".
        '┠' => return None,
        _ => return None,
    })
}

/// Family of *any* glyph a left delimiter column can contain (used by the
/// junction-resolution walk; ⎪ is shared with right braces, so it only
/// appears via the walk's own continue set).
fn left_col_spec(c: char) -> Option<char> {
    Some(match c {
        '(' | '⎛' | '⎜' | '⎝' => '(',
        '[' | '⎡' | '⎢' | '⎣' => '[',
        '{' | '⎧' | '⎨' | '⎩' => '{',
        '⎸' => '|',
        '▏' => '.',
        _ => return None,
    })
}

fn right_col_spec(c: char) -> Option<char> {
    Some(match c {
        ')' | '⎞' | '⎟' | '⎠' => ')',
        ']' | '⎤' | '⎥' | '⎦' => ']',
        '}' | '⎫' | '⎬' | '⎭' => '}',
        '⎹' => '|',
        '▕' => '.',
        _ => return None,
    })
}

/// Fused-grid markers of a delimiter block, if its interior is one: the
/// top/bottom rows must contain nothing but ┬ / ┴ (a *nested* lattice can
/// put stray ┬ on the top row otherwise), and at least one marker must
/// exist. Returns (marker_cols, marker_rows).
#[allow(clippy::type_complexity)]
fn fused_grid_markers(
    g: &Grid,
    top: usize,
    bot: usize,
    col: usize,
    close: usize,
) -> Option<(Vec<usize>, Vec<usize>)> {
    if close <= col + 1 || bot <= top + 1 {
        return None;
    }
    let clean = |r: usize, mark: char| {
        (col + 1..close).all(|c2| g.at(r, c2) == ' ' || g.at(r, c2) == mark)
    };
    if !clean(top, '┬') || !clean(bot, '┴') {
        return None;
    }
    let marker_cols: Vec<usize> = (col + 1..close).filter(|&c2| g.at(top, c2) == '┬').collect();
    let marker_rows: Vec<usize> = (top + 1..bot)
        .filter(|&r| g.at(r, col) == '┠' || g.at(r, close) == '┨')
        .collect();
    if marker_cols.is_empty() && marker_rows.is_empty() {
        return None;
    }
    Some((marker_cols, marker_rows))
}

/// Like `open_spec`, resolving a fused-grid junction (┠) by walking its
/// column to a family-distinct glyph (through ⎪ and further junctions).
fn open_spec_at(g: &Grid, row: usize, col: usize) -> Option<char> {
    let ch = g.at(row, col);
    if ch != '┠' {
        return open_spec(ch);
    }
    let h = g.g.len();
    let walk = |range: &mut dyn Iterator<Item = usize>| -> Option<char> {
        for r in range {
            match left_col_spec(g.at(r, col)) {
                Some(s) => return Some(s),
                None if matches!(g.at(r, col), '┠' | '⎪') => continue,
                None => return None,
            }
        }
        None
    };
    walk(&mut (0..row).rev()).or_else(|| walk(&mut (row + 1..h)))
}

fn close_spec(c: char) -> Option<char> {
    Some(match c {
        ')' | '⎞' | '⎟' | '⎠' => ')',
        ']' | '⎤' | '⎥' | '⎦' => ']',
        '}' | '⎬' => '}',
        '⟩' => '⟩',
        '⎹' => '|',
        '▕' => '.',
        _ => return None,
    })
}

/// Like `close_spec`, resolving a fused-grid junction (┨) by walking its
/// column to a family-distinct glyph (through ⎪ and further junctions).
fn close_spec_at(g: &Grid, row: usize, col: usize) -> Option<char> {
    let ch = g.at(row, col);
    if ch != '┨' {
        return close_spec(ch);
    }
    let h = g.g.len();
    let walk = |range: &mut dyn Iterator<Item = usize>| -> Option<char> {
        for r in range {
            match right_col_spec(g.at(r, col)) {
                Some(s) => return Some(s),
                None if matches!(g.at(r, col), '┨' | '⎪') => continue,
                None => return None,
            }
        }
        None
    };
    walk(&mut (0..row).rev()).or_else(|| walk(&mut (row + 1..h)))
}

/// Every glyph a left delimiter column of `spec` can contain (for the
/// vertical-extent scan).
fn left_family(spec: char) -> &'static [char] {
    match spec {
        '(' => &['(', '⎛', '⎜', '⎝', '┠'],
        '[' => &['[', '⎡', '⎢', '⎣', '┠'],
        '{' => &['{', '⎧', '⎪', '⎨', '⎩', '┠'],
        '⟨' => &['⟨', '╱', '╲'],
        '|' => &['⎸', '┠'],
        _ => &['▏', '┠'],
    }
}

/// Which side of a delimiter pair the glyph at (row, col) belongs to:
/// Some(true) = left/open, Some(false) = right/close, None = neither.
/// The shared glyphs (brace extension ⎪, angle arms ╱ ╲) resolve their
/// side by walking the contiguous column run to a side-distinct glyph —
/// without this, depth counting desyncs on rows that cut through a
/// mismatched pair (e.g. a { … ▕ cases block).
fn delim_side(g: &Grid, row: usize, col: usize) -> Option<bool> {
    let ch = g.at(row, col);
    if OPEN_BRACKETS.contains(&ch) {
        return Some(true);
    }
    if CLOSE_BRACKETS.contains(&ch) {
        return Some(false);
    }
    let family: &[char] = match ch {
        '⎪' => &['⎧', '⎨', '⎩', '⎫', '⎬', '⎭', '⎪'],
        '╱' | '╲' => &['⟨', '⟩', '╱', '╲'],
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
    // are centered with >= 1 column of slack on each side). '~' is the
    // lenient hand-written band char (see parse_region).
    if let Some(&r) = occupied
        .iter()
        .find(|&&r| matches!(g.at(r, c), FRAC_BAR | OP_BAND | DOUBLE_BODY | '~'))
    {
        return Ok(r);
    }

    // Accent marks stack directly above/below their base; over-marks and
    // under-marks are reserved chars disjoint from atoms, so stripping them
    // leaves the base row.
    while occupied.len() > 1 && is_over_mark(g.at(occupied[0], c)) {
        occupied.remove(0);
    }
    while occupied.len() > 1 && is_under_mark(g.at(*occupied.last().unwrap(), c)) {
        occupied.pop();
    }

    let first = *occupied.first().unwrap();
    let last = *occupied.last().unwrap();
    match g.at(first, c) {
        // Delimiter columns: a fused grid (┠ junctions in this column or
        // ┬ markers on the top row) centers on the extent; otherwise the
        // baseline is that of the inner region (a lattice inside answers
        // with its own ┌├└ center rule).
        '⎡' | '[' | '⎛' | '⎸' | '▏' => {
            if let Ok(close) = match_delim(g, first, c, rect.r) {
                if fused_grid_markers(g, first, last, c, close).is_some() {
                    return Ok((first + last) / 2);
                }
            }
            find_baseline(g, Rect { t: first, b: last, l: c + 1, r: rect.r })
        }
        '(' => Ok(first),
        // Brace / angle columns carry their vertex on the baseline row.
        '⎧' | '⎪' | '⎨' | '⎩' | '┠' => occupied
            .iter()
            .find(|&&r| g.at(r, c) == '⎨')
            .copied()
            // A fused grid whose baseline lands on a junction row shows ┠
            // there instead of the vertex; the grid centers on the extent.
            .or_else(|| {
                (first..=last).any(|r| g.at(r, c) == '┠').then_some((first + last) / 2)
            })
            .ok_or(())
            .or_else(|_| err("brace column without ⎨", first, c)),
        '⟨' | '╱' | '╲' => occupied
            .iter()
            .find(|&&r| g.at(r, c) == '⟨')
            .copied()
            .ok_or(())
            .or_else(|_| err("angle column without ⟨", first, c)),
        // Lattice left-edge column: the grid centers on its extent.
        '┌' | '├' | '└' => Ok((first + last) / 2),
        // Sqrt: stem covers exactly the content rows; recurse into content.
        '│' => find_baseline(g, Rect { t: first, b: last, l: c + 1, r: rect.r }),
        ch if radical_index(ch).is_some() => {
            find_baseline(g, Rect { t: first, b: last, l: c + 1, r: rect.r })
        }
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
                    rect.rows().all(|row| g.at(row, c) == ' ' || g.cancelled(row, c))
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
            let inner = Rect { l: col, r: end, ..rect };
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
                let run_end = match lattice_start {
                    Some(ls) => {
                        if ls > col {
                            parse_script_run(
                                g, rect, bl, col, ls - 1, depth, trace, in_cancel, &mut out,
                            )?;
                        }
                        let (node, right) = parse_lattice(g, rect, ls, depth, trace, in_cancel)?;
                        out.push(node);
                        col = right + 1;
                        continue;
                    }
                    None => run_end,
                };
                parse_script_run(g, rect, bl, col, run_end, depth, trace, in_cancel, &mut out)?;
                col = run_end + 1;
            }
            '┌' | '├' | '└' => {
                let (node, right) = parse_lattice(g, rect, col, depth, trace, in_cancel)?;
                out.push(node);
                col = right + 1;
            }
            _ if ch == FRAC_BAR => {
                // Maximal munch: a ─ run capped by → is a labeled arrow;
                // a fraction next to an arrow *atom* renders with a space
                // between (presence of the space is the disambiguator).
                let run_end = scan_while(g, bl, col, rect.r, |c| c == FRAC_BAR);
                if run_end < rect.r && g.at(bl, run_end + 1) == '→' {
                    let span = Rect { t: rect.t, b: rect.b, l: col, r: run_end + 1 };
                    let over = region_above(span, bl)
                        .map_or(Ok(vec![]), |r| parse_region(g, r, None, depth + 1, trace, in_cancel))?;
                    let under = region_below(span, bl)
                        .map_or(Ok(vec![]), |r| parse_region(g, r, None, depth + 1, trace, in_cancel))?;
                    out.push(Node::Arrow { op: '→', over, under });
                    col = run_end + 2;
                    continue;
                }
                let span = Rect { t: rect.t, b: rect.b, l: col, r: run_end };
                let num = region_above(span, bl)
                    .map_or(Ok(vec![]), |r| parse_region(g, r, None, depth + 1, trace, in_cancel))?;
                let den = region_below(span, bl)
                    .map_or(Ok(vec![]), |r| parse_region(g, r, None, depth + 1, trace, in_cancel))?;
                out.push(Node::Frac { num, den });
                col = run_end + 1;
            }
            _ if ch == OP_BAND => {
                // General band: ┄+ (piece ┄+)+ — anything sandwiched in ┄
                // without spaces takes over/under limits (∑, lim, arg┄max
                // …). Pieces are flat one-line rows joined into the base.
                let (pieces, end) = scan_band(g, rect, bl, col, OP_BAND)?;
                if pieces.is_empty() {
                    return err("band without content", bl, col);
                }
                let mut base: Row = Vec::new();
                for (l0, r0) in pieces {
                    let prect = Rect { t: bl, b: bl, l: l0, r: r0 };
                    base.extend(parse_region(g, prect, Some(bl), depth + 1, trace, in_cancel)?);
                }
                let span = Rect { t: rect.t, b: rect.b, l: col, r: end };
                let upper = region_above(span, bl)
                    .map_or(Ok(vec![]), |r| parse_region(g, r, None, depth + 1, trace, in_cancel))?;
                let lower = region_below(span, bl)
                    .map_or(Ok(vec![]), |r| parse_region(g, r, None, depth + 1, trace, in_cancel))?;
                out.push(Node::BigOp { base, lower, upper });
                col = end + 1;
            }
            _ if ch == DOUBLE_BODY => {
                // Double-arrow body: ═ run capped by ⇒ (═ has no other use).
                let run_end = scan_while(g, bl, col, rect.r, |c| c == DOUBLE_BODY);
                if run_end == rect.r || g.at(bl, run_end + 1) != '⇒' {
                    return err("═ run without a ⇒ head", bl, col);
                }
                let span = Rect { t: rect.t, b: rect.b, l: col, r: run_end + 1 };
                let over = region_above(span, bl)
                    .map_or(Ok(vec![]), |r| parse_region(g, r, None, depth + 1, trace, in_cancel))?;
                let under = region_below(span, bl)
                    .map_or(Ok(vec![]), |r| parse_region(g, r, None, depth + 1, trace, in_cancel))?;
                out.push(Node::Arrow { op: '⇒', over, under });
                col = run_end + 2;
            }
            '←' | '⇐'
                if col < rect.r
                    && g.at(bl, col + 1)
                        == (if ch == '←' { FRAC_BAR } else { DOUBLE_BODY }) =>
            {
                // Left-pointing labeled arrow: head, then the body run.
                let body = if ch == '←' { FRAC_BAR } else { DOUBLE_BODY };
                let run_end = scan_while(g, bl, col + 1, rect.r, |c| c == body);
                let span = Rect { t: rect.t, b: rect.b, l: col, r: run_end };
                let over = region_above(span, bl)
                    .map_or(Ok(vec![]), |r| parse_region(g, r, None, depth + 1, trace, in_cancel))?;
                let under = region_below(span, bl)
                    .map_or(Ok(vec![]), |r| parse_region(g, r, None, depth + 1, trace, in_cancel))?;
                out.push(Node::Arrow { op: ch, over, under });
                col = run_end + 1;
            }
            '"' => {
                // Quoted roman/text run ("dx", "if ␣ x"): flat baseline
                // chars up to the closing quote; ␣ maps back to a space.
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
                let t: String = (col + 1..close)
                    .map(|c2| match g.at(bl, c2) {
                        '␣' => ' ',
                        c2 => c2,
                    })
                    .collect();
                out.push(Node::Text(t));
                col = close + 1;
            }
            '~' => {
                // Lenient input: '~' doubles as the band char (easier to
                // type than ┄) when the full band pattern is present, the
                // pieces are all known operators/functions (an accented
                // atom between literal tildes must not read as a band),
                // AND there is limit content above or below. Otherwise a
                // plain atom (spaces around a literal ~ keep it one).
                let band = (|| {
                    let (pieces, end) = scan_band(g, rect, bl, col, '~').ok()?;
                    if pieces.is_empty() {
                        return None;
                    }
                    let mut base: Row = Vec::new();
                    for &(l0, r0) in &pieces {
                        let prect = Rect { t: bl, b: bl, l: l0, r: r0 };
                        let mut tr = Vec::new();
                        base.extend(parse_region(g, prect, Some(bl), depth + 1, &mut tr, in_cancel).ok()?);
                    }
                    let known = !base.is_empty()
                        && base.iter().all(|n| match n {
                            Node::Sym(c) => bigop_by_char(*c),
                            Node::Func(_) => true,
                            _ => false,
                        });
                    if !known {
                        return None;
                    }
                    let span = Rect { t: rect.t, b: rect.b, l: col, r: end };
                    let limits = region_above(span, bl).and_then(|r| trim(g, r)).is_some()
                        || region_below(span, bl).and_then(|r| trim(g, r)).is_some();
                    limits.then_some((base, span, end))
                })();
                if let Some((base, span, run_end)) = band {
                    let upper = region_above(span, bl)
                        .map_or(Ok(vec![]), |r| parse_region(g, r, None, depth + 1, trace, in_cancel))?;
                    let lower = region_below(span, bl)
                        .map_or(Ok(vec![]), |r| parse_region(g, r, None, depth + 1, trace, in_cancel))?;
                    out.push(Node::BigOp { base, lower, upper });
                    col = run_end + 1;
                } else {
                    // Same accent probe as ordinary atoms.
                    let (overs, unders) = accent_stacks(g, rect, bl, col);
                    if overs.is_empty() && unders.is_empty() {
                        out.push(Node::Sym('~'));
                    } else {
                        out.push(Node::Accent { overs, unders, base: '~' });
                    }
                    col += 1;
                }
            }
            ')' => {
                // Unmatched close (LyX lets you type one): plain atom.
                out.push(Node::Sym(')'));
                col += 1;
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
                if top == 0 {
                    return err("radical without overline", top, col);
                }
                let index = radical_index(g.at(bot, col)).unwrap();
                let w = scan_while(g, top - 1, col + 1, rect.r, |c| c == '_') - col;
                let inner = Rect { t: top, b: bot, l: col + 1, r: col + w };
                out.push(Node::Sqrt { arg: parse_region(g, inner, Some(bl), depth + 1, trace, in_cancel)?, index });
                col += w + 1;
            }
            _ if ch == PLACEHOLDER => col += 1,
            _ if unsuperscript_char(ch).is_some() => {
                let run_end =
                    scan_while_same_flag(g, bl, col, rect.r, |c| unsuperscript_char(c).is_some());
                let arg = (col..=run_end)
                    .map(|c| Node::Sym(unsuperscript_char(g.at(bl, c)).unwrap()))
                    .collect();
                out.push(Node::Sup { arg });
                col = run_end + 1;
            }
            _ if unsubscript_char(ch).is_some() => {
                let run_end =
                    scan_while_same_flag(g, bl, col, rect.r, |c| unsubscript_char(c).is_some());
                let arg = (col..=run_end)
                    .map(|c| Node::Sym(unsubscript_char(g.at(bl, c)).unwrap()))
                    .collect();
                out.push(Node::Sub { arg });
                col = run_end + 1;
            }
            _ if ch.is_ascii_alphabetic() => {
                // Upright ASCII letters: function names (canonical), with
                // leftover letters accepted as plain atoms (lenient input).
                let run_end =
                    scan_while_same_flag(g, bl, col, rect.r, |c| c.is_ascii_alphabetic());
                let word: String = (col..=run_end).map(|c| g.at(bl, c)).collect();
                let mut rest = word.as_str();
                while !rest.is_empty() {
                    if let Some(f) = func_prefix(rest) {
                        out.push(Node::Func(f.to_string()));
                        rest = &rest[f.len()..];
                    } else {
                        out.push(Node::Sym(rest.chars().next().unwrap()));
                        rest = &rest[1..];
                    }
                }
                col = run_end + 1;
            }
            _ => {
                // Marks in the cells directly above/below an atom are an
                // accent stack; those cells are otherwise always blank.
                let (overs, unders) = accent_stacks(g, rect, bl, col);
                let base = unstyle_char(ch);
                if !overs.is_empty() || !unders.is_empty() {
                    out.push(Node::Accent { overs, unders, base });
                } else {
                    out.push(Node::Sym(base));
                }
                col += 1;
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
    let run_span = Rect { t: rect.t, b: rect.b, l: from, r: to };
    for (side_rect, opposite, is_sup) in [
        (region_above(run_span, bl), region_below(run_span, bl), true),
        (region_below(run_span, bl), region_above(run_span, bl), false),
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
            let seg = Rect { t: side.t, b: side.b, l: from + start, r: from + i - 1 };
            if let Some(seg) = trim(g, seg) {
                parts.push((seg.l, is_sup, seg));
            }
        }
    }
    parts.sort_by_key(|&(l, _, _)| l);
    for (_, is_sup, r) in parts {
        let arg = parse_region(g, r, None, depth + 1, trace, in_cancel)?;
        out.push(if is_sup { Node::Sup { arg } } else { Node::Sub { arg } });
    }
    Ok(())
}

/// Scan a band starting at (bl, col): `B+ (piece B+)*` where B is the
/// band char. Returns the piece spans and the column of the final band
/// char. A piece not closed by another band char is an error (canonical
/// bands always are; the lenient caller treats it as "not a band").
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

/// Accent-mark stacks in the cells directly above/below (bl, col),
/// innermost first. Returns (overs, unders).
fn accent_stacks(g: &Grid, rect: Rect, bl: usize, col: usize) -> (Vec<char>, Vec<char>) {
    let mut overs = Vec::new();
    let mut r = bl;
    while r > rect.t && is_over_mark(g.at(r - 1, col)) {
        overs.push(g.at(r - 1, col));
        r -= 1;
    }
    let mut unders = Vec::new();
    let mut r = bl;
    while r < rect.b && is_under_mark(g.at(r + 1, col)) {
        unders.push(g.at(r + 1, col));
        r += 1;
    }
    (overs, unders)
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
    let kind = |i: usize, n: usize| if i == 0 { 0 } else if i == n { 2 } else { 1 };
    for (ri, &r) in marker_rows.iter().enumerate() {
        for (ci, &mc) in marker_cols.iter().enumerate() {
            let want = lattice_char(kind(ri, rows_n), kind(ci, cols_n));
            if g.at(r, mc) != want {
                return err(format!("broken lattice (expected {} at a crossing)", want), r, mc);
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
        && (top..=bot)
            .all(|r| (col..=right).all(|c| g.at(r, c) == ' ' || g.cancelled(r, c)));
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
    let node = if struck { Node::Cancel { arg: vec![node] } } else { node };
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
    let (top, bot) = vertical_extent(g, rect, col, bl, left_family(left));

    // Fused grid: ┬ markers on the delimiter's top row and/or ┠ junction
    // rows in the left column mean the interior is one grid whose edges
    // are the delimiter columns themselves.
    {
        if let Some((marker_cols, marker_rows)) = fused_grid_markers(g, top, bot, col, close_col)
        {
            for &r in &marker_rows {
                for &c2 in &marker_cols {
                    if g.at(r, c2) != '┼' {
                        return err("broken fused grid (expected ┼)", r, c2);
                    }
                }
            }
            let (rows_n, cols_n) = (marker_rows.len() + 1, marker_cols.len() + 1);
            let row_edges: Vec<usize> = std::iter::once(top)
                .chain(marker_rows.iter().copied())
                .chain(std::iter::once(bot))
                .collect();
            let col_edges: Vec<usize> = std::iter::once(col)
                .chain(marker_cols.iter().copied())
                .chain(std::iter::once(close_col))
                .collect();
            let mut cells = Vec::with_capacity(rows_n * cols_n);
            for ri in 0..rows_n {
                for ci in 0..cols_n {
                    let cell = Rect {
                        t: row_edges[ri] + 1,
                        b: row_edges[ri + 1] - 1,
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
            let array = Node::Array { rows: rows_n, cols: cols_n, cells };
            let node =
                Node::Delim { left, right, mids: vec![], segs: vec![vec![array]] };
            return Ok((node, close_col));
        }
    }

    let mid_cols = if close_col > col + 1 {
        mid_columns(g, Rect { t: top, b: bot, l: col + 1, r: close_col - 1 })
    } else {
        vec![]
    };

    let mut segs: Vec<Row> = Vec::new();
    let mut start = col + 1;
    for &m in mid_cols.iter().chain(std::iter::once(&close_col)) {
        let seg = if start >= m {
            vec![]
        } else {
            let rect = Rect { t: top, b: bot, l: start, r: m - 1 };
            parse_region(g, rect, Some(bl), depth + 1, trace, in_cancel)?
        };
        segs.push(seg);
        start = m + 1;
    }
    let node = Node::Delim { left, right, mids: vec!['|'; mid_cols.len()], segs };
    Ok((node, close_col))
}

/// Middle-separator columns of a delimiter interior: │ over the full
/// extent (a √ stem never spans the extent — its top row is the overline
/// row — and a nested delimiter's middles are bracket-protected).
fn mid_columns(g: &Grid, interior: Rect) -> Vec<usize> {
    let protected = protected_cols(g, interior, None);
    interior
        .cols()
        .filter(|&c| {
            interior.rows().all(|r| g.at(r, c) == '│') && !protected[c - interior.l]
        })
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
    let g = Grid { g: lines, cancel: flags };
    let rect = Rect { t: 0, b: g.g.len() - 1, l: 0, r: width - 1 };
    let mut trace = Vec::new();
    match trim(&g, rect) {
        // Normalize: a script arg that mixes padded structures with atoms
        // parses as adjacent script chunks; merging them restores the
        // canonical single node (render is only defined on normal forms).
        Some(rect) => parse_region(&g, rect, None, 0, &mut trace, false)
            .map(|row| (crate::ast::normalize(&row), trace)),
        None => Ok((vec![], trace)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::normalize;
    use crate::latex::row_to_latex;
    use crate::render::{render_row, RenderCtx};

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
        roundtrip(&vec![Node::Frac { num: syms("1"), den: syms("x+1") }]);
        roundtrip(&vec![Node::Sqrt { arg: syms("2"), index: 2 }]);
        roundtrip(&vec![Node::Sqrt { arg: syms("x+1"), index: 3 }]);
        roundtrip(&vec![Node::Accent { overs: vec!['^'], unders: vec![], base: 'x' }]);
        roundtrip(&vec![Node::Accent { overs: vec![], unders: vec!['‗'], base: 'y' }]);
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
        roundtrip(&vec![Node::BigOp { base: vec![Node::Sym('∑')], lower: syms("i=0"), upper: syms("n") }]);
        roundtrip(&vec![Node::BigOp { base: vec![Node::Sym('∫')], lower: vec![], upper: vec![] }]);
    }

    #[test]
    fn parses_handwritten_ascii() {
        let row = parse("x+1").unwrap();
        assert_eq!(row_to_latex(&row), "x+1");
        let row = parse("E=mc²").unwrap();
        assert_eq!(row_to_latex(&row), "E=mc^{2}");
    }

    #[test]
    fn lenient_tilde_band() {
        // Hand-written band with ~ instead of ┄ (canonical stays ┄).
        let aa = " ∞ \n~∑~\nn=1";
        let row = parse(aa).unwrap();
        assert_eq!(
            row,
            vec![Node::BigOp { base: vec![Node::Sym('∑')], lower: syms("n=1"), upper: syms("∞") }]
        );
        // Without limits (or with spaces around), ~ is a plain atom.
        assert_eq!(parse("a~b").unwrap(), syms("a~b"));
        assert_eq!(parse("a ~ b").unwrap(), syms("a~b"));
        // A literal ~ still takes accents.
        let row = parse("^\n~").unwrap();
        assert_eq!(row, vec![Node::Accent { overs: vec!['^'], unders: vec![], base: '~' }]);
    }

    #[test]
    fn ambiguity_counterexample_is_now_distinguishable() {
        // The two ASTs that rendered identically before the band notation.
        let ast1 = vec![Node::BigOp {
            base: vec![Node::Sym('∑')],
            lower: syms("n=1"),
            upper: vec![Node::BigOp { base: vec![Node::Sym('∫')], lower: vec![], upper: vec![] }],
        }];
        let ast2 = vec![Node::BigOp {
            base: vec![Node::Sym('∫')],
            lower: vec![Node::BigOp { base: vec![Node::Sym('∑')], lower: syms("n=1"), upper: vec![] }],
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
