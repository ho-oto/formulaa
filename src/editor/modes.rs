//! Interactive modes layered over the structural editor: the free
//! cursor (^F), grid editing (^G) and the display decoration they
//! paint through. None of these change the formula — they move the
//! cursor or hand a selection to the editor — which is why they live
//! apart from the editing operations.

use super::*;

impl Editor {
    // ----- free-cursor mode (^F) -----

    pub fn start_free(&mut self) {
        self.select_anchor = None;
        let start = self
            .nearest_position_of_cursor()
            .unwrap_or(((self.path.clone(), self.col), (0, 0)));
        self.free = Some(FreeCursor {
            at: start.1,
            snap: start.0,
            snap_at: start.1,
        });
        self.clear_message();
    }

    /// The cursor's own display cell (in the display frame).
    fn nearest_position_of_cursor(&self) -> Option<(CursorPos, (usize, usize))> {
        let cands = self.jump_candidates();
        let coords = self.coords_displayed(&cands);
        let i = cands.iter().position(|c| c.is_cursor)?;
        coords[i].map(|xy| (cands[i].pos.clone(), xy))
    }

    pub fn free_move(&mut self, dx: i32, dy: i32) {
        use crate::render::{RenderCtx, render_root};
        let Some(f) = &self.free else {
            return;
        };
        // (Clamping happens after the re-anchor step, against the
        // ghost-materialized display frame.)
        let at = (
            f.at.0.saturating_add_signed(dy as isize),
            f.at.1.saturating_add_signed(dx as isize),
        );
        // Collapsed elements (inline scripts, invisible slots) auto-
        // expand while the free cursor is near their always-visible
        // anchor, with hysteresis (FREE_EXPAND_IN/OUT) so the expansion
        // shift cannot make the test oscillate.
        let cands = self.jump_candidates();
        let disp = self.coords_displayed(&cands);
        let coord_of = |pos: &CursorPos| {
            cands
                .iter()
                .position(|c| &c.pos == pos)
                .and_then(|i| disp[i])
        };
        let mut ghosts: Vec<Vec<(usize, Field)>> = Vec::new();
        for cand in &cands {
            let expandable = matches!(
                cand.pos.0.last(),
                Some((
                    _,
                    Field::SupArg
                        | Field::SubArg
                        | Field::OpLower
                        | Field::OpUpper
                        | Field::ArrowOver
                        | Field::ArrowUnder
                        | Field::BraceLabel
                ))
            );
            if !expandable || ghosts.contains(&cand.pos.0) {
                continue;
            }
            let row_path = &cand.pos.0;
            let anchor = (
                row_path[..row_path.len() - 1].to_vec(),
                row_path[row_path.len() - 1].0,
            );
            let Some((ay, ax)) = coord_of(&anchor) else {
                continue;
            };
            let d = JUMP_W_Y * ay.abs_diff(at.0) + ax.abs_diff(at.1);
            let was = self.ghost.contains(row_path);
            if d <= FREE_EXPAND_IN || (was && d <= FREE_EXPAND_OUT) {
                ghosts.push(row_path.clone());
            }
        }
        self.ghost = ghosts;
        // Expansion can shift the whole canvas (a band adds a row above
        // the baseline). Re-anchor the free cursor by however much the
        // previous snap target moved between the two frames, so the
        // cell cursor stays glued to the content instead of floating.
        let mut at = at;
        if let Some(prev) = &self.free {
            let cands2 = self.jump_candidates();
            let disp2 = self.coords_displayed(&cands2);
            if let Some((ny, nx)) = cands2
                .iter()
                .position(|c| c.pos == prev.snap)
                .and_then(|i| disp2[i])
            {
                at.0 =
                    at.0.saturating_add_signed(ny as isize - prev.snap_at.0 as isize);
                at.1 =
                    at.1.saturating_add_signed(nx as isize - prev.snap_at.1 as isize);
            }
        }
        // Clamp to the display frame (ghosts included).
        let (droot, _) = self.decorated();
        let b = render_root(&droot, None, &RenderCtx::canonical());
        let (h, w) = (b.height().max(1), b.width().max(1));
        at.0 = at.0.min(h - 1);
        at.1 = at.1.min(w);
        if let Some((snap, snap_at)) = self.nearest_position(at.1, at.0) {
            if let Some(f) = &mut self.free {
                f.at = at;
                f.snap = snap;
                f.snap_at = snap_at;
            }
        } else if let Some(f) = &mut self.free {
            f.at = at;
        }
    }

