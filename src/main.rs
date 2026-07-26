use std::fs;

use ratatui::Frame;
use ratatui::crossterm;
use ratatui::crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
    MouseButton, MouseEventKind,
};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block as UiBlock, Borders, Paragraph};

mod theme;

use mascii::editor::{
    BLK_CLOSE, Editor, JUMP_CHAR_BASE, JUMP_LABELS, JUMP_RANK_BASE, SEL_CLOSE, SEL_OPEN,
};
use mascii::input::{Effect, Key};
use mascii::render::{RenderCtx, render_root};
use mascii::{ast, latex, parse};

const HELP: &str = "^G jump  ^F free move  ^B select block  \\cmd  ^/_ ( [ { // insets  Tab exit  ←→↑↓/click move  ⇧←→/⇧↑ select  ^Z/^R undo/redo  ^T italic  ^Y copy AA  ^S save  Esc/^Q quit";

/// Context-sensitive last line: generic keys normally, the relevant
/// commands when the cursor is inside a grid cell or a delimiter.
fn help_line(ed: &Editor) -> &'static str {
    use mascii::ast::Field;
    if ed.minibuffer.is_some() {
        return "command: type at the cursor  Enter/Space execute  Esc cancel";
    }
    if ed.op_entry.is_some() {
        return "op name: letters/digits + Space (word pieces)  Enter/Tab commit  Esc cancel";
    }
    if ed.free.is_some() {
        return if ed.jump.is_some() {
            "free markers: label letter or ←→↑↓ + Enter jumps (then keep flying)  Esc/^G markers off"
        } else {
            "free move: ←→↑↓ cells  ^G markers  Enter snap  Esc cancel"
        };
    }
    if ed.grid_mode {
        return "grid mode: ←→↑↓ move cells  r/R add row below/above  c/C add col right/left  d/D delete row/col  Esc/^O exit";
    }
    match ed.path.last() {
        Some((_, Field::Cell(_))) => {
            "grid: ^O edit mode (move cells, add/delete rows & cols)  Enter add row  ] exit  (then ^/_ etc. as usual)"
        }
        Some((_, Field::Seg(_))) => {
            "delim: \\mid adds a │ segment  ) ] } close  \\lr<spec> visual pairs (\\lr(] \\lr{|}, . = none)"
        }
        _ => HELP,
    }
}

const USAGE: &str = "\
usage: mascii                 interactive TUI editor
       mascii aa2tex   [FILE] AA formula (file or stdin) -> LaTeX
       mascii fmt      [FILE] AA formula -> canonical AA (normalize)";

/// View state the editor itself does not own: the scroll offset of the
/// canvas (a formula larger than the terminal does not fit).
#[derive(Default)]
struct View {
    scroll_x: usize,
    scroll_y: usize,
}

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some(m @ ("aa2tex" | "fmt")) => {
            return convert(m, args.get(1).map(String::as_str));
        }
        Some(_) => {
            println!("{}", USAGE);
            return Ok(());
        }
        None => {}
    }
    let mut terminal = ratatui::init();
    let _ = crossterm::execute!(std::io::stdout(), EnableMouseCapture);
    let mut ed = Editor::new();
    ed.message = "mascii — LyX-like math editor".into();

    let mut guard = RoundtripGuard::default();
    let mut origin = (0u16, 0u16);
    let mut view = View::default();
    let result = loop {
        if let Err(e) = terminal.draw(|f| origin = draw(f, &ed, &mut view)) {
            break Err(e);
        }
        match event::read() {
            Ok(Event::Key(key)) if key.kind == KeyEventKind::Press => {
                if handle_key(&mut ed, key.code, key.modifiers) {
                    break Ok(());
                }
                guard.check(&mut ed);
            }
            Ok(Event::Mouse(m)) if m.kind == MouseEventKind::Down(MouseButton::Left) => {
                if m.column >= origin.0 && m.row >= origin.1 {
                    ed.click(
                        (m.column - origin.0) as usize + view.scroll_x,
                        (m.row - origin.1) as usize + view.scroll_y,
                    );
                }
            }
            Ok(_) => {}
            Err(e) => break Err(e),
        }
    };

    let _ = crossterm::execute!(std::io::stdout(), DisableMouseCapture);
    ratatui::restore();
    result
}

/// aa2tex / fmt: read AA text (file or stdin), write to stdout.
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
        _ => {
            let block = render_root(&row, None, &RenderCtx::canonical());
            println!("{}", block.to_text());
        }
    }
    Ok(())
}

