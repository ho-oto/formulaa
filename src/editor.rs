//! Structural editing model. The cursor is a path of (node index, field)
//! pairs from the root row plus a column inside the innermost row —
//! the same model LyX uses for math insets.

use crate::ast::{row_at, row_at_mut, Field, Node, Row};

/// A cursor position: path into nested rows plus a column.
pub type CursorPos = (Vec<(usize, Field)>, usize);
use crate::symbols::{
    accent_by_name, bigop_by_char, bigop_by_name, is_func_name, symbol_by_name,
};

pub struct Editor {
    pub root: Row,
    pub path: Vec<(usize, Field)>,
    pub col: usize,
    /// Some(text) while the `\command` minibuffer is open.
    pub minibuffer: Option<String>,
    pub message: String,
    pub italic: bool,
    /// Display-only compat glyph mode (F3); see render::GlyphSet.
    pub compat: bool,
    /// EasyMotion-style jump: Some(targets) while waiting for a label key.
    pub jump: Option<Vec<CursorPos>>,
    /// Highlight the enclosing blocks of the cursor (F4).
    pub highlight: bool,
    /// Structure view: paint every block's background by nesting depth (F5).
    pub structure: bool,
}

/// Label keys for jump mode, most reachable first.
pub const JUMP_LABELS: &str =
    "asdfghjklqwertyuiopzxcvbnmASDFGHJKLQWERTYUIOPZXCVBNM0123456789";
/// Private-use chars used as display-time markers (never in a real AST):
/// jump label placeholders …
pub const JUMP_CHAR_BASE: u32 = 0xE000;
/// … and per-level open/close markers around the cursor's enclosing blocks
/// (level 0 = innermost).
pub const HL_OPEN_BASE: u32 = 0xE0E0;
pub const HL_CLOSE_BASE: u32 = 0xE0E8;
pub const HL_LEVELS: usize = 3;

