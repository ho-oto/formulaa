use std::fs;

use ratatui::Frame;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block as UiBlock, Borders, Paragraph};

use mascii::editor::{BLK_CLOSE, Editor, JUMP_CHAR_BASE, JUMP_LABELS, SEL_CLOSE, SEL_OPEN};
use mascii::input::{Effect, Key};
use mascii::parse::RegionSpan;
use mascii::render::{CURSOR_CHAR, RenderCtx, render_row};
use mascii::{ast, latex, parse, typst};

const HELP: &str = "\\cmd  ^/_ ( [ { // insets  Tab exit  ←→↑↓ move  ^A start  ⇧←→/⇧↑ select  ^B select block  ^C/^X/^V copy/cut/paste  ^G jump  ^O structure  ^T italic  ^Y copy AA  ^S save  Esc/^Q quit";

/// Context-sensitive last line: generic keys normally, the relevant
/// commands when the cursor is inside a grid cell or a delimiter.
fn help_line(ed: &Editor) -> &'static str {
    use mascii::ast::Field;
    match ed.path.last() {
        Some((_, Field::Cell(_))) => {
            "grid: Enter add row  \\addrow \\addcol \\delrow \\delcol  ] exit  (then ^/_ etc. as usual)"
        }
        Some((_, Field::Seg(_))) => {
            "delim: \\mid adds a │ segment  ) ] } close  \\lr<spec> visual pairs (\\lr(] \\lr{|}, . = none)"
        }
        _ => HELP,
    }
}

const USAGE: &str = "\
usage: mascii [--session] [SAVE_PATH]  interactive TUI editor (default: formula.tex)
       mascii aa2tex   [FILE]     AA formula (file or stdin) -> LaTeX
       mascii aa2typst [FILE]     AA formula (file or stdin) -> Typst
       mascii fmt      [FILE]     AA formula -> canonical AA (normalize)

--session: persist the formula to .mascii-session after every edit and
restore it on startup — survives restarts, e.g. cargo watch -x 'run -- --session'";

/// Session file for `--session` (canonical AA; formatting spacers are
/// lost on restore because reparsing drops them).
const SESSION_FILE: &str = ".mascii-session";

/// Load the session formula, if a valid one is on disk.
fn load_session() -> Option<ast::Row> {
    let text = fs::read_to_string(SESSION_FILE).ok()?;
    if text.trim().is_empty() {
        return None;
    }
    parse::parse(&text).ok()
}

/// Best-effort write of the current formula (empty formula = no file).
fn save_session(ed: &Editor) {
    let row = ast::normalize(&ed.root);
    if row.is_empty() {
        let _ = fs::remove_file(SESSION_FILE);
    } else {
        let aa = render_row(&row, None, false, &RenderCtx::canonical()).to_text();
        let _ = fs::write(SESSION_FILE, format!("{}\n", aa));
    }
}

fn main() -> std::io::Result<()> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let session = args.iter().any(|a| a == "--session");
    args.retain(|a| a != "--session");
    match args.first().map(String::as_str) {
        Some("aa2tex") | Some("aa2typst") | Some("fmt") => {
            let rest: Vec<&str> = args[1..].iter().map(String::as_str).collect();
            let file = rest.into_iter().find(|a| !a.starts_with("--"));
            return convert(&args[0].clone(), file);
        }
        Some("-h") | Some("--help") => {
            println!("{}", USAGE);
            return Ok(());
        }
        _ => {}
    }
    let save_path = args
        .first()
        .cloned()
        .unwrap_or_else(|| "formula.tex".into());
    let mut terminal = ratatui::init();
    let mut ed = Editor::new();
    ed.message = format!("mascii — LyX-like math editor (saves to {})", save_path);
    if session && let Some(row) = load_session() {
        ed.col = row.len();
        ed.root = row;
        ed.message = format!("session restored from {}", SESSION_FILE);
    }

    let mut guard = RoundtripGuard::default();
    let result = loop {
        if let Err(e) = terminal.draw(|f| draw(f, &ed)) {
            break Err(e);
        }
        match event::read() {
            Ok(Event::Key(key)) if key.kind == KeyEventKind::Press => {
                if handle_key(&mut ed, key.code, key.modifiers, &save_path) {
                    break Ok(());
                }
                guard.check(&mut ed);
                if session {
                    save_session(&ed);
                }
            }
            Ok(_) => {}
            Err(e) => break Err(e),
        }
    };

    ratatui::restore();
    result
}

