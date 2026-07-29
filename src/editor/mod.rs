//! Structural editing model. The cursor is a path of (node index, field)
//! pairs from the root row plus a column inside the innermost row —
//! the same model LyX uses for math insets.

use crate::ast::{Field, Node, Row, row_at, row_at_mut};

/// A cursor position: path into nested rows plus a column.
pub type CursorPos = (Vec<(usize, Field)>, usize);
/// A block-select target: (parent row path, node index).
pub type BlockRef = (Vec<(usize, Field)>, usize);
use crate::symbols::{Accent, Delim, bigop_by_char, is_func_name, symbol_by_name};

mod command;
mod modes;

pub use command::{Edit, resolve};

/// One undo step: the formula with the cursor that belonged to it.
type Snapshot = (Row, Vec<(usize, Field)>, usize);

#[derive(Clone)]
pub struct Editor {
    pub root: Row,
    pub path: Vec<(usize, Field)>,
    pub col: usize,
    /// Some(text) while the `\command` minibuffer is open.
    pub minibuffer: Option<String>,
    /// Some((kind, content)) while an in-place name box (\op \op* \rm
    /// \text) is open.
    pub op_entry: Option<(BoxKind, String)>,
    /// Pending backslash escape inside the \text box (the next key is
    /// typed literally, so \" enters a quote).
    pub op_escape: bool,
    /// Grid edit mode (^O inside a matrix): cell-unit selection, with
    /// column/row lane sub-modes (see `GridSel`).
    pub grid: Option<GridSel>,
    /// Undo/redo stacks of snapshots. Pushed by `input` whenever a key
    /// changes the formula; cursor-only motion is not a history step
    /// (but the cursor is restored with the formula it belonged to).
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    pub message: String,
    pub italic: bool,
    /// EasyMotion-style jump: Some(targets) while waiting for a label
    /// key. Document-ordered (needed for marker insertion); each entry
    /// carries its rank (0 = key 'a' = best candidate).
    pub jump: Option<Vec<(usize, CursorPos)>>,
    /// Arrow-key selection inside jump mode: the rank of the marker
    /// currently highlighted (Enter jumps to it).
    pub jump_selected: usize,
    /// Free-cursor mode (^F): Some while active.
    pub free: Option<FreeCursor>,
    /// Block-select mode (Ctrl+B): Some(ancestors of the cursor,
    /// innermost first); each target is (parent row path, node index).
    /// ↑/→ widen the highlighted ancestor, ↓/← narrow it, Enter (or the
    /// target's label key) selects the whole block.
    pub block: Option<Vec<BlockRef>>,
    /// Index into `block` of the highlighted ancestor.
    pub block_sel: usize,
    /// Empty slots kept materialized after jump mode ends (the ⬚ cells
    /// ^G labeled stay visible until the next input, so toggling ^G
    /// twice does not shift the layout).
    pub ghost: Vec<Vec<(usize, Field)>>,
    /// Structure view: paint every block's background by nesting depth (Ctrl+O).
    /// Selection anchor column in the current row (Shift+←/→). The selected
    /// node range is between the anchor and the cursor column.
    pub select_anchor: Option<usize>,
    /// Path at the moment the anchor was set: a selection is only valid
    /// while the cursor stays in that row (leaving the row would make the
    /// anchor point into a different — possibly shorter — row).
    select_path: Vec<(usize, Field)>,
    /// Editor-internal clipboard (^C/^X/^V): a sibling-node slice, or
    /// a rectangle of grid cells.
    clip: Clip,
}

/// Label keys for jump mode, most reachable first.
pub const JUMP_LABELS: &str = "asdfghjklqwertyuiopzxcvbnmASDFGHJKLQWERTYUIOPZXCVBNM0123456789";
/// Private-use chars used as display-time markers (never in a real AST):
/// jump label placeholders …
pub const JUMP_CHAR_BASE: u32 = 0xE000;
/// Selection range markers (drawn as a background-colored box).
pub const SEL_OPEN: char = '\u{E0F0}';
pub const SEL_CLOSE: char = '\u{E0F1}';
/// End-of-block marker paired with a ^B label (display only, like the
/// labels themselves; the TUI turns the pair into a colored box).
pub const BLK_CLOSE: char = '\u{E0F2}';
/// Ghost-slot marker: keeps an empty slot materialized (⬚) after jump
/// mode ends, so re-entering ^G does not shift the layout. No label,
/// no box — just the slot.
pub const SLOT_GHOST: char = '\u{E0F3}';
/// Grid lane-gap cursor: marks the ghost lane previewing an insertion
/// (painted as the green insert band, same shape as a lane selection).
/// Column and row gaps use separate chars so the display knows which
/// axis to stretch along.
pub const GRID_GAP: char = '\u{E0F4}';
pub const GRID_GAP_ROW: char = '\u{E0FF}';
/// Grid lane selection markers: like SEL_OPEN/SEL_CLOSE per cell, but
/// the display fills the union rectangle of all pairs — a whole-lane
/// band including the lattice gaps, not per-cell patches.
pub const LANE_OPEN: char = '\u{E0F5}';
pub const LANE_CLOSE: char = '\u{E0F6}';
/// Cell-rectangle selection markers (contents, not structure): same
/// union-fill display as the lane pairs, painted in the ordinary
/// selection color so "every cell of the column" and "the column
/// itself" stay tellable apart.
pub const CELLS_OPEN: char = '\u{E0F7}';
pub const CELLS_CLOSE: char = '\u{E0F8}';
/// Grid-mode frame markers: wrap the edited Array node so the display
/// can recolor its lattice frame (the mode signal). The FUSED pair is
/// used when the array is fused into its delimiter — the render pushes
/// the markers just inside the delimiter columns, so the display
/// widens its scan by one cell to reach them; the plain pair scans
/// exactly the array's own columns (an enclosing \left( must NOT
/// recolor).
pub const FRAME_OPEN: char = '\u{E0F9}';
pub const FRAME_CLOSE: char = '\u{E0FA}';
pub const FRAME_FUSED_OPEN: char = '\u{E0FB}';
pub const FRAME_FUSED_CLOSE: char = '\u{E0FC}';
/// Row-lane selection markers (LANE_* marks columns): the display
/// stretches a lane band to the matrix region's far edges along its
/// axis, so it needs to know which axis that is.
pub const ROWLANE_OPEN: char = '\u{E0FD}';
pub const ROWLANE_CLOSE: char = '\u{E0FE}';
/// Jump markers encode their rank as JUMP_RANK_BASE + rank (rank 0 =
/// label 'a'; ranks beyond the label alphabet display as unlabeled
/// highlights, reachable via arrow-key selection).
pub const JUMP_RANK_BASE: u32 = 0xE100;
/// Coordinate-probe marks (never displayed): one render with every
/// candidate marked yields the position → cell coordinate table.
const PROBE_BASE: u32 = 0xF000;
/// docs/jump-spec.md §5 — tuning knobs.
const JUMP_W_Y: usize = 3;
const JUMP_R_MIN: usize = 2;
/// α = 1 / JUMP_ALPHA_DIV.
const JUMP_ALPHA_DIV: usize = 4;
const JUMP_C_GHOST: usize = 4;
/// ^F auto-expansion hysteresis: collapsed elements expand within
/// R_IN of the free cursor and fold back only beyond R_OUT (measured
/// against the element's *visible anchor* in the current display
/// frame, so the expansion itself cannot oscillate the test).
const FREE_EXPAND_IN: usize = 3;
const FREE_EXPAND_OUT: usize = 8;
/// Rank-char capacity (E100..E4FF).
const JUMP_MAX_RANKS: usize = 0x400;

/// Free-cursor mode (^F): a display-cell cursor moved with the arrow
/// keys; Enter snaps to the nearest editable position. Both the free
/// cell and the snap preview are shown.
/// What the in-place name box (`\op` family) commits to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoxKind {
    /// \op: upright words (Func / bare \mathrm), joined by ␣.
    Op,
    /// \op* / \limits: an operator band, words as its pieces.
    OpStar,
    /// \rm: one \mathrm run (dictionary word falls back to its Func).
    Rm,
    /// \text: one "double-quoted" text run.
    Text,
    /// \tex / \latex: LaTeX math, read best-effort into nodes.
    Tex,
}

