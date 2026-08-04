//! Host-independent keymap: the single place where keystrokes are mapped
//! onto editor operations. The TUI (crossterm) and the wasm bindings
//! (DOM KeyboardEvent) only translate their native events into [`Key`]
//! and dispatch through [`Editor::input`], so the two frontends cannot
//! drift apart.

use crate::ast::Node;
use crate::editor::{BoxKind, Edit, Editor};
use crate::symbols::{ColDelim, Delim};

/// One keystroke, host-neutral. `Char` carries printable input
/// (including ' ', '\\', '^' …); everything else is a named key.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Key {
    Char(char),
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    Enter,
    Backspace,
    Delete,
    Esc,
    Tab,
}

/// Side effects the library cannot perform itself; the host handles
/// them after dispatch (file IO, clipboard, exiting the app).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Effect {
    None,
    Quit,
    /// Put the canonical AA on the host's clipboard.
    CopyAa,
}

impl Editor {
    /// One keystroke of the shared LyX-style keymap. Wraps the dispatch
    /// with undo bookkeeping: any key that changes the formula pushes
    /// the pre-state (undo/redo themselves are handled here so they
    /// never re-enter the history).
    pub fn input(&mut self, key: Key, shift: bool, ctrl: bool) -> Effect {
        if ctrl && !self.mode_active() {
            match key {
                Key::Char('z') => {
                    self.undo();
                    return Effect::None;
                }
                Key::Char('r') => {
                    self.redo();
                    return Effect::None;
                }
                _ => {}
            }
        }
        let before = (self.root.clone(), self.path.clone(), self.col);
        let effect = self.dispatch(key, shift, ctrl);
        if self.root != before.0 {
            self.push_undo(before);
        }
        // Whatever the key did (snap, click, grid surgery), the grid
        // state must fit the tree it now points at.
        self.reclamp_grid();
        effect
    }

    /// Key layers, outermost first: each modal layer either consumes
    /// the key (Some) or lets it fall through (None). Adding a mode =
    /// adding one handler here.
    fn dispatch(&mut self, key: Key, shift: bool, ctrl: bool) -> Effect {
        if let Some(e) = self.free_keys(key, ctrl) {
            return e;
        }
        if let Some(e) = self.block_keys(key, shift, ctrl) {
            return e;
        }
        if let Some(e) = self.minibuffer_keys(key) {
            return e;
        }
        if let Some(e) = self.op_box_keys(key, ctrl) {
            return e;
        }
        if let Some(e) = self.grid_keys(key, shift, ctrl) {
            return e;
        }
        self.base_keys(key, shift, ctrl)
    }

    /// Block-select mode: arrows walk the ancestor chain (↑/→ wider,
    /// ↓/← narrower), Enter selects, ^B/Esc cancels. Shift+arrows
    /// leave the mode and start an ordinary selection.
    fn block_keys(&mut self, key: Key, shift: bool, ctrl: bool) -> Option<Effect> {
        self.block.as_ref()?;
        if shift && matches!(key, Key::Up | Key::Down) {
            self.block_cancel();
            return None; // fall through to the normal selection keys
        }
        match key {
            // Shift+←/→ leave the mode into a live selection: the
            // highlighted block becomes the selection and the step
            // already extends it.
            Key::Left | Key::Right if shift && !ctrl => {
                self.block_commit();
                self.select_move(key == Key::Right);
            }
            Key::Up | Key::Right if !ctrl => self.block_move(true),
            Key::Down | Key::Left if !ctrl => self.block_move(false),
            Key::Enter => self.block_commit(),
            Key::Char('b') if ctrl => self.block_cancel(),
            _ => self.block_cancel(),
        }
        Some(Effect::None)
    }

    /// Free-cursor mode: arrows move the cell cursor, Enter snaps.
    fn free_keys(&mut self, key: Key, ctrl: bool) -> Option<Effect> {
        self.free.as_ref()?;
        let _ = ctrl;
        match key {
            Key::Left => self.free_move(-1, 0),
            Key::Right => self.free_move(1, 0),
            Key::Up => self.free_move(0, -1),
            Key::Down => self.free_move(0, 1),
            Key::Enter => self.free_confirm(),
            _ => self.free_cancel(),
        }
        Some(Effect::None)
    }

