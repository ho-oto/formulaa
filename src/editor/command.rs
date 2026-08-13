//! The `\\command` layer: the in-place name box (`\\op` `\\rm` `\\text`)
//! and the dispatch that turns a command name into an edit. Kept apart
//! from the editing operations it calls, so the precedence between the
//! command tables is readable in one place.

use super::*;

use crate::symbols::{Arrow, ColDelim, Delim, Radical};

impl Editor {
    /// Open the `\op` operator-name box at the cursor.
    pub fn op_start(&mut self, kind: BoxKind) {
        self.select_anchor = None;
        self.op_entry = Some((kind, String::new()));
        self.op_cmd = self.executing.clone();
        self.op_cursor = 0;
    }

    /// The byte offset of char index `i` in the open box's content.
    fn op_byte(buf: &str, i: usize) -> usize {
        buf.char_indices().nth(i).map_or(buf.len(), |(b, _)| b)
    }

    pub fn op_type(&mut self, c: char) {
        let cur = self.op_cursor;
        if let Some((_, buf)) = &mut self.op_entry {
            let at = Self::op_byte(buf, cur);
            buf.insert(at, c);
            self.op_cursor += 1;
        }
    }

    /// Backspace in the box: delete before the caret; an empty box
    /// cancels it.
    pub fn op_backspace(&mut self) {
        let cur = self.op_cursor;
        if let Some((_, buf)) = &mut self.op_entry {
            if buf.is_empty() {
                self.op_entry = None;
            } else if cur > 0 {
                let at = Self::op_byte(buf, cur - 1);
                buf.remove(at);
                self.op_cursor -= 1;
            }
        }
    }

    pub fn op_delete(&mut self) {
        let cur = self.op_cursor;
        if let Some((_, buf)) = &mut self.op_entry
            && cur < buf.chars().count()
        {
            let at = Self::op_byte(buf, cur);
            buf.remove(at);
        }
    }

