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
use crate::render::{unstyle_char, unsubscript_char, unsuperscript_char, FRAC_BAR, OP_BAND, PLACEHOLDER};
use crate::symbols::{bigop_by_char, func_prefix, is_over_mark, is_under_mark};

/// Optional explicit baseline marker for hand-written input: a single '▶'
/// anywhere in the text pins the baseline row (never emitted by the renderer).
pub const BASELINE_MARKER: char = '▶';

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

const OPEN_BRACKETS: &[char] = &['(', '⎛', '⎡', '['];
const CLOSE_BRACKETS: &[char] = &[')', '⎞', '⎤', ']'];

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
            let ch = g.at(r, c);
            if CLOSE_BRACKETS.contains(&ch) {
                depth -= 1;
            }
            if depth > 0 {
                protected[c - rect.l] = true;
            }
            if OPEN_BRACKETS.contains(&ch) {
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
    if let Some(&r) = occupied
        .iter()
        .find(|&&r| g.at(r, c) == FRAC_BAR || g.at(r, c) == OP_BAND)
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
        // Matrix: baseline is the vertical center of the bracket extent.
        '⎡' | '[' => Ok((first + last) / 2),
        // Paren: baseline is the baseline of the inner region.
        '⎛' => find_baseline(g, Rect { t: first, b: last, l: c + 1, r: rect.r }),
        '(' => Ok(first),
        // Sqrt: stem covers exactly the content rows; recurse into content.
        '│' => find_baseline(g, Rect { t: first, b: last, l: c + 1, r: rect.r }),
        ch if radical_index(ch).is_some() => {
            find_baseline(g, Rect { t: first, b: last, l: c + 1, r: rect.r })
        }
        _ => {
            if occupied.len() == 1 {
                Ok(first)
            } else {
                err("cannot determine baseline (ambiguous leftmost column)", first, c)
            }
        }
    }
}

