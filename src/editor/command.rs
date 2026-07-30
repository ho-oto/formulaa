//! The `\\command` layer: the in-place name box (`\\op` `\\rm` `\\text`)
//! and the dispatch that turns a command name into an edit. Kept apart
//! from the editing operations it calls, so the precedence between the
//! command tables is readable in one place.

use super::*;

use crate::symbols::{Arrow, Delim, Radical};

impl Editor {
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
        // Any upright run is a Func; a lone letter is Roman.
        let upright = |w: &str| match w.chars().count() {
            1 => Node::Roman(w.chars().next().unwrap()),
            _ => Node::Func(w.to_string()),
        };
        match kind {
            BoxKind::Rm => {
                let t = text.trim();
                if !t.is_empty() {
                    let node = upright(t);
                    let col = self.col;
                    self.cur_row_mut().insert(col, node);
                    self.col += 1;
                }
            }
            BoxKind::Text => {
                if !text.is_empty() {
                    let node = Node::Text(text);
                    let col = self.col;
                    self.cur_row_mut().insert(col, node);
                    self.col += 1;
                }
            }
            BoxKind::Tex => {
                // LaTeX, read best-effort; the guard re-checks the
                // resulting picture like any other edit.
                let mut row = crate::ast::normalize(&crate::from_latex::row_from_latex(&text));
                if !self.path.is_empty() {
                    // Formula line breaks only exist at the top level.
                    row.retain(|n| !matches!(n, Node::Break));
                }
                if row.is_empty() {
                    return;
                }
                let col = self.col;
                let n = row.len();
                self.cur_row_mut().splice(col..col, row);
                self.col += n;
            }
            BoxKind::Op | BoxKind::OpStar => {
                let words: Vec<&str> = text.split_whitespace().collect();
                if words.is_empty() {
                    return;
                }
                if kind == BoxKind::OpStar {
                    // A band name is one piece: the words are joined so
                    // the bare form reads back as exactly one Func.
                    self.insert_and_enter(Node::BigOp {
                        name: words.concat(),
                        lower: vec![],
                        upper: vec![],
                    });
                } else {
                    let mut nodes: Vec<Node> = Vec::new();
                    for (i, w) in words.iter().enumerate() {
                        if i > 0 {
                            nodes.push(Node::Sym('␣'));
                        }
                        nodes.push(upright(w));
                    }
                    let col = self.col;
                    let n = nodes.len();
                    self.cur_row_mut().splice(col..col, nodes);
                    self.col += n;
                }
            }
        }
    }

    /// Does `\cmd` name something the editor can execute? Pure —
    /// `resolve` is the single dispatch, so this can never drift from
    /// what `execute` does. (The TUI colors the minibuffer by this.)
    /// The `\lr` prefixes count as known while the spec is still being
    /// typed (execute shows the usage line for them).
    pub fn command_known(&self, cmd: &str) -> bool {
        !cmd.is_empty()
            && (resolve(cmd).is_some() || cmd.starts_with("delim") || cmd.starts_with("lr"))
    }

    /// What the open minibuffer would insert on commit, as a row the
    /// display can render (empty slots show as ⬚): \alpha previews α,
    /// \frac the empty fraction, \pmatrix the delimited grid. Mode
    /// openers and grid surgery preview as nothing. Pure, like
    /// `resolve`.
    pub fn command_preview_row(&self) -> Option<Row> {
        let cmd = self.minibuffer.as_deref()?;
        if cmd.is_empty() {
            return None;
        }
        match resolve(cmd)? {
            Edit::Sym(c) => Some(vec![Node::Sym(c)]),
            Edit::Insert { node, .. } => Some(vec![node]),
            Edit::Delim { left, right, mids } => Some(vec![Node::Delim {
                left,
                right,
                mids,
                segs: vec![vec![]; mids + 1],
            }]),
            Edit::Grid { wrap, rows, cols } => {
                let array = Node::Array {
                    rows,
                    cols,
                    cells: vec![vec![]; rows * cols],
                };
                Some(vec![match wrap {
                    GridWrap::Bare => array,
                    GridWrap::Norm => Node::Norm { arg: vec![array] },
                    GridWrap::Pair(l, r) => Node::Delim {
                        left: l,
                        right: r,
                        mids: 0,
                        segs: vec![vec![array]],
                    },
                }])
            }
            // An accent hangs its mark on the empty-slot glyph.
            Edit::Accent(mark) => Some(vec![if mark.under() {
                Node::Accent {
                    overs: vec![],
                    unders: vec![mark],
                    base: '⬚',
                }
            } else {
                Node::Accent {
                    overs: vec![mark],
                    unders: vec![],
                    base: '⬚',
                }
            }]),
            Edit::Mid
            | Edit::AddRow
            | Edit::AddCol
            | Edit::DelRow
            | Edit::DelCol
            | Edit::OpenBox(_) => None,
        }
    }

    /// The single-character preview (the text-screen hosts can only
    /// overlay one cell).
    pub fn command_preview(&self) -> Option<char> {
        match self.command_preview_row()?.as_slice() {
            [Node::Sym(c)] => Some(*c),
            [Node::BigOpSym { op, .. }] => Some(*op),
            _ => None,
        }
    }

    /// Execute a `\command` from the minibuffer: resolve the spelling
    /// to an `Edit`, then apply it.
    pub fn execute(&mut self, cmd: &str) {
        if cmd.is_empty() {
            return;
        }
        match resolve(cmd) {
            Some(edit) => self.apply(edit),
            None if cmd.starts_with("delim") || cmd.starts_with("lr") => {
                self.message =
                    "usage: \\lr<left>[|s]<right> in visual order, e.g. \\lr(] \\lr{|} \\lr\\langle||\\rangle"
                        .into();
            }
            None => self.message = format!("unknown command: \\{}", cmd),
        }
    }

    /// Apply a resolved edit to the tree. All command-driven mutation
    /// funnels through here; `resolve` decides *what*, this does *how*.
    pub fn apply(&mut self, edit: Edit) {
        match edit {
            Edit::Insert { node, wrap } => {
                let has_empty_slot = {
                    let fields = node.fields();
                    fields.iter().any(|&f| node.field(f).is_empty())
                };
                if wrap && self.selection().is_some() {
                    // The selection becomes the node's first field; the
                    // cursor lands in the next empty field (\frac: the
                    // denominator) or after the node (\sqrt, \cancel).
                    let content = self.take_selection().unwrap();
                    let mut node = node;
                    let fields = node.fields();
                    *node.field_mut(fields[0]) = content;
                    let enter = fields[1..]
                        .iter()
                        .copied()
                        .find(|&f| node.field(f).is_empty());
                    let col = self.col;
                    self.cur_row_mut().insert(col, node);
                    match enter {
                        Some(f) => {
                            self.path.push((col, f));
                            self.col = 0;
                        }
                        None => self.col += 1,
                    }
                } else if has_empty_slot {
                    // A template with an empty slot: enter its first
                    // field (\frac -> numerator, \lim -> lower limit).
                    self.insert_and_enter(node);
                } else {
                    // A leaf (or fully pre-filled node, like \^z's
                    // superscript): insert and step past.
                    let col = self.col;
                    self.cur_row_mut().insert(col, node);
                    self.col += 1;
                }
            }
            Edit::Sym(c) => self.insert_sym(c),
            Edit::Accent(mark) => self.apply_accent(mark),
            Edit::Delim { left, right, mids } => self.insert_delim(left, right, mids),
            Edit::Grid { wrap, rows, cols } => match wrap {
                GridWrap::Pair(l, r) => self.insert_grid(l, r, rows, cols),
                // A bare grid is a self-delimiting lattice: an Array
                // node, entered at its first cell like any template.
                GridWrap::Norm => self.insert_norm_grid(rows, cols),
                GridWrap::Bare => self.insert_and_enter(Node::Array {
                    rows,
                    cols,
                    cells: vec![vec![]; rows * cols],
                }),
            },
            Edit::Mid => self.insert_mid(),
            Edit::AddRow => self.add_row(),
            Edit::AddCol => self.add_col(),
            Edit::DelRow => self.del_row(),
            Edit::DelCol => self.del_col(),
            Edit::OpenBox(kind) => self.op_start(kind),
        }
    }
}

