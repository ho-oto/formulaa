//! Structural editing model. The cursor is a path of (node index, field)
//! pairs from the root row plus a column inside the innermost row —
//! the same model LyX uses for math insets.

use crate::ast::{Field, Node, Row, row_at, row_at_mut};

/// A cursor position: path into nested rows plus a column.
pub type CursorPos = (Vec<(usize, Field)>, usize);
/// A block-select target: (parent row path, node index).
pub type BlockRef = (Vec<(usize, Field)>, usize);
use crate::symbols::{accent_by_name, bigop_by_char, bigop_by_name, is_func_name, symbol_by_name};

pub struct Editor {
    pub root: Row,
    pub path: Vec<(usize, Field)>,
    pub col: usize,
    /// Some(text) while the `\command` minibuffer is open.
    pub minibuffer: Option<String>,
    pub message: String,
    pub italic: bool,
    /// EasyMotion-style jump: Some(targets) while waiting for a label
    /// key. Document-ordered (needed for marker insertion); each entry
    /// carries its label rank (0 = key 'a' = closest to the cursor).
    pub jump: Option<Vec<(usize, CursorPos)>>,
    /// Block-select mode (Ctrl+B): Some(targets) while waiting for a
    /// label key; each target is (parent row path, node index) of a
    /// structure node, and picking one selects the whole block.
    pub block: Option<Vec<BlockRef>>,
    /// Structure view: paint every block's background by nesting depth (Ctrl+O).
    pub structure: bool,
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

impl Default for Editor {
    fn default() -> Self {
        Self::new()
    }
}

/// Functions that take under-limits: the minibuffer command inserts a
/// ┄band┄ and puts the cursor in the lower limit.
const LIMIT_FUNCS: &[&str] = &[
    "lim", "liminf", "limsup", "max", "min", "sup", "inf", "det", "gcd", "Pr",
];

/// `\lr…` / `\delim…` (Typst-style) delimiter spec, read in *visual
/// order*: first token = left, interior tokens = middles ('|' only),
/// last token = right. A token is one spec char — `( ) [ ] { } | .`
/// with `<` `>` aliasing ⟨ ⟩ — or a `\name` (\langle, \vert, \none …),
/// so `\lr(]`, `\lr{|}` and `\lr\langle||\rangle` all read like the
/// picture. None when the string is not a delimiter spec (a `\lr…`
/// symbol name like \lrcorner then resolves normally).
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
            message: String::new(),
            italic: true,
            jump: None,
            block: None,
            structure: false,
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
    /// nearest boundary. Probe-based: each candidate position is laid
    /// out with its cursor and the rendered ▌ cell is compared against
    /// the click point (the displayed grid differs from a probe grid by
    /// at most a column around the two cursors, so nearest-match lands
    /// on the intended or an adjacent boundary).
    pub fn click(&mut self, x: usize, y: usize) {
        use crate::render::{CURSOR_CHAR, RenderCtx, render_row};
        self.jump = None;
        self.block = None;
        self.select_anchor = None;
        let ctx = RenderCtx {
            italic: self.italic,
        };
        let mut candidates = self.jump_targets();
        candidates.push(((self.path.clone(), self.col), false, false));
        let mut best: Option<(usize, CursorPos)> = None;
        for ((p, c), _, _) in candidates {
            let block = render_row(&self.root, Some((&p[..], c)), false, &ctx);
            let Some((cy, cx)) = block.lines.iter().enumerate().find_map(|(row, line)| {
                line.iter()
                    .position(|&ch| ch == CURSOR_CHAR)
                    .map(|col| (row, col))
            }) else {
                continue;
            };
            let score = cy.abs_diff(y) * 1000 + cx.abs_diff(x);
            if best.as_ref().is_none_or(|(s, _)| score < *s) {
                best = Some((score, (p, c)));
            }
        }
        if let Some((_, (p, c))) = best {
            self.path = p;
            self.col = c;
        }
    }

    /// Ctrl+A: jump to the very start of the whole formula.
    pub fn document_start(&mut self) {
        self.select_anchor = None;
        self.path.clear();
        self.col = 0;
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
        // Bare big operator to the left of the cursor: promote it to a
        // band and enter the limit. Needed to reopen the limits of a
        // normalized formula — an empty-limit BigOp does not survive the
        // canonical form (e.g. a --session restore), so a plain ∑ / lim
        // atom must be liftable back from the UI.
        if self.col > 0 {
            let col = self.col - 1;
            let promotable = match &self.cur_row()[col] {
                Node::Sym(c) => bigop_by_char(*c),
                Node::Func(name) => LIMIT_FUNCS.contains(&name.as_str()),
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
            }
        }
    }

