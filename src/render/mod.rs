//! 2D layout engine: AST -> rectangular block of chars with a baseline.
//!
//! The cursor-free output is the *canonical AA form* (see docs/aa-spec.md):
//! it is designed so that `parse.rs` can deterministically invert it back to
//! the AST. Every layout rule here has a matching rule in the parser —
//! change them in lockstep and keep the roundtrip tests green.

use crate::ast::{Field, Node, Row};

mod block;
use crate::glyphs::{
    ARM_FALL, ARM_RISE, COL_MARK_BOT, COL_MARK_TOP, CROSSING, DOUBLE_BODY, FRAC_BAR, HEAD_LEFT,
    HEAD_RIGHT, MID, Mark, NORM, OP_BAND, OVERLINE_CORNER, PLACEHOLDER, ROW_JUNCTION_L,
    ROW_JUNCTION_R, STEM, brace_corners, is_display_marker, lattice_char,
};
use crate::symbols::{Accent, Delim, DrawnForm, subscript_char, superscript_char};
pub use block::Block;
use block::{Annots, center_pad, hcat, vstack};

// The structural glyph vocabulary lives in crate::glyphs; re-exported
// here for the callers that grew up importing it from the render side.
pub use crate::glyphs::CURSOR_CHAR;

/// A display-marker atom (zero-width; transparent to layout decisions).
fn is_marker_node(n: &Node) -> bool {
    matches!(n, Node::Sym(c) if is_display_marker(*c))
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

/// Inverse of the render-time character styling (used by the parser).
pub fn unstyle_char(c: char) -> char {
    // Each arm maps one math-italic block back onto its plain range by
    // offset (Latin both cases, then the lenient-input Greek).
    let shift = |plain: char, styled: char| {
        char::from_u32(plain as u32 + (c as u32 - styled as u32)).unwrap()
    };
    match c {
        'ℎ' => 'h',
        '∗' => '*',
        '𝑎'..='𝑧' => shift('a', '𝑎'),
        '𝐴'..='𝑍' => shift('A', '𝐴'),
        '𝛼'..='𝜔' => shift('α', '𝛼'),
        '𝛢'..='𝛺' => shift('Α', '𝛢'),
        c => c,
    }
}

fn display_char(c: char, ctx: &RenderCtx) -> char {
    match c {
        '*' => '∗',
        c if ctx.italic => italic_char(c),
        c => c,
    }
}

/// Would this row render as an inline (codepoint) script, absent any
/// cursor or markers? Used by the editor to tell which positions are
/// currently invisible on screen.
pub fn is_inline_script_row(row: &Row, superscript: bool) -> bool {
    let map = if superscript {
        superscript_char
    } else {
        subscript_char
    };
    inline_script(row, map).is_some()
}

/// Inline (superscript/subscript codepoint) form of a script row. A
/// display marker inside the row forces the 2D form — labels overlaid
/// on the tiny ¹²³ glyphs are unreadable, so a marked script expands
/// while the marks are alive (jump mode and its ghosts).
fn inline_script(row: &Row, map: fn(char) -> Option<char>) -> Option<Vec<char>> {
    if row.is_empty() {
        return None;
    }
    row.iter()
        .map(|n| match n {
            Node::Sym(c) if is_display_marker(*c) => None,
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
}

/// Cursor position relative to the row being rendered: the remaining path
/// into descendants, and the column once the path is exhausted.
type CursorRef<'a> = (&'a [(usize, Field)], usize);

/// Upright run (`Func`). It draws bare when the picture reads back as
/// the same run: >= 2 ASCII letters, dots only where the run lexer
/// keeps them (i.i.d.). Anything else is 'single-quoted'.
fn func_block(t: &str) -> Block {
    // A dotted run (i.i.d.) is bare exactly when the run lexer reads it
    // back as one token: starts with a letter, every interior dot is
    // followed by a letter, and a trailing dot only with an interior
    // dot before it.
    let chars: Vec<char> = t.chars().collect();
    let dotted_ok = chars.first().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.iter().all(|&c| c.is_ascii_alphabetic() || c == '.')
        && chars.iter().enumerate().all(|(i, &c)| {
            c != '.'
                || match chars.get(i + 1) {
                    Some(n) => n.is_ascii_alphabetic(),
                    None => chars[..i].contains(&'.'),
                }
        });
    if dotted_ok && chars.len() >= 2 {
        Block::from_chars(chars)
    } else {
        quoted(t, '\'')
    }
}

/// A lone upright letter (`Roman`). It can drop its quotes only when a
/// neighbour glues it into the roman reading (`d𝑦`); standalone it
/// would read as an italic variable, so it keeps them.
fn roman_block(c: char, glue: bool) -> Block {
    if glue {
        Block::from_chars(vec![c])
    } else {
        quoted(&c.to_string(), '\'')
    }
}

/// `'…'` (\mathrm) or `"…"` (\text). The double-quoted form delimits
/// its own content so real spaces survive; the single-quoted scan stops
/// at a blank, so interior spaces show as ␣.
fn quoted(t: &str, q: char) -> Block {
    let mut chars = vec![q];
    for c in t.chars() {
        match c {
            ' ' if q == '\'' => chars.push('␣'),
            // Inside "…", a backslash escapes the next char, so a
            // literal " (or \) is representable.
            '"' | '\\' if q == '"' => {
                chars.push('\\');
                chars.push(c);
            }
            c => chars.push(c),
        }
    }
    chars.push(q);
    Block::from_chars(chars)
}

/// Does this node's picture put an accent band next to its neighbours?
/// A band has no closing glyph, so the scan would run into whatever
/// touches it — struck (`Cancel`) accents count too, which is why this
/// looks through the wrappers that keep their argument's outline.
fn has_wide_accent(n: &Node) -> bool {
    match n {
        Node::WideAccent { .. } => true,
        Node::Cancel { arg } => arg.iter().any(has_wide_accent),
        _ => false,
    }
}

/// Would this node's rendered baseline edge glue a lone upright letter
/// into the roman reading (an alphabetic cell with no separator space)?
/// Conservative whitelist — quoting is always safe, bare is not.
/// `right_edge` selects which baseline edge faces the letter: a ddot
/// accent's ․․ overhang leaves a blank baseline cell on its right, so
/// its right edge never glues.
fn glue_alpha(n: &Node, right_edge: bool) -> bool {
    match n {
        Node::Sym(c) => c.is_alphabetic(),
        Node::Accent { overs, base, .. } => {
            // A mark wider than its base spills a column past the
            // right edge, so that edge cannot glue.
            base.is_alphabetic() && !(right_edge && overs.iter().any(|m| m.cells().len() > 1))
        }
        _ => false,
    }
}

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

    // Display-marker atoms are zero-width annotations; a row holding
    // only markers lays out like the empty row it decorates — except
    // that a marked slot materializes as ⬚ so its label/ghost has a
    // cell to sit on (that is how ^G reaches the invisible limits of a
    // bare ∑).
    if row.iter().all(is_marker_node) {
        let mut b = match (cursor_col, placeholder, row.is_empty()) {
            // Caret on the placeholder cell; the geometry matches the
            // cursor-less render.
            (Some(_), _, _) => Block::from_chars(vec![PLACEHOLDER]).with_caret(0, 0),
            (None, true, _) | (None, false, false) => Block::from_chars(vec![PLACEHOLDER]),
            (None, false, true) => Block::empty(),
        };
        for n in row {
            if let Node::Sym(c) = n {
                b.marks.push((0, 0, *c));
            }
        }
        return b;
    }

    // Per-block info driving the spacer rules below.
    #[derive(Clone, Copy, Default)]
    struct Info {
        script: bool,
        script_2d: bool,
        cancel: bool,
        /// A bare dotted roman run (i.i.d.): its dots would absorb an
        /// adjacent letter run or period into one token, so the fuse
        /// rules keep a space after it.
        dot_run: bool,
        /// Wide accents carry off-baseline ┈ band rows with no closing
        /// glyph; a tall neighbour touching that row would be munched
        /// into the band scan, so they keep a space on both sides.
        wide_accent: bool,
        /// Zero-width display annotation: invisible to the fuse rules.
        marker: bool,
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
            wide_accent: has_wide_accent(node),
            dot_run: matches!(node, Node::Func(t) if t.contains('.')),
            marker: is_marker_node(node),
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
        let mut block = match node {
            // Single upright letters drop their quotes when a neighbour
            // makes the bare reading unambiguous (d𝑦); the quotes come
            // back automatically when the context changes.
            Node::Roman(c) => {
                let glue = row[..i]
                    .iter()
                    .rev()
                    .find(|n| !is_marker_node(n))
                    .is_some_and(|n| glue_alpha(n, true))
                    || row[i + 1..]
                        .iter()
                        .find(|n| !is_marker_node(n))
                        .is_some_and(|n| glue_alpha(n, false));
                roman_block(*c, glue)
            }
            _ => render_node(node, child_cursor, ctx),
        };
        // A script at the start of a row gets an explicit ⬚ base, so the
        // picture differs from the row without the script wrapper
        // (markers are invisible to "start of a row").
        let first_real = row.iter().position(|n| !is_marker_node(n)).unwrap_or(0);
        if i == first_real && info.script {
            block = hcat(&[Block::from_chars(vec![PLACEHOLDER]), block]);
        }
        blocks.push((block, info));
    }
    // A FRAME pair wraps a node whose frame the display recolors (grid
    // mode). The render owns the geometry, so the pair converts to the
    // node's exact corners here — the display reads the rectangle
    // straight off the two marks, whatever later compositions do.
    let frame_open = Mark::Frame { open: true }.ch();
    let frame_close = Mark::Frame { open: false }.ch();
    for m in 0..row.len() {
        if !matches!(&row[m], Node::Sym(c) if *c == frame_open) {
            continue;
        }
        let Some(t) = (m + 1..row.len()).find(|&t| !is_marker_node(&row[t])) else {
            continue;
        };
        let close =
            (t + 1..row.len()).find(|&c| matches!(&row[c], Node::Sym(ch) if *ch == frame_close));
        let Some(close) = close else { continue };
        let (h, w) = (blocks[t].0.height(), blocks[t].0.width());
        blocks[m].0.marks.retain(|&(_, _, c)| c != frame_open);
        blocks[close].0.marks.retain(|&(_, _, c)| c != frame_close);
        blocks[t].0.marks.push((0, 0, frame_open));
        blocks[t]
            .0
            .marks
            .push((h.saturating_sub(1), w.saturating_sub(1), frame_close));
    }
    if let Some(col) = cursor_col {
        blocks.insert(
            col,
            (
                Block::caret_marker(),
                Info {
                    marker: true,
                    ..Info::default()
                },
            ),
        );
    }

    // Single-space separators between siblings, inserted only where
    // adjacent glyphs would otherwise fuse into one token — space
    // *presence* (never count) is what changes a reading:
    //  - between two identical bar glyphs (── / ┈┈ / ══ would merge)
    //  - between a bar edge and a > head that would cap it (─ then >,
    //    ═ then >) and between a < head and a body that would absorb it
    //    (< then ─ or ═)
    //  - after a cancel with a blank baseline edge (the strike-extent scan
    //    must not fuse the ragged edge with the neighbour)
    // A 2D script right after a cancel instead needs a non-blank baseline
    // anchor (⬚), or the strike scan would fuse raised content.
    // Rows (relative to the baseline) where a block's edge column holds
    // '─': the sqrt overline scans its ─ run greedily off the baseline
    // (a sup's fraction bar can align with it), so two ─ cells touching
    // across a sibling boundary on any row get a separating space (the
    // baseline case is also covered by the fuse rules above).
    let bar_edge_rows = |b: &Block, right: bool| -> Vec<isize> {
        let w = b.width();
        b.lines
            .iter()
            .enumerate()
            .filter_map(|(i, l)| {
                let c = if right {
                    (l.len() == w).then(|| l.last().copied()).flatten()
                } else {
                    l.first().copied()
                };
                (c == Some(FRAC_BAR)).then_some(i as isize - b.baseline as isize)
            })
            .collect()
    };
    let mut spaced: Vec<Block> = Vec::with_capacity(blocks.len() * 2);
    let mut prev: Option<Info> = None;
    let mut last_edge: Option<char> = None;
    let mut last_bars: Vec<isize> = Vec::new();
    for (block, info) in blocks {
        // Zero-width annotations pass through without touching the
        // neighbour bookkeeping (the fuse rules must behave exactly as
        // in the undecorated render).
        if info.marker {
            spaced.push(block);
            continue;
        }
        let need = match prev {
            Some(p) => {
                let edges = (last_edge, block.baseline_edge(true));
                let fuse = match edges {
                    (Some(a), Some(b)) => {
                        // A band edge fuses with *anything* adjacent (the
                        // general ┈piece┈ grammar munches non-space runs).
                        a == OP_BAND
                            || b == OP_BAND
                            // Adjacent upright letter runs (Func / bare
                            // Text) would fuse into one token.
                            || (a.is_ascii_alphabetic() && b.is_ascii_alphabetic())
                            // A period directly before a letter would be
                            // absorbed into the run (exp.i.i.d.); digits
                            // keep decimals tight (3.14).
                            || (a == '.' && b.is_ascii_alphabetic())
                            || (a == b && (a == FRAC_BAR || a == DOUBLE_BODY))
                            || (a == FRAC_BAR && b == HEAD_RIGHT)
                            || (a == DOUBLE_BODY && b == HEAD_RIGHT)
                            || (a == HEAD_LEFT && (b == FRAC_BAR || b == DOUBLE_BODY))
                    }
                    _ => false,
                };
                let ragged_cancel = p.cancel && last_edge == Some(' ');
                // A dotted run would lexically absorb a following letter
                // or period (i.i.d. + ab → i.i.d.ab).
                let dotted = p.dot_run
                    && block
                        .baseline_edge(true)
                        .is_some_and(|b| b.is_ascii_alphabetic() || b == '.');
                let bar_touch = !last_bars.is_empty()
                    && bar_edge_rows(&block, false)
                        .iter()
                        .any(|r| last_bars.contains(r));
                fuse || ragged_cancel || dotted || p.wide_accent || info.wide_accent || bar_touch
            }
            _ => false,
        };
        let anchor = matches!(prev, Some(p) if p.cancel) && info.script_2d;
        if anchor {
            spaced.push(Block::from_chars(vec![PLACEHOLDER]));
        } else if need {
            spaced.push(Block::from_chars(vec![' ']));
        }
        let edge = block.baseline_edge(false);
        last_bars = bar_edge_rows(&block, true);
        spaced.push(block);
        prev = Some(info);
        last_edge = edge;
    }
    hcat(&spaced)
}

/// The canonical AA of `row`, with the ⬚ empty-slot cells blanked —
/// the export form (clipboard, document write-back). The slot marks
/// are editor chrome, not content: a formula with holes exports the
/// holes as holes. Blanking keeps every cell in place, so the geometry
/// is untouched, and the blanked text is used only when it still
/// parses back to the same tree; when a ⬚ is a picture's only ink on
/// its baseline (a bare tall script, say), the canonical text is
/// returned unchanged rather than exporting something unreadable.
pub fn export_aa(row: &Row) -> String {
    let row = crate::ast::normalize(row);
    let canonical = render_root(&row, None, &RenderCtx::canonical()).to_text();
    if !canonical.contains(PLACEHOLDER) {
        return canonical;
    }
    let blanked: String = canonical
        .lines()
        .map(|l| l.replace(PLACEHOLDER, " "))
        .map(|l| l.trim_end().to_string())
        .collect::<Vec<_>>()
        .join("\n");
    match (
        crate::parse::parse(&blanked),
        crate::parse::parse(&canonical),
    ) {
        (Ok(b), Ok(c)) if b == c => blanked,
        _ => canonical,
    }
}

/// Top-level entry: a root row may contain `Node::Break`s splitting the
/// formula into display lines. Lines are stacked left-aligned with one
/// blank row between them; every continuation line carries a lone `┈`
/// marker at its baseline (col 0) — it has no other reading, and it
/// hands the parser the line's baseline for free. Rows without Breaks
/// render exactly as render_row.
pub fn render_root(row: &Row, cursor: Option<CursorRef>, ctx: &RenderCtx) -> Block {
    if !row.iter().any(|n| matches!(n, Node::Break)) {
        return render_row(row, cursor, false, ctx);
    }
    // Split at the Breaks; the cursor belongs to exactly one segment.
    let mut segments: Vec<Block> = Vec::new();
    let mut start = 0usize;
    let cursor_col = match cursor {
        Some(([], col)) => Some(col),
        _ => None,
    };
    let bounds: Vec<usize> = row
        .iter()
        .enumerate()
        .filter_map(|(i, n)| matches!(n, Node::Break).then_some(i))
        .chain([row.len()])
        .collect();
    for &end in bounds.iter() {
        let seg = &row[start..end];
        // Rebase the cursor (path first step / top-level column) onto
        // the segment that contains it.
        let rebased: Option<(Vec<(usize, Field)>, usize)> = match cursor {
            Some((path, col)) => match path.first() {
                Some(&(i, f)) if (start..end).contains(&i) => {
                    let mut p = path.to_vec();
                    p[0] = (i - start, f);
                    Some((p, col))
                }
                None if cursor_col.is_some_and(|c| (start..=end).contains(&c)) => {
                    // Column boundary: attach to this segment unless it
                    // belongs to the next one (col == end == a Break —
                    // put the caret before the break, i.e. line end).
                    Some((Vec::new(), cursor_col.unwrap() - start))
                }
                _ => None,
            },
            None => None,
        };
        let seg_vec: Row = seg.to_vec();
        let cur_ref = rebased.as_ref().map(|(p, c)| (p.as_slice(), *c));
        segments.push(render_row(&seg_vec, cur_ref, false, ctx));
        start = end + 1;
    }
    vstack(&segments)
}

fn l_placeholder(cursor: Option<(Field, CursorRef)>) -> bool {
    cursor.is_some()
}

/// What the delimiter layer needs to know about a `Norm` / `Delim`
/// node, read off the types once: the glyph layer below works on spec
/// chars (the slot decides the side), but every *kind* question — does
/// this pair fuse with a sole grid, is a side an angle (diagonal arms,
/// no column), a brace (vertex column), is this the norm — is answered
/// here, not by comparing glyphs.
struct DelimShape<'a> {
    left: char,
    right: char,
    mids: usize,
    segs: &'a [Row],
    fusable: bool,
    is_norm: bool,
    angle_sided: bool,
    curly: bool,
}