    /// Minibuffer (`\command`) mode captures most keys.
    fn minibuffer_keys(&mut self, key: Key) -> Option<Effect> {
        self.minibuffer.is_some().then(|| {
            match key {
                Key::Esc => self.minibuffer = None,
                Key::Backspace => {
                    let buf = self.minibuffer.as_mut().unwrap();
                    if buf.pop().is_none() {
                        self.minibuffer = None;
                    }
                }
                Key::Enter | Key::Char(' ') => {
                    let cmd = self.minibuffer.take().unwrap();
                    if cmd.is_empty() && key == Key::Char(' ') {
                        // \ followed by Space: the meaningful space ␣.
                        self.execute("space");
                    } else {
                        self.execute(&cmd);
                    }
                }
                // Graphic chars (not just alphanumerics): the symbol
                // table has names like "->", "+-", "oo".
                Key::Char(c) if c.is_ascii_graphic() => {
                    self.minibuffer.as_mut().unwrap().push(c);
                }
                _ => {}
            }
            Effect::None
        })
    }

    /// \op name box: printable keys build the name; any key that is not
    /// part of it commits first, then falls through to the base layer.
    /// Space separates band pieces in \op* (`ess sup` → ┈ess┈sup┈) but
    /// simply commits a plain \op (one word is the whole name there).
    fn op_box_keys(&mut self, key: Key, ctrl: bool) -> Option<Effect> {
        let kind = self.op_entry.as_ref()?.0;
        // What may be typed into the box: operator/roman names are
        // alphanumerics plus dots (i.i.d.); \text takes any glyph but
        // the quotes. Space separates \op* pieces, is content in \rm
        // and \text, and commits a plain \op (one-word name).
        let name_char = |c: char| c.is_ascii_alphanumeric() || c == '.';
        // Inside \text, a backslash escapes the next key (so a literal
        // " or \ can be typed); a bare " closes the box. Brackets stay
        // out entirely — quoted content is opaque to the delimiter
        // depth scans, so ( ) [ ] { } inside would desync them.
        // Combining strike overlays are also barred: the parser hard-
        // rejects them (one char = one cell), so letting a paste smuggle
        // one into a Text would trip the roundtrip guard on every edit.
        let text_char = |c: char| !"()[]{}".contains(c) && !matches!(c, '\u{338}' | '\u{336}');
        if kind == BoxKind::Text && self.op_escape {
            self.op_escape = false;
            if let Key::Char(c) = key
                && !ctrl
                && text_char(c)
            {
                self.op_type(c);
                return Some(Effect::None);
            }
        }
        match key {
            Key::Esc => self.op_entry = None,
            Key::Backspace => self.op_backspace(),
            Key::Delete => self.op_delete(),
            Key::Home => while self.op_move(-1) {},
            Key::End => while self.op_move(1) {},
            // ←/→ move inside the box; stepping past either edge
            // commits it and lets the arrow act on the formula.
            Key::Left | Key::Right => {
                let delta = if key == Key::Left { -1 } else { 1 };
                if !self.op_move(delta) {
                    self.op_commit();
                    return None;
                }
            }
            // Space is content only where it means something: \op*
            // pieces, \text prose, \tex source. \op and \rm commit.
            Key::Char(' ') if matches!(kind, BoxKind::OpStar | BoxKind::Text | BoxKind::Tex) => {
                self.op_type(' ')
            }
            Key::Char('\\') if !ctrl && kind == BoxKind::Text => self.op_escape = true,
            Key::Char('"') if !ctrl && kind == BoxKind::Text => self.op_commit(),
            Key::Char(c) if !ctrl && kind == BoxKind::Text && text_char(c) => self.op_type(c),
            // The \tex box takes any printable input (LaTeX is pasted
            // into it verbatim; Enter commits).
            Key::Char(c)
                if !ctrl
                    && kind == BoxKind::Tex
                    && !c.is_control()
                    && !matches!(c, '\u{338}' | '\u{336}') =>
            {
                self.op_type(c)
            }
            Key::Char(c) if !ctrl && kind != BoxKind::Text && name_char(c) => self.op_type(c),
            Key::Enter | Key::Tab | Key::Char(' ') => self.op_commit(),
            _ => {
                self.op_commit();
                return None;
            }
        }
        Some(Effect::None)
    }

