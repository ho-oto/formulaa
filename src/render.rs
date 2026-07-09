//! 2D layout engine: AST -> rectangular block of chars with a baseline.
//!
//! The cursor-free output is the *canonical AA form* (see docs/aa-spec.md):
//! it is designed so that `parse.rs` can deterministically invert it back to
//! the AST. Every layout rule here has a matching rule in the parser —
//! change them in lockstep and keep the roundtrip tests green.

use crate::ast::{Field, Node, Row};
use crate::symbols::is_spaced_op;

pub const CURSOR_CHAR: char = '▌';
/// Placeholder for an empty mandatory slot, and explicit base of a script
/// that starts a row (so `[Sup(x)]` is distinguishable from `[Sym(x)]`).
pub const PLACEHOLDER: char = '⬚';
/// Fraction bar. Distinct from '-' (rendered '−') and the big-op band.
pub const FRAC_BAR: char = '─';
/// Big-operator band: marks the horizontal extent of over/under limits.
pub const OP_BAND: char = '┄';

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    /// Rectangular grid (all lines same length).
    pub lines: Vec<Vec<char>>,
    /// Index of the baseline row. May equal `height()` for blocks that sit
    /// entirely above the baseline (superscripts) — never index with it.
    pub baseline: usize,
}

impl Block {
    pub fn empty() -> Self {
        Block { lines: vec![], baseline: 0 }
    }

    pub fn from_chars(chars: Vec<char>) -> Self {
        Block { lines: vec![chars], baseline: 0 }
    }

    pub fn width(&self) -> usize {
        self.lines.first().map_or(0, |l| l.len())
    }

