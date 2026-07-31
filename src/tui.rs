//! Everything that paints: the canvas layout (centering, scrolling),
//! the marker/selection boxes and the per-cell styling. The formula
//! itself is rendered by `mascii::render`; this turns that block plus
//! the editor's zero-width annotations into styled terminal spans.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block as UiBlock, Borders, Paragraph};

use mascii::editor::{
    BLK_CLOSE, Editor, JUMP_CHAR_BASE, JUMP_LABELS, JUMP_RANK_BASE, SEL_CLOSE, SEL_OPEN,
};
use mascii::render::{RenderCtx, render_root};

use crate::theme;

const HELP: &str = "^G jump  ^F free move  ^B select block  \\cmd  ^/_ ( [ { // insets  Tab exit  ←→↑↓/click move  ⇧←→/⇧↑ select  ^Z/^R undo/redo  ^T italic  ^Y copy AA  ^S save  Esc/^Q quit";

/// Context-sensitive last line: generic keys normally, the relevant
/// commands when the cursor is inside a grid cell or a delimiter.
pub fn help_line(ed: &Editor) -> &'static str {
    use mascii::ast::Field;
    if ed.minibuffer.is_some() {
        return "command: type at the cursor  Enter/Space execute  Esc cancel";
    }
    if let Some((kind, _)) = &ed.op_entry {
        return if *kind == mascii::editor::BoxKind::Tex {
            "latex: type or paste math (no $ needed)  Enter/Tab commit  Esc cancel"
        } else {
            "op name: letters/digits + Space (word pieces)  Enter/Tab commit  Esc cancel"
        };
    }
    if ed.free.is_some() {
        return if ed.jump.is_some() {
            "free markers: label letter or ←→↑↓ + Enter jumps (then keep flying)  Esc/^G markers off"
        } else {
            "free move: ←→↑↓ cells  ^G markers  Enter snap  Esc cancel"
        };
    }
    if let Some(gs) = ed.grid {
        return match gs {
            mascii::editor::GridSel::Cells { .. } => {
                "grid: ←→↑↓ cells  ⇧ select (past the edge = lane)  c/| columns  r/- rows  ^C/^X/^V cells  ⌫ clear  Enter edit  Esc/^O exit"
            }
            mascii::editor::GridSel::Lanes { cols: true, .. } => {
                "columns: ←→ gap/column  Enter on gap = insert here  ⌫ delete column  ⇧←→ extend  ↑↓ cells  Esc back"
            }
            mascii::editor::GridSel::Lanes { cols: false, .. } => {
                "rows: ↑↓ gap/row  Enter on gap = insert here  ⌫ delete row  ⇧↑↓ extend  ←→ cells  Esc back"
            }
        };
    }
    match ed.path.last() {
        Some((_, Field::Cell(_))) => {
            "grid: ^O edit mode (move cells, add/delete rows & cols)  Enter add row  ] exit  (then ^/_ etc. as usual)"
        }
        Some((_, Field::Seg(_))) => {
            "delim: \\mid adds a │ segment  ) ] } close  \\lr<spec> visual pairs (\\lr(] \\lr{|}, . = none)"
        }
        _ => HELP,
    }
}

#[derive(Default)]
pub struct View {
    pub scroll_x: usize,
    pub scroll_y: usize,
}

/// Draw the whole UI; returns the screen coordinates of the formula's
/// top-left cell (for mouse hit-testing).
pub fn draw(f: &mut Frame, ed: &Editor, view: &mut View) -> (u16, u16) {
    let [canvas_area, help_area] =
        Layout::vertical([Constraint::Min(3), Constraint::Length(1)]).areas(f.area());

    let origin = draw_canvas(f, canvas_area, ed, view);

    // One bottom line: messages overlay the usage line when present
    // (the minibuffer itself shows in-place at the cursor).
    let bottom = if !ed.message.is_empty() {
        let fg = if ed.message_error {
            theme::MESSAGE_ERR_FG
        } else {
            theme::MESSAGE_FG
        };
        Line::from(Span::styled(
            format!(" {}", ed.message),
            Style::default().fg(fg),
        ))
    } else {
        Line::from(Span::styled(
            format!(" {}", help_line(ed)),
            Style::default().fg(theme::CHROME_FG),
        ))
    };
    f.render_widget(bottom, help_area);
    origin
}

/// Returns the screen position of the formula's top-left cell.
fn draw_canvas(f: &mut Frame, area: Rect, ed: &Editor, view: &mut View) -> (u16, u16) {
    let border = UiBlock::default()
        .borders(Borders::ALL)
        .title(" mascii ")
        .border_style(Style::default().fg(theme::BORDER_FG));
    let inner = border.inner(area);
    f.render_widget(border, area);

    let ctx = RenderCtx { italic: ed.italic };
    let (root, cursor) = ed.decorated();
    let cursor_ref = cursor.as_ref().map(|(p, c)| (p.as_slice(), *c));
    let block = render_root(&root, cursor_ref, &ctx);
    let (lines, mut bg, mut cursor_cell, frame) = marker_boxes(
        &block.lines,
        &ed.marker_extents(),
        &block.marks,
        block.caret,
        ed.jump.is_some().then_some(ed.jump_selected),
        ed.block.is_some().then_some(ed.block_sel),
    );
    let mut struck: std::collections::HashSet<(usize, usize)> =
        block.cancel.iter().copied().collect();
    let mut lines = lines;
    // ^F: the free cursor itself gets the prominent caret style; the
    // snap preview is the subtler colored cell.
    if let Some(f) = &ed.free {
        let (sy, sx) = f.snap_at;
        if sy < lines.len() {
            if sx >= lines[sy].len() {
                lines[sy].resize(sx + 1, ' ');
                bg[sy].resize(sx + 1, None);
            }
            let last = bg[sy].len().saturating_sub(1);
            bg[sy][sx.min(last)] = Some(theme::FREE_BG);
        }
        let (fy, fx) = f.at;
        if fy < lines.len() {
            if fx >= lines[fy].len() {
                lines[fy].resize(fx + 1, ' ');
                bg[fy].resize(fx + 1, None);
            }
            cursor_cell = Some((fy, fx));
        }
    }

    overlay_minibuffer(ed, &mut lines, &mut bg, &mut struck, &mut cursor_cell);

    let width = (block.width() as u16).max(lines.iter().map(|l| l.len() as u16).max().unwrap_or(0));
    let height = lines.len() as u16;
    // A formula larger than the canvas scrolls, following the cursor
    // (the free cursor included) with a few cells of margin; one that
    // fits is centered on that axis and its scroll resets. `scroll`
    // returns the offset, `pad` the centering pad.
    let scroll = |size: u16, avail: u16, cur: Option<usize>, off: &mut usize| -> u16 {
        if size <= avail {
            *off = 0;
            return avail.saturating_sub(size) / 2;
        }
        let vis = avail as usize;
        let margin = 4.min(vis / 4);
        if let Some(c) = cur {
            *off = (*off).min(c.saturating_sub(margin));
            if c + margin >= *off + vis {
                *off = c + margin + 1 - vis;
            }
        }
        *off = (*off).min(size as usize - vis);
        0
    };
    let left = scroll(
        width,
        inner.width,
        cursor_cell.map(|(_, cx)| cx),
        &mut view.scroll_x,
    );
    let top = scroll(
        height,
        inner.height,
        cursor_cell.map(|(cy, _)| cy),
        &mut view.scroll_y,
    );

    let mut text: Vec<Line> = Vec::with_capacity(top as usize + lines.len());
    for _ in 0..top {
        text.push(Line::raw(""));
    }
    let pad = " ".repeat(left as usize);
    for (y, (l, bgrow)) in lines.iter().zip(&bg).enumerate().skip(view.scroll_y) {
        let ccol = cursor_cell.and_then(|(cy, cx)| (cy == y).then_some(cx));
        let mut spans = vec![Span::raw(pad.clone())];
        spans.extend(decorate_line(
            l,
            y,
            &struck,
            bgrow,
            ccol,
            ed.free.is_some(),
            ed.op_entry.is_some(),
            view.scroll_x,
            frame,
        ));
        text.push(Line::from(spans));
    }
    f.render_widget(Paragraph::new(text), inner);
    (inner.x + left, inner.y + top)
}