/// Parse a region. `baseline` may be passed down when the caller already
/// knows it (paren/sqrt interiors share the caller's baseline row).
fn parse_region(
    g: &Grid,
    rect: Rect,
    baseline: Option<usize>,
    depth: usize,
    trace: &mut Vec<RegionSpan>,
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
        let ch = g.at(bl, col);
        match ch {
            ' ' => {
                let run_end = scan_while(g, bl, col, rect.r, |c| c == ' ');
                parse_script_run(g, rect, bl, col, run_end, depth, trace, &mut out)?;
                col = run_end + 1;
            }
            _ if ch == FRAC_BAR => {
                let run_end = scan_while(g, bl, col, rect.r, |c| c == FRAC_BAR);
                let span = Rect { t: rect.t, b: rect.b, l: col, r: run_end };
                let num = region_above(span, bl)
                    .map_or(Ok(vec![]), |r| parse_region(g, r, None, depth + 1, trace))?;
                let den = region_below(span, bl)
                    .map_or(Ok(vec![]), |r| parse_region(g, r, None, depth + 1, trace))?;
                out.push(Node::Frac { num, den });
                col = run_end + 1;
            }
            _ if ch == OP_BAND => {
                // Band structure: ┄+ op ┄+ (the op is centered, with at
                // least one band char on each side by construction).
                let lead_end = scan_while(g, bl, col, rect.r, |c| c == OP_BAND);
                if lead_end == rect.r {
                    return err("big-op band without operator", bl, col);
                }
                let op = g.at(bl, lead_end + 1);
                if op == ' ' || op == OP_BAND {
                    return err("big-op band without operator", bl, col);
                }
                let run_end = scan_while(g, bl, lead_end + 1, rect.r, |c| c == OP_BAND);
                if run_end == lead_end + 1 {
                    return err("big-op band must extend past its operator", bl, col);
                }
                let span = Rect { t: rect.t, b: rect.b, l: col, r: run_end };
                let upper = region_above(span, bl)
                    .map_or(Ok(vec![]), |r| parse_region(g, r, None, depth + 1, trace))?;
                let lower = region_below(span, bl)
                    .map_or(Ok(vec![]), |r| parse_region(g, r, None, depth + 1, trace))?;
                out.push(Node::BigOp { op, lower, upper });
                col = run_end + 1;
            }
            '(' => {
                let close = match_on_row(g, bl, col, rect.r, '(', ')')?;
                let inner = Rect { t: bl, b: bl, l: col + 1, r: close.wrapping_sub(1) };
                let inner_row = if close > col + 1 {
                    parse_region(g, inner, Some(bl), depth + 1, trace)?
                } else {
                    vec![]
                };
                out.push(Node::Paren { inner: inner_row });
                col = close + 1;
            }
            ')' => {
                // Unmatched close (LyX lets you type one): plain atom.
                out.push(Node::Sym(')'));
                col += 1;
            }
            '⎛' | '⎜' | '⎝' => {
                let (top, bot) = vertical_extent(g, rect, col, bl, &['⎛', '⎜', '⎝']);
                let close = match_on_row(g, top, col, rect.r, '⎛', '⎞')?;
                let inner = Rect { t: top, b: bot, l: col + 1, r: close - 1 };
                out.push(Node::Paren { inner: parse_region(g, inner, Some(bl), depth + 1, trace)? });
                col = close + 1;
            }
            '⎡' | '⎢' | '⎣' | '[' => {
                let (top, bot) = if ch == '[' {
                    (bl, bl)
                } else {
                    vertical_extent(g, rect, col, bl, &['⎡', '⎢', '⎣'])
                };
                let (open, close_ch) = if ch == '[' { ('[', ']') } else { ('⎡', '⎤') };
                let close = match_on_row(g, top, col, rect.r, open, close_ch)?;
                let inner = Rect { t: top, b: bot, l: col + 1, r: close - 1 };
                out.push(parse_matrix(g, inner, depth + 1, trace)?);
                col = close + 1;
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
                out.push(Node::Sqrt { arg: parse_region(g, inner, Some(bl), depth + 1, trace)?, index });
                col += w + 1;
            }
            _ if ch == PLACEHOLDER => col += 1,
            _ if unsuperscript_char(ch).is_some() => {
                let run_end = scan_while(g, bl, col, rect.r, |c| unsuperscript_char(c).is_some());
                let arg = (col..=run_end)
                    .map(|c| Node::Sym(unsuperscript_char(g.at(bl, c)).unwrap()))
                    .collect();
                out.push(Node::Sup { arg });
                col = run_end + 1;
            }
            _ if unsubscript_char(ch).is_some() => {
                let run_end = scan_while(g, bl, col, rect.r, |c| unsubscript_char(c).is_some());
                let arg = (col..=run_end)
                    .map(|c| Node::Sym(unsubscript_char(g.at(bl, c)).unwrap()))
                    .collect();
                out.push(Node::Sub { arg });
                col = run_end + 1;
            }
            _ if ch.is_ascii_alphabetic() => {
                // Upright ASCII letters: function names (canonical), with
                // leftover letters accepted as plain atoms (lenient input).
                let run_end = scan_while(g, bl, col, rect.r, |c| c.is_ascii_alphabetic());
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
                // A mark in the cell directly above (or below) an atom is
                // an accent; those cells are otherwise always blank.
                let over = bl > rect.t && is_over_mark(g.at(bl - 1, col));
                let under = bl < rect.b && is_under_mark(g.at(bl + 1, col));
                let base = unstyle_char(ch);
                if over && under {
                    return err("stacked accents are not supported", bl, col);
                } else if over {
                    out.push(Node::Accent { accent: g.at(bl - 1, col), base });
                } else if under {
                    out.push(Node::Accent { accent: g.at(bl + 1, col), base });
                } else if bigop_by_char(ch) {
                    // A bare big operator is a BigOp with empty limits.
                    out.push(Node::BigOp { op: ch, lower: vec![], upper: vec![] });
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
    out: &mut Row,
) -> Result<()> {
    let mut parts: Vec<(usize, bool, Rect)> = Vec::new();
    let run_span = Rect { t: rect.t, b: rect.b, l: from, r: to };
    for (side_rect, is_sup) in [
        (region_above(run_span, bl), true),
        (region_below(run_span, bl), false),
    ] {
        let Some(side) = side_rect else { continue };
        let protected = protected_cols(g, side, None);
        let occupied: Vec<bool> = side
            .cols()
            .map(|c| !col_blank(g, side, c) || protected[c - from])
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
        let arg = parse_region(g, r, None, depth + 1, trace)?;
        out.push(if is_sup { Node::Sup { arg } } else { Node::Sub { arg } });
    }
    Ok(())
}

/// Column of the matching `close` for the `open` at (row, col), scanning
/// right with depth counting (nested pairs whose top row coincides).
fn match_on_row(g: &Grid, row: usize, col: usize, max: usize, open: char, close: char) -> Result<usize> {
    let mut depth = 0;
    for c in col..=max {
        let ch = g.at(row, c);
        if ch == open {
            depth += 1;
        } else if ch == close {
            depth -= 1;
            if depth == 0 {
                return Ok(c);
            }
        }
    }
    err(format!("unmatched {}", open), row, col)
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

/// Split the interior of a matrix into cells.
/// Row separators: fully blank rows. Column separators: runs of >= 2 fully
/// blank, unprotected columns (see `protected_cols`).
fn parse_matrix(g: &Grid, inner: Rect, depth: usize, trace: &mut Vec<RegionSpan>) -> Result<Node> {
    let inner = match trim(g, inner) {
        Some(r) => r,
        None => return err("empty matrix", inner.t, inner.l),
    };

    let mut row_segs: Vec<(usize, usize)> = Vec::new();
    let mut r = inner.t;
    while r <= inner.b {
        if row_blank(g, inner, r) {
            r += 1;
            continue;
        }
        let start = r;
        while r <= inner.b && !row_blank(g, inner, r) {
            r += 1;
        }
        row_segs.push((start, r - 1));
    }

    let protected = protected_cols(g, inner, None);
    let mut col_segs: Vec<(usize, usize)> = Vec::new();
    let mut c = inner.l;
    while c <= inner.r {
        let is_sep_start = |c: usize| col_blank(g, inner, c) && !protected[c - inner.l];
        if is_sep_start(c) {
            // Only runs of >= 2 blank columns separate cells.
            let start = c;
            while c <= inner.r && is_sep_start(c) {
                c += 1;
            }
            if c - start < 2 {
                // Single blank column: belongs to the current cell.
                if let Some(last) = col_segs.last_mut() {
                    last.1 = c - 1;
                }
            }
            continue;
        }
        let start = c;
        while c <= inner.r && !is_sep_start(c) {
            c += 1;
        }
        match col_segs.last_mut() {
            // Merge with previous segment if separated by a single blank.
            Some(last) if last.1 + 1 == start => last.1 = c - 1,
            _ => col_segs.push((start, c - 1)),
        }
    }

    let (rows, cols) = (row_segs.len(), col_segs.len());
    if rows == 0 || cols == 0 {
        return err("matrix with no cells", inner.t, inner.l);
    }
    let mut cells = Vec::with_capacity(rows * cols);
    for &(rt, rb) in &row_segs {
        for &(cl, cr) in &col_segs {
            cells.push(parse_region(g, Rect { t: rt, b: rb, l: cl, r: cr }, None, depth, trace)?);
        }
    }
    Ok(Node::Matrix { rows, cols, cells })
}

/// Parse a formula from its AA text form.
pub fn parse(text: &str) -> Result<Row> {
    parse_with_regions(text).map(|(row, _)| row)
}

/// Like `parse`, but also return every structural region's rectangle and
/// nesting depth (for the TUI structure view).
pub fn parse_with_regions(text: &str) -> Result<(Row, Vec<RegionSpan>)> {
    let lines: Vec<Vec<char>> = text
        .lines()
        .map(|l| l.trim_end().replace('\t', " ").chars().collect())
        .collect();
    if lines.iter().all(|l| l.is_empty()) {
        return Ok((vec![], vec![]));
    }
    let width = lines.iter().map(|l| l.len()).max().unwrap();
    let mut g = Grid {
        g: lines
            .into_iter()
            .map(|mut l| {
                l.resize(width, ' ');
                l
            })
            .collect(),
    };
    // Optional explicit baseline marker (hand-written input aid).
    let mut baseline = None;
    let markers: Vec<(usize, usize)> = g
        .g
        .iter()
        .enumerate()
        .flat_map(|(r, line)| {
            line.iter()
                .enumerate()
                .filter(|&(_, &c)| c == BASELINE_MARKER)
                .map(move |(c, _)| (r, c))
        })
        .collect();
    match markers[..] {
        [] => {}
        [(r, c)] => {
            g.g[r][c] = ' ';
            baseline = Some(r);
        }
        _ => return err("multiple ▶ baseline markers", markers[1].0, markers[1].1),
    }
    let rect = Rect { t: 0, b: g.g.len() - 1, l: 0, r: width - 1 };
    let mut trace = Vec::new();
    match trim(&g, rect) {
        // Normalize: a script arg that mixes padded structures with atoms
        // parses as adjacent script chunks; merging them restores the
        // canonical single node (render is only defined on normal forms).
        Some(rect) => parse_region(&g, rect, baseline, 0, &mut trace)
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
        roundtrip(&vec![Node::Accent { accent: '^', base: 'x' }]);
        roundtrip(&vec![Node::Accent { accent: '‗', base: 'y' }]);
        roundtrip(&vec![Node::Func("sin".into()), Node::Sym('x')]);
        roundtrip(&vec![Node::Matrix {
            rows: 2,
            cols: 2,
            cells: vec![syms("a"), syms("b+1"), vec![], syms("d")],
        }]);
        roundtrip(&vec![Node::Paren { inner: syms("a+b") }]);
        roundtrip(&vec![Node::BigOp { op: '∑', lower: syms("i=0"), upper: syms("n") }]);
        roundtrip(&vec![Node::BigOp { op: '∫', lower: vec![], upper: vec![] }]);
    }

    #[test]
    fn parses_handwritten_ascii() {
        let row = parse("x+1").unwrap();
        assert_eq!(row_to_latex(&row), "x+1");
        let row = parse("E=mc²").unwrap();
        assert_eq!(row_to_latex(&row), "E=mc^{2}");
    }

    #[test]
    fn ambiguity_counterexample_is_now_distinguishable() {
        // The two ASTs that rendered identically before the band notation.
        let ast1 = vec![Node::BigOp {
            op: '∑',
            lower: syms("n=1"),
            upper: vec![Node::BigOp { op: '∫', lower: vec![], upper: vec![] }],
        }];
        let ast2 = vec![Node::BigOp {
            op: '∫',
            lower: vec![Node::BigOp { op: '∑', lower: syms("n=1"), upper: vec![] }],
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
