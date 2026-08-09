//! The live roundtrip check: after every edit the canonical AA is
//! re-parsed and compared with the formula it came from. A mismatch is
//! a renderer/parser bug. By default the edit that caused it is simply
//! refused (undone) — a formula that cannot survive its own file
//! format must not be handed to the user as if it could. With
//! `--debug` the broken state stands and a report is dumped instead,
//! which is the mode for fixing the toolchain.

use std::fs;

use mascii::editor::Editor;
use mascii::render::{RenderCtx, render_root};
use mascii::{ast, latex, parse};

/// The checker; with `debug`, reports land in
/// mascii_debug/roundtrip-N.txt so an AI (or human) can load one later
/// and fix the toolchain.
#[derive(Default)]
pub struct RoundtripGuard {
    /// `--debug`: keep the broken state and write the report.
    pub debug: bool,
    /// Last AA already reported (avoid one file per keystroke).
    reported: Option<String>,
    /// The tree the last check ran on. The full re-render + re-parse
    /// is O(document); a keystroke that only moved the cursor changes
    /// nothing worth re-checking, and navigation is most keystrokes.
    checked: Option<ast::Row>,
}

impl RoundtripGuard {
    pub fn new(debug: bool) -> Self {
        RoundtripGuard {
            debug,
            ..Default::default()
        }
    }

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
        let (kind, parsed): (String, Option<ast::Row>) = match parse::parse(&aa) {
            Err(e) => (format!("parse error: {}", e), None),
            Ok(p) if p != row => ("AST mismatch".into(), Some(p)),
            Ok(p) => {
                let aa2 = render_root(&p, None, &ctx).to_text();
                if aa2 == render_root(&row, None, &ctx).to_text() {
                    return; // roundtrip holds
                }
                ("re-render mismatch".into(), Some(p))
            }
        };
        if !self.debug {
            // Refuse the edit: the state before it is the last one
            // whose picture reads back, so it is the one to stand on.
            ed.undo();
            self.checked = Some(ed.root.clone());
            ed.error(format!(
                "⚠ edit refused — it would break the AA roundtrip ({}); --debug captures a report",
                kind
            ));
            return;
        }
        if self.reported.as_deref() == Some(&aa) {
            return;
        }
        self.reported = Some(aa.clone());
        match write_report(&kind, &aa, &row, parsed.as_ref()) {
            Ok(path) => ed.error(format!("⚠ roundtrip bug — report: {}", path)),
            Err(e) => ed.error(format!("⚠ roundtrip bug (report failed: {})", e)),
        }
    }
}

fn write_report(
    kind: &str,
    aa: &str,
    expected: &ast::Row,
    parsed: Option<&ast::Row>,
) -> std::io::Result<String> {
    fs::create_dir_all("mascii_debug")?;
    let path = (1..)
        .map(|i| format!("mascii_debug/roundtrip-{}.txt", i))
        .find(|p| !std::path::Path::new(p).exists())
        .unwrap();
    let mut report = String::new();
    use std::fmt::Write as _;
    let _ = writeln!(report, "mascii roundtrip failure report");
    let _ = writeln!(report, "kind: {}", kind);
    let _ = writeln!(report, "\n--- canonical AA (fed to parse) ---\n{}", aa);
    let _ = writeln!(
        report,
        "\n--- expected AST (normalized editor content) ---\n{:#?}",
        expected
    );
    match parsed {
        Some(p) => {
            let _ = writeln!(report, "\n--- parsed AST ---\n{:#?}", p);
            let _ = writeln!(
                report,
                "\n--- re-rendered AA from parsed ---\n{}",
                render_root(p, None, &RenderCtx::canonical()).to_text()
            );
            let _ = writeln!(
                report,
                "\n--- LaTeX expected ---\n{}",
                latex::row_to_latex(expected)
            );
            let _ = writeln!(report, "\n--- LaTeX parsed ---\n{}", latex::row_to_latex(p));
        }
        None => {
            let _ = writeln!(report, "\n--- parsed AST ---\n(parse failed)");
            let _ = writeln!(
                report,
                "\n--- LaTeX expected ---\n{}",
                latex::row_to_latex(expected)
            );
        }
    }
    fs::write(&path, report)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mascii::input::Key;

    /// Without --debug a roundtrip-breaking edit is refused: the guard
    /// undoes it and says so, and no report file is involved. The
    /// broken tree is fabricated directly (every real edit path is
    /// supposed to keep the roundtrip — that is the point), sitting on
    /// top of a real undo point the guard can fall back to.
    #[test]
    fn a_breaking_edit_is_refused() {
        let mut ed = Editor::new();
        ed.input(Key::Char('a'), false, false);
        ed.input(Key::Char('b'), false, false);
        // A bare Roman '(' cannot roundtrip: it renders as the one
        // character every parser must read as a delimiter.
        ed.root = vec![mascii::ast::Node::Roman('(')];
        let mut guard = RoundtripGuard::default();
        guard.check(&mut ed);
        assert_ne!(
            ed.root,
            vec![mascii::ast::Node::Roman('(')],
            "the broken edit stood"
        );
        assert!(ed.message.contains("refused"), "{:?}", ed.message);
    }
}
