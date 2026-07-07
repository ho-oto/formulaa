use std::fs;

use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block as UiBlock, Borders, Paragraph};
use ratatui::Frame;

use mascii::render::{render_row, RenderCtx, CURSOR_CHAR};
use mascii::{ast, editor::Editor, latex, parse, typst};

const HELP: &str = "\\cmd  ^ sup  _ sub  ( ) paren  ] exit matrix  Space exit inset  ←→↑↓ move  ⌫ delete  ^S save  ^Q quit  F2 italic";

const USAGE: &str = "\
usage: mascii [SAVE_PATH]          interactive TUI editor (default: formula.tex)
       mascii aa2tex   [FILE]     AA formula (file or stdin) -> LaTeX
       mascii aa2typst [FILE]     AA formula (file or stdin) -> Typst
       mascii fmt      [FILE]     AA formula -> canonical AA (normalize)";

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("aa2tex") | Some("aa2typst") | Some("fmt") => {
            return convert(&args[0].clone(), args.get(1).map(String::as_str));
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

/// Returns true when the app should quit.
fn handle_key(
    ed: &mut Editor,
    code: KeyCode,
    mods: KeyModifiers,
    save_path: &str,
) -> bool {
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
        KeyCode::Left => ed.left(),
        KeyCode::Right => ed.right(),
        KeyCode::Up => ed.vertical(true),
        KeyCode::Down => ed.vertical(false),
        KeyCode::Home => ed.home(),
        KeyCode::End => ed.end(),
        KeyCode::Backspace => ed.backspace(),
        KeyCode::Delete => ed.delete(),
        KeyCode::F(2) => ed.italic = !ed.italic,
        KeyCode::Char('\\') => ed.minibuffer = Some(String::new()),
        KeyCode::Char('^') => ed.insert_and_enter(ast::Node::Sup { arg: vec![] }),
        KeyCode::Char('_') => ed.insert_and_enter(ast::Node::Sub { arg: vec![] }),
        KeyCode::Char('(') => ed.insert_and_enter(ast::Node::Paren { inner: vec![] }),
        KeyCode::Char(')') => ed.close_paren(),
        KeyCode::Char('[') => {
            ed.message = "[ ] are reserved for matrices; insert one with \\matrix".into()
        }
        KeyCode::Char(']') => ed.close_bracket(),
        KeyCode::Char(' ') => ed.exit_inset(),
        KeyCode::Char(c) if c.is_ascii_graphic() => ed.insert_sym(c),
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

    let ctx = RenderCtx { italic: ed.italic, compact: false };
    let block = render_row(&ed.root, Some((&ed.path, ed.col)), false, &ctx);
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
    let cursor_style = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD | Modifier::SLOW_BLINK);
    for l in &lines {
        // Style the cursor glyph; everything else is plain text.
        let mut spans = vec![Span::raw(pad.clone())];
        for part in split_keep(l, CURSOR_CHAR) {
            match part {
                Part::Cursor => spans.push(Span::styled(CURSOR_CHAR.to_string(), cursor_style)),
                Part::Text(s) => spans.push(Span::raw(s)),
            }
        }
        text.push(Line::from(spans));
    }
    f.render_widget(Paragraph::new(text), inner);
}

enum Part {
    Text(String),
    Cursor,
}

fn split_keep(s: &str, sep: char) -> Vec<Part> {
    let mut out = Vec::new();
    let mut buf = String::new();
    for c in s.chars() {
        if c == sep {
            if !buf.is_empty() {
                out.push(Part::Text(std::mem::take(&mut buf)));
            }
            out.push(Part::Cursor);
        } else {
            buf.push(c);
        }
    }
    if !buf.is_empty() {
        out.push(Part::Text(buf));
    }
    out
}
