//! Structural editing model. The cursor is a path of (node index, field)
//! pairs from the root row plus a column inside the innermost row —
//! the same model LyX uses for math insets.

use crate::ast::{Field, Node, Row, row_at, row_at_mut};

/// A cursor position: path into nested rows plus a column.
pub type CursorPos = (Vec<(usize, Field)>, usize);
/// A block-select target: (parent row path, node index).
pub type BlockRef = (Vec<(usize, Field)>, usize);
use crate::symbols::{accent_by_name, bigop_by_char, bigop_by_name, is_func_name, symbol_by_name};

/// One undo step: the formula with the cursor that belonged to it.
type Snapshot = (Row, Vec<(usize, Field)>, usize);

pub struct Editor {
    pub root: Row,
    pub path: Vec<(usize, Field)>,
    pub col: usize,
    /// Some(text) while the `\command` minibuffer is open.
    pub minibuffer: Option<String>,
    /// Some((kind, content)) while an in-place name box (\op \op* \rm
    /// \text) is open.
    pub op_entry: Option<(BoxKind, String)>,
    /// Grid edit mode (^O inside a matrix): arrows move cells, r/R c/C
    /// add rows/columns, d/D delete them.
    pub grid_mode: bool,
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
    /// Block-select mode (Ctrl+B): Some(targets) while waiting for a
    /// label key; each target is (parent row path, node index) of a
    /// structure node, and picking one selects the whole block.
    pub block: Option<Vec<BlockRef>>,
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
    /// Editor-internal clipboard (^C/^X/^V), a sibling-node slice.
    clip: Row,
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
/// Argument row for a \^… / \_… script command: a symbol name
/// (\^gamma) or a run of ASCII alphanumerics (\^z, \_10).
fn script_cmd_arg(rest: &str) -> Option<crate::ast::Row> {
    if rest.is_empty() {
        return None;
    }
    if let Some(c) = symbol_by_name(rest) {
        return Some(vec![Node::Sym(c)]);
    }
    rest.chars()
        .all(|c| c.is_ascii_alphanumeric())
        .then(|| rest.chars().map(Node::Sym).collect())
}

fn lr_spec(cmd: &str) -> Option<(char, char, Vec<char>)> {
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
                match name.as_str() {
                    "langle" => '⟨',
                    "rangle" => '⟩',
                    "lparen" => '(',
                    "rparen" => ')',
                    "lbrack" => '[',
                    "rbrack" => ']',
                    "lbrace" => '{',
                    "rbrace" => '}',
                    "vert" | "mid" => '|',
                    "dot" | "none" => '.',
                    _ => return None,
                }
            }
            '<' => '⟨',
            '>' => '⟩',
            c => c,
        };
        tokens.push(tok);
    }
    if tokens.len() < 2 || !tokens.iter().all(|c| crate::ast::DELIM_SPECS.contains(c)) {
        return None;
    }
    let (left, right) = (tokens[0], *tokens.last().unwrap());
    let mids = tokens[1..tokens.len() - 1].to_vec();
    if !mids.iter().all(|&c| c == '|') {
        return None;
    }
    Some((left, right, mids))
}

/// Delimiter pair of a grid command (None = bare lattice array).
type GridDelims = Option<(char, char)>;