    /// Enter: land on the snap target.
    pub fn free_confirm(&mut self) {
        if let Some(f) = self.free.take() {
            self.path = f.snap.0;
            self.col = f.snap.1;
            self.clear_message();
        }
    }

    pub fn free_cancel(&mut self) {
        self.free = None;
        self.select_anchor = None;
        self.ghost.clear();
        self.clear_message();
    }

    pub fn document_end(&mut self) {
        self.select_anchor = None;
        self.path.clear();
        self.col = self.root.len();
    }

    pub fn document_start(&mut self) {
        self.select_anchor = None;
        self.path.clear();
        self.col = 0;
    }

    /// Enter at the top level: start a new formula line (Node::Break).
    pub fn break_line(&mut self) {
        // Enter is a content insert too: it replaces an active
        // selection with the line break.
        if self.selection().is_some() {
            self.take_selection();
        } else {
            self.select_anchor = None;
        }
        let col = self.col;
        self.cur_row_mut().insert(col, Node::Break);
        self.col += 1;
    }

    /// Insert a structure node and move the cursor into its first field.
    pub fn insert_and_enter(&mut self, node: Node) {
        let first = node.fields()[0];
        let col = self.col;
        self.cur_row_mut().insert(col, node);
        self.path.push((col, first));
        self.col = 0;
    }

    // ----- cursor-position enumeration (free-cursor snapping) -----

    /// Every cursor position in true document order (column, then that
    /// node's children, then the next column), with per-position flags.
    /// Document order is required by the coordinate probe (marker
    /// insertion invalidates later positions otherwise). The free
    /// cursor snaps and auto-expands against this enumeration.
    pub fn jump_candidates(&self) -> Vec<JumpCand> {
        fn flat_atom(n: &Node) -> bool {
            matches!(n, Node::Sym(c) if *c != '␣')
        }
        fn walk(
            row: &Row,
            path: &mut Vec<(usize, Field)>,
            in_cell: bool,
            cursor: &(&[(usize, Field)], usize),
            out: &mut Vec<JumpCand>,
        ) {
            fn is_cur(
                cursor: &(&[(usize, Field)], usize),
                path: &[(usize, Field)],
                col: usize,
            ) -> bool {
                cursor.0 == path && cursor.1 == col
            }
            if row.is_empty() {
                out.push(JumpCand {
                    pos: (path.clone(), 0),
                    empty: true,
                    cell_end: in_cell,
                    interior: false,
                    bound: true,
                    is_cursor: is_cur(cursor, path, 0),
                });
                return;
            }
            for (i, node) in row.iter().enumerate() {
                out.push(JumpCand {
                    pos: (path.clone(), i),
                    empty: false,
                    cell_end: false,
                    interior: i > 0 && flat_atom(&row[i - 1]) && flat_atom(node),
                    bound: i == 0,
                    is_cursor: is_cur(cursor, path, i),
                });
                for f in node.fields() {
                    path.push((i, f));
                    walk(
                        node.field(f),
                        path,
                        matches!(f, Field::Cell(_)),
                        cursor,
                        out,
                    );
                    path.pop();
                }
            }
            out.push(JumpCand {
                pos: (path.clone(), row.len()),
                empty: false,
                cell_end: in_cell,
                interior: false,
                bound: true,
                is_cursor: is_cur(cursor, path, row.len()),
            });
        }
        let mut out = Vec::new();
        walk(
            &self.root,
            &mut Vec::new(),
            false,
            &(&self.path[..], self.col),
            &mut out,
        );
        out
    }

    // ----- block-select mode (Ctrl+B: the cursor's ancestor chain) -----

    /// The cursor's enclosing structure nodes, innermost first: one
    /// (parent row path, node index) per ancestor.
    pub fn block_targets(&self) -> Vec<BlockRef> {
        // Walking out alternates between a slot taken whole — the
        // numerator, the limit, the cell the cursor stands in — and the
        // structure that owns it. The slot is the one range no node
        // names, and the outermost step is the root row for the same
        // reason. Two consecutive steps can land on the same box (a
        // slot holding one node, seen from inside that node), and an
        // empty slot has nothing to offer: neither becomes a target.
        fn push(out: &mut Vec<BlockRef>, t: BlockRef) {
            if !t.1.is_empty() && out.last() != Some(&t) {
                out.push(t);
            }
        }
        let mut targets: Vec<BlockRef> = Vec::new();
        for k in (0..self.path.len()).rev() {
            let slot = row_at(&self.root, &self.path[..k + 1]).len();
            push(&mut targets, (self.path[..k + 1].to_vec(), 0..slot));
            let i = self.path[k].0;
            push(&mut targets, (self.path[..k].to_vec(), i..i + 1));
        }
        // Walking ↑ always ends on "select everything", and ^B at the
        // top level starts there.
        push(&mut targets, (Vec::new(), 0..self.root.len()));
        targets
    }

