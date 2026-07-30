//! WASM bindings for mascii.
//!
//! Two layers:
//! - stateless converters (`aa_to_latex`, `aa_format`)
//! - `MasciiEditor`: the structural editor driven by a key API, for
//!   embedding the TUI-equivalent editing experience in VSCode/Obsidian
//!   webviews (see editors/ in the repository).

use mascii::ast::normalize;
use mascii::editor::{
    Editor, BLK_CLOSE, JUMP_CHAR_BASE, JUMP_LABELS, JUMP_RANK_BASE, SEL_CLOSE, SEL_OPEN,
};
use mascii::latex::row_to_latex;
use mascii::parse::parse;
use mascii::render::{render_root, RenderCtx, CURSOR_CHAR};
use wasm_bindgen::prelude::*;

fn parse_or_err(text: &str) -> Result<mascii::ast::Row, JsError> {
    parse(text).map_err(|e| JsError::new(&e.to_string()))
}

/// AA text -> LaTeX.
#[wasm_bindgen]
pub fn aa_to_latex(text: &str) -> Result<String, JsError> {
    Ok(row_to_latex(&parse_or_err(text)?))
}

/// AA text -> canonical AA.
#[wasm_bindgen]
pub fn aa_format(text: &str) -> Result<String, JsError> {
    let row = parse_or_err(text)?;
    Ok(render_root(&row, None, &RenderCtx::canonical()).to_text())
}

/// LaTeX math -> canonical AA, best effort (unknown commands are
/// skipped, never an error).
#[wasm_bindgen]
pub fn latex_to_aa(text: &str) -> String {
    let row = normalize(&mascii::from_latex::row_from_latex(text));
    render_root(&row, None, &RenderCtx::canonical()).to_text()
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
/// the result back with `aa()` / `latex()`.
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
        let block = render_root(&root, cursor_ref, &ctx);
        // Caret and decorations are zero-width metadata; the text screen
        // draws them over the glyphs (▌ caret, ⟦ ⟧ selection ends,
        // a/s/d… jump and block labels).
        // Cell grid (no embedded combining strikes): overlay coords are
        // cell coords, so strikes must not occupy indices. They are
        // re-applied when the screen string is assembled below.
        let mut lines: Vec<Vec<char>> = block.lines.clone();
        if lines.is_empty() {
            lines.push(Vec::new());
        }
        let put = |lines: &mut Vec<Vec<char>>, r: usize, c: usize, ch: char| {
            let row = &mut lines[r];
            if c >= row.len() {
                row.resize(c + 1, ' ');
            }
            row[c] = ch;
        };
        use mascii::editor::{
            CELLS_CLOSE, CELLS_OPEN, GRID_GAP, GRID_GAP_ROW, LANE_CLOSE, LANE_OPEN, ROWLANE_CLOSE,
            ROWLANE_OPEN,
        };
        for &(r, c, m) in &block.marks {
            let ch = match m {
                SEL_OPEN | CELLS_OPEN | LANE_OPEN | ROWLANE_OPEN => '⟦',
                SEL_CLOSE | BLK_CLOSE | CELLS_CLOSE | LANE_CLOSE | ROWLANE_CLOSE => '⟧',
                // The lane-gap ghost: a visible insert cursor even on
                // the colorless text screen.
                GRID_GAP | GRID_GAP_ROW => '◇',
                m => {
                    let u = m as u32;
                    let idx = if (JUMP_RANK_BASE..JUMP_RANK_BASE + 0x400).contains(&u) {
                        (u - JUMP_RANK_BASE) as usize
                    } else {
                        u.wrapping_sub(JUMP_CHAR_BASE) as usize
                    };
                    match JUMP_LABELS.chars().nth(idx) {
                        Some(l) => l,
                        // Ghost slots / unlabeled markers: no overlay.
                        None => continue,
                    }
                }
            };
            put(&mut lines, r, c, ch);
        }
        if let Some((r, c)) = block.caret {
            // Open minibuffer: the typed `\command` overlays the cells
            // right of the cursor (zero layout shift), caret after it.
            if let Some(buf) = &self.ed.minibuffer {
                let mut x = c;
                for ch in std::iter::once('\\').chain(buf.chars()) {
                    put(&mut lines, r, x, ch);
                    x += 1;
                }
                put(&mut lines, r, x, CURSOR_CHAR);
                // Symbol-command preview: the char a commit would give.
                if let Some(p) = self.ed.command_preview() {
                    put(&mut lines, r, x + 1, p);
                }
            } else {
                put(&mut lines, r, c, CURSOR_CHAR);
            }
        }
        // Re-apply the cancel strikes cell-by-cell (skipping cells the
        // caret overlay replaced).
        let struck: std::collections::HashSet<(usize, usize)> =
            block.cancel.iter().copied().collect();
        lines
            .into_iter()
            .enumerate()
            .map(|(r, l)| {
                let mut out = String::with_capacity(l.len() * 2);
                for (c, ch) in l.into_iter().enumerate() {
                    out.push(ch);
                    if ch != CURSOR_CHAR && struck.contains(&(r, c)) {
                        out.push('\u{338}');
                    }
                }
                out
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Mouse click at cell (x, y) of the rendered screen: moves the
    /// cursor to the nearest position (probe-based nearest match).
    pub fn click(&mut self, x: usize, y: usize) {
        self.ed.click(x, y);
    }

    /// Export AA (what should be written back into the document):
    /// canonical, with the ⬚ slot marks blanked where that reads back
    /// identically.
    pub fn aa(&self) -> String {
        mascii::render::export_aa(&self.ed.root)
    }

    pub fn latex(&self) -> String {
        row_to_latex(&normalize(&self.ed.root))
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
        self.key_with(key, shift, false)
    }

    /// Like `key`, with a ctrl flag for the editing chords (^C/^X/^V
    /// copy/cut/paste, ^B block select, ^G jump, ^T italic, ^O structure). Host effects
    /// (save/copy-AA/quit) are ignored here.
    pub fn key_with(&mut self, key: &str, shift: bool, ctrl: bool) {
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
        let _ = self.ed.input(key, shift, ctrl);
    }
}

impl Default for MasciiEditor {
    fn default() -> Self {
        Self::new()
    }
}