/// aa2tex / aa2typst / fmt: read AA text (file or stdin), write to stdout.
fn convert(mode: &str, file: Option<&str>) -> std::io::Result<()> {
    let text = match file {
        Some(path) => fs::read_to_string(path)?,
        None => std::io::read_to_string(std::io::stdin())?,
    };
    let row = match parse::parse(&text) {
        Ok(row) => row,
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    };
    match mode {
        "aa2tex" => println!("{}", latex::row_to_latex(&row)),
        "aa2typst" => println!("{}", typst::row_to_typst(&row)),
        _ => {
            let block = render_row(&row, None, false, &RenderCtx::canonical());
            println!("{}", block.to_text());
        }
    }
    Ok(())
}

/// Pipe `text` into the first available system clipboard command.
fn copy_to_clipboard(text: &str) -> Result<&'static str, String> {
    use std::io::Write as _;
    use std::process::{Command, Stdio};
    const CANDIDATES: &[(&str, &[&str])] = &[
        ("pbcopy", &[]),
        ("wl-copy", &[]),
        ("xclip", &["-selection", "clipboard"]),
        ("xsel", &["--clipboard", "--input"]),
        ("clip.exe", &[]),
    ];
    for &(cmd, args) in CANDIDATES {
        let child = Command::new(cmd)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        let Ok(mut child) = child else { continue };
        let ok = child
            .stdin
            .take()
            .map(|mut i| i.write_all(text.as_bytes()).is_ok())
            .unwrap_or(false);
        if child.wait().map(|s| s.success()).unwrap_or(false) && ok {
            return Ok(cmd);
        }
    }
    Err("no clipboard command found (pbcopy / wl-copy / xclip / xsel)".into())
}

/// Live roundtrip checker: after every edit, re-parse the canonical AA of
/// the current formula and compare. Any mismatch is a renderer/parser bug;
/// it is dumped to mascii_debug/roundtrip-N.txt so an AI (or human) can
/// load the report later and fix the toolchain.
#[derive(Default)]
struct RoundtripGuard {
    /// Last AA already reported (avoid one file per keystroke).
    reported: Option<String>,
}

