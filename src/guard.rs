//! The live roundtrip check: after every edit the canonical AA is
//! re-parsed and compared with the formula it came from. A mismatch is
//! a renderer/parser bug, so it is dumped to a report rather than
//! merely flagged.

use std::fs;

use mascii::editor::Editor;
use mascii::render::{RenderCtx, render_root};
use mascii::{ast, latex, parse};

/// Live roundtrip checker: after every edit, re-parse the canonical AA of
/// the current formula and compare. Any mismatch is a renderer/parser bug;
/// it is dumped to mascii_debug/roundtrip-N.txt so an AI (or human) can
/// load the report later and fix the toolchain.
#[derive(Default)]
pub struct RoundtripGuard {
    /// Last AA already reported (avoid one file per keystroke).
    reported: Option<String>,
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
