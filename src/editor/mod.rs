//! Structural editing model. The cursor is a path of (node index, field)
//! pairs from the root row plus a column inside the innermost row —
//! the same model LyX uses for math insets.

use crate::ast::{Field, Node, Row, row_at, row_at_mut};
use crate::glyphs::Mark;
pub use crate::symbols::{GRID_ENVS, GridWrap};

/// A cursor position: path into nested rows plus a column.
pub type CursorPos = (Vec<(usize, Field)>, usize);
/// A block-select target: (parent row path, node index).
pub type BlockRef = (Vec<(usize, Field)>, usize);
use crate::symbols::{Accent, ColDelim, Delim, is_bigop, is_func_name, symbol_by_name};

mod command;
mod modes;

use modes::gap_shift_cell as modes_gap_shift;

pub use command::{Edit, preview_row, resolve};

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
    /// \text \tex) is open.
    pub op_entry: Option<(BoxKind, String)>,
    /// Caret inside the open box, as a char index into its content
    /// (←/→ move it; stepping past either edge commits the box).
    pub op_cursor: usize,
    /// Pending backslash escape inside the \text box (the next key is
    /// typed literally, so \" enters a quote).
    pub(crate) op_escape: bool,
    /// Grid edit mode (^T inside a matrix): cell-unit selection, with
    /// column/row lane sub-modes (see `GridSel`).
    pub grid: Option<GridSel>,
    /// Undo/redo stacks of snapshots. Pushed by `input` whenever a key
    /// changes the formula; cursor-only motion is not a history step
    /// (but the cursor is restored with the formula it belonged to).
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    pub message: String,
    /// The status line's severity: errors paint red, everything else
    /// in the ordinary message color. Set through `info`/`error`.
    pub message_error: bool,
    /// Free-cursor mode (^F): Some while active.
    pub free: Option<FreeCursor>,
    /// Block-select mode (Ctrl+B): Some(ancestors of the cursor,
    /// innermost first); each target is (parent row path, node index).
    /// ↑/→ widen the highlighted ancestor, ↓/← narrow it, Enter
    /// selects the whole block.
    pub block: Option<Vec<BlockRef>>,
    /// Index into `block` of the highlighted ancestor.
    pub block_sel: usize,
    /// Empty slots the free cursor keeps materialized (the ⬚ cells and
    /// expanded inline scripts near it stay visible until the next
    /// plain input, so approaching them does not shift the layout).
    pub ghost: Vec<Vec<(usize, Field)>>,
    /// Selection anchor column in the current row (Shift+←/→). The selected
    /// node range is between the anchor and the cursor column.
    pub(crate) select_anchor: Option<usize>,
    /// Path at the moment the anchor was set: a selection is only valid
    /// while the cursor stays in that row (leaving the row would make the
    /// anchor point into a different — possibly shorter — row).
    select_path: Vec<(usize, Field)>,
    /// The selection was placed whole (Shift+↑), so the
    /// cursor sits on an end the user did not choose. The next
    /// Shift+←/→ may flip the ends to grow on the side pressed;
    /// afterwards the ordinary shrink-to-nothing semantics resume.
    select_whole: bool,
    /// Editor-internal clipboard (^C/^X/^V): a sibling-node slice, or
    /// a rectangle of grid cells.
    clip: Clip,
    /// The open Tab-completion popup (minibuffer only). Built and
    /// navigated by the key layer; `complete` owns everything about
    /// what it contains.
    pub completion: Option<crate::complete::Completion>,
    /// The cursor path at which Backspace/^D armed the enclosing
    /// delimiter for unwrapping. While it matches the cursor, the pair
    /// is highlighted and the next press removes it; every other key
    /// disarms (the key layer takes it, like a one-shot anchor).
    pub(crate) unwrap_armed: Option<Vec<(usize, Field)>>,
}

// The display markers live in `glyphs::Mark` — the one place their
// wire chars are spelled, so both front-ends decode alike.

/// Vertical distance weight for free-cursor snapping (a row of
/// distance costs as much as this many columns).
const JUMP_W_Y: usize = 3;
/// ^F auto-expansion hysteresis: collapsed elements expand within
/// R_IN of the free cursor and fold back only beyond R_OUT (measured
/// against the element's *visible anchor* in the current display
/// frame, so the expansion itself cannot oscillate the test).
const FREE_EXPAND_IN: usize = 3;
const FREE_EXPAND_OUT: usize = 8;

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
/// Free-cursor mode (^F): a display-cell cursor moved with the arrow
/// keys; Enter snaps to the nearest editable position. Both the free
/// cell and the snap preview are shown.
pub struct FreeCursor {
    /// Current display cell (row, col).
    pub at: (usize, usize),
    /// Nearest editable position (the snap target) …
    pub snap: CursorPos,
    /// … and its display cell.
    pub snap_at: (usize, usize),
}