    /// Grid edit mode (^O): a key layer for matrix surgery. The ctrl
    /// chords are handled ahead of it (grid mode swallows bare c/r for
    /// its column/row submodes), and the mode ends when the cursor
    /// leaves the grid (click, …).
    fn grid_keys(&mut self, key: Key, shift: bool, ctrl: bool) -> Option<Effect> {
        let gs = self.grid?;
        if ctrl {
            return None;
        }
        if self.enclosing_array().is_none() {
            self.grid = None;
            return None;
        }
        // Ghost slots survive only until the next real input — this
        // layer consumes its keys, so it owes the same clear the base
        // layer does (a stale ghost path outlives the grid surgery
        // that shrank the array under it).
        self.ghost.clear();
        self.clear_message();
        match gs {
            crate::editor::GridSel::Cells { .. } => match key {
                Key::Left if shift => self.grid_select_move(0, -1),
                Key::Right if shift => self.grid_select_move(0, 1),
                Key::Up if shift => self.grid_select_move(-1, 0),
                Key::Down if shift => self.grid_select_move(1, 0),
                Key::Left => self.grid_cell_move(0, -1),
                Key::Right => self.grid_cell_move(0, 1),
                Key::Up => self.grid_cell_move(-1, 0),
                Key::Down => self.grid_cell_move(1, 0),
                Key::Char('c') | Key::Char('|') => self.grid_lanes(true),
                Key::Char('r') | Key::Char('-') => self.grid_lanes(false),
                Key::Backspace | Key::Delete => self.grid_clear_cells(),
                // Enter: leave the mode and edit this cell.
                Key::Enter | Key::Esc | Key::Tab => self.grid = None,
                // Any other key: the help line already spells
                // the mode's keys (the key layer does not
                // author user-facing text).
                _ => {}
            },
            crate::editor::GridSel::Lanes { cols, .. } => {
                let (fwd, back) = if cols {
                    (Key::Right, Key::Left)
                } else {
                    (Key::Down, Key::Up)
                };
                match key {
                    k if k == fwd && shift => self.lane_extend(1),
                    k if k == back && shift => self.lane_extend(-1),
                    k if k == fwd => self.lane_step(1),
                    k if k == back => self.lane_step(-1),
                    // The cross-axis arrows drop back to cells.
                    Key::Left | Key::Right | Key::Up | Key::Down => self.lane_demote(),
                    Key::Enter => self.lane_commit(),
                    Key::Backspace | Key::Delete | Key::Char('d') => self.lane_delete_sel(),
                    Key::Char('c') | Key::Char('|') if !cols => self.grid_lanes(true),
                    Key::Char('r') | Key::Char('-') if cols => self.grid_lanes(false),
                    // Esc leaves grid mode altogether; the
                    // same-axis letter drops back to cells.
                    Key::Esc | Key::Tab => self.grid = None,
                    Key::Char('c') | Key::Char('|') | Key::Char('r') | Key::Char('-') => {
                        self.grid = Some(crate::editor::GridSel::Cells { anchor: None })
                    }
                    _ => {}
                }
            }
        }
        Some(Effect::None)
    }