/// The height and baseline a delimiter pair stretches to around a body
/// of `h` rows baselined at `bl`.
fn delim_extent(
    h: usize,
    bl: usize,
    angle: bool,
    curly: bool,
    inner_norm_full: bool,
) -> (usize, usize) {
    if angle {
        // Diagonal arms: the extent is symmetric about the turn, so it
        // is always even and the upper turn row is the baseline.
        let k = (bl + 1).max(h - 1 - bl);
        (2 * k, k - 1)
    } else if curly && h == 2 {
        // A curly column needs hook + ⎨ vertex + hook: a 2-row body
        // rides in a 3-row extent with the vertex centered.
        (3, 1)
    } else if inner_norm_full {
        // Norm-in-norm: the outer pair must outsize the inner one, or
        // the parser's extent grouping cannot tell them apart.
        (h + 2, bl + 1)
    } else {
        (h, bl)
    }
}

fn delim_shape(node: &Node) -> DelimShape<'_> {
    match node {
        Node::Norm { arg } => DelimShape {
            left: NORM,
            right: NORM,
            mids: 0,
            segs: std::slice::from_ref(arg),
            fusable: false,
            is_norm: true,
            angle_sided: false,
            curly: false,
        },
        Node::Delim {
            left,
            right,
            mids,
            segs,
        } => DelimShape {
            left: left.spec(true),
            right: right.spec(false),
            mids: *mids,
            segs,
            fusable: left.fuses() && right.fuses(),
            is_norm: false,
            angle_sided: *left == Delim::Angle || *right == Delim::Angle,
            // A vertex-bearing pair keeps its vertex column (needs
            // hook + vertex + hook, so a 2-row body grows to 3).
            curly: [left, right]
                .iter()
                .any(|d| d.col().is_some_and(|c| c.info().vertex.is_some())),
        },
        _ => unreachable!("delim_shape is for Norm / Delim"),
    }
}

