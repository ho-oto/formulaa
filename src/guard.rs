//! The live roundtrip check: after every edit the canonical AA is
//! re-parsed and compared with the formula it came from. A mismatch is
//! a renderer/parser bug, and the edit that caused it is refused
//! (undone) — a formula that cannot survive its own file format must
//! not be handed to the user as if it could. The message names the
//! failure, which with the picture on screen is what a bug report
//! needs; nothing is written to disk.

use formulaa::editor::Editor;
use formulaa::render::{RenderCtx, render_root};
use formulaa::{ast, parse};

#[derive(Default)]
pub struct RoundtripGuard {
    /// The tree the last check ran on. The full re-render + re-parse
    /// is O(document); a keystroke that only moved the cursor changes
    /// nothing worth re-checking, and navigation is most keystrokes.
    checked: Option<ast::Row>,
}

impl RoundtripGuard {
    pub fn check(&mut self, ed: &mut Editor) {
        if self.checked.as_ref() == Some(&ed.root) {
            return;
        }
        self.checked = Some(ed.root.clone());
        let row = ast::normalize(&ed.root);
        if row.is_empty() {
            return;
        }
        let ctx = RenderCtx::canonical();
        let aa = render_root(&row, None, &ctx).to_text();
        // Formatting spacers survive in the AA but vanish on reparse.
        let row = ast::normalize(&ast::strip_spacers(&row));
        let kind = match parse::parse(&aa) {
            Err(e) => format!("parse error: {}", e),
            Ok(p) if p != row => "AST mismatch".into(),
            Ok(p) => {
                let aa2 = render_root(&p, None, &ctx).to_text();
                if aa2 == render_root(&row, None, &ctx).to_text() {
                    return; // roundtrip holds
                }
                "re-render mismatch".into()
            }
        };
        // Refuse the edit: the state before it is the last one whose
        // picture reads back, so it is the one to stand on.
        ed.undo();
        self.checked = Some(ed.root.clone());
        ed.error(format!(
            "⚠ edit refused — it would break the AA roundtrip ({})",
            kind
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use formulaa::input::Key;

    /// A roundtrip-breaking edit is refused: the guard undoes it and
    /// says so. The broken tree is fabricated directly (every real
    /// edit path is supposed to keep the roundtrip — that is the
    /// point), sitting on top of a real undo point to fall back to.
    #[test]
    fn a_breaking_edit_is_refused() {
        let mut ed = Editor::new();
        ed.input(Key::Char('a'), false, false);
        ed.input(Key::Char('b'), false, false);
        // A bare Roman '(' cannot roundtrip: it renders as the one
        // character every parser must read as a delimiter.
        ed.root = vec![formulaa::ast::Node::Roman('(')];
        let mut guard = RoundtripGuard::default();
        guard.check(&mut ed);
        assert_ne!(
            ed.root,
            vec![formulaa::ast::Node::Roman('(')],
            "the broken edit stood"
        );
        assert!(ed.message.contains("refused"), "{:?}", ed.message);
    }
}