    pub fn start_block_select(&mut self) {
        if self.block.is_some() {
            // ^B again: back to where the cursor was (it never moved).
            self.block_cancel();
            return;
        }
        let targets = self.block_targets();
        if targets.is_empty() {
            // An empty formula has nothing to select; the help line
            // explains the mode, so silence is enough here.
            return;
        }
        self.block_sel = 0;
        self.block = Some(targets);
    }

    /// The ancestor ranks ^B paints: the highlighted one and its
    /// immediate neighbours (the innermost and outermost ends have one
    /// neighbour each). Ranks stay absolute, so the display can still
    /// tell which box is the selected one.
    pub fn block_shown(&self) -> Vec<usize> {
        let len = self.block.as_ref().map_or(0, Vec::len);
        let sel = self.block_sel;
        // The selection and the one step *outward*. The inner step is
        // not shown: selection starts at the innermost ancestor, so
        // "one step in" is always where you just came from — the
        // outer ring is the only step that needs announcing.
        (sel..=sel + 1).filter(|&r| r < len).collect()
    }

    pub fn block_cancel(&mut self) {
        self.block = None;
        self.clear_message();
    }

    /// Move the highlighted ancestor outward (↑/→) or inward (↓/←).
    pub fn block_move(&mut self, outward: bool) {
        let len = self.block.as_ref().map_or(0, Vec::len);
        if outward {
            self.block_sel = (self.block_sel + 1).min(len.saturating_sub(1));
        } else {
            self.block_sel = self.block_sel.saturating_sub(1);
        }
    }

    /// Select the highlighted ancestor: the whole node becomes the
    /// selection, ready for ^C/^X, wrapping or deletion.
    pub fn block_commit(&mut self) {
        let sel = self.block_sel;
        if let Some(targets) = self.block.take()
            && let Some((p, r)) = targets.get(sel)
        {
            self.path = p.clone();
            self.select_anchor = Some(r.start);
            self.select_path = p.clone();
            self.col = r.end;
            self.select_whole = true;
        }
        self.clear_message();
    }

    /// Vertical extents (rows above / below the marker's baseline row)
    /// of the boxes the display should paint: one entry for the active
    /// selection, one per ^B target in ascending-open-column
    /// (= document) order, or one per painted grid cell. Computed by
    /// laying out the covered slice — the char grid alone cannot tell
    /// a block's rows apart from other content (e.g. a denominator
    /// centered under the same columns).
    pub fn marker_extents(&self) -> Vec<(usize, usize, usize)> {
        use crate::render::{RenderCtx, render_root};
        let extent =
            |slice: &[Node], cursor: Option<(Vec<(usize, Field)>, usize)>, depth: usize| {
                let cur = cursor.as_ref().map(|(p, c)| (p.as_slice(), *c));
                let b = render_root(&slice.to_vec(), cur, &RenderCtx::canonical());
                let (h, bl) = (b.height(), b.baseline);
                (bl.min(h), h.saturating_sub(bl + 1), depth)
            };
        if let Some(targets) = &self.block {
            targets
                .iter()
                .enumerate()
                .map(|(rank, (p, r))| {
                    // An Array fused into its delimiter has no isolated
                    // layout of its own: measure the parent Delim slice
                    // instead (the fused interior spans its full height).
                    let (p, r) = if r.len() == 1 && self.fused_in_delim(p, r.start) {
                        let i = p.last().unwrap().0;
                        (&p[..p.len() - 1], i..i + 1)
                    } else {
                        (&p[..], r.clone())
                    };
                    // If the cursor is inside this block, lay the slice
                    // out in its editing view (matches the display).
                    let cur = (self.path.len() > p.len()
                        && self.path[..p.len()] == p[..]
                        && r.contains(&self.path[p.len()].0))
                    .then(|| {
                        let mut rel = self.path[p.len()..].to_vec();
                        rel[0].0 -= r.start;
                        (rel, self.col)
                    });
                    // Ranked by ancestry (innermost first); the display
                    // indexes this list by rank, so every target keeps an
                    // entry whether or not it is painted.
                    extent(&row_at(&self.root, p)[r], cur, rank)
                })
                .collect()
        } else if let Some(gs) = self.grid
            && let Some((k, i, rows, cols, c)) = self.grid_info()
        {
            // One extent per painted cell, in row-major (= mark
            // encounter) order — marker_boxes pairs them up in kind.
            let Node::Array { cells, .. } = &row_at(&self.root, &self.path[..k])[i] else {
                unreachable!()
            };
            let _ = (gs, rows, c);
            let sel: Vec<usize> = self
                .grid_rect()
                .map(|(r0, j0, r1, j1)| {
                    (r0..=r1)
                        .flat_map(|r| (j0..=j1).map(move |j| r * cols + j))
                        .collect()
                })
                .unwrap_or_default();
            let _ = i;
            sel.iter()
                .map(|&cell| extent(&cells[cell], None, 0))
                .collect()
        } else if let Some((lo, hi)) = self.selection() {
            vec![extent(&self.cur_row()[lo..hi], None, 0)]
        } else {
            Vec::new()
        }
    }

