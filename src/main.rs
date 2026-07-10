use std::fs;

use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block as UiBlock, Borders, Paragraph};
use ratatui::Frame;

use mascii::render::{render_row, GlyphSet, RenderCtx, CURSOR_CHAR};
use mascii::parse::RegionSpan;
use mascii::editor::{
    Editor, HL_CLOSE_BASE, HL_LEVELS, HL_OPEN_BASE, JUMP_CHAR_BASE, JUMP_LABELS, SEL_CLOSE,
    SEL_OPEN,
};
use mascii::{ast, latex, parse, typst};

const HELP: &str = "\\cmd  ^/_ ( ) insets  Space exit  ←→↑↓ move  ⇧←→ select  ^G jump  ^B blocks  ^O structure  ^T italic  ^P compat  ^S save  ^Q quit";

const USAGE: &str = "\
usage: mascii [SAVE_PATH]          interactive TUI editor (default: formula.tex)
       mascii aa2tex   [FILE]     AA formula (file or stdin) -> LaTeX
       mascii aa2typst [FILE]     AA formula (file or stdin) -> Typst
       mascii fmt      [FILE]     AA formula -> canonical AA (normalize)
       mascii fmt --compat [FILE] AA formula -> compat AA (box-drawing/ASCII,
                                  display only; not re-parseable)";

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("aa2tex") | Some("aa2typst") | Some("fmt") => {
            let rest: Vec<&str> = args[1..].iter().map(String::as_str).collect();
            let compat = rest.contains(&"--compat");
            let file = rest.into_iter().find(|a| !a.starts_with("--"));
            return convert(&args[0].clone(), file, compat);
        }
        Some("-h") | Some("--help") => {
            println!("{}", USAGE);
            return Ok(());
        }
        _ => {}
    }
    let save_path = args.first().cloned().unwrap_or_else(|| "formula.tex".into());
    let mut terminal = ratatui::init();
    let mut ed = Editor::new();
    ed.message = format!("mascii — LyX-like math editor (saves to {})", save_path);

    let result = loop {
        if let Err(e) = terminal.draw(|f| draw(f, &ed)) {
            break Err(e);
        }
        match event::read() {
            Ok(Event::Key(key)) if key.kind == KeyEventKind::Press => {
                if handle_key(&mut ed, key.code, key.modifiers, &save_path) {
                    break Ok(());
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
fn convert(mode: &str, file: Option<&str>, compat: bool) -> std::io::Result<()> {
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
            let ctx = if compat { RenderCtx::compat() } else { RenderCtx::canonical() };
            let block = render_row(&row, None, false, &ctx);
            println!("{}", block.to_text());
        }
    }
    Ok(())
}

/// Returns true when the app should quit.
fn handle_key(
    ed: &mut Editor,
    code: KeyCode,
    mods: KeyModifiers,
    save_path: &str,
) -> bool {
    // Jump mode: the next key picks a label (Esc cancels).
    if ed.jump.is_some() {
        match code {
            KeyCode::Char(c) => ed.jump_to(c),
            _ => {
                ed.jump = None;
                ed.message.clear();
            }
        }
        return false;
    }

    // Minibuffer (`\command`) mode captures most keys.
    if ed.minibuffer.is_some() {
        match code {
            KeyCode::Esc => {
                ed.minibuffer = None;
            }
            KeyCode::Backspace => {
                let buf = ed.minibuffer.as_mut().unwrap();
                if buf.pop().is_none() {
                    ed.minibuffer = None;
                }
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                let cmd = ed.minibuffer.take().unwrap();
                ed.execute(&cmd);
            }
            // Graphic chars (not just alphanumerics): the extended symbol
            // table has names like "->", "+-", "oo".
            KeyCode::Char(c) if c.is_ascii_graphic() => {
                ed.minibuffer.as_mut().unwrap().push(c);
            }
            _ => {}
        }
        return false;
    }

    if mods.contains(KeyModifiers::CONTROL) {
        match code {
            KeyCode::Char('q') | KeyCode::Char('c') => return true,
            KeyCode::Char('g') => ed.start_jump(),
            KeyCode::Char('t') => ed.italic = !ed.italic,
            KeyCode::Char('p') => ed.compat = !ed.compat,
            KeyCode::Char('b') => ed.highlight = !ed.highlight,
            KeyCode::Char('o') => ed.structure = !ed.structure,
            KeyCode::Char('s') => {
                let tex = latex::row_to_latex(&ed.root);
                match fs::write(save_path, format!("{}\n", tex)) {
                    Ok(()) => ed.message = format!("saved LaTeX to {}", save_path),
                    Err(e) => ed.message = format!("save failed: {}", e),
                }
            }
            _ => {}
        }
        return false;
    }

    ed.message.clear();
    match code {
        KeyCode::Left if mods.contains(KeyModifiers::SHIFT) => ed.select_move(false),
        KeyCode::Right if mods.contains(KeyModifiers::SHIFT) => ed.select_move(true),
        KeyCode::Left => {
            ed.select_anchor = None;
            ed.left();
        }
        KeyCode::Right => {
            ed.select_anchor = None;
            ed.right();
        }
        KeyCode::Up => {
            ed.select_anchor = None;
            ed.vertical(true);
        }
        KeyCode::Down => {
            ed.select_anchor = None;
            ed.vertical(false);
        }
        KeyCode::Esc => ed.select_anchor = None,
        KeyCode::Home => ed.home(),
        KeyCode::End => ed.end(),
        KeyCode::Backspace => {
            if !ed.delete_selection() {
                ed.backspace();
            }
        }
        KeyCode::Delete => {
            if !ed.delete_selection() {
                ed.delete();
            }
        }
        // F-keys kept as aliases (often captured by the terminal/OS).
        KeyCode::F(2) => ed.italic = !ed.italic,
        KeyCode::F(3) => ed.compat = !ed.compat,
        KeyCode::F(4) => ed.highlight = !ed.highlight,
        KeyCode::F(5) => ed.structure = !ed.structure,
        KeyCode::Char('\\') => ed.minibuffer = Some(String::new()),
        KeyCode::Char('^') => {
            if !ed.wrap_selection(|c| ast::Node::Sup { arg: c }) {
                ed.insert_and_enter(ast::Node::Sup { arg: vec![] });
            }
        }
        KeyCode::Char('_') => {
            if !ed.wrap_selection(|c| ast::Node::Sub { arg: c }) {
                ed.insert_and_enter(ast::Node::Sub { arg: vec![] });
            }
        }
        KeyCode::Char('(') => {
            if !ed.wrap_selection(|c| ast::Node::Paren { inner: c }) {
                ed.insert_and_enter(ast::Node::Paren { inner: vec![] });
            }
        }
        KeyCode::Char(')') => ed.close_paren(),
        KeyCode::Char('[') => {
            ed.message = "[ ] are reserved for matrices; insert one with \\matrix".into()
        }
        KeyCode::Char(']') => ed.close_bracket(),
        KeyCode::Char(' ') => ed.exit_inset(),
        KeyCode::Char(c) if c.is_ascii_graphic() => {
            ed.select_anchor = None;
            ed.insert_sym(c);
        }
        _ => {}
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
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
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
            format!(" {}", HELP),
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

    let ctx = RenderCtx {
        italic: ed.italic && !ed.compat,
        compact: false,
        glyphs: if ed.compat { GlyphSet::Compat } else { GlyphSet::Unicode },
    };
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
    let lines = block.to_strings();

    let width = block.width() as u16;
    let height = lines.len() as u16;
    let left = inner.width.saturating_sub(width) / 2;
    let top = inner.height.saturating_sub(height) / 2;

    let mut text: Vec<Line> = Vec::with_capacity(top as usize + lines.len());
    for _ in 0..top {
        text.push(Line::raw(""));
    }
    let pad = " ".repeat(left as usize);
    for l in &lines {
        let mut spans = vec![Span::raw(pad.clone())];
        spans.extend(decorate_line(l));
        text.push(Line::from(spans));
    }
    f.render_widget(Paragraph::new(text), inner);
}

/// Colors for the enclosing-block markers, innermost first.
const HL_COLORS: [Color; HL_LEVELS] = [Color::Yellow, Color::Cyan, Color::DarkGray];

/// Turn a rendered line into spans: private-use marker chars become
/// colored jump labels / block brackets, the cursor glyph blinks.
fn decorate_line(line: &str) -> Vec<Span<'static>> {
    let cursor_style = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD | Modifier::SLOW_BLINK);
    let label_style = Style::default()
        .fg(Color::Black)
        .bg(Color::Yellow)
        .add_modifier(Modifier::BOLD);

    let mut spans = Vec::new();
    let mut buf = String::new();
    let flush = |buf: &mut String, spans: &mut Vec<Span<'static>>| {
        if !buf.is_empty() {
            spans.push(Span::raw(std::mem::take(buf)));
        }
    };
    for c in line.chars() {
        let u = c as u32;
        if c == CURSOR_CHAR {
            flush(&mut buf, &mut spans);
            spans.push(Span::styled(CURSOR_CHAR.to_string(), cursor_style));
        } else if (JUMP_CHAR_BASE..JUMP_CHAR_BASE + JUMP_LABELS.chars().count() as u32)
            .contains(&u)
        {
            let label = JUMP_LABELS
                .chars()
                .nth((u - JUMP_CHAR_BASE) as usize)
                .unwrap();
            flush(&mut buf, &mut spans);
            spans.push(Span::styled(label.to_string(), label_style));
        } else if c == SEL_OPEN || c == SEL_CLOSE {
            let glyph = if c == SEL_OPEN { '⟦' } else { '⟧' };
            flush(&mut buf, &mut spans);
            spans.push(Span::styled(
                glyph.to_string(),
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            ));
        } else if (HL_OPEN_BASE..HL_OPEN_BASE + HL_LEVELS as u32).contains(&u)
            || (HL_CLOSE_BASE..HL_CLOSE_BASE + HL_LEVELS as u32).contains(&u)
        {
            let (glyph, level) = if u >= HL_CLOSE_BASE {
                ('⟩', (u - HL_CLOSE_BASE) as usize)
            } else {
                ('⟨', (u - HL_OPEN_BASE) as usize)
            };
            flush(&mut buf, &mut spans);
            spans.push(Span::styled(
                glyph.to_string(),
                Style::default().fg(HL_COLORS[level]).add_modifier(Modifier::BOLD),
            ));
        } else {
            buf.push(c);
        }
    }
    flush(&mut buf, &mut spans);
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
