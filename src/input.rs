//! Host-independent keymap: the single place where keystrokes are mapped
//! onto editor operations. The TUI (crossterm) and the wasm bindings
//! (DOM KeyboardEvent) only translate their native events into [`Key`]
//! and dispatch through [`Editor::input`], so the two frontends cannot
//! drift apart.

use crate::ast::Node;
use crate::editor::Editor;

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
    /// Dispatch one keystroke of the shared LyX-style keymap.
    pub fn input(&mut self, key: Key, shift: bool, ctrl: bool) -> Effect {
        // Jump mode: the next key picks a label (anything else cancels).
        if self.jump.is_some() {
            match key {
                Key::Char(c) if !ctrl => self.jump_to(c),
                _ => {
                    self.jump = None;
                    self.message.clear();
                }
            }
            return Effect::None;
        }

        // Minibuffer (`\command`) mode captures most keys.
        if self.minibuffer.is_some() {
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
            return Effect::None;
        }

        if ctrl {
            match key {
                Key::Char('q') | Key::Char('c') => return Effect::Quit,
                Key::Char('s') => return Effect::SaveTex,
                Key::Char('y') => return Effect::CopyAa,
                Key::Char('g') => self.start_jump(),
                Key::Char('t') => self.italic = !self.italic,
                Key::Char('b') => self.highlight = !self.highlight,
                Key::Char('o') => self.structure = !self.structure,
                _ => {}
            }
            return Effect::None;
        }

        self.message.clear();
        match key {
            Key::Left if shift => self.select_move(false),
            Key::Right if shift => self.select_move(true),
            Key::Left => {
                self.select_anchor = None;
                self.left();
            }
            Key::Right => {
                self.select_anchor = None;
                self.right();
            }
            Key::Up => {
                self.select_anchor = None;
                self.vertical(true);
            }
            Key::Down => {
                self.select_anchor = None;
                self.vertical(false);
            }
            Key::Esc => self.select_anchor = None,
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
                self.message = "[ ] are reserved for matrices; insert one with \\matrix".into()
            }
            Key::Char('"') => {
                self.message = "\" is reserved for text runs; use \\rm<text> or \\text<text>".into()
            }
            Key::Char(']') => self.close_bracket(),
            Key::Tab => self.exit_inset(),
            // Enter inside a grid: new row below (like LyX table editing).
            Key::Enter => self.add_row(),
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