    /// Is the node at (p, i) an Array that fuses with its enclosing
    /// delimiter (sole node of a Seg of a fusing ( [ ⌈ ⌊ | pair)?
    fn fused_in_delim(&self, p: &[(usize, Field)], i: usize) -> bool {
        let Some(&(pi, Field::Seg(_))) = p.last() else {
            return false;
        };
        let row = row_at(&self.root, p);
        if i != 0 || row.len() != 1 || !matches!(row[0], Node::Array { .. }) {
            return false;
        }
        match &row_at(&self.root, &p[..p.len() - 1])[pi] {
            Node::Delim {
                left, right, mids, ..
            } => *mids == 0 && left.fuses() && right.fuses(),
            _ => false,
        }
    }

    // ----- display decoration (selection / grid) -----

    /// A copy of the AST with display markers inserted, plus the
    /// cursor adjusted for those insertions. Markers are private-use
    /// `Sym`s the display turns into labels and colored boxes; they
    /// never appear in a real document. One branch per mode — they
    /// share only `bump`, which threads a position past an insertion.
    pub fn decorated(&self) -> (Row, Option<CursorPos>) {
        if self.block.is_some() {
            self.decorate_block()
        } else if self.grid.is_some() {
            self.decorate_grid()
        } else {
            self.decorate_plain()
        }
    }

    /// ^B: a rank mark left of each ancestor block and a close marker
    /// right of it (the display paints depth-shaded boxes; no letters).
    fn decorate_block(&self) -> (Row, Option<CursorPos>) {
        let mut root = self.root.clone();
        let targets = self.block.as_ref().expect("block mode");

        let mut path = self.path.clone();
        let mut col = self.col;
        let shown = self.block_shown();
        // The open mark sits immediately left of its block and a close
        // marker right after it, so the display can paint its extent.
        // Marks that land in the SAME row (the whole-formula target
        // shares the root row with the outermost ancestor) are
        // inserted right-to-left, so no insertion shifts a later
        // position; at a shared position, an outer close goes in
        // before an inner one (ending up to its right) and an inner
        // open before an outer one (ending up inside it).
        let mut rows: Vec<&Vec<(usize, Field)>> = Vec::new();
        let mut events: Vec<Vec<(usize, bool, usize)>> = Vec::new();
        for (idx, (p, r)) in targets.iter().enumerate() {
            if !shown.contains(&idx) {
                continue;
            }
            let at = match rows.iter().position(|q| *q == p) {
                Some(at) => at,
                None => {
                    rows.push(p);
                    events.push(Vec::new());
                    rows.len() - 1
                }
            };
            events[at].push((r.end, false, idx));
            events[at].push((r.start, true, idx));
        }
        for (p, mut evs) in rows.into_iter().zip(events) {
            evs.sort_by(|a, b| {
                b.0.cmp(&a.0).then_with(|| match (a.1, b.1) {
                    (false, false) => b.2.cmp(&a.2),
                    (true, true) => a.2.cmp(&b.2),
                    (false, true) => std::cmp::Ordering::Less,
                    (true, false) => std::cmp::Ordering::Greater,
                })
            });
            for (pos, open, rank) in evs {
                let mark = if open {
                    Mark::BlockOpen { rank }.ch()
                } else {
                    Mark::BlockClose.ch()
                };
                row_at_mut(&mut root, p).insert(pos, Node::Sym(mark));
                bump(&mut path, &mut col, p, pos);
            }
        }
        (root, Some((path, col)))
    }