/// Turn the zero-width display annotations of a rendered block into
/// colored boxes, overlaid labels and the caret cell. Marks carry
/// (row, col, char): jump/block labels overlay the glyph at their
/// position; selection/block mark pairs paint a background box (the
/// selection in theme::SELECTION_BG, ^B blocks in the depth palette
/// by nesting depth). Nothing is inserted or removed, so the geometry
/// always equals the undecorated render.
#[allow(clippy::type_complexity)]
fn marker_boxes(
    lines: &[Vec<char>],
    extents: &[(usize, usize, usize)],
    marks: &[(usize, usize, char)],
    caret: Option<(usize, usize)>,
    selected: Option<usize>,
    block_selected: Option<usize>,
) -> (
    Vec<Vec<char>>,
    Vec<Vec<Option<Color>>>,
    Option<(usize, usize)>,
    Option<(usize, usize, usize, usize)>,
) {
    // Cell coordinates throughout — cancel strikes (combining U+0338)
    // are a separate channel applied at span emission, so a struck cell
    // never desyncs the column indexing (or splits its ligature).
    let mut grid: Vec<Vec<char>> = lines.to_vec();
    if grid.is_empty() {
        grid.push(Vec::new());
    }
    let labels = JUMP_LABELS.chars().count() as u32;
    let is_label = |c: char| (JUMP_CHAR_BASE..JUMP_CHAR_BASE + labels).contains(&(c as u32));

    let mut bg: Vec<Vec<Option<Color>>> = grid.iter().map(|row| vec![None; row.len()]).collect();
    // Boxes: pair opens (selection start, ^B label) with closes within
    // each row, nesting by position. Selection pairs consume extents in
    // encounter order (rows top-down, columns left-right) — the same
    // order the editor lists them in (one per grid cell, or one).
    let mut boxes: Vec<(usize, usize, usize, Color, usize, usize)> = Vec::new();
    let mut order: Vec<usize> = Vec::new(); // paint order key: depth
    let mut by_row: std::collections::BTreeMap<usize, Vec<(usize, char)>> =
        std::collections::BTreeMap::new();
    for &(y, x, c) in marks {
        by_row.entry(y).or_default().push((x, c));
    }
    let mut sel_seq = 0usize;
    // Grid selection pairs: filled as one union rectangle (lattice
    // gaps included), not per-cell patches. Cell rectangles (contents)
    // and lanes (the column/row itself) collect separately — they
    // paint in different colors. The frame pair (whole edited array)
    // reads its extent from the end of the extents list.
    let mut cell_boxes: Vec<(usize, usize, usize, usize)> = Vec::new(); // (o, close, t, b)
    let mut lane_boxes: Vec<(usize, usize, usize, usize)> = Vec::new();
    let mut row_lane_boxes: Vec<(usize, usize, usize, usize)> = Vec::new();
    // The frame rectangle comes straight off its corner marks — the
    // render places FRAME_OPEN on the framed block's top-left cell and
    // FRAME_CLOSE on its bottom-right (delimiters included when fused).
    let tl = marks
        .iter()
        .find(|&&(_, _, c)| c == mascii::editor::FRAME_OPEN);
    let br = marks
        .iter()
        .find(|&&(_, _, c)| c == mascii::editor::FRAME_CLOSE);
    let frame: Option<(usize, usize, usize, usize)> = match (tl, br) {
        (Some(&(t, x0, _)), Some(&(b, x1, _))) => Some((x0, x1, t, b)),
        _ => None,
    };
    for (y, mut row_marks) in by_row {
        row_marks.sort_unstable();
        let mut stack: Vec<(usize, char)> = Vec::new();
        for (x, c) in row_marks {
            // A close mark pops its *matching* open — standalone marks
            // (jump ranks, gap ghosts) must never satisfy a pair.
            let pop_matching = |stack: &mut Vec<(usize, char)>,
                                ok: &dyn Fn(char) -> bool|
             -> Option<(usize, char)> {
                let at = stack.iter().rposition(|&(_, c)| ok(c))?;
                Some(stack.remove(at))
            };
            if c == mascii::editor::FRAME_CLOSE || c == mascii::editor::FRAME_OPEN {
                // Handled by the corner scan below the loop.
                continue;
            }
            if c == mascii::editor::LANE_CLOSE
                || c == mascii::editor::ROWLANE_CLOSE
                || c == mascii::editor::CELLS_CLOSE
            {
                let open_of = match c {
                    mascii::editor::LANE_CLOSE => mascii::editor::LANE_OPEN,
                    mascii::editor::ROWLANE_CLOSE => mascii::editor::ROWLANE_OPEN,
                    _ => mascii::editor::CELLS_OPEN,
                };
                if let Some((o, _)) = pop_matching(&mut stack, &|k| k == open_of) {
                    let (t, b) = match extents.get(sel_seq) {
                        Some(&(above, below, _)) => (
                            y.saturating_sub(above),
                            (y + below).min(grid.len().saturating_sub(1)),
                        ),
                        None => (y, y),
                    };
                    sel_seq += 1;
                    // An empty cell's pair is zero-width: keep one
                    // cell so the selection stays visible.
                    let entry = (o, x.max(o + 1), t, b);
                    if c == mascii::editor::CELLS_CLOSE {
                        cell_boxes.push(entry);
                    } else if c == mascii::editor::LANE_CLOSE {
                        lane_boxes.push(entry);
                    } else {
                        row_lane_boxes.push(entry);
                    }
                }
                continue;
            }
            if c == SEL_CLOSE || c == BLK_CLOSE {
                if let Some((o, oc)) = pop_matching(&mut stack, &|k| {
                    if c == SEL_CLOSE {
                        k == SEL_OPEN
                    } else {
                        is_label(k)
                    }
                }) {
                    let (color, depth, ext) = if oc == SEL_OPEN {
                        let e = extents.get(sel_seq);
                        sel_seq += 1;
                        (theme::SELECTION_BG, 0, e)
                    } else {
                        let idx = (oc as u32 - JUMP_CHAR_BASE) as usize;
                        let e = extents.get(idx);
                        let d = e.map_or(0, |&(_, _, d)| d);
                        let c = if block_selected == Some(idx) {
                            theme::SELECTED_BG
                        } else {
                            theme::DEPTH_BG[d % theme::DEPTH_BG.len()]
                        };
                        (c, d, e)
                    };
                    let (t, b) = match ext {
                        Some(&(above, below, _)) => (
                            y.saturating_sub(above),
                            (y + below).min(grid.len().saturating_sub(1)),
                        ),
                        None => (y, y),
                    };
                    boxes.push((y, o, x, color, t, b));
                    order.push(depth);
                }
            } else if c == SEL_OPEN
                || c == mascii::editor::LANE_OPEN
                || c == mascii::editor::ROWLANE_OPEN
                || c == mascii::editor::CELLS_OPEN
                || is_label(c)
            {
                stack.push((x, c));
            }
        }
    }
    // Outer boxes first so nested ones paint over them. The ^B rank is
    // innermost-first (rank 0 = the innermost parent), so paint in
    // descending rank: outermost ancestors below, inner ones on top.
    let mut idx: Vec<usize> = (0..boxes.len()).collect();
    idx.sort_by_key(|&i| std::cmp::Reverse(order[i]));
    for i in idx {
        let (_, o, close, color, t, b) = boxes[i];
        for row in bg.iter_mut().take(b + 1).skip(t) {
            for x in o..close {
                if x < row.len() {
                    row[x] = Some(color);
                }
            }
        }
    }
    // The grid selections paint one solid rectangle in the selection
    // color. A cell rectangle hugs the union of its slots; a lane
    // stretches along its axis to the matrix region's far edges (that
    // reach is what tells "the column itself" from "every cell of the
    // column").
    let mut fill = |x0: usize, x1: usize, t: usize, b: usize| {
        for row in bg.iter_mut().take(b + 1).skip(t) {
            for x in x0..x1 {
                if x < row.len() {
                    row[x] = Some(theme::SELECTION_BG);
                }
            }
        }
    };
    let bbox = |g: &Vec<(usize, usize, usize, usize)>| {
        (
            g.iter().map(|&(o, ..)| o).min().unwrap(),
            g.iter().map(|&(_, c, ..)| c).max().unwrap(),
            g.iter().map(|&(.., t, _)| t).min().unwrap(),
            g.iter().map(|&(.., b)| b).max().unwrap(),
        )
    };
    if !cell_boxes.is_empty() {
        let (x0, x1, t, b) = bbox(&cell_boxes);
        fill(x0, x1, t, b);
    }
    if !lane_boxes.is_empty() {
        let (x0, x1, t, b) = bbox(&lane_boxes);
        // Columns: top-to-bottom edge of the matrix region, and one
        // cell of slot padding sideways — a fused matrix has no extra
        // rows above its cells, so the extra width is what keeps the
        // column band tellable from a full-column cell selection.
        let (t, b) = frame.map_or((t, b), |(_, _, ft, fb)| (ft, fb));
        fill(x0.saturating_sub(1), x1 + 1, t, b);
    }
    if !row_lane_boxes.is_empty() {
        let (x0, x1, t, b) = bbox(&row_lane_boxes);
        // Rows: left-to-right edge of the matrix region.
        let (x0, x1) = frame.map_or((x0, x1), |(fo, fc, _, _)| (fo, fc + 1));
        fill(x0, x1, t, b);
    }
    // Labels overlay the glyph at their position. Jump markers carry
    // their rank: within the label alphabet they show the label letter,
    // beyond it a highlight cell; the arrow-selected marker gets its
    // own color either way.
    let rank_of = |c: char| {
        let u = c as u32;
        (JUMP_RANK_BASE..JUMP_RANK_BASE + 0x400)
            .contains(&u)
            .then(|| (u - JUMP_RANK_BASE) as usize)
    };
    // The grid lane-gap cursor: the ghost lane paints with exactly the
    // lane-selection geometry (edge-to-edge along its axis, slot
    // padding sideways for columns), just in the insert green.
    for (mark, is_col) in [
        (mascii::editor::GRID_GAP, true),
        (mascii::editor::GRID_GAP_ROW, false),
    ] {
        let gaps: Vec<(usize, usize)> = marks
            .iter()
            .filter(|&&(_, _, c)| c == mark)
            .map(|&(y, x, _)| (y, x))
            .collect();
        if gaps.is_empty() {
            continue;
        }
        let t = gaps.iter().map(|&(y, _)| y).min().unwrap();
        let b = gaps.iter().map(|&(y, _)| y).max().unwrap();
        let x0 = gaps.iter().map(|&(_, x)| x).min().unwrap();
        let x1 = gaps.iter().map(|&(_, x)| x).max().unwrap() + 1;
        let (x0, x1, t, b) = if is_col {
            let (t, b) = frame.map_or((t, b), |(_, _, ft, fb)| (ft, fb));
            (x0.saturating_sub(1), x1 + 1, t, b)
        } else {
            let (x0, x1) = frame.map_or((x0, x1), |(fo, fc, _, _)| (fo, fc + 1));
            (x0, x1, t, b)
        };
        for row in bg.iter_mut().take(b + 1).skip(t) {
            for x in x0..x1 {
                if x < row.len() {
                    row[x] = Some(theme::GRID_INSERT_BG);
                }
            }
        }
    }
    for &(y, x, c) in marks {
        if c == mascii::editor::GRID_GAP {
            continue;
        }
        let rank = rank_of(c);
        if !(is_label(c) || rank.is_some()) {
            continue;
        }
        let row = &mut grid[y];
        if x >= row.len() {
            row.resize(x + 1, ' ');
            bg[y].resize(x + 1, None);
        }
        match rank {
            // A ^B label: same look as the ^G labels — its cell keeps
            // the label ground (the depth box would swallow the black
            // glyph otherwise), or the selection color when its
            // ancestor is the arrow-picked one.
            None => {
                row[x] = c;
                let idx = (c as u32 - JUMP_CHAR_BASE) as usize;
                bg[y][x] = Some(if block_selected == Some(idx) {
                    theme::SELECTED_BG
                } else {
                    theme::LABEL_BG
                });
            }
            Some(r) => {
                let is_sel = selected == Some(r);
                if let Some(label) = JUMP_LABELS.chars().nth(r) {
                    // Re-encode as a display label char for styling.
                    row[x] = char::from_u32(JUMP_CHAR_BASE + r as u32).unwrap_or(label);
                } else if bg[y][x].is_none() {
                    bg[y][x] = Some(theme::UNLABELED_BG);
                }
                if is_sel {
                    bg[y][x] = Some(theme::SELECTED_BG);
                }
            }
        }
    }
    // The caret cell (padded blank at the row end).
    let cursor_cell = caret.map(|(y, x)| {
        if x >= grid[y].len() {
            grid[y].resize(x + 1, ' ');
            bg[y].resize(x + 1, None);
        }
        (y, x)
    });
    (grid, bg, cursor_cell, frame)
}

