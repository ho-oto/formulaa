//! 2D layout engine: AST -> rectangular block of chars with a baseline.
//!
//! The cursor-free output is the *canonical AA form* (see docs/aa-spec.md):
//! it is designed so that `parse.rs` can deterministically invert it back to
//! the AST. Every layout rule here has a matching rule in the parser —
//! change them in lockstep and keep the roundtrip tests green.

use crate::ast::{Field, Node, Row};

pub const CURSOR_CHAR: char = '▌';
/// Placeholder for an empty mandatory slot, and explicit base of a script
/// that starts a row (so `[Sup(x)]` is distinguishable from `[Sym(x)]`).
pub const PLACEHOLDER: char = '⬚';
/// Fraction bar. Distinct from '-' (rendered '−') and the big-op band.
pub const FRAC_BAR: char = '─';
/// Big-operator band: marks the horizontal extent of over/under limits.
pub const OP_BAND: char = '┄';
/// Stretchy single-arrow bodies reuse the ─ bar: a bar run directly
/// capped by a head (`──>`) *is* the arrow; a fraction next to a `>` atom
/// is written with a separating space (space presence, not count, is what
/// changes the reading). Double arrows (⇒ ⇐) use a ═ body. Heads render
/// as ASCII < > ; the Unicode arrows are accepted as lenient input heads.
pub const DOUBLE_BODY: char = '═';

/// Body glyph for an arrow head.
pub fn arrow_body(op: char) -> char {
    if op == '⇒' || op == '⇐' { DOUBLE_BODY } else { FRAC_BAR }
}
/// Grid lattice markers: a bare Array frames itself with box-drawing
/// junctions at every crossing of its separator rows/columns including the
/// outer edges (┌ ┬ ┐ / ├ ┼ ┤ / └ ┴ ┘), so it needs no delimiter to have a
/// parseable extent, and the explicit corners make adjacent lattices
/// unambiguous.
pub const LATTICE_CHARS: &[char] = &['┌', '┬', '┐', '├', '┼', '┤', '└', '┴', '┘'];

/// Junction glyph for a lattice crossing at (row kind, col kind):
/// 0 = first, 1 = internal, 2 = last.
pub fn lattice_char(row_kind: usize, col_kind: usize) -> char {
    let table = [['┌', '┬', '┐'], ['├', '┼', '┤'], ['└', '┴', '┘']];
    table[row_kind][col_kind]
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    /// Rectangular grid (all lines same length).
    pub lines: Vec<Vec<char>>,
    /// Index of the baseline row. May equal `height()` for blocks that sit
    /// entirely above the baseline (superscripts) — never index with it.
    pub baseline: usize,
    /// Cells struck through by \cancel, as (row, col). Emitted as a
    /// combining long solidus (U+0338) after the cell char in text output.
    pub cancel: Vec<(usize, usize)>,
}

impl Block {
    pub fn new(lines: Vec<Vec<char>>, baseline: usize) -> Self {
        Block { lines, baseline, cancel: vec![] }
    }

    pub fn empty() -> Self {
        Block::new(vec![], 0)
    }

    pub fn from_chars(chars: Vec<char>) -> Self {
        Block::new(vec![chars], 0)
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
        let flagged: std::collections::HashSet<(usize, usize)> =
            self.cancel.iter().copied().collect();
        self.lines
            .iter()
            .enumerate()
            .map(|(r, l)| {
                let mut out = String::with_capacity(l.len() * 2);
                for (c, &ch) in l.iter().enumerate() {
                    out.push(ch);
                    if flagged.contains(&(r, c)) {
                        out.push('\u{338}');
                    }
                }
                out
            })
            .collect()
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
    let mut cancel = Vec::new();
    let mut x = 0;
    for b in blocks {
        let y0 = above - b.baseline;
        for (dy, line) in b.lines.iter().enumerate() {
            for (dx, &c) in line.iter().enumerate() {
                grid[y0 + dy][x + dx] = c;
            }
        }
        cancel.extend(b.cancel.iter().map(|&(r, c)| (y0 + r, x + c)));
        x += b.width();
    }
    Block { lines: grid, baseline: above, cancel }
}

/// Cancel coords of a child centered into `width` at vertical offset.
fn centered_cancel(b: &Block, width: usize, row_off: usize) -> impl Iterator<Item = (usize, usize)> + '_ {
    let left = (width - b.width()) / 2;
    b.cancel.iter().map(move |&(r, c)| (r + row_off, c + left))
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
            } else if (0x1D6FC..=0x1D714).contains(&u) {
                // Lenient input: math-italic Greek small letters -> plain.
                char::from_u32(0x3B1 + (u - 0x1D6FC)).unwrap()
            } else if (0x1D6E2..=0x1D6FA).contains(&u) {
                char::from_u32(0x391 + (u - 0x1D6E2)).unwrap()
            } else {
                c
            }
        }
    }
}