    /// Base layer: ctrl chords, then ordinary keys.
    fn base_keys(&mut self, key: Key, shift: bool, ctrl: bool) -> Effect {
        // Ghost slots survive only until the next real input.
        self.ghost.clear();
        if ctrl {
            match key {
                Key::Char('q') => return Effect::Quit,
                Key::Char('y') => return Effect::CopyAa,
                Key::Char('a') => self.document_start(),
                Key::Char('f') => self.start_free(),
                Key::Char('b') => self.start_block_select(),
                // In grid mode, copy/cut act on the cell rectangle;
                // paste routes by clipboard shape inside `paste`.
                Key::Char('c') if self.grid.is_some() => self.grid_copy_cells(),
                Key::Char('x') if self.grid.is_some() => self.grid_cut_cells(),
                Key::Char('c') => self.copy_selection(),
                Key::Char('x') => self.cut_selection(),
                Key::Char('v') => self.paste(),
                // Emacs pairing: ^A start, ^E end of the formula.
                Key::Char('e') => self.document_end(),
                // ^T: grid edit mode (inside a matrix).
                Key::Char('t') => self.grid_mode_toggle(),
                _ => {}
            }
            return Effect::None;
        }

        self.clear_message();
        match key {
            Key::Left if shift => self.select_move(false),
            Key::Right if shift => self.select_move(true),
            Key::Up if shift => self.select_parent(),
            // With an active selection, ←/→ collapse onto its ends.
            Key::Left => match self.selection() {
                Some((lo, _)) => {
                    self.select_anchor = None;
                    self.col = lo;
                }
                None => {
                    self.select_anchor = None;
                    self.left();
                }
            },
            Key::Right => match self.selection() {
                Some((_, hi)) => {
                    self.select_anchor = None;
                    self.col = hi;
                }
                None => {
                    self.select_anchor = None;
                    self.right();
                }
            },
            Key::Up => {
                self.select_anchor = None;
                self.vertical(true);
            }
            Key::Down => {
                self.select_anchor = None;
                self.vertical(false);
            }
            // Esc peels the selection first; with nothing left to
            // cancel it quits (terminals often swallow ^Q, so Esc is
            // the reliable way out).
            Key::Esc => {
                if self.select_anchor.is_some() {
                    self.select_anchor = None;
                } else {
                    return Effect::Quit;
                }
            }
            Key::Home => self.home(),
            Key::End => self.end(),
            Key::Backspace => {
                if !self.delete_selection() {
                    self.backspace();
                }
            }
            Key::Delete => {
                if !self.delete_selection() {
                    self.delete();
                }
            }
            Key::Char('\\') => self.minibuffer = Some(String::new()),
            // ^ / _ / ( { [ spell the same Edits the \commands resolve
            // to: an empty template enters its slot, a selection lands
            // inside it (`apply` owns both readings).
            Key::Char('^') => self.apply(Edit::Insert {
                node: Node::Sup { arg: vec![] },
                wrap: true,
            }),
            Key::Char('_') => self.apply(Edit::Insert {
                node: Node::Sub { arg: vec![] },
                wrap: true,
            }),
            Key::Char('(') => self.apply(Edit::Insert {
                node: Node::Delim {
                    left: Delim::Col(ColDelim::Paren),
                    right: Delim::Col(ColDelim::Paren),
                    mids: 0,
                    segs: vec![vec![]],
                },
                wrap: true,
            }),
            Key::Char(')') => self.close_paren(),
            Key::Char('{') => self.apply(Edit::Insert {
                node: Node::Delim {
                    left: Delim::Col(ColDelim::Brace),
                    right: Delim::Col(ColDelim::Brace),
                    mids: 0,
                    segs: vec![vec![]],
                },
                wrap: true,
            }),
            Key::Char('}') => self.close_brace(),
            Key::Char('[') => self.apply(Edit::Insert {
                node: Node::Delim {
                    left: Delim::Col(ColDelim::Bracket),
                    right: Delim::Col(ColDelim::Bracket),
                    mids: 0,
                    segs: vec![vec![]],
                },
                wrap: true,
            }),
            // The quote glyphs are reserved for roman/text runs; the
            // prime is its own atom (\prime, ′ U+2032).
            Key::Char('\'') => self.apply(Edit::Sym('′')),
            Key::Char('"') => {
                // Text mode: the box commits on the closing " (like
                // typing the quoted run directly); \" escapes.
                self.apply(Edit::OpenBox(BoxKind::Text));
            }
            Key::Char(']') => self.close_bracket(),
            // `//` makes a fraction (a lone `/` stays the slash atom).
            Key::Char('/') => self.slash(),
            Key::Tab => self.exit_inset(),
            // Enter at the top level: a formula line break. Inside an
            // inset it does nothing (grid rows are added in ^T grid
            // mode or with \addrow).
            Key::Enter => {
                if self.path.is_empty() {
                    self.break_line();
                }
            }
            // Space is a formatting space (Tab leaves insets; \space gives
            // the semantic ␣ atom).
            Key::Char(' ') => {
                self.apply(Edit::Insert {
                    node: Node::Spacer,
                    wrap: false,
                });
            }
            // The same allow-list the parser uses: a key that is not a
            // valid atom (`~` `^` `\`, or anything non-ASCII) would build
            // a formula that cannot be read back.
            Key::Char(c) if crate::symbols::is_atom(c) => {
                // The anchor survives into `apply`: a plain symbol
                // replaces an active selection there.
                self.apply(Edit::Sym(c));
            }
            _ => {}
        }
        Effect::None
    }
}