/// One tree edit, as data. `resolve` turns a command spelling into one
/// of these before any state is touched; keys can build them directly.
/// Modes and navigation are *not* edits — they never change the tree,
/// so they stay in the key layer.
#[derive(Debug, Clone, PartialEq)]
pub enum Edit {
    /// Insert a node. A template with empty slots enters its first
    /// one; a leaf steps past. With `wrap`, an active selection lands
    /// in the node's first field.
    Insert {
        node: Node,
        wrap: bool,
    },
    /// A plain atom (participates in symbol runs).
    Sym(char),
    /// Accent the atom before the cursor, or wrap the selection in a
    /// wide accent.
    Accent(crate::symbols::Accent),
    /// A delimiter pair with `mids` │ middles.
    Delim {
        left: crate::symbols::Delim,
        right: crate::symbols::Delim,
        mids: usize,
    },
    /// A rows × cols grid, wrapped (`\matrix34`, `\Vmatrix`) or
    /// bare (`\array`).
    Grid {
        wrap: GridWrap,
        rows: usize,
        cols: usize,
    },
    /// A │ middle added to the enclosing delimiter.
    Mid,
    AddRow,
    AddCol,
    DelRow,
    DelCol,
    /// Open the in-place name box (`\op` `\op*` `\rm` `\text`).
    OpenBox(BoxKind),
}