impl RoundtripGuard {
    fn check(&mut self, ed: &mut Editor) {
        let row = ast::normalize(&ed.root);
        if row.is_empty() {
            return;
        }
        let ctx = RenderCtx::canonical();
        let aa = render_row(&row, None, false, &ctx).to_text();
        // Formatting spacers survive in the AA but vanish on reparse.
        let row = ast::normalize(&ast::strip_spacers(&row));
        let (kind, parsed): (String, Option<ast::Row>) = match parse::parse(&aa) {
            Err(e) => (format!("parse error: {}", e), None),
            Ok(p) if p != row => ("AST mismatch".into(), Some(p)),
            Ok(p) => {
                let aa2 = render_row(&p, None, false, &ctx).to_text();
                if aa2 == render_row(&row, None, false, &ctx).to_text() {
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
            Ok(path) => ed.message = format!("⚠ roundtrip bug — report: {}", path),
            Err(e) => ed.message = format!("⚠ roundtrip bug (report failed: {})", e),
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
                render_row(p, None, false, &RenderCtx::canonical()).to_text()
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

/// Returns true when the app should quit.
fn handle_key(ed: &mut Editor, code: KeyCode, mods: KeyModifiers, save_path: &str) -> bool {
    // F-keys kept as terminal-specific aliases (^T/^B/^O are often
    // captured by the terminal or OS).
    let key = match code {
        KeyCode::F(2) => {
            ed.italic = !ed.italic;
            return false;
        }
        KeyCode::F(5) => {
            ed.structure = !ed.structure;
            return false;
        }
        KeyCode::Char(c) => Key::Char(c),
        KeyCode::Left => Key::Left,
        KeyCode::Right => Key::Right,
        KeyCode::Up => Key::Up,
        KeyCode::Down => Key::Down,
        KeyCode::Home => Key::Home,
        KeyCode::End => Key::End,
        KeyCode::Enter => Key::Enter,
        KeyCode::Backspace => Key::Backspace,
        KeyCode::Delete => Key::Delete,
        KeyCode::Esc => Key::Esc,
        KeyCode::Tab => Key::Tab,
        _ => return false,
    };
    let effect = ed.input(
        key,
        mods.contains(KeyModifiers::SHIFT),
        mods.contains(KeyModifiers::CONTROL),
    );
    match effect {
        Effect::Quit => return true,
        Effect::SaveTex => {
            let tex = latex::row_to_latex(&ed.root);
            match fs::write(save_path, format!("{}\n", tex)) {
                Ok(()) => ed.message = format!("saved LaTeX to {}", save_path),
                Err(e) => ed.message = format!("save failed: {}", e),
            }
        }
        // Yank: canonical AA to the system clipboard.
        Effect::CopyAa => {
            let row = ast::normalize(&ed.root);
            let aa = render_row(&row, None, false, &RenderCtx::canonical()).to_text();
            match copy_to_clipboard(&aa) {
                Ok(cmd) => ed.message = format!("copied AA to clipboard ({})", cmd),
                Err(e) => ed.message = format!("copy failed: {}", e),
            }
        }
        Effect::None => {}
    }
    false
}

fn draw(f: &mut Frame, ed: &Editor) {
    let [canvas_area, tex_area, status_area, help_area] = Layout::vertical([
        Constraint::Min(3),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(f.area());

    draw_canvas(f, canvas_area, ed);

    let tex = latex::row_to_latex(&ed.root);
    f.render_widget(
        Line::from(vec![
            Span::styled(" LaTeX: ", Style::default().fg(Color::DarkGray)),
            Span::raw(tex),
        ]),
        tex_area,
    );

    let status = match &ed.minibuffer {
        Some(buf) => Line::from(vec![
            Span::styled(
                format!(" \\{}", buf),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("▌", Style::default().fg(Color::Yellow)),
            Span::styled(
                "  (Enter/Space: execute, Esc: cancel)",
                Style::default().fg(Color::DarkGray),
            ),
        ]),
        None => Line::from(Span::styled(
            format!(" {}", ed.message),
            Style::default().fg(Color::Green),
        )),
    };
    f.render_widget(status, status_area);

    f.render_widget(
        Line::from(Span::styled(
            format!(" {}", help_line(ed)),
            Style::default().fg(Color::DarkGray),
        )),
        help_area,
    );
}

fn draw_canvas(f: &mut Frame, area: Rect, ed: &Editor) {
    let border = UiBlock::default()
        .borders(Borders::ALL)
        .title(" mascii ")
        .border_style(Style::default().fg(Color::DarkGray));
    let inner = border.inner(area);
    f.render_widget(border, area);

    let ctx = RenderCtx { italic: ed.italic };
    // Structure view: cursor-free canonical render, re-parsed to recover
    // every block's rectangle, painted by nesting depth.
    if ed.structure {
        let block = render_row(&ed.root, None, false, &RenderCtx::canonical());
        let lines = block.to_strings();
        let regions = mascii::parse::parse_with_regions(&block.to_text())
            .map(|(_, r)| r)
            .unwrap_or_default();
        draw_structure(f, inner, &lines, &regions);
        return;
    }

    let (root, cursor) = ed.decorated();
    let cursor_ref = cursor.as_ref().map(|(p, c)| (p.as_slice(), *c));
    let block = render_row(&root, cursor_ref, false, &ctx);
    let (lines, bg) = marker_boxes(&block.to_strings(), &ed.marker_extents());

    let width = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0) as u16;
    let height = lines.len() as u16;
    let left = inner.width.saturating_sub(width) / 2;
    let top = inner.height.saturating_sub(height) / 2;

    let mut text: Vec<Line> = Vec::with_capacity(top as usize + lines.len());
    for _ in 0..top {
        text.push(Line::raw(""));
    }
    let pad = " ".repeat(left as usize);
    for (l, bgrow) in lines.iter().zip(&bg) {
        let mut spans = vec![Span::raw(pad.clone())];
        spans.extend(decorate_line(l, bgrow));
        text.push(Line::from(spans));
    }
    f.render_widget(Paragraph::new(text), inner);
}

/// Selection box background (replaces the old ⟦ ⟧ bracket display).
const SELECTION_BG: Color = Color::Indexed(89);

/// Convert marker atoms into colored boxes and overlaid labels: every
/// marker column (selection pair, ^G/^B labels, ^B closes) is removed
/// from the text so the geometry stays put, box interiors get a
/// background color (selection in SELECTION_BG, blocks in the
/// structure-view depth palette), and each label glyph is drawn *over*
/// the character that follows it instead of occupying a column.
fn marker_boxes(
    lines: &[String],
    extents: &[(usize, usize)],
) -> (Vec<String>, Vec<Vec<Option<Color>>>) {
    let grid: Vec<Vec<char>> = lines.iter().map(|l| l.chars().collect()).collect();
    let labels = JUMP_LABELS.chars().count() as u32;
    let is_label = |c: char| (JUMP_CHAR_BASE..JUMP_CHAR_BASE + labels).contains(&(c as u32));
    // Markers in column order (every marker owns its own column).
    let mut marks: Vec<(usize, usize, char)> = Vec::new();
    for (y, row) in grid.iter().enumerate() {
        for (x, &c) in row.iter().enumerate() {
            if c == SEL_OPEN || c == SEL_CLOSE || c == BLK_CLOSE || is_label(c) {
                marks.push((x, y, c));
            }
        }
    }
    marks.sort_unstable();
    // Pair opens with closes (properly nested by construction). A lone
    // label (jump mode) has no close and yields no box.
    let mut stack: Vec<(usize, usize, char)> = Vec::new();
    // (open col, open row, close col, depth, color, open char)
    #[allow(clippy::type_complexity)]
    let mut boxes: Vec<(usize, usize, usize, usize, Color, char)> = Vec::new();
    let mut overlays: Vec<(usize, usize, char)> = Vec::new(); // (row, orig col, label)
    for &(x, y, c) in &marks {
        if c == SEL_CLOSE || c == BLK_CLOSE {
            if let Some((o, oy, oc)) = stack.pop() {
                let depth = stack.len() + 1;
                let color = if oc == SEL_OPEN {
                    SELECTION_BG
                } else {
                    DEPTH_BG[(depth - 1) % DEPTH_BG.len()]
                };
                boxes.push((o, oy, x, depth, color, oc));
            }
        } else {
            if is_label(c) {
                overlays.push((y, x, c));
            }
            stack.push((x, y, c));
        }
    }
    // A marker column may only be removed when every row is blank, a
    // marker, or a stretchy filler there (bars shrink consistently) —
    // centered content in *other* rows can sit under a marker column,
    // and deleting it would corrupt that row. Unremovable markers are
    // blanked in place instead (a one-cell gap, but alignment is exact).
    let filler = |ch: char| matches!(ch, ' ' | '─' | '═' | '┄' | '_');
    let is_marker = |ch: char| ch == SEL_OPEN || ch == SEL_CLOSE || ch == BLK_CLOSE || is_label(ch);
    let strip: Vec<usize> = marks
        .iter()
        .map(|&(x, _, _)| x)
        .filter(|&x| {
            grid.iter()
                .all(|row| row.get(x).is_none_or(|&ch| filler(ch) || is_marker(ch)))
        })
        .collect();
    let shift = |x: usize| x - strip.iter().take_while(|&&s| s < x).count();

    let mut bg: Vec<Vec<Option<Color>>> = grid
        .iter()
        .map(|row| vec![None; row.len() - strip.iter().filter(|&&s| s < row.len()).count()])
        .collect();
    // Exact vertical extents come from the editor; a ^B label char
    // encodes its target index, the selection has a single entry. The
    // char grid alone cannot separate a block's rows from other content
    // sharing its columns.
    let extent_of = |oc: char| {
        if oc == SEL_OPEN {
            extents.first().copied()
        } else {
            extents.get((oc as u32 - JUMP_CHAR_BASE) as usize).copied()
        }
    };
    // Outer boxes first so nested ones paint over them.
    boxes.sort_by_key(|&(_, _, _, d, _, _)| d);
    for &(o, oy, close, _, color, oc) in &boxes {
        let (t, b) = match extent_of(oc) {
            Some((above, below)) => (
                oy.saturating_sub(above),
                (oy + below).min(grid.len().saturating_sub(1)),
            ),
            // Fallback: rows showing content between the markers.
            None => {
                let rows: Vec<usize> = (0..grid.len())
                    .filter(|&y| {
                        (o + 1..close).any(|x| grid[y].get(x).is_some_and(|&ch| ch != ' '))
                    })
                    .collect();
                match (rows.first(), rows.last()) {
                    (Some(&t), Some(&b)) => (t, b),
                    _ => continue,
                }
            }
        };
        for row in bg.iter_mut().take(b + 1).skip(t) {
            for x in o + 1..close {
                if strip.binary_search(&x).is_err() {
                    let sx = shift(x);
                    if sx < row.len() {
                        row[sx] = Some(color);
                    }
                }
            }
        }
    }
    let mut stripped: Vec<Vec<char>> = grid
        .iter()
        .map(|row| {
            row.iter()
                .enumerate()
                .filter(|(x, _)| strip.binary_search(x).is_err())
                .map(|(_, &c)| if is_marker(c) { ' ' } else { c })
                .collect()
        })
        .collect();
    // Labels sit on top of the glyph that followed them.
    for &(y, x, label) in &overlays {
        let sx = shift(x);
        let row = &mut stripped[y];
        if sx < row.len() {
            row[sx] = label;
        } else {
            row.push(label);
        }
    }
    (stripped.into_iter().map(String::from_iter).collect(), bg)
}

/// Turn a rendered line into spans: private-use marker chars become
/// colored jump/block labels, the cursor glyph blinks, and box
/// backgrounds from `marker_boxes` are applied to plain glyphs.
fn decorate_line(line: &str, bg: &[Option<Color>]) -> Vec<Span<'static>> {
    let cursor_style = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD | Modifier::SLOW_BLINK);
    let label_style = Style::default()
        .fg(Color::Black)
        .bg(Color::Yellow)
        .add_modifier(Modifier::BOLD);

    let mut spans = Vec::new();
    let mut buf = String::new();
    let mut buf_bg: Option<Color> = None;
    let flush = |buf: &mut String, buf_bg: Option<Color>, spans: &mut Vec<Span<'static>>| {
        if !buf.is_empty() {
            let s = std::mem::take(buf);
            spans.push(match buf_bg {
                Some(color) => Span::styled(s, Style::default().bg(color)),
                None => Span::raw(s),
            });
        }
    };
    for (i, c) in line.chars().enumerate() {
        let u = c as u32;
        let cell_bg = bg.get(i).copied().flatten();
        if c == CURSOR_CHAR {
            flush(&mut buf, buf_bg, &mut spans);
            let style = match cell_bg {
                Some(color) => cursor_style.bg(color),
                None => cursor_style,
            };
            spans.push(Span::styled(CURSOR_CHAR.to_string(), style));
        } else if c == '␣' {
            // Explicit space atom: keep visible but unobtrusive.
            flush(&mut buf, buf_bg, &mut spans);
            let mut style = Style::default().fg(Color::DarkGray);
            if let Some(color) = cell_bg {
                style = style.bg(color);
            }
            spans.push(Span::styled("␣", style));
        } else if (JUMP_CHAR_BASE..JUMP_CHAR_BASE + JUMP_LABELS.chars().count() as u32).contains(&u)
        {
            let label = JUMP_LABELS
                .chars()
                .nth((u - JUMP_CHAR_BASE) as usize)
                .unwrap();
            flush(&mut buf, buf_bg, &mut spans);
            spans.push(Span::styled(label.to_string(), label_style));
        } else {
            if cell_bg != buf_bg {
                flush(&mut buf, buf_bg, &mut spans);
                buf_bg = cell_bg;
            }
            buf.push(c);
        }
    }
    flush(&mut buf, buf_bg, &mut spans);
    spans
}

/// Background palette for the structure view, cycling with depth.
const DEPTH_BG: [Color; 5] = [
    Color::Indexed(17), // dark blue
    Color::Indexed(22), // dark green
    Color::Indexed(54), // purple
    Color::Indexed(23), // teal
    Color::Indexed(58), // olive
];

fn draw_structure(f: &mut Frame, inner: Rect, lines: &[String], regions: &[RegionSpan]) {
    let height = lines.len();
    let width = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);
    // Deepest region wins per cell.
    let mut depth = vec![vec![0usize; width]; height];
    let mut sorted: Vec<&RegionSpan> = regions.iter().collect();
    sorted.sort_by_key(|(_, d)| *d);
    for ((t, b, l, r), d) in sorted.into_iter().copied() {
        for row in depth.iter_mut().take(b.min(height - 1) + 1).skip(t) {
            for cell in row.iter_mut().take(r.min(width - 1) + 1).skip(l) {
                *cell = d;
            }
        }
    }

    let left = inner.width.saturating_sub(width as u16) / 2;
    let top = inner.height.saturating_sub(height as u16) / 2;
    let pad = " ".repeat(left as usize);
    let mut text: Vec<Line> = Vec::with_capacity(top as usize + height);
    for _ in 0..top {
        text.push(Line::raw(""));
    }
    for (y, l) in lines.iter().enumerate() {
        let mut spans = vec![Span::raw(pad.clone())];
        let mut buf = String::new();
        let mut cur = 0usize;
        for (x, c) in l.chars().enumerate() {
            let d = depth[y][x];
            if d != cur && !buf.is_empty() {
                spans.push(styled_depth(std::mem::take(&mut buf), cur));
            }
            cur = d;
            buf.push(c);
        }
        if !buf.is_empty() {
            spans.push(styled_depth(buf, cur));
        }
        text.push(Line::from(spans));
    }
    f.render_widget(Paragraph::new(text), inner);
}

fn styled_depth(s: String, depth: usize) -> Span<'static> {
    if depth == 0 {
        Span::raw(s)
    } else {
        Span::styled(
            s,
            Style::default().bg(DEPTH_BG[(depth - 1) % DEPTH_BG.len()]),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marker_boxes_strip_markers_and_paint_cells() {
        // One-line selection: ⟦ab⟧ → markers vanish, a/b get the bg.
        let lines = vec![format!(" {}ab{} ", SEL_OPEN, SEL_CLOSE)];
        let (stripped, bg) = marker_boxes(&lines, &[(0, 0)]);
        assert_eq!(stripped, vec![" ab ".to_string()]);
        assert_eq!(
            bg[0],
            vec![None, Some(SELECTION_BG), Some(SELECTION_BG), None]
        );
    }

    #[test]
    fn marker_boxes_cover_multiline_content() {
        // A selected fraction: the box spans all three rows.
        let lines = vec![
            "  1  ".to_string(),
            format!("{}───{}", SEL_OPEN, SEL_CLOSE),
            "  2  ".to_string(),
        ];
        let (stripped, bg) = marker_boxes(&lines, &[(1, 1)]);
        assert_eq!(stripped[1], "───");
        assert!(bg.iter().all(|row| row.iter().all(|c| c.is_some())));
    }

    #[test]
    fn unsafe_marker_columns_are_blanked_not_removed() {
        // The ⟧ column carries the denominator's `2` in another row —
        // removing that column would delete the 2. It must be kept,
        // with the marker cell blanked (exact alignment, 1-cell gap).
        let lines = vec![
            format!("{}ab{}", SEL_OPEN, SEL_CLOSE),
            "────".to_string(),
            "   2".to_string(),
        ];
        let (stripped, bg) = marker_boxes(&lines, &[(0, 0)]);
        assert_eq!(stripped[0], "ab ");
        assert_eq!(stripped[1], "───");
        assert_eq!(stripped[2], "  2", "denominator content must survive");
        assert_eq!(bg[0], vec![Some(SELECTION_BG), Some(SELECTION_BG), None]);
        // The box must not leak into the bar/denominator rows even
        // though they have content under the same columns.
        assert!(bg[1].iter().all(|c| c.is_none()), "bar row painted");
        assert!(bg[2].iter().all(|c| c.is_none()), "den row painted");
    }

    #[test]
    fn jump_labels_overlay_without_shifting() {
        let label = char::from_u32(JUMP_CHAR_BASE).unwrap();
        // Label before 'x': the label column vanishes and the label is
        // drawn over 'x' — the line keeps its original width.
        let lines = vec![format!("{}xy", label)];
        let (stripped, bg) = marker_boxes(&lines, &[]);
        assert_eq!(stripped[0].chars().collect::<Vec<_>>(), vec![label, 'y']);
        assert!(bg[0].iter().all(|c| c.is_none()));
    }
}
