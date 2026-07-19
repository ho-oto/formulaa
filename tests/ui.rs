//! Key-driven UI tests: everything goes through the shared keymap
//! (`Editor::input`), i.e. the exact path the TUI and the wasm bindings
//! use, so these tests pin down "press this, get that" behaviour.
//!
//! A tiny key-script DSL keeps the cases readable:
//!   - a bare token is typed character by character (`x+1` = three keys)
//!   - `\frac` types the minibuffer command and executes it (trailing
//!     Enter); a lone `\` just opens the minibuffer
//!   - named keys: Left Right Up Down Home End Tab Enter Backspace
//!     Delete Esc Space, with `S-` (shift) / `C-` (ctrl) prefixes

use mascii::ast::{normalize, strip_spacers};
use mascii::editor::Editor;
use mascii::input::{Effect, Key};
use mascii::latex::row_to_latex;
use mascii::parse::parse;
use mascii::render::{RenderCtx, render_row};

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

fn type_script(ed: &mut Editor, script: &str) -> Vec<Effect> {
    let mut effects = Vec::new();
    for tok in script.split_whitespace() {
        let (tok, shift, ctrl) = match tok.strip_prefix("S-") {
            Some(rest) => (rest, true, false),
            None => match tok.strip_prefix("C-") {
                Some(rest) => (rest, false, true),
                None => (tok, false, false),
            },
        };
        if let Some(key) = named(tok) {
            effects.push(ed.input(key, shift, ctrl));
        } else if let Some(cmd) = tok.strip_prefix('\\').filter(|c| !c.is_empty()) {
            effects.push(ed.input(Key::Char('\\'), false, false));
            for c in cmd.chars() {
                effects.push(ed.input(Key::Char(c), false, false));
            }
            effects.push(ed.input(Key::Enter, false, false));
        } else {
            for c in tok.chars() {
                effects.push(ed.input(Key::Char(c), shift, ctrl));
            }
        }
    }
    effects
}

fn latex(ed: &Editor) -> String {
    row_to_latex(&normalize(&ed.root))
}

fn aa(ed: &Editor) -> String {
    render_row(&normalize(&ed.root), None, false, &RenderCtx::canonical()).to_text()
}

#[test]
fn typing_a_formula_by_keys() {
    let mut ed = Editor::new();
    type_script(&mut ed, r"x ^ 2 Tab + \frac 1 Down 2");
    assert_eq!(latex(&ed), "x^{2}+\\frac{1}{2}");
}

#[test]
fn minibuffer_esc_cancels_and_backspace_erases() {
    let mut ed = Editor::new();
    type_script(&mut ed, r"\ f r a Esc a");
    assert_eq!(latex(&ed), "a");
    // Backspacing past the start closes the minibuffer.
    let mut ed = Editor::new();
    type_script(&mut ed, r"\ f Backspace Backspace b");
    assert_eq!(latex(&ed), "b");
    assert!(ed.minibuffer.is_none());
}

#[test]
fn selection_wraps_into_fraction() {
    let mut ed = Editor::new();
    type_script(&mut ed, r"a+b S-Left S-Left S-Left \frac 2");
    assert_eq!(latex(&ed), "\\frac{a+b}{2}");
}

#[test]
fn ctrl_keys_return_host_effects() {
    let mut ed = Editor::new();
    assert_eq!(ed.input(Key::Char('q'), false, true), Effect::Quit);
    assert_eq!(ed.input(Key::Char('s'), false, true), Effect::SaveTex);
    assert_eq!(ed.input(Key::Char('y'), false, true), Effect::CopyAa);
    // Plain typing never asks the host for anything.
    assert_eq!(ed.input(Key::Char('q'), false, false), Effect::None);
}

#[test]
fn esc_cancels_modes_before_quitting() {
    let mut ed = Editor::new();
    // With a selection, Esc only clears it …
    type_script(&mut ed, "a S-Left");
    assert_eq!(ed.input(Key::Esc, false, false), Effect::None);
    assert_eq!(ed.selection(), None);
    // … in the minibuffer it closes that …
    assert_eq!(ed.input(Key::Char('\\'), false, false), Effect::None);
    assert_eq!(ed.input(Key::Esc, false, false), Effect::None);
    assert!(ed.minibuffer.is_none());
    // … and with nothing left to cancel it quits.
    assert_eq!(ed.input(Key::Esc, false, false), Effect::Quit);
}

