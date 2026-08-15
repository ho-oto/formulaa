//! Host-independent keymap: the single place where keystrokes are mapped
//! onto editor operations. The TUI (crossterm) and the wasm bindings
//! (DOM KeyboardEvent) only translate their native events into [`Key`]
//! and dispatch through [`Editor::input`], so the two frontends cannot
//! drift apart.

use crate::ast::Node;
use crate::editor::{Ask, BoxKind, Edit, Editor};
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
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Effect {
    None,
    /// The user asked to leave. Unsaved work is the host's to notice —
    /// it can answer by asking (`Editor::ask_save_first`) instead.
    Quit,
    /// Leave, edits and all: the answer to that question was "no".
    Discard,
    /// Save the formula where it came from.
    Write,
    /// Save it under this name (the path question was answered).
    WriteTo(String),
    /// Save, then leave.
    WriteQuit,
    /// Put the canonical AA on the host's clipboard.
    CopyAa,
}

impl Editor {
    /// One keystroke of the shared keymap. Wraps the dispatch with undo
    /// bookkeeping: any key that changes the formula pushes the
    /// pre-state (undo/redo themselves are handled here so they never
    /// re-enter the history).
    pub fn input(&mut self, key: Key, shift: bool, ctrl: bool) -> Effect {
        // ^H is Backspace, the Emacs pairing for ^D's forward delete.
        // Aliased ahead of the layers, so every one of them sees the
        // key it already handles.
        let (key, ctrl) = match (key, ctrl) {
            (Key::Char('h'), true) => (Key::Backspace, false),
            pressed => pressed,
        };
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
        if let Some(e) = self.ask_keys(key, ctrl) {
            return e;
        }
        if let Some(e) = self.free_keys(key, ctrl) {
            return e;
        }
        if let Some(e) = self.block_keys(key, shift, ctrl) {
            return e;
        }
        if let Some(e) = self.minibuffer_keys(key, ctrl) {
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

    /// The status-line question, outermost of all: while one stands
    /// nothing else may act, and every key it does not use is spent on
    /// it (a stray chord must not leak into a file name). Esc drops the
    /// question, leaving whatever prompted it undone.
    fn ask_keys(&mut self, key: Key, ctrl: bool) -> Option<Effect> {
        let ask = self.ask.take()?;
        let (next, effect) = match (ask, key) {
            (_, Key::Esc) => (None, Effect::None),
            (Ask::Path(mut buf), Key::Char(c)) if !ctrl && !c.is_control() => {
                buf.push(c);
                (Some(Ask::Path(buf)), Effect::None)
            }
            (Ask::Path(mut buf), Key::Backspace) => {
                buf.pop();
                (Some(Ask::Path(buf)), Effect::None)
            }
            (Ask::Path(buf), Key::Enter) => match buf.trim() {
                // A name is the whole answer: an empty one cannot be
                // corrected into anything, so it just closes.
                "" => (None, Effect::None),
                name => (None, Effect::WriteTo(name.to_string())),
            },
            // Enter takes the default the prompt spells ([Y/n]).
            (Ask::SaveFirst, Key::Char('y' | 'Y') | Key::Enter) if !ctrl => {
                (None, Effect::WriteQuit)
            }
            (Ask::SaveFirst, Key::Char('n' | 'N')) if !ctrl => (None, Effect::Discard),
            (ask, _) => (Some(ask), Effect::None),
        };
        self.ask = next;
        Some(effect)
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

    /// Run a committed minibuffer spelling. The mode commands are
    /// the input layer's own — they move a mode, the clipboard, or
    /// the app itself, which no `Edit` can say — and everything else
    /// is the editor's `execute`.
    fn run_command(&mut self, cmd: &str) -> Effect {
        use crate::editor::ModeCmd;
        match crate::editor::mode_command(cmd) {
            Some(ModeCmd::Free) => self.start_free(),
            Some(ModeCmd::BlockSelect) => self.start_block_select(),
            Some(ModeCmd::GridEdit) => self.grid_mode_toggle(),
            Some(ModeCmd::CopyAa) => return Effect::CopyAa,
            Some(ModeCmd::Write) => return Effect::Write,
            Some(ModeCmd::WriteQuit) => return Effect::WriteQuit,
            Some(ModeCmd::Quit) => return Effect::Quit,
            None => self.execute(cmd),
        }
        Effect::None
    }

    /// Minibuffer (`\command`) mode captures most keys. Tab opens the
    /// completion popup; while it is open the arrows pick a row and
    /// Enter takes it, so the list behaves like any editor's.
    fn minibuffer_keys(&mut self, key: Key, ctrl: bool) -> Option<Effect> {
        self.minibuffer.is_some().then(|| {
            // This layer clears the status line too, or a stale "no
            // completion for \x" notice outlives the very keystroke
            // that opens a populated list.
            self.clear_message();
            let mut fx = Effect::None;
            match key {
                // Esc peels the popup first, then the minibuffer.
                Key::Esc => {
                    if self.completion.take().is_none() {
                        self.minibuffer = None;
                    }
                }
                // A step row continues the spelling rather than
                // running: `\lr` offers `\lr(…`, and taking it leaves
                // `\lr(` typed with the list asking for what comes
                // next. Both commit keys and Tab do it, because on
                // such a row "accept" can only mean "go on".
                Key::Enter | Key::Char(' ') | Key::Tab
                    if self
                        .completion
                        .as_ref()
                        .and_then(|l| l.highlighted())
                        .is_some_and(|i| i.is_step()) =>
                {
                    let next = self
                        .completion
                        .as_ref()
                        .and_then(|l| l.highlighted())
                        .and_then(|i| i.step_to().map(str::to_string))
                        .unwrap_or_default();
                    // A step that changes nothing has said all it can:
                    // `\frak`'s own row leaves `frak` typed and the
                    // rest is free input (letters, digits the list
                    // cannot enumerate), so accepting it again closes
                    // the popup — the signal that it is the keyboard's
                    // turn now — instead of rebuilding the same list.
                    if self.minibuffer.as_deref() == Some(next.as_str()) {
                        self.completion = None;
                    } else {
                        self.minibuffer = Some(next.clone());
                        self.completion = crate::complete::Completion::build(&next);
                    }
                }
                // Tab accepts: the highlighted row if a list is open,
                // else the typed name if it is already a command —
                // finishing what you started is what Tab is for. Only
                // when it is *not* a command does Tab ask for help.
                Key::Tab => {
                    // Tab never runs a mode command — leaving the
                    // formula (or the program) must be an explicit
                    // Enter, not a completion reflex.
                    let picked = self
                        .completion
                        .as_ref()
                        .and_then(|l| l.selected())
                        .and_then(|i| i.commit().map(str::to_string))
                        .filter(|c| crate::editor::mode_command(c).is_none());
                    let query = self.minibuffer.clone().unwrap_or_default();
                    // Only a spelling that runs something: `\lr` and
                    // `\matrix` print their usage instead, and Tab
                    // must ask the list about those, not accept them.
                    let ready = crate::editor::resolve(&query).is_some();
                    match picked.or_else(|| ready.then_some(query.clone())) {
                        Some(cmd) => {
                            self.completion = None;
                            self.minibuffer = None;
                            fx = self.run_command(&cmd);
                        }
                        None => match crate::complete::Completion::build(&query) {
                            Some(list) => self.completion = Some(list),
                            None if query.is_empty() => {
                                self.info("type a command name (\\frac, \\alpha, …)")
                            }
                            // No matches: silence — the popup simply
                            // does not open, which says the same.
                            None => {}
                        },
                    }
                }
                // The arrows always mean "show me the list": browsing
                // is their whole job, so they open it if it is shut.
                Key::Down | Key::Up => {
                    let opening = self.completion.is_none();
                    if opening {
                        let query = self.minibuffer.clone().unwrap_or_default();
                        self.completion = crate::complete::Completion::build(&query);
                    }
                    // The press that reveals the list leaves the first
                    // row highlighted: it asked to see the list, not to
                    // skip its first answer.
                    if let Some(list) = &mut self.completion
                        && !opening
                    {
                        list.step(key == Key::Down);
                    }
                }
                Key::Backspace => {
                    let buf = self.minibuffer.as_mut().unwrap();
                    if buf.pop().is_none() {
                        self.minibuffer = None;
                    }
                    self.refresh_completion();
                }
                // With a row highlighted, both commit keys take it —
                // the whole point of having picked one. An empty list
                // falls through to the ordinary commit below.
                Key::Enter | Key::Char(' ')
                    if self
                        .completion
                        .as_ref()
                        .and_then(|l| l.selected())
                        .is_some() =>
                {
                    let picked = self
                        .completion
                        .take()
                        .and_then(|l| l.selected().and_then(|i| i.commit()).map(str::to_string));
                    if let Some(cmd) = picked {
                        self.minibuffer = None;
                        fx = self.run_command(&cmd);
                    }
                }
                Key::Enter | Key::Char(' ') => {
                    let cmd = self.minibuffer.take().unwrap();
                    self.completion = None;
                    if cmd.is_empty() && key == Key::Char(' ') {
                        // \ followed by Space: the meaningful space ␣.
                        self.execute("space");
                    } else {
                        fx = self.run_command(&cmd);
                    }
                }
                // Graphic chars (not just alphanumerics): the symbol
                // table has names like "->", "+-", "oo". A ctrl chord
                // is not typing — it falls to the catch-all rather
                // than pushing its letter into the query.
                Key::Char(c) if !ctrl && c.is_ascii_graphic() => {
                    self.minibuffer.as_mut().unwrap().push(c);
                    self.refresh_completion();
                }
                _ => {}
            }
            fx
        })
    }

    /// A mouse pick in the completion popup: select row `idx` and
    /// accept it, exactly as Enter on that row would (a step row
    /// continues the spelling, a command row runs).
    pub fn completion_click(&mut self, idx: usize) -> Effect {
        match &mut self.completion {
            Some(list) if idx < list.items.len() => {
                list.sel = idx;
                self.input(Key::Enter, false, false)
            }
            _ => Effect::None,
        }
    }

    /// Keep an open popup in step with what has been typed. A query
    /// that matches nothing leaves the popup open but empty rather than
    /// closing it: the list was asked for, and typing a letter too many
    /// should not mean Tab has to be pressed again after backspacing.
    /// An empty list draws nothing and commits nothing.
    fn refresh_completion(&mut self) {
        if self.completion.is_none() {
            return;
        }
        self.completion = self
            .minibuffer
            .as_deref()
            .map(|q| crate::complete::Completion::build(q).unwrap_or_default());
    }

    /// \op name box: printable keys build the name; any key that is not
    /// part of it commits first, then falls through to the base layer.
    /// Space separates band pieces in \op* (`ess sup` → ┈ess┈sup┈) but
    /// simply commits a plain \op (one word is the whole name there).
    fn op_box_keys(&mut self, key: Key, ctrl: bool) -> Option<Effect> {
        let kind = self.op_entry.as_ref()?.0;
        // What may be typed into the box: operator/roman names are
        // alphanumerics plus dots (i.i.d.); \text takes any glyph but
        // the quotes.
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
            // Space is content only where it survives the commit:
            // \text prose and \latex source. In \op and \op* it
            // commits — the band name is one piece, so a typed space
            // would silently vanish from the result.
            Key::Char(' ') if matches!(kind, BoxKind::Text | BoxKind::Tex) => self.op_type(' '),
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

    /// Grid edit mode (^G): a key layer for matrix surgery. The ctrl
    /// chords are handled ahead of it (grid mode swallows bare c/r for
    /// its column/row submodes), and the mode ends when the cursor
    /// leaves the grid (click, …).
    fn grid_keys(&mut self, key: Key, shift: bool, ctrl: bool) -> Option<Effect> {
        let gs = self.grid?;
        if ctrl {
            // Ctrl chords are the base layer's (^C/^X/^V act on cells
            // through `paste`'s shape routing). ^D is the exception:
            // it is a delete, and a node-level delete inside a grid
            // would edit a cell's contents while the selection on
            // screen is a rectangle of cells.
            return match key {
                Key::Char('d') => {
                    self.grid_clear_cells();
                    Some(Effect::None)
                }
                _ => None,
            };
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
                // Enter: leave the mode with the highlighted cell as
                // the selection (a multi-cell rectangle has no linear
                // equivalent and just leaves).
                Key::Enter => {
                    let single = match gs {
                        crate::editor::GridSel::Cells { anchor } => {
                            anchor.is_none() || anchor == self.grid_info().map(|(.., c)| c)
                        }
                        _ => false,
                    };
                    if single {
                        self.grid_commit_cell();
                    } else {
                        self.grid = None;
                    }
                }
                // `\` leaves the mode the way it leaves ^F and ^B —
                // reaching for a command is reaching for the editor.
                Key::Esc | Key::Tab | Key::Char('\\') => self.grid = None,
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
                    // Esc (and `\`, like ^F/^B) leaves grid mode
                    // altogether; the same-axis letter drops back to
                    // cells.
                    Key::Esc | Key::Tab | Key::Char('\\') => self.grid = None,
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
        // A wrapper armed for unwrapping survives exactly one
        // keystroke: taking it here disarms on everything except the
        // key that continues the armed gesture — Backspace/^D pressed
        // again in the same place, a delete on a pair armed by
        // Shift-selection, or the Shift+arrow that moves past it.
        let armed_at = self.unwrap_armed.take();
        let armed = armed_at.as_deref() == Some(&self.path[..]);
        // The armed │ middle is the same one-shot: it survives into
        // exactly the delete that follows it.
        let mid_at = self
            .mid_armed
            .take()
            .filter(|(p, _)| p[..] == self.path[..])
            .map(|(_, m)| m);
        if ctrl {
            match key {
                Key::Char('q') => return Effect::Quit,
                Key::Char('y') => return Effect::CopyAa,
                Key::Char('o') => return Effect::Write,
                Key::Char('w') => return Effect::WriteQuit,
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
                Key::Char('g') => self.grid_mode_toggle(),
                // ^D: delete forward, the Emacs pairing for Backspace
                // (and the only forward delete on keyboards without a
                // Delete key).
                Key::Char('d') => {
                    if let Some(m) = mid_at {
                        self.merge_mid(m);
                    } else if !self.unwrap_armed_outside(&armed_at) {
                        self.delete_forward(armed)
                    }
                }
                _ => {}
            }
            return Effect::None;
        }

        self.clear_message();
        match key {
            // Shift onto a wrapper arms its pair first (see
            // `select_arm`); the ordinary step is everything else.
            Key::Left if shift => {
                if !self.select_arm(false, &armed_at) {
                    self.select_move(false)
                }
            }
            Key::Right if shift => {
                if !self.select_arm(true, &armed_at) {
                    self.select_move(true)
                }
            }
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
            // Deleting toward the delimiter you are standing just
            // inside offers to unwrap it (arm, then lift the contents
            // out) instead of stepping over it.
            Key::Backspace => {
                if let Some(m) = mid_at {
                    self.merge_mid(m);
                } else if !self.delete_selection()
                    && !self.unwrap_armed_outside(&armed_at)
                    && !self.delete_toward_delim(true, armed)
                {
                    self.backspace();
                }
            }
            Key::Delete => {
                if let Some(m) = mid_at {
                    self.merge_mid(m);
                } else if !self.unwrap_armed_outside(&armed_at) {
                    self.delete_forward(armed)
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
            // inset it does nothing (grid rows are added in ^G grid
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