/// Draw the open minibuffer as an overlay at the caret cell: the typed
/// `\command` covers the glyphs to the right of the cursor without
/// moving them (zero layout shift — the eye stays on the formula), and
/// the caret sits after the text. With a selection active the caret is
/// at the selection's moving end, so the overlay shows next to it.
fn overlay_minibuffer(
    ed: &Editor,
    lines: &mut Vec<Vec<char>>,
    bg: &mut Vec<Vec<Option<Color>>>,
    struck: &mut std::collections::HashSet<(usize, usize)>,
    cursor_cell: &mut Option<(usize, usize)>,
) {
    // The in-place name box (\op \rm \text \tex) overlays exactly
    // like the minibuffer: content cells at the cursor, caret at the
    // box's own cursor. Drawing it as cells (not AST nodes) keeps the
    // run free of the reparse-quoting rules — no stray '…' quotes.
    if let Some((kind, buf)) = &ed.op_entry {
        let Some((cy, cx)) = *cursor_cell else { return };
        if cy >= lines.len() {
            return;
        }
        let text: Vec<char> = if buf.is_empty() {
            vec!['⬚']
        } else if *kind == mascii::editor::BoxKind::OpStar {
            // Only \op* gives a space meaning (piece separator), so
            // only there it draws as ␣ — matching the committed band.
            buf.replace(' ', "␣").chars().collect()
        } else {
            buf.chars().collect()
        };
        let end = cx + text.len();
        if lines[cy].len() < end + 1 {
            lines[cy].resize(end + 1, ' ');
            bg[cy].resize(end + 1, None);
        }
        for (i, &ch) in text.iter().enumerate() {
            lines[cy][cx + i] = ch;
            bg[cy][cx + i] = Some(theme::MINIBUF_BG);
        }
        *cursor_cell = Some((cy, cx + ed.op_cursor.min(text.len())));
        return;
    }
    let Some(buf) = &ed.minibuffer else { return };
    let Some((cy, cx)) = *cursor_cell else { return };
    if cy >= lines.len() {
        return;
    }
    let text: Vec<char> = std::iter::once('\\').chain(buf.chars()).collect();
    let end = cx + text.len();
    if lines[cy].len() < end + 1 {
        lines[cy].resize(end + 1, ' ');
        bg[cy].resize(end + 1, None);
    }
    // Live feedback: the overlay turns red as soon as the typed name
    // is not something \execute knows (it goes back on completion).
    let color = if ed.command_known(buf) {
        theme::MINIBUF_BG
    } else {
        theme::MINIBUF_BAD_BG
    };
    for (i, &ch) in text.iter().enumerate() {
        lines[cy][cx + i] = ch;
        bg[cy][cx + i] = Some(color);
    }
    // The command previews what committing would insert, as a small
    // box right under the typed name — a symbol as its one character,
    // a structure (\frac, \pmatrix …) with its empty ⬚ slots. The box
    // gets rows of its own: everything below the caret row shifts down
    // by the preview's height (backgrounds and cancel strikes ride
    // along), so the preview never hides the formula it points into.
    if let Some(row) = ed.command_preview_row() {
        use mascii::render::{RenderCtx, render_root};
        let block = render_root(&row, None, &RenderCtx::canonical());
        let h = block.height();
        let at = cy + 1;
        while lines.len() < at {
            lines.push(Vec::new());
            bg.push(Vec::new());
        }
        for _ in 0..h {
            lines.insert(at, Vec::new());
            bg.insert(at, Vec::new());
        }
        *struck = struck
            .iter()
            .map(|&(y, x)| if y >= at { (y + h, x) } else { (y, x) })
            .collect();
        for (dy, bline) in block.lines.iter().enumerate() {
            let y = at + dy;
            let need = cx + bline.len();
            lines[y].resize(need, ' ');
            bg[y].resize(need, None);
            for (dx, &ch) in bline.iter().enumerate() {
                lines[y][cx + dx] = ch;
                bg[y][cx + dx] = Some(theme::PREVIEW_BG);
            }
        }
    }
    *cursor_cell = Some((cy, end));
}

