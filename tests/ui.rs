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
use mascii::render::{RenderCtx, render_root};

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
    render_root(&normalize(&ed.root), None, &RenderCtx::canonical()).to_text()
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
fn wrap_keys_wrap_the_selection() {
    // ^ and ( spell the same wrap-Edits the \commands resolve to: the
    // selection lands in the first slot, the cursor steps past.
    let mut ed = Editor::new();
    type_script(&mut ed, "y x S-Left ^");
    assert_eq!(latex(&ed), "y^{x}");
    let mut ed = Editor::new();
    type_script(&mut ed, "a + b S-Left S-Left S-Left (");
    assert_eq!(latex(&ed), "\\left(a+b\\right)");
}

#[test]
fn every_radical_wraps_the_selection() {
    // \qdrt used to insert instead of wrap — the roots are uniform now.
    for (cmd, want) in [
        ("sqrt", "\\sqrt{a+b}"),
        ("cbrt", "\\sqrt[3]{a+b}"),
        ("qdrt", "\\sqrt[4]{a+b}"),
    ] {
        let mut ed = Editor::new();
        type_script(&mut ed, r"a + b S-Left S-Left S-Left");
        ed.execute(cmd);
        assert_eq!(latex(&ed), want, "\\{}", cmd);
    }
}

#[test]
fn box_caret_moves_and_edge_commits() {
    // ←/→ edit inside the box: type "ac", step left, insert "b".
    let mut ed = Editor::new();
    type_script(&mut ed, r"\rm a c Left b Enter");
    assert_eq!(latex(&ed), "\\operatorname{abc}");
    // Stepping past the right edge commits and the arrow then acts on
    // the formula (here: nothing left to move over).
    let mut ed = Editor::new();
    type_script(&mut ed, r"\rm a b Right");
    assert!(ed.op_entry.is_none(), "right edge committed the box");
    assert_eq!(latex(&ed), "\\operatorname{ab}");
    // …and past the left edge likewise, with the cursor stepping left
    // of the freshly committed run.
    let mut ed = Editor::new();
    type_script(&mut ed, r"\rm a b Left Left Left");
    assert!(ed.op_entry.is_none(), "left edge committed the box");
    assert_eq!(latex(&ed), "\\operatorname{ab}");
    assert_eq!(ed.col, 0, "the exiting ← still moved the cursor");
    // Delete works at the caret; Home/End jump.
    let mut ed = Editor::new();
    type_script(&mut ed, r"\rm a b c Home Delete End d Enter");
    assert_eq!(latex(&ed), "\\operatorname{bcd}");
}

#[test]
fn tex_box_reads_latex_in_place() {
    // \tex opens a box; typing (or pasting) LaTeX and committing
    // splices the parsed nodes at the cursor.
    let mut ed = Editor::new();
    ed.execute("tex");
    assert!(ed.op_entry.is_some());
    for c in r"\frac{1}{2}+\alpha".chars() {
        ed.op_type(c);
    }
    ed.op_commit();
    assert_eq!(latex(&ed), "\\frac{1}{2}+\\alpha ");
    // The cursor sits after the spliced nodes; typing continues.
    ed.input(Key::Char('x'), false, false);
    assert_eq!(latex(&ed), "\\frac{1}{2}+\\alpha x");
    // \latex is the same box; junk input inserts nothing and the
    // editor stays consistent.
    let mut ed = Editor::new();
    ed.execute("latex");
    for c in r"\begin{".chars() {
        ed.op_type(c);
    }
    ed.op_commit();
    assert_eq!(ed.root, vec![]);
    // Keys reach the box through the shared keymap too: space is
    // content, Enter commits.
    let mut ed = Editor::new();
    type_script(&mut ed, r"\ t e x Space");
    assert!(ed.op_entry.is_some(), "{:?}", ed.op_entry);
    for c in r"x ^ 2".chars() {
        ed.input(Key::Char(c), false, false);
    }
    ed.input(Key::Enter, false, false);
    assert_eq!(latex(&ed), "x^{2}");
}

