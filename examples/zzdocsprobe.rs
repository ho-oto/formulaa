use mascii::ast::normalize;
use mascii::editor::Editor;
use mascii::input::Key;
use mascii::latex::row_to_latex;

fn named(tok: &str) -> Option<Key> {
    Some(match tok {
        "Left" => Key::Left,
        "Right" => Key::Right,
        "Up" => Key::Up,
        "Down" => Key::Down,
        "Home" => Key::Home,
        "End" => Key::End,
        "Tab" => Key::Tab,
        "Enter" => Key::Enter,
        "Backspace" => Key::Backspace,
        "Delete" => Key::Delete,
        "Esc" => Key::Esc,
        "Space" => Key::Char(' '),
        _ => return None,
    })
}

fn type_script(ed: &mut Editor, script: &str) {
    for tok in script.split_whitespace() {
        let (tok, shift, ctrl) = match tok.strip_prefix("S-") {
            Some(rest) => (rest, true, false),
            None => match tok.strip_prefix("C-") {
                Some(rest) => (rest, false, true),
                None => (tok, false, false),
            },
        };
        if let Some(key) = named(tok) {
            ed.input(key, shift, ctrl);
        } else if let Some(cmd) = tok.strip_prefix('\\').filter(|c| !c.is_empty()) {
            ed.input(Key::Char('\\'), false, false);
            for c in cmd.chars() {
                ed.input(Key::Char(c), false, false);
            }
            ed.input(Key::Enter, false, false);
        } else {
            for c in tok.chars() {
                ed.input(Key::Char(c), shift, ctrl);
            }
        }
    }
}

fn latex(ed: &Editor) -> String {
    row_to_latex(&normalize(&ed.root))
}

fn trace(label: &str, steps: &[&str]) {
    println!("== {} ==", label);
    let mut ed = Editor::new();
    for s in steps {
        type_script(&mut ed, s);
        println!(
            "  after {:12} sel={:?} latex={}",
            s,
            ed.selection(),
            latex(&ed)
        );
    }
}

fn main() {
    trace(
        "set",
        &[r"\set", "x", "Home", "Backspace", "Backspace", "Backspace"],
    );
    trace(
        "pmatrix",
        &[r"\pmatrix", "x", "Home", "Backspace", "Backspace", "Backspace"],
    );
    trace("paren", &["(", "foo", "Home", "Backspace", "Backspace"]);
    trace("paren-ctrl-d", &["(", "foo", "C-d", "C-d"]);

    println!("\n== complete(\"al\") ==");
    for (i, it) in mascii::complete::complete("al").iter().enumerate() {
        println!("  {} sym={:6} names={}  commit={}", i, it.symbol, it.names, it.commit);
    }
    println!("== complete(\"in\") ==");
    for (i, it) in mascii::complete::complete("in").iter().enumerate() {
        println!("  {} sym={:6} names={}  commit={}", i, it.symbol, it.names, it.commit);
    }
    println!("== complete(\"m\") ==");
    for it in mascii::complete::complete("m").iter() {
        println!("  sym={:?} names={:?}", it.symbol, it.names);
    }
    println!("== complete(\"addrow\") ==");
    for it in mascii::complete::complete("addrow").iter().take(4) {
        println!("  sym={:?} names={:?}", it.symbol, it.names);
    }
}