    /// ^G: the frame corners, the cell/lane selection pair, and the gap ghost (the one decoration with real width).
    fn decorate_grid(&self) -> (Row, Option<CursorPos>) {
        let mut root = self.root.clone();
        let gs = self.grid.expect("grid mode");
        let Some((k, i, _rows, cols, c)) = self.grid_info() else {
            return self.decorate_plain();
        };
        let mut path = self.path.clone();
        let mut col = self.col;
        let parent_path = self.path[..k].to_vec();
        // The frame pair around the Array node lets the display
        // recolor its lattice — the "grid mode is on" signal. The
        // render resolves the pair to the framed block's corners
        // (fused or not), so one pair serves every layout.
        {
            let prow = row_at_mut(&mut root, &parent_path);
            prow.insert(i + 1, Node::Sym(Mark::Frame { open: false }.ch()));
            prow.insert(i, Node::Sym(Mark::Frame { open: true }.ch()));
        }
        if path.len() > k {
            path[k].0 += 1;
        }
        let i = i + 1;
        let Node::Array {
            rows: nr,
            cols: nc,
            cells,
        } = &mut row_at_mut(&mut root, &parent_path)[i]
        else {
            unreachable!()
        };
        // The painted cells: the cell rectangle, or the full-axis
        // rectangle of the selected lane(s) — one source of truth
        // (`grid_rect`) shared with the editing ops.
        let gap = matches!(gs, GridSel::Lanes { pos, .. } if pos % 2 == 0);
        if !gap {
            let Some((r0, j0, r1, j1)) = self.grid_rect() else {
                return (root, Some((path, col)));
            };
            let (op, cl) = match gs {
                GridSel::Cells { .. } => (Mark::Cells { open: true }, Mark::Cells { open: false }),
                GridSel::Lanes { cols, .. } => (
                    Mark::Lane { open: true, cols },
                    Mark::Lane { open: false, cols },
                ),
            };
            for r in r0..=r1 {
                for j in j0..=j1 {
                    let cell = &mut cells[r * cols + j];
                    let hi = cell.len();
                    cell.insert(hi, Node::Sym(cl.ch()));
                    cell.insert(0, Node::Sym(op.ch()));
                    if r * cols + j == c {
                        // The cursor lives in this cell: the open
                        // mark at 0 shifts it right once.
                        if path.len() > k + 1 {
                            path[k + 1].0 += 1;
                        } else {
                            col += 1;
                        }
                    }
                }
            }
            return (root, Some((path, col)));
        }
        // Gap cursor: a ghost lane previews the insert (a Spacer
        // cell painted by the gap mark). The ghost has real width, so
        // the parked cell's index shifts with it — the splice numbers
        // come from gap_splice, the same source the coordinate probes
        // read, so paint and probe cannot disagree.
        let gsp = self.gap_splice().expect("lane gap state");
        let mark = Mark::Gap { cols: gsp.cmode }.ch();
        (*nr, *nc) = splice_lane(cells, gsp.rows, gsp.cols, gsp.cmode, gsp.g, || {
            vec![Node::Sym(mark), Node::Spacer]
        });
        if let Field::Cell(cell) = path[k].1 {
            let _ = (c, gs);
            path[k].1 = Field::Cell(gap_shift_cell(cell, gsp.cmode, gsp.g, gsp.rows, gsp.cols));
        }
        (root, Some((path, col)))
    }