fn display_char(c: char, ctx: &RenderCtx) -> char {
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

#[derive(Clone, Copy)]
pub struct RenderCtx {
    pub italic: bool,
}

impl RenderCtx {
    pub fn canonical() -> Self {
        RenderCtx { italic: true }
    }

    fn placeholder(&self) -> char {
        PLACEHOLDER
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

    // Per-block info driving the spacer rules below.
    #[derive(Clone, Copy, Default)]
    struct Info {
        script: bool,
        script_2d: bool,
        cancel: bool,
    }

    let mut blocks: Vec<(Block, Info)> = Vec::with_capacity(row.len() + 1);
    for (i, node) in row.iter().enumerate() {
        let child_cursor = match cursor {
            Some((path, col)) => match path.first() {
                Some(&(pi, pf)) if pi == i => Some((pf, (&path[1..], col))),
                _ => None,
            },
            None => None,
        };
        let info = Info {
            script: matches!(node, Node::Sup { .. } | Node::Sub { .. }),
            script_2d: match node {
                Node::Sup { arg } => {
                    child_cursor.is_some() || inline_script(arg, superscript_char).is_none()
                }
                Node::Sub { arg } => {
                    child_cursor.is_some() || inline_script(arg, subscript_char).is_none()
                }
                _ => false,
            },
            cancel: matches!(node, Node::Cancel { .. }),
        };
        let mut block = render_node(node, child_cursor, ctx);
        // A script at the start of a row gets an explicit ⬚ base, so the
        // picture differs from the row without the script wrapper.
        if i == 0 && info.script {
            block = hcat(&[Block::from_chars(vec![ctx.placeholder()]), block]);
        }
        blocks.push((block, info));
    }
    if let Some(col) = cursor_col {
        blocks.insert(col, (Block::from_chars(vec![CURSOR_CHAR]), Info::default()));
    }

    // Single-space separators between siblings, inserted only where
    // adjacent glyphs would otherwise fuse into one token — space
    // *presence* (never count) is what changes a reading:
    //  - between two identical bar glyphs (── / ┄┄ / ══ would merge)
    //  - between a bar edge and an arrow head that would cap it (─ then →,
    //    ═ then ⇒) and between a head and a body that would absorb it
    //    (← then ─, ⇐ then ═)
    //  - after a cancel with a blank baseline edge (the strike-extent scan
    //    must not fuse the ragged edge with the neighbour)
    // A 2D script right after a cancel instead needs a non-blank baseline
    // anchor (⬚), or the strike scan would fuse raised content.
    let mut spaced: Vec<Block> = Vec::with_capacity(blocks.len() * 2);
    let mut prev: Option<Info> = None;
    for (block, info) in blocks {
        let need = match (prev, &spaced.last()) {
            (Some(p), Some(last)) => {
                let edges = (last.baseline_edge(false), block.baseline_edge(true));
                let fuse = match edges {
                    (Some(a), Some(b)) => {
                        // A band edge fuses with *anything* adjacent (the
                        // general ┄piece┄ grammar munches non-space runs).
                        a == OP_BAND
                            || b == OP_BAND
                            || (a == b && (a == FRAC_BAR || a == DOUBLE_BODY))
                            || (a == FRAC_BAR && (b == '>' || b == '→'))
                            || (a == DOUBLE_BODY && (b == '>' || b == '⇒'))
                            || ((a == '<' || a == '←') && b == FRAC_BAR)
                            || ((a == '<' || a == '⇐') && b == DOUBLE_BODY)
                    }
                    _ => false,
                };
                let ragged_cancel = p.cancel && last.baseline_edge(false) == Some(' ');
                fuse || ragged_cancel
            }
            _ => false,
        };
        let anchor = matches!(prev, Some(p) if p.cancel) && info.script_2d;
        if anchor {
            spaced.push(Block::from_chars(vec![ctx.placeholder()]));
        } else if need {
            spaced.push(Block::from_chars(vec![' ']));
        }
        spaced.push(block);
        prev = Some(info);
    }
    hcat(&spaced)
}

fn l_placeholder(cursor: Option<(Field, CursorRef)>) -> bool {
    cursor.is_some()
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
        Node::Spacer => Block::from_chars(vec![' ']),
        // No automatic spacing anywhere (operators included): spacing is
        // the user's, via formatting Spacers or the semantic ␣ atom.
        Node::Sym(c) => Block::from_chars(vec![display_char(*c, ctx)]),

        // Upright letters are reserved for function names (plain letters
        // render math-italic), which is what makes them parseable.
        Node::Func(name) => Block::from_chars(name.chars().collect()),

        // Quoted roman/text run; interior spaces drawn as ␣ so the quotes
        // never contain structurally meaningful blank columns.
        Node::Text(t) => {
            let mut chars = vec!['"'];
            chars.extend(t.chars().map(|c| if c == ' ' { '␣' } else { c }));
            chars.push('"');
            Block::from_chars(chars)
        }

        // Marks in the cells directly above/below the base, stacking
        // outward (innermost first in each list). Those cells are never
        // used by anything else (scripts go up-right, limits live inside
        // their band), so a column probe parses the stack back.
        Node::Accent { overs, unders, base } => {
            let b = display_char(*base, ctx);
            let mut lines: Vec<Vec<char>> = overs.iter().rev().map(|&m| vec![m]).collect();
            lines.push(vec![b]);
            lines.extend(unders.iter().map(|&m| vec![m]));
            Block::new(lines, overs.len())
        }

        Node::Frac { num, den } => {
            let n = render_row(num, cur(Field::FracNum), true, ctx);
            let d = render_row(den, cur(Field::FracDen), true, ctx);
            let w = n.width().max(d.width()) + 2;
            let mut lines = center_pad(&n, w);
            let baseline = lines.len();
            lines.push(vec![FRAC_BAR; w]);
            lines.extend(center_pad(&d, w));
            let cancel = centered_cancel(&n, w, 0)
                .chain(centered_cancel(&d, w, baseline + 1))
                .collect();
            Block { lines, baseline, cancel }
        }

        Node::Sqrt { arg, index } => {
            let a = render_row(arg, cur(Field::SqrtArg), true, ctx);
            let h = a.height();
            let w = a.width();
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
            let cancel = a.cancel.iter().map(|&(r, c)| (r + 1, c + 1)).collect();
            Block { lines, baseline: a.baseline + 1, cancel }
        }

        Node::Sup { arg } => {
            if cursor.is_none() {
                if let Some(chars) = inline_script(arg, superscript_char) {
                    return Block::from_chars(chars);
                }
            }
            let a = render_row(arg, cur(Field::SupArg), true, ctx);
            let h = a.height();
            Block { lines: a.lines, baseline: h, cancel: a.cancel }
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
            let cancel = a.cancel.iter().map(|&(r, c)| (r + 1, c)).collect();
            Block { lines, baseline: 0, cancel }
        }

        Node::BigOp { base, lower, upper } => {
            // Empty limits vanish in canonical output (normalize splices
            // the base), but while the cursor is inside both slots must
            // stay visible (⬚) so they can be navigated to.
            let editing = cursor.is_some();
            let u = render_row(upper, cur(Field::OpUpper), editing, ctx);
            let l = render_row(lower, cur(Field::OpLower), editing, ctx);
            let b = render_row(base, None, true, ctx);
            if u.is_empty() && l.is_empty() && cursor.is_none() {
                // Transient un-normalized state: just the base.
                return b;
            }
            // Band marks the horizontal extent of the limits; the base is
            // centered on the band row with its blank cells drawn as ┄
            // ("anything sandwiched in ┄ without spaces takes limits").
            let bw = b.width().max(1);
            let w = u.width().max(l.width()).max(bw) + 2;
            let mut band = vec![OP_BAND; w];
            let left = (w - bw) / 2;
            if !b.is_empty() {
                for (i, &c0) in b.lines[b.baseline.min(b.height() - 1)].iter().enumerate() {
                    if c0 != ' ' {
                        band[left + i] = c0;
                    }
                }
            }
            let mut lines = center_pad(&u, w);
            let baseline = lines.len();
            lines.push(band);
            lines.extend(center_pad(&l, w));
            let cancel = centered_cancel(&u, w, 0)
                .chain(centered_cancel(&l, w, baseline + 1))
                .collect();
            Block { lines, baseline, cancel }
        }

        Node::Brace { over, arg, label } => {
            // ╭──╮ hugging the argument block (╰──╯ underneath for
            // \underbrace), label centered beyond the brace. Width =
            // max(arg, label) + 2, like a fraction bar.
            let a = render_row(arg, cur(Field::BraceArg), true, ctx);
            let l = render_row(label, cur(Field::BraceLabel), l_placeholder(cursor), ctx);
            let w = a.width().max(l.width()).max(1) + 2;
            let mut brace = vec![FRAC_BAR; w];
            brace[0] = if *over { '╭' } else { '╰' };
            brace[w - 1] = if *over { '╮' } else { '╯' };
            let mut lines: Vec<Vec<char>> = Vec::new();
            let mut cancel: Vec<(usize, usize)> = Vec::new();
            if *over {
                lines.extend(center_pad(&l, w));
                lines.push(brace);
                let a_off = lines.len();
                let baseline = a_off + a.baseline;
                cancel.extend(centered_cancel(&l, w, 0));
                cancel.extend(centered_cancel(&a, w, a_off));
                lines.extend(center_pad(&a, w));
                Block { lines, baseline, cancel }
            } else {
                let baseline = a.baseline;
                cancel.extend(centered_cancel(&a, w, 0));
                lines.extend(center_pad(&a, w));
                let brace_off = lines.len();
                lines.push(brace);
                cancel.extend(centered_cancel(&l, w, brace_off + 1));
                lines.extend(center_pad(&l, w));
                Block { lines, baseline, cancel }
            }
        }

        Node::Arrow { op, over, under } => {
            // Same shape as the big-op band: labels centered over the
            // extent; empty labels vanish except while editing.
            let editing = cursor.is_some();
            let o = render_row(over, cur(Field::ArrowOver), editing, ctx);
            let u = render_row(under, cur(Field::ArrowUnder), editing, ctx);
            let w = o.width().max(u.width()).max(1) + 3;
            // Heads are ASCII < > (box-drawing bodies and Unicode arrow
            // glyphs rarely align in height across fonts).
            let mut body = vec![arrow_body(*op); w];
            if *op == '←' || *op == '⇐' {
                body[0] = '<';
            } else {
                body[w - 1] = '>';
            }
            let mut lines = center_pad(&o, w);
            let baseline = lines.len();
            lines.push(body);
            lines.extend(center_pad(&u, w));
            let cancel = centered_cancel(&o, w, 0)
                .chain(centered_cancel(&u, w, baseline + 1))
                .collect();
            Block { lines, baseline, cancel }
        }

        Node::Delim { left, right, mids, segs } => {
            // A sole Array segment fuses with the delimiter: the delimiter
            // columns absorb the lattice edges (junction rows show ┠ ┨,
            // column markers ┬ ┴ ride the delimiter's top/bottom rows).
            // Angles keep their vertex geometry and wrap a bare lattice
            // instead.
            if mids.is_empty() && *left != '⟨' && *right != '⟩' {
                if let [seg] = &segs[..] {
                    if let [Node::Array { rows, cols, cells }] = &seg[..] {
                        let seg_cursor = cur(Field::Seg(0));
                        let acur = match seg_cursor {
                            Some((path, c)) => match path.first() {
                                Some(&(0, f)) => Some((f, (&path[1..], c))),
                                _ => None,
                            },
                            None => None,
                        };
                        if !matches!(seg_cursor, Some(([], _))) {
                            return render_fused_grid(
                                *left, *right, *rows, *cols, cells, acur, ctx,
                            );
                        }
                    }
                }
            }
            // Segments render as ordinary rows (an Array node inside is a
            // grid body); 1-column gaps between them become full-height │
            // middles after concatenation.
            let mut parts: Vec<Block> = Vec::with_capacity(segs.len() * 2);
            for (k, seg) in segs.iter().enumerate() {
                if k > 0 {
                    parts.push(Block::from_chars(vec![' ']));
                }
                parts.push(render_row(seg, cur(Field::Seg(k)), true, ctx));
            }
            let mut body = hcat(&parts);
            let mut x = 0;
            for (k, b) in parts.iter().enumerate() {
                if k % 2 == 1 {
                    for line in body.lines.iter_mut() {
                        line[x] = '│';
                    }
                }
                x += b.width();
            }
            let h = body.height().max(1);
            let bl = body.baseline.min(h - 1);
            let lcol = delim_column(*left, true, h, bl);
            let rcol = delim_column(*right, false, h, bl);
            let mut lines = Vec::with_capacity(h);
            for (r, line) in body.lines.iter().enumerate() {
                let mut row = Vec::with_capacity(body.width() + 2);
                row.push(lcol[r]);
                row.extend_from_slice(line);
                row.push(rcol[r]);
                lines.push(row);
            }
            let cancel = body.cancel.iter().map(|&(r, c)| (r, c + 1)).collect();
            Block { lines, baseline: body.baseline, cancel }
        }

        Node::Cancel { arg } => {
            let a = render_row(arg, cur(Field::CancelArg), true, ctx);
            // Strike every non-blank cell with the combining overlay.
            let mut cancel: Vec<(usize, usize)> = Vec::new();
            for (r, line) in a.lines.iter().enumerate() {
                for (c, &ch) in line.iter().enumerate() {
                    if ch != ' ' {
                        cancel.push((r, c));
                    }
                }
            }
            Block { lines: a.lines, baseline: a.baseline, cancel }
        }

        Node::Array { rows, cols, cells } => {
            render_lattice(*rows, *cols, cells, cursor, ctx)
        }
    }
}

/// Self-delimiting grid: ┼ markers at every crossing of the separator
/// rows/columns *including the outer edges*, so the extent and the cell
/// boundaries are explicit without any delimiter (LaTeX \begin{matrix}).
/// Baseline = vertical center of the whole lattice.
/// Delimiter fused with a sole grid segment, in the minimal shape:
/// - rows>=2 and cols>=2: separator rows carry only ┼ at the crossings
/// - one row: ┬ / ┴ marker rows above and below the cells
/// - one column: ┠ ┨ junctions in the delimiter columns
///
/// The older ┬┴+┠┨ shape stays legal input; fmt tightens it.
#[allow(clippy::too_many_arguments)]
fn render_fused_grid(
    left: char,
    right: char,
    rows: usize,
    cols: usize,
    cells: &[Row],
    cursor: Option<(Field, CursorRef)>,
    ctx: &RenderCtx,
) -> Block {
    let blocks: Vec<Block> = cells
        .iter()
        .enumerate()
        .map(|(i, cell)| {
            let cur = match cursor {
                Some((Field::Cell(ci), c)) if ci == i => Some(c),
                _ => None,
            };
            render_row(cell, cur, true, ctx)
        })
        .collect();
    let col_w: Vec<usize> = (0..cols)
        .map(|j| (0..rows).map(|i| blocks[i * cols + j].width()).max().unwrap_or(1))
        .collect();
    // Interior: pad cell pad, with a marker column between cells.
    let mut marker_x: Vec<usize> = Vec::new();
    let mut x = 0;
    for (j, w) in col_w.iter().enumerate() {
        x += w + 2;
        if j + 1 < cols {
            marker_x.push(x);
            x += 1;
        }
    }
    let width = x;
    let edge_row = |mark: char| {
        let mut r = vec![' '; width];
        for &mx in &marker_x {
            r[mx] = mark;
        }
        r
    };

    let one_row = rows == 1;
    let mut lines: Vec<Vec<char>> = Vec::new();
    let mut cancel: Vec<(usize, usize)> = Vec::new();
    let mut sep_rows: Vec<usize> = Vec::new();
    if one_row {
        lines.push(edge_row('┬'));
    }
    for i in 0..rows {
        if i > 0 {
            sep_rows.push(lines.len());
            lines.push(edge_row('┼'));
        }
        let mut parts: Vec<Block> = Vec::new();
        for j in 0..cols {
            if j > 0 {
                parts.push(Block::new(vec![vec![' ']], 0));
            }
            parts.push(Block::new(vec![vec![' ']], 0));
            let b = &blocks[i * cols + j];
            parts.push(Block {
                lines: center_pad(b, col_w[j]),
                baseline: b.baseline,
                cancel: centered_cancel(b, col_w[j], 0).collect(),
            });
            parts.push(Block::new(vec![vec![' ']], 0));
        }
        let row_block = hcat(&parts);
        let row_off = lines.len();
        cancel.extend(row_block.cancel.iter().map(|&(r, c)| (r + row_off, c)));
        for line in row_block.lines {
            let mut l = line;
            l.resize(width, ' ');
            lines.push(l);
        }
    }
    if one_row {
        lines.push(edge_row('┴'));
    }
    let h = lines.len();
    let bl = (h - 1) / 2;
    // Single column: the delimiter columns carry the row junctions.
    let junction = cols == 1;
    let lcol = delim_column(left, true, h, bl);
    let rcol = delim_column(right, false, h, bl);
    let mut out = Vec::with_capacity(h);
    for (r, line) in lines.into_iter().enumerate() {
        let (lc, rc) = if junction && sep_rows.contains(&r) {
            ('┠', '┨')
        } else {
            (lcol[r], rcol[r])
        };
        let mut row = Vec::with_capacity(width + 2);
        row.push(lc);
        row.extend(line);
        row.push(rc);
        out.push(row);
    }
    let cancel = cancel.into_iter().map(|(r, c)| (r, c + 1)).collect();
    Block { lines: out, baseline: bl, cancel }
}

fn render_lattice(
    rows: usize,
    cols: usize,
    cells: &[Row],
    cursor: Option<(Field, CursorRef)>,
    ctx: &RenderCtx,
) -> Block {
    let cctx = *ctx;
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
    // Marker columns at x = 0 and after every cell span (cell + 1 pad on
    // each side).
    let mut marker_x = vec![0usize];
    for w in &col_w {
        marker_x.push(marker_x.last().unwrap() + w + 3);
    }
    let width = *marker_x.last().unwrap() + 1;
    let kind = |i: usize, n: usize| if i == 0 { 0 } else if i == n { 2 } else { 1 };
    let marker_row = |ri: usize| {
        let mut r = vec![' '; width];
        for (ci, &x) in marker_x.iter().enumerate() {
            r[x] = lattice_char(kind(ri, rows), kind(ci, cols));
        }
        r
    };

    let mut lines: Vec<Vec<char>> = vec![marker_row(0)];
    let mut cancel: Vec<(usize, usize)> = Vec::new();
    for i in 0..rows {
        let mut parts: Vec<Block> = Vec::new();
        for j in 0..cols {
            // marker column + 1 pad, then the centered cell, then 1 pad.
            parts.push(Block::new(vec![vec![' '; 2]], 0));
            let b = &blocks[i * cols + j];
            parts.push(Block {
                lines: center_pad(b, col_w[j]),
                baseline: b.baseline,
                cancel: centered_cancel(b, col_w[j], 0).collect(),
            });
            parts.push(Block::new(vec![vec![' ']], 0));
        }
        let row_block = hcat(&parts);
        let row_off = lines.len();
        cancel.extend(row_block.cancel.iter().map(|&(r, c)| (r + row_off, c)));
        for line in row_block.lines {
            let mut l = line;
            l.resize(width, ' ');
            lines.push(l);
        }
        lines.push(marker_row(i + 1));
    }
    let h = lines.len();
    Block { lines, baseline: (h - 1) / 2, cancel }
}

/// One rendered column of a delimiter. `spec` is the delimiter spec char,
/// `bl` the baseline row of the body being wrapped. Vertex glyphs (⎨ ⎬ ⟨ ⟩)
/// always sit on the baseline row — that is what lets the parser read the
/// baseline straight off a brace/angle column.
fn delim_column(spec: char, left: bool, h: usize, bl: usize) -> Vec<char> {
    if h == 1 {
        return vec![match spec {
            '|' => {
                if left {
                    '⎸'
                } else {
                    '⎹'
                }
            }
            '.' => {
                if left {
                    '▏'
                } else {
                    '▕'
                }
            }
            c => c,
        }];
    }
    (0..h)
        .map(|r| match spec {
            '(' | ')' | '[' | ']' => {
                let (top, ext, bot) = match spec {
                    '(' => ('⎛', '⎜', '⎝'),
                    ')' => ('⎞', '⎟', '⎠'),
                    '[' => ('⎡', '⎢', '⎣'),
                    _ => ('⎤', '⎥', '⎦'),
                };
                if r == 0 {
                    top
                } else if r == h - 1 {
                    bot
                } else {
                    ext
                }
            }
            '{' | '}' => {
                let (top, mid, bot) = if spec == '{' { ('⎧', '⎨', '⎩') } else { ('⎫', '⎬', '⎭') };
                if r == bl {
                    mid
                } else if r == 0 {
                    top
                } else if r == h - 1 {
                    bot
                } else {
                    '⎪'
                }
            }
            '⟨' => match r.cmp(&bl) {
                std::cmp::Ordering::Equal => '⟨',
                std::cmp::Ordering::Less => '╱',
                std::cmp::Ordering::Greater => '╲',
            },
            '⟩' => match r.cmp(&bl) {
                std::cmp::Ordering::Equal => '⟩',
                std::cmp::Ordering::Less => '╲',
                std::cmp::Ordering::Greater => '╱',
            },
            '|' => {
                if left {
                    '⎸'
                } else {
                    '⎹'
                }
            }
            _ => {
                if left {
                    '▏'
                } else {
                    '▕'
                }
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sym_row(s: &str) -> Row {
        s.chars().map(Node::Sym).collect()
    }

    fn plain(root: &Row) -> Vec<String> {
        let ctx = RenderCtx { italic: false };
        render_row(root, None, false, &ctx)
            .to_strings()
            .iter()
            .map(|l| l.trim_end().to_string())
            .collect()
    }

    #[test]
    fn fraction_renders_with_bar() {
        let root = vec![Node::Frac { num: sym_row("1"), den: sym_row("x+1") }];
        assert_eq!(plain(&root), vec!["  1", "─────", " x+1"]);
    }

    #[test]
    fn inline_superscript() {
        let root = vec![Node::Sym('x'), Node::Sup { arg: sym_row("2") }];
        assert_eq!(plain(&root), vec!["x²"]);
    }

    #[test]
    fn bigop_band_marks_limit_extent() {
        let root = vec![Node::BigOp {
            base: vec![Node::Sym('∑')],
            lower: sym_row("i=0"),
            upper: sym_row("n"),
        }];
        assert_eq!(plain(&root), vec!["  n", "┄┄∑┄┄", " i=0"]);
    }

    #[test]
    fn bigop_without_limits_is_bare() {
        let root = vec![Node::BigOp { base: vec![Node::Sym('∫')], lower: vec![], upper: vec![] }];
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
        let root = vec![Node::Delim {
            left: '[',
            right: ']',
            mids: vec![],
            segs: vec![vec![Node::Array {
                rows: 2,
                cols: 2,
                cells: vec![sym_row("a"), sym_row("b"), sym_row("c"), sym_row("d")],
            }]],
        }];
        assert_eq!(plain(&root), vec!["⎡ a   b ⎤", "⎢   ┼   ⎥", "⎣ c   d ⎦"]);
    }

    #[test]
    fn delim_families_render() {
        // {x} with a tall fraction: brace vertex ⎨ on the baseline row.
        let root = vec![Node::Delim {
            left: '{',
            right: '}',
            mids: vec![],
            segs: vec![vec![Node::Frac { num: sym_row("1"), den: sym_row("2") }]],
        }];
        assert_eq!(plain(&root), vec!["⎧ 1 ⎫", "⎨───⎬", "⎩ 2 ⎭"]);
        // ⟨x|y⟩ single-line braket.
        let root = vec![Node::Delim {
            left: '⟨',
            right: '⟩',
            mids: vec!['|'],
            segs: vec![sym_row("x"), sym_row("y")],
        }];
        assert_eq!(plain(&root), vec!["⟨x│y⟩"]);
        // Bare 2×2 array: self-delimiting ┼ lattice.
        let root = vec![Node::Array {
            rows: 2,
            cols: 2,
            cells: vec![sym_row("a"), sym_row("b"), sym_row("c"), sym_row("d")],
        }];
        assert_eq!(
            plain(&root),
            vec!["┌   ┬   ┐", "  a   b", "├   ┼   ┤", "  c   d", "└   ┴   ┘"]
        );
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
    fn bigop_shows_placeholders_while_editing() {
        // Cursor in the (empty) lower limit: both slots must be visible.
        let root = vec![Node::BigOp { base: vec![Node::Sym('∑')], lower: vec![], upper: vec![] }];
        let path = [(0, Field::OpLower)];
        let b = render_row(&root, Some((&path, 0)), false, &RenderCtx::canonical());
        let text = b.to_text();
        assert!(text.contains(CURSOR_CHAR), "cursor visible:\n{}", text);
        assert!(text.contains(PLACEHOLDER), "empty upper slot visible:\n{}", text);
        // Cursor elsewhere: canonical bare operator, no placeholders.
        let plain = render_row(&root, None, false, &RenderCtx::canonical()).to_text();
        assert_eq!(plain, "∑");
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
        assert!(lines[1].starts_with("a+"));
    }
}
