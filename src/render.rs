//! 2D layout engine: AST -> rectangular block of chars with a baseline.
//! Prefers Unicode math glyphs (─ fraction bars, ⎛⎜⎝ scaled parens, inline
//! superscript/subscript chars like x², math-italic letters).

use crate::ast::{Field, Node, Row};
use crate::symbols::is_spaced_op;

pub const CURSOR_CHAR: char = '▌';
pub const PLACEHOLDER: char = '⬚';

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
fn italic_char(c: char) -> char {
    match c {
        'h' => 'ℎ', // U+1D455 is unassigned; Unicode uses PLANCK CONSTANT
        'a'..='z' => char::from_u32(0x1D44E + (c as u32 - 'a' as u32)).unwrap(),
        'A'..='Z' => char::from_u32(0x1D434 + (c as u32 - 'A' as u32)).unwrap(),
        _ => c,
    }
}

fn display_char(c: char, italic: bool) -> char {
    match c {
        '-' => '−',
        '*' => '∗',
        c if italic => italic_char(c),
        c => c,
    }
}

fn superscript_char(c: char) -> Option<char> {
    Some(match c {
        '0' => '⁰', '1' => '¹', '2' => '²', '3' => '³', '4' => '⁴',
        '5' => '⁵', '6' => '⁶', '7' => '⁷', '8' => '⁸', '9' => '⁹',
        '+' => '⁺', '-' => '⁻', '=' => '⁼', '(' => '⁽', ')' => '⁾',
        'n' => 'ⁿ', 'i' => 'ⁱ',
        _ => return None,
    })
}

fn subscript_char(c: char) -> Option<char> {
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

pub struct RenderCtx {
    pub italic: bool,
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
            (None, true) => Block::from_chars(vec![PLACEHOLDER]),
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
        blocks.push(render_node(node, child_cursor, ctx));
    }
    if let Some(col) = cursor_col {
        blocks.insert(col, Block::from_chars(vec![CURSOR_CHAR]));
    }
    hcat(&blocks)
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
            let d = display_char(*c, ctx.italic);
            if is_spaced_op(*c) {
                Block::from_chars(vec![' ', d, ' '])
            } else if *c == ',' {
                Block::from_chars(vec![d, ' '])
            } else {
                Block::from_chars(vec![d])
            }
        }

        Node::Frac { num, den } => {
            let n = render_row(num, cur(Field::FracNum), true, ctx);
            let d = render_row(den, cur(Field::FracDen), true, ctx);
            let w = n.width().max(d.width()) + 2;
            let mut lines = center_pad(&n, w);
            let baseline = lines.len();
            lines.push(vec!['─'; w]);
            lines.extend(center_pad(&d, w));
            Block { lines, baseline }
        }

        Node::Sqrt { arg } => {
            let a = render_row(arg, cur(Field::SqrtArg), true, ctx);
            let h = a.height();
            let w = a.width();
            // Overline row on top, radical sign hugging the bottom-left:
            //  ___
            // √x+1
            let mut lines = Vec::with_capacity(h + 1);
            let mut top = vec![' '; w + 1];
            for c in top.iter_mut().skip(1) {
                *c = '_';
            }
            lines.push(top);
            for (r, line) in a.lines.iter().enumerate() {
                let head = if r == h - 1 { '√' } else { '│' };
                let mut row = Vec::with_capacity(w + 1);
                row.push(head);
                row.extend_from_slice(line);
                lines.push(row);
            }
            Block { lines, baseline: a.baseline + 1 }
        }

        Node::Sup { arg } => {
            if cursor.is_none() {
                if let Some(chars) = inline_script(arg, superscript_char) {
                    return Block::from_chars(chars);
                }
            }
            let a = render_row(arg, cur(Field::SupArg), true, ctx);
            let h = a.height();
            Block { lines: a.lines, baseline: h }
        }

        Node::Sub { arg } => {
            if cursor.is_none() {
                if let Some(chars) = inline_script(arg, subscript_char) {
                    return Block::from_chars(chars);
                }
            }
            let a = render_row(arg, cur(Field::SubArg), true, ctx);
            let mut lines = vec![vec![' '; a.width()]];
            lines.extend(a.lines);
            Block { lines, baseline: 0 }
        }

        Node::BigOp { op, lower, upper } => {
            let u = render_row(upper, cur(Field::OpUpper), false, ctx);
            let l = render_row(lower, cur(Field::OpLower), false, ctx);
            let w = u.width().max(l.width()).max(1);
            let op_block = Block::from_chars(vec![*op]);
            let mut lines = center_pad(&u, w);
            let baseline = lines.len();
            lines.extend(center_pad(&op_block, w));
            lines.extend(center_pad(&l, w));
            // One space of margin on each side for readability.
            let lines = lines
                .into_iter()
                .map(|mut row| {
                    row.insert(0, ' ');
                    row.push(' ');
                    row
                })
                .collect();
            Block { lines, baseline }
        }

        Node::Paren { inner } => {
            let a = render_row(inner, cur(Field::ParenInner), true, ctx);
            let h = a.height().max(1);
            let mut lines = Vec::with_capacity(h);
            for (r, line) in a.lines.iter().enumerate() {
                let (lc, rc) = if h == 1 {
                    ('(', ')')
                } else if r == 0 {
                    ('⎛', '⎞')
                } else if r == h - 1 {
                    ('⎝', '⎠')
                } else {
                    ('⎜', '⎟')
                };
                let mut row = Vec::with_capacity(a.width() + 2);
                row.push(lc);
                row.extend_from_slice(line);
                row.push(rc);
                lines.push(row);
            }
            Block { lines, baseline: a.baseline }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sym_row(s: &str) -> Row {
        s.chars().map(Node::Sym).collect()
    }

    fn plain(root: &Row) -> Vec<String> {
        render_row(root, None, false, &RenderCtx { italic: false }).to_strings()
    }

    #[test]
    fn fraction_renders_with_bar() {
        let root = vec![Node::Frac { num: sym_row("1"), den: sym_row("x+1") }];
        assert_eq!(plain(&root), vec!["   1   ", "───────", " x + 1 "]);
    }

    #[test]
    fn inline_superscript() {
        let root = vec![Node::Sym('x'), Node::Sup { arg: sym_row("2") }];
        assert_eq!(plain(&root), vec!["x²"]);
    }

    #[test]
    fn bigop_with_limits() {
        let root = vec![Node::BigOp {
            op: '∑',
            lower: sym_row("i=0"),
            upper: sym_row("n"),
        }];
        let lines = plain(&root);
        assert_eq!(lines, vec!["   n   ", "   ∑   ", " i = 0 "]);
    }

    #[test]
    fn sqrt_single_line() {
        let root = vec![Node::Sqrt { arg: sym_row("2") }];
        assert_eq!(plain(&root), vec![" _", "√2"]);
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
        assert!(lines[1].starts_with("a + "));
    }
}