#[test]
fn enter_adds_a_grid_row() {
    let mut ed = Editor::new();
    type_script(&mut ed, r"\pmatrix a Enter");
    let aa = aa(&ed);
    // 2×2 pmatrix plus one added row = 3 rows ⇒ two ┼ separator rows.
    assert_eq!(aa.matches('┼').count(), 2, "grid:\n{}", aa);
}

#[test]
fn brackets_insert_a_delimiter_pair() {
    let mut ed = Editor::new();
    type_script(&mut ed, "[ x ]");
    assert_eq!(latex(&ed), "\\left[x\\right]");
    // `"` stays reserved (text runs).
    type_script(&mut ed, "\"");
    assert!(ed.message.contains("text"), "message: {}", ed.message);
}

#[test]
fn double_slash_makes_a_fraction() {
    let mut ed = Editor::new();
    type_script(&mut ed, "a // 1 Down 2 Tab");
    assert_eq!(latex(&ed), "a\\frac{1}{2}");
    // A lone slash stays an atom.
    let mut ed = Editor::new();
    type_script(&mut ed, "a / b");
    assert_eq!(latex(&ed), "a/b");
}

#[test]
fn copy_cut_paste_by_keys() {
    let mut ed = Editor::new();
    type_script(&mut ed, "a+b S-Left S-Left C-c End C-v");
    assert_eq!(latex(&ed), "a+b+b");
    type_script(&mut ed, "S-Left S-Left C-x Home C-v");
    assert_eq!(latex(&ed), "+ba+b");
}

#[test]
fn arrows_collapse_selection_to_its_ends() {
    let mut ed = Editor::new();
    type_script(&mut ed, "a+b S-Left S-Left Left x");
    assert_eq!(latex(&ed), "ax+b");
    let mut ed = Editor::new();
    type_script(&mut ed, "a+b S-Left S-Left Right y");
    assert_eq!(latex(&ed), "a+by");
}

#[test]
fn ctrl_a_jumps_to_document_start() {
    let mut ed = Editor::new();
    type_script(&mut ed, r"a // 1 Down 2 C-a x");
    assert_eq!(latex(&ed), "xa\\frac{1}{2}");
    assert!(ed.path.is_empty());
}

#[test]
fn click_moves_the_cursor() {
    // Flat row: clicking between a and + lands the cursor at col 1.
    let mut ed = Editor::new();
    type_script(&mut ed, "a+b");
    ed.click(1, 0);
    assert!(ed.path.is_empty());
    assert_eq!(ed.col, 1);
    // Clicking the denominator row of a fraction enters it.
    let mut ed = Editor::new();
    type_script(&mut ed, r"x + \frac 1 Down 22 Tab");
    ed.click(4, 2);
    assert!(
        matches!(ed.path.last(), Some((_, mascii::ast::Field::FracDen))),
        "path: {:?}",
        ed.path
    );
}

#[test]
fn shift_up_selects_enclosing_structure() {
    let mut ed = Editor::new();
    type_script(&mut ed, r"x + \frac 1 Down 2 S-Up Backspace");
    assert_eq!(latex(&ed), "x+");
}

#[test]
fn space_is_a_formatting_spacer() {
    let mut ed = Editor::new();
    type_script(&mut ed, "a Space b");
    assert_eq!(aa(&ed), "𝑎 𝑏");
    assert_eq!(latex(&ed), "ab"); // spacers never reach LaTeX
}

#[test]
fn selection_does_not_survive_leaving_the_row() {
    // Select inside the numerator, Tab out, then ^: the stale anchor
    // must not wrap outer-row nodes (this used to panic via a
    // drain past the row end — found by the random-key property test).
    let mut ed = Editor::new();
    type_script(&mut ed, r"\frac ab S-Left Tab ^ x");
    assert_eq!(latex(&ed), "\\frac{ab}{}^{x}");
}