    /// Move the box caret; false when the step would leave the box
    /// (the key layer commits then).
    pub fn op_move(&mut self, delta: isize) -> bool {
        let Some((_, buf)) = &self.op_entry else {
            return false;
        };
        let len = buf.chars().count();
        match self.op_cursor.checked_add_signed(delta) {
            Some(c) if c <= len => {
                self.op_cursor = c;
                true
            }
            _ => false,
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
                    let name = words.concat();
                    // A one-character band is the ∑-class symbol form
                    // (┈T┈ would read back as a symbol with limits, not
                    // a named operator), so the picture has no way to
                    // spell this and the parser rejects it.
                    if name.chars().count() < 2 {
                        // Answer in the spelling that opened the box
                        // (\op* / \limits / \operatorname*).
                        let cmd = self.op_cmd.take().unwrap_or_else(|| "op*".into());
                        self.error(format!("\\{cmd} needs a name of two or more characters"));
                        return;
                    }
                    self.insert_and_enter(Node::BigOp {
                        name,
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
    ///
    /// A spelling that only *explains itself* is not known: `\lr` and
    /// `\matrix` print a usage line rather than build anything, and
    /// showing them as ready would promise an Enter that does nothing.
    /// Half-typed commands read red the same way `\alph` does, and the
    /// completion list is what says where the spelling can still go.
    pub fn command_known(&self, cmd: &str) -> bool {
        resolve(cmd).is_some()
    }

    /// What the open minibuffer would insert on commit, as a row the
    /// display can render (empty slots show as ⬚): \alpha previews α,
    /// \frac the empty fraction, \pmatrix the delimited grid. Mode
    /// openers and grid surgery preview as nothing. Pure, like
    /// `resolve`.
    pub fn command_preview_row(&self) -> Option<Row> {
        preview_row(self.minibuffer.as_deref()?)
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
            Some(edit) => {
                // Errors raised while applying (and later, when an
                // opened box commits) answer in this spelling.
                self.executing = Some(cmd.to_string());
                self.apply(edit);
                self.executing = None;
            }
            None if cmd.starts_with("delim") || cmd.starts_with("lr") => {
                // The message answers in the spelling the user chose.
                let p = if cmd.starts_with("delim") {
                    "delim"
                } else {
                    "lr"
                };
                // Short on purpose: the completion is the manual for
                // the token vocabulary.
                self.message = format!(
                    "\\{p} takes delimiters in visual order: opening, (middle vertical lines,) closing"
                );
            }
            None => self.error(format!("\\{} is not a command", cmd)),
        }
    }

    /// Apply a resolved edit to the tree. All command-driven mutation
    /// funnels through here; `resolve` decides *what*, this does *how*.
    pub fn apply(&mut self, edit: Edit) {
        // Three fates for an active selection. The wrapping edits
        // consume it into the node (\frac, ( — the selection becomes
        // the first field); every content-inserting edit *replaces* it
        // (typing over a selection, the standard editor behavior); the
        // structural rest just drops the anchor (the row shifts under
        // it, so a survivor would designate a different range).
        let wraps = matches!(
            edit,
            Edit::Insert { wrap: true, .. } | Edit::Accent(_) | Edit::Delim { .. }
        );
        let replaces = matches!(
            edit,
            Edit::Sym(_)
                | Edit::Insert { wrap: false, .. }
                | Edit::Grid { .. }
                | Edit::Mid
                | Edit::OpenBox(_)
        );
        if replaces {
            if self.selection().is_some() {
                self.take_selection();
            } else {
                self.select_anchor = None;
            }
        } else if !wraps {
            self.select_anchor = None;
        }
        match edit {
            Edit::Insert { node, wrap } => {
                let has_empty_slot = {
                    let fields = node.fields();
                    fields.iter().any(|&f| node.field(f).is_empty())
                };
                if wrap && self.selection().is_some() {
                    // The selection becomes the node's first field; the
                    // cursor lands in the next empty field (\frac: the
                    // denominator) or after the node (\sqrt).
                    // A field is always an inset, where a formula line
                    // break cannot live.
                    let content: crate::ast::Row = self
                        .take_selection()
                        .unwrap()
                        .into_iter()
                        .filter(|n| *n != Node::Break)
                        .collect();
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
            // A delimiter wraps like `(` does: the selection becomes
            // its first segment (the Insert machinery owns that
            // reading, and \braket then enters the second segment).
            Edit::Delim { left, right, mids } => self.apply(Edit::Insert {
                node: Node::Delim {
                    left,
                    right,
                    mids,
                    segs: vec![vec![]; mids + 1],
                },
                wrap: true,
            }),
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
            Edit::AddRow => self.add_lane(false),
            Edit::AddCol => self.add_lane(true),
            Edit::DelRow => self.del_lane(false),
            Edit::DelCol => self.del_lane(true),
            Edit::OpenBox(kind) => self.op_start(kind),
            Edit::Negate => self.negate_prev(),
        }
    }
}

/// What committing `cmd` would insert, as a row the display can
/// render (empty slots show as ⬚): \alpha previews α, \frac the empty
/// fraction, \pmatrix the delimited grid. Mode openers and grid
/// surgery preview as nothing. Pure, like `resolve`.
pub fn preview_row(cmd: &str) -> Option<Row> {
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
        // An accent previews as its spacing form (`ˆ`, `˙`): the mark
        // hung on `⬚` is two lines tall, which reads as a fragment in
        // a one-line preview box.
        Edit::Accent(mark) => Some(vec![Node::Sym(mark.info().preview)]),
        // A name box previews as the slot it opens: what commits is a
        // box to fill, and the empty-slot glyph is exactly that.
        Edit::OpenBox(_) => Some(vec![Node::Sym(crate::glyphs::PLACEHOLDER)]),
        // The rest act on what is already there, so what they produce
        // depends on where the cursor is — which `resolve` does not
        // know. Better nothing than a guess.
        Edit::Mid | Edit::Negate | Edit::AddRow | Edit::AddCol | Edit::DelRow | Edit::DelCol => {
            None
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
    /// `\!`: replace the symbol atom left of the cursor with its
    /// slashed negation (= → ≠), when the table has one.
    Negate,
    AddRow,
    AddCol,
    DelRow,
    DelCol,
    /// Open the in-place name box (`\op` `\op*` `\rm` `\text`).
    OpenBox(BoxKind),
}

/// The mode and system commands: minibuffer spellings for what
/// normally lives on a ctrl chord (^F, ^B, ^T, ^C, ^Q), so the
/// editor still works in a terminal that steals those chords. They
/// are not `Edit`s — they move a mode, the clipboard, or the app
/// itself — so `resolve` never sees them; the input layer runs them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModeCmd {
    Free,
    BlockSelect,
    GridEdit,
    /// Put the canonical AA on the *system* clipboard (^Y) — the copy
    /// that leaves the program, which is what a clipboard spelling
    /// means; the editor-internal ^C/^X have selection semantics of
    /// their own.
    CopyAa,
    Quit,
}

/// One row per mode command — its spellings, the chord it stands in
/// for, and a one-line gloss — for the completion list. `mode_command`
/// is the dispatch; a test keeps the two agreeing spelling by
/// spelling.
pub const MODE_COMMANDS: &[(&[&str], ModeCmd, &str, &str)] = &[
    (&["f", "F", "free"], ModeCmd::Free, "^F", "free cursor"),
    // No `\bs` — it reads as "backslash" as easily as "block
    // select".
    (
        &["b", "B", "block", "blockselect"],
        ModeCmd::BlockSelect,
        "^B",
        "block select",
    ),
    // `\t` would sit one letter from τ's `\ta`; the g family
    // matches the chord.
    (&["g", "G", "grid"], ModeCmd::GridEdit, "^G", "grid edit"),
    // Only the full word: \c beside ^C (the *internal* copy) would
    // read as the same thing, and it is not.
    (&["clipboard"], ModeCmd::CopyAa, "^Y", "AA to clipboard"),
    // Quit alone has no one-letter spelling: \q is one typo from
    // quitting, and nothing else here is that irreversible.
    (&["quit"], ModeCmd::Quit, "^Q", "quit"),
];

/// The mode command a spelling names, if any.
pub fn mode_command(cmd: &str) -> Option<ModeCmd> {
    MODE_COMMANDS
        .iter()
        .find(|(names, ..)| names.contains(&cmd))
        .map(|&(_, mode, ..)| mode)
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
        "norm" => wrap(Node::Norm { arg: vec![] }),
        "overbrace" | "underbrace" => wrap(Node::Brace {
            over: cmd == "overbrace",
            arg: vec![],
            label: vec![],
        }),
        "ceil" => delim(Delim::Col(ColDelim::Ceil), Delim::Col(ColDelim::Ceil), 0),
        "floor" => delim(Delim::Col(ColDelim::Floor), Delim::Col(ColDelim::Floor), 0),
        "abs" => delim(Delim::Col(ColDelim::Bar), Delim::Col(ColDelim::Bar), 0),
        // A lone angle pair is spelled \lr<> — `\langle` stays an
        // \lr token only, like \lceil (one rule for every side name).
        "bra" => delim(Delim::Angle, Delim::Col(ColDelim::Bar), 0),
        "ket" => delim(Delim::Col(ColDelim::Bar), Delim::Angle, 0),
        "braket" => delim(Delim::Angle, Delim::Angle, 1),
        "set" => delim(Delim::Col(ColDelim::Brace), Delim::Col(ColDelim::Brace), 1),
        "mid" => Some(Edit::Mid),
        "!" | "negate" => Some(Edit::Negate),
        "addrow" => Some(Edit::AddRow),
        "addcol" => Some(Edit::AddCol),
        "delrow" => Some(Edit::DelRow),
        "delcol" => Some(Edit::DelCol),
        // The exact spelling `\delim` is `\lr` in every way — including
        // the way a bare `\lr` falls through the spec parser onto the
        // ext symbol `lr` (↔).
        "op" | "operatorname" => Some(Edit::OpenBox(BoxKind::Op)),
        "op*" | "operatorname*" | "limits" => Some(Edit::OpenBox(BoxKind::OpStar)),
        "rm" => Some(Edit::OpenBox(BoxKind::Rm)),
        "text" => Some(Edit::OpenBox(BoxKind::Text)),
        "latex" => Some(Edit::OpenBox(BoxKind::Tex)),
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
                if is_bigop(c) {
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A mode spelling must never shadow an edit: `\t` is grid mode
    /// only because no symbol answers to it, and the moment one does
    /// this fails instead of a command silently changing meaning. The
    /// walk the docs promise — \t (mode) -> \ta (edit) — is pinned
    /// too.
    #[test]
    fn mode_spellings_shadow_nothing() {
        for (names, mode, ..) in MODE_COMMANDS {
            for n in *names {
                assert_eq!(mode_command(n), Some(*mode), "\\{} dispatches elsewhere", n);
                assert!(resolve(n).is_none(), "\\{} is also an edit", n);
            }
        }
        assert!(mode_command("t").is_none(), "\\t came back");
        assert!(mode_command("b").is_some() && resolve("bar").is_some());
        assert!(mode_command("ba").is_none());
    }
}
