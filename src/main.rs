use std::fs;

use ratatui::crossterm;
use ratatui::crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
    MouseButton, MouseEventKind,
};

mod guard;
mod theme;
mod tui;

use formulaa::editor::Editor;
use formulaa::input::{Effect, Key};
use formulaa::render::{RenderCtx, render_root};
use formulaa::{ast, latex, parse};

const USAGE: &str = "\
usage: formulaa [--debug]       interactive TUI editor
       formulaa aa2tex   [FILE] AA formula (file or stdin) -> LaTeX
       formulaa tex2aa   [FILE] LaTeX math (file or stdin) -> AA, best effort
       formulaa fmt      [FILE] AA formula -> canonical AA (normalize)

--debug: on a roundtrip failure, keep the state and dump a report to
formulaa_debug/ (default: refuse the edit)";

fn main() -> std::io::Result<()> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let debug = args.iter().any(|a| a == "--debug");
    args.retain(|a| a != "--debug");
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

    let mut guard = guard::RoundtripGuard::new(debug);
    let mut origin = (0u16, 0u16);
    let mut view = tui::View::default();
    // The copy acknowledgement: a ~120ms inverted blip of the
    // selection, the only animation in the program.
    let mut blip_until: Option<std::time::Instant> = None;
    let result = loop {
        if std::mem::take(&mut ed.copy_flash) {
            blip_until = Some(std::time::Instant::now() + std::time::Duration::from_millis(120));
        }
        view.copy_blip = blip_until.is_some_and(|t| std::time::Instant::now() < t);
        if !view.copy_blip {
            blip_until = None;
        }
        if let Err(e) = terminal.draw(|f| origin = tui::draw(f, &ed, &mut view)) {
            break Err(e);
        }
        // While the blip is up, wait only until it ends; a quiet
        // timeout just redraws (turning the blip off again).
        if let Some(t) = blip_until {
            let left = t.saturating_duration_since(std::time::Instant::now());
            match event::poll(left) {
                Ok(false) => continue,
                Ok(true) => {}
                Err(e) => break Err(e),
            }
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
                    let (cx, cy) = (
                        (m.column - origin.0) as usize + view.scroll_x,
                        (m.row - origin.1) as usize + view.scroll_y,
                    );
                    // A click on a completion row accepts that row
                    // (the popup floats over the formula, so the two
                    // targets never overlap ambiguously).
                    let picked = view.popup.and_then(|(top, left, w, start, shown)| {
                        ((top..top + shown).contains(&cy) && (left..left + w).contains(&cx))
                            .then(|| start + (cy - top))
                    });
                    match picked {
                        Some(idx) => {
                            let fx = ed.completion_click(idx);
                            if handle_effect(&mut ed, fx) {
                                break Ok(());
                            }
                            guard.check(&mut ed);
                        }
                        None => ed.click(cx, cy),
                    }
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
        let row = ast::normalize(&formulaa::from_latex::row_from_latex(&text));
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
    // F2 aliases ^G (grid edit) for terminals that capture the chord.
    let key = match code {
        KeyCode::F(2) => {
            let _ = ed.input(Key::Char('g'), false, true);
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
    handle_effect(ed, effect)
}

/// Run an `Effect` the shared keymap returned; true means quit.
fn handle_effect(ed: &mut Editor, effect: Effect) -> bool {
    match effect {
        Effect::Quit => return true,
        // Yank: canonical AA to the system clipboard.
        Effect::CopyAa => {
            let aa = formulaa::render::export_aa(&ed.root);
            match copy_to_clipboard(&aa) {
                Ok(()) => ed.info("copied AA to clipboard"),
                Err(e) => ed.error(format!("could not reach the system clipboard: {}", e)),
            }
        }
        Effect::None => {}
    }
    false
}