    /// No mode: the ghost slots the free cursor keeps expanded, plus
    /// the selection.
    fn decorate_plain(&self) -> (Row, Option<CursorPos>) {
        let mut root = self.root.clone();
        let mut path = self.path.clone();
        let mut col = self.col;
        // Deepest rows first: inserting into an ancestor row would shift
        // the node indices a deeper ghost path still needs. The cursor
        // is adjusted in one batch against the original coordinates
        // (incremental nudging breaks on nested ghosts).
        let mut ghosts: Vec<&Vec<(usize, Field)>> = self.ghost.iter().collect();
        ghosts.sort_by_key(|p| std::cmp::Reverse(p.len()));
        for p in ghosts {
            row_at_mut(&mut root, p).insert(0, Node::Sym(Mark::SlotGhost.ch()));
        }
        let adjusted = ghost_adjust(&self.ghost, &path, col);
        path = adjusted.0;
        col = adjusted.1;
        if let Some((lo, hi)) = self.selection() {
            let row = row_at_mut(&mut root, &path);
            row.insert(hi, Node::Sym(Mark::Sel { open: false }.ch()));
            row.insert(lo, Node::Sym(Mark::Sel { open: true }.ch()));
            col = if col >= hi {
                col + 2
            } else if col > lo {
                col + 1
            } else {
                col
            };
        }
        // A wrapper armed for unwrapping: the pair around its node,
        // which the render turns into the block's corners so only the
        // delimiter columns (a radical's root) light up. Armed from
        // inside, the node is in the *parent* row and the cursor's own
        // path shifts by the open mark; armed from outside (a
        // Shift-selection), it sits beside the cursor in the current
        // row and the column shifts instead.
        if self.unwrap_armed.as_deref() == Some(&self.path[..])
            && let Some(&(i, _)) = self.path.last()
        {
            let k = self.path.len() - 1;
            let prow = row_at_mut(&mut root, &path[..k]);
            prow.insert(i + 1, Node::Sym(Mark::Delims { open: false }.ch()));
            prow.insert(i, Node::Sym(Mark::Delims { open: true }.ch()));
            path[k].0 += 1;
        } else if let Some((&(i, _), parent)) =
            self.unwrap_armed.as_deref().and_then(|p| p.split_last())
            && parent == &self.path[..]
        {
            let row = row_at_mut(&mut root, &path);
            if i < row.len() {
                row.insert(i + 1, Node::Sym(Mark::Delims { open: false }.ch()));
                row.insert(i, Node::Sym(Mark::Delims { open: true }.ch()));
                if col > i {
                    col += 2;
                }
            }
        } else if let Some((p, m)) = &self.mid_armed
            && p[..] == self.path[..]
            && let Some(&(i, Field::Seg(_))) = p.last()
        {
            // A │ middle armed for removal: the mark sits at the end
            // of the segment left of the mid; the display walks right
            // from there to the │ column and lights it.
            let seg_path: Vec<(usize, Field)> = p[..p.len() - 1]
                .iter()
                .copied()
                .chain([(i, Field::Seg(*m))])
                .collect();
            let row = row_at_mut(&mut root, &seg_path);
            let end = row.len();
            row.push(Node::Sym(Mark::MidArm.ch()));
            bump(&mut path, &mut col, &seg_path, end);
        }
        (root, Some((path, col)))
    }

    /// Leave grid mode through Enter: the highlighted cell stays
    /// meaningful — its contents become the ordinary selection, so a
    /// wrap (\norm), a delete, or typing-over can act on the cell at
    /// once. An empty cell has nothing to select and just exits.
    pub fn grid_commit_cell(&mut self) {
        self.grid = None;
        // The highlighted thing is the cell, not wherever the cursor
        // is parked inside it — climb to the cell row first (as
        // grid_move does).
        if let Some((k, i, c)) = self.enclosing_array() {
            self.path.truncate(k);
            self.path.push((i, Field::Cell(c)));
        }
        let n = self.cur_row().len();
        if n > 0 {
            self.select_anchor = Some(0);
            self.select_path = self.path.clone();
            self.col = n;
            self.select_whole = false;
        }
    }

    /// Toggle grid edit mode (^G; only meaningful inside a matrix).
    pub fn grid_mode_toggle(&mut self) {
        if self.grid.is_some() {
            self.grid = None;
        } else if self.enclosing_array().is_some() {
            self.grid = Some(GridSel::Cells { anchor: None });
            self.select_anchor = None;
        } else {
            self.info("not inside a grid");
        }
    }

    // ----- grid mode flow (cell rectangle / lane sub-modes) -----

    /// Plain arrow in cell state: move the cell cursor, drop the anchor.
    pub fn grid_cell_move(&mut self, dr: isize, dc: isize) {
        self.grid = Some(GridSel::Cells { anchor: None });
        self.grid_move(dr, dc);
    }