fn render_node(node: &Node, cursor: Option<(Field, CursorRef)>, ctx: &RenderCtx) -> Block {
    // Cursor for a specific field of this node.
    let cur = |f: Field| -> Option<CursorRef> {
        match cursor {
            Some((cf, c)) if cf == f => Some(c),
            _ => None,
        }
    };

    match node {
        Node::Spacer => Block::from_chars(vec![' ']),
        // Breaks are handled by render_root; one reaching a nested row
        // renders as nothing.
        Node::Break => Block::empty(),
        // No automatic spacing anywhere (operators included): spacing is
        // the user's, via formatting Spacers or the semantic ␣ atom.
        // Display markers (jump/block labels, selection ends) are
        // zero-width annotations: no cells, only a position.
        Node::Sym(c) if is_display_marker(*c) => {
            let mut b = Block::empty();
            b.marks.push((0, 0, *c));
            b
        }
        Node::Sym(c) => Block::from_chars(vec![display_char(*c, ctx)]),

        // Upright letters are reserved for function names (plain letters
        // render math-italic), which is what makes them parseable.
        Node::Func(name) => func_block(name),

        // Upright run. \mathrm draws bare when self-identifying (>= 2
        // ASCII letters, not a dictionary word), else 'single-quoted';
        // \text always "double-quotes". Interior spaces are ␣ so quotes
        // never contain structurally meaningful blank columns.
        // Reached only for rows not built by render_row (which decides
        // glue from the neighbours); standalone = never glued.
        Node::Text(t) => quoted(t, '"'),
        Node::Roman(c) => roman_block(*c, false),

        // Stretchy accent: the base stays bare on the baseline; an
        // accent band — a ┈ run whose only piece is the mark — rides
        // above (over) and/or below (under) marking the extent.
        Node::WideAccent {
            overs,
            unders,
            base,
        } => {
            let b = render_row(base, None, true, ctx);
            // A mark's material needs its cells between the band
            // edges, so the band widens past a narrower base.
            let material = overs
                .iter()
                .chain(unders.iter())
                .map(|m| m.cells().len())
                .max()
                .unwrap_or(1);
            let bw = b.width().max(material).max(1);
            let w = bw + 2;
            // Stretchable marks fill the base width: arrows grow a ─
            // body, line-like marks repeat; dot-like marks stay a
            // single centered glyph (low drawn forms, like the compact
            // accents).
            let band_row = |m: Accent| {
                let mut r = vec![OP_BAND; w];
                match m.drawn() {
                    // Centered single glyphs (￫ ˰ ˯ ˳ ․).
                    DrawnForm::Center(g) => r[w / 2] = g,
                    // Fills hug the base across the width: the overline
                    // draws low (_ like the √ overline), the underline
                    // high (¯), the tildes with their hugging forms.
                    DrawnForm::Fill(g) => {
                        for cell in r.iter_mut().take(w - 1).skip(1) {
                            *cell = g;
                        }
                    }
                    // Multi-cell material, centered (the ․․ pair).
                    DrawnForm::Dots => {
                        let cells = m.cells();
                        let s = (w - cells.len()) / 2;
                        for (i, &g) in cells.iter().enumerate() {
                            r[s + i] = g;
                        }
                    }
                }
                r
            };
            // Bands stack outward, innermost first in each list — the
            // same order the compact Accent uses, so the two forms read
            // alike (the outermost over band ends up on top).
            let mut lines: Vec<Vec<char>> = overs.iter().rev().map(|&m| band_row(m)).collect();
            let off = lines.len();
            let baseline = off + b.baseline.min(b.height().saturating_sub(1));
            lines.extend(center_pad(&b, w));
            lines.extend(unders.iter().map(|&m| band_row(m)));
            Annots::default()
                .centered(&b, w, off)
                .into_block(lines, baseline)
        }

        // Marks in the cells directly above/below the base, stacking
        // outward (innermost first in each list). Those cells are never
        // used by anything else (scripts go up-right, limits live inside
        // their band), so a column probe parses the stack back.
        Node::Accent {
            overs,
            unders,
            base,
        } => {
            let b = display_char(*base, ctx);
            // Every mark hugs the base like the wide-band forms: over
            // marks draw low in their cell (bar ¯ as _, tilde ˜ as ˷,
            // hat ^ as ˰, check ˇ as ˯, ring ˚ as ˳, dot ˙ as the
            // leader ․ U+2024, distinct from the '.' atom), under marks
            // draw high (bar ‗ as ¯, tilde ˷ as ˜ — the tilde pair
            // swaps between AST mark and drawn glyph). The ddot draws
            // as ․․ overhanging one column right of the base; the spill
            // column keeps a blank baseline so the pair reads back
            // uniquely.
            let w = overs
                .iter()
                .chain(unders.iter())
                .map(|m| m.cells().len())
                .max()
                .unwrap_or(1)
                .max(1);
            let pad = |mut v: Vec<char>| {
                v.resize(w, ' ');
                v
            };
            let mut lines: Vec<Vec<char>> = overs.iter().rev().map(|&m| pad(m.cells())).collect();
            lines.push(pad(vec![b]));
            lines.extend(unders.iter().map(|&m| pad(m.cells())));
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
            Annots::default()
                .centered(&n, w, 0)
                .centered(&d, w, baseline + 1)
                .into_block(lines, baseline)
        }

        Node::Sqrt { arg, index } => {
            let a = render_row(arg, cur(Field::SqrtArg), true, ctx);
            let h = a.height();
            let w = a.width();
            // Box-drawing overline row on top (┌ corner + ─ run),
            // radical stem hugging the left:
            // ┌────
            // √x+1
            let radical = index.glyph();
            let mut lines = Vec::with_capacity(h + 1);
            let mut top = vec![FRAC_BAR; w + 1];
            top[0] = OVERLINE_CORNER;
            lines.push(top);
            for (r, line) in a.lines.iter().enumerate() {
                let head = if r == h - 1 { radical } else { STEM };
                let mut row = Vec::with_capacity(w + 1);
                row.push(head);
                row.extend_from_slice(line);
                lines.push(row);
            }
            let cancel = a.cancel.iter().map(|&(r, c)| (r + 1, c + 1)).collect();
            Block {
                lines,
                baseline: a.baseline + 1,
                cancel,
                caret: a.caret.map(|(r, c)| (r + 1, c + 1)),
                marks: a
                    .marks
                    .iter()
                    .map(|&(r, c, ch)| (r + 1, c + 1, ch))
                    .collect(),
            }
        }

        Node::Sup { arg } => {
            if cursor.is_none()
                && let Some(chars) = inline_script(arg, superscript_char)
            {
                return Block::from_chars(chars);
            }
            let a = render_row(arg, cur(Field::SupArg), true, ctx);
            let h = a.height();
            Block {
                lines: a.lines,
                baseline: h,
                cancel: a.cancel,
                caret: a.caret,
                marks: a.marks,
            }
        }

        Node::Sub { arg } => {
            if cursor.is_none()
                && let Some(chars) = inline_script(arg, subscript_char)
            {
                return Block::from_chars(chars);
            }
            let a = render_row(arg, cur(Field::SubArg), true, ctx);
            let mut lines = vec![vec![' '; a.width()]];
            lines.extend(a.lines);
            let cancel = a.cancel.iter().map(|&(r, c)| (r + 1, c)).collect();
            Block {
                lines,
                baseline: 0,
                cancel,
                caret: a.caret.map(|(r, c)| (r + 1, c)),
                marks: a.marks.iter().map(|&(r, c, ch)| (r + 1, c, ch)).collect(),
            }
        }

        Node::BigOp { .. } | Node::BigOpSym { .. } => {
            let (lower, upper) = match node {
                Node::BigOp { lower, upper, .. } | Node::BigOpSym { lower, upper, .. } => {
                    (lower, upper)
                }
                _ => unreachable!(),
            };
            // Empty limits vanish in canonical output (normalize bares
            // the band), but while the cursor is inside both slots must
            // stay visible (⬚) so they can be navigated to.
            let editing = cursor.is_some();
            let u = render_row(upper, cur(Field::OpUpper), editing, ctx);
            let l = render_row(lower, cur(Field::OpLower), editing, ctx);
            let b = match node {
                Node::BigOpSym { op, .. } => Block::from_chars(vec![display_char(*op, ctx)]),
                Node::BigOp { name, .. } => Block::from_chars(name.chars().collect()),
                _ => unreachable!(),
            };
            if u.is_empty() && l.is_empty() && cursor.is_none() {
                // Transient un-normalized state: the bare form, which is
                // what normalize would have produced.
                return b;
            }
            // Band marks the horizontal extent of the limits; the base is
            // centered on the band row with its blank cells drawn as ┈
            // ("anything sandwiched in ┈ without spaces takes limits").
            let bw = b.width().max(1);
            let w = u.width().max(l.width()).max(bw) + 2;
            let mut band = vec![OP_BAND; w];
            let left = (w - bw) / 2;
            for (i, &c0) in b.lines[0].iter().enumerate() {
                band[left + i] = c0;
            }
            let mut lines = center_pad(&u, w);
            let baseline = lines.len();
            lines.push(band);
            lines.extend(center_pad(&l, w));
            Annots::default()
                .centered(&u, w, 0)
                .centered(&l, w, baseline + 1)
                .into_block(lines, baseline)
        }

        Node::Brace { over, arg, label } => {
            // ╭──╮ hugging the argument block (╰──╯ underneath for
            // \underbrace), label centered beyond the brace. Width =
            // max(arg, label) + 2, like a fraction bar.
            let a = render_row(arg, cur(Field::BraceArg), true, ctx);
            let l = render_row(label, cur(Field::BraceLabel), l_placeholder(cursor), ctx);
            let w = a.width().max(l.width()).max(1) + 2;
            let mut brace = vec![FRAC_BAR; w];
            let corners = brace_corners(*over);
            brace[0] = corners.0;
            brace[w - 1] = corners.1;
            let mut lines: Vec<Vec<char>> = Vec::new();
            if *over {
                lines.extend(center_pad(&l, w));
                lines.push(brace);
                let a_off = lines.len();
                let baseline = a_off + a.baseline;
                lines.extend(center_pad(&a, w));
                Annots::default()
                    .centered(&l, w, 0)
                    .centered(&a, w, a_off)
                    .into_block(lines, baseline)
            } else {
                let baseline = a.baseline;
                lines.extend(center_pad(&a, w));
                let brace_off = lines.len();
                lines.push(brace);
                lines.extend(center_pad(&l, w));
                Annots::default()
                    .centered(&a, w, 0)
                    .centered(&l, w, brace_off + 1)
                    .into_block(lines, baseline)
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
            let mut body = vec![op.body(); w];
            if !op.right() {
                body[0] = HEAD_LEFT;
            } else {
                body[w - 1] = HEAD_RIGHT;
            }
            let mut lines = center_pad(&o, w);
            let baseline = lines.len();
            lines.push(body);
            lines.extend(center_pad(&u, w));
            Annots::default()
                .centered(&o, w, 0)
                .centered(&u, w, baseline + 1)
                .into_block(lines, baseline)
        }

        // A norm renders exactly like a one-segment ‖ ‖ delimiter; it
        // is a separate node only because its *parse* needs the extent
        // rule (both sides are the same glyph).
        Node::Norm { .. } | Node::Delim { .. } => {
            let DelimShape {
                left,
                right,
                mids,
                segs,
                fusable,
                is_norm,
                angle_sided,
                curly,
            } = delim_shape(node);
            let (left, right) = (&left, &right);
            // A sole Array segment fuses with the delimiter: the delimiter
            // columns absorb the lattice edges (junction rows show ├ ┤,
            // column markers ┬ ┴ ride the delimiter's top/bottom rows).
            // Angles keep their vertex geometry and wrap a bare lattice
            // instead.
            if mids == 0
                && fusable
                && let [seg] = segs
                // Display markers are transparent: a labeled sole-Array
                // segment still fuses (the label overlays a cell).
                && let [Node::Array { rows, cols, cells }] =
                    &seg.iter().filter(|n| !is_marker_node(n)).collect::<Vec<_>>()[..]
            {
                let ai = seg.iter().position(|n| !is_marker_node(n)).unwrap_or(0);
                let seg_cursor = cur(Field::Seg(0));
                let acur = match seg_cursor {
                    Some((path, c)) => match path.first() {
                        Some(&(i, f)) if i == ai => Some((f, (&path[1..], c))),
                        _ => None,
                    },
                    None => None,
                };
                if !matches!(seg_cursor, Some(([], _))) {
                    let mut b = render_fused_grid(*left, *right, *rows, *cols, cells, acur, ctx);
                    // Seg-row markers overlay the delimiter columns;
                    // the FRAME pair lands on the fused block's corners
                    // (the delimiters ARE this matrix's frame).
                    let (bl, w, h) = (b.baseline, b.width(), b.height());
                    for (i, n) in seg.iter().enumerate() {
                        if let Node::Sym(c) = n
                            && is_display_marker(*c)
                        {
                            let (y, x) = match Mark::decode(*c) {
                                Some(Mark::Frame { open: true }) => (0, 0),
                                Some(Mark::Frame { open: false }) => {
                                    (h.saturating_sub(1), w.saturating_sub(1))
                                }
                                _ if i < ai => (bl, 1),
                                _ => (bl, w.saturating_sub(2)),
                            };
                            b.marks.push((y, x, *c));
                        }
                    }
                    return b;
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
                        line[x] = MID;
                    }
                }
                x += b.width();
            }
            let h = body.height().max(1);
            let bl = body.baseline.min(h - 1);
            // Tall angles are drawn from diagonals alone (the ⟨ ⟩ glyphs
            // appear only in the one-line form — mixing them with ╱ ╲
            // reads as a kink). The turn is a same-column vertical pair
            // (╱ over ╲ on the left, ╲ over ╱ on the right); the extent
            // is always even, the baseline is the upper turn row.
            let angle = h >= 2 && angle_sided;
            // Norm-in-norm: both sides are the same ‖, so the outer
            // pair must outsize the inner one — whenever the body holds
            // a full-height ‖ column, grow the extent by a row on each
            // side (the parser groups ‖ columns by vertical extent).
            let inner_norm_full = is_norm
                && (0..body.width()).any(|c| {
                    body.lines
                        .iter()
                        .all(|line| line.get(c).copied() == Some(NORM))
                });
            let (ext_h, ext_bl) = delim_extent(h, bl, angle, curly, inner_norm_full);
            // Pad the body to the delimiter extent, then draw the │
            // middles over the full extent (mid columns must span it).
            let top_pad = ext_bl - bl;
            let width = body.width();
            let mut inner: Vec<Vec<char>> = Vec::with_capacity(ext_h);
            for r in 0..ext_h {
                inner.push(match r.checked_sub(top_pad) {
                    Some(br) if br < h && br < body.lines.len() => body.lines[br].clone(),
                    _ => vec![' '; width],
                });
            }
            let mut x = 0;
            for (kk, b) in parts.iter().enumerate() {
                if kk % 2 == 1 {
                    for line in inner.iter_mut() {
                        line[x] = MID;
                    }
                }
                x += b.width();
            }
            let side = |spec: char, is_left: bool| -> Vec<Vec<char>> {
                if angle && (spec == '⟨' || spec == '⟩') {
                    let k = ext_h / 2;
                    (0..ext_h)
                        .map(|r| {
                            let (dist, glyph) = if r <= ext_bl {
                                (ext_bl - r, if is_left { ARM_RISE } else { ARM_FALL })
                            } else {
                                (r - ext_bl - 1, if is_left { ARM_FALL } else { ARM_RISE })
                            };
                            let col = if is_left { dist } else { (k - 1) - dist };
                            let mut row = vec![' '; k];
                            row[col] = glyph;
                            row
                        })
                        .collect()
                } else {
                    delim_column(spec, is_left, ext_h, ext_bl)
                        .into_iter()
                        .map(|c| vec![c])
                        .collect()
                }
            };
            let lcols = side(*left, true);
            let rcols = side(*right, false);
            let lw = lcols[0].len();
            let mut lines = Vec::with_capacity(ext_h);
            for (r, line) in inner.into_iter().enumerate() {
                let mut row = Vec::with_capacity(lw + width + rcols[0].len());
                row.extend_from_slice(&lcols[r]);
                row.extend(line);
                row.extend_from_slice(&rcols[r]);
                lines.push(row);
            }
            let cancel = body
                .cancel
                .iter()
                .map(|&(r, c)| (r + top_pad, c + lw))
                .collect();
            Block {
                lines,
                baseline: ext_bl,
                cancel,
                caret: body.caret.map(|(r, c)| (r + top_pad, c + lw)),
                marks: body
                    .marks
                    .iter()
                    .map(|&(r, c, ch)| (r + top_pad, c + lw, ch))
                    .collect(),
            }
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
            Block {
                lines: a.lines,
                baseline: a.baseline,
                cancel,
                caret: a.caret,
                marks: a.marks,
            }
        }

        Node::Array { rows, cols, cells } => render_lattice(*rows, *cols, cells, cursor, ctx),
    }
}