/// Grid minibuffer commands with an optional RxC digit suffix
/// (`matrix34` = 3 rows × 4 cols; bare name = 2×2). Returns the delimiter
/// pair and the dimensions.
fn grid_command(cmd: &str) -> Option<(GridDelims, usize, usize)> {
    const GRIDS: &[(&str, GridDelims)] = &[
        ("matrix", Some(('[', ']'))),
        ("bmatrix", Some(('[', ']'))),
        ("pmatrix", Some(('(', ')'))),
        ("Bmatrix", Some(('{', '}'))),
        ("vmatrix", Some(('|', '|'))),
        ("cases", Some(('{', '.'))),
        ("rcases", Some(('.', '}'))),
        ("array", None),
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
            grid_mode: false,
            undo: Vec::new(),
            redo: Vec::new(),
            message: String::new(),
            italic: true,
            jump: None,
            jump_selected: 0,
            free: None,
            block: None,
            ghost: Vec::new(),
            select_anchor: None,
            select_path: Vec::new(),
            clip: Vec::new(),
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

    /// Formatting space (Space key): blank column in the AA, nothing in
    /// LaTeX/Typst, vanishes on reparse.
    pub fn insert_spacer(&mut self) {
        let col = self.col;
        self.cur_row_mut().insert(col, Node::Spacer);
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

    // ----- free-cursor mode (^F) -----

    pub fn start_free(&mut self) {
        self.select_anchor = None;
        let start = self
            .nearest_position_of_cursor()
            .unwrap_or(((self.path.clone(), self.col), (0, 0)));
        self.free = Some(FreeCursor {
            at: start.1,
            snap: start.0,
            snap_at: start.1,
        });
        self.message.clear();
    }

    /// The cursor's own display cell (in the display frame).
    fn nearest_position_of_cursor(&self) -> Option<(CursorPos, (usize, usize))> {
        let cands = self.jump_candidates();
        let coords = self.display_coords(&cands);
        let i = cands.iter().position(|c| c.is_cursor)?;
        coords[i].map(|xy| (cands[i].pos.clone(), xy))
    }

    pub fn free_move(&mut self, dx: i32, dy: i32) {
        use crate::render::{RenderCtx, render_root};
        let Some(f) = &self.free else {
            return;
        };
        // (Clamping happens after the re-anchor step, against the
        // ghost-materialized display frame.)
        let at = (
            f.at.0.saturating_add_signed(dy as isize),
            f.at.1.saturating_add_signed(dx as isize),
        );
        // Collapsed elements (inline scripts, invisible slots) expand
        // automatically while the free cursor is near them. Proximity is
        // measured in the *current display frame* against the element's
        // always-visible anchor (the position before its node in the
        // parent row), with hysteresis (R_IN/R_OUT) so the expansion
        // shift cannot make the test oscillate.
        let cands = self.jump_candidates();
        let disp = self.display_coords(&cands);
        let coord_of = |pos: &CursorPos| {
            cands
                .iter()
                .position(|c| &c.pos == pos)
                .and_then(|i| disp[i])
        };
        let mut ghosts: Vec<Vec<(usize, Field)>> = Vec::new();
        for cand in &cands {
            let expandable = matches!(
                cand.pos.0.last(),
                Some((
                    _,
                    Field::SupArg
                        | Field::SubArg
                        | Field::OpLower
                        | Field::OpUpper
                        | Field::ArrowOver
                        | Field::ArrowUnder
                        | Field::BraceLabel
                ))
            );
            if !expandable || ghosts.contains(&cand.pos.0) {
                continue;
            }
            let row_path = &cand.pos.0;
            let anchor = (
                row_path[..row_path.len() - 1].to_vec(),
                row_path[row_path.len() - 1].0,
            );
            let Some((ay, ax)) = coord_of(&anchor) else {
                continue;
            };
            let d = JUMP_W_Y * ay.abs_diff(at.0) + ax.abs_diff(at.1);
            let was = self.ghost.contains(row_path);
            if d <= FREE_EXPAND_IN || (was && d <= FREE_EXPAND_OUT) {
                ghosts.push(row_path.clone());
            }
        }
        self.ghost = ghosts;
        // Expansion can shift the whole canvas (a band adds a row above
        // the baseline). Re-anchor the free cursor by however much the
        // previous snap target moved between the two frames, so the
        // cell cursor stays glued to the content instead of floating.
        let mut at = at;
        if let Some(prev) = &self.free {
            let cands2 = self.jump_candidates();
            let disp2 = self.display_coords(&cands2);
            if let Some((ny, nx)) = cands2
                .iter()
                .position(|c| c.pos == prev.snap)
                .and_then(|i| disp2[i])
            {
                at.0 =
                    at.0.saturating_add_signed(ny as isize - prev.snap_at.0 as isize);
                at.1 =
                    at.1.saturating_add_signed(nx as isize - prev.snap_at.1 as isize);
            }
        }
        // Clamp to the display frame (ghosts included).
        let (droot, _) = self.decorated();
        let b = render_root(&droot, None, &RenderCtx::canonical());
        let (h, w) = (b.height().max(1), b.width().max(1));
        at.0 = at.0.min(h - 1);
        at.1 = at.1.min(w);
        if let Some((snap, snap_at)) = self.nearest_position(at.1, at.0) {
            if let Some(f) = &mut self.free {
                f.at = at;
                f.snap = snap;
                f.snap_at = snap_at;
            }
        } else if let Some(f) = &mut self.free {
            f.at = at;
        }
    }

    /// Enter: land on the snap target.
    pub fn free_confirm(&mut self) {
        if let Some(t) = self.jump.take() {
            self.keep_ghosts(&t);
        }
        if let Some(f) = self.free.take() {
            self.path = f.snap.0;
            self.col = f.snap.1;
            self.message.clear();
        }
    }

    pub fn free_cancel(&mut self) {
        self.free = None;
        self.jump = None;
        self.select_anchor = None;
        self.ghost.clear();
        self.message.clear();
    }

    /// ^G in free mode: toggle the jump markers.
    pub fn free_toggle_markers(&mut self) {
        if self.jump.is_some() {
            self.free_markers_off();
        } else {
            self.start_jump();
        }
    }

    /// Drop the markers (Esc / toggle), keeping ghosts and the free
    /// cursor anchored to the stable layout.
    pub fn free_markers_off(&mut self) {
        if let Some(t) = self.jump.take() {
            self.keep_ghosts(&t);
        }
        self.free_reanchor();
        self.message.clear();
    }

    /// A jump label pressed while markers are up: move there and drop
    /// back to plain free motion.
    pub fn free_jump(&mut self, label: char) {
        let Some(idx) = JUMP_LABELS.chars().position(|c| c == label) else {
            return;
        };
        self.free_goto_rank(idx);
    }

    /// Enter while markers are up: land on the arrow-selected marker,
    /// back to free motion from there.
    pub fn free_goto_selected(&mut self) {
        self.free_goto_rank(self.jump_selected);
    }

    fn free_goto_rank(&mut self, rank: usize) {
        let Some(targets) = &self.jump else { return };
        let Some((_, (p, c))) = targets.iter().find(|(r, _)| *r == rank) else {
            return;
        };
        let (p, c) = (p.clone(), *c);
        if let Some(t) = self.jump.take() {
            self.keep_ghosts(&t);
        }
        self.path = p;
        self.col = c;
        self.free_reanchor();
        self.message.clear();
    }

    /// Re-anchor the free cursor onto the cursor's current display cell.
    fn free_reanchor(&mut self) {
        if self.free.is_none() {
            return;
        }
        if let Some((pos, xy)) = self.nearest_position_of_cursor()
            && let Some(f) = &mut self.free
        {
            f.at = xy;
            f.snap = pos;
            f.snap_at = xy;
        }
    }

    /// Ctrl+E: jump to the very end of the formula.
    pub fn document_end(&mut self) {
        self.select_anchor = None;
        self.path.clear();
        self.col = self.root.len();
    }

    /// Ctrl+A: jump to the very start of the whole formula.
    pub fn document_start(&mut self) {
        self.select_anchor = None;
        self.path.clear();
        self.col = 0;
    }

    /// Enter at the top level: start a new formula line (Node::Break).
    pub fn break_line(&mut self) {
        self.select_anchor = None;
        let col = self.col;
        self.cur_row_mut().insert(col, Node::Break);
        self.col += 1;
    }

    /// Insert a structure node and move the cursor into its first field.
    pub fn insert_and_enter(&mut self, node: Node) {
        let first = node.fields()[0];
        let col = self.col;
        self.cur_row_mut().insert(col, node);
        self.path.push((col, first));
        self.col = 0;
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
                let base = vec![row[col].clone()];
                row[col] = Node::BigOp {
                    base,
                    lower: vec![],
                    upper: vec![],
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
    pub fn insert_delim(&mut self, left: char, right: char, mids: Vec<char>) {
        let segs = vec![vec![]; mids.len() + 1];
        self.insert_and_enter(Node::Delim {
            left,
            right,
            mids,
            segs,
        });
    }

    /// Close (step out of) the innermost enclosing Delim whose right
    /// delimiter is `right`. Returns false when there is none.
    pub fn close_delim(&mut self, right: char) -> bool {
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
        if !self.close_delim(')') {
            self.message = "not inside a ( ) inset (( inserts one)".into();
        }
    }

    /// `]` leaves the innermost [ … ] block (matrix) or bare array.
    pub fn close_bracket(&mut self) {
        if self.close_delim(']') {
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
        if !self.close_delim('}') {
            self.message = "not inside a { } block ({ inserts one)".into();
        }
    }

    /// Insert a rows×cols grid wrapped in the given delimiter pair and put
    /// the cursor into the first cell.
    pub fn insert_grid(&mut self, left: char, right: char, rows: usize, cols: usize) {
        let array = Node::Array {
            rows,
            cols,
            cells: vec![vec![]; rows * cols],
        };
        let node = Node::Delim {
            left,
            right,
            mids: vec![],
            segs: vec![vec![array]],
        };
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

    /// Insert an empty row above the cursor's row (grid mode `R`).
    pub fn add_row_above(&mut self) {
        self.edit_array(|rows, cols, cells, c| {
            let r = c / cols;
            for j in 0..cols {
                cells.insert(r * cols + j, vec![]);
            }
            Some((rows + 1, cols, r * cols + c % cols))
        });
    }

    /// Insert an empty column left of the cursor's column (grid mode `C`).
    pub fn add_col_left(&mut self) {
        self.edit_array(|rows, cols, cells, c| {
            let j = c % cols;
            for r in (0..rows).rev() {
                cells.insert(r * cols + j, vec![]);
            }
            Some((rows, cols + 1, (c / cols) * (cols + 1) + j))
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
        mids.insert(k, '|');
        *self.path.last_mut().unwrap() = (i, Field::Seg(k + 1));
        self.col = 0;
    }

    /// Wrap the atom just before the cursor with an accent mark.
    fn apply_accent(&mut self, mark: char) {
        // A selection becomes the base of a stretchy accent (\widehat
        // over anything — the accent band rides above/below the block).
        if self.selection().is_some() {
            if let Some(content) = self.take_selection() {
                let under = crate::symbols::is_under_mark(mark);
                let node = Node::WideAccent {
                    over: (!under).then_some(mark),
                    under: under.then_some(mark),
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
        let under = crate::symbols::is_under_mark(mark);
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
            // Fill the free side of a wide accent (¯ then ‗, say).
            Node::WideAccent { over, under: u, .. } => {
                let slot = if under { u } else { over };
                if slot.is_none() {
                    *slot = Some(mark);
                } else {
                    self.message = "that side of the wide accent is taken".into();
                }
            }
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
                self.clip = self.cur_row()[lo..hi].to_vec();
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
                self.clip = content;
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
        let clip = self.clip.clone();
        let col = self.col;
        let row = self.cur_row_mut();
        row.splice(col..col, clip.iter().cloned());
        self.col += clip.len();
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

    /// Replace the selection with `build(selection)`, cursor after the node.
    pub fn wrap_selection(&mut self, build: impl FnOnce(Row) -> Node) -> bool {
        match self.take_selection() {
            Some(content) => {
                let node = build(content);
                let col = self.col;
                self.cur_row_mut().insert(col, node);
                self.col += 1;
                true
            }
            None => false,
        }
    }

    // ----- jump mode (EasyMotion-style) -----

    /// Every cursor position in true document order (column, then that
    /// node's children, then the next column), with the flags the jump
    /// selection needs. Document order is required by the coordinate
    /// probe (marker insertion invalidates later positions otherwise).
    pub fn jump_candidates(&self) -> Vec<JumpCand> {
        fn flat_atom(n: &Node) -> bool {
            matches!(n, Node::Sym(c) if *c != '␣')
        }
        fn walk(
            row: &Row,
            path: &mut Vec<(usize, Field)>,
            in_cell: bool,
            cursor: &(&[(usize, Field)], usize),
            out: &mut Vec<JumpCand>,
        ) {
            fn is_cur(
                cursor: &(&[(usize, Field)], usize),
                path: &[(usize, Field)],
                col: usize,
            ) -> bool {
                cursor.0 == path && cursor.1 == col
            }
            if row.is_empty() {
                out.push(JumpCand {
                    pos: (path.clone(), 0),
                    empty: true,
                    cell_end: in_cell,
                    interior: false,
                    bound: true,
                    is_cursor: is_cur(cursor, path, 0),
                });
                return;
            }
            for (i, node) in row.iter().enumerate() {
                out.push(JumpCand {
                    pos: (path.clone(), i),
                    empty: false,
                    cell_end: false,
                    interior: i > 0 && flat_atom(&row[i - 1]) && flat_atom(node),
                    bound: i == 0,
                    is_cursor: is_cur(cursor, path, i),
                });
                for f in node.fields() {
                    path.push((i, f));
                    walk(
                        node.field(f),
                        path,
                        matches!(f, Field::Cell(_)),
                        cursor,
                        out,
                    );
                    path.pop();
                }
            }
            out.push(JumpCand {
                pos: (path.clone(), row.len()),
                empty: false,
                cell_end: in_cell,
                interior: false,
                bound: true,
                is_cursor: is_cur(cursor, path, row.len()),
            });
        }
        let mut out = Vec::new();
        walk(
            &self.root,
            &mut Vec::new(),
            false,
            &(&self.path[..], self.col),
            &mut out,
        );
        out
    }

    /// Physical cell coordinates of the given document-ordered
    /// positions: one render with every position carrying a zero-width
    /// probe mark — the geometry (and thus every coordinate) is
    /// identical to the displayed render.
    fn position_coords(&self, positions: &[&CursorPos]) -> Vec<Option<(usize, usize)>> {
        use crate::render::{RenderCtx, render_root};
        let n = positions.len().min(0x800);
        let mut root = self.root.clone();
        for (idx, (p, c)) in positions.iter().take(n).enumerate().rev() {
            let mark = char::from_u32(PROBE_BASE + idx as u32).unwrap();
            row_at_mut(&mut root, p).insert(*c, Node::Sym(mark));
        }
        let b = render_root(&root, None, &RenderCtx::canonical());
        let mut out = vec![None; positions.len()];
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

    pub fn start_jump(&mut self) {
        use std::cmp::Reverse;
        let cands = self.jump_candidates();
        let positions: Vec<&CursorPos> = cands.iter().map(|c| &c.pos).collect();
        let coords = self.position_coords(&positions);
        let Some(cur_i) = cands.iter().position(|c| c.is_cursor) else {
            self.message = "no jump targets".into();
            return;
        };
        let Some((cy, cx)) = coords[cur_i] else {
            self.message = "no jump targets".into();
            return;
        };
        let dist2 =
            |a: (usize, usize), b: (usize, usize)| JUMP_W_Y * a.0.abs_diff(b.0) + a.1.abs_diff(b.1);
        let dist = |i: usize| coords[i].map(|xy| dist2(xy, (cy, cx)));

        // Hard filter (spec §2): the cursor itself, run interiors, and
        // anything the probe could not place.
        let usable: Vec<usize> = (0..cands.len())
            .filter(|&i| !cands[i].is_cursor && !cands[i].interior && coords[i].is_some())
            .collect();

        // Classes (spec §3). B: bounds of every row enclosing the cursor.
        let ancestor_bound = |i: usize| {
            let (p, _) = &cands[i].pos;
            cands[i].bound && p.len() <= self.path.len() && self.path[..p.len()] == p[..]
        };
        let cursor_grid = {
            let grid_key = |p: &[(usize, Field)]| -> Option<(usize, Vec<(usize, Field)>)> {
                let k = p.iter().rposition(|(_, f)| matches!(f, Field::Cell(_)))?;
                Some((p[k].0, p[..k].to_vec()))
            };
            grid_key(&self.path)
        };
        let same_grid = |i: usize| {
            let k = cands[i]
                .pos
                .0
                .iter()
                .rposition(|(_, f)| matches!(f, Field::Cell(_)));
            match (k, &cursor_grid) {
                (Some(k), Some((gi, gp))) => {
                    cands[i].pos.0[k].0 == *gi && cands[i].pos.0[..k] == gp[..]
                }
                _ => false,
            }
        };
        let class = |i: usize| {
            if cands[i].empty {
                0
            } else if ancestor_bound(i) {
                1
            } else if cands[i].cell_end {
                2
            } else {
                3
            }
        };

        // One arrow press away (spec §4.1): same row, adjacent column.
        let arrow1 = |a: &CursorPos, b: &CursorPos| a.0 == b.0 && a.1.abs_diff(b.1) <= 1;

        // Priority classes first (by distance), then the general pool.
        let mut order = usable.clone();
        order.sort_by_key(|&i| (class(i), dist(i).unwrap_or(usize::MAX), !same_grid(i)));
        let mut chosen: Vec<usize> = Vec::new();
        let cursor_pos = (self.path.clone(), self.col);
        for &i in &order {
            if chosen.len() >= JUMP_MAX_RANKS {
                break;
            }
            let pos = &cands[i].pos;
            let xy = coords[i].unwrap();
            if arrow1(pos, &cursor_pos) {
                continue;
            }
            if chosen.iter().any(|&j| arrow1(pos, &cands[j].pos)) {
                continue;
            }
            if class(i) == 3 {
                // Density control (spec §4.2) + materialization cost.
                let mut d = dist(i).unwrap();
                if matches!(pos.0.last(), Some((_, Field::SupArg | Field::SubArg))) {
                    d += JUMP_C_GHOST;
                }
                let radius = JUMP_R_MIN.max(d / JUMP_ALPHA_DIV);
                let near = chosen
                    .iter()
                    .filter_map(|&j| coords[j])
                    .chain([(cy, cx)])
                    .map(|xy2| dist2(xy, xy2))
                    .min()
                    .unwrap_or(usize::MAX);
                if near < radius {
                    continue;
                }
            }
            chosen.push(i);
        }
        if chosen.is_empty() {
            self.message = "no jump targets".into();
            return;
        }
        // Ranks = selection order; markers need document order.
        let mut picked: Vec<(usize, CursorPos)> = chosen
            .iter()
            .enumerate()
            .map(|(rank, &i)| (rank, cands[i].pos.clone()))
            .collect();
        picked.sort_by_key(|&(rank, _)| chosen[rank]);
        let _ = Reverse(0); // (kept for potential ordering tweaks)
        self.message = "jump: label key / arrows + Enter (Esc cancels)".into();
        self.jump = Some(picked);
        self.jump_selected = 0;
    }

    /// Move the arrow-key selection to the nearest marker in the given
    /// direction (dx, dy ∈ {-1, 0, 1}).
    pub fn jump_select(&mut self, dx: i32, dy: i32) {
        let coords = self.jump_marker_coords();
        let Some(&(_, cur)) = coords.iter().find(|(r, _)| *r == self.jump_selected) else {
            return;
        };
        let best = coords
            .iter()
            .filter(|(r, _)| *r != self.jump_selected)
            .filter(|(_, xy)| {
                let (gx, gy) = (xy.1 as i64 - cur.1 as i64, xy.0 as i64 - cur.0 as i64);
                (dx != 0 && gx.signum() == dx as i64 || dy != 0 && gy.signum() == dy as i64)
                    && (dx != 0 || gx.abs() <= gy.abs() * 2)
                    && (dy != 0 || gy.abs() <= gx.abs() * 2)
            })
            .min_by_key(|(_, xy)| JUMP_W_Y * xy.0.abs_diff(cur.0) + xy.1.abs_diff(cur.1));
        if let Some(&(r, _)) = best {
            self.jump_selected = r;
        }
    }

    /// Enter: jump to the arrow-selected marker.
    pub fn jump_confirm(&mut self) {
        let rank = self.jump_selected;
        if let Some(targets) = self.jump.take() {
            self.keep_ghosts(&targets);
            if let Some((_, (p, c))) = targets.iter().find(|(r, _)| *r == rank) {
                self.path = p.clone();
                self.col = *c;
            }
            self.message.clear();
        }
    }

    /// Live cell coordinates of the current jump markers, decoded from
    /// the decorated render (rank chars carry their identity).
    fn jump_marker_coords(&self) -> Vec<(usize, (usize, usize))> {
        use crate::render::{RenderCtx, render_root};
        let (root, cursor) = self.decorated();
        let cur = cursor.as_ref().map(|(p, c)| (p.as_slice(), *c));
        let b = render_root(&root, cur, &RenderCtx::canonical());
        b.marks
            .iter()
            .filter_map(|&(y, x, ch)| {
                let u = ch as u32;
                (JUMP_RANK_BASE..JUMP_RANK_BASE + JUMP_MAX_RANKS as u32)
                    .contains(&u)
                    .then(|| ((u - JUMP_RANK_BASE) as usize, (y, x)))
            })
            .collect()
    }

    /// Remember which labeled rows change their rendering while marked:
    /// empty slots (materialized as ⬚) and inline-script args (expanded
    /// to 2D). They keep that form until the next non-^G input, so
    /// re-entering ^G never shifts the layout.
    pub fn keep_ghosts(&mut self, targets: &[(usize, CursorPos)]) {
        self.ghost.clear();
        for (_, (p, _)) in targets {
            let keep = row_at(&self.root, p).is_empty()
                || matches!(p.last(), Some((_, Field::SupArg | Field::SubArg)));
            if keep && !self.ghost.contains(p) {
                self.ghost.push(p.clone());
            }
        }
    }

    pub fn jump_to(&mut self, label: char) {
        if let Some(targets) = self.jump.take() {
            self.keep_ghosts(&targets);
            if let Some(idx) = JUMP_LABELS.chars().position(|c| c == label)
                && let Some((_, (p, c))) = targets.iter().find(|(rank, _)| *rank == idx)
            {
                self.path = p.clone();
                self.col = *c;
            }
            self.message.clear();
        }
    }

    // ----- block-select mode (Ctrl+B: labels on structure blocks) -----

    /// All structure nodes (anything with cursor fields) in document
    /// order: (parent row path, node index).
    pub fn block_targets(&self) -> Vec<BlockRef> {
        fn walk(row: &Row, path: &mut Vec<(usize, Field)>, out: &mut Vec<BlockRef>) {
            // A grid that is a delimiter segment's sole node fuses with
            // the delimiter — label atoms inside the segment would break
            // that shape, and the enclosing Delim target covers it.
            let fused_array = row.len() == 1
                && matches!(row[0], Node::Array { .. })
                && matches!(path.last(), Some((_, Field::Seg(_))));
            for (i, node) in row.iter().enumerate() {
                if !node.fields().is_empty() && !fused_array {
                    out.push((path.clone(), i));
                }
                for f in node.fields() {
                    path.push((i, f));
                    walk(node.field(f), path, out);
                    path.pop();
                }
            }
        }
        let mut out = Vec::new();
        walk(&self.root, &mut Vec::new(), &mut out);
        out
    }

    pub fn start_block_select(&mut self) {
        let mut targets = self.block_targets();
        if targets.is_empty() {
            self.message = "no blocks to select".into();
            return;
        }
        let max = JUMP_LABELS.chars().count();
        if targets.len() > max {
            targets.truncate(max);
            self.message = "block: press a label key (some blocks unlabeled)".into();
        } else {
            self.message = "block: press a label key (Esc cancels)".into();
        }
        self.block = Some(targets);
    }

    /// Pick a labeled block: the whole node becomes the selection, ready
    /// for ^C/^X, wrapping or deletion.
    pub fn block_to(&mut self, label: char) {
        if let Some(targets) = self.block.take() {
            if let Some(idx) = JUMP_LABELS.chars().position(|c| c == label)
                && let Some((p, i)) = targets.get(idx)
            {
                self.path = p.clone();
                self.select_anchor = Some(*i);
                self.select_path = p.clone();
                self.col = i + 1;
            }
            self.message.clear();
        }
    }

    /// Vertical extents (rows above / below the marker's baseline row)
    /// of the boxes the display should paint: one entry for the active
    /// selection, or one per ^B target in ascending-open-column
    /// (= document) order. Computed by laying out the covered slice —
    /// the char grid alone cannot tell a block's rows apart from other
    /// content (e.g. a denominator centered under the same columns).
    pub fn marker_extents(&self) -> Vec<(usize, usize, usize)> {
        use crate::render::{RenderCtx, render_root};
        let extent =
            |slice: &[Node], cursor: Option<(Vec<(usize, Field)>, usize)>, depth: usize| {
                let cur = cursor.as_ref().map(|(p, c)| (p.as_slice(), *c));
                let b = render_root(&slice.to_vec(), cur, &RenderCtx::canonical());
                let (h, bl) = (b.height(), b.baseline);
                (bl.min(h), h.saturating_sub(bl + 1), depth)
            };
        if let Some(targets) = &self.block {
            targets
                .iter()
                .map(|(p, i)| {
                    // If the cursor is inside this block, lay the slice
                    // out in its editing view (matches the display).
                    let cur = (self.path.len() > p.len()
                        && self.path[..p.len()] == p[..]
                        && self.path[p.len()].0 == *i)
                        .then(|| {
                            let mut rel = self.path[p.len()..].to_vec();
                            rel[0].0 = 0;
                            (rel, self.col)
                        });
                    extent(&row_at(&self.root, p)[*i..*i + 1], cur, p.len())
                })
                .collect()
        } else if let Some((lo, hi)) = self.selection() {
            vec![extent(&self.cur_row()[lo..hi], None, 0)]
        } else {
            Vec::new()
        }
    }

    // ----- display decoration (jump labels / selection) -----

    /// A copy of the AST with display markers inserted, plus the cursor
    /// adjusted for the insertions (None while jump mode hides it).
    /// Markers are private-use `Sym`s that the TUI turns into colored
    /// labels / brackets; they never appear in a real document.
    pub fn decorated(&self) -> (Row, Option<CursorPos>) {
        let mut root = self.root.clone();
        // Nudge the cursor position past a marker inserted at slot `k`
        // of the row at `at`, so the cursor can stay threaded through
        // mode displays (keeping the editing-view geometry: ⬚ limit
        // slots, unfused matrices, expanded inline scripts).
        fn bump(path: &mut [(usize, Field)], col: &mut usize, at: &[(usize, Field)], k: usize) {
            let d = at.len();
            if path.len() >= d && path[..d] == *at {
                if path.len() > d {
                    if path[d].0 >= k {
                        path[d].0 += 1;
                    }
                } else if *col > k {
                    *col += 1;
                }
            }
        }
        if let Some(targets) = &self.jump {
            let mut path = self.path.clone();
            let mut col = self.col;
            // A live selection is tracked through the marker insertions
            // like the cursor, then painted with the same SEL markers
            // as the normal branch.
            let mut sel = self
                .selection()
                .map(|(lo, hi)| ((self.path.clone(), lo), (self.path.clone(), hi)));
            // Reverse document order keeps not-yet-inserted positions valid.
            for (rank, (p, c)) in targets.iter().rev() {
                let mark = char::from_u32(JUMP_RANK_BASE + *rank as u32).unwrap();
                row_at_mut(&mut root, p).insert(*c, Node::Sym(mark));
                bump(&mut path, &mut col, p, *c);
                if let Some(((lp, lc), (hp, hc))) = &mut sel {
                    bump(lp, lc, p, *c);
                    bump(hp, hc, p, *c);
                }
            }
            if let Some(((lp, lo), (_, hi))) = sel {
                let row = row_at_mut(&mut root, &lp);
                row.insert(hi, Node::Sym(SEL_CLOSE));
                row.insert(lo, Node::Sym(SEL_OPEN));
                // The cursor may sit inside this row (or deeper): thread
                // it through both insertions like any other marker.
                bump(&mut path, &mut col, &lp, hi);
                bump(&mut path, &mut col, &lp, lo);
            }
            return (root, Some((path, col)));
        }
        if let Some(targets) = &self.block {
            let mut path = self.path.clone();
            let mut col = self.col;
            // Label sits immediately left of its block and a close marker
            // right after it, so the display can paint the block's extent
            // (reverse document order keeps positions valid).
            for (idx, (p, i)) in targets.iter().enumerate().rev() {
                let mark = char::from_u32(JUMP_CHAR_BASE + idx as u32).unwrap();
                let row = row_at_mut(&mut root, p);
                row.insert(i + 1, Node::Sym(BLK_CLOSE));
                row.insert(*i, Node::Sym(mark));
                bump(&mut path, &mut col, p, i + 1);
                bump(&mut path, &mut col, p, *i);
            }
            return (root, Some((path, col)));
        }
        let mut path = self.path.clone();
        let mut col = self.col;
        // Deepest rows first: inserting into an ancestor row would shift
        // the node indices a deeper ghost path still needs. The cursor
        // is adjusted in one batch against the original coordinates
        // (incremental nudging breaks on nested ghosts).
        let mut ghosts: Vec<&Vec<(usize, Field)>> = self.ghost.iter().collect();
        ghosts.sort_by_key(|p| std::cmp::Reverse(p.len()));
        for p in ghosts {
            row_at_mut(&mut root, p).insert(0, Node::Sym(SLOT_GHOST));
        }
        let adjusted = ghost_adjust(&self.ghost, &path, col);
        path = adjusted.0;
        col = adjusted.1;
        if let Some((_, buf)) = &self.op_entry {
            // The in-progress name shows as an upright run (a display-
            // only Func atom: never quoted, spaces drawn as ␣) framed by
            // the selection markers, cursor right after the text.
            let row = row_at_mut(&mut root, &path);
            let node = if buf.is_empty() {
                Node::Sym('⬚')
            } else {
                Node::Func(buf.replace(' ', "␣"))
            };
            row.insert(col, Node::Sym(SEL_CLOSE));
            row.insert(col, node);
            row.insert(col, Node::Sym(SEL_OPEN));
            return (root, Some((path, col + 2)));
        }
        if let Some((lo, hi)) = self.selection() {
            let row = row_at_mut(&mut root, &path);
            row.insert(hi, Node::Sym(SEL_CLOSE));
            row.insert(lo, Node::Sym(SEL_OPEN));
            col = if col >= hi {
                col + 2
            } else if col > lo {
                col + 1
            } else {
                col
            };
        }
        (root, Some((path, col)))
    }

    /// Toggle grid edit mode (only meaningful inside a matrix).
    pub fn grid_mode_toggle(&mut self) {
        if self.grid_mode {
            self.grid_mode = false;
        } else if self.enclosing_array().is_some() {
            self.grid_mode = true;
            self.select_anchor = None;
        } else {
            self.message = "^O works inside a matrix/array".into();
        }
    }

    /// A modal state is capturing keys (jump/block/free/minibuffer/op
    /// box) — undo/redo chords stay out of the way there.
    pub fn mode_active(&self) -> bool {
        self.jump.is_some()
            || self.block.is_some()
            || self.free.is_some()
            || self.minibuffer.is_some()
            || self.op_entry.is_some()
    }

    pub(crate) fn push_undo(&mut self, state: Snapshot) {
        const UNDO_CAP: usize = 1000;
        self.undo.push(state);
        if self.undo.len() > UNDO_CAP {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    pub fn undo(&mut self) {
        let Some((root, path, col)) = self.undo.pop() else {
            self.message = "nothing to undo".into();
            return;
        };
        self.select_anchor = None;
        self.ghost.clear();
        self.redo.push((
            std::mem::replace(&mut self.root, root),
            self.path.clone(),
            self.col,
        ));
        self.path = path;
        self.col = col;
    }

    pub fn redo(&mut self) {
        let Some((root, path, col)) = self.redo.pop() else {
            self.message = "nothing to redo".into();
            return;
        };
        self.select_anchor = None;
        self.ghost.clear();
        self.undo.push((
            std::mem::replace(&mut self.root, root),
            self.path.clone(),
            self.col,
        ));
        self.path = path;
        self.col = col;
    }

    /// Open the `\op` operator-name box at the cursor.
    pub fn op_start(&mut self, kind: BoxKind) {
        self.select_anchor = None;
        self.op_entry = Some((kind, String::new()));
    }

    pub fn op_type(&mut self, c: char) {
        if let Some((_, buf)) = &mut self.op_entry {
            buf.push(c);
        }
    }

    /// Backspace in the box; an empty box cancels it.
    pub fn op_backspace(&mut self) {
        if let Some((_, buf)) = &mut self.op_entry
            && buf.pop().is_none()
        {
            self.op_entry = None;
        }
    }

    /// Commit the box: each space-separated word becomes a Func
    /// (dictionary words) or a bare upright run (Text). `\op*` builds a
    /// ┈band┈ with the words as its pieces (┈arg┈max┈) and enters the
    /// lower limit; plain `\op` inserts the words joined by ␣.
    pub fn op_commit(&mut self) {
        let Some((kind, text)) = self.op_entry.take() else {
            return;
        };
        let piece = |w: &str| {
            if !w.contains('.') && crate::symbols::is_func_name(w) {
                Node::Func(w.to_string())
            } else {
                Node::Text {
                    t: w.to_string(),
                    math: true,
                }
            }
        };
        match kind {
            BoxKind::Rm => {
                let t = text.trim();
                if !t.is_empty() {
                    let node = piece(t);
                    let col = self.col;
                    self.cur_row_mut().insert(col, node);
                    self.col += 1;
                }
            }
            BoxKind::Text => {
                if !text.is_empty() {
                    let node = Node::Text {
                        t: text,
                        math: false,
                    };
                    let col = self.col;
                    self.cur_row_mut().insert(col, node);
                    self.col += 1;
                }
            }
            BoxKind::Op | BoxKind::OpStar => {
                let words: Vec<&str> = text.split_whitespace().collect();
                if words.is_empty() {
                    return;
                }
                if kind == BoxKind::OpStar {
                    let base: Vec<Node> = words.iter().map(|w| piece(w)).collect();
                    self.insert_and_enter(Node::BigOp {
                        base,
                        lower: vec![],
                        upper: vec![],
                    });
                } else {
                    let mut nodes: Vec<Node> = Vec::new();
                    for (i, w) in words.iter().enumerate() {
                        if i > 0 {
                            nodes.push(Node::Sym('␣'));
                        }
                        nodes.push(piece(w));
                    }
                    let col = self.col;
                    let n = nodes.len();
                    self.cur_row_mut().splice(col..col, nodes);
                    self.col += n;
                }
            }
        }
    }

    /// Execute a `\command` from the minibuffer.
    pub fn execute(&mut self, cmd: &str) {
        if cmd.is_empty() {
            return;
        }
        // With an active selection, structure commands wrap it (LyX-like).
        match cmd {
            "frac" => {
                // Selection becomes the numerator; cursor moves to the
                // denominator.
                if let Some(content) = self.take_selection() {
                    let col = self.col;
                    self.cur_row_mut().insert(
                        col,
                        Node::Frac {
                            num: content,
                            den: vec![],
                        },
                    );
                    self.path.push((col, Field::FracDen));
                    self.col = 0;
                } else {
                    self.insert_and_enter(Node::Frac {
                        num: vec![],
                        den: vec![],
                    });
                }
                return;
            }
            "sqrt" if self.wrap_selection(|c| Node::Sqrt { arg: c, index: 2 }) => return,
            "cbrt" | "sqrt3" if self.wrap_selection(|c| Node::Sqrt { arg: c, index: 3 }) => return,
            "cancel" if self.wrap_selection(|c| Node::Cancel { arg: c }) => return,
            _ => {}
        }
        match cmd {
            "sqrt" => self.insert_and_enter(Node::Sqrt {
                arg: vec![],
                index: 2,
            }),
            "cbrt" | "sqrt3" => self.insert_and_enter(Node::Sqrt {
                arg: vec![],
                index: 3,
            }),
            "qdrt" | "sqrt4" => self.insert_and_enter(Node::Sqrt {
                arg: vec![],
                index: 4,
            }),
            "cancel" => self.insert_and_enter(Node::Cancel { arg: vec![] }),
            "ceil" => self.insert_delim('⌈', '⌉', vec![]),
            "floor" => self.insert_delim('⌊', '⌋', vec![]),
            "norm" | "Vert" => self.insert_delim('‖', '‖', vec![]),
            // Grid commands take an optional RxC digit suffix:
            // \matrix (2×2), \matrix34 (3 rows × 4 cols), \cases41 …
            _ if grid_command(cmd).is_some() => {
                let (delims, rows, cols) = grid_command(cmd).unwrap();
                match delims {
                    Some((l, r)) => self.insert_grid(l, r, rows, cols),
                    None => {
                        // Bare grid: self-delimiting ┌┬┐ lattice.
                        let array = Node::Array {
                            rows,
                            cols,
                            cells: vec![vec![]; rows * cols],
                        };
                        let col = self.col;
                        self.cur_row_mut().insert(col, array);
                        self.path.push((col, Field::Cell(0)));
                        self.col = 0;
                    }
                }
            }
            "abs" => self.insert_delim('|', '|', vec![]),
            "langle" => self.insert_delim('⟨', '⟩', vec![]),
            "braket" => self.insert_delim('⟨', '⟩', vec!['|']),
            "set" => self.insert_delim('{', '}', vec!['|']),
            "mid" => self.insert_mid(),
            "overbrace" | "underbrace" => {
                let over = cmd == "overbrace";
                if let Some(content) = self.take_selection() {
                    // Selection becomes the argument; cursor to the label.
                    let col = self.col;
                    self.cur_row_mut().insert(
                        col,
                        Node::Brace {
                            over,
                            arg: content,
                            label: vec![],
                        },
                    );
                    self.path.push((col, Field::BraceLabel));
                    self.col = 0;
                } else {
                    self.insert_and_enter(Node::Brace {
                        over,
                        arg: vec![],
                        label: vec![],
                    });
                }
            }
            "xrightarrow" | "xto" => self.insert_and_enter(Node::Arrow {
                op: '→',
                over: vec![],
                under: vec![],
            }),
            "xleftarrow" | "xfrom" => self.insert_and_enter(Node::Arrow {
                op: '←',
                over: vec![],
                under: vec![],
            }),
            "xRightarrow" | "xTo" => self.insert_and_enter(Node::Arrow {
                op: '⇒',
                over: vec![],
                under: vec![],
            }),
            "xLeftarrow" | "xFrom" => self.insert_and_enter(Node::Arrow {
                op: '⇐',
                over: vec![],
                under: vec![],
            }),
            // Multi-piece limit operators (┈arg┈max┈). Hardcoded for now;
            // arbitrary bases need OpBase editing (roadmap).
            "argmax" | "argmin" => {
                let f = if cmd == "argmax" { "max" } else { "min" };
                self.insert_and_enter(Node::BigOp {
                    base: vec![Node::Func("arg".into()), Node::Func(f.into())],
                    lower: vec![],
                    upper: vec![],
                });
            }
            "addrow" => self.add_row(),
            "addcol" => self.add_col(),
            "delrow" => self.del_row(),
            "delcol" => self.del_col(),
            _ if lr_spec(cmd).is_some() => {
                let (l, r, mids) = lr_spec(cmd).unwrap();
                self.insert_delim(l, r, mids);
            }
            _ => {
                if let Some(op) = bigop_by_name(cmd) {
                    self.insert_and_enter(Node::BigOp {
                        base: vec![Node::Sym(op)],
                        lower: vec![],
                        upper: vec![],
                    });
                } else if crate::symbols::func_takes_limits(cmd) {
                    // Limit-taking operators enter the lower limit (┈lim┈).
                    self.insert_and_enter(Node::BigOp {
                        base: vec![Node::Func(cmd.to_string())],
                        lower: vec![],
                        upper: vec![],
                    });
                } else if is_func_name(cmd) {
                    let col = self.col;
                    self.cur_row_mut().insert(col, Node::Func(cmd.to_string()));
                    self.col += 1;
                } else if let Some((mark, _)) = accent_by_name(cmd) {
                    self.apply_accent(mark);
                } else if let Some(arg) = cmd
                    .strip_prefix('^')
                    .map(|r| (true, r))
                    .or_else(|| cmd.strip_prefix('_').map(|r| (false, r)))
                    .and_then(|(sup, rest)| script_cmd_arg(rest).map(|a| (sup, a)))
                {
                    // \^z / \_i insert a real Sup / Sub node (the ext
                    // symbol table's modifier-letter spellings like
                    // "^A" -> ᴬ are shadowed on purpose: a superscript
                    // should be structure, not a look-alike atom).
                    let (sup, arg) = arg;
                    let col = self.col;
                    let node = if sup {
                        Node::Sup { arg }
                    } else {
                        Node::Sub { arg }
                    };
                    self.cur_row_mut().insert(col, node);
                    self.col += 1;
                } else if let Some(c) = symbol_by_name(cmd) {
                    if bigop_by_char(c) {
                        self.insert_and_enter(Node::BigOp {
                            base: vec![Node::Sym(c)],
                            lower: vec![],
                            upper: vec![],
                        });
                    } else {
                        self.insert_sym(c);
                    }
                } else if matches!(
                    cmd,
                    "op" | "op*" | "operatorname" | "operatorname*" | "limits" | "rm" | "text"
                ) {
                    // Bare \op / \op* (alias \limits) / \rm / \text open
                    // the in-place box (see op_commit). The old attached
                    // \op<name> style is gone; \rm<x> / \text<x> remain.
                    self.op_start(match cmd {
                        "op" | "operatorname" => BoxKind::Op,
                        "rm" => BoxKind::Rm,
                        "text" => BoxKind::Text,
                        _ => BoxKind::OpStar,
                    });
                } else if let Some((t, math)) = cmd
                    .strip_prefix("rm")
                    .map(|t| (t, true))
                    .or_else(|| cmd.strip_prefix("text").map(|t| (t, false)))
                    .filter(|(t, math)| {
                        // \mathrm content must survive the quoted form
                        // '…', which only reads ASCII alphanumerics —
                        // anything else would break the roundtrip
                        // (\op* alone used to make a Text{"*"}). \text
                        // ("…") reads any glyph except the quotes.
                        !t.is_empty()
                            && if *math {
                                t.chars().all(|c| c.is_ascii_alphanumeric() || c == '.')
                            } else {
                                !t.contains('"') && !t.contains('\'')
                            }
                    })
                {
                    // \rm<chars> = \mathrm, \text<chars> = \text. An
                    // upright dictionary word IS the function (the letter
                    // -run rule reads it back as Func anyway), so \rmsin
                    // falls back to \sin instead of a quoted 'sin'.
                    let node = if math && is_func_name(t) {
                        Node::Func(t.to_string())
                    } else {
                        Node::Text {
                            t: t.to_string(),
                            math,
                        }
                    };
                    let col = self.col;
                    self.cur_row_mut().insert(col, node);
                    self.col += 1;
                } else if cmd.starts_with("delim") || cmd.starts_with("lr") {
                    self.message =
                        "usage: \\lr<left>[|s]<right> in visual order, e.g. \\lr(] \\lr{|} \\lr\\langle||\\rangle"
                            .into();
                } else {
                    self.message = format!("unknown command: \\{}", cmd);
                }
            }
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
        assert!(matches!(ed.root[0],
            Node::Delim { left: '(', right: ']', ref mids, .. } if mids == &['|']));
    }

    #[test]
    fn lr_spec_reads_visually_and_accepts_names() {
        let mut ed = Editor::new();
        ed.execute("lr(]");
        assert!(matches!(
            ed.root[0],
            Node::Delim {
                left: '(',
                right: ']',
                ..
            }
        ));
        // Named tokens: \lr\langle||\rangle = ⟨ · | · | · ⟩.
        let mut ed = Editor::new();
        ed.execute("lr\\langle||\\rangle");
        assert!(matches!(ed.root[0],
            Node::Delim { left: '⟨', right: '⟩', ref mids, ref segs }
                if mids == &['|', '|'] && segs.len() == 3));
        // A non-spec `lr…` name falls through to the symbol table
        // (bare \lr is the ↔ arrow in the extended table).
        let mut ed = Editor::new();
        ed.execute("lr");
        assert_eq!(ed.root, vec![Node::Sym('↔')]);
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
        assert_eq!(row_to_latex(&ed.root), "\\mathrm{vol}");
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
                Node::Text {
                    t: "blah".into(),
                    math: true
                }
            ]
        );
        // \op*: an operator band; space-separated words become the band
        // pieces (┈ess┈sup┈), dictionary words as Funcs.
        let mut ed = Editor::new();
        ed.execute("op*");
        type_name(&mut ed, "ess sup");
        assert_eq!(ed.path.last().unwrap().1, Field::OpLower);
        ed.insert_sym('x');
        ed.exit_inset();
        assert!(matches!(&ed.root[0],
            Node::BigOp { base, .. } if base.len() == 2
                && matches!(&base[0], Node::Text { t, .. } if t == "ess")
                && base[1] == Node::Func("sup".into())));
        assert_eq!(row_to_latex(&ed.root), "\\operatorname*{ess sup}_{x}");
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
        // \text keeps the wider charset (only quotes are excluded).
        let mut ed = Editor::new();
        ed.execute("text(a)");
        assert_eq!(row_to_latex(&ed.root), "\\text{(a)}");
    }

    #[test]
    fn argmax_makes_a_two_piece_bigop() {
        let mut ed = Editor::new();
        ed.execute("argmax");
        // Cursor lands in the lower limit.
        assert_eq!(ed.path.last().unwrap().1, Field::OpLower);
        ed.insert_sym('x');
        ed.exit_inset();
        assert_eq!(row_to_latex(&ed.root), "\\mathop{\\arg \\max}_{x}");
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
        ed.insert_delim('(', ')', vec![]);
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
        assert_eq!(row_to_latex(&ed.root), "\\sum _{n}");
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
        assert_eq!(row_to_latex(&ed.root), "\\lim _{x\\to 0}f");
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
        assert_eq!(row_to_latex(&ed.root), "A\\xrightarrow{f}B\\mathrm{dx}");
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