    /// Shift+arrow in cell state: extend the rectangle; pushing past
    /// the edge with a full-axis selection promotes to lane selection
    /// (the same "widen past the boundary = climb to the structure"
    /// grammar as Shift+↑).
    pub fn grid_select_move(&mut self, dr: isize, dc: isize) {
        let Some((_, _, rows, cols, c)) = self.grid_info() else {
            return;
        };
        let Some(GridSel::Cells { anchor }) = self.grid else {
            return;
        };
        let a = anchor.unwrap_or(c);
        let (r, j) = (c / cols, c % cols);
        let (ar, aj) = (a / cols, a % cols);
        if dr != 0 {
            let full = ar.min(r) == 0 && ar.max(r) == rows - 1;
            let at_edge = (dr < 0 && r == 0) || (dr > 0 && r == rows - 1);
            if full && at_edge {
                let (lo, hi) = (aj.min(j), aj.max(j));
                self.grid = Some(GridSel::Lanes {
                    cols: true,
                    pos: 2 * lo + 1,
                    ext: (lo != hi).then_some(hi),
                });
                return;
            }
        }
        if dc != 0 {
            let full = aj.min(j) == 0 && aj.max(j) == cols - 1;
            let at_edge = (dc < 0 && j == 0) || (dc > 0 && j == cols - 1);
            if full && at_edge {
                let (lo, hi) = (ar.min(r), ar.max(r));
                self.grid = Some(GridSel::Lanes {
                    cols: false,
                    pos: 2 * lo + 1,
                    ext: (lo != hi).then_some(hi),
                });
                return;
            }
        }
        self.grid = Some(GridSel::Cells { anchor: Some(a) });
        self.grid_move(dr, dc);
    }

    /// Enter lane mode (`c`/`|` = columns, `r`/`-` = rows) on the
    /// cursor's lane.
    pub fn grid_lanes(&mut self, cols_mode: bool) {
        let Some((_, _, _, cols, c)) = self.grid_info() else {
            return;
        };
        let lane = if cols_mode { c % cols } else { c / cols };
        self.grid = Some(GridSel::Lanes {
            cols: cols_mode,
            pos: 2 * lane + 1,
            ext: None,
        });
    }

    /// Arrow along the lane axis: step the alternating gap/lane cursor.
    pub fn lane_step(&mut self, delta: isize) {
        let Some(GridSel::Lanes { cols, pos, .. }) = self.grid else {
            return;
        };
        let Some((_, _, rows, ncols, _)) = self.grid_info() else {
            return;
        };
        let n = if cols { ncols } else { rows };
        let pos = pos.saturating_add_signed(delta).min(2 * n);
        self.grid = Some(GridSel::Lanes {
            cols,
            pos,
            ext: None,
        });
    }

    /// Shift+arrow along the axis: extend the lane selection; from a
    /// gap it just steps onto the neighbouring lane.
    pub fn lane_extend(&mut self, delta: isize) {
        let Some(GridSel::Lanes { cols, pos, ext }) = self.grid else {
            return;
        };
        let Some((_, _, rows, ncols, _)) = self.grid_info() else {
            return;
        };
        let n = if cols { ncols } else { rows };
        if pos % 2 == 0 {
            return self.lane_step(delta);
        }
        let end = ext.unwrap_or(pos / 2);
        let end = end.saturating_add_signed(delta).min(n - 1);
        self.grid = Some(GridSel::Lanes {
            cols,
            pos,
            ext: (end != pos / 2).then_some(end),
        });
    }

    /// The selected lane range (lo..=hi), when the cursor is on a lane.
    fn lane_range(&self) -> Option<(bool, usize, usize)> {
        let Some(GridSel::Lanes { cols, pos, ext }) = self.grid else {
            return None;
        };
        if pos % 2 == 0 {
            return None;
        }
        let lane = pos / 2;
        let end = ext.unwrap_or(lane);
        Some((cols, lane.min(end), lane.max(end)))
    }

    /// Enter in lane mode: on a gap, insert a lane there (and land on
    /// it); on a lane, drop back to cell selection of that lane.
    pub fn lane_commit(&mut self) {
        let Some(GridSel::Lanes { cols, pos, .. }) = self.grid else {
            return;
        };
        if pos % 2 == 0 {
            self.lane_insert(cols, pos / 2);
            self.grid = Some(GridSel::Lanes {
                cols,
                pos: pos + 1,
                ext: None,
            });
        } else {
            self.lane_demote();
        }
    }

    pub fn lane_delete_sel(&mut self) {
        let Some((cols, lo, hi)) = self.lane_range() else {
            return;
        };
        self.lane_delete(cols, lo, hi);
        if let Some((_, _, rows, ncols, _)) = self.grid_info() {
            let n = if cols { ncols } else { rows };
            self.grid = Some(GridSel::Lanes {
                cols,
                pos: 2 * lo.min(n - 1) + 1,
                ext: None,
            });
        }
    }