impl Default for Editor {
    fn default() -> Self {
        Self::new()
    }
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
            compat: false,
            jump: None,
            highlight: false,
            structure: false,
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
                (Field::Cell(c), up) => match node {
                    Node::Matrix { cols, cells, .. } => {
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

    /// `)` closes the innermost paren inset if we are inside one.
    pub fn close_paren(&mut self) {
        let inside = self
            .path
            .iter()
            .rposition(|&(_, f)| f == Field::ParenInner);
        match inside {
            Some(k) => {
                let (i, _) = self.path[k];
                self.path.truncate(k);
                self.col = i + 1;
            }
            None => self.insert_sym(')'),
        }
    }

    /// `]` leaves the innermost matrix if we are inside one.
    pub fn close_bracket(&mut self) {
        let inside = self
            .path
            .iter()
            .rposition(|&(_, f)| matches!(f, Field::Cell(_)));
        match inside {
            Some(k) => {
                let (i, _) = self.path[k];
                self.path.truncate(k);
                self.col = i + 1;
            }
            None => self.message = "not inside a matrix ([ ] are reserved; use \\matrix)".into(),
        }
    }

    /// Wrap the atom just before the cursor with an accent mark.
    fn apply_accent(&mut self, mark: char) {
        if self.col == 0 {
            self.message = "accent needs a base character before the cursor".into();
            return;
        }
        let col = self.col;
        let row = self.cur_row_mut();
        match &row[col - 1] {
            Node::Sym(c) => row[col - 1] = Node::Accent { accent: mark, base: *c },
            Node::Accent { .. } => self.message = "stacked accents are not supported".into(),
            _ => self.message = "accents apply to a single character".into(),
        }
    }

    // ----- jump mode (EasyMotion-style) -----

    /// All cursor positions in the document, in document order (a row's
    /// columns first, then its children), excluding the current position.
    pub fn jump_targets(&self) -> Vec<CursorPos> {
        fn walk(row: &Row, path: &mut Vec<(usize, Field)>, out: &mut Vec<CursorPos>) {
            for col in 0..=row.len() {
                out.push((path.clone(), col));
            }
            for (i, node) in row.iter().enumerate() {
                for f in node.fields() {
                    path.push((i, f));
                    walk(node.field(f), path, out);
                    path.pop();
                }
            }
        }
        let mut out = Vec::new();
        walk(&self.root, &mut Vec::new(), &mut out);
        out.retain(|(p, c)| !(p == &self.path && *c == self.col));
        out
    }

    pub fn start_jump(&mut self) {
        let mut targets = self.jump_targets();
        if targets.is_empty() {
            self.message = "no jump targets".into();
            return;
        }
        let max = JUMP_LABELS.chars().count();
        if targets.len() > max {
            targets.truncate(max);
            self.message = "jump: press a label key (some targets unlabeled)".into();
        } else {
            self.message = "jump: press a label key (Esc cancels)".into();
        }
        self.jump = Some(targets);
    }

    pub fn jump_to(&mut self, label: char) {
        if let Some(targets) = self.jump.take() {
            if let Some(idx) = JUMP_LABELS.chars().position(|c| c == label) {
                if let Some((p, c)) = targets.get(idx) {
                    self.path = p.clone();
                    self.col = *c;
                }
            }
            self.message.clear();
        }
    }

    // ----- display decoration (jump labels / block highlight) -----

    /// A copy of the AST with display markers inserted, plus the cursor
    /// adjusted for the insertions (None while jump mode hides it).
    /// Markers are private-use `Sym`s that the TUI turns into colored
    /// labels / brackets; they never appear in a real document.
    pub fn decorated(&self) -> (Row, Option<CursorPos>) {
        let mut root = self.root.clone();
        if let Some(targets) = &self.jump {
            // Reverse document order keeps not-yet-inserted positions valid.
            for (idx, (p, c)) in targets.iter().enumerate().rev() {
                let mark = char::from_u32(JUMP_CHAR_BASE + idx as u32).unwrap();
                row_at_mut(&mut root, p).insert(*c, Node::Sym(mark));
            }
            return (root, None);
        }
        let mut path = self.path.clone();
        if self.highlight && !path.is_empty() {
            let n = path.len();
            // Outermost shown level first, so deeper lookups see the
            // already-adjusted indices.
            for d in n.saturating_sub(HL_LEVELS)..n {
                let level = (n - 1 - d) as u32;
                let open = char::from_u32(HL_OPEN_BASE + level).unwrap();
                let close = char::from_u32(HL_CLOSE_BASE + level).unwrap();
                let i = path[d].0;
                let row = row_at_mut(&mut root, &path[..d]);
                row.insert(i + 1, Node::Sym(close));
                row.insert(i, Node::Sym(open));
                path[d].0 += 1;
            }
        }
        (root, Some((path, self.col)))
    }

    /// Execute a `\command` from the minibuffer.
    pub fn execute(&mut self, cmd: &str) {
        if cmd.is_empty() {
            return;
        }
        match cmd {
            "frac" => self.insert_and_enter(Node::Frac { num: vec![], den: vec![] }),
            "sqrt" => self.insert_and_enter(Node::Sqrt { arg: vec![], index: 2 }),
            "cbrt" | "sqrt3" => self.insert_and_enter(Node::Sqrt { arg: vec![], index: 3 }),
            "qdrt" | "sqrt4" => self.insert_and_enter(Node::Sqrt { arg: vec![], index: 4 }),
            "cancel" => self.insert_and_enter(Node::Cancel { arg: vec![] }),
            "matrix" => self.insert_and_enter(Node::Matrix {
                rows: 2,
                cols: 2,
                cells: vec![vec![], vec![], vec![], vec![]],
            }),
            _ => {
                if let Some(op) = bigop_by_name(cmd) {
                    self.insert_and_enter(Node::BigOp {
                        op,
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
                            op: c,
                            lower: vec![],
                            upper: vec![],
                        });
                    } else {
                        self.insert_sym(c);
                    }
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
        // top row: cols 0..=3 minus current (3) = 3; num row: 0..=1; den: 0..=1
        let targets = ed.jump_targets();
        assert_eq!(targets.len(), 3 + 2 + 2);
        ed.start_jump();
        assert!(ed.jump.is_some());
        // Third label ('d') is top-row col 2 (before the fraction).
        ed.jump_to('d');
        assert!(ed.jump.is_none());
        assert!(ed.path.is_empty());
        assert_eq!(ed.col, 2);
        // First label ('a') would be col 0.
        ed.start_jump();
        ed.jump_to('a');
        assert_eq!((ed.path.len(), ed.col), (0, 0));
    }

    #[test]
    fn decorated_highlight_wraps_ancestors() {
        let mut ed = Editor::new();
        ed.execute("frac");
        ed.execute("sqrt"); // cursor inside sqrt inside frac numerator
        ed.insert_sym('x');
        ed.highlight = true;
        let (root, cursor) = ed.decorated();
        let (path, col) = cursor.unwrap();
        // Markers shift both ancestor indices by one.
        assert_eq!(path[0].0, 1, "frac shifted by its open marker");
        assert_eq!(path[1].0, 1, "sqrt shifted by its open marker");
        assert_eq!(col, 1);
        // Top row: open(level1), frac, close(level1)
        let open1 = char::from_u32(HL_OPEN_BASE + 1).unwrap();
        assert_eq!(root[0], Node::Sym(open1));
        assert!(matches!(root[1], Node::Frac { .. }));
        // Inside the numerator: open(level0), sqrt, close(level0)
        let num = root[1].field(Field::FracNum);
        let open0 = char::from_u32(HL_OPEN_BASE).unwrap();
        assert_eq!(num[0], Node::Sym(open0));
        assert!(matches!(num[1], Node::Sqrt { .. }));
    }

    #[test]
    fn close_paren_exits_inset() {
        let mut ed = Editor::new();
        ed.insert_and_enter(Node::Paren { inner: vec![] });
        ed.insert_sym('x');
        ed.close_paren();
        assert!(ed.path.is_empty());
        assert_eq!(ed.col, 1);
        assert_eq!(row_to_latex(&ed.root), "\\left(x\\right)");
    }
}
