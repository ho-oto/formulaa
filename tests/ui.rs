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

use formulaa::ast::{normalize, strip_spacers};
use formulaa::editor::Editor;
use formulaa::input::{Effect, Key};
use formulaa::latex::row_to_latex;
use formulaa::parse::parse;
use formulaa::render::{RenderCtx, render_root};

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
fn rm_box_space_commits() {
    // Space is not \rm content (names are alphanumerics + dots): it
    // commits the box, like in \op.
    let mut ed = Editor::new();
    type_script(&mut ed, r"\rm a Space b");
    assert!(ed.op_entry.is_none());
    assert_eq!(latex(&ed), "\\mathrm{a}b");
    // \text keeps spaces as real content.
    let mut ed = Editor::new();
    ed.execute("text");
    for c in "if x".chars() {
        ed.input(Key::Char(c), false, false);
    }
    ed.input(Key::Enter, false, false);
    assert_eq!(latex(&ed), "\\text{if x}");
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
        Some(vec![formulaa::ast::Node::Sym('α')])
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
        Some([formulaa::ast::Node::Frac { .. }])
    ));
    // An unknown spelling previews nothing…
    let mut ed = Editor::new();
    type_script(&mut ed, r"\ f r");
    assert_eq!(ed.command_preview_row(), None);
    // …a name box previews the slot it opens, since that is what
    // committing gives you…
    let mut ed = Editor::new();
    type_script(&mut ed, r"\ t e x");
    assert_eq!(
        ed.command_preview_row(),
        Some(vec![formulaa::ast::Node::Sym('⬚')])
    );
    // …and the commands that act on their surroundings preview
    // nothing, because what they do depends on where the cursor is.
    let mut ed = Editor::new();
    type_script(&mut ed, r"\ a d d r o w");
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
    type_script(&mut ed, r"\bmatrix22 x C-t");
    assert!(ed.grid.is_some());
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
        "frac",
        "sqrt",
        "alpha",
        "sin",
        "lim",
        "argmax",
        "hat",
        "^z",
        "rmdx",
        "pmatrix22",
        "lr(]",
    ] {
        assert!(ed.command_known(ok), "\\{} should be known", ok);
    }
    // Known means Enter inserts something. A spelling that only prints
    // its usage (`\lr`, `\matrix` — an argument is part of the name)
    // is as unready as a half-typed one.
    for bad in [
        "",
        "fra",
        "nosuchthing",
        "zzz",
        "lr",
        "lr(",
        "matrix",
        "pmatrix",
        "matrix3",
    ] {
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
    let symbol_aliases = formulaa::symbols::NAMES
        .entries()
        .filter_map(|(&from, &ch)| {
            // A spelling an earlier resolve stage claims (\ch is the
            // hyperbolic function, not χ) only acts as this char in \^ch
            // positions; a styled shortcut (\RR) has no canonical command
            // spelling. Neither has a command to compare against here.
            if formulaa::symbols::is_func_name(from) {
                return None;
            }
            let to = formulaa::symbols::latex_name(ch)?;
            // Compound `\not\xxx` spellings are not typeable as one name.
            if to.contains('\\') {
                return None;
            }
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
fn plain_motions_shed_the_anchor() {
    // Home/End are plain motions: keeping the anchor would silently
    // grow the selection to the whole line and the next Backspace
    // would delete it all.
    let mut ed = Editor::new();
    type_script(&mut ed, r"a+b S-Left Home");
    assert_eq!(ed.selection(), None);
    type_script(&mut ed, r"Backspace");
    assert_eq!(latex(&ed), "a+b", "nothing left of the cursor to delete");
    let mut ed = Editor::new();
    type_script(&mut ed, r"a+b Left S-Left End");
    assert_eq!(ed.selection(), None);
    // A dormant anchor inside an inset must not resurrect when
    // Backspace lands beside it: the press selects the inset whole
    // (the announced two-step delete), never the stale inner range.
    let mut ed = Editor::new();
    type_script(&mut ed, r"x ^ abc Left Left S-Right Tab Backspace");
    assert_eq!(ed.selection(), Some((1, 2)), "the sup is selected whole");
    type_script(&mut ed, r"Backspace");
    assert_eq!(latex(&ed), "x", "the second press deletes the sup");
    // Same via the contextual close key.
    let mut ed = Editor::new();
    type_script(&mut ed, r"( abc Left Left S-Right ) Backspace");
    assert_eq!(ed.selection(), Some((0, 1)), "the pair is selected whole");
    type_script(&mut ed, r"Backspace");
    assert_eq!(latex(&ed), "");
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
        r"\bmatrix22 a C-t Right Enter b C-t Down Left Enter c C-t Right Enter d",
    );
    assert_eq!(
        latex(&ed),
        "\\begin{bmatrix} a & b \\\\ c & d \\end{bmatrix}"
    );
    // Row lanes: r selects the cursor's row (purple), ⌫ deletes it.
    type_script(&mut ed, "C-t r Backspace");
    assert_eq!(latex(&ed), "\\begin{bmatrix} a & b \\end{bmatrix}");
    // Column lanes from row mode: c switches axis; ⌫ deletes the
    // column; a 1x1 grid inside brackets normalizes to plain content.
    type_script(&mut ed, "c Backspace");
    assert_eq!(latex(&ed), "\\left[a\\right]");
    // Gap insert: | enters column mode, ← steps onto the left gap
    // (green), Enter inserts a column there and lands on it; a
    // cross-axis arrow drops back to cell selection of that lane.
    let mut ed = Editor::new();
    type_script(&mut ed, r"\bmatrix22 x C-t | Left Enter Up Enter y");
    assert_eq!(
        latex(&ed),
        "\\begin{bmatrix}  & x &  \\\\ y &  &  \\end{bmatrix}"
    );
    // Undo works inside grid mode (row deletion is one step).
    let mut ed = Editor::new();
    type_script(&mut ed, r"\bmatrix22 a Down b C-t r Backspace C-z");
    assert!(
        latex(&ed).contains("\\\\"),
        "undo restored the row: {}",
        latex(&ed)
    );
    // ^O outside a grid only reports.
    let mut ed = Editor::new();
    type_script(&mut ed, "x C-t d");
    assert_eq!(latex(&ed), "xd");
}

#[test]
fn grid_selection_promotes_and_clears() {
    // Backspace on a full-column CELL selection clears the contents…
    let mut ed = Editor::new();
    type_script(&mut ed, r"\bmatrix22 a Down b C-t S-Up Backspace");
    assert_eq!(latex(&ed), "\\begin{bmatrix}  &  \\\\  &  \\end{bmatrix}");
    // …while pushing the selection past the edge promotes it to the
    // column itself, where Backspace deletes the column.
    let mut ed = Editor::new();
    type_script(&mut ed, r"\bmatrix22 a Down b C-t S-Up S-Up");
    assert!(
        matches!(
            ed.grid,
            Some(formulaa::editor::GridSel::Lanes {
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
    type_script(&mut ed, r"\bmatrix22 x C-t c Esc");
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
    type_script(&mut ed, r"\bmatrix22 a C-t | Right Right Right Enter C-z");
    redraw(&ed);
    type_script(&mut ed, "Backspace");
    redraw(&ed);
    type_script(&mut ed, "Down Enter");
    redraw(&ed);
    // Undo shrinks the grid under a cell anchor.
    let mut ed = Editor::new();
    type_script(
        &mut ed,
        r"\bmatrix22 a C-t S-Down C-c Down Right C-v Down Down Right S-Up C-z",
    );
    redraw(&ed);
    type_script(&mut ed, "C-c C-v Backspace");
    redraw(&ed);
    // A click relocates the cursor (possibly outside any grid): the
    // grid state follows or ends, never dangles.
    let mut ed = Editor::new();
    type_script(&mut ed, r"\bmatrix22 a Tab Tab + x");
    type_script(&mut ed, "C-a"); // back to formula start…
    // …enter the matrix and grid mode with an anchor:
    type_script(&mut ed, r"Right C-t S-Down");
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
    type_script(&mut ed, r"\bmatrix22 a Down b C-t S-Up S-Up C-c");
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
    type_script(&mut ed, r"\bmatrix22 a Down b C-t S-Up C-c Esc Right C-v");
    assert_eq!(
        latex(&ed),
        "\\begin{bmatrix} a & a \\\\ b & b \\end{bmatrix}"
    );
}

#[test]
fn vmatrix_wraps_a_grid_in_a_norm() {
    let mut ed = Editor::new();
    ed.execute("Vmatrix22");
    type_script(&mut ed, "a Right b");
    assert_eq!(latex(&ed), "\\begin{Vmatrix} a & b \\\\  &  \\end{Vmatrix}");
}

#[test]
fn grid_cells_copy_paste() {
    // Copy a 2x1 block, paste at the far corner: overwrite semantics,
    // and the grid grows to fit the overhang.
    let mut ed = Editor::new();
    type_script(&mut ed, r"\bmatrix22 a Down b C-t S-Up C-c Down Right C-v");
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
fn enter_inside_an_inset_is_inert() {
    // Enter only breaks lines at the top level; inside a grid cell it
    // does nothing (rows are added in ^T grid mode or with \addrow).
    let mut ed = Editor::new();
    type_script(&mut ed, r"\pmatrix22 a Enter");
    let pic = aa(&ed);
    assert_eq!(pic.matches('┼').count(), 1, "still 2×2:\n{}", pic);
    assert!(
        !ed.root
            .iter()
            .any(|n| matches!(n, formulaa::ast::Node::Break))
    );
    // \addrow still works.
    type_script(&mut ed, r"\addrow");
    assert_eq!(aa(&ed).matches('┼').count(), 2);
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
    type_script(&mut ed, r"\pmatrix22 a Down b Down x");
    assert!(ed.path.is_empty(), "path: {:?}", ed.path);
    assert!(latex(&ed).ends_with('x'), "latex: {}", latex(&ed));
    let mut ed = Editor::new();
    type_script(&mut ed, r"\pmatrix22 a Up y");
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
        matches!(ed.path.last(), Some((_, formulaa::ast::Field::FracDen))),
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
            .any(|p| matches!(p.last(), Some((_, formulaa::ast::Field::SupArg)))),
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
        matches!(ed.path.last(), Some((_, formulaa::ast::Field::FracDen))),
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

/// A command that inserts without consuming the selection must clear
/// it: the insert shifts every index, so a surviving anchor would
/// designate a different range and the next Backspace would eat it.
#[test]
fn a_symbol_replaces_the_selection() {
    // Typing a plain symbol over a selection replaces it (standard
    // editor behavior) — typed directly or through the minibuffer.
    let mut ed = Editor::new();
    type_script(&mut ed, r"a b S-Left S-Left");
    assert_eq!(ed.selection(), Some((0, 2)));
    type_script(&mut ed, r"\alpha");
    assert_eq!(ed.selection(), None, "the selection was consumed");
    assert_eq!(latex(&ed), "\\alpha ");
    let mut ed = Editor::new();
    type_script(&mut ed, r"abc S-Left S-Left x");
    assert_eq!(latex(&ed), "ax");
    // The / atom replaces too (its // fraction shortcut is untouched).
    let mut ed = Editor::new();
    type_script(&mut ed, r"ab S-Left /");
    assert_eq!(latex(&ed), "a/");
    // A wrapping command still consumes the selection it was given.
    let mut ed = Editor::new();
    type_script(&mut ed, r"a b S-Left S-Left \hat");
    assert_eq!(latex(&ed), "\\widehat{ab}");
}

/// Every content-inserting edit replaces the selection: spacer, Func,
/// ∑-band, grid, name box, and the Enter line break.
#[test]
fn content_inserts_replace_the_selection() {
    // Space: the range becomes one formatting spacer.
    let mut ed = Editor::new();
    type_script(&mut ed, r"abc S-Left S-Left Space");
    assert_eq!(latex(&ed), "a");
    // A dictionary function name.
    let mut ed = Editor::new();
    type_script(&mut ed, r"ab S-Left \sin");
    assert_eq!(latex(&ed), "a\\operatorname{sin}");
    // A ∑-class band replaces and enters its lower limit.
    let mut ed = Editor::new();
    type_script(&mut ed, r"ab S-Left \sum n Tab");
    assert_eq!(latex(&ed), "a\\sum_{n}");
    // A grid replaces and enters its first cell.
    let mut ed = Editor::new();
    type_script(&mut ed, r"ab S-Left \pmatrix22 x");
    assert_eq!(latex(&ed), "a\\begin{pmatrix} x &  \\\\  &  \\end{pmatrix}");
    // A name box deletes the selection when it opens; the commit
    // lands where the range was.
    let mut ed = Editor::new();
    type_script(&mut ed, r"ab S-Left \rm d Enter");
    assert_eq!(latex(&ed), "a\\mathrm{d}");
    // Enter: the range becomes the line break.
    let mut ed = Editor::new();
    type_script(&mut ed, r"ab S-Left Enter c");
    assert_eq!(latex(&ed), "a \\\\ c");
}

/// \mid is contextual: the divides atom ∣ in a plain row, the segment
/// separator directly inside a delimiter block.
#[test]
fn block_select_mode_selects_a_structure() {
    let mut ed = Editor::new();
    // Cursor inside the fraction's denominator: ^B highlights the
    // fraction (innermost parent); Enter selects it.
    type_script(&mut ed, r"1 // 2 Down 3 C-b");
    // The block marks must appear in the decorated view.
    let (root, cursor) = ed.decorated();
    assert!(cursor.is_some(), "cursor stays threaded during modes");
    assert!(
        root.iter().any(
            |n| matches!(n, formulaa::ast::Node::Sym(c) if (0xE000..0xE0F0).contains(&(*c as u32)))
        ),
        "block mark missing: {:?}",
        root
    );
    type_script(&mut ed, "Enter");
    assert_eq!(ed.selection(), Some((1, 2)));
    // The cursor leaves on the right end; Shift+← must still grow the
    // selection leftward (the ends flip instead of collapsing).
    type_script(&mut ed, "S-Left");
    assert_eq!(ed.selection(), Some((0, 2)));
    type_script(&mut ed, "S-Right");
    assert_eq!(ed.selection(), Some((1, 2)), "shrinks back");
    type_script(&mut ed, "Backspace");
    assert_eq!(latex(&ed), "1");
    // Shift+←/→ inside the mode: select the highlighted block and move
    // straight into the linear selection.
    let mut ed = Editor::new();
    type_script(&mut ed, r"1 // 2 Down 3 C-b S-Left");
    assert!(ed.block.is_none(), "mode exits");
    assert_eq!(ed.selection(), Some((0, 2)));
    // Arrow walk: outward Array -> Delim; a second ^B cancels without
    // moving the cursor.
    let mut ed = Editor::new();
    type_script(&mut ed, r"\pmatrix22 x");
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
    // Enter on the outer ancestor selects the delimiter block, ready
    // for wrapping.
    let mut ed = Editor::new();
    type_script(&mut ed, r"\pmatrix22 x C-b Up Enter \sqrt");
    assert_eq!(
        latex(&ed),
        "\\sqrt{\\begin{pmatrix} x &  \\\\  &  \\end{pmatrix}}"
    );
    // A letter key is no longer a label: it cancels the mode.
    let mut ed = Editor::new();
    type_script(&mut ed, r"1 // 2 Down 3 C-b a");
    assert!(ed.block.is_none());
}

/// Shift+↑ places a whole selection with the cursor on an end the
/// user did not pick: the first Shift+←/→ flips to grow on that side,
/// then plain shrink semantics resume.
#[test]
fn whole_selection_flip_then_plain_semantics() {
    let mut ed = Editor::new();
    // Cursor in the denominator: Shift+↑ selects the whole fraction.
    type_script(&mut ed, r"1 // 2 Down 3 S-Up S-Left");
    assert_eq!(ed.selection(), Some((0, 2)));
    type_script(&mut ed, "S-Right");
    assert_eq!(ed.selection(), Some((1, 2)), "shrinks back");
    // A hand-made selection keeps the plain semantics: Shift+→ back
    // onto the anchor clears it.
    let mut ed = Editor::new();
    type_script(&mut ed, r"abcde Left Left S-Left");
    assert_eq!(ed.selection(), Some((2, 3)));
    type_script(&mut ed, "S-Right");
    assert_eq!(ed.selection(), None, "shrink collapses");
}

#[test]
fn mid_is_divides_outside_a_delimiter() {
    let mut ed = Editor::new();
    type_script(&mut ed, r"a \mid b");
    assert_eq!(latex(&ed), "a\\mid b");
    let mut ed = Editor::new();
    type_script(&mut ed, r"a \!mid b");
    assert_eq!(latex(&ed), "a\\nmid b");
    // Inside a paren it still splits the segment.
    let mut ed = Editor::new();
    type_script(&mut ed, r"( x \mid P");
    assert_eq!(latex(&ed), "\\left(x\\middle|P\\right)");
}

/// `\!` right after a symbol toggles it with its slashed negation.
#[test]
fn bang_toggles_the_preceding_symbol() {
    let mut ed = Editor::new();
    type_script(&mut ed, r"a = \! b");
    assert_eq!(latex(&ed), "a\\ne b");
    // The toggle closes: ∈ → ∉ → ∈ → ∉.
    let mut ed = Editor::new();
    type_script(&mut ed, r"x \in \!");
    assert_eq!(latex(&ed), "x\\notin ");
    type_script(&mut ed, r"\!");
    assert_eq!(latex(&ed), "x\\in ");
    type_script(&mut ed, r"\!");
    assert_eq!(latex(&ed), "x\\notin ");
    // A directly-typed slashed atom un-negates the same way.
    let mut ed = Editor::new();
    type_script(&mut ed, r"a \ne \! b");
    assert_eq!(latex(&ed), "a=b");
    // No negation for a letter; nothing left of the cursor at col 0.
    let mut ed = Editor::new();
    type_script(&mut ed, r"a \!");
    assert!(ed.message_error);
    let mut ed = Editor::new();
    type_script(&mut ed, r"\!");
    assert!(ed.message_error);
}

/// `!`-prefixed (and `!`-suffixed) spellings are the slashed
/// relations: `\!=` and `\=!` are ≠, `\!in` is ∉.
#[test]
fn bang_spellings_negate_relations() {
    let mut ed = Editor::new();
    type_script(&mut ed, r"a \!= b");
    assert_eq!(latex(&ed), "a\\ne b");
    let mut ed = Editor::new();
    type_script(&mut ed, r"a \=! b");
    assert_eq!(latex(&ed), "a\\ne b");
    let mut ed = Editor::new();
    type_script(&mut ed, r"a \!in B");
    assert_eq!(latex(&ed), "a\\notin B");
    let mut ed = Editor::new();
    type_script(&mut ed, r"a \!le b");
    assert_eq!(latex(&ed), "a\\nleq b");
    let mut ed = Editor::new();
    type_script(&mut ed, r"a \subset! b");
    assert_eq!(latex(&ed), "a\\not\\subset b");
}

/// \rm of a digit has no upright/italic distinction to preserve: it
/// canonicalizes to the plain atom, so gluing it to a letter cannot
/// break the roundtrip (Roman('1') next to an alpha once did).
#[test]
fn rm_of_a_digit_is_just_the_digit() {
    let mut ed = Editor::new();
    type_script(&mut ed, r"\rm1 Tab x");
    assert_roundtrip(&ed, &[]);
    assert_eq!(latex(&ed), "1x");
}

/// A formula line break lives only at the top level: a selection may
/// not span one, so neither a wrap nor a copy can carry one into an
/// inset (the picture would lose it, breaking the roundtrip).
#[test]
fn a_line_break_stays_out_of_insets() {
    let mut ed = Editor::new();
    type_script(&mut ed, r"a Enter b S-Left S-Left S-Left");
    assert_eq!(ed.selection(), Some((2, 3)), "selection stops at the break");
    type_script(&mut ed, "^");
    assert_eq!(latex(&ed), "a \\\\ ^{b}");
    assert_roundtrip(&ed, &[]);
    // Shift+↑ selects a whole top-level row, breaks included — an
    // accent's base is an inset, so the break must not ride in.
    let mut ed = Editor::new();
    type_script(&mut ed, r"a Enter b S-Up \vec");
    assert_roundtrip(&ed, &[]);
    assert!(
        !latex(&ed).contains("\\vec{a \\\\ b}"),
        "no break in the base: {}",
        latex(&ed)
    );
    // \mid only splits a real delimiter; a norm numbers its field the
    // same way but is not a pair, so inside one it is the divides
    // atom (like any plain row).
    let mut ed = Editor::new();
    type_script(&mut ed, r"\norm \mid");
    assert_eq!(latex(&ed), "\\left\\|\\mid \\right\\|");
    // The same guard the other way round: with the cursor right before
    // the break, Shift+→ refuses to cross it.
    let mut ed = Editor::new();
    type_script(&mut ed, r"a Enter b Left Left S-Right");
    assert_eq!(ed.selection(), None, "and stops before it too");
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
    let n: usize = std::env::var("FORMULAA_UI_PROP_N")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(200);
    let seed: u64 = std::env::var("FORMULAA_UI_PROP_SEED")
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
                // Ctrl toggles (host effects are inert here).
                (
                    Key::Char(
                        *rng.pick(&['t', 'b', 'e', 'y', 's', 'z', 'r', 'c', 'x', 'v', 'f', 'a']),
                    ),
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

/// Backspace just inside a delimiter unwraps it in two steps, so the
/// bracket can be shed without losing what it holds: the first press
/// arms the pair (the display lights it up), the second lifts the
/// contents out and selects them, a third deletes those — and an arrow
/// key after the second leaves the unwrap standing.
#[test]
fn backspace_unwraps_a_delimiter() {
    // ( f o o ) with the cursor just after the opening bracket.
    let open = |ed: &mut Editor| {
        type_script(ed, "( foo Home");
    };
    let mut ed = Editor::new();
    open(&mut ed);
    assert_eq!(latex(&ed), "\\left(foo\\right)");

    // First press arms: nothing is deleted yet.
    type_script(&mut ed, "Backspace");
    assert_eq!(latex(&ed), "\\left(foo\\right)", "the first press deletes");
    // Second press unwraps and selects what came out.
    type_script(&mut ed, "Backspace");
    assert_eq!(latex(&ed), "foo");
    assert_eq!(ed.selection(), Some((0, 3)), "the contents are selected");
    // Third press deletes the selection.
    type_script(&mut ed, "Backspace");
    assert_eq!(latex(&ed), "");

    // …and an arrow key instead just walks away from the unwrap.
    let mut ed = Editor::new();
    open(&mut ed);
    type_script(&mut ed, "Backspace Backspace Right");
    assert_eq!(latex(&ed), "foo");
    assert_eq!(ed.selection(), None);

    // The arming is one-shot: a key in between disarms it, so the next
    // Backspace starts over rather than unwrapping by surprise.
    let mut ed = Editor::new();
    open(&mut ed);
    type_script(&mut ed, "Backspace Right Left Backspace");
    assert_eq!(latex(&ed), "\\left(foo\\right)");
}

/// ^D deletes forward, and against the closing bracket it unwraps the
/// same way Backspace does against the opening one.
#[test]
fn ctrl_d_deletes_forward_and_unwraps() {
    let mut ed = Editor::new();
    type_script(&mut ed, "abc Home C-d");
    assert_eq!(latex(&ed), "bc", "^D deletes the character ahead");

    let mut ed = Editor::new();
    type_script(&mut ed, "( foo");
    // The cursor sits at the end of the contents, against the ')'.
    type_script(&mut ed, "C-d");
    assert_eq!(latex(&ed), "\\left(foo\\right)", "the first press deletes");
    type_script(&mut ed, "C-d");
    assert_eq!(latex(&ed), "foo");
    assert_eq!(ed.selection(), Some((0, 3)));
}

/// Shift-selecting *onto* a bracket arms the pair — the selection
/// asked for "just the bracket", and a bracket's meaning is its pair —
/// so the next Backspace/Delete unwraps. A second shift step selects
/// the node whole, and extending an existing selection swallows it in
/// one step, as before.
#[test]
fn shift_selecting_a_bracket_arms_the_pair() {
    // From the left: arm (nothing selected, nothing deleted), unwrap.
    // The gesture named the bracket, so the unwrap selects nothing —
    // the contents just stay, with the cursor keeping its side.
    let mut ed = Editor::new();
    type_script(&mut ed, "( foo ) Home S-Right");
    assert_eq!(latex(&ed), "\\left(foo\\right)");
    assert_eq!(ed.selection(), None, "arming made a selection");
    type_script(&mut ed, "Backspace");
    assert_eq!(latex(&ed), "foo");
    assert_eq!(ed.selection(), None, "the shift unwrap selected something");
    assert_eq!(ed.col, 0, "the cursor left its side");

    // From the right, with Delete.
    let mut ed = Editor::new();
    type_script(&mut ed, "( foo ) S-Left Delete");
    assert_eq!(latex(&ed), "foo");
    assert_eq!((ed.selection(), ed.col), (None, 3));

    // While armed, the display lights the pair up.
    let mut ed = Editor::new();
    type_script(&mut ed, "( foo ) Home S-Right");
    let (root, _) = ed.decorated();
    let has_mark = root.iter().any(|n| {
        matches!(n, formulaa::ast::Node::Sym(c)
            if formulaa::glyphs::Mark::decode(*c) == Some(formulaa::glyphs::Mark::Delims { open: true }))
    });
    assert!(has_mark, "no armed marks in {:?}", root);

    // The second step takes the node whole, as before.
    let mut ed = Editor::new();
    type_script(&mut ed, "( foo ) Home S-Right S-Right");
    assert_eq!(ed.selection(), Some((0, 1)));
    type_script(&mut ed, "Backspace");
    assert_eq!(latex(&ed), "");

    // Extending an existing selection swallows the pair in one step.
    let mut ed = Editor::new();
    type_script(&mut ed, "x ( foo ) Home S-Right S-Right Backspace");
    assert_eq!(latex(&ed), "");

    // A pair with middles has no single contents: the first shift
    // step selects it whole rather than arming.
    let mut ed = Editor::new();
    type_script(&mut ed, r"\lr(|) a ) Home S-Right");
    assert_eq!(ed.selection(), Some((0, 1)));

    // From inside, the row's edge is the bracket too: Shift there
    // arms the same way, and either delete key then unwraps (the
    // staged flow, contents selected).
    let mut ed = Editor::new();
    type_script(&mut ed, "( foo Home S-Left Backspace");
    assert_eq!(latex(&ed), "foo");
    assert_eq!(ed.selection(), Some((0, 3)));
    let mut ed = Editor::new();
    type_script(&mut ed, "( foo S-Right Backspace");
    assert_eq!(latex(&ed), "foo");
}

/// A radical unwraps like a pair: Backspace at the start of its
/// argument — the side its root glyph is on — arms it, and
/// shift-selecting the root does the same from outside. The far end
/// has nothing to delete toward, so ^D there keeps its old no-op.
#[test]
fn a_radical_unwraps_like_a_pair() {
    let mut ed = Editor::new();
    type_script(&mut ed, r"\sqrt foo Home Backspace");
    assert_eq!(latex(&ed), "\\sqrt{foo}", "the first press deletes");
    type_script(&mut ed, "Backspace");
    assert_eq!(latex(&ed), "foo");
    assert_eq!(ed.selection(), Some((0, 3)));

    // From outside, selecting the root arms the radical.
    let mut ed = Editor::new();
    type_script(&mut ed, r"\sqrt foo Right S-Left Backspace");
    assert_eq!(latex(&ed), "foo");

    // The argument's end arms nothing — there is nothing ahead — for
    // ^D and Shift+→ alike (the Backspace then deletes a char).
    let mut ed = Editor::new();
    type_script(&mut ed, r"\sqrt foo C-d C-d");
    assert_eq!(latex(&ed), "\\sqrt{foo}");
    let mut ed = Editor::new();
    type_script(&mut ed, r"\sqrt foo S-Right Backspace");
    assert_eq!(latex(&ed), "\\sqrt{fo}");
}

/// `\norm` unwraps like the pair it is: from either edge inside, and
/// from a shift-selection outside.
#[test]
fn norm_unwraps_like_a_delimiter() {
    let mut ed = Editor::new();
    type_script(&mut ed, r"\norm x Home Backspace Backspace");
    assert_eq!(latex(&ed), "x");

    let mut ed = Editor::new();
    type_script(&mut ed, r"\norm x C-d C-d");
    assert_eq!(latex(&ed), "x");

    let mut ed = Editor::new();
    type_script(&mut ed, r"\norm x Right S-Left Backspace");
    assert_eq!(latex(&ed), "x");
}

/// A pair with │ middles, and a fused matrix, keep the old behaviour:
/// there is no single "contents" to lift out of either.
#[test]
fn unwrap_leaves_middles_and_grids_alone() {
    // Both halves must actually reach the guard: the cursor has to sit
    // at the start of the delimiter's own segment, or the presses never
    // consult `unwrappable` and the assertions pin nothing. The
    // outcome that separates guarded from armed is the SECOND press —
    // an armed pair unwraps on it.
    let mut ed = Editor::new();
    type_script(&mut ed, r"\set x Home");
    assert_eq!(ed.col, 0, "cursor is against the opening brace");
    assert!(
        matches!(ed.path.last(), Some((_, formulaa::ast::Field::Seg(0)))),
        "cursor is in the delimiter's segment: {:?}",
        ed.path
    );
    type_script(&mut ed, "Backspace Backspace");
    assert!(
        latex(&ed).contains("\\middle|"),
        "the set-builder bar survived: {}",
        latex(&ed)
    );

    // A fused grid: the cursor starts in the first cell, so step out
    // to the segment before pressing — otherwise Backspace never
    // reaches the delimiter at all.
    let mut ed = Editor::new();
    type_script(&mut ed, r"\pmatrix22 x Tab Home");
    assert!(
        matches!(ed.path.last(), Some((_, formulaa::ast::Field::Seg(0)))),
        "cursor is in the delimiter's segment: {:?}",
        ed.path
    );
    type_script(&mut ed, "Backspace Backspace");
    assert!(
        latex(&ed).contains("pmatrix"),
        "the matrix survived: {}",
        latex(&ed)
    );

    // …and the empty pair still goes in one press, as it always did.
    let mut ed = Editor::new();
    type_script(&mut ed, "x ( Backspace");
    assert_eq!(latex(&ed), "x", "an empty pair needs one Backspace");
}

/// Tab opens the completion list, the arrows pick a row and Enter
/// takes it — and the row that lands is the one that was highlighted,
/// not whatever was typed.
#[test]
fn tab_completion_picks_a_row() {
    // \al + Tab + Enter commits the first row: α.
    let mut ed = Editor::new();
    type_script(&mut ed, r"\ a l Down Enter");
    assert_eq!(latex(&ed), "\\alpha ");
    assert!(ed.completion.is_none(), "the popup outlived the pick");
    assert!(ed.minibuffer.is_none());

    // ↓ moves to the next row, and that row is what Enter inserts.
    let mut ed = Editor::new();
    type_script(&mut ed, r"\ a l Down");
    let second = ed.completion.as_ref().unwrap().items[1]
        .commit()
        .unwrap()
        .to_string();
    type_script(&mut ed, "Down Enter");
    let mut expected = Editor::new();
    expected.execute(&second);
    assert_eq!(latex(&ed), latex(&expected), "picked \\{}", second);

    // ↑ from the first row wraps to the last: the list cycles.
    let mut ed = Editor::new();
    type_script(&mut ed, r"\ a l Down Up");
    let list = ed.completion.as_ref().unwrap();
    assert_eq!(list.sel, list.items.len() - 1);
}

/// The popup tracks what is typed, and Esc peels it before the
/// minibuffer so a stray Tab is one keypress to undo.
#[test]
fn tab_completion_follows_the_query() {
    let mut ed = Editor::new();
    type_script(&mut ed, r"\ a l Down");
    assert_eq!(
        ed.completion.as_ref().unwrap().items[0].symbol,
        "α",
        "the α row leads \\al"
    );
    // Typing on narrows the list without closing it.
    type_script(&mut ed, "e p h");
    let list = ed.completion.as_ref().expect("the popup stayed open");
    assert_eq!(list.items[0].commit(), Some("aleph"));
    // Backspace widens it again.
    type_script(&mut ed, "Backspace Backspace Backspace");
    assert_eq!(ed.completion.as_ref().unwrap().items[0].symbol, "α");
    // Esc closes the popup, keeping what was typed.
    type_script(&mut ed, "Esc");
    assert!(ed.completion.is_none());
    assert_eq!(ed.minibuffer.as_deref(), Some("al"));
    // …and a second Esc closes the minibuffer.
    type_script(&mut ed, "Esc");
    assert!(ed.minibuffer.is_none());
}

/// A query nothing matches leaves the popup closed and says so,
/// rather than opening an empty box.
#[test]
fn tab_completion_with_no_matches_says_so() {
    let mut ed = Editor::new();
    type_script(&mut ed, r"\ q q z z Tab");
    assert!(ed.completion.is_none());
    assert!(!ed.message.is_empty(), "no message");
    // The minibuffer is untouched, so the typing can be fixed.
    assert_eq!(ed.minibuffer.as_deref(), Some("qqzz"));
}

/// ^B paints the highlighted ancestor and the one step outward, not
/// the whole chain: the arrows move one step at a time, and the inner
/// step is always where the selection just came from.
#[test]
fn block_select_paints_only_a_step_in_each_direction() {
    use formulaa::glyphs::Mark;
    let mut ed = Editor::new();
    // Nest deeply: a matrix cell inside a fraction inside a bracket.
    type_script(&mut ed, r"( 1 // \pmatrix22 x");
    type_script(&mut ed, "C-b");
    let depth = ed.block.as_ref().map(Vec::len).unwrap_or(0);
    assert!(depth >= 4, "expected a deep chain, got {}", depth);

    let painted = |ed: &Editor| -> Vec<usize> {
        let (root, _) = ed.decorated();
        fn walk(row: &formulaa::ast::Row, out: &mut Vec<usize>) {
            for n in row {
                if let formulaa::ast::Node::Sym(c) = n
                    && let Some(Mark::BlockOpen { rank }) = Mark::decode(*c)
                {
                    out.push(rank);
                }
                for f in n.fields() {
                    walk(n.field(f), out);
                }
            }
        }
        let mut out = Vec::new();
        walk(&root, &mut out);
        out.sort_unstable();
        out
    };

    // Itself and the one step *outward*, never the whole chain and
    // never the inner step — selection starts at the innermost
    // ancestor, so the way back in needs no announcing.
    assert_eq!(painted(&ed), vec![0, 1]);
    type_script(&mut ed, "Up");
    assert_eq!(painted(&ed), vec![1, 2]);
    type_script(&mut ed, "Up");
    assert_eq!(painted(&ed), vec![2, 3]);
    // Outermost: itself alone (nothing further out to step to).
    for _ in 0..depth {
        type_script(&mut ed, "Up");
    }
    assert_eq!(painted(&ed), vec![depth - 1]);
}

/// The pending unwrap is a one-shot that belongs to the tree and the
/// place it was armed in. Anything that walks away from either must
/// disarm it, or the display keeps promising an unwrap that the next
/// Backspace performs on a tree the user never armed.
#[test]
fn arming_does_not_survive_undo_or_a_click() {
    // ^Z returns from `input` before the key layer's one-shot take, so
    // it has to clear the arming itself. The undo has to land back on
    // the armed spot for the staleness to bite: same path, column 0,
    // and a pair that still has contents to lift out.
    let mut ed = Editor::new();
    type_script(&mut ed, "( a Home x Home Backspace C-z Backspace");
    assert!(
        latex(&ed).contains("\\left("),
        "undo left the pair armed, so one Backspace unwrapped it: {}",
        latex(&ed)
    );

    // A mouse click moves the cursor without passing through the key
    // layer at all. The arming must go with it — otherwise the pair
    // stays lit while Backspace does something else entirely.
    let lit = |ed: &Editor| {
        use formulaa::glyphs::Mark;
        fn walk(row: &formulaa::ast::Row, out: &mut bool) {
            for n in row {
                if let formulaa::ast::Node::Sym(c) = n
                    && matches!(Mark::decode(*c), Some(Mark::Delims { .. }))
                {
                    *out = true;
                }
                for f in n.fields() {
                    walk(n.field(f), out);
                }
            }
        }
        let (root, _) = ed.decorated();
        let mut found = false;
        walk(&root, &mut found);
        found
    };
    // The click has to land INSIDE the same segment: a click that
    // leaves the path makes decorate_plain's path test fail on its own,
    // so it would pass whether or not the arming was cleared.
    let mut ed = Editor::new();
    type_script(&mut ed, "( foo Home Backspace");
    let armed_path = ed.path.clone();
    assert!(lit(&ed), "the pair should be lit after arming");
    ed.click(3, 0); // still inside (foo), one cell along
    assert_eq!(ed.path, armed_path, "the click stayed in the segment");
    assert_ne!(ed.col, 0, "…but moved off the armed column");
    assert!(!lit(&ed), "a click left the pair lit up as armed");
}

/// A click also closes the completion popup. It is invisible while the
/// minibuffer is shut, so an orphaned one springs back on the next `\`
/// — with the old query — and Enter commits a row for something the
/// user can no longer see.
#[test]
fn a_click_closes_the_completion_popup() {
    let mut ed = Editor::new();
    type_script(&mut ed, r"\ a l Down");
    assert!(ed.completion.is_some());
    ed.click(0, 0);
    assert!(ed.completion.is_none(), "the popup outlived the click");
    // …so `\` + Enter is an empty command again, not a stale pick.
    type_script(&mut ed, r"\ Enter");
    assert_eq!(latex(&ed), "");
}

/// The popup is a live list, not a snapshot: both commit keys take the
/// highlighted row, a query that matches nothing leaves it open so
/// backspacing brings the list back, and the status line does not keep
/// a notice from a keystroke ago.
#[test]
fn the_popup_tracks_the_query_and_both_commit_keys_take_it() {
    // Space commits the highlighted row, exactly as Enter does.
    let mut ed = Editor::new();
    type_script(&mut ed, r"\ a l Down Down");
    let picked = ed
        .completion
        .as_ref()
        .unwrap()
        .selected()
        .and_then(|i| i.commit())
        .unwrap()
        .to_string();
    type_script(&mut ed, "Space");
    let mut expected = Editor::new();
    expected.execute(&picked);
    assert_eq!(latex(&ed), latex(&expected), "Space took \\{}", picked);

    // Typing past every match keeps the popup open (empty), so
    // backspacing restores the list instead of needing another Tab.
    let mut ed = Editor::new();
    type_script(&mut ed, r"\ a l Down");
    assert!(!ed.completion.as_ref().unwrap().items.is_empty());
    type_script(&mut ed, "q q z");
    assert!(
        ed.completion.as_ref().is_some_and(|l| l.items.is_empty()),
        "the popup closed on a non-matching query"
    );
    type_script(&mut ed, "Backspace Backspace Backspace");
    assert!(
        ed.completion.as_ref().is_some_and(|l| !l.items.is_empty()),
        "backspacing did not bring the list back"
    );

    // An empty list commits the typed text rather than nothing.
    let mut ed = Editor::new();
    type_script(&mut ed, r"\ a l Down q q z Enter");
    assert!(ed.message.contains("unknown command"), "{:?}", ed.message);

    // …and the "no completion" notice does not outlive its keystroke.
    let mut ed = Editor::new();
    type_script(&mut ed, r"\ q q z z Tab");
    assert!(!ed.message.is_empty(), "no notice");
    type_script(&mut ed, "Backspace");
    assert!(ed.message.is_empty(), "stale notice: {:?}", ed.message);
}

/// Tab finishes, the arrows browse. A name that is already a command
/// commits on Tab — that is what "complete" means once there is
/// nothing left to complete — and only a name that is not a command
/// makes Tab ask for the list.
#[test]
fn tab_finishes_and_the_arrows_browse() {
    // \alpha is a command: Tab runs it.
    let mut ed = Editor::new();
    type_script(&mut ed, r"\ a l p h a Tab");
    assert_eq!(latex(&ed), "\\alpha ");
    assert!(ed.minibuffer.is_none() && ed.completion.is_none());

    // \al is a command too (a shorthand for the same α).
    let mut ed = Editor::new();
    type_script(&mut ed, r"\ a l Tab");
    assert_eq!(latex(&ed), "\\alpha ");

    // \alp… is not: Tab asks for the list instead of running anything.
    let mut ed = Editor::new();
    type_script(&mut ed, r"\ x y z z Tab");
    assert_eq!(latex(&ed), "", "Tab executed a non-command");
    assert!(
        ed.completion.is_some() || !ed.message.is_empty(),
        "Tab neither listed nor explained"
    );

    // An arrow opens the list on the first press without skipping its
    // first row, and Tab then takes whatever is highlighted.
    let mut ed = Editor::new();
    type_script(&mut ed, r"\ a l Down");
    let first = ed.completion.as_ref().unwrap();
    assert_eq!(first.sel, 0, "the revealing press skipped a row");
    type_script(&mut ed, "Down");
    let second = ed
        .completion
        .as_ref()
        .unwrap()
        .selected()
        .and_then(|i| i.commit())
        .unwrap()
        .to_string();
    type_script(&mut ed, "Tab");
    let mut expected = Editor::new();
    expected.execute(&second);
    assert_eq!(latex(&ed), latex(&expected), "Tab took \\{}", second);
}

/// A step that would change nothing closes the list instead: `\frak`'s
/// own row leaves `frak` typed and the rest is free input the list
/// cannot enumerate, so a second accept means "let me type". (It used
/// to rebuild the identical list, and Enter looked dead.)
#[test]
fn a_stalled_step_closes_the_list() {
    // The first accept completes the prefix; the second yields.
    // (`\fra`'s first row is `frac`; the frak family sits under it.)
    let mut ed = Editor::new();
    type_script(&mut ed, r"\ f r a Down Down Enter");
    assert_eq!(
        ed.minibuffer.as_deref(),
        Some("frak"),
        "the step stalled early"
    );
    assert!(ed.completion.is_some(), "the list closed with a step taken");
    type_script(&mut ed, "Enter");
    assert_eq!(ed.minibuffer.as_deref(), Some("frak"));
    assert!(
        ed.completion.is_none(),
        "the list stayed up with nothing to add"
    );
    // …and typing carries straight on.
    type_script(&mut ed, "a Enter");
    assert_eq!(latex(&ed), "\\mathfrak{a}");
}

/// A shape row is a step, not an answer: taking it writes the next
/// piece of the spelling and asks for the rest, so a delimiter spec
/// can be built by picking tokens without ever knowing them by heart.
#[test]
fn shape_rows_extend_the_spelling() {
    let mut ed = Editor::new();
    type_script(&mut ed, r"\ l r Down");
    let list = ed.completion.as_ref().expect("the spec tokens are listed");
    // The spec rows lead the list (ordinary matches for the letters
    // `lr` keep a tail below them) and none of them can be taken.
    let spec: Vec<_> = list
        .items
        .iter()
        .filter(|i| i.names.starts_with("lr"))
        .collect();
    assert!(spec.len() >= 8, "{:?}", list.items);
    assert!(spec.iter().all(|i| i.is_step()), "{:?}", spec);
    assert!(list.selected().is_none(), "a step row was committable");

    // Enter writes the highlighted token and offers what may follow.
    let token = ed
        .completion
        .as_ref()
        .unwrap()
        .highlighted()
        .and_then(|i| i.step_to())
        .map(str::to_string);
    type_script(&mut ed, "Enter");
    assert_eq!(ed.minibuffer, token, "the token was not written");
    assert!(latex(&ed).is_empty(), "a step row executed something");
    assert!(
        ed.completion.as_ref().is_some_and(|l| !l.items.is_empty()),
        "the list did not ask for the rest"
    );

    // The arrows walk shape rows too: a row hidden below them has to
    // be reachable.
    let mut ed = Editor::new();
    type_script(&mut ed, r"\ f r a k Down");
    let first = ed.completion.as_ref().unwrap().sel;
    type_script(&mut ed, "Down");
    assert_ne!(
        ed.completion.as_ref().unwrap().sel,
        first,
        "the list did not move"
    );

    // A family row leaves the family's own prefix typed.
    let mut ed = Editor::new();
    type_script(&mut ed, r"\ f r Down");
    while ed
        .completion
        .as_ref()
        .and_then(|l| l.highlighted())
        .is_some_and(|i| !i.names.starts_with("frak{"))
    {
        type_script(&mut ed, "Down");
    }
    type_script(&mut ed, "Enter");
    assert_eq!(ed.minibuffer.as_deref(), Some("frak"));
    // …and the styled letter still types, which is the point of the row.
    type_script(&mut ed, "A Enter");
    assert_eq!(aa(&ed), "𝔄");
}

/// The delimiter names are spec tokens, not commands: `\lceil` alone
/// would have to mean a pair, and then `\rceil` alone would mean the
/// same one, which reads as nonsense. They work where they are read
/// in visual order — inside `\lr`.
#[test]
fn delimiter_names_are_spec_tokens() {
    for cmd in ["lparen", "lbrack", "lceil", "lfloor", "rparen", "rceil"] {
        let mut ed = Editor::new();
        ed.execute(cmd);
        assert!(
            ed.message.contains("unknown"),
            "\\{}: {:?}",
            cmd,
            ed.message
        );
        assert!(aa(&ed).is_empty(), "\\{} inserted something", cmd);
    }
    // \mid and \dot keep the meanings they already had.
    let mut ed = Editor::new();
    ed.execute("mid");
    assert_eq!(aa(&ed), "∣", "\\mid is the divides atom outside a pair");
    // A spec can name ceil/floor, so a mismatched pair is writable.
    let mut ed = Editor::new();
    type_script(&mut ed, r"\lr\lceil\rfloor x");
    assert_eq!(aa(&ed), "⌈𝑥⌋");
    // …and bare \lr is the start of a spec, not the ↔ arrow.
    let mut ed = Editor::new();
    type_script(&mut ed, r"\lr");
    assert!(aa(&ed).is_empty(), "bare \\lr inserted {}", aa(&ed));
}

/// Tab means "run this", so it must not fire on a spelling that only
/// explains itself: `\lr` is the start of a spec, and pressing Tab on
/// it used to print the usage line instead of offering the tokens.
#[test]
fn tab_does_not_commit_a_half_written_spec() {
    for q in [r"\ l r", r"\ d e l i m"] {
        let mut ed = Editor::new();
        type_script(&mut ed, &format!("{} Tab", q));
        assert!(ed.message.is_empty(), "{:?} -> {:?}", q, ed.message);
        assert!(ed.minibuffer.is_some(), "{:?} closed the minibuffer", q);
        assert!(
            ed.completion.as_ref().is_some_and(|l| !l.items.is_empty()),
            "{:?} offered nothing",
            q
        );
    }
    // …while a spelling that really is a command still commits on Tab.
    let mut ed = Editor::new();
    type_script(&mut ed, r"\ a l p h a Tab");
    assert_eq!(latex(&ed), "\\alpha ");
}

#[test]
fn container_commands_wrap_the_selection() {
    // \norm used to *replace* the selection — ‖ ‖ is a container, and
    // its contents were being typed over like a symbol would be.
    let mut ed = Editor::new();
    type_script(&mut ed, r"ab S-Left S-Left \norm");
    assert_eq!(latex(&ed), "\\left\\|ab\\right\\|");

    // The named delimiter pairs wrap the way `(` does…
    let mut ed = Editor::new();
    type_script(&mut ed, r"ab S-Left S-Left \abs");
    assert_eq!(latex(&ed), "\\left|ab\\right|");
    let mut ed = Editor::new();
    type_script(&mut ed, r"ab S-Left S-Left \lr(]");
    assert_eq!(latex(&ed), "\\left(ab\\right]");

    // …and a pair with a middle lands the selection in its first
    // segment with the cursor in the next, ready for the other half.
    let mut ed = Editor::new();
    type_script(&mut ed, r"a S-Left \braket b");
    assert_eq!(latex(&ed), "\\left\\langle a\\middle|b\\right\\rangle ");
    let mut ed = Editor::new();
    type_script(&mut ed, r"a S-Left \set b");
    assert_eq!(latex(&ed), "\\left\\{a\\middle|b\\right\\}");
}

/// The mode commands: minibuffer spellings for the ctrl chords, so a
/// terminal that steals ^F/^B/^T/^C/^Q still has every mode. They
/// run on commit like any command, and the ordinary edits keep their
/// meaning beside them (\t is a mode, \ta and \tau stay edits).
#[test]
fn mode_commands_run_from_the_minibuffer() {
    // \free enters free-cursor mode; \f is the short form.
    let mut ed = Editor::new();
    type_script(&mut ed, r"x \free");
    assert!(ed.free.is_some(), "free mode did not start");
    let mut ed = Editor::new();
    type_script(&mut ed, r"x \f");
    assert!(ed.free.is_some());

    // \b starts block select, \t toggles grid edit inside a matrix.
    let mut ed = Editor::new();
    type_script(&mut ed, r"x ^ 2 \b");
    assert!(ed.block.is_some(), "block select did not start");
    let mut ed = Editor::new();
    type_script(&mut ed, r"\pmatrix22 x \g");
    assert!(ed.grid.is_some(), "grid mode did not start");

    // \clipboard puts the AA on the *system* clipboard, exactly like
    // ^Y. No \c: one letter beside ^C (the internal copy) would read
    // as the same thing, and it is not.
    let mut ed = Editor::new();
    let fx = type_script(&mut ed, r"ab \clipboard");
    assert!(fx.contains(&Effect::CopyAa), "{:?}", fx);
    let mut ed = Editor::new();
    let fx = type_script(&mut ed, r"ab \c");
    assert!(!fx.contains(&Effect::CopyAa), "\\c still copies");

    // \quit quits: the effect reaches the host. The one-letter \q
    // does not — a quit one typo away is the wrong price for brevity.
    let mut ed = Editor::new();
    let fx = type_script(&mut ed, r"\quit");
    assert!(fx.contains(&Effect::Quit), "{:?}", fx);
    let mut ed = Editor::new();
    let fx = type_script(&mut ed, r"\q");
    assert!(!fx.contains(&Effect::Quit), "\\q still quits");

    // Tab never runs a mode command — no matter how complete the
    // spelling or which row is highlighted, only an explicit
    // Enter/Space commits one.
    let mut ed = Editor::new();
    type_script(&mut ed, r"x \ f Tab");
    assert!(ed.free.is_none(), "Tab ran a mode command");
    type_script(&mut ed, r"Tab Tab");
    assert!(ed.free.is_none(), "Tab took the mode row");
    type_script(&mut ed, r"Enter");
    assert!(ed.free.is_some(), "Enter did not run it");

    // …and the neighbouring edits are untouched.
    let mut ed = Editor::new();
    type_script(&mut ed, r"\tau");
    assert_eq!(latex(&ed), "\\tau ");
    let mut ed = Editor::new();
    type_script(&mut ed, r"\ta");
    assert_eq!(latex(&ed), "\\tau ");
}

/// Grid mode leaves the way the other modes do, and Enter keeps the
/// cell meaningful: its contents become the ordinary selection, so a
/// wrap or a replacement can act on the cell at once.
#[test]
fn grid_mode_exits_on_backslash_and_enter_keeps_the_cell() {
    // `\` leaves the mode like it leaves ^F/^B (consumed, no
    // minibuffer yet — the next `\` opens it).
    let mut ed = Editor::new();
    type_script(&mut ed, r"\pmatrix22 ab C-t");
    assert!(ed.grid.is_some());
    type_script(&mut ed, r"\");
    assert!(ed.grid.is_none(), "backslash did not leave grid mode");
    assert!(
        ed.minibuffer.is_none(),
        "the leaving key opened the minibuffer"
    );

    // Enter: out of the mode with the cell's contents selected —
    // `\norm` can wrap them immediately.
    let mut ed = Editor::new();
    type_script(&mut ed, r"\pmatrix22 ab C-t Enter");
    assert!(ed.grid.is_none());
    assert_eq!(ed.selection(), Some((0, 2)), "the cell is not selected");
    type_script(&mut ed, r"\norm");
    assert!(latex(&ed).contains("\\|ab"), "{}", latex(&ed));

    // A multi-cell rectangle has no linear reading: Enter just leaves.
    let mut ed = Editor::new();
    type_script(&mut ed, r"\pmatrix22 ab C-t S-Right Enter");
    assert!(ed.grid.is_none());
    assert_eq!(ed.selection(), None);
}

/// A mouse pick in the completion popup accepts the clicked row like
/// Enter would: command rows run, step rows continue the spelling, and
/// an out-of-range index is a no-op.
#[test]
fn clicking_a_completion_row_accepts_it() {
    // Click the second row of \al's list and get exactly what
    // Enter on it gives.
    let mut ed = Editor::new();
    type_script(&mut ed, r"\ a l Down");
    let second = ed.completion.as_ref().unwrap().items[1]
        .commit()
        .unwrap()
        .to_string();
    ed.completion_click(1);
    let mut expected = Editor::new();
    expected.execute(&second);
    assert_eq!(latex(&ed), latex(&expected), "clicked \\{}", second);
    assert!(ed.completion.is_none() && ed.minibuffer.is_none());

    // A step row continues the spelling instead of running.
    let mut ed = Editor::new();
    type_script(&mut ed, r"\ l r Down");
    ed.completion_click(0);
    assert!(ed.minibuffer.as_deref().is_some_and(|m| m.len() > 2));
    assert!(latex(&ed).is_empty(), "a step row executed something");

    // Out of range: nothing happens.
    let mut ed = Editor::new();
    type_script(&mut ed, r"\ a l Down");
    ed.completion_click(99);
    assert!(ed.completion.is_some() && latex(&ed).is_empty());
}