/// Resolve a command spelling to the edit it performs. Pure: this is
/// the whole input side of the command layer, so "is it valid" and
/// "what would it do" (the preview) come for free. Order matters and
/// mirrors the old dispatch: structural names (alternative spellings
/// as extra patterns on their arm), grids, `\lr` specs, then the name
/// tables (funcs -> accents -> scripts -> symbols), and the `\rm<x>` /
/// `\text<x>` attached forms last.
pub fn resolve(cmd: &str) -> Option<Edit> {
    let ins = |node: Node| Some(Edit::Insert { node, wrap: false });
    let wrap = |node: Node| Some(Edit::Insert { node, wrap: true });
    let delim = |l: Delim, r: Delim, mids: usize| {
        Some(Edit::Delim {
            left: l,
            right: r,
            mids,
        })
    };
    match cmd {
        "frac" => wrap(Node::Frac {
            num: vec![],
            den: vec![],
        }),
        "cancel" => wrap(Node::Cancel { arg: vec![] }),
        "norm" | "Vert" => ins(Node::Norm { arg: vec![] }),
        "overbrace" | "underbrace" => wrap(Node::Brace {
            over: cmd == "overbrace",
            arg: vec![],
            label: vec![],
        }),
        "ceil" => delim(Delim::Ceil, Delim::Ceil, 0),
        "floor" => delim(Delim::Floor, Delim::Floor, 0),
        "abs" => delim(Delim::Bar, Delim::Bar, 0),
        "langle" => delim(Delim::Angle, Delim::Angle, 0),
        "braket" => delim(Delim::Angle, Delim::Angle, 1),
        "set" => delim(Delim::Brace, Delim::Brace, 1),
        "mid" => Some(Edit::Mid),
        "addrow" => Some(Edit::AddRow),
        "addcol" => Some(Edit::AddCol),
        "delrow" => Some(Edit::DelRow),
        "delcol" => Some(Edit::DelCol),
        // The exact spelling `\delim` is `\lr` in every way — including
        // the way a bare `\lr` falls through the spec parser onto the
        // ext symbol `lr` (↔).
        "delim" => resolve("lr"),
        "op" | "operatorname" => Some(Edit::OpenBox(BoxKind::Op)),
        "op*" | "operatorname*" | "limits" => Some(Edit::OpenBox(BoxKind::OpStar)),
        "rm" => Some(Edit::OpenBox(BoxKind::Rm)),
        "text" => Some(Edit::OpenBox(BoxKind::Text)),
        "tex" | "latex" => Some(Edit::OpenBox(BoxKind::Tex)),
        _ => {
            if let Some(index) = Radical::of_name(cmd) {
                // The root signs (\sqrt \cbrt \qdrt) wrap a selection.
                wrap(Node::Sqrt { arg: vec![], index })
            } else if let Some(op) = Arrow::of_name(cmd) {
                // The stretchy labeled arrows (\xto and friends).
                ins(Node::Arrow {
                    op,
                    over: vec![],
                    under: vec![],
                })
            } else if let Some((wrap, rows, cols)) = grid_command(cmd) {
                Some(Edit::Grid { wrap, rows, cols })
            } else if let Some((l, r, mids)) = lr_spec(cmd) {
                delim(l, r, mids)
            } else if crate::symbols::func_takes_limits(cmd) {
                // Limit-taking operators enter the lower limit (┈lim┈).
                ins(Node::BigOp {
                    name: cmd.to_string(),
                    lower: vec![],
                    upper: vec![],
                })
            } else if is_func_name(cmd) {
                ins(Node::Func(cmd.to_string()))
            } else if let Some(mark) = crate::symbols::Accent::of_name(cmd) {
                Some(Edit::Accent(mark))
            } else if let Some((sup, arg)) = script_cmd(cmd) {
                // \^z / \z^ / \^z^ (and the _ variants) insert a real
                // Sup / Sub node: a superscript is structure, not a
                // look-alike modifier-letter atom.
                ins(if sup {
                    Node::Sup { arg }
                } else {
                    Node::Sub { arg }
                })
            } else if let Some(c) = symbol_by_name(cmd) {
                // The atom table decides how a character materializes:
                // ∑-class operators come in as their band.
                if bigop_by_char(c) {
                    ins(Node::BigOpSym {
                        op: c,
                        lower: vec![],
                        upper: vec![],
                    })
                } else {
                    Some(Edit::Sym(c))
                }
            } else if let Some((t, math)) = cmd
                .strip_prefix("rm")
                .map(|t| (t, true))
                .or_else(|| cmd.strip_prefix("text").map(|t| (t, false)))
                .filter(|(t, math)| {
                    // \mathrm content must survive the quoted form '…',
                    // which only reads ASCII alphanumerics; \text ("…")
                    // takes anything except quotes/brackets (opaque
                    // quoted spans would desync the delimiter scans).
                    !t.is_empty()
                        && if *math {
                            t.chars().all(|c| c.is_ascii_alphanumeric() || c == '.')
                        } else {
                            !t.contains(['\'', '(', ')', '[', ']', '{', '}'])
                        }
                })
            {
                // \rm<chars> = \mathrm, \text<chars> = \text. An
                // upright dictionary word IS the function, so \rmsin
                // falls back to \sin instead of a quoted 'sin'.
                ins(match (math, t.chars().count()) {
                    (false, _) => Node::Text(t.to_string()),
                    (true, 1) => Node::Roman(t.chars().next().unwrap()),
                    (true, _) => Node::Func(t.to_string()),
                })
            } else {
                None
            }
        }
    }
}