    pub fn height(&self) -> usize {
        self.lines.len()
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    pub fn to_strings(&self) -> Vec<String> {
        self.lines.iter().map(|l| l.iter().collect()).collect()
    }

    pub fn to_text(&self) -> String {
        self.to_strings()
            .iter()
            .map(|l| l.trim_end())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Char at the baseline row edge (None for scripts, which have no
    /// baseline row of their own).
    fn baseline_edge(&self, first: bool) -> Option<char> {
        let line = self.lines.get(self.baseline)?;
        if first { line.first() } else { line.last() }.copied()
    }
}

/// Horizontally concatenate blocks, aligning baselines.
fn hcat(blocks: &[Block]) -> Block {
    let blocks: Vec<&Block> = blocks.iter().filter(|b| !b.is_empty()).collect();
    if blocks.is_empty() {
        return Block::empty();
    }
    let above = blocks.iter().map(|b| b.baseline).max().unwrap();
    let below = blocks
        .iter()
        .map(|b| b.height().saturating_sub(b.baseline))
        .max()
        .unwrap();
    let height = above + below;
    let width: usize = blocks.iter().map(|b| b.width()).sum();
    let mut grid = vec![vec![' '; width]; height];
    let mut x = 0;
    for b in blocks {
        let y0 = above - b.baseline;
        for (dy, line) in b.lines.iter().enumerate() {
            for (dx, &c) in line.iter().enumerate() {
                grid[y0 + dy][x + dx] = c;
            }
        }
        x += b.width();
    }
    Block { lines: grid, baseline: above }
}

/// Pad every line of `b` to `width`, centered.
fn center_pad(b: &Block, width: usize) -> Vec<Vec<char>> {
    let left = (width - b.width()) / 2;
    b.lines
        .iter()
        .map(|line| {
            let mut row = vec![' '; width];
            row[left..left + line.len()].copy_from_slice(line);
            row
        })
        .collect()
}

/// Math-italic mapping for rendered letters (LaTeX output keeps ASCII).
pub fn italic_char(c: char) -> char {
    match c {
        'h' => 'ℎ', // U+1D455 is unassigned; Unicode uses PLANCK CONSTANT
        'a'..='z' => char::from_u32(0x1D44E + (c as u32 - 'a' as u32)).unwrap(),
        'A'..='Z' => char::from_u32(0x1D434 + (c as u32 - 'A' as u32)).unwrap(),
        _ => c,
    }
}

/// Inverse of the render-time character styling (used by the parser).
pub fn unstyle_char(c: char) -> char {
    match c {
        'ℎ' => 'h',
        '−' => '-',
        '∗' => '*',
        c => {
            let u = c as u32;
            if (0x1D44E..=0x1D467).contains(&u) {
                char::from_u32('a' as u32 + (u - 0x1D44E)).unwrap()
            } else if (0x1D434..=0x1D44D).contains(&u) {
                char::from_u32('A' as u32 + (u - 0x1D434)).unwrap()
            } else {
                c
            }
        }
    }
}

fn display_char(c: char, ctx: &RenderCtx) -> char {
    if ctx.is_compat() {
        // ASCII stays ASCII (math-italic letters are not monospaced in
        // most fonts); a few symbols get safer equivalents.
        return match c {
            '⋅' => '·',
            c => c,
        };
    }
    match c {
        '-' => '−',
        '*' => '∗',
        c if ctx.italic => italic_char(c),
        c => c,
    }
}

pub fn superscript_char(c: char) -> Option<char> {
    Some(match c {
        '0' => '⁰', '1' => '¹', '2' => '²', '3' => '³', '4' => '⁴',
        '5' => '⁵', '6' => '⁶', '7' => '⁷', '8' => '⁸', '9' => '⁹',
        '+' => '⁺', '-' => '⁻', '=' => '⁼', '(' => '⁽', ')' => '⁾',
        'n' => 'ⁿ', 'i' => 'ⁱ',
        _ => return None,
    })
}

pub fn subscript_char(c: char) -> Option<char> {
    Some(match c {
        '0' => '₀', '1' => '₁', '2' => '₂', '3' => '₃', '4' => '₄',
        '5' => '₅', '6' => '₆', '7' => '₇', '8' => '₈', '9' => '₉',
        '+' => '₊', '-' => '₋', '=' => '₌', '(' => '₍', ')' => '₎',
        'a' => 'ₐ', 'e' => 'ₑ', 'h' => 'ₕ', 'i' => 'ᵢ', 'j' => 'ⱼ',
        'k' => 'ₖ', 'l' => 'ₗ', 'm' => 'ₘ', 'n' => 'ₙ', 'o' => 'ₒ',
        'p' => 'ₚ', 'r' => 'ᵣ', 's' => 'ₛ', 't' => 'ₜ', 'u' => 'ᵤ',
        'v' => 'ᵥ', 'x' => 'ₓ',
        _ => return None,
    })
}

pub fn unsuperscript_char(c: char) -> Option<char> {
    "⁰¹²³⁴⁵⁶⁷⁸⁹⁺⁻⁼⁽⁾ⁿⁱ"
        .chars()
        .position(|x| x == c)
        .map(|i| "0123456789+-=()ni".chars().nth(i).unwrap())
}

pub fn unsubscript_char(c: char) -> Option<char> {
    "₀₁₂₃₄₅₆₇₈₉₊₋₌₍₎ₐₑₕᵢⱼₖₗₘₙₒₚᵣₛₜᵤᵥₓ"
        .chars()
        .position(|x| x == c)
        .map(|i| "0123456789+-=()aehijklmnoprstuvx".chars().nth(i).unwrap())
}

/// If every node in `row` is a plain char with an inline script equivalent,
/// return the converted chars.
fn inline_script(row: &Row, map: fn(char) -> Option<char>) -> Option<Vec<char>> {
    if row.is_empty() {
        return None;
    }
    row.iter()
        .map(|n| match n {
            Node::Sym(c) => map(*c),
            _ => None,
        })
        .collect()
}

/// Which glyph repertoire the renderer may use.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum GlyphSet {
    /// Canonical form: math italics, inline scripts, ⎛⎡ brackets, ∑√ …
    /// This is the parseable interchange format.
    Unicode,
    /// Display-only fallback for fonts/terminals where Unicode math glyphs
    /// are missing or not monospaced: ASCII letters (no math italics; the
    /// upright-Func distinction is lost, so this form is NOT parseable),
    /// no inline scripts, box-drawing structural glyphs (tex2utf style).
    Compat,
}

#[derive(Clone, Copy)]
pub struct RenderCtx {
    pub italic: bool,
    /// Script style: no spacing around binary operators. Set inside
    /// scripts, big-op limits and matrix cells; required for parseability.
    pub compact: bool,
    pub glyphs: GlyphSet,
}

impl RenderCtx {
    pub fn canonical() -> Self {
        RenderCtx { italic: true, compact: false, glyphs: GlyphSet::Unicode }
    }

    pub fn compat() -> Self {
        RenderCtx { italic: false, compact: false, glyphs: GlyphSet::Compat }
    }

    fn compact(self) -> Self {
        RenderCtx { compact: true, ..self }
    }

    fn is_compat(&self) -> bool {
        self.glyphs == GlyphSet::Compat
    }

    fn placeholder(&self) -> char {
        if self.is_compat() { '□' } else { PLACEHOLDER }
    }
}

/// Cursor position relative to the row being rendered: the remaining path
/// into descendants, and the column once the path is exhausted.
type CursorRef<'a> = (&'a [(usize, Field)], usize);

/// `placeholder`: render an empty row as ⬚ (used for mandatory slots like
/// fraction numerators; big-operator limits pass false so empty limits vanish).
pub fn render_row(
    row: &Row,
    cursor: Option<CursorRef>,
    placeholder: bool,
    ctx: &RenderCtx,
) -> Block {
    let cursor_col = match cursor {
        Some(([], col)) => Some(col),
        _ => None,
    };

    if row.is_empty() {
        return match (cursor_col, placeholder) {
            (Some(_), _) => Block::from_chars(vec![CURSOR_CHAR]),
            (None, true) => Block::from_chars(vec![ctx.placeholder()]),
            (None, false) => Block::empty(),
        };
    }

    let mut blocks: Vec<Block> = Vec::with_capacity(row.len() + 1);
    for (i, node) in row.iter().enumerate() {
        let child_cursor = match cursor {
            Some((path, col)) => match path.first() {
                Some(&(pi, pf)) if pi == i => Some((pf, (&path[1..], col))),
                _ => None,
            },
            None => None,
        };
        let mut block = render_node(node, child_cursor, ctx);
        // A script at the start of a row gets an explicit ⬚ base, so the
        // picture differs from the row without the script wrapper.
        if i == 0 && matches!(node, Node::Sup { .. } | Node::Sub { .. }) {
            block = hcat(&[Block::from_chars(vec![ctx.placeholder()]), block]);
        }
        blocks.push(block);
    }
    if let Some(col) = cursor_col {
        blocks.insert(col, Block::from_chars(vec![CURSOR_CHAR]));
    }
    // Two identical bar glyphs (── or ┄┄ from adjacent fractions/big-ops)
    // would merge into one run; keep them apart with a one-column spacer.
    // No blanket margins: any fully blank column inside a row must separate
    // same-baseline siblings, or script-region segmentation breaks.
    let mut spaced: Vec<Block> = Vec::with_capacity(blocks.len() * 2);
    for block in blocks {
        let touching_bars = match (
            spaced.last().and_then(|b: &Block| b.baseline_edge(false)),
            block.baseline_edge(true),
        ) {
            (Some(a), Some(b)) => a == b && (a == FRAC_BAR || a == OP_BAND),
            _ => false,
        };
        if touching_bars {
            spaced.push(Block::from_chars(vec![' ']));
        }
        spaced.push(block);
    }
    hcat(&spaced)
}

fn render_node(
    node: &Node,
    cursor: Option<(Field, CursorRef)>,
    ctx: &RenderCtx,
) -> Block {
    // Cursor for a specific field of this node.
    let cur = |f: Field| -> Option<CursorRef> {
        match cursor {
            Some((cf, c)) if cf == f => Some(c),
            _ => None,
        }
    };

    match node {
        Node::Sym(c) => {
            let d = display_char(*c, ctx);
            if ctx.compact {
                Block::from_chars(vec![d])
            } else if is_spaced_op(*c) {
                Block::from_chars(vec![' ', d, ' '])
            } else if *c == ',' {
                Block::from_chars(vec![d, ' '])
            } else {
                Block::from_chars(vec![d])
            }
        }

        // Upright letters are reserved for function names (plain letters
        // render math-italic), which is what makes them parseable.
        Node::Func(name) => Block::from_chars(name.chars().collect()),

        // Mark in the cell directly above (or below) the base. That cell is
        // never used by anything else (scripts go up-right, limits live
        // inside their band), so a one-cell probe parses it back.
        Node::Accent { accent, base } => {
            let b = display_char(*base, ctx);
            let mark = if ctx.is_compat() {
                match *accent {
                    '⇀' => '→',
                    '˜' => '~',
                    m => m,
                }
            } else {
                *accent
            };
            if crate::symbols::is_under_mark(*accent) {
                Block { lines: vec![vec![b], vec![mark]], baseline: 0 }
            } else {
                Block { lines: vec![vec![mark], vec![b]], baseline: 1 }
            }
        }

        Node::Frac { num, den } => {
            let n = render_row(num, cur(Field::FracNum), true, ctx);
            let d = render_row(den, cur(Field::FracDen), true, ctx);
            let w = n.width().max(d.width()) + 2;
            let mut lines = center_pad(&n, w);
            let baseline = lines.len();
            lines.push(vec![FRAC_BAR; w]);
            lines.extend(center_pad(&d, w));
            Block { lines, baseline }
        }

        Node::Sqrt { arg, index } => {
            let a = render_row(arg, cur(Field::SqrtArg), true, ctx);
            let h = a.height();
            let w = a.width();
            if ctx.is_compat() {
                // tex2utf style:   ___        ____
                //                 \/ 2       |  1
                //                            \|  2   (index shown as prefix digit)
                let mut lines = Vec::with_capacity(h + 1);
                let mut top = vec![' '; w + 2];
                for c in top.iter_mut().skip(2) {
                    *c = '_';
                }
                lines.push(top);
                for (r, line) in a.lines.iter().enumerate() {
                    let head = if h == 1 {
                        ['\\', '/']
                    } else if r == h - 1 {
                        ['\\', '│']
                    } else {
                        [' ', '│']
                    };
                    let mut row = Vec::with_capacity(w + 2);
                    row.extend(head);
                    row.extend_from_slice(line);
                    lines.push(row);
                }
                let mut block = Block { lines, baseline: a.baseline + 1 };
                if *index != 2 {
                    let digit = char::from_digit(*index as u32, 10).unwrap();
                    block = hcat(&[Block::from_chars(vec![digit]), block]);
                }
                return block;
            }
            // Overline row on top, radical stem hugging the left:
            //  ___
            // √x+1
            let radical = match index {
                3 => '∛',
                4 => '∜',
                _ => '√',
            };
            let mut lines = Vec::with_capacity(h + 1);
            let mut top = vec![' '; w + 1];
            for c in top.iter_mut().skip(1) {
                *c = '_';
            }
            lines.push(top);
            for (r, line) in a.lines.iter().enumerate() {
                let head = if r == h - 1 { radical } else { '│' };
                let mut row = Vec::with_capacity(w + 1);
                row.push(head);
                row.extend_from_slice(line);
                lines.push(row);
            }
            Block { lines, baseline: a.baseline + 1 }
        }

        Node::Sup { arg } => {
            if cursor.is_none() && !ctx.is_compat() {
                if let Some(chars) = inline_script(arg, superscript_char) {
                    return Block::from_chars(chars);
                }
            }
            let a = render_row(arg, cur(Field::SupArg), true, &ctx.compact());
            let h = a.height();
            Block { lines: a.lines, baseline: h }
        }

        Node::Sub { arg } => {
            if cursor.is_none() && !ctx.is_compat() {
                if let Some(chars) = inline_script(arg, subscript_char) {
                    return Block::from_chars(chars);
                }
            }
            let a = render_row(arg, cur(Field::SubArg), true, &ctx.compact());
            let mut lines = vec![vec![' '; a.width()]];
            lines.extend(a.lines);
            Block { lines, baseline: 0 }
        }

        Node::BigOp { op, lower, upper } => {
            let u = render_row(upper, cur(Field::OpUpper), false, &ctx.compact());
            let l = render_row(lower, cur(Field::OpLower), false, &ctx.compact());
            if ctx.is_compat() {
                // Multi-row box-drawing art with limits stacked over/under
                // (display only: without the band this layout is ambiguous).
                let art = compat_op_art(*op);
                let w = art.width().max(u.width()).max(l.width());
                let mut lines = center_pad(&u, w);
                let baseline = lines.len() + art.baseline;
                lines.extend(center_pad(&art, w));
                lines.extend(center_pad(&l, w));
                return Block { lines, baseline };
            }
            if u.is_empty() && l.is_empty() && cursor.is_none() {
                // No limits: a bare operator character.
                return Block::from_chars(vec![*op]);
            }
            // Band marks the horizontal extent of the limits; this is what
            // makes over/under limits unambiguous (see docs/aa-spec.md).
            let w = u.width().max(l.width()).max(1) + 2;
            let mut band = vec![OP_BAND; w];
            band[(w - 1) / 2] = *op;
            let mut lines = center_pad(&u, w);
            let baseline = lines.len();
            lines.push(band);
            lines.extend(center_pad(&l, w));
            Block { lines, baseline }
        }

        Node::Paren { inner } => {
            let a = render_row(inner, cur(Field::ParenInner), true, ctx);
            let h = a.height().max(1);
            let mut lines = Vec::with_capacity(h);
            for (r, line) in a.lines.iter().enumerate() {
                let compat = ctx.is_compat();
                let (lc, rc) = if h == 1 {
                    ('(', ')')
                } else if r == 0 {
                    if compat { ('╭', '╮') } else { ('⎛', '⎞') }
                } else if r == h - 1 {
                    if compat { ('╰', '╯') } else { ('⎝', '⎠') }
                } else if compat { ('│', '│') } else { ('⎜', '⎟') };
                let mut row = Vec::with_capacity(a.width() + 2);
                row.push(lc);
                row.extend_from_slice(line);
                row.push(rc);
                lines.push(row);
            }
            Block { lines, baseline: a.baseline }
        }

        Node::Matrix { rows, cols, cells } => {
            render_matrix(*rows, *cols, cells, cursor, ctx)
        }
    }
}

/// Box-drawing/ASCII art for big operators in compat mode.
fn compat_op_art(op: char) -> Block {
    let (rows, baseline): (&[&str], usize) = match op {
        '∑' => (&["__", "> ", "‾‾"], 1),
        '∏' => (&["___", "│ │", "│ │"], 1),
        '∫' | '∬' => (&["╭", "│", "╯"], 1),
        '∮' => (&["╭", "O", "╯"], 1),
        _ => return Block::from_chars(vec![op]),
    };
    Block {
        lines: rows.iter().map(|r| r.chars().collect()).collect(),
        baseline,
    }
}

fn render_matrix(
    rows: usize,
    cols: usize,
    cells: &[Row],
    cursor: Option<(Field, CursorRef)>,
    ctx: &RenderCtx,
) -> Block {
    let cctx = ctx.compact();
    let blocks: Vec<Block> = cells
        .iter()
        .enumerate()
        .map(|(i, cell)| {
            let cur = match cursor {
                Some((Field::Cell(ci), c)) if ci == i => Some(c),
                _ => None,
            };
            render_row(cell, cur, true, &cctx)
        })
        .collect();

    let col_w: Vec<usize> = (0..cols)
        .map(|j| (0..rows).map(|i| blocks[i * cols + j].width()).max().unwrap_or(1))
        .collect();

    // Each grid row: cells centered in their column, baseline-aligned,
    // separated by exactly two blank columns (the column-separator rule).
    let gap = Block { lines: vec![vec![' ', ' ']], baseline: 0 };
    let mut body: Vec<Vec<char>> = Vec::new();
    for i in 0..rows {
        let mut parts: Vec<Block> = Vec::new();
        for j in 0..cols {
            if j > 0 {
                parts.push(gap.clone());
            }
            let b = &blocks[i * cols + j];
            let centered = Block { lines: center_pad(b, col_w[j]), baseline: b.baseline };
            parts.push(centered);
        }
        let row_block = hcat(&parts);
        if i > 0 {
            // Exactly one blank line separates grid rows (row-separator rule).
            body.push(vec![' '; row_block.width()]);
        }
        body.extend(row_block.lines);
    }
    let w = body.iter().map(|l| l.len()).max().unwrap_or(0);
    for l in &mut body {
        l.resize(w, ' ');
    }

    let h = body.len().max(1);
    if h == 1 {
        let mut row = vec!['['];
        row.extend(body.into_iter().next().unwrap_or_default());
        row.push(']');
        return Block { lines: vec![row], baseline: 0 };
    }
    let compat = ctx.is_compat();
    let mut lines = Vec::with_capacity(h);
    for (r, line) in body.into_iter().enumerate() {
        let (lc, rc) = if r == 0 {
            if compat { ('┌', '┐') } else { ('⎡', '⎤') }
        } else if r == h - 1 {
            if compat { ('└', '┘') } else { ('⎣', '⎦') }
        } else if compat { ('│', '│') } else { ('⎢', '⎥') };
        let mut row = Vec::with_capacity(w + 2);
        row.push(lc);
        row.extend(line);
        row.push(rc);
        lines.push(row);
    }
    // Matches the parser: baseline of a matrix is its vertical center.
    Block { lines, baseline: (h - 1) / 2 }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sym_row(s: &str) -> Row {
        s.chars().map(Node::Sym).collect()
    }

    fn plain(root: &Row) -> Vec<String> {
        let ctx = RenderCtx { italic: false, ..RenderCtx::canonical() };
        render_row(root, None, false, &ctx)
            .to_strings()
            .iter()
            .map(|l| l.trim_end().to_string())
            .collect()
    }

    #[test]
    fn fraction_renders_with_bar() {
        let root = vec![Node::Frac { num: sym_row("1"), den: sym_row("x+1") }];
        assert_eq!(plain(&root), vec!["   1", "───────", " x + 1"]);
    }

    #[test]
    fn inline_superscript() {
        let root = vec![Node::Sym('x'), Node::Sup { arg: sym_row("2") }];
        assert_eq!(plain(&root), vec!["x²"]);
    }

    #[test]
    fn bigop_band_marks_limit_extent() {
        let root = vec![Node::BigOp {
            op: '∑',
            lower: sym_row("i=0"),
            upper: sym_row("n"),
        }];
        assert_eq!(plain(&root), vec!["  n", "┄┄∑┄┄", " i=0"]);
    }

    #[test]
    fn bigop_without_limits_is_bare() {
        let root = vec![Node::BigOp { op: '∫', lower: vec![], upper: vec![] }];
        assert_eq!(plain(&root), vec!["∫"]);
    }

    #[test]
    fn sqrt_single_line() {
        let root = vec![Node::Sqrt { arg: sym_row("2"), index: 2 }];
        assert_eq!(plain(&root), vec![" _", "√2"]);
    }

    #[test]
    fn leading_script_gets_explicit_base() {
        let root = vec![Node::Sup { arg: sym_row("2") }];
        assert_eq!(plain(&root), vec!["⬚²"]);
    }

    #[test]
    fn matrix_2x2() {
        let root = vec![Node::Matrix {
            rows: 2,
            cols: 2,
            cells: vec![sym_row("a"), sym_row("b"), sym_row("c"), sym_row("d")],
        }];
        assert_eq!(plain(&root), vec!["⎡a  b⎤", "⎢    ⎥", "⎣c  d⎦"]);
    }

    #[test]
    fn func_renders_upright() {
        let root = vec![
            Node::Func("sin".into()),
            Node::Sym('x'),
        ];
        let ctx = RenderCtx::canonical();
        let b = render_row(&root, None, false, &ctx);
        assert_eq!(b.to_text(), "sin𝑥");
    }

    #[test]
    fn compat_mode_uses_limited_charset() {
        let root = vec![
            Node::BigOp { op: '∑', lower: sym_row("n=1"), upper: sym_row("∞") },
            Node::Frac {
                num: sym_row("1"),
                den: vec![Node::Sym('n'), Node::Sup { arg: sym_row("2") }],
            },
            Node::Paren { inner: vec![Node::Frac { num: sym_row("x"), den: sym_row("2") }] },
            Node::Matrix {
                rows: 2,
                cols: 2,
                cells: vec![sym_row("a"), sym_row("b"), sym_row("c"), sym_row("d")],
            },
            Node::Sqrt { arg: sym_row("2"), index: 2 },
        ];
        let text = render_row(&root, None, false, &RenderCtx::compat()).to_text();
        // Structure must use only ASCII / box-drawing / a few safe glyphs;
        // no ⎛⎡√∑², no math italics. User symbols (∞) pass through.
        for c in text.chars() {
            assert!(
                c.is_ascii() || "─│┌┐└┘╭╮╰╯‾·□→∞\n".contains(c),
                "unexpected char {:?} in compat output:\n{}",
                c,
                text
            );
        }
        assert!(text.contains('╭') && text.contains('┌') && text.contains("\\/"));
    }

    #[test]
    fn compat_bigop_art() {
        let root = vec![Node::BigOp { op: '∫', lower: sym_row("0"), upper: sym_row("1") }];
        let text = render_row(&root, None, false, &RenderCtx::compat()).to_text();
        assert_eq!(text, "1\n╭\n│\n╯\n0");
    }

    #[test]
    fn baseline_alignment_of_fraction_in_row() {
        let root = vec![
            Node::Sym('a'),
            Node::Sym('+'),
            Node::Frac { num: sym_row("1"), den: sym_row("2") },
        ];
        let lines = plain(&root);
        assert_eq!(lines.len(), 3);
        // 'a' sits on the fraction-bar row.
        assert!(lines[1].starts_with("a +"));
    }
}