/// Turn a rendered cell row into spans: private-use marker chars become
/// colored jump/block labels, the cursor glyph blinks, and box
/// backgrounds from `marker_boxes` are applied to plain glyphs. A
/// struck cell gets its combining U+0338 appended *inside* its span,
/// so the ligature is never split across style boundaries.
/// Lattice glyphs (the array's own frame and separators): recolored
/// anywhere inside the frame rectangle.
fn is_lattice_glyph(c: char) -> bool {
    "┌┬┐├┼┤└┴┘".contains(c)
}

/// Delimiter pieces: recolored only on the rectangle's edge columns —
/// a fused matrix's own parens sit exactly there, while a delimiter
/// nested inside a cell does not.
fn is_delim_piece(c: char) -> bool {
    "()[]{}⌈⌉⌊⌋⟨⟩╱╲│╭╮╰╯⎡⎢⎣⎤⎥⎦⎛⎜⎝⎞⎟⎠⎧⎪⎨⎩⎫⎬⎭‖┆┊".contains(c)
}

#[allow(clippy::too_many_arguments)]
fn decorate_line(
    line: &[char],
    y: usize,
    struck: &std::collections::HashSet<(usize, usize)>,
    bg: &[Option<Color>],
    cursor: Option<usize>,
    free_caret: bool,
    box_caret: bool,
    scroll_x: usize,
    frame: Option<(usize, usize, usize, usize)>,
) -> Vec<Span<'static>> {
    // The ordinary caret is terminal-style reverse video; the free
    // cursor is a solid colored block instead (reverse video would swap
    // the tint onto the glyph, which reads as "the character changed
    // color", not "the cursor changed color").
    let cursor_style = if box_caret {
        // Inside a name box: the caret turns green so the modal layer
        // is visible at the cursor itself.
        Style::default()
            .fg(theme::BOX_CURSOR_FG)
            .bg(theme::BOX_CURSOR_BG)
            .add_modifier(Modifier::BOLD | Modifier::SLOW_BLINK)
    } else if free_caret {
        Style::default()
            .fg(theme::FREE_CURSOR_FG)
            .bg(theme::FREE_CURSOR_BG)
            .add_modifier(Modifier::BOLD | Modifier::SLOW_BLINK)
    } else {
        Style::default().add_modifier(Modifier::REVERSED | Modifier::BOLD | Modifier::SLOW_BLINK)
    };
    let label_style = Style::default()
        .fg(theme::LABEL_FG)
        .bg(theme::LABEL_BG)
        .add_modifier(Modifier::BOLD);

    let mut spans = Vec::new();
    let mut buf = String::new();
    let mut buf_bg: Option<Color> = None;
    let flush = |buf: &mut String, buf_bg: Option<Color>, spans: &mut Vec<Span<'static>>| {
        if !buf.is_empty() {
            let s = std::mem::take(buf);
            spans.push(match buf_bg {
                Some(color) => Span::styled(s, Style::default().bg(color)),
                None => Span::raw(s),
            });
        }
    };
    for (i, &c) in line.iter().enumerate().skip(scroll_x) {
        let u = c as u32;
        let cell_bg = bg.get(i).copied().flatten();
        let cell = if struck.contains(&(y, i)) {
            format!("{}\u{338}", c)
        } else {
            c.to_string()
        };
        if cursor == Some(i) {
            // Terminal-style caret: reverse video on the glyph right of
            // the insertion point.
            flush(&mut buf, buf_bg, &mut spans);
            let style = match cell_bg {
                Some(color) if !free_caret => cursor_style.bg(color),
                _ => cursor_style,
            };
            spans.push(Span::styled(cell, style));
        } else if c == '␣' {
            // Explicit space atom: keep visible but unobtrusive.
            flush(&mut buf, buf_bg, &mut spans);
            let mut style = Style::default().fg(theme::SPACE_FG);
            if let Some(color) = cell_bg {
                style = style.bg(color);
            }
            spans.push(Span::styled(cell, style));
        } else if (JUMP_CHAR_BASE..JUMP_CHAR_BASE + JUMP_LABELS.chars().count() as u32).contains(&u)
        {
            let label = JUMP_LABELS
                .chars()
                .nth((u - JUMP_CHAR_BASE) as usize)
                .unwrap();
            flush(&mut buf, buf_bg, &mut spans);
            // The arrow-selected marker keeps its highlight color.
            let style = match cell_bg {
                Some(color) => label_style.bg(color),
                None => label_style,
            };
            spans.push(Span::styled(label.to_string(), style));
        } else if frame.is_some_and(|(o, close, t, b)| {
            // The edited grid's frame recolors while grid mode is on.
            // The rectangle is exact (render-placed corners): lattice
            // glyphs recolor anywhere inside it, delimiter pieces only
            // on its edge columns — so a delimiter nested in a cell
            // stays untinted, and a one-line pmatrix's parens tint.
            (t..=b).contains(&y)
                && ((is_lattice_glyph(c) && (o..=close).contains(&i))
                    || (is_delim_piece(c) && (i == o || i == close)))
        }) {
            flush(&mut buf, buf_bg, &mut spans);
            let mut style = Style::default().fg(theme::GRID_FRAME_FG);
            if let Some(color) = cell_bg {
                style = style.bg(color);
            }
            spans.push(Span::styled(cell, style));
        } else {
            if cell_bg != buf_bg {
                flush(&mut buf, buf_bg, &mut spans);
                buf_bg = cell_bg;
            }
            buf.push_str(&cell);
        }
    }
    flush(&mut buf, buf_bg, &mut spans);
    spans
}

#[cfg(test)]
mod tests {
    /// A formula larger than the canvas scrolls on both axes so the
    /// cursor stays visible; one that fits is centered (offsets 0).
    #[test]
    fn canvas_scrolls_to_follow_the_cursor() {
        use ratatui::{Terminal, backend::TestBackend};
        let render = |ed: &Editor, w: u16, h: u16| -> View {
            let mut view = View::default();
            let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
            term.draw(|f| {
                draw(f, ed, &mut view);
            })
            .unwrap();
            view
        };
        // Fits: centered, no scrolling.
        let mut ed = Editor::new();
        type_script_keys(&mut ed, "x+1");
        let v = render(&ed, 40, 12);
        assert_eq!((v.scroll_x, v.scroll_y), (0, 0));
        // Wide: the cursor sits at the right end, so the view scrolled.
        let mut ed = Editor::new();
        type_script_keys(&mut ed, &"a+".repeat(40));
        let v = render(&ed, 24, 12);
        assert!(v.scroll_x > 0, "no horizontal scroll: {}", v.scroll_x);
        assert_eq!(v.scroll_y, 0);
        // Tall: many display lines, cursor on the last one.
        let mut ed = Editor::new();
        for _ in 0..12 {
            ed.input(Key::Char('x'), false, false);
            ed.input(Key::Enter, false, false);
        }
        let v = render(&ed, 40, 8);
        assert!(v.scroll_y > 0, "no vertical scroll: {}", v.scroll_y);
        // …and moving back to the top scrolls back.
        ed.input(Key::Char('a'), false, true); // ^A: document start
        let v = render(&ed, 40, 8);
        assert_eq!(v.scroll_y, 0, "did not scroll back to the top");
    }