/// The grid's lane-gap preview, as numbers: where the ghost lane
/// splices in and how cell indices shift past it. Built by
/// `Editor::gap_splice` — the ONE place that decides whether the
/// displayed frame carries the extra lane — and consumed by both the
/// paint (`decorate_grid`) and the coordinate probes (`Frame`), so
/// they cannot disagree about the geometry.
pub(crate) struct GapSplice {
    pub k: usize,
    pub i: usize,
    pub cmode: bool,
    pub g: usize,
    pub rows: usize,
    pub cols: usize,
    pub parent: Vec<(usize, Field)>,
}

/// How the *displayed* picture's geometry differs from the raw tree:
/// the kept ghost slots and (in lane-gap state) the grid's preview
/// lane. `map` carries a raw position into the frame.
struct Frame {
    gap: Option<GapSplice>,
    ghosts: Vec<Vec<(usize, Field)>>,
}

impl Frame {
    fn map(&self, p: &[(usize, Field)], c: usize) -> (Vec<(usize, Field)>, usize) {
        let (mut p2, c2) = ghost_adjust(&self.ghosts, p, c);
        self.gap_fix(&mut p2);
        (p2, c2)
    }

    fn gap_fix(&self, p: &mut [(usize, Field)]) {
        let Some(gs) = &self.gap else { return };
        if p.len() > gs.k
            && p[..gs.k] == gs.parent[..]
            && p[gs.k].0 == gs.i
            && let Field::Cell(cell) = p[gs.k].1
        {
            p[gs.k].1 = Field::Cell(modes_gap_shift(cell, gs.cmode, gs.g, gs.rows, gs.cols));
        }
    }
}

/// Adjust a position for one ghost mark inserted at col 0 of every
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

/// A cursor position with per-position flags (the free cursor snaps
/// and auto-expands against this enumeration).
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

/// The spec's tokens, one glyph each: a spec char (`<` `>` aliasing
/// ⟨ ⟩) or a `\name`. None when a name is not a delimiter's.
fn lr_tokens(spec: &str) -> Option<Vec<char>> {
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
    Some(tokens)
}

/// A spec split at the name still being typed: everything through the
/// last complete token, and the trailing `\name` fragment — `""` when
/// the spec ends on a token boundary (`lr\` splits to a settled part
/// and an empty fragment: the backslash itself says a name is coming).
/// The split follows the tokenizer, not string search: in `\vert|`
/// the name ended at its `t` and the `|` is a token of its own, which
/// a "text after the last backslash" reading gets wrong. None when an
/// unknown name is sealed by more spec after it — nothing can finish
/// that name, so the spec is broken beyond continuing.
pub(crate) fn lr_split(spec: &str) -> Option<(&str, &str)> {
    let mut it = spec.char_indices().peekable();
    while let Some((at, c)) = it.next() {
        if c != '\\' {
            continue;
        }
        let mut name = String::new();
        while let Some(&(_, ch)) = it.peek() {
            if !ch.is_ascii_alphabetic() {
                break;
            }
            name.push(ch);
            it.next();
        }
        if crate::symbols::DELIM_NAMES.contains_key(name.as_str()) {
            continue;
        }
        return it.peek().is_none().then(|| (&spec[..at], &spec[at + 1..]));
    }
    Some((spec, ""))
}

/// Whether a spec written so far can still become one, and if so
/// whether the opening token is still to come. A spec is an opener,
/// any number of `|` middles, then a closer — so `\lr)` (a closer in
/// the opening slot) and `\lrx` (not a delimiter at all) are already
/// finished as failures, and the completion must stop offering them a
/// continuation rather than let the spelling grow forever.
pub(crate) fn lr_spec_more(spec: &str) -> Option<bool> {
    let tokens = lr_tokens(spec)?;
    let Some((first, rest)) = tokens.split_first() else {
        return Some(true);
    };
    (Delim::of_spec_side(*first, true).is_some() && rest.iter().all(|&c| c == '|')).then_some(false)
}

/// `\lr…` / `\delim…` delimiter spec, read in *visual order*: first
/// token = left, interior tokens = middles ('|' only), last token =
/// right. A token is one spec char — `( ) [ ] { } | .` with `<` `>`
/// aliasing ⟨ ⟩ — or a `\name` (\langle, \vert, \none …), so `\lr(]`,
/// `\lr{|}` and `\lr\langle||\rangle` all read like the picture. None
/// when the string is not a delimiter spec (a `\lr…` symbol name like
/// \lrcorner then resolves normally).
fn lr_spec(cmd: &str) -> Option<(Delim, Delim, usize)> {
    let spec = cmd
        .strip_prefix("delim")
        .or_else(|| cmd.strip_prefix("lr"))?;
    let tokens = lr_tokens(spec)?;
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

/// The environment a delimited grid spells, if any. `cols` decides the
/// two-column-only environments.
pub fn grid_env_name(left: Delim, right: Delim, cols: usize) -> Option<&'static str> {
    GRID_ENVS
        .entries()
        .find(|(name, w)| {
            **w == GridWrap::Pair(left, right)
                // (Null, Null) is deliberately absent: \begin{matrix} is
                // the bare Array's spelling, so a null pair keeps its
                // \left. shell and reads back as itself.
                && (cols <= 2 || !matches!(**name, "cases" | "rcases"))
        })
        .map(|(&name, _)| name)
}

