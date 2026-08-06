use mascii::editor::Editor;
use mascii::glyphs::Mark;
use mascii::input::Key;
use mascii::render::{RenderCtx, render_root};

fn keys(ed: &mut Editor, script: &str) {
    for tok in script.split_whitespace() {
        let (tok, shift, ctrl) = match tok.strip_prefix("S-") {
            Some(rest) => (rest, true, false),
            None => match tok.strip_prefix("C-") {
                Some(rest) => (rest, false, true),
                None => (tok, false, false),
            },
        };
        let named = match tok {
            "Left" => Some(Key::Left),
            "Right" => Some(Key::Right),
            "Up" => Some(Key::Up),
            "Down" => Some(Key::Down),
            "Home" => Some(Key::Home),
            "End" => Some(Key::End),
            "Tab" => Some(Key::Tab),
            "Enter" => Some(Key::Enter),
            "Backspace" => Some(Key::Backspace),
            "Delete" => Some(Key::Delete),
            "Esc" => Some(Key::Esc),
            "Space" => Some(Key::Char(' ')),
            _ => None,
        };
        if let Some(k) = named {
            ed.input(k, shift, ctrl);
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

fn dump(label: &str, ed: &Editor) {
    let (root, cursor) = ed.decorated();
    let cur = cursor.as_ref().map(|(p, c)| (p.as_slice(), *c));
    let b = render_root(&root, cur, &RenderCtx::canonical());
    println!("=== {} ===", label);
    for (y, line) in b.lines.iter().enumerate() {
        let s: String = line
            .iter()
            .map(|&c| if (c as u32) >= 0xE000 && (c as u32) <= 0xF8FF { '.' } else { c })
            .collect();
        println!("{:>2}| {}", y, s);
    }
    let mut ms: Vec<_> = b
        .marks
        .iter()
        .filter_map(|&(y, x, c)| Mark::decode(c).map(|m| (y, x, m)))
        .collect();
    ms.sort_by_key(|&(y, x, _)| (y, x));
    for (y, x, m) in ms {
        println!("   mark ({},{}) {:?}", y, x, m);
    }
    println!("   cursor path {:?}", cursor.as_ref().map(|(p, c)| (p.clone(), *c)));
}

fn main() {
    // 1. plain: (foo), cursor at start, one Backspace arms.
    let mut ed = Editor::new();
    keys(&mut ed, "( foo Home Backspace");
    dump("armed (foo) at top level", &ed);

    // 2. delimiter not first in the row: a+(foo)
    let mut ed = Editor::new();
    keys(&mut ed, "a+ ( foo Home Backspace");
    dump("armed a+(foo)", &ed);

    // 3. tall content: (\frac x y)
    let mut ed = Editor::new();
    keys(&mut ed, "( \\frac x Tab y Home Home Backspace");
    dump("armed (x/y)", &ed);

    // 4. nested: ((foo)) inner armed
    let mut ed = Editor::new();
    keys(&mut ed, "( ( foo Home Backspace");
    dump("armed inner of ((foo))", &ed);

    // 5. delim inside a sup at row start
    let mut ed = Editor::new();
    keys(&mut ed, "^ ( foo Home Backspace");
    dump("armed inside a leading sup", &ed);

    // 6. armed + selection at the same time? (mouse-free reachability check)
    let mut ed = Editor::new();
    keys(&mut ed, "( foo Home Backspace S-Right");
    dump("armed then shift-right", &ed);
}