    #[test]
    fn block_mode_paints_the_ancestor_gradient() {
        // Cursor in a cell of a fused (matrix): ^B shows two ancestors
        // — the Array (innermost, highlighted) and the Delim — painted
        // as distinct boxes: the interior gets the selected color, the
        // delimiter columns the next gradient shade.
        let mut ed = Editor::new();
        for c in "\\pmatrix\n".chars() {
            let key = if c == '\n' { Key::Enter } else { Key::Char(c) };
            ed.input(key, false, false);
        }
        ed.input(Key::Char('x'), false, false);
        ed.input(Key::Char('b'), false, true);
        assert_eq!(ed.block.as_ref().map(Vec::len), Some(2));
        let (root, cursor) = ed.decorated();
        let cursor_ref = cursor.as_ref().map(|(p, c)| (p.as_slice(), *c));
        let block = render_root(&root, cursor_ref, &RenderCtx { italic: true });
        let (lines, bg, _, _) = marker_boxes(
            &block.lines,
            &ed.marker_extents(),
            &block.marks,
            block.caret,
            None,
            Some(ed.block_sel),
        );
        // Left delimiter column = outer (Delim) shade; some interior
        // cell = the highlighted innermost (Array) color. The baseline
        // cell holds a label, which keeps the label ground instead of
        // the box shade (that is what makes it readable).
        let row = block.baseline;
        assert_eq!(
            bg[0][0],
            Some(theme::DEPTH_BG[1]),
            "delim column shade\n{:?}",
            lines
        );
        assert_eq!(bg[row][0], Some(theme::LABEL_BG), "label ground");
        let inner = bg[row][2..lines[row].len() - 1]
            .iter()
            .filter_map(|c| *c)
            .collect::<Vec<_>>();
        assert!(
            inner.contains(&theme::SELECTED_BG),
            "interior highlighted: {:?}",
            inner
        );
    }

    use super::*;
    use mascii::input::Key;

    /// Full display pipeline: decorated AST -> render -> marker_boxes.
    fn display(ed: &Editor) -> Vec<String> {
        let (root, cursor) = ed.decorated();
        let cursor_ref = cursor.as_ref().map(|(p, c)| (p.as_slice(), *c));
        let ctx = RenderCtx { italic: true };
        let block = render_root(&root, cursor_ref, &ctx);
        let (lines, _, _, _) = marker_boxes(
            &block.lines,
            &ed.marker_extents(),
            &block.marks,
            block.caret,
            ed.jump.is_some().then_some(ed.jump_selected),
            ed.block.is_some().then_some(ed.block_sel),
        );
        lines.into_iter().map(String::from_iter).collect()
    }

    /// The full grid-mode display pipeline: selected cells paint the
    /// selection color at their own height, and a lane gap shows the
    /// green ghost column.
    #[test]
    fn grid_mode_paints_cells_and_gaps() {
        let bg_of = |ed: &Editor| {
            let (root, cursor) = ed.decorated();
            let cursor_ref = cursor.as_ref().map(|(p, c)| (p.as_slice(), *c));
            let block = render_root(&root, cursor_ref, &RenderCtx { italic: true });
            let (_, bg, _, _) = marker_boxes(
                &block.lines,
                &ed.marker_extents(),
                &block.marks,
                block.caret,
                None,
                None,
            );
            bg
        };
        let mut ed = Editor::new();
        for c in "\\bmatrix a".chars() {
            ed.input(Key::Char(c), false, false);
        }
        ed.input(Key::Char('o'), false, true); // ^O
        // Cell cursor: the current (top-left) cell is painted.
        let bg = bg_of(&ed);
        let painted = bg
            .iter()
            .flatten()
            .filter(|c| **c == Some(theme::SELECTION_BG))
            .count();
        assert!(painted > 0, "cell cursor paints its cell");
        // Column mode on a gap: the green ghost lane appears.
        ed.input(Key::Char('|'), false, false);
        ed.input(Key::Left, false, false); // gap 0
        let bg = bg_of(&ed);
        let green = bg
            .iter()
            .flatten()
            .filter(|c| **c == Some(theme::GRID_INSERT_BG))
            .count();
        assert!(green > 0, "gap cursor paints the ghost lane");
        // Back on a column: the lane band stretches to the matrix
        // region's top and bottom edges — the reach (not a second
        // color) is what tells "the column itself" apart from a
        // cell selection.
        ed.input(Key::Right, false, false);
        let bg = bg_of(&ed);
        let sel_rows: Vec<usize> = bg
            .iter()
            .enumerate()
            .filter(|(_, row)| row.contains(&Some(theme::SELECTION_BG)))
            .map(|(y, _)| y)
            .collect();
        assert!(
            sel_rows.len() >= 3,
            "column lane spans every display row: {:?}",
            sel_rows
        );
        // The frame rect is reported while grid mode is on (the draw
        // path recolors the lattice glyphs inside it).
        let (root, cursor) = ed.decorated();
        let cursor_ref = cursor.as_ref().map(|(p, c)| (p.as_slice(), *c));
        let block = render_root(&root, cursor_ref, &RenderCtx { italic: true });
        let (_, _, _, frame) = marker_boxes(
            &block.lines,
            &ed.marker_extents(),
            &block.marks,
            block.caret,
            None,
            None,
        );
        assert!(frame.is_some(), "grid mode reports its frame rect");
        // The gap's green bar runs unbroken through the lattice rows:
        // every display row between the first and last green cell has
        // green in it (a 2-row matrix has a separator row between).
        ed.input(Key::Left, false, false); // back to gap 0
        let bg = bg_of(&ed);
        let green_rows: Vec<usize> = bg
            .iter()
            .enumerate()
            .filter(|(_, row)| row.contains(&Some(theme::GRID_INSERT_BG)))
            .map(|(y, _)| y)
            .collect();
        assert!(green_rows.len() >= 3, "green spans rows: {:?}", green_rows);
        let (lo, hi) = (green_rows[0], *green_rows.last().unwrap());
        assert_eq!(
            green_rows.len(),
            hi - lo + 1,
            "no holes in the gap bar: {:?}",
            green_rows
        );
        // The column band is strictly wider than the same column's
        // cell selection: it claims the slot padding, which is what
        // keeps "the column itself" visible as such in a fused matrix.
        let width_at = |bg: &Vec<Vec<Option<Color>>>, y: usize| {
            bg[y]
                .iter()
                .filter(|c| **c == Some(theme::SELECTION_BG))
                .count()
        };
        ed.input(Key::Right, false, false); // gap 0 -> column 0
        let bg = bg_of(&ed);
        let lane_row = bg
            .iter()
            .position(|row| row.contains(&Some(theme::SELECTION_BG)))
            .unwrap();
        let lane_w = width_at(&bg, lane_row);
        ed.input(Key::Up, false, false); // demote: full-column cells
        let bg = bg_of(&ed);
        let cells_row = bg
            .iter()
            .position(|row| row.contains(&Some(theme::SELECTION_BG)))
            .unwrap();
        let cells_w = width_at(&bg, cells_row);
        assert!(
            lane_w > cells_w,
            "lane band wider than cell band: {} vs {}",
            lane_w,
            cells_w
        );
    }

    /// The frame survives a gap cursor on the matrix's baseline row:
    /// the gap's ghost marks share that row with the frame pair, and
    /// pairing must not let a ghost mark satisfy the frame close.
    #[test]
    fn frame_survives_a_middle_row_gap() {
        let mut ed = Editor::new();
        for c in r"\bmatrix a".chars() {
            ed.input(Key::Char(c), false, false);
        }
        ed.input(Key::Char('o'), false, true);
        ed.input(Key::Char('r'), false, false);
        ed.input(Key::Down, false, false); // the gap between the rows
        let (root, cursor) = ed.decorated();
        let cursor_ref = cursor.as_ref().map(|(p, c)| (p.as_slice(), *c));
        let block = render_root(&root, cursor_ref, &RenderCtx { italic: true });
        let (_, _, _, frame) = marker_boxes(
            &block.lines,
            &ed.marker_extents(),
            &block.marks,
            block.caret,
            None,
            None,
        );
        let (o, close, t, b) = frame.expect("frame rect survives the gap row");
        assert!(close - o >= 6, "full-width frame: {:?}", (o, close));
        assert!(b - t >= 2, "full-height frame: {:?}", (t, b));
    }

