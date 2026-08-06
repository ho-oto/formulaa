use mascii::complete::complete;
use mascii::editor::Editor;
use mascii::input::Key;

fn k(ed: &mut Editor, key: Key) {
    ed.input(key, false, false);
}

fn main() {
    println!("complete(\"rm\").len() = {}", complete("rm").len());
    println!("complete(\"\").len() = {}", complete("").len());

    // ---- stale popup after a click dismisses the minibuffer
    let mut ed = Editor::new();
    for c in "x+y".chars() {
        k(&mut ed, Key::Char(c));
    }
    k(&mut ed, Key::Char('\\'));
    for c in "alpha".chars() {
        k(&mut ed, Key::Char(c));
    }
    k(&mut ed, Key::Tab);
    println!("popup open: {:?}", ed.completion.is_some());
    ed.click(0, 0);
    println!(
        "after click: minibuffer={:?} completion query={:?} items={:?}",
        ed.minibuffer,
        ed.completion.as_ref().map(|c| c.query.clone()),
        ed.completion
            .as_ref()
            .map(|c| c.items[c.sel].commit.clone())
    );
    // Re-open the minibuffer and press Enter immediately.
    k(&mut ed, Key::Char('\\'));
    println!(
        "after \\: minibuffer={:?} popup={:?}",
        ed.minibuffer,
        ed.completion
            .as_ref()
            .map(|c| c.items[c.sel].commit.clone())
    );
    k(&mut ed, Key::Enter);
    println!(
        "after Enter: latex={:?}",
        mascii::latex::row_to_latex(&mascii::ast::normalize(&ed.root))
    );

    // ---- Esc, then reopen
    let mut ed2 = Editor::new();
    k(&mut ed2, Key::Char('\\'));
    for c in "alpha".chars() {
        k(&mut ed2, Key::Char(c));
    }
    k(&mut ed2, Key::Tab);
    k(&mut ed2, Key::Esc);
    k(&mut ed2, Key::Esc);
    println!(
        "after 2x Esc: mb={:?} popup={:?}",
        ed2.minibuffer,
        ed2.completion.is_some()
    );

    // ---- Tab with an empty query
    let mut ed3 = Editor::new();
    k(&mut ed3, Key::Char('\\'));
    k(&mut ed3, Key::Tab);
    println!(
        "empty query popup: {:?}",
        ed3.completion
            .as_ref()
            .map(|c| (c.query.clone(), c.items.len()))
    );
    k(&mut ed3, Key::Enter);
    println!(
        "empty-query Enter: latex={:?}",
        mascii::latex::row_to_latex(&mascii::ast::normalize(&ed3.root))
    );

    // ---- backspace to empty
    let mut ed4 = Editor::new();
    k(&mut ed4, Key::Char('\\'));
    k(&mut ed4, Key::Char('a'));
    k(&mut ed4, Key::Tab);
    k(&mut ed4, Key::Backspace);
    println!(
        "after backspace-to-empty: mb={:?} popup={:?}",
        ed4.minibuffer,
        ed4.completion.as_ref().map(|c| c.query.clone())
    );
    k(&mut ed4, Key::Backspace);
    println!(
        "after 2nd backspace: mb={:?} popup={:?}",
        ed4.minibuffer,
        ed4.completion.is_some()
    );
}
