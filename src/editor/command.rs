//! The `\\command` layer: the in-place name box (`\\op` `\\rm` `\\text`)
//! and the dispatch that turns a command name into an edit. Kept apart
//! from the editing operations it calls, so the precedence between the
//! command tables is readable in one place.

use super::*;

use crate::symbols::Arrow;

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
            Edit::Grid { delims, rows, cols } => match delims {
                Some((l, r)) => self.insert_grid(l, r, rows, cols),
                // A bare grid is a self-delimiting lattice: an Array
                // node, entered at its first cell like any template.
                None => self.insert_and_enter(Node::Array {
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
    /// A delimiter pair with optional │ middles.
    Delim {
        left: char,
        right: char,
        mids: Vec<char>,
    },
    /// A rows × cols grid, delimited (`\matrix34`) or bare (`\array`).
    Grid {
        delims: GridDelims,
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
/// mirrors the old dispatch: structural names, grids, `\lr` specs,
/// then the name tables (funcs -> accents -> scripts -> symbols), and
/// the `\rm<x>` / `\text<x>` attached forms last.
pub fn resolve(cmd: &str) -> Option<Edit> {
    let cmd = crate::symbols::unalias(cmd);
    let ins = |node: Node| Some(Edit::Insert { node, wrap: false });
    let wrap = |node: Node| Some(Edit::Insert { node, wrap: true });
    let delim = |l: char, r: char, mids: Vec<char>| {
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
        "sqrt" => wrap(Node::Sqrt {
            arg: vec![],
            index: Radical::Sqrt,
        }),
        "cbrt" => wrap(Node::Sqrt {
            arg: vec![],
            index: Radical::Cbrt,
        }),
        "qdrt" => ins(Node::Sqrt {
            arg: vec![],
            index: Radical::Qdrt,
        }),
        "cancel" => wrap(Node::Cancel { arg: vec![] }),
        "norm" => ins(Node::Norm { arg: vec![] }),
        "overbrace" | "underbrace" => wrap(Node::Brace {
            over: cmd == "overbrace",
            arg: vec![],
            label: vec![],
        }),
        "xto" | "xfrom" | "xTo" | "xFrom" => ins(Node::Arrow {
            op: match cmd {
                "xto" => Arrow::To,
                "xfrom" => Arrow::From,
                "xTo" => Arrow::DoubleTo,
                _ => Arrow::DoubleFrom,
            },
            over: vec![],
            under: vec![],
        }),
        "ceil" => delim('⌈', '⌉', vec![]),
        "floor" => delim('⌊', '⌋', vec![]),
        "abs" => delim('|', '|', vec![]),
        "langle" => delim('⟨', '⟩', vec![]),
        "braket" => delim('⟨', '⟩', vec!['|']),
        "set" => delim('{', '}', vec!['|']),
        "mid" => Some(Edit::Mid),
        "addrow" => Some(Edit::AddRow),
        "addcol" => Some(Edit::AddCol),
        "delrow" => Some(Edit::DelRow),
        "delcol" => Some(Edit::DelCol),
        "op" => Some(Edit::OpenBox(BoxKind::Op)),
        "op*" => Some(Edit::OpenBox(BoxKind::OpStar)),
        "rm" => Some(Edit::OpenBox(BoxKind::Rm)),
        "text" => Some(Edit::OpenBox(BoxKind::Text)),
        _ => {
            if let Some((delims, rows, cols)) = grid_command(cmd) {
                Some(Edit::Grid { delims, rows, cols })
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