    /// A fused matrix (pmatrix/bmatrix) recolors BOTH delimiter
    /// columns as its frame — the markers sit just inside them.
    #[test]
    fn fused_frame_recolors_both_delimiters() {
        let mut ed = Editor::new();
        for c in r"\pmatrix a".chars() {
            ed.input(Key::Char(c), false, false);
        }
        ed.input(Key::Char('o'), false, true); // ^O
        let (root, cursor) = ed.decorated();
        let cursor_ref = cursor.as_ref().map(|(p, c)| (p.as_slice(), *c));
        let block = render_root(&root, cursor_ref, &RenderCtx { italic: true });
        let (lines, bg, _, frame) = marker_boxes(
            &block.lines,
            &ed.marker_extents(),
            &block.marks,
            block.caret,
            None,
            None,
        );
        let (o, close, t, b) = frame.expect("frame rect");
        // Walk every framed row and collect which columns the draw
        // pass would tint; the leftmost and rightmost delimiter pieces
        // must both be inside the scan range.
        let mut tinted = std::collections::HashSet::new();
        for line in lines.iter().take(b + 1).skip(t) {
            for (i, &c) in line.iter().enumerate() {
                if (is_lattice_glyph(c) && (o..=close).contains(&i))
                    || (is_delim_piece(c) && (i == o || i == close))
                {
                    tinted.insert(c);
                }
            }
        }
        let _ = bg;
        assert!(
            tinted.iter().any(|c| "⎛⎜⎝(".contains(*c)),
            "left paren column tinted: {:?}",
            tinted
        );
        assert!(
            tinted.iter().any(|c| "⎞⎟⎠)".contains(*c)),
            "right paren column tinted: {:?}",
            tinted
        );
    }

    /// A bare array that merely sits INSIDE a delimiter (not fused —
    /// it has siblings) keeps the frame recolor to its own lattice:
    /// the enclosing parens stay untinted.
    #[test]
    fn unfused_frame_leaves_the_outer_delimiter_alone() {
        use mascii::ast::{Field, Node};
        use mascii::symbols::Delim as D;
        let mut ed = Editor::new();
        ed.root = vec![Node::Delim {
            left: D::Paren,
            right: D::Paren,
            mids: 0,
            segs: vec![vec![
                Node::Array {
                    rows: 2,
                    cols: 2,
                    cells: vec![vec![Node::Sym('x')], vec![], vec![], vec![]],
                },
                Node::Sym('f'),
            ]],
        }];
        ed.path = vec![(0, Field::Seg(0)), (0, Field::Cell(0))];
        ed.col = 0;
        ed.input(Key::Char('o'), false, true); // ^O
        let (root, cursor) = ed.decorated();
        let cursor_ref = cursor.as_ref().map(|(p, c)| (p.as_slice(), *c));
        let block = render_root(&root, cursor_ref, &RenderCtx { italic: true });
        let (lines, _, _, frame) = marker_boxes(
            &block.lines,
            &ed.marker_extents(),
            &block.marks,
            block.caret,
            None,
            None,
        );
        let (o, close, t, b) = frame.expect("frame rect");
        let mut tinted = std::collections::HashSet::new();
        for line in lines.iter().take(b + 1).skip(t) {
            for (i, &c) in line.iter().enumerate() {
                if (is_lattice_glyph(c) && (o..=close).contains(&i))
                    || (is_delim_piece(c) && (i == o || i == close))
                {
                    tinted.insert(c);
                }
            }
        }
        assert!(
            tinted.iter().any(|c| "┌┼┬├".contains(*c)),
            "array lattice tinted: {:?}",
            tinted
        );
        assert!(
            !tinted.iter().any(|c| "⎛⎜⎝⎞⎟⎠()".contains(*c)),
            "outer parens must stay untinted: {:?}",
            tinted
        );
    }

    /// The exact corner-based frame: a matrix nested inside a cell
    /// keeps its own delimiters untinted while the outer frame colors.
    #[test]
    fn frame_rect_is_exact() {
        let mut ed = Editor::new();
        for c in r"\bmatrix ".chars() {
            ed.input(Key::Char(c), false, false);
        }
        for c in r"\pmatrix x".chars() {
            ed.input(Key::Char(c), false, false);
        }
        ed.input(Key::Char('o'), false, true); // grid mode on the INNER grid
        ed.input(Key::Esc, false, false);
        ed.input(Key::Tab, false, false); // out of inner seg…
        ed.input(Key::Tab, false, false); // …back into the outer cell
        ed.input(Key::Char('o'), false, true); // grid mode on the OUTER grid
        let (root, cursor) = ed.decorated();
        let cursor_ref = cursor.as_ref().map(|(p, c)| (p.as_slice(), *c));
        let block = render_root(&root, cursor_ref, &RenderCtx { italic: true });
        let (lines, _, _, frame) = marker_boxes(
            &block.lines,
            &ed.marker_extents(),
            &block.marks,
            block.caret,
            None,
            None,
        );
        let (o, close, t, b) = frame.expect("outer frame rect");
        let mut tinted = std::collections::HashSet::new();
        for line in lines.iter().take(b + 1).skip(t) {
            for (i, &c) in line.iter().enumerate() {
                if (is_lattice_glyph(c) && (o..=close).contains(&i))
                    || (is_delim_piece(c) && (i == o || i == close))
                {
                    tinted.insert(c);
                }
            }
        }
        assert!(
            !tinted.iter().any(|c| "⎛⎜⎝⎞⎟⎠()".contains(*c)),
            "nested parens stay untinted: {:?}",
            tinted
        );
    }

    /// Mouse clicks stay true while the gap ghost (a width-bearing
    /// decoration) is up: the probe render includes the ghost lane.
    #[test]
    fn click_lands_true_while_the_gap_ghost_is_up() {
        let mut ed = Editor::new();
        for c in r"\bmatrix x".chars() {
            ed.input(Key::Char(c), false, false);
        }
        ed.input(Key::Char('o'), false, true);
        ed.input(Key::Char('|'), false, false);
        ed.input(Key::Left, false, false); // gap 0: ghost left of column 0
        let (root, cursor) = ed.decorated();
        let cursor_ref = cursor.as_ref().map(|(p, c)| (p.as_slice(), *c));
        let block = render_root(&root, cursor_ref, &RenderCtx { italic: true });
        let mut xy = None;
        for (y, line) in block.lines.iter().enumerate() {
            if let Some(x) = line.iter().position(|&c| c == '𝑥') {
                xy = Some((y, x));
            }
        }
        let (y, x) = xy.expect("x visible in the ghosted display");
        ed.click(x, y);
        assert!(
            matches!(ed.path.last(), Some((_, mascii::ast::Field::Cell(0)))),
            "click lands in x's own cell: {:?}",
            ed.path
        );
    }

    /// Typing \alpha shows α as a ghost at the caret; typing \frac
    /// floats the empty template under it — before committing.
    #[test]
    fn minibuffer_preview_shows_the_shape() {
        let mut ed = Editor::new();
        for c in "x\\alpha".chars() {
            ed.input(Key::Char(c), false, false);
        }
        let (root, cursor) = ed.decorated();
        let cursor_ref = cursor.as_ref().map(|(p, c)| (p.as_slice(), *c));
        let block = render_root(&root, cursor_ref, &RenderCtx { italic: true });
        let (mut lines, mut bg, mut cursor_cell, _) = marker_boxes(
            &block.lines,
            &ed.marker_extents(),
            &block.marks,
            block.caret,
            None,
            None,
        );
        let mut struck = std::collections::HashSet::new();
        overlay_minibuffer(&ed, &mut lines, &mut bg, &mut struck, &mut cursor_cell);
        let (cy, _) = cursor_cell.unwrap();
        let below: String = lines[cy + 1..].iter().flatten().collect();
        assert!(below.contains('α'), "symbol previews below: {:?}", below);
        let painted = bg[cy + 1..]
            .iter()
            .flatten()
            .filter(|c| **c == Some(theme::PREVIEW_BG))
            .count();
        assert!(painted > 0, "preview cell painted");
        // Structural command: the template floats below the caret.
        let mut ed = Editor::new();
        for c in "x\\frac".chars() {
            ed.input(Key::Char(c), false, false);
        }
        let (root, cursor) = ed.decorated();
        let cursor_ref = cursor.as_ref().map(|(p, c)| (p.as_slice(), *c));
        let block = render_root(&root, cursor_ref, &RenderCtx { italic: true });
        let (mut lines, mut bg, mut cursor_cell, _) = marker_boxes(
            &block.lines,
            &ed.marker_extents(),
            &block.marks,
            block.caret,
            None,
            None,
        );
        let mut struck = std::collections::HashSet::new();
        overlay_minibuffer(&ed, &mut lines, &mut bg, &mut struck, &mut cursor_cell);
        let below: String = lines[1..].iter().flatten().collect();
        assert!(
            below.contains('─'),
            "fraction bar in the preview: {:?}",
            below
        );
        assert!(
            below.contains('⬚'),
            "empty slots in the preview: {:?}",
            below
        );
        let previewed = bg[1..]
            .iter()
            .flatten()
            .filter(|c| **c == Some(theme::PREVIEW_BG))
            .count();
        assert!(previewed > 0, "preview box painted");
    }