    /// LyX-style Space: leave the innermost inset, landing just after it.
    pub fn exit_inset(&mut self) {
        if let Some((i, _)) = self.path.pop() {
            self.col = i + 1;
        }
    }

    pub fn home(&mut self) {
        self.col = 0;
    }

    pub fn end(&mut self) {
        self.col = self.cur_row().len();
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
    fn enclosing_array(&self) -> Option<(usize, usize, usize)> {
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

    /// Jump candidates in true document order (column, then that
    /// node's children, then the next column): every cursor position,
    /// flagged as (position, is_empty_slot, is_cell_end). `start_jump`
    /// thins this out; the order doubles as the distance metric.
    #[allow(clippy::type_complexity)]
    pub fn jump_targets(&self) -> Vec<(CursorPos, bool, bool)> {
        fn walk(
            row: &Row,
            path: &mut Vec<(usize, Field)>,
            in_cell: bool,
            out: &mut Vec<(CursorPos, bool, bool)>,
        ) {
            if row.is_empty() {
                out.push(((path.clone(), 0), true, in_cell));
                return;
            }
            for (i, node) in row.iter().enumerate() {
                out.push(((path.clone(), i), false, false));
                for f in node.fields() {
                    path.push((i, f));
                    walk(node.field(f), path, matches!(f, Field::Cell(_)), out);
                    path.pop();
                }
            }
            out.push(((path.clone(), row.len()), false, in_cell));
        }
        let mut out = Vec::new();
        walk(&self.root, &mut Vec::new(), false, &mut out);
        out.retain(|((p, c), _, _)| !(p == &self.path && *c == self.col));
        out
    }

    /// Max number of jump labels shown at once.
    pub const JUMP_MAX: usize = 16;

    pub fn start_jump(&mut self) {
        let targets = self.jump_targets();
        if targets.is_empty() {
            self.message = "no jump targets".into();
            return;
        }
        let cursor_seq = targets
            .iter()
            .enumerate()
            .filter(|(_, ((p, _), _, _))| p == &self.path)
            .min_by_key(|(_, ((_, c), _, _))| c.abs_diff(self.col))
            .map(|(k, _)| k)
            .unwrap_or(0);
        // Unfilled slots first (nearest first, capped at half the budget
        // so movement anchors keep their share) …
        let mut picked_idx: Vec<usize> = Vec::new();
        let mut empties: Vec<usize> = (0..targets.len()).filter(|&k| targets[k].1).collect();
        empties.sort_by_key(|&k| k.abs_diff(cursor_seq));
        let taken_empties = empties.len().min(Self::JUMP_MAX / 2);
        picked_idx.extend(empties.drain(..taken_empties));
        // … then grid cells: editing the neighbour of a matrix/vector
        // entry is common, so each cell's end gets an early anchor,
        // the cursor's own grid first (a few slots stay reserved for
        // the distance anchors below).
        let grid_key = |p: &[(usize, Field)]| -> Option<(usize, Vec<(usize, Field)>)> {
            let k = p.iter().rposition(|(_, f)| matches!(f, Field::Cell(_)))?;
            Some((p[k].0, p[..k].to_vec()))
        };
        let cursor_grid = grid_key(&self.path);
        let mut cells: Vec<usize> = (0..targets.len())
            .filter(|&k| targets[k].2 && !targets[k].1 && !picked_idx.contains(&k))
            .collect();
        cells.sort_by_key(|&k| {
            let same = cursor_grid.is_some() && grid_key(&targets[k].0.0) == cursor_grid;
            (!same, k.abs_diff(cursor_seq))
        });
        let reserve = 4;
        cells.truncate((Self::JUMP_MAX - picked_idx.len()).saturating_sub(reserve));
        picked_idx.extend(cells);
        // … then movement anchors: per side of the cursor, halving
        // distances from the far end (L, L/2, …, 1) so the far end, the
        // middle grounds and the vicinity are all one keystroke away.
        let budget = Self::JUMP_MAX - picked_idx.len();
        let free = |k: usize| !targets[k].1 && !picked_idx.contains(&k);
        let left: Vec<usize> = (0..cursor_seq).rev().filter(|&k| free(k)).collect();
        let right: Vec<usize> = (cursor_seq..targets.len()).filter(|&k| free(k)).collect();
        let halving = |list: &[usize]| -> Vec<usize> {
            let mut out = Vec::new();
            let mut d = list.len();
            while d >= 1 {
                out.push(list[d - 1]);
                if d == 1 {
                    break;
                }
                d /= 2;
            }
            out
        };
        let mut anchors: Vec<usize> = halving(&left).into_iter().chain(halving(&right)).collect();
        anchors.sort_by_key(|&k| k.abs_diff(cursor_seq));
        anchors.dedup();
        // Over budget: drop the most redundant middle anchors — the
        // nearest and the farthest always survive.
        while anchors.len() > budget && anchors.len() > 2 {
            let (mut worst, mut gap) = (1, usize::MAX);
            for i in 1..anchors.len() - 1 {
                let g = anchors[i + 1].abs_diff(cursor_seq) - anchors[i - 1].abs_diff(cursor_seq);
                if g < gap {
                    gap = g;
                    worst = i;
                }
            }
            anchors.remove(worst);
        }
        anchors.truncate(budget);
        picked_idx.extend(anchors);
        // Ranks follow pick order ('a' = best); markers need doc order.
        let mut picked: Vec<(usize, CursorPos)> = picked_idx
            .iter()
            .enumerate()
            .map(|(rank, &k)| (rank, targets[k].0.clone()))
            .collect();
        picked.sort_by_key(|&(rank, _)| picked_idx[rank]);
        self.message = "jump: press a label key (Esc cancels)".into();
        self.jump = Some(picked);
    }

    pub fn jump_to(&mut self, label: char) {
        if let Some(targets) = self.jump.take() {
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
            for (i, node) in row.iter().enumerate() {
                if !node.fields().is_empty() {
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
    pub fn marker_extents(&self) -> Vec<(usize, usize)> {
        use crate::render::{RenderCtx, render_row};
        let extent = |slice: &[Node]| -> (usize, usize) {
            let b = render_row(&slice.to_vec(), None, false, &RenderCtx::canonical());
            let (h, bl) = (b.height(), b.baseline);
            (bl.min(h), h.saturating_sub(bl + 1))
        };
        if let Some(targets) = &self.block {
            targets
                .iter()
                .map(|(p, i)| extent(&row_at(&self.root, p)[*i..*i + 1]))
                .collect()
        } else if let Some((lo, hi)) = self.selection() {
            vec![extent(&self.cur_row()[lo..hi])]
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
        if let Some(targets) = &self.jump {
            // Reverse document order keeps not-yet-inserted positions valid.
            for (rank, (p, c)) in targets.iter().rev() {
                let mark = char::from_u32(JUMP_CHAR_BASE + *rank as u32).unwrap();
                row_at_mut(&mut root, p).insert(*c, Node::Sym(mark));
            }
            return (root, None);
        }
        if let Some(targets) = &self.block {
            // Label sits immediately left of its block and a close marker
            // right after it, so the display can paint the block's extent
            // (reverse document order keeps positions valid).
            for (idx, (p, i)) in targets.iter().enumerate().rev() {
                let mark = char::from_u32(JUMP_CHAR_BASE + idx as u32).unwrap();
                let row = row_at_mut(&mut root, p);
                row.insert(i + 1, Node::Sym(BLK_CLOSE));
                row.insert(*i, Node::Sym(mark));
            }
            return (root, None);
        }
        let path = self.path.clone();
        let mut col = self.col;
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
            // Multi-piece limit operators (┄arg┄max┄). Hardcoded for now;
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
                } else if LIMIT_FUNCS.contains(&cmd) {
                    // Limit-taking operators enter the lower limit (┄lim┄).
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
                } else if let Some(t) = cmd
                    .strip_prefix("operatorname*")
                    .or_else(|| cmd.strip_prefix("op*"))
                    .filter(|t| !t.is_empty() && t.chars().all(|c| c.is_ascii_alphanumeric()))
                {
                    // \op*<name> (\operatorname*): upright operator that
                    // takes under-limits — a ┄band┄ with a Text base.
                    self.insert_and_enter(Node::BigOp {
                        base: vec![Node::Text {
                            t: t.to_string(),
                            math: true,
                        }],
                        lower: vec![],
                        upper: vec![],
                    });
                } else if let Some((t, math)) = cmd
                    .strip_prefix("rm")
                    .map(|t| (t, true))
                    .or_else(|| cmd.strip_prefix("operatorname").map(|t| (t, true)))
                    .or_else(|| cmd.strip_prefix("op").map(|t| (t, true)))
                    .or_else(|| cmd.strip_prefix("text").map(|t| (t, false)))
                    .filter(|(t, math)| {
                        // \mathrm content must survive the quoted form
                        // '…', which only reads ASCII alphanumerics —
                        // anything else would break the roundtrip
                        // (\op* alone used to make a Text{"*"}). \text
                        // ("…") reads any glyph except the quotes.
                        !t.is_empty()
                            && if *math {
                                t.chars().all(|c| c.is_ascii_alphanumeric())
                            } else {
                                !t.contains('"') && !t.contains('\'')
                            }
                    })
                {
                    // \rm<chars> = \mathrm, \text<chars> = \text.
                    let col = self.col;
                    self.cur_row_mut().insert(
                        col,
                        Node::Text {
                            t: t.to_string(),
                            math,
                        },
                    );
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
    fn op_and_op_star() {
        // \op<name> = arbitrary \mathrm (same AST as \rm).
        let mut ed = Editor::new();
        ed.execute("opvol");
        assert_eq!(row_to_latex(&ed.root), "\\mathrm{vol}");
        // \op*<name> = operator band taking under-limits.
        let mut ed = Editor::new();
        ed.execute("op*esssup");
        assert_eq!(ed.path.last().unwrap().1, Field::OpLower);
        ed.insert_sym('x');
        ed.exit_inset();
        assert_eq!(row_to_latex(&ed.root), "\\operatorname*{esssup}_{x}");
    }

    #[test]
    fn rm_and_op_arguments_are_alphanumeric_only() {
        // `\op*` alone: the `*` must not become the \op argument —
        // Text{math} with non-alphanumeric content cannot roundtrip
        // (the '…' quoted form only reads ASCII alphanumerics).
        for cmd in ["op*", "op*)", "rm*", "opα"] {
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
        assert_eq!(row_to_latex(&ed.root), "\\arg \\max _{x}");
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
    fn jump_targets_and_jump() {
        let mut ed = Editor::new();
        // x + 1/2 with cursor at the end of the top row
        ed.insert_sym('x');
        ed.insert_sym('+');
        ed.execute("frac");
        ed.insert_sym('1');
        ed.vertical(false);
        ed.insert_sym('2');
        ed.exit_inset();
        // Every position, in document order, minus the cursor's own:
        // top row (0, 1, 2; 3 = cursor), num (0, 1), den (0, 1).
        let targets = ed.jump_targets();
        assert_eq!(targets.len(), 7);
        assert!(targets.iter().all(|(_, empty, _)| !empty));
        ed.start_jump();
        assert!(ed.jump.is_some());
        // 'a' = nearest anchor: top-row col 2, right before the fraction.
        ed.jump_to('a');
        assert!(ed.jump.is_none());
        assert_eq!((ed.path.len(), ed.col), (0, 2));
    }

    #[test]
    fn jump_anchors_balance_near_and_far() {
        let mut ed = Editor::new();
        // Twelve filled fractions; cursor back at the far left.
        for _ in 0..12 {
            ed.execute("frac");
            ed.insert_sym('1');
            ed.vertical(false);
            ed.insert_sym('2');
            ed.exit_inset();
        }
        ed.home();
        ed.start_jump();
        let targets = ed.jump.as_ref().unwrap();
        assert!(targets.len() <= Editor::JUMP_MAX);
        // Anchors must reach both the vicinity and the far end (top-row
        // node index of the target path as a rough position).
        let pos = |p: &Vec<(usize, Field)>, c: usize| p.first().map_or(c, |&(i, _)| i);
        assert!(
            targets.iter().any(|(_, (p, c))| pos(p, *c) >= 8),
            "no far anchor: {:?}",
            targets
        );
        assert!(
            targets.iter().any(|(_, (p, c))| pos(p, *c) <= 2),
            "no near anchor: {:?}",
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
    fn jump_prefers_empty_slots_and_caps_labels() {
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
        // Label count never exceeds JUMP_MAX.
        let mut ed = Editor::new();
        for _ in 0..20 {
            ed.execute("frac");
            ed.exit_inset();
            ed.right();
        }
        ed.start_jump();
        assert!(ed.jump.as_ref().unwrap().len() <= Editor::JUMP_MAX);
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
        // Same for limit-taking functions (┄lim┄).
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