#[test]
fn minibuffer_previews_the_commit() {
    // Symbol commands preview their character…
    let mut ed = Editor::new();
    type_script(&mut ed, r"\ a l p h a");
    assert_eq!(ed.command_preview(), Some('α'));
    assert_eq!(
        ed.command_preview_row(),
        Some(vec![mascii::ast::Node::Sym('α')])
    );
    // …∑-class commands their operator…
    let mut ed = Editor::new();
    type_script(&mut ed, r"\ s u m");
    assert_eq!(ed.command_preview(), Some('∑'));
    // …structures the empty template (single-char preview: none).
    let mut ed = Editor::new();
    type_script(&mut ed, r"\ f r a c");
    assert_eq!(ed.command_preview(), None);
    assert!(matches!(
        ed.command_preview_row().as_deref(),
        Some([mascii::ast::Node::Frac { .. }])
    ));
    // Unknown or empty spellings preview nothing; mode openers too.
    let mut ed = Editor::new();
    type_script(&mut ed, r"\ f r");
    assert_eq!(ed.command_preview_row(), None);
    let mut ed = Editor::new();
    type_script(&mut ed, r"\ t e x");
    assert_eq!(ed.command_preview_row(), None);
    // The preview never touches the formula.
    let mut ed = Editor::new();
    type_script(&mut ed, r"x \ f r a c");
    assert_eq!(latex(&ed), "x");
}