    /// A preview opened INSIDE a structure shifts the rows below it
    /// instead of painting over them: the fraction bar survives.
    #[test]
    fn preview_shifts_rows_instead_of_hiding_them() {
        let mut ed = Editor::new();
        // 1/(2) with the cursor back in the numerator, then \alpha.
        for c in "1/2".chars() {
            ed.input(Key::Char(c), false, false);
        }
        ed.input(Key::Up, false, false); // into the numerator
        let bars_before = {
            let (root, cursor) = ed.decorated();
            let cursor_ref = cursor.as_ref().map(|(p, c)| (p.as_slice(), *c));
            let block = render_root(&root, cursor_ref, &RenderCtx { italic: true });
            block.lines.iter().filter(|l| l.contains(&'─')).count()
        };
        for c in "\\alpha".chars() {
            ed.input(Key::Char(c), false, false);
        }
        let (root, cursor) = ed.decorated();
        let cursor_ref = cursor.as_ref().map(|(p, c)| (p.as_slice(), *c));
        let block = render_root(&root, cursor_ref, &RenderCtx { italic: true });
        let (mut lines, mut bg, mut cursor_cell, _) = marker_boxes(
            &block.lines,
            &ed.marker_extents(),
            &block.marks,
            block.caret,
            None,
            None,
        );
        let mut struck = std::collections::HashSet::new();
        overlay_minibuffer(&ed, &mut lines, &mut bg, &mut struck, &mut cursor_cell);
        // The α preview is present AND every fraction bar still is.
        let all: String = lines.iter().flatten().collect();
        assert!(all.contains('α'), "{}", all);
        let bars_after = lines.iter().filter(|l| l.contains(&'─')).count();
        assert_eq!(bars_after, bars_before, "the bar was not painted over");
    }

    /// The name box never shows the reparse-quoting quotes: typing a
    /// single letter into \rm displays that letter, bare, with the
    /// caret at the box cursor.
    #[test]
    fn op_box_overlay_is_quote_free() {
        let mut ed = Editor::new();
        for c in "\\rm a".chars() {
            ed.input(Key::Char(c), false, false);
        }
        let (root, cursor) = ed.decorated();
        let cursor_ref = cursor.as_ref().map(|(p, c)| (p.as_slice(), *c));
        let block = render_root(&root, cursor_ref, &RenderCtx { italic: true });
        let (mut lines, mut bg, mut cursor_cell, _) = marker_boxes(
            &block.lines,
            &ed.marker_extents(),
            &block.marks,
            block.caret,
            None,
            None,
        );
        let mut struck = std::collections::HashSet::new();
        overlay_minibuffer(&ed, &mut lines, &mut bg, &mut struck, &mut cursor_cell);
        let all: String = lines.iter().flatten().collect();
        assert!(all.contains('a'), "{}", all);
        assert!(!all.contains('\''), "no quote artifacts: {}", all);
        // ← moves the display caret inside the box.
        let (cy, cx) = cursor_cell.unwrap();
        ed.input(Key::Left, false, false);
        let (root, cursor) = ed.decorated();
        let cursor_ref = cursor.as_ref().map(|(p, c)| (p.as_slice(), *c));
        let block = render_root(&root, cursor_ref, &RenderCtx { italic: true });
        let (mut lines, mut bg, mut cursor_cell2, _) = marker_boxes(
            &block.lines,
            &ed.marker_extents(),
            &block.marks,
            block.caret,
            None,
            None,
        );
        overlay_minibuffer(&ed, &mut lines, &mut bg, &mut struck, &mut cursor_cell2);
        let (cy2, cx2) = cursor_cell2.unwrap();
        assert_eq!(cy2, cy);
        assert_eq!(cx2 + 1, cx, "caret stepped left inside the box");
    }

    #[test]
    fn minibuffer_overlays_at_the_cursor_without_layout_shift() {
        let mut ed = Editor::new();
        type_script_keys(&mut ed, "a+b");
        ed.input(Key::Left, false, false);
        ed.input(Key::Left, false, false);
        let before = display(&ed);
        ed.input(Key::Char('\\'), false, false);
        type_script_keys(&mut ed, "fr");
        let (root, cursor) = ed.decorated();
        let cursor_ref = cursor.as_ref().map(|(p, c)| (p.as_slice(), *c));
        let block = render_root(&root, cursor_ref, &RenderCtx { italic: true });
        let (mut lines, mut bg, mut cursor_cell, _) = marker_boxes(
            &block.lines,
            &ed.marker_extents(),
            &block.marks,
            block.caret,
            None,
            None,
        );
        let (cy, cx) = cursor_cell.unwrap();
        let mut struck = std::collections::HashSet::new();
        overlay_minibuffer(&ed, &mut lines, &mut bg, &mut struck, &mut cursor_cell);
        // The overlay covers the glyphs to the right of the cursor in
        // place: same height, and the cells left of the cursor are
        // untouched.
        assert_eq!(lines.len(), before.len(), "no vertical shift");
        let text: String = lines[cy][cx..cx + 3].iter().collect();
        assert_eq!(text, "\\fr", "typed command shown at the cursor");
        let kept: String = lines[cy][..cx].iter().collect();
        assert_eq!(kept, before[cy].chars().take(cx).collect::<String>());
        assert_eq!(cursor_cell, Some((cy, cx + 3)), "caret after the text");
        assert!(bg[cy][cx].is_some(), "overlay cells are tinted");
    }

    #[test]
    fn struck_cells_survive_cursor_decoration() {
        // A cancel next to the cursor: cell indexing must not shift and
        // the base+U+0338 ligature must stay inside one span (the bug:
        // to_strings() embedded the strike, so char index != column and
        // the caret split the ligature — the glyph vanished).
        let mut ed = Editor::new();
        type_script_keys(&mut ed, "ab");
        ed.input(Key::Left, true, false); // select b
        ed.input(Key::Char('\\'), false, false);
        for c in "cancel".chars() {
            ed.input(Key::Char(c), false, false);
        }
        ed.input(Key::Enter, false, false);
        // Cursor sits right of the struck b; walk it across the strike.
        for _ in 0..3 {
            let (root, cursor) = ed.decorated();
            let cursor_ref = cursor.as_ref().map(|(p, c)| (p.as_slice(), *c));
            let block = render_root(&root, cursor_ref, &RenderCtx { italic: true });
            let struck: std::collections::HashSet<(usize, usize)> =
                block.cancel.iter().copied().collect();
            assert!(!struck.is_empty(), "cancel present");
            let (lines, bg, cursor_cell, _) = marker_boxes(
                &block.lines,
                &ed.marker_extents(),
                &block.marks,
                block.caret,
                None,
                None,
            );
            let (y, x) = cursor_cell.expect("caret visible");
            let spans = decorate_line(
                &lines[y],
                y,
                &struck,
                &bg[y],
                Some(x),
                false,
                false,
                0,
                None,
            );
            let joined: String = spans.iter().map(|s| s.content.as_ref()).collect();
            // Every struck glyph is still there, followed by its strike.
            for &(r, c) in &block.cancel {
                if r == y {
                    let cell: Vec<char> = joined.chars().collect();
                    // find the base char count up to cell c: strikes are
                    // combining, so count non-combining chars.
                    let mut col = 0usize;
                    let mut ok = false;
                    let mut it = cell.iter().peekable();
                    while let Some(&ch) = it.next() {
                        if ch == '\u{338}' {
                            continue;
                        }
                        if col == c {
                            ok = it.peek() == Some(&&'\u{338}');
                            break;
                        }
                        col += 1;
                    }
                    assert!(ok, "strike stays glued at col {}: {:?}", c, joined);
                }
            }
            ed.input(Key::Left, false, false);
        }
    }

