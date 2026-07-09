//! WASM bindings for mascii.
//!
//! Two layers:
//! - stateless converters (`aa_to_latex`, `aa_to_typst`, `aa_format`)
//! - `MasciiEditor`: the structural editor driven by a key API, for
//!   embedding the TUI-equivalent editing experience in VSCode/Obsidian
//!   webviews (see editors/ in the repository).

use mascii::ast::normalize;
use mascii::editor::{Editor, SEL_CLOSE, SEL_OPEN};
use mascii::latex::row_to_latex;
use mascii::parse::parse;
use mascii::render::{render_row, GlyphSet, RenderCtx};
use mascii::typst::row_to_typst;
use wasm_bindgen::prelude::*;

fn parse_or_err(text: &str) -> Result<mascii::ast::Row, JsError> {
    parse(text).map_err(|e| JsError::new(&e.to_string()))
}

/// AA text -> LaTeX.
#[wasm_bindgen]
pub fn aa_to_latex(text: &str) -> Result<String, JsError> {
    Ok(row_to_latex(&parse_or_err(text)?))
}

/// AA text -> Typst.
#[wasm_bindgen]
pub fn aa_to_typst(text: &str) -> Result<String, JsError> {
    Ok(row_to_typst(&parse_or_err(text)?))
}

/// AA text -> canonical (or compat, display-only) AA.
#[wasm_bindgen]
pub fn aa_format(text: &str, compat: bool) -> Result<String, JsError> {
    let row = parse_or_err(text)?;
    let ctx = if compat { RenderCtx::compat() } else { RenderCtx::canonical() };
    Ok(render_row(&row, None, false, &ctx).to_text())
}

/// Quick validity check (empty string on success, message on error).
#[wasm_bindgen]
pub fn aa_check(text: &str) -> String {
    match parse(text) {
        Ok(_) => String::new(),
        Err(e) => e.to_string(),
    }
}

/// The structural editor. Feed keys with `key()`, display `screen()`
/// (the cursor is the ▌ character; selection is wrapped in ⟦ ⟧), and read
/// the result back with `aa()` / `latex()` / `typst()`.
#[wasm_bindgen]
pub struct MasciiEditor {
    ed: Editor,
}

#[wasm_bindgen]
impl MasciiEditor {
    #[wasm_bindgen(constructor)]
    pub fn new() -> MasciiEditor {
        MasciiEditor { ed: Editor::new() }
    }

    /// Replace the content by parsing AA text (empty string clears).
    pub fn load(&mut self, aa: &str) -> Result<(), JsError> {
        let row = parse_or_err(aa)?;
        self.ed = Editor::new();
        self.ed.root = row;
        self.ed.col = self.ed.root.len();
        Ok(())
    }

    /// Editor view: canonical rendering plus cursor (▌) and selection (⟦ ⟧).
    pub fn screen(&self) -> String {
        let (root, cursor) = self.ed.decorated();
        let cursor_ref = cursor.as_ref().map(|(p, c)| (p.as_slice(), *c));
        let ctx = RenderCtx {
            italic: self.ed.italic && !self.ed.compat,
            compact: false,
            glyphs: if self.ed.compat { GlyphSet::Compat } else { GlyphSet::Unicode },
        };
        let text = render_row(&root, cursor_ref, false, &ctx).to_text();
        text.chars()
            .map(|c| match c {
                SEL_OPEN => '⟦',
                SEL_CLOSE => '⟧',
                c => c,
            })
            .collect()
    }

    /// Canonical AA (what should be written back into the document).
    pub fn aa(&self) -> String {
        let row = normalize(&self.ed.root);
        render_row(&row, None, false, &RenderCtx::canonical()).to_text()
    }

    pub fn latex(&self) -> String {
        row_to_latex(&normalize(&self.ed.root))
    }

    pub fn typst(&self) -> String {
        row_to_typst(&normalize(&self.ed.root))
    }

    pub fn message(&self) -> String {
        self.ed.message.clone()
    }

    /// Current minibuffer content, or null when closed.
    pub fn minibuffer(&self) -> Option<String> {
        self.ed.minibuffer.clone()
    }

    pub fn set_compat(&mut self, on: bool) {
        self.ed.compat = on;
    }

    /// Handle one key. `key` uses KeyboardEvent.key names for specials
    /// ("ArrowLeft", "Backspace", "Enter", "Escape", "Home", "End",
    /// "Delete", " ") and single characters otherwise. Mirrors the TUI
    /// bindings (^ _ ( ) [ ] \ and selection with shift+arrows).
    pub fn key(&mut self, key: &str, shift: bool) {
        use mascii::ast::Node;
        let ed = &mut self.ed;

        // Minibuffer captures input first.
        if ed.minibuffer.is_some() {
            match key {
                "Escape" => ed.minibuffer = None,
                "Backspace" => {
                    let buf = ed.minibuffer.as_mut().unwrap();
                    if buf.pop().is_none() {
                        ed.minibuffer = None;
                    }
                }
                "Enter" | " " => {
                    let cmd = ed.minibuffer.take().unwrap();
                    ed.execute(&cmd);
                }
                k if k.chars().count() == 1 => {
                    let c = k.chars().next().unwrap();
                    if c.is_ascii_graphic() {
                        ed.minibuffer.as_mut().unwrap().push(c);
                    }
                }
                _ => {}
            }
            return;
        }

        ed.message.clear();
        match key {
            "ArrowLeft" if shift => ed.select_move(false),
            "ArrowRight" if shift => ed.select_move(true),
            "ArrowLeft" => {
                ed.select_anchor = None;
                ed.left();
            }
            "ArrowRight" => {
                ed.select_anchor = None;
                ed.right();
            }
            "ArrowUp" => {
                ed.select_anchor = None;
                ed.vertical(true);
            }
            "ArrowDown" => {
                ed.select_anchor = None;
                ed.vertical(false);
            }
            "Escape" => ed.select_anchor = None,
            "Home" => ed.home(),
            "End" => ed.end(),
            "Backspace" => {
                if !ed.delete_selection() {
                    ed.backspace();
                }
            }
            "Delete" => {
                if !ed.delete_selection() {
                    ed.delete();
                }
            }
            "\\" => ed.minibuffer = Some(String::new()),
            "^" => {
                if !ed.wrap_selection(|c| Node::Sup { arg: c }) {
                    ed.insert_and_enter(Node::Sup { arg: vec![] });
                }
            }
            "_" => {
                if !ed.wrap_selection(|c| Node::Sub { arg: c }) {
                    ed.insert_and_enter(Node::Sub { arg: vec![] });
                }
            }
            "(" => {
                if !ed.wrap_selection(|c| Node::Paren { inner: c }) {
                    ed.insert_and_enter(Node::Paren { inner: vec![] });
                }
            }
            ")" => ed.close_paren(),
            "[" => ed.message = "[ ] are reserved for matrices; use \\matrix".into(),
            "]" => ed.close_bracket(),
            " " => ed.exit_inset(),
            k if k.chars().count() == 1 => {
                let c = k.chars().next().unwrap();
                if c.is_ascii_graphic() {
                    ed.select_anchor = None;
                    ed.insert_sym(c);
                }
            }
            _ => {}
        }
    }
}

impl Default for MasciiEditor {
    fn default() -> Self {
        Self::new()
    }
}