    /// Cross-axis arrow / Enter on a lane: back to cell selection —
    /// the lane(s) become a full-axis cell rectangle.
    pub fn lane_demote(&mut self) {
        let Some((k, i, rows, ncols, c)) = self.grid_info() else {
            return;
        };
        match self.lane_range() {
            Some((cols, lo, hi)) => {
                let (anchor, cursor) = if cols {
                    (lo, (rows - 1) * ncols + hi)
                } else {
                    (lo * ncols, (hi + 1) * ncols - 1)
                };
                self.path.truncate(k);
                self.path.push((i, Field::Cell(cursor)));
                self.col = self.cur_row().len();
                self.grid = Some(GridSel::Cells {
                    anchor: Some(anchor),
                });
            }
            None => {
                let _ = c;
                self.grid = Some(GridSel::Cells { anchor: None });
            }
        }
    }

    /// A modal state is capturing keys — undo/redo chords stay out
    /// of the way there.
    pub fn mode_active(&self) -> bool {
        self.free.is_some()
            || self.block.is_some()
            || self.minibuffer.is_some()
            || self.op_entry.is_some()
            || self.ask.is_some()
    }

    pub(crate) fn push_undo(&mut self, state: Snapshot) {
        const UNDO_CAP: usize = 1000;
        self.undo.push(state);
        if self.undo.len() > UNDO_CAP {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    pub fn undo(&mut self) {
        // The pending unwrap and armed mid belong to the tree that is
        // being replaced: `input` returns before the key layer's
        // one-shot take, so they would otherwise survive onto a
        // different tree.
        self.unwrap_armed = None;
        self.mid_armed = None;
        let Some((root, path, col)) = self.undo.pop() else {
            return;
        };
        self.select_anchor = None;
        self.ghost.clear();
        self.redo.push((
            std::mem::replace(&mut self.root, root),
            self.path.clone(),
            self.col,
        ));
        self.path = path;
        self.col = col;
        self.reclamp_grid();
    }

    pub fn redo(&mut self) {
        // Same one-shot shedding as `undo`.
        self.unwrap_armed = None;
        self.mid_armed = None;
        let Some((root, path, col)) = self.redo.pop() else {
            return;
        };
        self.select_anchor = None;
        self.ghost.clear();
        self.undo.push((
            std::mem::replace(&mut self.root, root),
            self.path.clone(),
            self.col,
        ));
        self.path = path;
        self.col = col;
        self.reclamp_grid();
    }
}

/// Splice one lane into a cell rectangle at gap `g` (0..=n), filling
/// it with `fill()`. The row-major arithmetic is identical for a real
/// insertion and for the display's ghost preview, so both go through
/// here; returns the grown (rows, cols).
pub(crate) fn splice_lane(
    cells: &mut Vec<Row>,
    rows: usize,
    cols: usize,
    cols_mode: bool,
    g: usize,
    fill: impl Fn() -> Row,
) -> (usize, usize) {
    if cols_mode {
        for r in (0..rows).rev() {
            cells.insert(r * cols + g.min(cols), fill());
        }
        (rows, cols + 1)
    } else {
        for j in 0..cols {
            cells.insert(g.min(rows) * cols + j, fill());
        }
        (rows + 1, cols)
    }
}

// Nudge the cursor position past a marker inserted at slot `k` of the
// row at `at` (only positions in that row, at or right of `k`, move).
fn bump(path: &mut [(usize, Field)], col: &mut usize, at: &[(usize, Field)], k: usize) {
    let d = at.len();
    if path.len() >= d && path[..d] == *at {
        if path.len() > d {
            if path[d].0 >= k {
                path[d].0 += 1;
            }
        } else if *col > k {
            *col += 1;
        }
    }
}

/// A flat cell index re-based past a ghost lane inserted at gap `g`
/// of a rows×cols array (the gap-cursor preview has real width, so
/// every coordinate that touches the array must shift with it).
pub(crate) fn gap_shift_cell(
    cell: usize,
    cmode: bool,
    g: usize,
    rows: usize,
    cols: usize,
) -> usize {
    let (r, j) = (cell / cols, cell % cols);
    if cmode {
        r * (cols + 1) + j + usize::from(j >= g.min(cols))
    } else {
        (r + usize::from(r >= g.min(rows))) * cols + j
    }
}