#[test]
fn ctrl_keys_return_host_effects() {
    let mut ed = Editor::new();
    assert_eq!(ed.input(Key::Char('q'), false, true), Effect::Quit);
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
fn ctrl_e_jumps_to_the_end_outside_grids() {
    let mut ed = Editor::new();
    type_script(&mut ed, r"a+b C-a");
    assert_eq!((ed.path.len(), ed.col), (0, 0));
    type_script(&mut ed, "C-e");
    assert_eq!((ed.path.len(), ed.col), (0, 3), "formula end");
    // ^E is the end jump even inside a grid cell (grid mode is ^O).
    let mut ed = Editor::new();
    type_script(&mut ed, r"\bmatrix x C-o");
    assert!(ed.grid.is_some());
}

#[test]
fn free_mode_marker_flow() {
    // ^F starts plain free motion (no markers). ^G inside brings up the
    // jump markers; a label letter or arrows+Enter jumps there and
    // returns to plain free motion.
    let mut ed = Editor::new();
    type_script(&mut ed, r"a+ \frac b Down c Tab +z C-f");
    assert!(
        ed.free.is_some() && ed.jump.is_none(),
        "plain free at first"
    );
    type_script(&mut ed, "C-g");
    assert!(ed.jump.is_some(), "markers on demand");
    type_script(&mut ed, "a"); // best-ranked label
    assert!(
        ed.free.is_some() && ed.jump.is_none(),
        "label jump lands back in plain free motion"
    );
    // Arrow-select path: markers on, arrows pick, Enter jumps.
    type_script(&mut ed, "C-g Left Right Enter");
    assert!(ed.free.is_some() && ed.jump.is_none());
    // Esc drops the markers without leaving free mode; a second ^G
    // toggles them off too.
    type_script(&mut ed, "C-g Esc");
    assert!(ed.free.is_some() && ed.jump.is_none());
    type_script(&mut ed, "Enter");
    assert!(ed.free.is_none(), "Enter snaps out");
    // The formula was never modified by any of this.
    assert_eq!(latex(&ed), "a+\\frac{b}{c}+z");
}

#[test]
fn accent_wraps_selection_as_wide_accent() {
    let mut ed = Editor::new();
    type_script(&mut ed, r"abc S-Left S-Left S-Left \hat");
    assert_eq!(latex(&ed), "\\widehat{abc}");
    // Stacking the other side: no selection needed, the accent command
    // right after the node fills its free slot.
    type_script(&mut ed, r"\underline");
    assert_eq!(latex(&ed), "\\widehat{\\underline{abc}}");
    // A one-char selection is the ordinary compact accent.
    let mut ed = Editor::new();
    type_script(&mut ed, r"x S-Left \vec");
    assert_eq!(latex(&ed), "\\vec{x}");
    // Tall bases work too (the band rides over the whole block).
    let mut ed = Editor::new();
    type_script(&mut ed, r"\frac 1 Down 2 Tab S-Left \bar");
    assert_eq!(latex(&ed), "\\overline{\\frac{1}{2}}");
    // Two marks on the same side stack, like a compact accent's.
    let mut ed = Editor::new();
    type_script(&mut ed, r"abc S-Left S-Left S-Left \hat");
    type_script(&mut ed, r"\vec");
    assert_eq!(latex(&ed), "\\overrightarrow{\\widehat{abc}}");
    // The under tilde wraps a selection like any under mark.
    let mut ed = Editor::new();
    type_script(&mut ed, r"AB S-Left S-Left \utilde");
    assert_eq!(latex(&ed), "\\utilde{AB}");
    let mut ed = Editor::new();
    type_script(&mut ed, r"x S-Left \utilde");
    assert_eq!(latex(&ed), "\\utilde{x}");
}

#[test]
fn caret_underscore_commands_insert_scripts() {
    // \^z and \_i make real Sup/Sub nodes (not modifier-letter atoms).
    let mut ed = Editor::new();
    type_script(&mut ed, r"x \^z");
    assert_eq!(latex(&ed), "x^{z}");
    let mut ed = Editor::new();
    type_script(&mut ed, r"x \_10 \^gamma");
    assert_eq!(latex(&ed), "x_{10}^{\\gamma }");
    // The marker may lead, trail, or both: \^z = \z^ = \^z^.
    for spelling in [r"x \z^", r"x \^z^"] {
        let mut ed = Editor::new();
        type_script(&mut ed, spelling);
        assert_eq!(latex(&ed), "x^{z}", "{}", spelling);
    }
    let mut ed = Editor::new();
    type_script(&mut ed, r"x \i_");
    assert_eq!(latex(&ed), "x_{i}");
}

#[test]
fn command_known_tracks_execute() {
    let ed = Editor::new();
    for ok in [
        "frac", "sqrt", "alpha", "sin", "lim", "argmax", "hat", "^z", "rmdx", "pmatrix",
    ] {
        assert!(ed.command_known(ok), "\\{} should be known", ok);
    }
    for bad in ["", "fra", "nosuchthing", "zzz"] {
        assert!(!ed.command_known(bad), "\\{} should be unknown", bad);
    }
    // The probe never touches the real editor.
    let mut ed = Editor::new();
    type_script(&mut ed, "x");
    assert!(ed.command_known("frac"));
    assert_eq!(latex(&ed), "x");
}

#[test]
fn aliases_resolve_to_the_same_command() {
    // Every alias must do exactly what its target does. Command
    // aliases are extra patterns on their `resolve` arm (this list
    // names each of them once); symbol aliases are the `NAMES`
    // spellings that are not their char's canonical one.
    let command_aliases = [
        ("sqrt3", "cbrt"),
        ("sqrt4", "qdrt"),
        ("Vert", "norm"),
        ("xrightarrow", "xto"),
        ("xleftarrow", "xfrom"),
        ("xRightarrow", "xTo"),
        ("xLeftarrow", "xFrom"),
        ("operatorname", "op"),
        ("operatorname*", "op*"),
        ("limits", "op*"),
        ("delim", "lr"),
    ];
    let symbol_aliases = mascii::symbols::NAMES.entries().filter_map(|(&from, &ch)| {
        // A spelling an earlier resolve stage claims (\ch is the
        // hyperbolic function, not χ) only acts as this char in \^ch
        // positions; a styled shortcut (\RR) has no canonical command
        // spelling. Neither has a command to compare against here.
        if mascii::symbols::is_func_name(from) {
            return None;
        }
        let to = mascii::symbols::latex_name(ch)?;
        (from != to).then_some((from, to))
    });
    for (from, to) in command_aliases.into_iter().chain(symbol_aliases) {
        // The box commands open a mode rather than insert; compare the
        // mode instead of the formula.
        let (mut a, mut b) = (Editor::new(), Editor::new());
        a.execute(from);
        b.execute(to);
        assert_eq!(
            (normalize(&a.root), a.op_entry.as_ref().map(|e| e.0)),
            (normalize(&b.root), b.op_entry.as_ref().map(|e| e.0)),
            "\\{} should be \\{}",
            from,
            to
        );
        assert!(
            a.message.is_empty() || !a.message.starts_with("unknown"),
            "\\{}",
            from
        );
    }
}

#[test]
fn typing_inside_a_norm_works() {
    // Regression: Norm's field accessors were missing their Seg(0)
    // arms, so entering the node and typing panicked.
    let mut ed = Editor::new();
    ed.execute("norm");
    ed.input(Key::Char('x'), false, false);
    assert_eq!(latex(&ed), "\\left\\|x\\right\\|");
}

#[test]
fn cancel_wraps_the_selection() {
    // Shift-selection then \cancel.
    let mut ed = Editor::new();
    type_script(&mut ed, r"x+y S-Left S-Left \cancel");
    assert_eq!(latex(&ed), "x\\cancel{+y}");
    // Block selection (^B from inside, Enter picks the parent) then
    // \cancel.
    let mut ed = Editor::new();
    type_script(&mut ed, r"a+(b+c C-b Enter \cancel");
    assert_eq!(latex(&ed), "a+\\cancel{\\left(b+c\\right)}");
    // Parent selection (Shift+Up) then \cancel.
    let mut ed = Editor::new();
    type_script(&mut ed, r"\frac u Down v Tab S-Up \cancel");
    assert_eq!(latex(&ed), "\\cancel{\\frac{u}{v}}");
}

#[test]
fn enter_on_empty_formula_does_not_crash() {
    // All-empty segments: the ┈ separator still needs a column (fuzz
    // found a zero-width vstack panic here).
    let mut ed = Editor::new();
    type_script(&mut ed, "Enter Enter Up Down a");
    assert_eq!(latex(&ed), "a");
}

#[test]
fn grid_edit_mode() {
    // ^O: cell-unit cursor; Enter leaves the mode to edit that cell.
    let mut ed = Editor::new();
    type_script(
        &mut ed,
        r"\bmatrix a C-o Right Enter b C-o Down Left Enter c C-o Right Enter d",
    );
    assert_eq!(
        latex(&ed),
        "\\begin{bmatrix} a & b \\\\ c & d \\end{bmatrix}"
    );
    // Row lanes: r selects the cursor's row (purple), ⌫ deletes it.
    type_script(&mut ed, "C-o r Backspace");
    assert_eq!(latex(&ed), "\\begin{bmatrix} a & b \\end{bmatrix}");
    // Column lanes from row mode: c switches axis; ⌫ deletes the
    // column; a 1x1 grid inside brackets normalizes to plain content.
    type_script(&mut ed, "c Backspace");
    assert_eq!(latex(&ed), "\\left[a\\right]");
    // Gap insert: | enters column mode, ← steps onto the left gap
    // (green), Enter inserts a column there and lands on it; a
    // cross-axis arrow drops back to cell selection of that lane.
    let mut ed = Editor::new();
    type_script(&mut ed, r"\bmatrix x C-o | Left Enter Up Enter y");
    assert_eq!(
        latex(&ed),
        "\\begin{bmatrix}  & x &  \\\\ y &  &  \\end{bmatrix}"
    );
    // Undo works inside grid mode (row deletion is one step).
    let mut ed = Editor::new();
    type_script(&mut ed, r"\bmatrix a Down b C-o r Backspace C-z");
    assert!(
        latex(&ed).contains("\\\\"),
        "undo restored the row: {}",
        latex(&ed)
    );
    // ^O outside a grid only reports.
    let mut ed = Editor::new();
    type_script(&mut ed, "x C-o d");
    assert_eq!(latex(&ed), "xd");
}

#[test]
fn grid_selection_promotes_and_clears() {
    // Backspace on a full-column CELL selection clears the contents…
    let mut ed = Editor::new();
    type_script(&mut ed, r"\bmatrix a Down b C-o S-Up Backspace");
    assert_eq!(latex(&ed), "\\begin{bmatrix}  &  \\\\  &  \\end{bmatrix}");
    // …while pushing the selection past the edge promotes it to the
    // column itself, where Backspace deletes the column.
    let mut ed = Editor::new();
    type_script(&mut ed, r"\bmatrix a Down b C-o S-Up S-Up");
    assert!(
        matches!(
            ed.grid,
            Some(mascii::editor::GridSel::Lanes {
                cols: true,
                pos: 1,
                ..
            })
        ),
        "{:?}",
        ed.grid
    );
    type_script(&mut ed, "Backspace");
    assert_eq!(latex(&ed), "\\begin{bmatrix}  \\\\  \\end{bmatrix}");
}

#[test]
fn esc_exits_grid_mode_from_lane_mode() {
    let mut ed = Editor::new();
    type_script(&mut ed, r"\bmatrix x C-o c Esc");
    assert!(ed.grid.is_none(), "{:?}", ed.grid);
}

/// A display redraw after any editor state: decorated + extents must
/// never panic (the TUI calls these on every frame).
fn redraw(ed: &Editor) {
    let _ = ed.decorated();
    let _ = ed.marker_extents();
}

#[test]
fn grid_state_survives_undo_redo_and_clicks() {
    // Undo shrinks the grid under a lane cursor parked past the end:
    // used to drain past the cells vec / index out of bounds.
    let mut ed = Editor::new();
    type_script(&mut ed, r"\bmatrix a C-o | Right Right Right Enter C-z");
    redraw(&ed);
    type_script(&mut ed, "Backspace");
    redraw(&ed);
    type_script(&mut ed, "Down Enter");
    redraw(&ed);
    // Undo shrinks the grid under a cell anchor.
    let mut ed = Editor::new();
    type_script(
        &mut ed,
        r"\bmatrix a C-o S-Down C-c Down Right C-v Down Down Right S-Up C-z",
    );
    redraw(&ed);
    type_script(&mut ed, "C-c C-v Backspace");
    redraw(&ed);
    // A click relocates the cursor (possibly outside any grid): the
    // grid state follows or ends, never dangles.
    let mut ed = Editor::new();
    type_script(&mut ed, r"\bmatrix a Tab Tab + x");
    type_script(&mut ed, "C-a"); // back to formula start…
    // …enter the matrix and grid mode with an anchor:
    type_script(&mut ed, r"Right C-o S-Down");
    ed.click(1000, 1000); // far away: lands at the formula edge
    redraw(&ed);
    type_script(&mut ed, "C-c C-v");
    redraw(&ed);
}

#[test]
fn lane_selection_copies_its_cells() {
    // ^C on a purple lane copies the lane's cells (it used to be a
    // silent no-op).
    let mut ed = Editor::new();
    type_script(&mut ed, r"\bmatrix a Down b C-o S-Up S-Up C-c");
    assert!(ed.message.contains("copied"), "{}", ed.message);
    type_script(&mut ed, "Esc Tab Tab C-v");
    assert!(
        latex(&ed).ends_with("\\begin{matrix} a \\\\ b \\end{matrix}"),
        "{}",
        latex(&ed)
    );
}

#[test]
fn cell_clip_pastes_over_cells_even_outside_grid_mode() {
    // With the cursor in a grid cell but ^O off, a cell clipboard
    // still pastes as an overwrite — never a nested matrix.
    let mut ed = Editor::new();
    type_script(&mut ed, r"\bmatrix a Down b C-o S-Up C-c Esc Right C-v");
    assert_eq!(
        latex(&ed),
        "\\begin{bmatrix} a & a \\\\ b & b \\end{bmatrix}"
    );
}

#[test]
fn vmatrix_wraps_a_grid_in_a_norm() {
    let mut ed = Editor::new();
    ed.execute("Vmatrix");
    type_script(&mut ed, "a Right b");
    assert_eq!(latex(&ed), "\\begin{Vmatrix} a & b \\\\  &  \\end{Vmatrix}");
}

#[test]
fn grid_cells_copy_paste() {
    // Copy a 2x1 block, paste at the far corner: overwrite semantics,
    // and the grid grows to fit the overhang.
    let mut ed = Editor::new();
    type_script(&mut ed, r"\bmatrix a Down b C-o S-Up C-c Down Right C-v");
    assert_eq!(
        latex(&ed),
        "\\begin{bmatrix} a &  \\\\ b & a \\\\  & b \\end{bmatrix}"
    );
    // Outside a grid, the same cell clipboard pastes as a bare Array.
    type_script(&mut ed, "Esc Tab Tab C-v");
    assert!(
        latex(&ed).ends_with("\\begin{matrix} a \\\\ b \\end{matrix}"),
        "{}",
        latex(&ed)
    );
}

#[test]
fn rm_and_text_boxes() {
    // \rm opens the box; dots are part of the name (i.i.d.).
    let mut ed = Editor::new();
    type_script(&mut ed, r"\rm i.i.d. Enter x");
    assert_eq!(latex(&ed), "\\operatorname{i.i.d.}x");
    // A dictionary word still falls back to its Func.
    let mut ed = Editor::new();
    type_script(&mut ed, r"\rm sin Enter");
    assert_eq!(latex(&ed), "\\operatorname{sin}");
    // \text takes free content incl. spaces, committed as "…".
    let mut ed = Editor::new();
    type_script(&mut ed, r"\text if Space x Enter");
    assert_eq!(latex(&ed), "\\text{if x}");
    // Esc cancels either box.
    let mut ed = Editor::new();
    type_script(&mut ed, r"\rm foo Esc \text bar Esc");
    assert!(ed.root.is_empty());
}

#[test]
fn undo_redo() {
    let mut ed = Editor::new();
    type_script(&mut ed, r"a+b \frac 1 Down 2 Tab");
    let full = latex(&ed);
    type_script(&mut ed, "C-z");
    assert_eq!(latex(&ed), "a+b\\frac{1}{}");
    type_script(&mut ed, "C-z C-z C-z");
    assert_eq!(latex(&ed), "a+");
    // Redo walks forward again, restoring the cursor with each state.
    type_script(&mut ed, "C-r C-r C-r C-r");
    assert_eq!(latex(&ed), full);
    // A fresh edit clears the redo branch — and lands where the undone
    // edit happened (the cursor is restored with the state, here the
    // empty numerator).
    type_script(&mut ed, "C-z C-z x");
    assert_eq!(latex(&ed), "a+b\\frac{x}{}");
    type_script(&mut ed, "C-r");
    assert_eq!(latex(&ed), "a+b\\frac{x}{}");
    // Cursor-only motion is not an undo step.
    let mut ed = Editor::new();
    type_script(&mut ed, "a b Left Left Right C-z");
    assert_eq!(latex(&ed), "a");
    // Undo restores the cursor of the undone state: typing lands where
    // the removed edit happened.
    type_script(&mut ed, "z");
    assert_eq!(latex(&ed), "az");
}

#[test]
fn op_box_via_keys() {
    // \op* opens the in-place name box; Space separates band pieces,
    // Enter commits into the lower limit.
    let mut ed = Editor::new();
    type_script(&mut ed, r"\op* ess Space sup Enter n Tab");
    assert_eq!(latex(&ed), "\\operatorname*{esssup}_{n}");
    // Arrow keys (anything not part of the name) commit the box too.
    let mut ed = Editor::new();
    type_script(&mut ed, r"\op vol Right +1");
    assert_eq!(latex(&ed), "\\operatorname{vol}+1");
    // Esc cancels.
    let mut ed = Editor::new();
    type_script(&mut ed, r"\op foo Esc");
    assert!(ed.root.is_empty());
    // \limits is an alias for \op*.
    let mut ed = Editor::new();
    type_script(&mut ed, r"\limits vol Enter n Tab");
    assert_eq!(latex(&ed), "\\operatorname*{vol}_{n}");
}

#[test]
fn enter_at_top_level_breaks_the_line() {
    let mut ed = Editor::new();
    type_script(&mut ed, "a+b Enter =c");
    assert_eq!(latex(&ed), "a+b \\\\ =c");
    let pic = aa(&ed);
    assert!(
        pic.lines().nth(1).is_some_and(|l| l.trim_end() == "┈"),
        "separator row:\n{}",
        pic
    );
    // ↑/↓ move between the lines; Backspace at a line start merges.
    type_script(&mut ed, "Up");
    assert!(ed.col < 4, "moved to line 1, col {}", ed.col);
    type_script(&mut ed, "Down Home Backspace");
    assert_eq!(latex(&ed), "a+b=c");
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
    // `"` opens the text box; typing through the closing quote makes
    // a \text run, and \" escapes a literal quote inside.
    let mut ed = Editor::new();
    for k in ['"', 'i', 'f', ' ', 'x', '"'] {
        ed.input(Key::Char(k), false, false);
    }
    assert_eq!(latex(&ed), "\\text{if x}");
    let mut ed = Editor::new();
    for k in ['"', 'a', '\\', '"', 'b', '"'] {
        ed.input(Key::Char(k), false, false);
    }
    assert_eq!(latex(&ed), "\\text{a\"b}");
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
fn vertical_exits_a_grid_at_its_edge() {
    // ↓ on the bottom row leaves the matrix (after it); ↑ on the top
    // row leaves before it.
    let mut ed = Editor::new();
    type_script(&mut ed, r"\pmatrix a Down b Down x");
    assert!(ed.path.is_empty(), "path: {:?}", ed.path);
    assert!(latex(&ed).ends_with('x'), "latex: {}", latex(&ed));
    let mut ed = Editor::new();
    type_script(&mut ed, r"\pmatrix a Up y");
    assert!(ed.path.is_empty());
    assert!(latex(&ed).starts_with('y'), "latex: {}", latex(&ed));
}

#[test]
fn free_cursor_mode_snaps_on_enter() {
    // x + 1/2, cursor at the top-row end; ^F, move down, Enter → the
    // free cursor over the denominator area snaps into the denominator.
    let mut ed = Editor::new();
    type_script(&mut ed, r"x + \frac 1 Down 2 Tab C-f Down Enter");
    assert!(
        matches!(ed.path.last(), Some((_, mascii::ast::Field::FracDen))),
        "path: {:?}",
        ed.path
    );
    // Esc cancels back to the original position.
    let mut ed = Editor::new();
    type_script(&mut ed, r"a+b");
    let before = (ed.path.clone(), ed.col);
    type_script(&mut ed, "C-f Left Left Esc");
    assert_eq!((ed.path.clone(), ed.col), before);
    assert!(ed.free.is_none());
}

#[test]
fn free_cursor_auto_expands_collapsed_elements() {
    // x² + ∑ (both collapsed); walking the free cursor toward them
    // materializes the ∑ slots and expands the ² (with hysteresis:
    // they stay open while the cursor is nearby).
    let mut ed = Editor::new();
    type_script(&mut ed, r"x ^ 2 Tab + \sum Tab C-f");
    assert!(ed.ghost.is_empty());
    type_script(&mut ed, "Left");
    assert!(
        !ed.ghost.is_empty(),
        "approaching the bare ∑ must materialize its slots"
    );
    // Walk further left towards the ²: it expands too, and the ∑
    // ghosts persist within the hysteresis radius.
    for _ in 0..4 {
        type_script(&mut ed, "Left");
    }
    assert!(
        ed.ghost
            .iter()
            .any(|p| matches!(p.last(), Some((_, mascii::ast::Field::SupArg)))),
        "ghosts: {:?}",
        ed.ghost
    );
    // Enter snaps somewhere valid; ghosts survive until real input.
    type_script(&mut ed, "Enter");
    assert!(ed.free.is_none());
    assert!(ed.col <= ed.cur_row().len());
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
    // Cursor inside the fraction's denominator: ^B highlights the
    // fraction (innermost parent); Enter selects it.
    type_script(&mut ed, r"1 // 2 Down 3 C-b");
    // The label marker must actually appear in the decorated view
    // (this display path once silently missed the block branch).
    let (root, cursor) = ed.decorated();
    assert!(cursor.is_some(), "cursor stays threaded during modes");
    assert!(
        root.iter().any(
            |n| matches!(n, mascii::ast::Node::Sym(c) if (0xE000..0xE100).contains(&(*c as u32)))
        ),
        "label marker missing: {:?}",
        root
    );
    type_script(&mut ed, "Enter");
    assert_eq!(ed.selection(), Some((1, 2)));
    type_script(&mut ed, "Backspace");
    assert_eq!(latex(&ed), "1");
    // ^B at the top level has no enclosing block: mode does not start.
    let mut ed = Editor::new();
    type_script(&mut ed, r"x+y C-b");
    assert!(ed.block.is_none());
    // Walking the chain: from a cell of a matrix inside parens, ↑ moves
    // outward Array -> Delim; a second ^B cancels without moving.
    let mut ed = Editor::new();
    type_script(&mut ed, r"\pmatrix x");
    let (path, col) = (ed.path.clone(), ed.col);
    type_script(&mut ed, r"C-b");
    assert_eq!(ed.block.as_ref().map(Vec::len), Some(2), "Array + Delim");
    assert_eq!(ed.block_sel, 0, "innermost parent first");
    type_script(&mut ed, "Up");
    assert_eq!(ed.block_sel, 1);
    type_script(&mut ed, "Down");
    assert_eq!(ed.block_sel, 0);
    type_script(&mut ed, "C-b");
    assert!(ed.block.is_none());
    assert_eq!((ed.path, ed.col), (path, col), "cursor untouched");
    // Enter on the outer ancestor selects the delimiter block.
    let mut ed = Editor::new();
    type_script(&mut ed, r"\pmatrix x C-b Up Enter \cancel");
    assert_eq!(
        latex(&ed),
        "\\cancel{\\begin{pmatrix} x &  \\\\  &  \\end{pmatrix}}"
    );
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
/// checked after every keystroke of a random session — plus "the caret
/// survives every render composition" (a missed propagation would show
/// no cursor at all).
fn assert_roundtrip(ed: &Editor, history: &[String]) {
    let (droot, cursor) = ed.decorated();
    if let Some((p, c)) = cursor {
        let b = render_root(&droot, Some((&p[..], c)), &RenderCtx::canonical());
        assert!(
            b.caret.is_some(),
            "caret lost\n--- keys ---\n{}",
            history.join(" ")
        );
    }
    let row = normalize(&ed.root);
    if row.is_empty() {
        return;
    }
    let aa = render_root(&row, None, &RenderCtx::canonical()).to_text();
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
                    Key::Char(*rng.pick(&[
                        'g', 't', 'b', 'e', 'y', 's', 'z', 'r', 'c', 'x', 'v', 'f', 'a',
                    ])),
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
