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
use mascii::render::{render_row, RenderCtx};
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

/// AA text -> canonical AA.
#[wasm_bindgen]
pub fn aa_format(text: &str) -> Result<String, JsError> {
    let row = parse_or_err(text)?;
    Ok(render_row(&row, None, false, &RenderCtx::canonical()).to_text())
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
            italic: self.ed.italic,
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

    /// Handle one key. `key` uses KeyboardEvent.key names for specials
    /// ("ArrowLeft", "Backspace", "Enter", "Escape", "Home", "End",
    /// "Delete", "Tab", " ") and single characters otherwise. Dispatches
    /// through the shared library keymap (`mascii::input`), so the
    /// bindings cannot drift from the TUI. Host effects (save/copy/quit)
    /// do not apply here and are ignored.
    pub fn key(&mut self, key: &str, shift: bool) {
        use mascii::input::Key;
        let key = match key {
            "ArrowLeft" => Key::Left,
            "ArrowRight" => Key::Right,
            "ArrowUp" => Key::Up,
            "ArrowDown" => Key::Down,
            "Home" => Key::Home,
            "End" => Key::End,
            "Enter" => Key::Enter,
            "Backspace" => Key::Backspace,
            "Delete" => Key::Delete,
            "Escape" => Key::Esc,
            "Tab" => Key::Tab,
            k => match k.chars().next() {
                Some(c) if k.chars().count() == 1 => Key::Char(c),
                _ => return,
            },
        };
        let _ = self.ed.input(key, shift, false);
    }
}

impl Default for MasciiEditor {
    fn default() -> Self {
        Self::new()
    }
}