/// The system clipboard via arboard (X11/Wayland/macOS/Windows).
fn copy_to_clipboard(text: &str) -> Result<(), String> {
    arboard::Clipboard::new()
        .and_then(|mut c| c.set_text(text.to_string()))
        .map_err(|e| e.to_string())
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

/// Returns true when the app should quit.
fn handle_key(ed: &mut Editor, code: KeyCode, mods: KeyModifiers) -> bool {
    // F-keys kept as terminal-specific aliases (^T/^B/^O are often
    // captured by the terminal or OS).
    let key = match code {
        KeyCode::F(2) => {
            ed.italic = !ed.italic;
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
        // Yank: canonical AA to the system clipboard.
        Effect::CopyAa => {
            let row = ast::normalize(&ed.root);
            let aa = render_root(&row, None, &RenderCtx::canonical()).to_text();
            ed.message = match copy_to_clipboard(&aa) {
                Ok(()) => "copied AA to clipboard".into(),
                Err(e) => format!("copy failed: {}", e),
            };
        }
        Effect::None => {}
    }
    false
}

/// Draw the whole UI; returns the screen coordinates of the formula's
/// top-left cell (for mouse hit-testing).
fn draw(f: &mut Frame, ed: &Editor, view: &mut View) -> (u16, u16) {
    let [canvas_area, help_area] =
        Layout::vertical([Constraint::Min(3), Constraint::Length(1)]).areas(f.area());

    let origin = draw_canvas(f, canvas_area, ed, view);

    // One bottom line: messages overlay the usage line when present
    // (the minibuffer itself shows in-place at the cursor).
    let bottom = if !ed.message.is_empty() {
        Line::from(Span::styled(
            format!(" {}", ed.message),
            Style::default().fg(theme::MESSAGE_FG),
        ))
    } else {
        Line::from(Span::styled(
            format!(" {}", help_line(ed)),
            Style::default().fg(theme::CHROME_FG),
        ))
    };
    f.render_widget(bottom, help_area);
    origin
}

/// Returns the screen position of the formula's top-left cell.
fn draw_canvas(f: &mut Frame, area: Rect, ed: &Editor, view: &mut View) -> (u16, u16) {
    let border = UiBlock::default()
        .borders(Borders::ALL)
        .title(" mascii ")
        .border_style(Style::default().fg(theme::BORDER_FG));
    let inner = border.inner(area);
    f.render_widget(border, area);

    let ctx = RenderCtx { italic: ed.italic };
    let (root, cursor) = ed.decorated();
    let cursor_ref = cursor.as_ref().map(|(p, c)| (p.as_slice(), *c));
    let block = render_root(&root, cursor_ref, &ctx);
    let (lines, mut bg, mut cursor_cell) = marker_boxes(
        &block.lines,
        &ed.marker_extents(),
        &block.marks,
        block.caret,
        ed.jump.is_some().then_some(ed.jump_selected),
        ed.block.is_some().then_some(ed.block_sel),
    );
    let struck: std::collections::HashSet<(usize, usize)> = block.cancel.iter().copied().collect();
    let mut lines = lines;
    // ^F: the free cursor itself gets the prominent caret style; the
    // snap preview is the subtler colored cell.
    if let Some(f) = &ed.free {
        let (sy, sx) = f.snap_at;
        if sy < lines.len() {
            if sx >= lines[sy].len() {
                lines[sy].resize(sx + 1, ' ');
                bg[sy].resize(sx + 1, None);
            }
            let last = bg[sy].len().saturating_sub(1);
            bg[sy][sx.min(last)] = Some(theme::FREE_BG);
        }
        let (fy, fx) = f.at;
        if fy < lines.len() {
            if fx >= lines[fy].len() {
                lines[fy].resize(fx + 1, ' ');
                bg[fy].resize(fx + 1, None);
            }
            cursor_cell = Some((fy, fx));
        }
    }

    overlay_minibuffer(ed, &mut lines, &mut bg, &mut cursor_cell);

    let width = (block.width() as u16).max(lines.iter().map(|l| l.len() as u16).max().unwrap_or(0));
    let height = lines.len() as u16;
    // A formula larger than the canvas scrolls, following the cursor
    // (the free cursor included) with a few cells of margin; one that
    // fits is centered on that axis and its scroll resets. `scroll`
    // returns the offset, `pad` the centering pad.
    let scroll = |size: u16, avail: u16, cur: Option<usize>, off: &mut usize| -> u16 {
        if size <= avail {
            *off = 0;
            return avail.saturating_sub(size) / 2;
        }
        let vis = avail as usize;
        let margin = 4.min(vis / 4);
        if let Some(c) = cur {
            *off = (*off).min(c.saturating_sub(margin));
            if c + margin >= *off + vis {
                *off = c + margin + 1 - vis;
            }
        }
        *off = (*off).min(size as usize - vis);
        0
    };
    let left = scroll(
        width,
        inner.width,
        cursor_cell.map(|(_, cx)| cx),
        &mut view.scroll_x,
    );
    let top = scroll(
        height,
        inner.height,
        cursor_cell.map(|(cy, _)| cy),
        &mut view.scroll_y,
    );

    let mut text: Vec<Line> = Vec::with_capacity(top as usize + lines.len());
    for _ in 0..top {
        text.push(Line::raw(""));
    }
    let pad = " ".repeat(left as usize);
    for (y, (l, bgrow)) in lines.iter().zip(&bg).enumerate().skip(view.scroll_y) {
        let ccol = cursor_cell.and_then(|(cy, cx)| (cy == y).then_some(cx));
        let mut spans = vec![Span::raw(pad.clone())];
        spans.extend(decorate_line(
            l,
            y,
            &struck,
            bgrow,
            ccol,
            ed.free.is_some(),
            view.scroll_x,
        ));
        text.push(Line::from(spans));
    }
    f.render_widget(Paragraph::new(text), inner);
    (inner.x + left, inner.y + top)
}

/// Turn the zero-width display annotations of a rendered block into
/// colored boxes, overlaid labels and the caret cell. Marks carry
/// (row, col, char): jump/block labels overlay the glyph at their
/// position; selection/block mark pairs paint a background box (the
/// selection in theme::SELECTION_BG, ^B blocks in the depth palette
/// by nesting depth). Nothing is inserted or removed, so the geometry
/// always equals the undecorated render.
#[allow(clippy::type_complexity)]
fn marker_boxes(
    lines: &[Vec<char>],
    extents: &[(usize, usize, usize)],
    marks: &[(usize, usize, char)],
    caret: Option<(usize, usize)>,
    selected: Option<usize>,
    block_selected: Option<usize>,
) -> (
    Vec<Vec<char>>,
    Vec<Vec<Option<Color>>>,
    Option<(usize, usize)>,
) {
    // Cell coordinates throughout — cancel strikes (combining U+0338)
    // are a separate channel applied at span emission, so a struck cell
    // never desyncs the column indexing (or splits its ligature).
    let mut grid: Vec<Vec<char>> = lines.to_vec();
    if grid.is_empty() {
        grid.push(Vec::new());
    }
    let labels = JUMP_LABELS.chars().count() as u32;
    let is_label = |c: char| (JUMP_CHAR_BASE..JUMP_CHAR_BASE + labels).contains(&(c as u32));

    let mut bg: Vec<Vec<Option<Color>>> = grid.iter().map(|row| vec![None; row.len()]).collect();
    // Boxes: pair opens (selection start, ^B label) with closes within
    // each row, nesting by position.
    let mut boxes: Vec<(usize, usize, usize, Color)> = Vec::new(); // (row, open, close, color) sorted later
    let mut order: Vec<usize> = Vec::new(); // paint order key: depth
    let mut by_row: std::collections::BTreeMap<usize, Vec<(usize, char)>> =
        std::collections::BTreeMap::new();
    for &(y, x, c) in marks {
        by_row.entry(y).or_default().push((x, c));
    }
    for (y, mut row_marks) in by_row.clone() {
        row_marks.sort_unstable();
        let mut stack: Vec<(usize, char)> = Vec::new();
        for (x, c) in row_marks {
            if c == SEL_CLOSE || c == BLK_CLOSE {
                if let Some((o, oc)) = stack.pop() {
                    let (color, depth) = if oc == SEL_OPEN {
                        (theme::SELECTION_BG, 0)
                    } else {
                        let idx = (oc as u32 - JUMP_CHAR_BASE) as usize;
                        let d = extents.get(idx).map_or(0, |&(_, _, d)| d);
                        let c = if block_selected == Some(idx) {
                            theme::SELECTED_BG
                        } else {
                            theme::DEPTH_BG[d % theme::DEPTH_BG.len()]
                        };
                        (c, d)
                    };
                    boxes.push((y, o, x, color));
                    order.push(depth);
                }
            } else {
                stack.push((x, c));
            }
        }
    }
    // Outer boxes first so nested ones paint over them. The ^B rank is
    // innermost-first (rank 0 = the innermost parent), so paint in
    // descending rank: outermost ancestors below, inner ones on top.
    let mut idx: Vec<usize> = (0..boxes.len()).collect();
    idx.sort_by_key(|&i| std::cmp::Reverse(order[i]));
    for i in idx {
        let (oy, o, close, color) = boxes[i];
        // Which extent? Selection has one entry; a ^B box finds its
        // extent through its label char.
        let ext = by_row
            .get(&oy)
            .and_then(|ms| ms.iter().find(|&&(x, _)| x == o))
            .map(|&(_, c)| c)
            .and_then(|c| {
                if c == SEL_OPEN {
                    extents.first()
                } else if is_label(c) {
                    extents.get((c as u32 - JUMP_CHAR_BASE) as usize)
                } else {
                    None
                }
            });
        let (t, b) = match ext {
            Some(&(above, below, _)) => (
                oy.saturating_sub(above),
                (oy + below).min(grid.len().saturating_sub(1)),
            ),
            None => (oy, oy),
        };
        for row in bg.iter_mut().take(b + 1).skip(t) {
            for x in o..close {
                if x < row.len() {
                    row[x] = Some(color);
                }
            }
        }
    }
    // Labels overlay the glyph at their position. Jump markers carry
    // their rank: within the label alphabet they show the label letter,
    // beyond it a highlight cell; the arrow-selected marker gets its
    // own color either way.
    let rank_of = |c: char| {
        let u = c as u32;
        (JUMP_RANK_BASE..JUMP_RANK_BASE + 0x400)
            .contains(&u)
            .then(|| (u - JUMP_RANK_BASE) as usize)
    };
    for &(y, x, c) in marks {
        let rank = rank_of(c);
        if !(is_label(c) || rank.is_some()) {
            continue;
        }
        let row = &mut grid[y];
        if x >= row.len() {
            row.resize(x + 1, ' ');
            bg[y].resize(x + 1, None);
        }
        match rank {
            None => row[x] = c, // a ^B label
            Some(r) => {
                let is_sel = selected == Some(r);
                if let Some(label) = JUMP_LABELS.chars().nth(r) {
                    // Re-encode as a display label char for styling.
                    row[x] = char::from_u32(JUMP_CHAR_BASE + r as u32).unwrap_or(label);
                } else if bg[y][x].is_none() {
                    bg[y][x] = Some(theme::UNLABELED_BG);
                }
                if is_sel {
                    bg[y][x] = Some(theme::SELECTED_BG);
                }
            }
        }
    }
    // The caret cell (padded blank at the row end).
    let cursor_cell = caret.map(|(y, x)| {
        if x >= grid[y].len() {
            grid[y].resize(x + 1, ' ');
            bg[y].resize(x + 1, None);
        }
        (y, x)
    });
    (grid, bg, cursor_cell)
}

/// Draw the open minibuffer as an overlay at the caret cell: the typed
/// `\command` covers the glyphs to the right of the cursor without
/// moving them (zero layout shift — the eye stays on the formula), and
/// the caret sits after the text. With a selection active the caret is
/// at the selection's moving end, so the overlay shows next to it.
fn overlay_minibuffer(
    ed: &Editor,
    lines: &mut [Vec<char>],
    bg: &mut [Vec<Option<Color>>],
    cursor_cell: &mut Option<(usize, usize)>,
) {
    let Some(buf) = &ed.minibuffer else { return };
    let Some((cy, cx)) = *cursor_cell else { return };
    if cy >= lines.len() {
        return;
    }
    let text: Vec<char> = std::iter::once('\\').chain(buf.chars()).collect();
    let end = cx + text.len();
    if lines[cy].len() < end + 1 {
        lines[cy].resize(end + 1, ' ');
        bg[cy].resize(end + 1, None);
    }
    // Live feedback: the overlay turns red as soon as the typed name
    // is not something \execute knows (it goes back on completion).
    let color = if ed.command_known(buf) {
        theme::MINIBUF_BG
    } else {
        theme::MINIBUF_BAD_BG
    };
    for (i, &ch) in text.iter().enumerate() {
        lines[cy][cx + i] = ch;
        bg[cy][cx + i] = Some(color);
    }
    *cursor_cell = Some((cy, end));
}

/// Turn a rendered cell row into spans: private-use marker chars become
/// colored jump/block labels, the cursor glyph blinks, and box
/// backgrounds from `marker_boxes` are applied to plain glyphs. A
/// struck cell gets its combining U+0338 appended *inside* its span,
/// so the ligature is never split across style boundaries.
fn decorate_line(
    line: &[char],
    y: usize,
    struck: &std::collections::HashSet<(usize, usize)>,
    bg: &[Option<Color>],
    cursor: Option<usize>,
    free_caret: bool,
    scroll_x: usize,
) -> Vec<Span<'static>> {
    // The ordinary caret is terminal-style reverse video; the free
    // cursor is a solid colored block instead (reverse video would swap
    // the tint onto the glyph, which reads as "the character changed
    // color", not "the cursor changed color").
    let cursor_style = if free_caret {
        Style::default()
            .fg(theme::FREE_CURSOR_FG)
            .bg(theme::FREE_CURSOR_BG)
            .add_modifier(Modifier::BOLD | Modifier::SLOW_BLINK)
    } else {
        Style::default().add_modifier(Modifier::REVERSED | Modifier::BOLD | Modifier::SLOW_BLINK)
    };
    let label_style = Style::default()
        .fg(theme::LABEL_FG)
        .bg(theme::LABEL_BG)
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
    for (i, &c) in line.iter().enumerate().skip(scroll_x) {
        let u = c as u32;
        let cell_bg = bg.get(i).copied().flatten();
        let cell = if struck.contains(&(y, i)) {
            format!("{}\u{338}", c)
        } else {
            c.to_string()
        };
        if cursor == Some(i) {
            // Terminal-style caret: reverse video on the glyph right of
            // the insertion point.
            flush(&mut buf, buf_bg, &mut spans);
            let style = match cell_bg {
                Some(color) if !free_caret => cursor_style.bg(color),
                _ => cursor_style,
            };
            spans.push(Span::styled(cell, style));
        } else if c == '␣' {
            // Explicit space atom: keep visible but unobtrusive.
            flush(&mut buf, buf_bg, &mut spans);
            let mut style = Style::default().fg(theme::SPACE_FG);
            if let Some(color) = cell_bg {
                style = style.bg(color);
            }
            spans.push(Span::styled(cell, style));
        } else if (JUMP_CHAR_BASE..JUMP_CHAR_BASE + JUMP_LABELS.chars().count() as u32).contains(&u)
        {
            let label = JUMP_LABELS
                .chars()
                .nth((u - JUMP_CHAR_BASE) as usize)
                .unwrap();
            flush(&mut buf, buf_bg, &mut spans);
            // The arrow-selected marker keeps its highlight color.
            let style = match cell_bg {
                Some(color) => label_style.bg(color),
                None => label_style,
            };
            spans.push(Span::styled(label.to_string(), style));
        } else {
            if cell_bg != buf_bg {
                flush(&mut buf, buf_bg, &mut spans);
                buf_bg = cell_bg;
            }
            buf.push_str(&cell);
        }
    }
    flush(&mut buf, buf_bg, &mut spans);
    spans
}

#[cfg(test)]
mod tests {
    /// A formula larger than the canvas scrolls on both axes so the
    /// cursor stays visible; one that fits is centered (offsets 0).
    #[test]
    fn canvas_scrolls_to_follow_the_cursor() {
        use ratatui::{Terminal, backend::TestBackend};
        let render = |ed: &Editor, w: u16, h: u16| -> View {
            let mut view = View::default();
            let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
            term.draw(|f| {
                draw(f, ed, &mut view);
            })
            .unwrap();
            view
        };
        // Fits: centered, no scrolling.
        let mut ed = Editor::new();
        type_script_keys(&mut ed, "x+1");
        let v = render(&ed, 40, 12);
        assert_eq!((v.scroll_x, v.scroll_y), (0, 0));
        // Wide: the cursor sits at the right end, so the view scrolled.
        let mut ed = Editor::new();
        type_script_keys(&mut ed, &"a+".repeat(40));
        let v = render(&ed, 24, 12);
        assert!(v.scroll_x > 0, "no horizontal scroll: {}", v.scroll_x);
        assert_eq!(v.scroll_y, 0);
        // Tall: many display lines, cursor on the last one.
        let mut ed = Editor::new();
        for _ in 0..12 {
            ed.input(Key::Char('x'), false, false);
            ed.input(Key::Enter, false, false);
        }
        let v = render(&ed, 40, 8);
        assert!(v.scroll_y > 0, "no vertical scroll: {}", v.scroll_y);
        // …and moving back to the top scrolls back.
        ed.input(Key::Char('a'), false, true); // ^A: document start
        let v = render(&ed, 40, 8);
        assert_eq!(v.scroll_y, 0, "did not scroll back to the top");
    }

    #[test]
    fn block_mode_paints_the_ancestor_gradient() {
        // Cursor in a cell of a fused (matrix): ^B shows two ancestors
        // — the Array (innermost, highlighted) and the Delim — painted
        // as distinct boxes: the interior gets the selected color, the
        // delimiter columns the next gradient shade.
        let mut ed = Editor::new();
        for c in "\\pmatrix\n".chars() {
            let key = if c == '\n' { Key::Enter } else { Key::Char(c) };
            ed.input(key, false, false);
        }
        ed.input(Key::Char('x'), false, false);
        ed.input(Key::Char('b'), false, true);
        assert_eq!(ed.block.as_ref().map(Vec::len), Some(2));
        let (root, cursor) = ed.decorated();
        let cursor_ref = cursor.as_ref().map(|(p, c)| (p.as_slice(), *c));
        let block = render_root(&root, cursor_ref, &RenderCtx { italic: true });
        let (lines, bg, _) = marker_boxes(
            &block.lines,
            &ed.marker_extents(),
            &block.marks,
            block.caret,
            None,
            Some(ed.block_sel),
        );
        // Left delimiter column = outer (Delim) shade; some interior
        // cell = the highlighted innermost (Array) color.
        let row = block.baseline;
        assert_eq!(
            bg[row][0],
            Some(theme::DEPTH_BG[1]),
            "delim column shade\n{:?}",
            lines
        );
        let inner = bg[row][2..lines[row].len() - 1]
            .iter()
            .filter_map(|c| *c)
            .collect::<Vec<_>>();
        assert!(
            inner.contains(&theme::SELECTED_BG),
            "interior highlighted: {:?}",
            inner
        );
    }

    use super::*;
    use mascii::input::Key;

    /// Full display pipeline: decorated AST -> render -> marker_boxes.
    fn display(ed: &Editor) -> Vec<String> {
        let (root, cursor) = ed.decorated();
        let cursor_ref = cursor.as_ref().map(|(p, c)| (p.as_slice(), *c));
        let ctx = RenderCtx { italic: true };
        let block = render_root(&root, cursor_ref, &ctx);
        let (lines, _, _) = marker_boxes(
            &block.lines,
            &ed.marker_extents(),
            &block.marks,
            block.caret,
            ed.jump.is_some().then_some(ed.jump_selected),
            ed.block.is_some().then_some(ed.block_sel),
        );
        lines.into_iter().map(String::from_iter).collect()
    }

    #[test]
    fn minibuffer_overlays_at_the_cursor_without_layout_shift() {
        let mut ed = Editor::new();
        type_script_keys(&mut ed, "a+b");
        ed.input(Key::Left, false, false);
        ed.input(Key::Left, false, false);
        let before = display(&ed);
        ed.input(Key::Char('\\'), false, false);
        type_script_keys(&mut ed, "fr");
        let (root, cursor) = ed.decorated();
        let cursor_ref = cursor.as_ref().map(|(p, c)| (p.as_slice(), *c));
        let block = render_root(&root, cursor_ref, &RenderCtx { italic: true });
        let (mut lines, mut bg, mut cursor_cell) = marker_boxes(
            &block.lines,
            &ed.marker_extents(),
            &block.marks,
            block.caret,
            None,
            None,
        );
        let (cy, cx) = cursor_cell.unwrap();
        overlay_minibuffer(&ed, &mut lines, &mut bg, &mut cursor_cell);
        // The overlay covers the glyphs to the right of the cursor in
        // place: same height, and the cells left of the cursor are
        // untouched.
        assert_eq!(lines.len(), before.len(), "no vertical shift");
        let text: String = lines[cy][cx..cx + 3].iter().collect();
        assert_eq!(text, "\\fr", "typed command shown at the cursor");
        let kept: String = lines[cy][..cx].iter().collect();
        assert_eq!(kept, before[cy].chars().take(cx).collect::<String>());
        assert_eq!(cursor_cell, Some((cy, cx + 3)), "caret after the text");
        assert!(bg[cy][cx].is_some(), "overlay cells are tinted");
    }

    #[test]
    fn struck_cells_survive_cursor_decoration() {
        // A cancel next to the cursor: cell indexing must not shift and
        // the base+U+0338 ligature must stay inside one span (the bug:
        // to_strings() embedded the strike, so char index != column and
        // the caret split the ligature — the glyph vanished).
        let mut ed = Editor::new();
        type_script_keys(&mut ed, "ab");
        ed.input(Key::Left, true, false); // select b
        ed.input(Key::Char('\\'), false, false);
        for c in "cancel".chars() {
            ed.input(Key::Char(c), false, false);
        }
        ed.input(Key::Enter, false, false);
        // Cursor sits right of the struck b; walk it across the strike.
        for _ in 0..3 {
            let (root, cursor) = ed.decorated();
            let cursor_ref = cursor.as_ref().map(|(p, c)| (p.as_slice(), *c));
            let block = render_root(&root, cursor_ref, &RenderCtx { italic: true });
            let struck: std::collections::HashSet<(usize, usize)> =
                block.cancel.iter().copied().collect();
            assert!(!struck.is_empty(), "cancel present");
            let (lines, bg, cursor_cell) = marker_boxes(
                &block.lines,
                &ed.marker_extents(),
                &block.marks,
                block.caret,
                None,
                None,
            );
            let (y, x) = cursor_cell.expect("caret visible");
            let spans = decorate_line(&lines[y], y, &struck, &bg[y], Some(x), false, 0);
            let joined: String = spans.iter().map(|s| s.content.as_ref()).collect();
            // Every struck glyph is still there, followed by its strike.
            for &(r, c) in &block.cancel {
                if r == y {
                    let cell: Vec<char> = joined.chars().collect();
                    // find the base char count up to cell c: strikes are
                    // combining, so count non-combining chars.
                    let mut col = 0usize;
                    let mut ok = false;
                    let mut it = cell.iter().peekable();
                    while let Some(&ch) = it.next() {
                        if ch == '\u{338}' {
                            continue;
                        }
                        if col == c {
                            ok = it.peek() == Some(&&'\u{338}');
                            break;
                        }
                        col += 1;
                    }
                    assert!(ok, "strike stays glued at col {}: {:?}", c, joined);
                }
            }
            ed.input(Key::Left, false, false);
        }
    }

    fn type_script_keys(ed: &mut Editor, s: &str) {
        for c in s.chars() {
            ed.input(Key::Char(c), false, false);
        }
    }

    #[test]
    fn selection_box_paints_without_touching_the_text() {
        let lines: Vec<Vec<char>> = vec![" ab ".chars().collect()];
        let marks = [(0, 1, SEL_OPEN), (0, 3, SEL_CLOSE)];
        let (out, bg, _) = marker_boxes(&lines, &[(0, 0, 0)], &marks, None, None, None);
        assert_eq!(out, lines, "text must be untouched");
        assert_eq!(
            bg[0],
            vec![
                None,
                Some(theme::SELECTION_BG),
                Some(theme::SELECTION_BG),
                None
            ]
        );
    }

    #[test]
    fn selection_box_covers_the_block_extent_only() {
        // A selected fraction: rows from the extent, not from content
        // scanning — the denominator row below the box stays unpainted
        // when the extent says so.
        let lines: Vec<Vec<char>> = vec![
            " 1 ".chars().collect(),
            "───".chars().collect(),
            " 2 ".chars().collect(),
        ];
        let marks = [(1, 0, SEL_OPEN), (1, 3, SEL_CLOSE)];
        let (_, bg, _) = marker_boxes(&lines, &[(1, 1, 0)], &marks, None, None, None);
        assert!(bg.iter().all(|row| row.iter().all(|c| c.is_some())));
        let (_, bg, _) = marker_boxes(&lines, &[(0, 0, 0)], &marks, None, None, None);
        assert!(
            bg[2].iter().all(|c| c.is_none()),
            "extent must bound the box"
        );
    }

    #[test]
    fn labels_overlay_and_caret_pads() {
        let label = char::from_u32(JUMP_CHAR_BASE).unwrap();
        let lines: Vec<Vec<char>> = vec!["xy".chars().collect()];
        let (out, _, cursor) =
            marker_boxes(&lines, &[], &[(0, 0, label)], Some((0, 2)), None, None);
        let chars: Vec<char> = out[0].clone();
        assert_eq!(chars[0], label, "label over the glyph");
        assert_eq!(chars[1], 'y');
        assert_eq!(cursor, Some((0, 2)));
        assert_eq!(chars.len(), 3, "caret cell padded at the row end");
    }

    #[test]
    fn caret_display_never_shifts_the_layout() {
        // (x^a, not x^2: an inlinable script like ² must re-expand to 2D
        // while the caret is inside it — that shift is inherent.)
        let mut ed = Editor::new();
        for k in "x^a".chars() {
            ed.input(Key::Char(k), false, false);
        }
        ed.input(Key::Tab, false, false);
        ed.input(Key::Char('+'), false, false);
        ed.input(Key::Char('/'), false, false);
        ed.input(Key::Char('/'), false, false);
        ed.input(Key::Char('1'), false, false);
        ed.input(Key::Down, false, false);
        ed.input(Key::Char('2'), false, false);
        let ctx = RenderCtx { italic: true };
        let plain: Vec<String> = render_root(&ed.root, None, &ctx)
            .to_strings()
            .iter()
            .map(|l| l.trim_end().to_string())
            .collect();
        // Every cursor position must display with the same geometry as
        // the cursor-less render (the caret is an overlay, not a column).
        for cand in ed.jump_candidates() {
            if cand.is_cursor {
                continue;
            }
            let (p, c) = cand.pos;
            ed.path = p;
            ed.col = c;
            let got: Vec<String> = display(&ed)
                .iter()
                .map(|l| l.trim_end().to_string())
                .collect();
            assert_eq!(
                got, plain,
                "layout shifted at path {:?} col {}",
                ed.path, ed.col
            );
        }
    }

    #[test]
    fn ghost_slots_keep_jump_reentry_stable() {
        // \sum then exit: the empty band shows as a bare ∑. The first
        // ^G materializes the limit slots (a necessary shift); after
        // cancelling, the slots stay as ghost ⬚ until the next input,
        // so the second ^G overlays the identical picture.
        let mut ed = Editor::new();
        ed.input(Key::Char('\\'), false, false);
        for k in "sum".chars() {
            ed.input(Key::Char(k), false, false);
        }
        ed.input(Key::Enter, false, false);
        ed.input(Key::Tab, false, false);
        let plain = display(&ed);
        ed.input(Key::Char('g'), false, true);
        let jump1 = display(&ed);
        assert_ne!(plain, jump1, "slots must materialize under ^G");
        ed.input(Key::Esc, false, false); // cancel: slots become ghosts
        let ghost = display(&ed);
        assert!(
            ghost.concat().contains('⬚'),
            "ghost slots stay visible:\n{}",
            ghost.join("\n")
        );
        ed.input(Key::Char('g'), false, true);
        let jump2 = display(&ed);
        assert_eq!(jump1, jump2, "re-entering ^G must not shift");
        assert_eq!(jump2.len(), ghost.len());
        for (m, e) in jump2.iter().zip(&ghost) {
            let (m, e): (Vec<char>, Vec<char>) = (m.chars().collect(), e.chars().collect());
            for x in 0..m.len().max(e.len()) {
                let (mc, ec) = (
                    m.get(x).copied().unwrap_or(' '),
                    e.get(x).copied().unwrap_or(' '),
                );
                assert!(
                    mc == ec || (0xE000..0xE100).contains(&(mc as u32)),
                    "ghost/jump mismatch at {}: {:?} vs {:?}",
                    x,
                    mc,
                    ec
                );
            }
        }
        // Any real input clears the ghosts.
        ed.input(Key::Esc, false, false);
        ed.input(Key::Char('x'), false, false);
        assert!(!display(&ed).concat().contains('⬚'));
    }

    #[test]
    fn mode_displays_keep_the_editing_geometry() {
        // Formulas whose editing view differs from the plain one: an
        // empty-limit ∑ band, a fused matrix, an inline superscript.
        // Entering ^G / ^B must keep the geometry — cells may only
        // change where a label got overlaid.
        for keys in [
            vec!["\\", "s", "u", "m", "\n", "x"],
            vec!["\\", "p", "m", "a", "t", "r", "i", "x", "\n", "a", ">", "b"],
            vec!["x", "^", "2"],
        ] {
            let mut ed = Editor::new();
            for k in keys {
                match k {
                    "\n" => ed.input(Key::Enter, false, false),
                    ">" => ed.input(Key::Right, false, false),
                    k => ed.input(Key::Char(k.chars().next().unwrap()), false, false),
                };
            }
            let editing = display(&ed);
            for key in ['g', 'b'] {
                ed.input(Key::Char(key), false, true);
                let mode = display(&ed);
                assert_eq!(mode.len(), editing.len(), "height changed in ^{}", key);
                for (y, (m, e)) in mode.iter().zip(&editing).enumerate() {
                    let (m, e): (Vec<char>, Vec<char>) = (m.chars().collect(), e.chars().collect());
                    for x in 0..m.len().max(e.len()) {
                        let (mc, ec) = (
                            m.get(x).copied().unwrap_or(' '),
                            e.get(x).copied().unwrap_or(' '),
                        );
                        let label = (0xE000..0xE100).contains(&(mc as u32));
                        assert!(
                            mc == ec || label,
                            "^{} shifted cell ({}, {}): {:?} vs {:?}",
                            key,
                            y,
                            x,
                            mc,
                            ec
                        );
                    }
                }
                ed.input(Key::Esc, false, false);
            }
        }
    }
}
