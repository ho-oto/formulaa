use std::fs;

use ratatui::crossterm;
use ratatui::crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
    MouseButton, MouseEventKind,
};

mod guard;
mod theme;
mod tui;

use mascii::editor::Editor;
use mascii::input::{Effect, Key};
use mascii::render::{RenderCtx, render_root};
use mascii::{ast, latex, parse};

const USAGE: &str = "\
usage: mascii                 interactive TUI editor
       mascii aa2tex   [FILE] AA formula (file or stdin) -> LaTeX
       mascii tex2aa   [FILE] LaTeX math (file or stdin) -> AA, best effort
       mascii fmt      [FILE] AA formula -> canonical AA (normalize)";

/// View state the editor itself does not own: the scroll offset of the
/// canvas (a formula larger than the terminal does not fit).
fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some(m @ ("aa2tex" | "tex2aa" | "fmt")) => {
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
    ed.info("mascii — LyX-like math editor");

    let mut guard = guard::RoundtripGuard::default();
    let mut origin = (0u16, 0u16);
    let mut view = tui::View::default();
    let result = loop {
        if let Err(e) = terminal.draw(|f| origin = tui::draw(f, &ed, &mut view)) {
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

/// aa2tex / tex2aa / fmt: read text (file or stdin), write to stdout.
fn convert(mode: &str, file: Option<&str>) -> std::io::Result<()> {
    let text = match file {
        Some(path) => fs::read_to_string(path)?,
        None => std::io::read_to_string(std::io::stdin())?,
    };
    if mode == "tex2aa" {
        let row = ast::normalize(&mascii::from_latex::row_from_latex(&text));
        let block = render_root(&row, None, &RenderCtx::canonical());
        println!("{}", block.to_text());
        return Ok(());
    }
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
            let aa = mascii::render::export_aa(&ed.root);
            match copy_to_clipboard(&aa) {
                Ok(()) => ed.info("copied AA to clipboard"),
                Err(e) => ed.error(format!("copy failed: {}", e)),
            }
        }
        Effect::None => {}
    }
    false
}
