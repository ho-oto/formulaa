//! The 2D block algebra: rectangular character grids with a baseline
//! and two zero-width annotation channels (the caret, display marks),
//! plus the compositions — horizontal concatenation on the baseline,
//! vertical stacking, centered padding — that carry them through. Nothing here knows what a `Node`
//! is; the AST rules live in the parent module. The channel
//! propagation is the footgun CLAUDE.md warns about, so it is
//! unit-tested HERE, where a violation names the composition that
//! dropped it.

use crate::glyphs::OP_BAND;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    /// Rectangular grid (all lines same length).
    pub lines: Vec<Vec<char>>,
    /// Index of the baseline row. May equal `height()` for blocks that sit
    /// entirely above the baseline (superscripts) — never index with it.
    pub baseline: usize,
    /// Editing caret cell (row, col): the glyph right of the insertion
    /// point. Zero-width metadata — the caret never occupies a column,
    /// so the layout is identical to the cursor-less render. None in
    /// canonical output (cursor = None).
    pub caret: Option<(usize, usize)>,
    /// Display markers (row, col, char): jump/block labels, selection
    /// and block-end marks. Like the caret they are zero-width — the
    /// marker atoms the editor inserts render as empty blocks carrying
    /// these annotations, so decorations never move the layout. Always
    /// empty in canonical output.
    pub marks: Vec<(usize, usize, char)>,
}

impl Block {
    pub fn new(lines: Vec<Vec<char>>, baseline: usize) -> Self {
        Block {
            lines,
            baseline,
            caret: None,
            marks: vec![],
        }
    }

    pub(super) fn with_caret(mut self, r: usize, c: usize) -> Self {
        self.caret = Some((r, c));
        self
    }

    /// A zero-width caret marker: contributes no cells, only an
    /// insertion-point position to `hcat`.
    pub(super) fn caret_marker() -> Self {
        Block::empty().with_caret(0, 0)
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
    pub(super) fn baseline_edge(&self, first: bool) -> Option<char> {
        let line = self.lines.get(self.baseline)?;
        if first { line.first() } else { line.last() }.copied()
    }
}

/// Horizontally concatenate blocks, aligning baselines.
pub(super) fn hcat(blocks: &[Block]) -> Block {
    let blocks: Vec<&Block> = blocks
        .iter()
        .filter(|b| !b.is_empty() || b.caret.is_some() || !b.marks.is_empty())
        .collect();
    if blocks.is_empty() {
        return Block::empty();
    }
    if blocks.iter().all(|b| b.is_empty()) {
        // Only zero-width annotations: collect them at the origin.
        let mut out = Block::empty();
        for b in &blocks {
            if b.caret.is_some() {
                out.caret = Some((0, 0));
            }
            out.marks
                .extend(b.marks.iter().map(|&(_, _, ch)| (0, 0, ch)));
        }
        return out;
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
    let mut caret = None;
    let mut marks = Vec::new();
    let mut x = 0;
    for b in blocks {
        let y0 = above - b.baseline;
        for (dy, line) in b.lines.iter().enumerate() {
            for (dx, &c) in line.iter().enumerate() {
                grid[y0 + dy][x + dx] = c;
            }
        }
        if let Some((r, c)) = b.caret {
            caret = Some(if b.is_empty() {
                // Zero-width marker: caret at this x, on the baseline.
                (above, x)
            } else {
                (y0 + r, x + c)
            });
        }
        marks.extend(b.marks.iter().map(|&(r, c, ch)| {
            if b.is_empty() {
                (above, x, ch)
            } else {
                (y0 + r, x + c, ch)
            }
        }));
        x += b.width();
    }
    Block {
        lines: grid,
        baseline: above,
        caret,
        marks,
    }
}

/// The zero-width annotation channels of a Block (caret,
/// caret, display marks), accumulated as children are centered into a
/// parent. One `centered` call per child replaces the three per-channel
/// translations every composite node used to spell out.
#[derive(Default)]
pub(super) struct Annots {
    caret: Option<(usize, usize)>,
    marks: Vec<(usize, usize, char)>,
}

impl Annots {
    /// Fold in a child's annotations, centered into `width` at `row_off`
    /// (first caret wins — a cursor lives in at most one child).
    pub(super) fn centered(mut self, b: &Block, width: usize, row_off: usize) -> Self {
        let left = (width - b.width()) / 2;
        self.caret = self
            .caret
            .or_else(|| b.caret.map(|(r, c)| (r + row_off, c + left)));
        self.marks.extend(
            b.marks
                .iter()
                .map(|&(r, c, ch)| (r + row_off, c + left, ch)),
        );
        self
    }

    pub(super) fn into_block(self, lines: Vec<Vec<char>>, baseline: usize) -> Block {
        Block {
            lines,
            baseline,
            caret: self.caret,
            marks: self.marks,
        }
    }
}

/// Pad every line of `b` to `width`, centered.
pub(super) fn center_pad(b: &Block, width: usize) -> Vec<Vec<char>> {
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

/// Stack blocks vertically, left-aligned, a lone-┈ separator row
/// between lines; the result's baseline is the first block's. All
/// annotations translate.
pub(super) fn vstack(blocks: &[Block]) -> Block {
    // At least one column: the ┈ separator row needs a cell even when
    // every segment is empty (Enter on an empty formula).
    let width = blocks.iter().map(|b| b.width()).max().unwrap_or(0).max(1);
    let mut lines: Vec<Vec<char>> = Vec::new();
    let mut caret = None;
    let mut marks = Vec::new();
    let mut baseline = 0;
    for (i, b) in blocks.iter().enumerate() {
        if i > 0 {
            let mut sep = vec![' '; width];
            sep[0] = OP_BAND;
            lines.push(sep);
        }
        let y0 = lines.len();
        if i == 0 {
            baseline = b.baseline.min(b.height().saturating_sub(1));
        }
        // An empty line still occupies one (blank) row so the marker of
        // the following line keeps its distance.
        if b.is_empty() {
            lines.push(vec![' '; width]);
        }
        for line in &b.lines {
            let mut l = line.clone();
            l.resize(width, ' ');
            lines.push(l);
        }
        if let Some((r, c)) = b.caret {
            caret = Some((y0 + r.min(b.height().saturating_sub(1)), c));
        }
        marks.extend(b.marks.iter().map(|&(r, c, ch)| (y0 + r, c, ch)));
    }
    Block {
        lines,
        baseline,
        caret,
        marks,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b(lines: &[&str], baseline: usize) -> Block {
        Block::new(
            lines.iter().map(|l| l.chars().collect()).collect(),
            baseline,
        )
    }

    /// A baseline may equal height() (a block entirely above the
    /// baseline); no composition may index lines[baseline].
    #[test]
    fn baseline_at_height_survives_composition() {
        let sup = Block::new(vec!["2".chars().collect()], 1);
        assert_eq!(sup.baseline, sup.height());
        let out = hcat(&[b(&["x"], 0), sup]);
        assert_eq!(out.to_strings(), vec![" 2", "x "]);
    }
}