#[test]
fn block_select_mode_selects_a_structure() {
    let mut ed = Editor::new();
    // 1 + a fraction; ^B labels the fraction 'a', picking it selects it.
    type_script(&mut ed, r"1 // 2 Down 3 Tab C-b");
    // The label marker must actually appear in the decorated view
    // (this display path once silently missed the block branch).
    let (root, cursor) = ed.decorated();
    assert!(cursor.is_none(), "cursor hidden while labels are shown");
    assert!(
        root.iter().any(
            |n| matches!(n, mascii::ast::Node::Sym(c) if (0xE000..0xE100).contains(&(*c as u32)))
        ),
        "label marker missing: {:?}",
        root
    );
    type_script(&mut ed, "a");
    assert_eq!(ed.selection(), Some((1, 2)));
    type_script(&mut ed, "Backspace");
    assert_eq!(latex(&ed), "1");
}

// ----- random key sequences: never panic, always roundtrip -----

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn pick<'a, T>(&mut self, xs: &'a [T]) -> &'a T {
        &xs[(self.next() % xs.len() as u64) as usize]
    }
}

/// The same invariant the TUI's RoundtripGuard enforces at runtime,
/// checked after every keystroke of a random session.
fn assert_roundtrip(ed: &Editor, history: &[String]) {
    let row = normalize(&ed.root);
    if row.is_empty() {
        return;
    }
    let aa = render_row(&row, None, false, &RenderCtx::canonical()).to_text();
    let expected = normalize(&strip_spacers(&row));
    let parsed = parse(&aa).unwrap_or_else(|e| {
        panic!(
            "parse failed: {}\n--- AA ---\n{}\n--- keys ---\n{}",
            e,
            aa,
            history.join(" ")
        )
    });
    assert_eq!(
        parsed,
        expected,
        "AST mismatch\n--- AA ---\n{}\n--- keys ---\n{}",
        aa,
        history.join(" ")
    );
}

#[test]
fn property_random_key_sequences_roundtrip() {
    let n: usize = std::env::var("MASCII_UI_PROP_N")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(200);
    let seed: u64 = std::env::var("MASCII_UI_PROP_SEED")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0xDEC0DE);
    let mut rng = Rng(seed);

    let chars: Vec<char> = "abxyn12+=-*/.,<>|'~αβ∑∫()^_{}[]\\\" ".chars().collect();
    let named_pool = [
        Key::Left,
        Key::Right,
        Key::Up,
        Key::Down,
        Key::Home,
        Key::End,
        Key::Tab,
        Key::Enter,
        Key::Backspace,
        Key::Delete,
        Key::Esc,
    ];

    for _ in 0..n {
        let mut ed = Editor::new();
        let mut history: Vec<String> = Vec::new();
        for _ in 0..60 {
            let r = rng.next() % 100;
            let (key, shift, ctrl) = if r < 55 {
                (Key::Char(*rng.pick(&chars)), false, false)
            } else if r < 75 {
                (*rng.pick(&named_pool), false, false)
            } else if r < 85 {
                // Selections.
                (*rng.pick(&[Key::Left, Key::Right]), true, false)
            } else if r < 95 {
                // Ctrl toggles/jump (host effects are inert here).
                (
                    Key::Char(*rng.pick(&['g', 't', 'b', 'o', 'y', 's'])),
                    false,
                    true,
                )
            } else if r < 98 {
                (Key::Char(*rng.pick(&chars)), true, false)
            } else {
                // Occasional mouse click (not a key; roundtrip-checked too).
                let (x, y) = ((rng.next() % 24) as usize, (rng.next() % 8) as usize);
                history.push(format!("Click({},{})", x, y));
                ed.click(x, y);
                assert_roundtrip(&ed, &history);
                continue;
            };
            history.push(format!(
                "{}{}{:?}",
                if ctrl { "C-" } else { "" },
                if shift { "S-" } else { "" },
                key
            ));
            let _ = ed.input(key, shift, ctrl);
            assert_roundtrip(&ed, &history);
        }
    }
}