/// Delimiter fused with a sole grid segment, in the minimal shape:
/// - rows>=2 and cols>=2: separator rows carry only ┼ at the crossings
/// - one row: ┬ / ┴ marker rows above and below the cells
/// - one column: ├ ┤ junctions in the delimiter columns
///
/// The older ┬┴+junction mixed shape stays legal input; fmt tightens it.
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
        .map(|j| {
            (0..rows)
                .map(|i| blocks[i * cols + j].width())
                .max()
                .unwrap_or(1)
        })
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
    let mut caret: Option<(usize, usize)> = None;
    let mut marks: Vec<(usize, usize, char)> = Vec::new();
    let mut sep_rows: Vec<usize> = Vec::new();
    if one_row {
        lines.push(edge_row(COL_MARK_TOP));
    }
    for i in 0..rows {
        if i > 0 {
            sep_rows.push(lines.len());
            lines.push(edge_row(CROSSING));
        }
        let mut parts: Vec<Block> = Vec::new();
        for j in 0..cols {
            if j > 0 {
                parts.push(Block::new(vec![vec![' ']], 0));
            }
            parts.push(Block::new(vec![vec![' ']], 0));
            let b = &blocks[i * cols + j];
            parts.push(
                Annots::default()
                    .centered(b, col_w[j], 0)
                    .into_block(center_pad(b, col_w[j]), b.baseline),
            );
            parts.push(Block::new(vec![vec![' ']], 0));
        }
        let row_block = hcat(&parts);
        let row_off = lines.len();
        cancel.extend(row_block.cancel.iter().map(|&(r, c)| (r + row_off, c)));
        if let Some((r, c)) = row_block.caret {
            caret = Some((r + row_off, c));
        }
        marks.extend(
            row_block
                .marks
                .iter()
                .map(|&(r, c, ch)| (r + row_off, c, ch)),
        );
        for line in row_block.lines {
            let mut l = line;
            l.resize(width, ' ');
            lines.push(l);
        }
    }
    if one_row {
        lines.push(edge_row(COL_MARK_BOT));
    }
    let h = lines.len();
    let bl = (h - 1) / 2;
    // Single column: the delimiter columns carry the row junctions.
    let junction = cols == 1;
    let lcol = delim_column(left, true, h, bl);
    let rcol = delim_column(right, false, h, bl);
    let mut out = Vec::with_capacity(h);
    for (r, line) in lines.into_iter().enumerate() {
        // Single-column grids dig their row junctions into the
        // delimiter columns as ├ ┤ (U+251C/2524 — the light joints; the
        // heavy ┠ ┨ stood out as the only heavy strokes in the format).
        let (lc, rc) = if junction && sep_rows.contains(&r) {
            (ROW_JUNCTION_L, ROW_JUNCTION_R)
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
    Block {
        lines: out,
        baseline: bl,
        cancel,
        caret: caret.map(|(r, c)| (r, c + 1)),
        marks: marks.into_iter().map(|(r, c, ch)| (r, c + 1, ch)).collect(),
    }
}

/// Self-delimiting grid: ┼ markers at every crossing of the separator
/// rows/columns *including the outer edges*, so the extent and the cell
/// boundaries are explicit without any delimiter (LaTeX \begin{matrix}).
/// Baseline = vertical center of the whole lattice.
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
        .map(|j| {
            (0..rows)
                .map(|i| blocks[i * cols + j].width())
                .max()
                .unwrap_or(1)
        })
        .collect();
    // Marker columns at x = 0 and after every cell span (cell + 1 pad on
    // each side).
    let mut marker_x = vec![0usize];
    for w in &col_w {
        marker_x.push(marker_x.last().unwrap() + w + 3);
    }
    let width = *marker_x.last().unwrap() + 1;
    let kind = |i: usize, n: usize| {
        if i == 0 {
            0
        } else if i == n {
            2
        } else {
            1
        }
    };
    let marker_row = |ri: usize| {
        let mut r = vec![' '; width];
        for (ci, &x) in marker_x.iter().enumerate() {
            r[x] = lattice_char(kind(ri, rows), kind(ci, cols));
        }
        r
    };

    let mut lines: Vec<Vec<char>> = vec![marker_row(0)];
    let mut cancel: Vec<(usize, usize)> = Vec::new();
    let mut caret: Option<(usize, usize)> = None;
    let mut marks: Vec<(usize, usize, char)> = Vec::new();
    for i in 0..rows {
        let mut parts: Vec<Block> = Vec::new();
        for j in 0..cols {
            // marker column + 1 pad, then the centered cell, then 1 pad.
            parts.push(Block::new(vec![vec![' '; 2]], 0));
            let b = &blocks[i * cols + j];
            parts.push(
                Annots::default()
                    .centered(b, col_w[j], 0)
                    .into_block(center_pad(b, col_w[j]), b.baseline),
            );
            parts.push(Block::new(vec![vec![' ']], 0));
        }
        let row_block = hcat(&parts);
        let row_off = lines.len();
        cancel.extend(row_block.cancel.iter().map(|&(r, c)| (r + row_off, c)));
        if let Some((r, c)) = row_block.caret {
            caret = Some((r + row_off, c));
        }
        marks.extend(
            row_block
                .marks
                .iter()
                .map(|&(r, c, ch)| (r + row_off, c, ch)),
        );
        for line in row_block.lines {
            let mut l = line;
            l.resize(width, ' ');
            lines.push(l);
        }
        lines.push(marker_row(i + 1));
    }
    let h = lines.len();
    Block {
        lines,
        baseline: (h - 1) / 2,
        cancel,
        caret,
        marks,
    }
}

