//! Host-independent keymap: the single place where keystrokes are mapped
//! onto editor operations. The TUI (crossterm) and the wasm bindings
//! (DOM KeyboardEvent) only translate their native events into [`Key`]
//! and dispatch through [`Editor::input`], so the two frontends cannot
//! drift apart.

use crate::ast::Node;
use crate::editor::{Editor, JUMP_LABELS};

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
    /// Write the current LaTeX to the host's save target.
    SaveTex,
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
        effect
    }

    /// Key layers, outermost first: each modal layer either consumes
    /// the key (Some) or lets it fall through (None). Adding a mode =
    /// adding one handler here.
    fn dispatch(&mut self, key: Key, shift: bool, ctrl: bool) -> Effect {
        if let Some(e) = self.free_keys(key, shift, ctrl) {
            return e;
        }
        if let Some(e) = self.jump_keys(key, ctrl) {
            return e;
        }
        if let Some(e) = self.block_keys(key, ctrl) {
            return e;
        }
        if let Some(e) = self.minibuffer_keys(key) {
            return e;
        }
        if let Some(e) = self.op_box_keys(key, ctrl) {
            return e;
        }
        self.base_keys(key, shift, ctrl)
    }

    /// Jump mode: the next key picks a label (anything else cancels).
    fn jump_keys(&mut self, key: Key, ctrl: bool) -> Option<Effect> {
        self.jump.is_some().then(|| {
            match key {
                Key::Char(c) if !ctrl => self.jump_to(c),
                // Arrow keys move the marker selection; Enter confirms.
                Key::Left => self.jump_select(-1, 0),
                Key::Right => self.jump_select(1, 0),
                Key::Up => self.jump_select(0, -1),
                Key::Down => self.jump_select(0, 1),
                Key::Enter => self.jump_confirm(),
                _ => {
                    if let Some(targets) = self.jump.take() {
                        self.keep_ghosts(&targets);
                    }
                    self.message.clear();
                }
            }
            Effect::None
        })
    }

    /// Block-select mode: the next key picks a block label.
    fn block_keys(&mut self, key: Key, ctrl: bool) -> Option<Effect> {
        self.block.is_some().then(|| {
            match key {
                Key::Char(c) if !ctrl => self.block_to(c),
                _ => {
                    self.block = None;
                    self.message.clear();
                }
            }
            Effect::None
        })
    }

    /// Free-cursor mode: arrows move the cell cursor, Enter snaps.
    /// ^G toggles jump markers; while they are up, a label letter or
    /// the arrows+Enter jump there and free motion continues.
    fn free_keys(&mut self, key: Key, _shift: bool, ctrl: bool) -> Option<Effect> {
        self.free.as_ref()?;
        let markers = self.jump.is_some();
        match key {
            Key::Char('g') if ctrl => self.free_toggle_markers(),
            Key::Char(c) if markers && !ctrl && JUMP_LABELS.contains(c) => self.free_jump(c),
            Key::Left if markers => self.jump_select(-1, 0),
            Key::Right if markers => self.jump_select(1, 0),
            Key::Up if markers => self.jump_select(0, -1),
            Key::Down if markers => self.jump_select(0, 1),
            Key::Enter if markers => self.free_goto_selected(),
            Key::Esc if markers => self.free_markers_off(),
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
                    self.execute(&cmd);
                }
                // Graphic chars (not just alphanumerics): the extended
                // symbol table has names like "->", "+-", "oo".
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
        use crate::editor::BoxKind;
        let kind = self.op_entry.as_ref()?.0;
        // What may be typed into the box: operator/roman names are
        // alphanumerics plus dots (i.i.d.); \text takes any glyph but
        // the quotes. Space separates \op* pieces, is content in \rm
        // and \text, and commits a plain \op (one-word name).
        let name_char = |c: char| c.is_ascii_alphanumeric() || c == '.';
        match key {
            Key::Esc => self.op_entry = None,
            Key::Backspace => self.op_backspace(),
            Key::Char(' ') if kind != BoxKind::Op => self.op_type(' '),
            Key::Char(c) if !ctrl && kind == BoxKind::Text && c != '"' && c != '\'' => {
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

    /// Base layer: ctrl chords, the grid-edit layer, then ordinary keys.
    fn base_keys(&mut self, key: Key, shift: bool, ctrl: bool) -> Effect {
        // Ghost slots survive only until the next real input; ^G itself
        // re-labels the identical picture.
        if !(ctrl && key == Key::Char('g')) {
            self.ghost.clear();
        }
        if ctrl {
            match key {
                Key::Char('q') => return Effect::Quit,
                Key::Char('s') => return Effect::SaveTex,
                Key::Char('y') => return Effect::CopyAa,
                Key::Char('a') => self.document_start(),
                Key::Char('f') => self.start_free(),
                Key::Char('g') => self.start_jump(),
                Key::Char('b') => self.start_block_select(),
                Key::Char('t') => self.italic = !self.italic,
                Key::Char('c') => self.copy_selection(),
                Key::Char('x') => self.cut_selection(),
                Key::Char('v') => self.paste(),
                // Emacs pairing: ^A start, ^E end of the formula.
                Key::Char('e') => self.document_end(),
                // ^O: grid edit mode (inside a matrix).
                Key::Char('o') => self.grid_mode_toggle(),
                _ => {}
            }
            return Effect::None;
        }

        // Grid edit mode (^E): a key layer for matrix surgery. Ctrl
        // chords above still work; the mode ends when the cursor leaves
        // the grid (jump, click, …).
        if self.grid_mode {
            if self.enclosing_array().is_none() {
                self.grid_mode = false;
            } else {
                self.message.clear();
                match key {
                    Key::Left => self.grid_move(0, -1),
                    Key::Right => self.grid_move(0, 1),
                    Key::Up => self.grid_move(-1, 0),
                    Key::Down => self.grid_move(1, 0),
                    Key::Enter | Key::Char('r') => self.add_row(),
                    Key::Char('R') => self.add_row_above(),
                    Key::Char('c') => self.add_col(),
                    Key::Char('C') => self.add_col_left(),
                    Key::Char('d') => self.del_row(),
                    Key::Char('D') => self.del_col(),
                    Key::Esc | Key::Tab => self.grid_mode = false,
                    _ => {
                        self.message =
                            "grid mode: ←→↑↓ move cells  r/R add row  c/C add col  d/D delete row/col  Esc exit"
                                .into();
                    }
                }
                return Effect::None;
            }
        }

        self.message.clear();
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
            Key::Char('^') => {
                if !self.wrap_selection(|c| Node::Sup { arg: c }) {
                    self.insert_and_enter(Node::Sup { arg: vec![] });
                }
            }
            Key::Char('_') => {
                if !self.wrap_selection(|c| Node::Sub { arg: c }) {
                    self.insert_and_enter(Node::Sub { arg: vec![] });
                }
            }
            Key::Char('(') => {
                if !self.wrap_selection(|c| Node::Delim {
                    left: '(',
                    right: ')',
                    mids: vec![],
                    segs: vec![c],
                }) {
                    self.insert_delim('(', ')', vec![]);
                }
            }
            Key::Char(')') => self.close_paren(),
            Key::Char('{') => {
                if !self.wrap_selection(|c| Node::Delim {
                    left: '{',
                    right: '}',
                    mids: vec![],
                    segs: vec![c],
                }) {
                    self.insert_delim('{', '}', vec![]);
                }
            }
            Key::Char('}') => self.close_brace(),
            Key::Char('[') => {
                if !self.wrap_selection(|c| Node::Delim {
                    left: '[',
                    right: ']',
                    mids: vec![],
                    segs: vec![c],
                }) {
                    self.insert_delim('[', ']', vec![]);
                }
            }
            Key::Char('"') => {
                self.message = "\" is reserved for text runs; use \\rm<text> or \\text<text>".into()
            }
            Key::Char(']') => self.close_bracket(),
            // `//` makes a fraction (a lone `/` stays the slash atom).
            Key::Char('/') => self.slash(),
            Key::Tab => self.exit_inset(),
            // Enter inside a grid: new row below (like LyX table
            // editing); at the top level: a formula line break.
            Key::Enter => {
                if self.path.is_empty() {
                    self.break_line();
                } else {
                    self.add_row();
                }
            }
            // Space is a formatting space (Tab leaves insets; \space gives
            // the semantic ␣ atom).
            Key::Char(' ') => {
                self.select_anchor = None;
                self.insert_spacer();
            }
            Key::Char(c) if c.is_ascii_graphic() => {
                self.select_anchor = None;
                self.insert_sym(c);
            }
            _ => {}
        }
        Effect::None
    }
}
