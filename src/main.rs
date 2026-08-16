use std::fs;
use std::path::{Path, PathBuf};

use clap::Parser;
use ratatui::crossterm;
use ratatui::crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
    MouseButton, MouseEventKind,
};

mod guard;
mod theme;
mod tui;

use formulaa::ast::{self, Row};
use formulaa::editor::Editor;
use formulaa::input::{Effect, Key};
use formulaa::render::{RenderCtx, export_aa, render_root};
use formulaa::{latex, parse};

/// WYSIWYG TUI math editor rendering Unicode/ASCII-art formulas.
#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Formula to edit: an AA file (created when you save it) or `-`
    /// for standard input. Without one the editor starts empty.
    file: Option<String>,

    /// Print the canonical AA instead of editing (the formatter).
    #[arg(long, group = "convert")]
    format: bool,
    /// Print the formula as LaTeX instead of editing.
    #[arg(long, alias = "aa2tex", group = "convert")]
    aa2latex: bool,
    /// Read LaTeX math and print it as AA, best effort.
    #[arg(long, alias = "tex2aa", group = "convert")]
    latex2aa: bool,
}

fn main() -> std::io::Result<()> {
    let cli = Cli::parse();
    if cli.format || cli.aa2latex || cli.latex2aa {
        return convert(&cli);
    }

    let (mut ed, mut file) = open(cli.file.as_deref())?;
    let mut terminal = ratatui::init();
    let _ = crossterm::execute!(std::io::stdout(), EnableMouseCapture);

    let mut guard = guard::RoundtripGuard::default();
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
        view.title = file.title(&ed);
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
                if handle_key(&mut ed, key.code, key.modifiers, &mut file) {
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
                            if handle_effect(&mut ed, fx, &mut file) {
                                break Ok(());
                            }
                            guard.check(&mut ed);
                        }
                        None => {
                            ed.click(cx, cy);
                            // A click can commit an open box (a real
                            // edit), so it faces the guard like any
                            // keystroke.
                            guard.check(&mut ed);
                        }
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

/// The document to start from: a file (which need not exist yet — the
/// name is where the first save goes), standard input, or nothing.
/// A formula that does not parse is fatal: the editor would silently
/// stand in for a file it cannot read back.
fn open(file: Option<&str>) -> std::io::Result<(Editor, File)> {
    let (text, path) = match file {
        None => (None, None),
        Some("-") => (Some(std::io::read_to_string(std::io::stdin())?), None),
        Some(name) => {
            let path = PathBuf::from(name);
            match fs::read_to_string(&path) {
                Ok(text) => (Some(text), Some(path)),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => (None, Some(path)),
                Err(e) => die(name, &e),
            }
        }
    };
    let mut ed = Editor::new();
    if let Some(text) = text
        && !text.trim().is_empty()
    {
        match parse::parse(&text) {
            Ok(row) => ed.root = row,
            Err(e) => die(file.unwrap_or("-"), &e),
        }
        ed.document_end();
    }
    let saved = ed.root.clone();
    Ok((
        ed,
        File {
            path,
            saved,
            quit_after: false,
        },
    ))
}

fn die(name: &str, e: &dyn std::fmt::Display) -> ! {
    eprintln!("{}: {}", name, e);
    std::process::exit(1)
}

/// What the host knows about the document and the editor does not:
/// where it came from, and what the copy on disk holds.
struct File {
    path: Option<PathBuf>,
    /// The formula as last read or written. Anything else is unsaved.
    saved: Row,
    /// A save that is the first half of leaving (^W, or answering the
    /// unsaved-work question), waiting for the name to be typed.
    quit_after: bool,
}

impl File {
    fn dirty(&self, ed: &Editor) -> bool {
        ed.root != self.saved
    }

    fn title(&self, ed: &Editor) -> String {
        let mut title = String::from("formulAA");
        if let Some(path) = &self.path {
            title.push_str(&format!(" — {}", path_name(path)));
        }
        if self.dirty(ed) {
            title.push_str(" *");
        }
        title
    }

    /// Save. False when nothing was written: either the name is still
    /// being asked for, or the write failed and said so.
    fn write(&mut self, ed: &mut Editor) -> bool {
        let Some(path) = self.path.clone() else {
            ed.ask_path("");
            return false;
        };
        match fs::write(&path, format!("{}\n", export_aa(&ed.root))) {
            Ok(()) => {
                self.saved = ed.root.clone();
                ed.info(format!("wrote {}", path_name(&path)));
                true
            }
            Err(e) => {
                ed.error(format!("could not write {}: {}", path_name(&path), e));
                false
            }
        }
    }
}

fn path_name(p: &Path) -> &str {
    p.to_str().unwrap_or("(file)")
}

/// The conversions, which read a file or stdin and write stdout.
fn convert(cli: &Cli) -> std::io::Result<()> {
    let text = match cli.file.as_deref() {
        Some(path) if path != "-" => fs::read_to_string(path)?,
        _ => std::io::read_to_string(std::io::stdin())?,
    };
    let name = cli.file.as_deref().unwrap_or("-");
    if cli.latex2aa {
        let row = ast::normalize(&formulaa::from_latex::row_from_latex(&text));
        println!(
            "{}",
            render_root(&row, None, &RenderCtx::canonical()).to_text()
        );
        return Ok(());
    }
    let row = match parse::parse(&text) {
        Ok(row) => row,
        Err(e) => die(name, &e),
    };
    if cli.aa2latex {
        println!("{}", latex::row_to_latex(&row));
    } else {
        println!(
            "{}",
            render_root(&row, None, &RenderCtx::canonical()).to_text()
        );
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
fn handle_key(ed: &mut Editor, code: KeyCode, mods: KeyModifiers, file: &mut File) -> bool {
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
    handle_effect(ed, effect, file)
}

/// Run an `Effect` the shared keymap returned; true means quit.
fn handle_effect(ed: &mut Editor, effect: Effect, file: &mut File) -> bool {
    match effect {
        // Leaving with unsaved work asks first; the answer comes back
        // as WriteQuit or Discard.
        Effect::Quit => {
            if !file.dirty(ed) {
                return true;
            }
            file.quit_after = true;
            ed.ask_save_first();
        }
        Effect::Discard => return true,
        Effect::Write => {
            file.quit_after = false;
            file.write(ed);
        }
        Effect::WriteQuit => {
            file.quit_after = true;
            if file.write(ed) {
                return true;
            }
        }
        Effect::WriteTo(name) => {
            file.path = Some(PathBuf::from(name));
            let leaving = std::mem::take(&mut file.quit_after);
            if file.write(ed) && leaving {
                return true;
            }
        }
        Effect::CopyAa => {
            let aa = export_aa(&ed.root);
            match copy_to_clipboard(&aa) {
                Ok(()) => ed.info("copied AA to clipboard"),
                Err(e) => ed.error(format!("could not reach the system clipboard: {}", e)),
            }
        }
        Effect::None => {}
    }
    false
}