/// One rendered column of a delimiter. `spec` is the delimiter spec char,
/// `bl` the baseline row of the body being wrapped. Vertex glyphs (⎨ ⎬ ⟨ ⟩)
/// always sit on the baseline row — that is what lets the parser read the
/// baseline straight off a brace/angle column.
fn delim_column(spec: char, left: bool, h: usize, bl: usize) -> Vec<char> {
    // Angles are drawn from diagonal arms rather than stacked pieces,
    // so they are the one family the table does not describe; a norm
    // keeps the same ‖ at every height.
    match spec {
        '\u{27e8}' | '\u{27e9}' => {
            let (up, down) = if spec == '\u{27e8}' {
                (ARM_RISE, ARM_FALL)
            } else {
                (ARM_FALL, ARM_RISE)
            };
            return (0..h)
                .map(|r| match r.cmp(&bl) {
                    std::cmp::Ordering::Equal => spec,
                    std::cmp::Ordering::Less => up,
                    std::cmp::Ordering::Greater => down,
                })
                .collect();
        }
        '\u{2016}' => return vec!['\u{2016}'; h],
        _ => {}
    }
    let Some((d, spec_side)) = crate::symbols::Delim::of_spec(spec) else {
        return vec![spec; h];
    };
    // The angles never reach here (their arms are drawn above); every
    // column pair lists its tall pieces.
    let Some(cd) = d.col() else {
        return vec![spec; h];
    };
    let info = cd.info();
    // A side-distinct spec names its own side, which is what makes a
    // mismatched pair like `(0,1]` render correctly. `|` and `.` spell
    // both sides the same, so there the caller's side decides.
    let is_left = spec_side.unwrap_or(left);
    if h == 1 {
        return vec![if is_left { info.short.0 } else { info.short.1 }];
    }
    let (top, ext, bot) = info.tall[usize::from(!is_left)];
    (0..h)
        .map(|r| match info.vertex {
            // A brace shows its vertex on the baseline row, ahead of
            // the corners.
            Some(vx) if r == bl => {
                if is_left {
                    vx.0
                } else {
                    vx.1
                }
            }
            _ if r == 0 => top,
            _ if r == h - 1 => bot,
            _ => ext,
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
        let root = vec![Node::Frac {
            num: sym_row("1"),
            den: sym_row("x+1"),
        }];
        assert_eq!(plain(&root), vec!["  1", "─────", " x+1"]);
    }

    #[test]
    fn inline_superscript() {
        let root = vec![Node::Sym('x'), Node::Sup { arg: sym_row("2") }];
        assert_eq!(plain(&root), vec!["x²"]);
    }

    #[test]
    fn bigop_band_marks_limit_extent() {
        let root = vec![Node::BigOpSym {
            op: '∑',
            lower: sym_row("i=0"),
            upper: sym_row("n"),
        }];
        assert_eq!(plain(&root), vec!["  n", "┈┈∑┈┈", " i=0"]);
    }

    #[test]
    fn bigop_without_limits_is_bare() {
        let root = vec![Node::BigOpSym {
            op: '∫',
            lower: vec![],
            upper: vec![],
        }];
        assert_eq!(plain(&root), vec!["∫"]);
    }

    #[test]
    fn sqrt_single_line() {
        let root = vec![Node::Sqrt {
            arg: sym_row("2"),
            index: crate::symbols::Radical::Sqrt,
        }];
        assert_eq!(plain(&root), vec!["┌─", "√2"]);
    }

    #[test]
    fn leading_script_gets_explicit_base() {
        let root = vec![Node::Sup { arg: sym_row("2") }];
        assert_eq!(plain(&root), vec!["⬚²"]);
    }

    #[test]
    fn matrix_2x2() {
        let root = vec![Node::Delim {
            left: crate::symbols::Delim::Col(crate::symbols::ColDelim::Bracket),
            right: crate::symbols::Delim::Col(crate::symbols::ColDelim::Bracket),
            mids: 0,
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
            left: crate::symbols::Delim::Col(crate::symbols::ColDelim::Brace),
            right: crate::symbols::Delim::Col(crate::symbols::ColDelim::Brace),
            mids: 0,
            segs: vec![vec![Node::Frac {
                num: sym_row("1"),
                den: sym_row("2"),
            }]],
        }];
        assert_eq!(plain(&root), vec!["⎧ 1 ⎫", "⎨───⎬", "⎩ 2 ⎭"]);
        // ⟨x|y⟩ single-line braket.
        let root = vec![Node::Delim {
            left: crate::symbols::Delim::Angle,
            right: crate::symbols::Delim::Angle,
            mids: 1,
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
        let root = vec![Node::Func("sin".into()), Node::Sym('x')];
        let ctx = RenderCtx::canonical();
        let b = render_row(&root, None, false, &ctx);
        assert_eq!(b.to_text(), "sin𝑥");
    }

    #[test]
    fn bigop_shows_placeholders_while_editing() {
        // Cursor in the (empty) lower limit: both slots must be visible.
        let root = vec![Node::BigOpSym {
            op: '∑',
            lower: vec![],
            upper: vec![],
        }];
        let path = [(0, Field::OpLower)];
        let b = render_row(&root, Some((&path, 0)), false, &RenderCtx::canonical());
        let text = b.to_text();
        // The caret is zero-width metadata on the lower slot's ⬚ cell.
        let (cy, cx) = b.caret.expect("caret present");
        assert_eq!(b.lines[cy][cx], PLACEHOLDER, "caret on the ⬚:\n{}", text);
        assert!(cy > b.baseline, "caret in the lower limit:\n{}", text);
        assert!(
            text.contains(PLACEHOLDER),
            "empty upper slot visible:\n{}",
            text
        );
        // Cursor elsewhere: canonical bare operator, no placeholders.
        let plain = render_row(&root, None, false, &RenderCtx::canonical()).to_text();
        assert_eq!(plain, "∑");
    }

    #[test]
    fn baseline_alignment_of_fraction_in_row() {
        let root = vec![
            Node::Sym('a'),
            Node::Sym('+'),
            Node::Frac {
                num: sym_row("1"),
                den: sym_row("2"),
            },
        ];
        let lines = plain(&root);
        assert_eq!(lines.len(), 3);
        // 'a' sits on the fraction-bar row.
        assert!(lines[1].starts_with("a+"));
    }
}