/// Grid edit mode (^O): what the mode's cursor is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridSel {
    /// Cell-unit cursor (the cell the edit cursor is parked in), with
    /// an optional rectangle anchor (Shift+arrows).
    Cells { anchor: Option<usize> },
    /// Lane mode (`c`/`|` for columns, `r`/`-` for rows): the cursor
    /// alternates gap, lane, gap, … — `pos` is `2*i` for the gap left
    /// of/above lane `i` (up to `2*n` for the far edge) and `2*i + 1`
    /// for lane `i` itself. Enter on a gap inserts a lane there;
    /// Delete on a lane removes it. `ext` extends a lane selection
    /// (Shift along the axis) to the other end lane.
    Lanes {
        cols: bool,
        pos: usize,
        ext: Option<usize>,
    },
}

/// The editor clipboard: ordinary sibling nodes, or a rectangle of
/// grid cells (copied from cell selection; pasted over cells when the
/// cursor is in a grid, or as a bare Array node anywhere else).
#[derive(Debug, Clone, PartialEq)]
pub enum Clip {
    Nodes(Row),
    Cells {
        rows: usize,
        cols: usize,
        cells: Vec<Row>,
    },
}

impl Clip {
    fn is_empty(&self) -> bool {
        match self {
            Clip::Nodes(r) => r.is_empty(),
            Clip::Cells { cells, .. } => cells.is_empty(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct FreeCursor {
    /// Current display cell (row, col).
    pub at: (usize, usize),
    /// Nearest editable position (the snap target) …
    pub snap: CursorPos,
    /// … and its display cell.
    pub snap_at: (usize, usize),
}

/// Adjust a position for one SLOT_GHOST inserted at col 0 of every
/// ghost row. All comparisons use the *original* coordinates — nudging
/// incrementally breaks on nested ghosts (an outer adjustment makes the
/// inner ghost's prefix no longer match).
fn ghost_adjust(
    ghost: &[Vec<(usize, Field)>],
    p: &[(usize, Field)],
    c: usize,
) -> (Vec<(usize, Field)>, usize) {
    let mut p2 = p.to_vec();
    let mut c2 = c;
    for g in ghost {
        if p.len() > g.len() && p[..g.len()] == g[..] {
            p2[g.len()].0 += 1;
        } else if p == &g[..] {
            c2 += 1;
        }
    }
    (p2, c2)
}

/// A jump candidate position with the flags the selection rules need
/// (docs/jump-spec.md §2–3).
#[derive(Clone, Debug)]
pub struct JumpCand {
    pub pos: CursorPos,
    /// The row is empty (an unfilled slot).
    pub empty: bool,
    /// End-of-cell position of a grid.
    pub cell_end: bool,
    /// Interior of a spaceless atom run (hard-filtered).
    pub interior: bool,
    /// Start or end of its row (col 0 / col len).
    pub bound: bool,
    /// The current cursor position itself.
    pub is_cursor: bool,
}

impl Default for Editor {
    fn default() -> Self {
        Self::new()
    }
}

/// `\lr…` / `\delim…` (Typst-style) delimiter spec, read in *visual
/// order*: first token = left, interior tokens = middles ('|' only),
/// last token = right. A token is one spec char — `( ) [ ] { } | .`
/// with `<` `>` aliasing ⟨ ⟩ — or a `\name` (\langle, \vert, \none …),
/// so `\lr(]`, `\lr{|}` and `\lr\langle||\rangle` all read like the
/// picture. None when the string is not a delimiter spec (a `\lr…`
/// symbol name like \lrcorner then resolves normally).
/// A script-spelled command: the ^ / _ marker may lead, trail, or both
/// (\^z, \z^ and \^z^ are the same superscript). Returns (sup?, arg).
fn script_cmd(cmd: &str) -> Option<(bool, crate::ast::Row)> {
    let lead = cmd.chars().next().filter(|c| matches!(c, '^' | '_'));
    let trail = if cmd.chars().count() > 1 {
        cmd.chars().last().filter(|c| matches!(c, '^' | '_'))
    } else {
        None
    };
    let marker = match (lead, trail) {
        (Some(a), Some(b)) if a == b => a,
        (Some(a), None) => a,
        (None, Some(b)) => b,
        _ => return None,
    };
    let mut rest = cmd;
    if lead.is_some() {
        rest = &rest[1..];
    }
    if trail.is_some() {
        rest = &rest[..rest.len() - 1];
    }
    if rest.is_empty() {
        return None;
    }
    // A symbol name (\^gamma) or a run of ASCII alphanumerics (\_10).
    let arg: crate::ast::Row = if let Some(c) = symbol_by_name(rest) {
        vec![Node::Sym(c)]
    } else if rest.chars().all(|c| c.is_ascii_alphanumeric()) {
        rest.chars().map(Node::Sym).collect()
    } else {
        return None;
    };
    Some((marker == '^', arg))
}

fn lr_spec(cmd: &str) -> Option<(Delim, Delim, usize)> {
    let spec = cmd
        .strip_prefix("delim")
        .or_else(|| cmd.strip_prefix("lr"))?;
    let mut tokens = Vec::new();
    let mut it = spec.chars().peekable();
    while let Some(c) = it.next() {
        let tok = match c {
            '\\' => {
                let mut name = String::new();
                while it.peek().is_some_and(|c| c.is_ascii_alphabetic()) {
                    name.push(it.next().unwrap());
                }
                *crate::symbols::DELIM_NAMES.get(name.as_str())?
            }
            '<' => '⟨',
            '>' => '⟩',
            c => c,
        };
        tokens.push(tok);
    }
    if tokens.len() < 2 {
        return None;
    }
    // The slot decides the side: a right-shaped glyph cannot open
    // (`\lr][` would draw a picture that reads back as unmatched).
    let left = Delim::of_spec_side(tokens[0], true)?;
    let right = Delim::of_spec_side(*tokens.last().unwrap(), false)?;
    let mids = &tokens[1..tokens.len() - 1];
    if !mids.iter().all(|&c| c == '|') {
        return None;
    }
    Some((left, right, mids.len()))
}

/// How a grid command wraps its lattice: a delimiter pair, the ‖ ‖
/// norm (\Vmatrix), or nothing (bare \array).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridWrap {
    Bare,
    Pair(Delim, Delim),
    Norm,
}

/// Grid minibuffer commands with an optional RxC digit suffix
/// (`matrix34` = 3 rows × 4 cols; bare name = 2×2). Returns the delimiter
/// pair and the dimensions.
fn grid_command(cmd: &str) -> Option<(GridWrap, usize, usize)> {
    const GRIDS: &[(&str, GridWrap)] = &[
        ("matrix", GridWrap::Pair(Delim::Bracket, Delim::Bracket)),
        ("bmatrix", GridWrap::Pair(Delim::Bracket, Delim::Bracket)),
        ("pmatrix", GridWrap::Pair(Delim::Paren, Delim::Paren)),
        ("Bmatrix", GridWrap::Pair(Delim::Brace, Delim::Brace)),
        ("vmatrix", GridWrap::Pair(Delim::Bar, Delim::Bar)),
        ("Vmatrix", GridWrap::Norm),
        ("cases", GridWrap::Pair(Delim::Brace, Delim::Null)),
        ("rcases", GridWrap::Pair(Delim::Null, Delim::Brace)),
        ("array", GridWrap::Bare),
    ];
    for &(name, delims) in GRIDS {
        let Some(rest) = cmd.strip_prefix(name) else {
            continue;
        };
        match rest.as_bytes() {
            [] => return Some((delims, 2, 2)),
            [r, c] if r.is_ascii_digit() && c.is_ascii_digit() => {
                let (rows, cols) = ((r - b'0') as usize, (c - b'0') as usize);
                if rows >= 1 && cols >= 1 {
                    return Some((delims, rows, cols));
                }
            }
            _ => {}
        }
    }
    None
}

impl Editor {
    pub fn new() -> Self {
        Editor {
            root: Vec::new(),
            path: Vec::new(),
            col: 0,
            minibuffer: None,
            op_entry: None,
            op_escape: false,
            grid: None,
            undo: Vec::new(),
            redo: Vec::new(),
            message: String::new(),
            italic: true,
            jump: None,
            jump_selected: 0,
            free: None,
            block: None,
            block_sel: 0,
            ghost: Vec::new(),
            select_anchor: None,
            select_path: Vec::new(),
            clip: Clip::Nodes(Vec::new()),
        }
    }

    pub fn cur_row(&self) -> &Row {
        row_at(&self.root, &self.path)
    }

    fn cur_row_mut(&mut self) -> &mut Row {
        row_at_mut(&mut self.root, &self.path)
    }

    // ----- insertion -----

    pub fn insert_sym(&mut self, c: char) {
        let col = self.col;
        self.cur_row_mut().insert(col, Node::Sym(c));
        self.col += 1;
    }

    /// `/` key: a second `/` right after a `/` atom replaces it with an
    /// empty fraction (`//` shorthand for \frac — for a literal `//`,
    /// type `/ /` and delete the spacer).
    pub fn slash(&mut self) {
        if self.col > 0 && matches!(self.cur_row()[self.col - 1], Node::Sym('/')) {
            self.col -= 1;
            let col = self.col;
            self.cur_row_mut().remove(col);
            self.insert_and_enter(Node::Frac {
                num: vec![],
                den: vec![],
            });
        } else {
            self.select_anchor = None;
            self.insert_sym('/');
        }
    }

    /// Mouse click at canvas cell (x, y): move the cursor to the
    /// nearest position (single-render coordinate table).
    pub fn click(&mut self, x: usize, y: usize) {
        self.jump = None;
        self.block = None;
        self.free = None;
        self.select_anchor = None;
        if let Some((pos, _)) = self.nearest_position(x, y) {
            self.path = pos.0;
            self.col = pos.1;
        }
    }

    /// Nearest editable position to a display cell, with its own cell
    /// (shared by mouse clicks and the ^F snap preview). Run interiors
    /// stay reachable — density rules only apply to ^G labels. Only
    /// positions actually visible on screen participate, so the
    /// coordinates match the display exactly.
    fn nearest_position(&self, x: usize, y: usize) -> Option<(CursorPos, (usize, usize))> {
        let cands = self.jump_candidates();
        let coords = self.display_coords(&cands);
        (0..cands.len())
            .filter_map(|i| coords[i].map(|xy| (i, xy)))
            .min_by_key(|&(_, (cy, cx))| cy.abs_diff(y) * 1000 + cx.abs_diff(x))
            .map(|(i, xy)| (cands[i].pos.clone(), xy))
    }

    /// Is this position visible in the current display? Rows collapse
    /// when they are inline scripts or empty optional slots, unless a
    /// ghost mark or the cursor keeps them open.
    fn display_visible(&self, pos: &CursorPos) -> bool {
        use crate::render::is_inline_script_row;
        let mut row: &Row = &self.root;
        let mut prefix: Vec<(usize, Field)> = Vec::new();
        for &(i, f) in &pos.0 {
            prefix.push((i, f));
            let child: &Row = row[i].field(f);
            let ghosted = self.ghost.iter().any(|g| g == &prefix);
            let d = prefix.len() - 1;
            // The cursor anywhere inside this node keeps its slots open.
            let editing = self.path.len() > d
                && self.path[..d] == prefix[..d]
                && self.path[d].0 == prefix[d].0;
            let visible = match f {
                Field::SupArg => ghosted || editing || !is_inline_script_row(child, true),
                Field::SubArg => ghosted || editing || !is_inline_script_row(child, false),
                Field::OpLower
                | Field::OpUpper
                | Field::ArrowOver
                | Field::ArrowUnder
                | Field::BraceLabel => ghosted || editing || !child.is_empty(),
                _ => true,
            };
            if !visible {
                return false;
            }
            row = child;
        }
        true
    }

    /// Coordinates in the *display* geometry: ghost rows materialized,
    /// probes only on visible positions (invisible ones get None).
    fn display_coords(&self, cands: &[JumpCand]) -> Vec<Option<(usize, usize)>> {
        use crate::render::{RenderCtx, render_root};
        let n = cands.len().min(0x800);
        let mut root = self.root.clone();
        // Ghosts first (deepest rows first), then the probes with their
        // paths nudged past the ghost insertions — mixing the orders
        // corrupts whichever set is inserted second.
        let mut ghosts: Vec<&Vec<(usize, Field)>> = self.ghost.iter().collect();
        ghosts.sort_by_key(|p| std::cmp::Reverse(p.len()));
        for p in ghosts {
            row_at_mut(&mut root, p).insert(0, Node::Sym(SLOT_GHOST));
        }
        for (idx, cand) in cands.iter().take(n).enumerate().rev() {
            if !self.display_visible(&cand.pos) {
                continue;
            }
            let (p, c) = &cand.pos;
            let (p2, c2) = ghost_adjust(&self.ghost, p, *c);
            let mark = char::from_u32(PROBE_BASE + idx as u32).unwrap();
            row_at_mut(&mut root, &p2).insert(c2, Node::Sym(mark));
        }
        let b = render_root(&root, None, &RenderCtx::canonical());
        let mut out = vec![None; cands.len()];
        for &(y, x, ch) in &b.marks {
            let u = ch as u32;
            if u >= PROBE_BASE {
                let i = (u - PROBE_BASE) as usize;
                if i < out.len() {
                    out[i] = Some((y, x));
                }
            }
        }
        out
    }

    // ----- navigation -----

    pub fn right(&mut self) {
        let row = self.cur_row();
        if self.col < row.len() {
            if let Some(&f) = row[self.col].fields().first() {
                self.path.push((self.col, f));
                self.col = 0;
            } else {
                self.col += 1;
            }
        } else if let Some((i, f)) = self.path.pop() {
            let parent = self.cur_row();
            let fields = parent[i].fields();
            let k = fields.iter().position(|&x| x == f).unwrap();
            if k + 1 < fields.len() {
                self.path.push((i, fields[k + 1]));
                self.col = 0;
            } else {
                self.col = i + 1;
            }
        }
    }

    pub fn left(&mut self) {
        let row = self.cur_row();
        if self.col > 0 {
            if let Some(&f) = row[self.col - 1].fields().last() {
                let end = row[self.col - 1].field(f).len();
                self.path.push((self.col - 1, f));
                self.col = end;
            } else {
                self.col -= 1;
            }
        } else if let Some((i, f)) = self.path.pop() {
            let parent = self.cur_row();
            let fields = parent[i].fields();
            let k = fields.iter().position(|&x| x == f).unwrap();
            if k > 0 {
                let prev = fields[k - 1];
                let end = parent[i].field(prev).len();
                self.path.push((i, prev));
                self.col = end;
            } else {
                self.col = i;
            }
        }
    }

    /// Up/Down switch between vertically stacked fields (num/den, limits,
    /// matrix rows).
    pub fn vertical(&mut self, up: bool) {
        if let Some(&(i, f)) = self.path.last() {
            let parent_path = &self.path[..self.path.len() - 1];
            let node = &row_at(&self.root, parent_path)[i];
            let target = match (f, up) {
                (Field::FracNum, false) => Some(Field::FracDen),
                (Field::FracDen, true) => Some(Field::FracNum),
                (Field::OpLower, true) => Some(Field::OpUpper),
                (Field::OpUpper, false) => Some(Field::OpLower),
                (Field::ArrowUnder, true) => Some(Field::ArrowOver),
                (Field::ArrowOver, false) => Some(Field::ArrowUnder),
                (Field::BraceArg, dir) => match node {
                    Node::Brace { over, .. } if *over == dir => Some(Field::BraceLabel),
                    _ => None,
                },
                (Field::BraceLabel, dir) => match node {
                    Node::Brace { over, .. } if *over != dir => Some(Field::BraceArg),
                    _ => None,
                },
                (Field::Cell(c), up) => match node {
                    Node::Array { cols, cells, .. } => {
                        if up && c >= *cols {
                            Some(Field::Cell(c - cols))
                        } else if !up && c + cols < cells.len() {
                            Some(Field::Cell(c + cols))
                        } else {
                            None
                        }
                    }
                    _ => None,
                },
                _ => None,
            };
            if let Some(t) = target {
                self.path.pop();
                let len = {
                    let parent = self.cur_row();
                    parent[i].field(t).len()
                };
                self.path.push((i, t));
                self.col = self.col.min(len);
                return;
            }
        }
        // ↑/↓ at the top/bottom row of a grid: exit the matrix, landing
        // before/after it in the surrounding row (stepping out of the
        // wrapping delimiter as well).
        if let Some(&(i, Field::Cell(c))) = self.path.last() {
            let parent_path = &self.path[..self.path.len() - 1];
            if let Node::Array { cols, cells, .. } = &row_at(&self.root, parent_path)[i] {
                let at_edge = if up {
                    c < *cols
                } else {
                    c + cols >= cells.len()
                };
                if at_edge {
                    self.path.pop();
                    let mut idx = i;
                    if let Some(&(d, Field::Seg(_))) = self.path.last() {
                        self.path.pop();
                        idx = d;
                    }
                    self.col = if up { idx } else { idx + 1 };
                    return;
                }
            }
        }
        // Bare big operator to the left of the cursor: promote it to a
        // band and enter the limit. Needed to reopen the limits of a
        // normalized formula — an empty-limit BigOp does not survive the
        // canonical form (e.g. a --session restore), so a plain ∑ / lim
        // atom must be liftable back from the UI.
        if self.col > 0 {
            let col = self.col - 1;
            let promotable = match &self.cur_row()[col] {
                Node::Sym(c) => bigop_by_char(*c),
                Node::Func(name) => crate::symbols::func_takes_limits(name),
                _ => false,
            };
            if promotable {
                let row = self.cur_row_mut();
                row[col] = match &row[col] {
                    Node::Sym(c) => Node::BigOpSym {
                        op: *c,
                        lower: vec![],
                        upper: vec![],
                    },
                    Node::Func(name) => Node::BigOp {
                        name: name.clone(),
                        lower: vec![],
                        upper: vec![],
                    },
                    _ => unreachable!(),
                };
                let f = if up { Field::OpUpper } else { Field::OpLower };
                self.path.push((col, f));
                self.col = 0;
                return;
            }
        }
        // Multi-line formulas: with no vertical target in the enclosing
        // structure, ↑/↓ move between formula lines (whatever the
        // nesting depth), keeping the horizontal position close.
        if self.root.iter().any(|n| matches!(n, Node::Break)) {
            let seg_of = |steps: &[Node]| steps.iter().filter(|n| matches!(n, Node::Break)).count();
            let top = self.path.first().map_or(self.col, |&(i, _)| i);
            let cur_seg = seg_of(&self.root[..top]);
            let max_seg = seg_of(&self.root);
            let target = if up {
                cur_seg.checked_sub(1)
            } else {
                (cur_seg < max_seg).then_some(cur_seg + 1)
            };
            let Some(target) = target else { return };
            let cands = self.jump_candidates();
            let coords = self.display_coords(&cands);
            let Some(my) = cands
                .iter()
                .position(|c| c.is_cursor)
                .and_then(|i| coords[i])
            else {
                return;
            };
            let seg_of_pos = |pos: &CursorPos| {
                let top = pos.0.first().map_or(pos.1, |&(i, _)| i);
                self.root[..top]
                    .iter()
                    .filter(|n| matches!(n, Node::Break))
                    .count()
            };
            let best = (0..cands.len())
                .filter(|&i| !cands[i].is_cursor && seg_of_pos(&cands[i].pos) == target)
                .filter_map(|i| coords[i].map(|xy| (i, xy)))
                .min_by_key(|&(_, (y, x))| x.abs_diff(my.1) * 10 + y.abs_diff(my.0));
            if let Some((i, _)) = best {
                let (p, c) = cands[i].pos.clone();
                self.path = p;
                self.col = c;
            }
        }
    }

    /// LyX-style Space: leave the innermost inset, landing just after it.
    pub fn exit_inset(&mut self) {
        if let Some((i, _)) = self.path.pop() {
            self.col = i + 1;
        }
    }

    /// Start of the current formula line (Breaks bound lines at the
    /// top level; inside insets it is the row start).
    pub fn home(&mut self) {
        let row = self.cur_row();
        self.col = row[..self.col]
            .iter()
            .rposition(|n| matches!(n, Node::Break))
            .map_or(0, |i| i + 1);
    }

    /// End of the current formula line (before the next Break).
    pub fn end(&mut self) {
        let row = self.cur_row();
        self.col = row[self.col..]
            .iter()
            .position(|n| matches!(n, Node::Break))
            .map_or(row.len(), |i| self.col + i);
    }

    // ----- deletion -----

    pub fn backspace(&mut self) {
        if self.col > 0 {
            let target = &self.cur_row()[self.col - 1];
            if !target.fields().is_empty() && !target.is_empty_structure() {
                // Non-empty structure: step inside (LyX behaviour) so the
                // user deletes its content first instead of losing it all.
                self.left();
            } else {
                self.col -= 1;
                let col = self.col;
                self.cur_row_mut().remove(col);
            }
        } else if let Some(&(i, _)) = self.path.last() {
            let parent_path = &self.path[..self.path.len() - 1];
            let node = &row_at(&self.root, parent_path)[i];
            if node.is_empty_structure() {
                self.path.pop();
                let col = i;
                self.cur_row_mut().remove(col);
                self.col = col;
            } else {
                self.left();
            }
        }
    }

    pub fn delete(&mut self) {
        let row = self.cur_row();
        if self.col < row.len() {
            let target = &row[self.col];
            if !target.fields().is_empty() && !target.is_empty_structure() {
                self.right();
            } else {
                let col = self.col;
                self.cur_row_mut().remove(col);
            }
        }
    }

    // ----- LyX-like keys -----

    /// Insert a delimiter block and enter its first segment.
    pub fn insert_delim(&mut self, left: Delim, right: Delim, mids: usize) {
        let segs = vec![vec![]; mids + 1];
        self.insert_and_enter(Node::Delim {
            left,
            right,
            mids,
            segs,
        });
    }

    /// Close (step out of) the innermost enclosing Delim whose right
    /// delimiter is `right`. Returns false when there is none.
    pub fn close_delim(&mut self, right: Delim) -> bool {
        for k in (0..self.path.len()).rev() {
            let (i, f) = self.path[k];
            if !matches!(f, Field::Seg(_)) {
                continue;
            }
            let node = &row_at(&self.root, &self.path[..k])[i];
            if matches!(node, Node::Delim { right: r, .. } if *r == right) {
                self.path.truncate(k);
                self.col = i + 1;
                return true;
            }
        }
        false
    }

    /// `)` closes the innermost ( … ) inset. A literal `)` atom is not
    /// allowed: it is indistinguishable from a closing delimiter, so a
    /// mismatched-pair scan inside any delimiter would misread it.
    pub fn close_paren(&mut self) {
        if !self.close_delim(Delim::Paren) {
            self.message = "not inside a ( ) inset (( inserts one)".into();
        }
    }

    /// `]` leaves the innermost [ … ] block (matrix) or bare array.
    pub fn close_bracket(&mut self) {
        if self.close_delim(Delim::Bracket) {
            return;
        }
        if let Some((k, i, _)) = self.enclosing_array() {
            self.path.truncate(k);
            self.col = i + 1;
        } else {
            self.message = "not inside a [ ] block ([ inserts one; \\matrix for grids)".into();
        }
    }

    /// `}` leaves the innermost { … } block.
    pub fn close_brace(&mut self) {
        if !self.close_delim(Delim::Brace) {
            self.message = "not inside a { } block ({ inserts one)".into();
        }
    }

    /// Insert a rows×cols grid wrapped in the given delimiter pair and put
    /// the cursor into the first cell.
    pub fn insert_grid(&mut self, left: Delim, right: Delim, rows: usize, cols: usize) {
        let array = Node::Array {
            rows,
            cols,
            cells: vec![vec![]; rows * cols],
        };
        let node = Node::Delim {
            left,
            right,
            mids: 0,
            segs: vec![vec![array]],
        };
        let col = self.col;
        self.cur_row_mut().insert(col, node);
        self.path.push((col, Field::Seg(0)));
        self.path.push((0, Field::Cell(0)));
        self.col = 0;
    }

    /// Insert a rows×cols grid inside a ‖ ‖ norm (\Vmatrix) and put
    /// the cursor into the first cell.
    pub fn insert_norm_grid(&mut self, rows: usize, cols: usize) {
        let array = Node::Array {
            rows,
            cols,
            cells: vec![vec![]; rows * cols],
        };
        let node = Node::Norm { arg: vec![array] };
        let col = self.col;
        self.cur_row_mut().insert(col, node);
        self.path.push((col, Field::Seg(0)));
        self.path.push((0, Field::Cell(0)));
        self.col = 0;
    }

    /// Innermost enclosing Array: (path index, node index, cell index).
    pub(crate) fn enclosing_array(&self) -> Option<(usize, usize, usize)> {
        self.path
            .iter()
            .rposition(|&(_, f)| matches!(f, Field::Cell(_)))
            .map(|k| {
                let (i, Field::Cell(c)) = self.path[k] else {
                    unreachable!()
                };
                (k, i, c)
            })
    }

    /// Grid editing. `MutOp` computes (new rows, new cols, new cells, new
    /// cursor cell) from the current grid and cursor cell.
    fn edit_array(
        &mut self,
        op: impl FnOnce(usize, usize, &mut Vec<Row>, usize) -> Option<(usize, usize, usize)>,
    ) {
        let Some((k, i, c)) = self.enclosing_array() else {
            self.message = "not inside a matrix/array".into();
            return;
        };
        let parent_path = self.path[..k].to_vec();
        let Node::Array { rows, cols, cells } = &mut row_at_mut(&mut self.root, &parent_path)[i]
        else {
            unreachable!()
        };
        let Some((nr, nc, ncell)) = op(*rows, *cols, cells, c) else {
            self.message = "cannot remove the last row/column".into();
            return;
        };
        *rows = nr;
        *cols = nc;
        self.path.truncate(k);
        self.path.push((i, Field::Cell(ncell)));
        self.col = 0;
    }

    /// Insert an empty row below the cursor's row (Enter inside a grid).
    pub fn add_row(&mut self) {
        self.edit_array(|rows, cols, cells, c| {
            let r = c / cols;
            for j in 0..cols {
                cells.insert((r + 1) * cols + j, vec![]);
            }
            Some((rows + 1, cols, (r + 1) * cols + c % cols))
        });
    }

    /// Insert an empty column right of the cursor's column.
    pub fn add_col(&mut self) {
        self.edit_array(|rows, cols, cells, c| {
            let j = c % cols;
            for r in (0..rows).rev() {
                cells.insert(r * cols + j + 1, vec![]);
            }
            Some((rows, cols + 1, (c / cols) * (cols + 1) + j + 1))
        });
    }

    /// Delete the cursor's row (unless it is the only one).
    pub fn del_row(&mut self) {
        self.edit_array(|rows, cols, cells, c| {
            if rows == 1 {
                return None;
            }
            let r = c / cols;
            cells.drain(r * cols..(r + 1) * cols);
            Some((rows - 1, cols, r.min(rows - 2) * cols + c % cols))
        });
    }

    /// Delete the cursor's column (unless it is the only one).
    pub fn del_col(&mut self) {
        self.edit_array(|rows, cols, cells, c| {
            if cols == 1 {
                return None;
            }
            let j = c % cols;
            for r in (0..rows).rev() {
                cells.remove(r * cols + j);
            }
            Some((rows, cols - 1, (c / cols) * (cols - 1) + j.min(cols - 2)))
        });
    }

    /// Insert an empty lane (column when `cols_mode`, row otherwise)
    /// at gap `g` (0..=n), parking the cursor in the new lane.
    pub fn lane_insert(&mut self, cols_mode: bool, g: usize) {
        self.edit_array(|rows, cols, cells, c| {
            if cols_mode {
                for r in (0..rows).rev() {
                    cells.insert(r * cols + g.min(cols), vec![]);
                }
                Some((rows, cols + 1, (c / cols) * (cols + 1) + g.min(cols)))
            } else {
                for j in 0..cols {
                    cells.insert(g.min(rows) * cols + j, vec![]);
                }
                Some((rows + 1, cols, g.min(rows) * cols + c % cols))
            }
        });
    }

    /// Delete lanes `lo..=hi` (columns when `cols_mode`); refuses to
    /// delete every lane of the axis.
    pub fn lane_delete(&mut self, cols_mode: bool, lo: usize, hi: usize) {
        self.edit_array(|rows, cols, cells, c| {
            let n = hi - lo + 1;
            if cols_mode {
                if n >= cols {
                    return None;
                }
                for r in (0..rows).rev() {
                    cells.drain(r * cols + lo..r * cols + hi + 1);
                }
                let j = c % cols;
                let j = if j > hi { j - n } else { j.min(cols - n - 1) };
                Some((rows, cols - n, (c / cols) * (cols - n) + j))
            } else {
                if n >= rows {
                    return None;
                }
                cells.drain(lo * cols..(hi + 1) * cols);
                let r = c / cols;
                let r = if r > hi { r - n } else { r.min(rows - n - 1) };
                Some((rows - n, cols, r * cols + c % cols))
            }
        });
    }

    /// Grid mode: move one cell in the given direction (clamped at the
    /// edges), cursor at the end of the target cell.
    pub fn grid_move(&mut self, dr: isize, dc: isize) {
        let Some((k, i, c)) = self.enclosing_array() else {
            return;
        };
        let parent_path = self.path[..k].to_vec();
        let Node::Array { rows, cols, .. } = &row_at(&self.root, &parent_path)[i] else {
            unreachable!()
        };
        let (rows, cols) = (*rows, *cols);
        let r = (c / cols).saturating_add_signed(dr).min(rows - 1);
        let j = (c % cols).saturating_add_signed(dc).min(cols - 1);
        self.path.truncate(k);
        self.path.push((i, Field::Cell(r * cols + j)));
        self.col = self.cur_row().len();
    }

    /// The enclosing array's shape: (path index, node index, rows,
    /// cols, cursor cell).
    pub(crate) fn grid_info(&self) -> Option<(usize, usize, usize, usize, usize)> {
        let (k, i, c) = self.enclosing_array()?;
        let Node::Array { rows, cols, .. } = &row_at(&self.root, &self.path[..k])[i] else {
            unreachable!()
        };
        Some((k, i, *rows, *cols, c))
    }

    /// The selected cell rectangle in grid mode: (r0, c0, r1, c1),
    /// inclusive — the current cell alone when no anchor is set.
    pub fn grid_rect(&self) -> Option<(usize, usize, usize, usize)> {
        let (_, _, _, cols, c) = self.grid_info()?;
        let Some(GridSel::Cells { anchor }) = self.grid else {
            return None;
        };
        let a = anchor.unwrap_or(c);
        let (r0, r1) = ((a / cols).min(c / cols), (a / cols).max(c / cols));
        let (j0, j1) = ((a % cols).min(c % cols), (a % cols).max(c % cols));
        Some((r0, j0, r1, j1))
    }

    /// Clear the contents of the selected cells (grid-mode Backspace:
    /// the cells stay, their contents go).
    pub fn grid_clear_cells(&mut self) {
        let Some((r0, j0, r1, j1)) = self.grid_rect() else {
            return;
        };
        self.edit_array(|rows, cols, cells, c| {
            for r in r0..=r1.min(rows - 1) {
                for j in j0..=j1.min(cols - 1) {
                    cells[r * cols + j].clear();
                }
            }
            Some((rows, cols, c))
        });
        self.grid = Some(GridSel::Cells { anchor: None });
    }

    /// Copy the selected cell rectangle into the clipboard.
    pub fn grid_copy_cells(&mut self) {
        let Some((r0, j0, r1, j1)) = self.grid_rect() else {
            return;
        };
        let Some((k, i, _, cols, _)) = self.grid_info() else {
            return;
        };
        let Node::Array { cells, .. } = &row_at(&self.root, &self.path[..k])[i] else {
            unreachable!()
        };
        let (ch, cw) = (r1 - r0 + 1, j1 - j0 + 1);
        let mut out = Vec::with_capacity(ch * cw);
        for r in r0..=r1 {
            for j in j0..=j1 {
                out.push(cells[r * cols + j].clone());
            }
        }
        self.clip = Clip::Cells {
            rows: ch,
            cols: cw,
            cells: out,
        };
        self.message = format!("copied {}×{} cell(s)", ch, cw);
    }

    /// Cut = copy + clear.
    pub fn grid_cut_cells(&mut self) {
        self.grid_copy_cells();
        if matches!(self.clip, Clip::Cells { .. }) {
            self.grid_clear_cells();
        }
    }

    /// Paste a cell rectangle at the current cell, overwriting; the
    /// grid grows to fit an overhanging block (^Z undoes the lot).
    pub fn grid_paste_cells(&mut self, ch: usize, cw: usize, clip: Vec<Row>) {
        self.edit_array(|rows, cols, cells, c| {
            let (r0, j0) = (c / cols, c % cols);
            let (nr, nc) = ((r0 + ch).max(rows), (j0 + cw).max(cols));
            let mut grown = vec![Vec::new(); nr * nc];
            for r in 0..rows {
                for j in 0..cols {
                    grown[r * nc + j] = std::mem::take(&mut cells[r * cols + j]);
                }
            }
            for r in 0..ch {
                for j in 0..cw {
                    grown[(r0 + r) * nc + (j0 + j)] = clip[r * cw + j].clone();
                }
            }
            *cells = grown;
            Some((nr, nc, r0 * nc + j0))
        });
        self.grid = Some(GridSel::Cells { anchor: None });
    }

    /// `\mid`: split the current Delim segment at the cursor, inserting a
    /// │ middle; the cursor lands at the start of the new segment.
    pub fn insert_mid(&mut self) {
        let Some(&(i, Field::Seg(k))) = self.path.last() else {
            self.message = "\\mid works directly inside a delimiter block".into();
            return;
        };
        let col = self.col;
        let parent_path = self.path[..self.path.len() - 1].to_vec();
        let Node::Delim { mids, segs, .. } = &mut row_at_mut(&mut self.root, &parent_path)[i]
        else {
            unreachable!()
        };
        let tail: Row = segs[k].split_off(col);
        segs.insert(k + 1, tail);
        *mids += 1;
        *self.path.last_mut().unwrap() = (i, Field::Seg(k + 1));
        self.col = 0;
    }

    /// Wrap the atom just before the cursor with an accent mark.
    fn apply_accent(&mut self, mark: Accent) {
        // A selection becomes the base of a stretchy accent (\widehat
        // over anything — the accent band rides above/below the block).
        if self.selection().is_some() {
            if let Some(content) = self.take_selection() {
                let under = mark.under();
                let (overs, unders) = if under {
                    (vec![], vec![mark])
                } else {
                    (vec![mark], vec![])
                };
                let node = Node::WideAccent {
                    overs,
                    unders,
                    base: content,
                };
                let col = self.col;
                self.cur_row_mut().insert(col, node);
                self.col += 1;
            }
            return;
        }
        if self.col == 0 {
            self.message = "accent needs a base character before the cursor".into();
            return;
        }
        let col = self.col;
        let row = self.cur_row_mut();
        let under = mark.under();
        match &mut row[col - 1] {
            Node::Sym(c) => {
                let (mut overs, mut unders) = (vec![], vec![]);
                if under {
                    unders.push(mark)
                } else {
                    overs.push(mark)
                }
                row[col - 1] = Node::Accent {
                    overs,
                    unders,
                    base: *c,
                };
            }
            // Applying another accent stacks it outside the existing ones.
            Node::Accent { overs, unders, .. } => {
                if under {
                    unders.push(mark)
                } else {
                    overs.push(mark)
                }
            }
            // Stack onto a wide accent, the same way a compact one
            // stacks (the extra band rides outside the existing ones).
            Node::WideAccent { overs, unders, .. } => if under { unders } else { overs }.push(mark),
            _ => self.message = "accents apply to a single character".into(),
        }
    }

    // ----- selection (Shift+←/→ over sibling nodes) -----

    pub fn select_move(&mut self, right: bool) {
        if self.select_anchor.is_none() || self.select_path != self.path {
            self.select_anchor = Some(self.col);
            self.select_path = self.path.clone();
        }
        if right {
            self.col = (self.col + 1).min(self.cur_row().len());
        } else {
            self.col = self.col.saturating_sub(1);
        }
        if self.select_anchor == Some(self.col) {
            self.select_anchor = None;
        }
    }

    /// Selected node index range [lo, hi) in the current row. A stale
    /// anchor — set in a different row, or beyond the row after it
    /// shrank — yields no selection instead of a bogus range.
    pub fn selection(&self) -> Option<(usize, usize)> {
        let a = self.select_anchor?;
        if a == self.col || self.select_path != self.path {
            return None;
        }
        let (lo, hi) = (a.min(self.col), a.max(self.col));
        if hi > self.cur_row().len() {
            return None;
        }
        Some((lo, hi))
    }

    /// Remove and return the selected nodes, leaving the cursor at the gap.
    pub fn take_selection(&mut self) -> Option<Row> {
        let (lo, hi) = self.selection()?;
        self.select_anchor = None;
        self.col = lo;
        Some(self.cur_row_mut().drain(lo..hi).collect())
    }

    pub fn delete_selection(&mut self) -> bool {
        self.take_selection().is_some()
    }

    /// Copy the selection into the editor clipboard (kept on selection).
    pub fn copy_selection(&mut self) {
        match self.selection() {
            Some((lo, hi)) => {
                self.clip = Clip::Nodes(self.cur_row()[lo..hi].to_vec());
                self.message = format!("copied {} node(s)", hi - lo);
            }
            None => self.message = "nothing selected (⇧←/→ or ⇧↑)".into(),
        }
    }

    /// Cut the selection into the editor clipboard.
    pub fn cut_selection(&mut self) {
        match self.take_selection() {
            Some(content) => {
                self.message = format!("cut {} node(s)", content.len());
                self.clip = Clip::Nodes(content);
            }
            None => self.message = "nothing selected (⇧←/→ or ⇧↑)".into(),
        }
    }

    /// Paste the editor clipboard at the cursor.
    pub fn paste(&mut self) {
        if self.clip.is_empty() {
            self.message = "clipboard is empty (^C copies, ^X cuts)".into();
            return;
        }
        self.select_anchor = None;
        match self.clip.clone() {
            Clip::Nodes(clip) => {
                let col = self.col;
                let row = self.cur_row_mut();
                row.splice(col..col, clip.iter().cloned());
                self.col += clip.len();
            }
            // A cell rectangle pastes over cells in grid mode, and as
            // a bare Array node anywhere else.
            Clip::Cells { rows, cols, cells } => {
                if self.grid.is_some() && self.enclosing_array().is_some() {
                    self.grid_paste_cells(rows, cols, cells);
                } else {
                    let node = Node::Array { rows, cols, cells };
                    let col = self.col;
                    self.cur_row_mut().insert(col, node);
                    self.col += 1;
                }
            }
        }
    }

    /// Widen the selection to the enclosing structure (Shift+↑): select
    /// the parent node the cursor is in; at the top level, select the
    /// whole formula. Repeated presses climb further out.
    pub fn select_parent(&mut self) {
        if let Some((i, _)) = self.path.pop() {
            self.select_anchor = Some(i);
            self.select_path = self.path.clone();
            self.col = i + 1;
        } else if !self.cur_row().is_empty() {
            self.select_anchor = Some(0);
            self.select_path = self.path.clone();
            self.col = self.cur_row().len();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::latex::row_to_latex;

    /// Type "x^2 + \frac 1 2" the way a user would.
    #[test]
    fn typing_a_formula() {
        let mut ed = Editor::new();
        ed.insert_sym('x');
        ed.insert_and_enter(Node::Sup { arg: vec![] });
        ed.insert_sym('2');
        ed.exit_inset();
        ed.insert_sym('+');
        ed.execute("frac");
        ed.insert_sym('1');
        ed.vertical(false); // down to denominator
        ed.insert_sym('2');
        ed.exit_inset();
        assert_eq!(row_to_latex(&ed.root), "x^{2}+\\frac{1}{2}");
    }

    #[test]
    fn arrow_navigation_enters_and_leaves_structures() {
        let mut ed = Editor::new();
        ed.execute("frac");
        ed.insert_sym('a');
        // right at end of numerator -> denominator
        ed.right();
        assert_eq!(ed.path.last().unwrap().1, Field::FracDen);
        ed.insert_sym('b');
        // right at end of denominator -> after the fraction
        ed.right();
        assert!(ed.path.is_empty());
        assert_eq!(ed.col, 1);
        // left steps back into the denominator at its end
        ed.left();
        assert_eq!(ed.path.last().unwrap().1, Field::FracDen);
        assert_eq!(ed.col, 1);
    }

    #[test]
    fn delim_mids_are_pipe_only() {
        let mut ed = Editor::new();
        ed.execute("delim([]");
        assert!(
            ed.root.is_empty(),
            "\\delim([] (bracket mid) must be rejected, got {:?}",
            ed.root
        );
        // Visual order: left ( mid | right ].
        ed.execute("delim(|]");
        assert!(matches!(
            ed.root[0],
            Node::Delim {
                left: Delim::Paren,
                right: Delim::Bracket,
                mids: 1,
                ..
            }
        ));
    }

    #[test]
    fn lr_spec_reads_visually_and_accepts_names() {
        let mut ed = Editor::new();
        ed.execute("lr(]");
        assert!(matches!(
            ed.root[0],
            Node::Delim {
                left: Delim::Paren,
                right: Delim::Bracket,
                ..
            }
        ));
        // Named tokens: \lr\langle||\rangle = ⟨ · | · | · ⟩.
        let mut ed = Editor::new();
        ed.execute("lr\\langle||\\rangle");
        assert!(matches!(ed.root[0],
            Node::Delim { left: Delim::Angle, right: Delim::Angle, mids: 2, ref segs }
                if segs.len() == 3));
        // A non-spec `lr…` name falls through to the symbol table
        // (bare \lr is the ↔ arrow in the extended table).
        let mut ed = Editor::new();
        ed.execute("lr");
        assert_eq!(ed.root, vec![Node::Sym('↔')]);
        // A right-shaped glyph cannot open (and vice versa): `\lr][`
        // used to build a picture that read back as unmatched.
        assert_eq!(lr_spec("lr]["), None);
        assert_eq!(lr_spec("lr)("), None);
        // The side-symmetric | and . stand anywhere.
        assert!(lr_spec("lr|.").is_some());
    }

    #[test]
    fn angle_is_the_symbol_not_the_delimiter() {
        let mut ed = Editor::new();
        ed.execute("angle");
        assert_eq!(ed.root, vec![Node::Sym('∠')]);
    }

    #[test]
    fn op_box() {
        let type_name = |ed: &mut Editor, s: &str| {
            for c in s.chars() {
                ed.op_type(c);
            }
            ed.op_commit();
        };
        // \op opens the name box; the committed word is a bare upright
        // run (no quotes).
        let mut ed = Editor::new();
        ed.execute("op");
        assert!(ed.op_entry.is_some());
        type_name(&mut ed, "vol");
        assert_eq!(row_to_latex(&ed.root), "\\operatorname{vol}");
        // A dictionary word falls back to the Func it names.
        let mut ed = Editor::new();
        ed.execute("op");
        type_name(&mut ed, "sin");
        assert_eq!(ed.root, vec![Node::Func("sin".into())]);
        // Plain \op with spaces: words joined by ␣.
        let mut ed = Editor::new();
        ed.execute("op");
        type_name(&mut ed, "arg blah");
        assert_eq!(
            ed.root,
            vec![
                Node::Func("arg".into()),
                Node::Sym('␣'),
                Node::Func("blah".into())
            ]
        );
        // \op*: an operator band. A band name is one piece, so the
        // typed words are joined (the bare form must read back as one
        // Func); LaTeX still spaces the known ones.
        let mut ed = Editor::new();
        ed.execute("op*");
        type_name(&mut ed, "ess sup");
        assert_eq!(ed.path.last().unwrap().1, Field::OpLower);
        ed.insert_sym('x');
        ed.exit_inset();
        assert!(matches!(&ed.root[0], Node::BigOp { name, .. } if name == "esssup"));
        assert_eq!(row_to_latex(&ed.root), "\\operatorname*{esssup}_{x}");
        // Empty box commits to nothing; Esc-like cancel via backspace.
        let mut ed = Editor::new();
        ed.execute("op");
        ed.op_commit();
        assert!(ed.root.is_empty() && ed.op_entry.is_none());
        let mut ed = Editor::new();
        ed.execute("op");
        ed.op_type('a');
        ed.op_backspace();
        ed.op_backspace();
        assert!(ed.op_entry.is_none() && ed.root.is_empty());
    }

    #[test]
    fn rm_and_op_arguments_are_alphanumeric_only() {
        // Attached-argument \op styles are gone (the box took over);
        // \rm keeps its arg but rejects non-alphanumerics — Text{math}
        // with such content cannot roundtrip (the '…' quoted form only
        // reads ASCII alphanumerics).
        for cmd in ["op*)", "rm*", "opα", "opvol"] {
            let mut ed = Editor::new();
            ed.execute(cmd);
            assert!(
                ed.root.is_empty(),
                "\\{} must be rejected, got {:?}",
                cmd,
                ed.root
            );
            assert!(!ed.message.is_empty());
        }
        // \text keeps a wide charset but rejects brackets (opaque
        // quoted spans would desync the delimiter depth scans).
        let mut ed = Editor::new();
        ed.execute("text(a)");
        assert!(ed.root.is_empty(), "brackets rejected: {:?}", ed.root);
        let mut ed = Editor::new();
        ed.execute("texta+b");
        assert_eq!(row_to_latex(&ed.root), "\\text{a+b}");
    }

    #[test]
    fn argmax_makes_a_named_band() {
        let mut ed = Editor::new();
        ed.execute("argmax");
        // Cursor lands in the lower limit.
        assert_eq!(ed.path.last().unwrap().1, Field::OpLower);
        ed.insert_sym('x');
        ed.exit_inset();
        assert_eq!(row_to_latex(&ed.root), "\\operatorname*{arg\\,max}_{x}");
    }

    #[test]
    fn backspace_deletes_empty_structure() {
        let mut ed = Editor::new();
        ed.execute("sqrt");
        ed.backspace();
        assert!(ed.root.is_empty());
        assert_eq!(ed.col, 0);
    }

    #[test]
    fn jump_v2_basic_flow() {
        let mut ed = Editor::new();
        // x + 1/2 with cursor at the end of the top row.
        ed.insert_sym('x');
        ed.insert_sym('+');
        ed.execute("frac");
        ed.insert_sym('1');
        ed.vertical(false);
        ed.insert_sym('2');
        ed.exit_inset();
        let before = (ed.path.clone(), ed.col);
        ed.start_jump();
        assert!(ed.jump.is_some());
        // 'a' exists and jumping moves the cursor to a valid position.
        ed.jump_to('a');
        assert!(ed.jump.is_none());
        assert_ne!((ed.path.clone(), ed.col), before);
        assert!(ed.col <= ed.cur_row().len());
    }

    #[test]
    fn jump_skips_run_interiors_and_arrow1_neighbours() {
        let mut ed = Editor::new();
        for c in "abcde".chars() {
            ed.insert_sym(c);
        }
        ed.home();
        ed.start_jump();
        let targets = ed.jump.as_ref().unwrap();
        // Interior of the spaceless run: no marker between the atoms.
        assert!(
            targets
                .iter()
                .all(|(_, (p, c))| !(p.is_empty() && (1..5).contains(c))),
            "interior marked: {:?}",
            targets
        );
        // Nothing one arrow press from the cursor (col 0 → col 1 is
        // interior anyway; check no duplicate adjacent markers).
        for (i, (_, (p1, c1))) in targets.iter().enumerate() {
            for (_, (p2, c2)) in targets.iter().skip(i + 1) {
                assert!(
                    !(p1 == p2 && c1.abs_diff(*c2) <= 1),
                    "adjacent markers at {:?}",
                    (p1, c1, c2)
                );
            }
        }
    }

    #[test]
    fn jump_marks_ancestor_bounds() {
        let mut ed = Editor::new();
        ed.insert_sym('x');
        ed.insert_sym('+');
        ed.execute("frac");
        ed.insert_sym('1');
        // Cursor deep in the numerator: the top row's start must carry
        // a marker (class B), reachable in one label press.
        ed.start_jump();
        let targets = ed.jump.as_ref().unwrap();
        assert!(
            targets.iter().any(|(_, (p, c))| p.is_empty() && *c == 0),
            "top-row start unmarked: {:?}",
            targets
        );
    }

    #[test]
    fn jump_anchors_cover_grid_cells() {
        let mut ed = Editor::new();
        ed.execute("pmatrix");
        for c in "abcd".chars() {
            ed.insert_sym(c);
            ed.right(); // next cell (and finally out of the matrix)
        }
        ed.start_jump();
        let cell_targets = ed
            .jump
            .as_ref()
            .unwrap()
            .iter()
            .filter(|(_, (p, _))| matches!(p.last(), Some((_, Field::Cell(_)))))
            .count();
        assert!(cell_targets >= 3, "only {} cell anchors", cell_targets);
    }

    #[test]
    fn jump_prefers_empty_slots() {
        let mut ed = Editor::new();
        // A fraction with an empty denominator, cursor back at top level.
        ed.insert_sym('x');
        ed.execute("frac");
        ed.insert_sym('1');
        ed.exit_inset();
        ed.start_jump();
        // 'a' goes to the unfilled denominator slot.
        ed.jump_to('a');
        assert_eq!(ed.path.last().unwrap().1, Field::FracDen);
        assert_eq!(ed.cur_row().len(), 0);
    }

    #[test]
    fn jump_arrow_selection_moves_and_confirms() {
        let mut ed = Editor::new();
        ed.insert_sym('x');
        ed.insert_sym('+');
        ed.execute("frac");
        ed.insert_sym('1');
        ed.vertical(false);
        ed.insert_sym('2');
        ed.exit_inset();
        ed.start_jump();
        let initial = ed.jump_selected;
        // Some direction must move the selection to another marker.
        for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
            ed.jump_select(dx, dy);
            if ed.jump_selected != initial {
                break;
            }
        }
        assert_ne!(ed.jump_selected, initial, "selection did not move");
        let rank = ed.jump_selected;
        let expect = ed
            .jump
            .as_ref()
            .unwrap()
            .iter()
            .find(|(r, _)| *r == rank)
            .map(|(_, pos)| pos.clone())
            .unwrap();
        ed.jump_confirm();
        assert!(ed.jump.is_none());
        assert_eq!((ed.path.clone(), ed.col), expect);
    }

    #[test]
    fn copy_cut_paste_and_parent_selection() {
        let mut ed = Editor::new();
        for c in "ab".chars() {
            ed.insert_sym(c);
        }
        ed.execute("frac");
        ed.insert_sym('x');
        // Shift+↑ from inside the numerator selects the whole fraction.
        ed.select_parent();
        assert_eq!(ed.selection(), Some((2, 3)));
        ed.copy_selection();
        ed.paste(); // paste after the selection: a b frac frac
        assert_eq!(ed.root.len(), 4);
        assert!(matches!(ed.root[3], Node::Frac { .. }));
        // Cut one node and paste it elsewhere (= move).
        ed.select_move(false);
        ed.cut_selection();
        assert_eq!(ed.root.len(), 3);
        ed.home();
        ed.paste();
        assert!(matches!(ed.root[0], Node::Frac { .. }));
        assert_eq!(ed.col, 1);
        // Shift+↑ at top level selects everything.
        ed.select_parent();
        assert_eq!(ed.selection(), Some((0, 4)));
    }

    #[test]
    fn selection_wrap_and_delete() {
        let mut ed = Editor::new();
        for c in "abc".chars() {
            ed.insert_sym(c);
        }
        // Select b, c (cursor at end, extend left twice).
        ed.select_move(false);
        ed.select_move(false);
        assert_eq!(ed.selection(), Some((1, 3)));
        ed.execute("cancel");
        assert_eq!(
            ed.root,
            vec![
                Node::Sym('a'),
                Node::Cancel {
                    arg: vec![Node::Sym('b'), Node::Sym('c')]
                }
            ]
        );
        assert_eq!(ed.col, 2);

        // Select the cancel node and delete it.
        ed.select_move(false);
        assert!(ed.delete_selection());
        assert_eq!(ed.root, vec![Node::Sym('a')]);

        // Selection into a fraction numerator.
        ed.insert_sym('b');
        ed.select_move(false);
        ed.select_move(false);
        ed.execute("frac");
        ed.insert_sym('2');
        assert_eq!(
            ed.root,
            vec![Node::Frac {
                num: vec![Node::Sym('a'), Node::Sym('b')],
                den: vec![Node::Sym('2')],
            }]
        );
    }

    #[test]
    fn close_paren_exits_inset() {
        let mut ed = Editor::new();
        ed.insert_delim(Delim::Paren, Delim::Paren, 0);
        ed.insert_sym('x');
        ed.close_paren();
        assert!(ed.path.is_empty());
        assert_eq!(ed.col, 1);
        assert_eq!(row_to_latex(&ed.root), "\\left(x\\right)");
    }

    #[test]
    fn vertical_promotes_a_bare_big_operator() {
        // A --session restore normalizes an empty-limit band to a bare
        // atom; ↑/↓ next to it must reopen the limits.
        let mut ed = Editor::new();
        ed.root = vec![Node::Sym('∑')];
        ed.col = 1;
        ed.vertical(false); // ↓ = lower limit
        assert_eq!(ed.path.last().unwrap().1, Field::OpLower);
        ed.insert_sym('n');
        ed.exit_inset();
        assert_eq!(row_to_latex(&ed.root), "\\sum_{n}");
        // Same for limit-taking functions (┈lim┈).
        let mut ed = Editor::new();
        ed.root = vec![Node::Func("lim".into())];
        ed.col = 1;
        ed.vertical(true); // ↑ = upper limit
        assert_eq!(ed.path.last().unwrap().1, Field::OpUpper);
        // Plain atoms are not promoted.
        let mut ed = Editor::new();
        ed.root = vec![Node::Sym('x')];
        ed.col = 1;
        ed.vertical(false);
        assert!(ed.path.is_empty());
    }

    #[test]
    fn limit_functions_enter_lower() {
        let mut ed = Editor::new();
        ed.execute("lim");
        for c in "x→0".chars() {
            ed.insert_sym(c);
        }
        ed.exit_inset();
        ed.insert_sym('f');
        assert_eq!(row_to_latex(&ed.root), "\\operatorname*{lim}_{x\\to 0}f");
    }

    #[test]
    fn arrows_and_text_runs() {
        let mut ed = Editor::new();
        ed.insert_sym('A');
        ed.execute("xto");
        ed.insert_sym('f');
        ed.exit_inset();
        ed.insert_sym('B');
        ed.execute("rmdx");
        assert_eq!(
            row_to_latex(&ed.root),
            "A\\xrightarrow{f}B\\operatorname{dx}"
        );
    }

    #[test]
    fn accent_stacking() {
        let mut ed = Editor::new();
        ed.insert_sym('a');
        ed.execute("vec");
        ed.execute("hat");
        ed.execute("underline");
        assert_eq!(row_to_latex(&ed.root), "\\hat{\\vec{\\underline{a}}}");
    }

    #[test]
    fn grid_size_suffix() {
        let mut ed = Editor::new();
        ed.execute("matrix13"); // 1×3 row vector
        ed.insert_sym('a');
        assert_eq!(
            row_to_latex(&ed.root),
            "\\begin{bmatrix} a &  &  \\end{bmatrix}"
        );
        let mut ed = Editor::new();
        ed.execute("cases32");
        let Node::Delim { segs, .. } = &ed.root[0] else {
            panic!()
        };
        let [
            Node::Array {
                rows: 3, cols: 2, ..
            },
        ] = &segs[0][..]
        else {
            panic!()
        };
    }

    #[test]
    fn grid_row_col_editing() {
        let mut ed = Editor::new();
        ed.execute("matrix"); // 2x2, cursor in cell 0
        ed.insert_sym('a');
        ed.execute("addcol"); // now 2x3, cursor in new empty cell (0,1)
        ed.insert_sym('x');
        ed.execute("addrow"); // 3x3, cursor at (1,1)
        ed.insert_sym('y');
        assert_eq!(
            row_to_latex(&ed.root),
            "\\begin{bmatrix} a & x &  \\\\  & y &  \\\\  &  &  \\end{bmatrix}"
        );
        ed.execute("delcol"); // drop middle column, cursor stays in row 1
        ed.execute("delrow");
        assert_eq!(
            row_to_latex(&ed.root),
            "\\begin{bmatrix} a &  \\\\  &  \\end{bmatrix}"
        );
        // Deleting down to a single row/col is refused.
        ed.execute("delrow");
        ed.execute("delrow");
        assert_eq!(ed.message, "cannot remove the last row/column");
    }

    #[test]
    fn grid_and_mid_editing() {
        // \matrix puts the cursor into cell 0 of a [ ] grid.
        let mut ed = Editor::new();
        ed.execute("matrix");
        ed.insert_sym('a');
        assert_eq!(ed.path.len(), 2);
        ed.close_bracket();
        assert!(ed.path.is_empty());
        assert_eq!(
            row_to_latex(&ed.root),
            "\\begin{bmatrix} a &  \\\\  &  \\end{bmatrix}"
        );

        // ⟨x|y⟩ via \braket, plus \mid splitting.
        let mut ed = Editor::new();
        ed.execute("braket");
        ed.insert_sym('x');
        ed.right(); // into second segment
        ed.insert_sym('y');
        assert_eq!(
            row_to_latex(&ed.root),
            "\\left\\langle x\\middle|y\\right\\rangle "
        );
        ed.execute("mid"); // split after y -> third (empty) segment
        ed.insert_sym('z');
        assert_eq!(
            row_to_latex(&ed.root),
            "\\left\\langle x\\middle|y\\middle|z\\right\\rangle "
        );
    }
}