/// Grid minibuffer commands with their RxC digit suffix
/// (`matrix34` = 3 rows × 4 cols). Returns the delimiter pair and the
/// dimensions.
fn grid_command(cmd: &str) -> Option<(GridWrap, usize, usize)> {
    for (name, &delims) in GRID_ENVS.entries() {
        let Some(rest) = cmd.strip_prefix(name) else {
            continue;
        };
        match rest.as_bytes() {
            // A bare name builds nothing: a grid's size is part of what
            // the command says, and every default is a guess about the
            // formula. The completion answers with the shape instead
            // (`matrix{1…9}{1…9}`), the way `\lr` answers with tokens.
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

/// What a node would give up if unwrapped, and the field its contents
/// live in: a delimiter pair around one plain segment, the ‖ ‖ norm,
/// or a radical. None for anything else — a pair with middles has no
/// single contents, and a fused grid would strand its lattice.
fn unwrap_contents(node: &Node) -> Option<(Field, &Row)> {
    match node {
        Node::Delim { mids: 0, segs, .. }
            if segs.len() == 1 && !matches!(segs[0][..], [Node::Array { .. }]) =>
        {
            Some((Field::Seg(0), &segs[0]))
        }
        Node::Norm { arg } => Some((Field::Seg(0), arg)),
        Node::Sqrt { arg, .. } => Some((Field::SqrtArg, arg)),
        _ => None,
    }
}

impl Editor {
    pub fn new() -> Self {
        Editor {
            root: Vec::new(),
            path: Vec::new(),
            col: 0,
            minibuffer: None,
            op_entry: None,
            op_cursor: 0,
            op_escape: false,
            grid: None,
            undo: Vec::new(),
            redo: Vec::new(),
            message: String::new(),
            message_error: false,
            free: None,
            block: None,
            block_sel: 0,
            ghost: Vec::new(),
            select_anchor: None,
            select_path: Vec::new(),
            select_whole: false,
            clip: Clip::Nodes(Vec::new()),
            completion: None,
            unwrap_armed: None,
        }
    }

    /// Set an informational status message.
    pub fn info(&mut self, m: impl Into<String>) {
        self.message = m.into();
        self.message_error = false;
    }

    /// Set an error status message (the display paints it red).
    pub fn error(&mut self, m: impl Into<String>) {
        self.message = m.into();
        self.message_error = true;
    }

    /// Clear the status line (and its severity).
    pub fn clear_message(&mut self) {
        self.message.clear();
        self.message_error = false;
    }

    pub fn cur_row(&self) -> &Row {
        row_at(&self.root, &self.path)
    }

    fn cur_row_mut(&mut self) -> &mut Row {
        row_at_mut(&mut self.root, &self.path)
    }

    // ----- insertion -----

    pub fn insert_sym(&mut self, c: char) {
        // Typing over an active selection replaces it.
        if self.selection().is_some() {
            self.take_selection();
        }
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
            self.insert_sym('/');
        }
    }

    /// Mouse click at canvas cell (x, y): move the cursor to the
    /// nearest position (single-render coordinate table).
    pub fn click(&mut self, x: usize, y: usize) {
        self.free = None;
        self.block = None;
        self.select_anchor = None;
        // Every transient the key layer would have cleared: a click is
        // an exit from all of them. The completion popup in particular
        // is invisible while the minibuffer is closed, so an orphaned
        // one would spring back — with its old query — the next time
        // `\` is pressed, and Enter would commit a row for a query the
        // user can no longer see.
        self.completion = None;
        self.unwrap_armed = None;
        // Clicking away from an open name box commits it, like the
        // edge-exit does — the box must not follow the cursor to the
        // clicked position.
        if self.op_entry.is_some() {
            self.op_commit();
        }
        // The minibuffer closes on a click. It used to swallow the
        // click as well whenever a preview was open, because the
        // preview opened rows the coordinate probe knew nothing about
        // — the preview floats now, so the coordinates are sound and
        // the click lands where it was aimed.
        if self.minibuffer.is_some() {
            self.minibuffer = None;
            self.clear_message();
        }
        if let Some((pos, _)) = self.nearest_position(x, y) {
            self.path = pos.0;
            self.col = pos.1;
        }
        // The click may have landed anywhere — grid mode follows the
        // cursor or ends, never keeps stale indices; a cell-rectangle
        // anchor never survives (the user did not drag this).
        if let Some(GridSel::Cells { .. }) = self.grid {
            self.grid = Some(GridSel::Cells { anchor: None });
        }
        self.reclamp_grid();
    }

    /// Nearest editable position to a display cell, with its own cell
    /// (shared by mouse clicks and the ^F snap preview). Run interiors
    /// stay reachable — density rules only apply to ^G labels. Only
    /// positions actually visible on screen participate, so the
    /// coordinates match the display exactly.
    fn nearest_position(&self, x: usize, y: usize) -> Option<(CursorPos, (usize, usize))> {
        let cands = self.jump_candidates();
        let coords = self.coords_displayed(&cands);
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

    /// The lane-gap preview's splice, when the grid is parked on a
    /// gap. The single source of the decision AND the numbers.
    pub(crate) fn gap_splice(&self) -> Option<GapSplice> {
        match self.grid {
            Some(GridSel::Lanes {
                cols: cmode, pos, ..
            }) if pos.is_multiple_of(2) => {
                self.grid_info().map(|(k, i, rows, cols, _)| GapSplice {
                    k,
                    i,
                    cmode,
                    g: pos / 2,
                    rows,
                    cols,
                    parent: self.path[..k].to_vec(),
                })
            }
            _ => None,
        }
    }

    /// Materialize the displayed frame's corrections into `root` (a
    /// clone of the raw tree) and return the map that carries raw
    /// positions into it.
    fn displayed_frame(&self, root: &mut Row) -> Frame {
        let gap = self.gap_splice();
        if let Some(gs) = &gap {
            let Node::Array {
                rows: nr,
                cols: nc,
                cells,
            } = &mut row_at_mut(root, &gs.parent)[gs.i]
            else {
                unreachable!()
            };
            (*nr, *nc) = modes::splice_lane(cells, gs.rows, gs.cols, gs.cmode, gs.g, || {
                vec![Node::Spacer]
            });
        }
        let frame = Frame {
            gap,
            ghosts: self.ghost.clone(),
        };
        // Ghosts deepest-first (an ancestor insertion would shift a
        // deeper path), each carried past the gap lane.
        let mut ghosts: Vec<&Vec<(usize, Field)>> = self.ghost.iter().collect();
        ghosts.sort_by_key(|p| std::cmp::Reverse(p.len()));
        for p in ghosts {
            let mut p = p.clone();
            frame.gap_fix(&mut p);
            row_at_mut(root, &p).insert(0, Node::Sym(Mark::SlotGhost.ch()));
        }
        frame
    }

    /// Cell coordinates of the candidates in the *displayed* frame
    /// (ghosts and the gap lane materialized; hidden candidates get
    /// None). This is the geometry the free cursor and a mouse click
    /// land in.
    fn coords_displayed(&self, cands: &[JumpCand]) -> Vec<Option<(usize, usize)>> {
        let mut root = self.root.clone();
        let frame = self.displayed_frame(&mut root);
        self.probe(root, cands, Some(&frame))
    }

    fn probe(
        &self,
        mut root: Row,
        cands: &[JumpCand],
        frame: Option<&Frame>,
    ) -> Vec<Option<(usize, usize)>> {
        use crate::render::{RenderCtx, render_root};
        let n = cands.len().min(crate::glyphs::PROBE_MAX);
        for (idx, cand) in cands.iter().take(n).enumerate().rev() {
            if frame.is_some() && !self.display_visible(&cand.pos) {
                continue;
            }
            let (p, c) = &cand.pos;
            let (p2, c2) = match frame {
                Some(f) => f.map(p, *c),
                None => (p.clone(), *c),
            };
            let mark = Mark::Probe { index: idx }.ch();
            row_at_mut(&mut root, &p2).insert(c2, Node::Sym(mark));
        }
        let b = render_root(&root, None, &RenderCtx::canonical());
        let mut out = vec![None; cands.len()];
        for &(y, x, ch) in &b.marks {
            if let Some(Mark::Probe { index: i }) = Mark::decode(ch)
                && i < out.len()
            {
                out[i] = Some((y, x));
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
                Node::Sym(c) => is_bigop(*c),
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
            let coords = self.coords_displayed(&cands);
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
            // A dormant anchor in the inset would resurrect the moment
            // a later step lands back on its row — moving out sheds it,
            // like any other plain cursor motion.
            self.select_anchor = None;
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
        // A plain motion, not an extension: keeping the anchor would
        // silently grow the selection to the whole line.
        self.select_anchor = None;
    }

    /// End of the current formula line (before the next Break).
    pub fn end(&mut self) {
        let row = self.cur_row();
        self.col = row[self.col..]
            .iter()
            .position(|n| matches!(n, Node::Break))
            .map_or(row.len(), |i| self.col + i);
        // Same anchor shedding as `home`.
        self.select_anchor = None;
    }

    /// `\!`: toggle the symbol atom left of the cursor with its
    /// slashed negation (= → ≠ → = …) when the tables have one.
    pub fn negate_prev(&mut self) {
        let Some(col) = self.col.checked_sub(1) else {
            self.error("\\! toggles the negation of the symbol left of the cursor");
            return;
        };
        match self.cur_row()[col] {
            Node::Sym(c) => {
                match crate::symbols::negated(c).or_else(|| crate::symbols::unnegated(c)) {
                    Some(flipped) => self.cur_row_mut()[col] = Node::Sym(flipped),
                    None => self.error(format!("{} has no slashed negation", c)),
                }
            }
            _ => self.error("\\! toggles the negation of the symbol left of the cursor"),
        }
    }

    // ----- deletion -----

    pub fn backspace(&mut self) {
        if self.col > 0 {
            let target = &self.cur_row()[self.col - 1];
            if !target.fields().is_empty() && !target.is_empty_structure() {
                // Non-empty structure: select it whole, so the delete
                // stays an announced two-step (the next press removes
                // it) instead of the cursor silently stepping inside.
                // Entering to edit is what the arrow keys are for.
                self.select_anchor = Some(self.col - 1);
                self.select_path = self.path.clone();
                self.select_whole = true;
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

    /// The enclosing wrapper the cursor is pressed against, if
    /// deleting toward it should offer to unwrap rather than step out:
    /// `at_start` means Backspace at the very start of the contents,
    /// otherwise ^D/Delete at the very end. A pair with middles keeps
    /// the old behaviour (there is no single "contents" to lift out),
    /// and so does a fused grid (unwrapping would strand a lattice
    /// with no delimiter to hang off). A radical arms only from the
    /// start — that is the side its root glyph is on; its far end has
    /// nothing to delete toward. Returns the node's index in its
    /// parent row.
    fn unwrappable(&self, at_start: bool) -> Option<usize> {
        let stop = if at_start { 0 } else { self.cur_row().len() };
        if self.col != stop {
            return None;
        }
        let &(i, field) = self.path.last()?;
        let parent = &self.path[..self.path.len() - 1];
        let node = row_at(&self.root, parent).get(i)?;
        let (slot, contents) = unwrap_contents(node)?;
        // An empty wrapper has nothing to lift out, and Backspace
        // already deletes it whole in one press — arming would only
        // add a keystroke.
        (field == slot && !contents.is_empty() && (at_start || !matches!(node, Node::Sqrt { .. })))
            .then_some(i)
    }

    /// Lift the enclosing wrapper's contents out of it, replacing it,
    /// and select what came out — so pressing Backspace once more
    /// deletes it, while an arrow key walks away leaving just the
    /// unwrap. Empty contents leave nothing to select.
    fn unwrap_delim(&mut self, i: usize) {
        self.path.pop();
        self.unwrap_at(i, true);
    }

    /// Replace the node at `i` in the current row with its contents.
    /// `select` leaves them selected — the staged Backspace flow wants
    /// its third press to delete them; an unwrap asked for by
    /// Shift-selecting the bracket does not, because that gesture
    /// already said all it wanted (the bracket gone, the contents
    /// kept).
    fn unwrap_at(&mut self, i: usize, select: bool) {
        let node = self.cur_row_mut().remove(i);
        let content: Row = match node {
            Node::Delim { segs, .. } => segs.into_iter().next().unwrap_or_default(),
            Node::Norm { arg } | Node::Sqrt { arg, .. } => arg,
            other => {
                self.cur_row_mut().insert(i, other);
                return;
            }
        };
        let n = content.len();
        let from_left = self.col <= i;
        self.cur_row_mut().splice(i..i, content);
        self.select_whole = false;
        if select {
            self.col = i + n;
            self.select_anchor = (n > 0).then_some(i);
            self.select_path = self.path.clone();
        } else {
            // The cursor keeps its side of what was unwrapped.
            self.col = if from_left { i } else { i + n };
            self.select_anchor = None;
        }
    }

    /// Backspace/^D pressed against an enclosing delimiter: the first
    /// press arms it (the display lights the pair up), the second
    /// unwraps. `armed` is the arming this keystroke inherited.
    /// Returns false when the cursor is not against such a pair, so the
    /// caller falls back to ordinary deletion.
    pub fn delete_toward_delim(&mut self, at_start: bool, armed: bool) -> bool {
        // Once armed, either delete key finishes the job from either
        // edge: the arming already named the wrapper, and making the
        // second press match the first key's direction would turn
        // Backspace after a ^D arming into an ordinary deletion.
        let Some(i) = self
            .unwrappable(at_start)
            .or_else(|| armed.then(|| self.unwrappable(!at_start)).flatten())
        else {
            return false;
        };
        if armed {
            self.unwrap_delim(i);
        } else {
            self.unwrap_armed = Some(self.path.clone());
        }
        true
    }

    /// Shift-selecting *onto* a wrapper arms it instead of taking it:
    /// the selection asked for "just the bracket", and a bracket's
    /// meaning is its pair — so both delimiters light up (a radical
    /// its root) and the next Backspace unwraps. A second step selects
    /// the node whole, and extending an existing selection swallows
    /// the node in one step as before. Returns true when it armed
    /// (the caller then skips the ordinary selection step).
    pub fn select_arm(&mut self, right: bool, armed: &Option<Vec<(usize, Field)>>) -> bool {
        if self.selection().is_some() {
            return false;
        }
        // At the row's edge the touch lands on the enclosing wrapper's
        // own glyph — the same place Backspace/^D against it reaches —
        // so it arms from inside too. Pressing again keeps it armed
        // (there is no node here for a second step to select).
        let edge = if right {
            self.col == self.cur_row().len()
        } else {
            self.col == 0
        };
        if edge {
            if self.unwrappable(!right).is_none() {
                return false;
            }
            self.unwrap_armed = Some(self.path.clone());
            return true;
        }
        let Some(crossed) = (if right {
            Some(self.col)
        } else {
            self.col.checked_sub(1)
        }) else {
            return false;
        };
        let Some(node) = self.cur_row().get(crossed) else {
            return false;
        };
        let Some((slot, contents)) = unwrap_contents(node) else {
            return false;
        };
        if contents.is_empty() {
            return false;
        }
        let mut p = self.path.clone();
        p.push((crossed, slot));
        if armed.as_deref() == Some(&p[..]) {
            // The second press means "the node, then": select it.
            return false;
        }
        self.unwrap_armed = Some(p);
        true
    }

    /// A wrapper armed from the outside (`select_arm`): Backspace or
    /// Delete unwraps it in place. The armed path reaches one step
    /// below the cursor's — into the slot of a node beside it.
    pub fn unwrap_armed_outside(&mut self, armed: &Option<Vec<(usize, Field)>>) -> bool {
        let Some((&(i, _), parent)) = armed.as_deref().and_then(|p| p.split_last()) else {
            return false;
        };
        if parent != &self.path[..] || (self.col != i && self.col != i + 1) {
            return false;
        }
        if self.cur_row().get(i).and_then(unwrap_contents).is_none() {
            return false;
        }
        self.unwrap_at(i, false);
        true
    }

    /// Forward delete (Delete / ^D): the selection if there is one,
    /// else an unwrap against the closing delimiter, else the node
    /// ahead of the cursor.
    pub fn delete_forward(&mut self, armed: bool) {
        if !self.delete_selection() && !self.delete_toward_delim(false, armed) {
            self.delete();
        }
    }

    pub fn delete(&mut self) {
        let row = self.cur_row();
        if self.col < row.len() {
            let target = &row[self.col];
            if !target.fields().is_empty() && !target.is_empty_structure() {
                // Mirror of backspace: select the structure whole and
                // let the next press delete it.
                self.select_anchor = Some(self.col + 1);
                self.select_path = self.path.clone();
                self.select_whole = true;
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
                // Same anchor shedding as `exit_inset`.
                self.select_anchor = None;
                return true;
            }
        }
        false
    }

    /// `)` closes the innermost ( … ) inset. A literal `)` atom is not
    /// allowed: it is indistinguishable from a closing delimiter, so a
    /// mismatched-pair scan inside any delimiter would misread it.
    pub fn close_paren(&mut self) {
        if !self.close_delim(Delim::Col(ColDelim::Paren)) {
            self.info("not inside a ( ) inset (( inserts one)");
        }
    }

    /// `]` leaves the innermost [ … ] block (matrix) or bare array.
    pub fn close_bracket(&mut self) {
        if self.close_delim(Delim::Col(ColDelim::Bracket)) {
            return;
        }
        if let Some((k, i, _)) = self.enclosing_array() {
            self.path.truncate(k);
            self.col = i + 1;
            self.select_anchor = None;
        } else {
            self.info("not inside a [ ] block ([ inserts one; \\matrix for grids)");
        }
    }

    /// `}` leaves the innermost { … } block.
    pub fn close_brace(&mut self) {
        if !self.close_delim(Delim::Col(ColDelim::Brace)) {
            self.info("not inside a { } block ({ inserts one)");
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
            self.info("not inside a matrix/array");
            return;
        };
        let parent_path = self.path[..k].to_vec();
        let Node::Array { rows, cols, cells } = &mut row_at_mut(&mut self.root, &parent_path)[i]
        else {
            unreachable!()
        };
        let Some((nr, nc, ncell)) = op(*rows, *cols, cells, c) else {
            self.error("cannot remove the last row/column");
            return;
        };
        *rows = nr;
        *cols = nc;
        self.path.truncate(k);
        self.path.push((i, Field::Cell(ncell)));
        self.col = 0;
    }

    /// Insert an empty row below the cursor's row, or a column right
    /// of its column — the `Edit::Add*` commands, expressed as the
    /// lane ops with the gap the cursor names.
    pub fn add_lane(&mut self, cols_mode: bool) {
        let Some((_, _, _, cols, c)) = self.grid_info() else {
            self.info("not inside a grid");
            return;
        };
        let after = if cols_mode { c % cols } else { c / cols };
        self.lane_insert(cols_mode, after + 1);
    }

    /// Delete the cursor's row or column (never the last one).
    pub fn del_lane(&mut self, cols_mode: bool) {
        let Some((_, _, _, cols, c)) = self.grid_info() else {
            self.info("not inside a grid");
            return;
        };
        let at = if cols_mode { c % cols } else { c / cols };
        self.lane_delete(cols_mode, at, at);
    }

    /// Insert an empty lane (column when `cols_mode`, row otherwise)
    /// at gap `g` (0..=n), parking the cursor in the new lane.
    pub fn lane_insert(&mut self, cols_mode: bool, g: usize) {
        self.edit_array(|rows, cols, cells, c| {
            let (nr, nc) = modes::splice_lane(cells, rows, cols, cols_mode, g, Vec::new);
            let at = if cols_mode {
                (c / cols) * nc + g.min(cols)
            } else {
                g.min(rows) * cols + c % cols
            };
            Some((nr, nc, at))
        });
    }

    /// Delete lanes `lo..=hi` (columns when `cols_mode`); refuses to
    /// delete every lane of the axis.
    pub fn lane_delete(&mut self, cols_mode: bool, lo: usize, hi: usize) {
        self.edit_array(|rows, cols, cells, c| {
            let n = hi - lo + 1;
            if cols_mode {
                if n >= cols || hi >= cols {
                    return None;
                }
                for r in (0..rows).rev() {
                    cells.drain(r * cols + lo..r * cols + hi + 1);
                }
                let j = c % cols;
                let j = if j > hi { j - n } else { j.min(cols - n - 1) };
                Some((rows, cols - n, (c / cols) * (cols - n) + j))
            } else {
                if n >= rows || hi >= rows {
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

    /// Clamp the grid-mode state against the current tree — the tree
    /// or cursor may have moved from under it (undo, click, jump).
    /// Indices that no longer fit are dropped rather than left to
    /// panic; outside any array the mode simply ends.
    pub(crate) fn reclamp_grid(&mut self) {
        let Some(gs) = self.grid else { return };
        let Some((_, _, rows, cols, _)) = self.grid_info() else {
            self.grid = None;
            return;
        };
        self.grid = Some(match gs {
            GridSel::Cells { anchor } => GridSel::Cells {
                anchor: anchor.filter(|&a| a < rows * cols),
            },
            GridSel::Lanes { cols: cm, pos, ext } => {
                let n = if cm { cols } else { rows };
                GridSel::Lanes {
                    cols: cm,
                    pos: pos.min(2 * n),
                    ext: ext.filter(|&e| e < n),
                }
            }
        });
    }

    /// The selected cell rectangle in grid mode: (r0, c0, r1, c1),
    /// inclusive. A cell state selects its rectangle (the current cell
    /// alone without an anchor); a lane state selects the full-axis
    /// rectangle of its lane(s), so ^C/^X work there too. None on a
    /// gap.
    pub fn grid_rect(&self) -> Option<(usize, usize, usize, usize)> {
        let (_, _, rows, cols, c) = self.grid_info()?;
        match self.grid? {
            GridSel::Cells { anchor } => {
                let a = anchor.unwrap_or(c).min(rows * cols - 1);
                let c = c.min(rows * cols - 1);
                let (r0, r1) = ((a / cols).min(c / cols), (a / cols).max(c / cols));
                let (j0, j1) = ((a % cols).min(c % cols), (a % cols).max(c % cols));
                Some((r0, j0, r1, j1))
            }
            GridSel::Lanes { cols: cm, pos, ext } if pos % 2 == 1 => {
                let n = if cm { cols } else { rows };
                let lane = (pos / 2).min(n - 1);
                let end = ext.unwrap_or(lane).min(n - 1);
                let (lo, hi) = (lane.min(end), lane.max(end));
                Some(if cm {
                    (0, lo, rows - 1, hi)
                } else {
                    (lo, 0, hi, cols - 1)
                })
            }
            _ => None,
        }
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

    /// Copy the selected cell rectangle (or lane) into the clipboard.
    pub fn grid_copy_cells(&mut self) {
        let Some((r0, j0, r1, j1)) = self.grid_rect() else {
            self.info("nothing to copy here (a gap has no cells)");
            return;
        };
        let Some((k, i, rows, cols, _)) = self.grid_info() else {
            return;
        };
        let (r1, j1) = (r1.min(rows - 1), j1.min(cols - 1));
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
        self.info(format!("copied {}×{} cell(s)", ch, cw));
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
        // A cell paste is also reachable from a plain ^V outside grid
        // mode: land on the pasted cell there, but do not switch the
        // mode on behind the user's back.
        if self.grid.is_some() {
            self.grid = Some(GridSel::Cells { anchor: None });
        }
    }

    /// `\mid`: split the current Delim segment at the cursor, inserting a
    /// │ middle; the cursor lands at the start of the new segment.
    pub fn insert_mid(&mut self) {
        // Outside a delimiter block, \mid is the divides atom ∣ (its
        // LaTeX meaning); directly inside one it splits the segment.
        let Some(&(i, Field::Seg(k))) = self.path.last() else {
            self.insert_sym('∣');
            return;
        };
        let col = self.col;
        let parent_path = self.path[..self.path.len() - 1].to_vec();
        // A Norm numbers its only field Seg(0) too, but it is not a
        // pair and has nothing to split.
        let Node::Delim { mids, segs, .. } = &mut row_at_mut(&mut self.root, &parent_path)[i]
        else {
            self.insert_sym('∣');
            return;
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
                // A wide accent's base is an inset, where a formula
                // line break cannot live (Shift+↑ can select a whole
                // top-level row, breaks included).
                let content: Row = content.into_iter().filter(|n| *n != Node::Break).collect();
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
            self.info("accent needs a base character before the cursor");
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
            Node::WideAccent { overs, unders, .. } => if under { unders } else { overs }.push(mark),
            _ => self.info("accents apply to a single character"),
        }
    }

    // ----- selection (Shift+←/→ over sibling nodes) -----

    pub fn select_move(&mut self, right: bool) {
        // A formula line break only exists at the top level, so a
        // selection may not span one: the picture would have no box to
        // paint, and wrapping it would splice a Break into an inset.
        let crossed = if right {
            Some(self.col)
        } else {
            self.col.checked_sub(1)
        };
        if crossed.is_some_and(|i| matches!(self.cur_row().get(i), Some(Node::Break))) {
            return;
        }
        if self.select_anchor.is_none() || self.select_path != self.path {
            self.select_anchor = Some(self.col);
            self.select_path = self.path.clone();
        }
        // A whole-node selection (^B, Shift+↑) leaves the cursor on an
        // end the user did not pick, so the first step toward the
        // anchor would collapse what they just selected. Flip the ends
        // once instead, and the step grows the selection on the side
        // they pressed. Any later step shrinks and collapses normally.
        if self.select_whole
            && let Some(a) = self.select_anchor
            && a != self.col
        {
            let next = if right {
                self.col + 1
            } else {
                self.col.wrapping_sub(1)
            };
            if next == a {
                self.select_anchor = Some(self.col);
                self.col = a;
            }
        }
        self.select_whole = false;
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
                self.info(format!("copied {} node(s)", hi - lo));
            }
            None => self.info("nothing selected (⇧←/→ or ⇧↑)"),
        }
    }

    /// Cut the selection into the editor clipboard.
    pub fn cut_selection(&mut self) {
        match self.take_selection() {
            Some(content) => {
                self.info(format!("cut {} node(s)", content.len()));
                self.clip = Clip::Nodes(content);
            }
            None => self.info("nothing selected (⇧←/→ or ⇧↑)"),
        }
    }

    /// Paste the editor clipboard at the cursor.
    pub fn paste(&mut self) {
        if self.clip.is_empty() {
            self.info("clipboard is empty (^C copies, ^X cuts)");
            return;
        }
        self.select_anchor = None;
        match self.clip.clone() {
            Clip::Nodes(clip) => {
                // Formula line breaks only exist at the top level.
                let clip: Row = if self.path.is_empty() {
                    clip
                } else {
                    clip.into_iter().filter(|n| *n != Node::Break).collect()
                };
                let col = self.col;
                let row = self.cur_row_mut();
                row.splice(col..col, clip.iter().cloned());
                self.col += clip.len();
            }
            // A cell rectangle pastes over cells whenever the cursor
            // is inside a grid, and as a bare Array node anywhere else.
            Clip::Cells { rows, cols, cells } => {
                if self.enclosing_array().is_some() {
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
            self.select_whole = true;
        } else if !self.cur_row().is_empty() {
            self.select_anchor = Some(0);
            self.select_path = self.path.clone();
            self.col = self.cur_row().len();
            self.select_whole = true;
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
                left: Delim::Col(ColDelim::Paren),
                right: Delim::Col(ColDelim::Bracket),
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
                left: Delim::Col(ColDelim::Paren),
                right: Delim::Col(ColDelim::Bracket),
                ..
            }
        ));
        // Named tokens: \lr\langle||\rangle = ⟨ · | · | · ⟩.
        let mut ed = Editor::new();
        ed.execute("lr\\langle||\\rangle");
        assert!(matches!(ed.root[0],
            Node::Delim { left: Delim::Angle, right: Delim::Angle, mids: 2, ref segs }
                if segs.len() == 3));
        // Bare `\lr` builds nothing: it is the prefix of a spec, not a
        // command. (It used to be a spelling of ↔, which meant the
        // first half of every spec inserted an arrow instead.)
        let mut ed = Editor::new();
        ed.execute("lr");
        assert!(ed.root.is_empty());
        assert!(ed.message.contains("usage"), "{:?}", ed.message);
        // A `lr…` name that is not a spec still falls through to the
        // symbol table rather than being read as one.
        assert_eq!(lr_spec("lrfoo"), None);
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
        // Delete the selection.
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
        ed.insert_delim(Delim::Col(ColDelim::Paren), Delim::Col(ColDelim::Paren), 0);
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
        ed.execute("bmatrix13"); // 1×3 row vector
        ed.insert_sym('a');
        assert_eq!(
            row_to_latex(&ed.root),
            "\\begin{bmatrix} a &  &  \\end{bmatrix}"
        );
        // \matrix is the bare lattice, like \array.
        let mut ed = Editor::new();
        ed.execute("matrix13");
        ed.insert_sym('a');
        assert_eq!(
            row_to_latex(&ed.root),
            "\\begin{matrix} a &  &  \\end{matrix}"
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
        ed.execute("bmatrix22"); // 2x2, cursor in cell 0
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
        ed.execute("bmatrix22");
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