    fn type_script_keys(ed: &mut Editor, s: &str) {
        for c in s.chars() {
            ed.input(Key::Char(c), false, false);
        }
    }

    #[test]
    fn selection_box_paints_without_touching_the_text() {
        let lines: Vec<Vec<char>> = vec![" ab ".chars().collect()];
        let marks = [(0, 1, SEL_OPEN), (0, 3, SEL_CLOSE)];
        let (out, bg, _, _) = marker_boxes(&lines, &[(0, 0, 0)], &marks, None, None, None);
        assert_eq!(out, lines, "text must be untouched");
        assert_eq!(
            bg[0],
            vec![
                None,
                Some(theme::SELECTION_BG),
                Some(theme::SELECTION_BG),
                None
            ]
        );
    }

    #[test]
    fn selection_box_covers_the_block_extent_only() {
        // A selected fraction: rows from the extent, not from content
        // scanning — the denominator row below the box stays unpainted
        // when the extent says so.
        let lines: Vec<Vec<char>> = vec![
            " 1 ".chars().collect(),
            "───".chars().collect(),
            " 2 ".chars().collect(),
        ];
        let marks = [(1, 0, SEL_OPEN), (1, 3, SEL_CLOSE)];
        let (_, bg, _, _) = marker_boxes(&lines, &[(1, 1, 0)], &marks, None, None, None);
        assert!(bg.iter().all(|row| row.iter().all(|c| c.is_some())));
        let (_, bg, _, _) = marker_boxes(&lines, &[(0, 0, 0)], &marks, None, None, None);
        assert!(
            bg[2].iter().all(|c| c.is_none()),
            "extent must bound the box"
        );
    }

    #[test]
    fn labels_overlay_and_caret_pads() {
        let label = char::from_u32(JUMP_CHAR_BASE).unwrap();
        let lines: Vec<Vec<char>> = vec!["xy".chars().collect()];
        let (out, _, cursor, _) =
            marker_boxes(&lines, &[], &[(0, 0, label)], Some((0, 2)), None, None);
        let chars: Vec<char> = out[0].clone();
        assert_eq!(chars[0], label, "label over the glyph");
        assert_eq!(chars[1], 'y');
        assert_eq!(cursor, Some((0, 2)));
        assert_eq!(chars.len(), 3, "caret cell padded at the row end");
    }

    #[test]
    fn caret_display_never_shifts_the_layout() {
        // (x^a, not x^2: an inlinable script like ² must re-expand to 2D
        // while the caret is inside it — that shift is inherent.)
        let mut ed = Editor::new();
        for k in "x^a".chars() {
            ed.input(Key::Char(k), false, false);
        }
        ed.input(Key::Tab, false, false);
        ed.input(Key::Char('+'), false, false);
        ed.input(Key::Char('/'), false, false);
        ed.input(Key::Char('/'), false, false);
        ed.input(Key::Char('1'), false, false);
        ed.input(Key::Down, false, false);
        ed.input(Key::Char('2'), false, false);
        let ctx = RenderCtx { italic: true };
        let plain: Vec<String> = render_root(&ed.root, None, &ctx)
            .to_strings()
            .iter()
            .map(|l| l.trim_end().to_string())
            .collect();
        // Every cursor position must display with the same geometry as
        // the cursor-less render (the caret is an overlay, not a column).
        for cand in ed.jump_candidates() {
            if cand.is_cursor {
                continue;
            }
            let (p, c) = cand.pos;
            ed.path = p;
            ed.col = c;
            let got: Vec<String> = display(&ed)
                .iter()
                .map(|l| l.trim_end().to_string())
                .collect();
            assert_eq!(
                got, plain,
                "layout shifted at path {:?} col {}",
                ed.path, ed.col
            );
        }
    }

    #[test]
    fn ghost_slots_keep_jump_reentry_stable() {
        // \sum then exit: the empty band shows as a bare ∑. The first
        // ^G materializes the limit slots (a necessary shift); after
        // cancelling, the slots stay as ghost ⬚ until the next input,
        // so the second ^G overlays the identical picture.
        let mut ed = Editor::new();
        ed.input(Key::Char('\\'), false, false);
        for k in "sum".chars() {
            ed.input(Key::Char(k), false, false);
        }
        ed.input(Key::Enter, false, false);
        ed.input(Key::Tab, false, false);
        let plain = display(&ed);
        ed.input(Key::Char('g'), false, true);
        let jump1 = display(&ed);
        assert_ne!(plain, jump1, "slots must materialize under ^G");
        ed.input(Key::Esc, false, false); // cancel: slots become ghosts
        let ghost = display(&ed);
        assert!(
            ghost.concat().contains('⬚'),
            "ghost slots stay visible:\n{}",
            ghost.join("\n")
        );
        ed.input(Key::Char('g'), false, true);
        let jump2 = display(&ed);
        assert_eq!(jump1, jump2, "re-entering ^G must not shift");
        assert_eq!(jump2.len(), ghost.len());
        for (m, e) in jump2.iter().zip(&ghost) {
            let (m, e): (Vec<char>, Vec<char>) = (m.chars().collect(), e.chars().collect());
            for x in 0..m.len().max(e.len()) {
                let (mc, ec) = (
                    m.get(x).copied().unwrap_or(' '),
                    e.get(x).copied().unwrap_or(' '),
                );
                assert!(
                    mc == ec || (0xE000..0xE100).contains(&(mc as u32)),
                    "ghost/jump mismatch at {}: {:?} vs {:?}",
                    x,
                    mc,
                    ec
                );
            }
        }
        // Any real input clears the ghosts.
        ed.input(Key::Esc, false, false);
        ed.input(Key::Char('x'), false, false);
        assert!(!display(&ed).concat().contains('⬚'));
    }

    #[test]
    fn mode_displays_keep_the_editing_geometry() {
        // Formulas whose editing view differs from the plain one: an
        // empty-limit ∑ band, a fused matrix, an inline superscript.
        // Entering ^G / ^B must keep the geometry — cells may only
        // change where a label got overlaid.
        for keys in [
            vec!["\\", "s", "u", "m", "\n", "x"],
            vec!["\\", "p", "m", "a", "t", "r", "i", "x", "\n", "a", ">", "b"],
            vec!["x", "^", "2"],
        ] {
            let mut ed = Editor::new();
            for k in keys {
                match k {
                    "\n" => ed.input(Key::Enter, false, false),
                    ">" => ed.input(Key::Right, false, false),
                    k => ed.input(Key::Char(k.chars().next().unwrap()), false, false),
                };
            }
            let editing = display(&ed);
            for key in ['g', 'b'] {
                ed.input(Key::Char(key), false, true);
                let mode = display(&ed);
                assert_eq!(mode.len(), editing.len(), "height changed in ^{}", key);
                for (y, (m, e)) in mode.iter().zip(&editing).enumerate() {
                    let (m, e): (Vec<char>, Vec<char>) = (m.chars().collect(), e.chars().collect());
                    for x in 0..m.len().max(e.len()) {
                        let (mc, ec) = (
                            m.get(x).copied().unwrap_or(' '),
                            e.get(x).copied().unwrap_or(' '),
                        );
                        let label = (0xE000..0xE100).contains(&(mc as u32));
                        assert!(
                            mc == ec || label,
                            "^{} shifted cell ({}, {}): {:?} vs {:?}",
                            key,
                            y,
                            x,
                            mc,
                            ec
                        );
                    }
                }
                ed.input(Key::Esc, false, false);
            }
        }
    }
}
