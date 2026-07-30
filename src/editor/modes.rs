//! Interactive modes layered over the structural editor: the free
//! cursor (^F), jump labels (^G), block select (^B) and the display
//! decoration they all paint through. None of these change the
//! formula — they move the cursor or hand a selection to the editor —
//! which is why they live apart from the editing operations.

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
        self.message.clear();
    }

    /// The cursor's own display cell (in the display frame).
    fn nearest_position_of_cursor(&self) -> Option<(CursorPos, (usize, usize))> {
        let cands = self.jump_candidates();
        let coords = self.display_coords(&cands);
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
        // Collapsed elements (inline scripts, invisible slots) expand
        // automatically while the free cursor is near them. Proximity is
        // measured in the *current display frame* against the element's
        // always-visible anchor (the position before its node in the
        // parent row), with hysteresis (R_IN/R_OUT) so the expansion
        // shift cannot make the test oscillate.
        let cands = self.jump_candidates();
        let disp = self.display_coords(&cands);
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
            let disp2 = self.display_coords(&cands2);
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
        if let Some(t) = self.jump.take() {
            self.keep_ghosts(&t);
        }
        if let Some(f) = self.free.take() {
            self.path = f.snap.0;
            self.col = f.snap.1;
            self.message.clear();
        }
    }

    pub fn free_cancel(&mut self) {
        self.free = None;
        self.jump = None;
        self.select_anchor = None;
        self.ghost.clear();
        self.message.clear();
    }

    /// ^G in free mode: toggle the jump markers.
    pub fn free_toggle_markers(&mut self) {
        if self.jump.is_some() {
            self.free_markers_off();
        } else {
            self.start_jump();
        }
    }

    /// Drop the markers (Esc / toggle), keeping ghosts and the free
    /// cursor anchored to the stable layout.
    pub fn free_markers_off(&mut self) {
        if let Some(t) = self.jump.take() {
            self.keep_ghosts(&t);
        }
        self.free_reanchor();
        self.message.clear();
    }

    /// A jump label pressed while markers are up: move there and drop
    /// back to plain free motion.
    pub fn free_jump(&mut self, label: char) {
        let Some(idx) = JUMP_LABELS.chars().position(|c| c == label) else {
            return;
        };
        self.free_goto_rank(idx);
    }

    /// Enter while markers are up: land on the arrow-selected marker,
    /// back to free motion from there.
    pub fn free_goto_selected(&mut self) {
        self.free_goto_rank(self.jump_selected);
    }

    fn free_goto_rank(&mut self, rank: usize) {
        let Some(targets) = &self.jump else { return };
        let Some((_, (p, c))) = targets.iter().find(|(r, _)| *r == rank) else {
            return;
        };
        let (p, c) = (p.clone(), *c);
        if let Some(t) = self.jump.take() {
            self.keep_ghosts(&t);
        }
        self.path = p;
        self.col = c;
        self.free_reanchor();
        self.message.clear();
    }

    /// Re-anchor the free cursor onto the cursor's current display cell.
    fn free_reanchor(&mut self) {
        if self.free.is_none() {
            return;
        }
        if let Some((pos, xy)) = self.nearest_position_of_cursor()
            && let Some(f) = &mut self.free
        {
            f.at = xy;
            f.snap = pos;
            f.snap_at = xy;
        }
    }

    /// Ctrl+E: jump to the very end of the formula.
    pub fn document_end(&mut self) {
        self.select_anchor = None;
        self.path.clear();
        self.col = self.root.len();
    }

    /// Ctrl+A: jump to the very start of the whole formula.
    pub fn document_start(&mut self) {
        self.select_anchor = None;
        self.path.clear();
        self.col = 0;
    }

    /// Enter at the top level: start a new formula line (Node::Break).
    pub fn break_line(&mut self) {
        self.select_anchor = None;
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

    // ----- jump mode (EasyMotion-style) -----

    /// Every cursor position in true document order (column, then that
    /// node's children, then the next column), with the flags the jump
    /// selection needs. Document order is required by the coordinate
    /// probe (marker insertion invalidates later positions otherwise).
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

    /// Physical cell coordinates of the given document-ordered
    /// positions: one render with every position carrying a zero-width
    /// probe mark — the geometry (and thus every coordinate) is
    /// identical to the displayed render.
    fn position_coords(&self, positions: &[&CursorPos]) -> Vec<Option<(usize, usize)>> {
        use crate::render::{RenderCtx, render_root};
        let n = positions.len().min(0x800);
        let mut root = self.root.clone();
        for (idx, (p, c)) in positions.iter().take(n).enumerate().rev() {
            let mark = char::from_u32(PROBE_BASE + idx as u32).unwrap();
            row_at_mut(&mut root, p).insert(*c, Node::Sym(mark));
        }
        let b = render_root(&root, None, &RenderCtx::canonical());
        let mut out = vec![None; positions.len()];
        for &(y, x, ch) in &b.marks {
            let u = ch as u32;
            if u >= PROBE_BASE {
                let i = (u - PROBE_BASE) as usize;
                if i < out.len() {
                    out[i] = Some((y, x));
                }
            }
        }
        out
    }

    pub fn start_jump(&mut self) {
        use std::cmp::Reverse;
        let cands = self.jump_candidates();
        let positions: Vec<&CursorPos> = cands.iter().map(|c| &c.pos).collect();
        let coords = self.position_coords(&positions);
        let Some(cur_i) = cands.iter().position(|c| c.is_cursor) else {
            self.message = "no jump targets".into();
            return;
        };
        let Some((cy, cx)) = coords[cur_i] else {
            self.message = "no jump targets".into();
            return;
        };
        let dist2 =
            |a: (usize, usize), b: (usize, usize)| JUMP_W_Y * a.0.abs_diff(b.0) + a.1.abs_diff(b.1);
        let dist = |i: usize| coords[i].map(|xy| dist2(xy, (cy, cx)));

        // Hard filter (spec §2): the cursor itself, run interiors, and
        // anything the probe could not place.
        let usable: Vec<usize> = (0..cands.len())
            .filter(|&i| !cands[i].is_cursor && !cands[i].interior && coords[i].is_some())
            .collect();

        // Classes (spec §3). B: bounds of every row enclosing the cursor.
        let ancestor_bound = |i: usize| {
            let (p, _) = &cands[i].pos;
            cands[i].bound && p.len() <= self.path.len() && self.path[..p.len()] == p[..]
        };
        let cursor_grid = {
            let grid_key = |p: &[(usize, Field)]| -> Option<(usize, Vec<(usize, Field)>)> {
                let k = p.iter().rposition(|(_, f)| matches!(f, Field::Cell(_)))?;
                Some((p[k].0, p[..k].to_vec()))
            };
            grid_key(&self.path)
        };
        let same_grid = |i: usize| {
            let k = cands[i]
                .pos
                .0
                .iter()
                .rposition(|(_, f)| matches!(f, Field::Cell(_)));
            match (k, &cursor_grid) {
                (Some(k), Some((gi, gp))) => {
                    cands[i].pos.0[k].0 == *gi && cands[i].pos.0[..k] == gp[..]
                }
                _ => false,
            }
        };
        let class = |i: usize| {
            if cands[i].empty {
                0
            } else if ancestor_bound(i) {
                1
            } else if cands[i].cell_end {
                2
            } else {
                3
            }
        };

        // One arrow press away (spec §4.1): same row, adjacent column.
        let arrow1 = |a: &CursorPos, b: &CursorPos| a.0 == b.0 && a.1.abs_diff(b.1) <= 1;

        // Priority classes first (by distance), then the general pool.
        let mut order = usable.clone();
        order.sort_by_key(|&i| (class(i), dist(i).unwrap_or(usize::MAX), !same_grid(i)));
        let mut chosen: Vec<usize> = Vec::new();
        let cursor_pos = (self.path.clone(), self.col);
        for &i in &order {
            if chosen.len() >= JUMP_MAX_RANKS {
                break;
            }
            let pos = &cands[i].pos;
            let xy = coords[i].unwrap();
            if arrow1(pos, &cursor_pos) {
                continue;
            }
            if chosen.iter().any(|&j| arrow1(pos, &cands[j].pos)) {
                continue;
            }
            if class(i) == 3 {
                // Density control (spec §4.2) + materialization cost.
                let mut d = dist(i).unwrap();
                if matches!(pos.0.last(), Some((_, Field::SupArg | Field::SubArg))) {
                    d += JUMP_C_GHOST;
                }
                let radius = JUMP_R_MIN.max(d / JUMP_ALPHA_DIV);
                let near = chosen
                    .iter()
                    .filter_map(|&j| coords[j])
                    .chain([(cy, cx)])
                    .map(|xy2| dist2(xy, xy2))
                    .min()
                    .unwrap_or(usize::MAX);
                if near < radius {
                    continue;
                }
            }
            chosen.push(i);
        }
        if chosen.is_empty() {
            self.message = "no jump targets".into();
            return;
        }
        // Ranks = selection order; markers need document order.
        let mut picked: Vec<(usize, CursorPos)> = chosen
            .iter()
            .enumerate()
            .map(|(rank, &i)| (rank, cands[i].pos.clone()))
            .collect();
        picked.sort_by_key(|&(rank, _)| chosen[rank]);
        let _ = Reverse(0); // (kept for potential ordering tweaks)
        self.message = "jump: label key / arrows + Enter (Esc cancels)".into();
        self.jump = Some(picked);
        self.jump_selected = 0;
    }

    /// Move the arrow-key selection to the nearest marker in the given
    /// direction (dx, dy ∈ {-1, 0, 1}).
    pub fn jump_select(&mut self, dx: i32, dy: i32) {
        let coords = self.jump_marker_coords();
        let Some(&(_, cur)) = coords.iter().find(|(r, _)| *r == self.jump_selected) else {
            return;
        };
        let best = coords
            .iter()
            .filter(|(r, _)| *r != self.jump_selected)
            .filter(|(_, xy)| {
                let (gx, gy) = (xy.1 as i64 - cur.1 as i64, xy.0 as i64 - cur.0 as i64);
                (dx != 0 && gx.signum() == dx as i64 || dy != 0 && gy.signum() == dy as i64)
                    && (dx != 0 || gx.abs() <= gy.abs() * 2)
                    && (dy != 0 || gy.abs() <= gx.abs() * 2)
            })
            .min_by_key(|(_, xy)| JUMP_W_Y * xy.0.abs_diff(cur.0) + xy.1.abs_diff(cur.1));
        if let Some(&(r, _)) = best {
            self.jump_selected = r;
        }
    }

    /// Enter: jump to the arrow-selected marker.
    pub fn jump_confirm(&mut self) {
        let rank = self.jump_selected;
        if let Some(targets) = self.jump.take() {
            self.keep_ghosts(&targets);
            if let Some((_, (p, c))) = targets.iter().find(|(r, _)| *r == rank) {
                self.path = p.clone();
                self.col = *c;
            }
            self.message.clear();
        }
    }

    /// Live cell coordinates of the current jump markers, decoded from
    /// the decorated render (rank chars carry their identity).
    fn jump_marker_coords(&self) -> Vec<(usize, (usize, usize))> {
        use crate::render::{RenderCtx, render_root};
        let (root, cursor) = self.decorated();
        let cur = cursor.as_ref().map(|(p, c)| (p.as_slice(), *c));
        let b = render_root(&root, cur, &RenderCtx::canonical());
        b.marks
            .iter()
            .filter_map(|&(y, x, ch)| {
                let u = ch as u32;
                (JUMP_RANK_BASE..JUMP_RANK_BASE + JUMP_MAX_RANKS as u32)
                    .contains(&u)
                    .then(|| ((u - JUMP_RANK_BASE) as usize, (y, x)))
            })
            .collect()
    }

    /// Remember which labeled rows change their rendering while marked:
    /// empty slots (materialized as ⬚) and inline-script args (expanded
    /// to 2D). They keep that form until the next non-^G input, so
    /// re-entering ^G never shifts the layout.
    pub fn keep_ghosts(&mut self, targets: &[(usize, CursorPos)]) {
        self.ghost.clear();
        for (_, (p, _)) in targets {
            let keep = row_at(&self.root, p).is_empty()
                || matches!(p.last(), Some((_, Field::SupArg | Field::SubArg)));
            if keep && !self.ghost.contains(p) {
                self.ghost.push(p.clone());
            }
        }
    }

    pub fn jump_to(&mut self, label: char) {
        if let Some(targets) = self.jump.take() {
            self.keep_ghosts(&targets);
            if let Some(idx) = JUMP_LABELS.chars().position(|c| c == label)
                && let Some((_, (p, c))) = targets.iter().find(|(rank, _)| *rank == idx)
            {
                self.path = p.clone();
                self.col = *c;
            }
            self.message.clear();
        }
    }

    // ----- block-select mode (Ctrl+B: the cursor's ancestor chain) -----

    /// The cursor's enclosing structure nodes, innermost first: one
    /// (parent row path, node index) per ancestor.
    pub fn block_targets(&self) -> Vec<BlockRef> {
        (0..self.path.len())
            .rev()
            .map(|k| (self.path[..k].to_vec(), self.path[k].0))
            .collect()
    }

    pub fn start_block_select(&mut self) {
        if self.block.is_some() {
            // ^B again: back to where the cursor was (it never moved).
            self.block_cancel();
            return;
        }
        let targets = self.block_targets();
        if targets.is_empty() {
            self.message = "no enclosing block (cursor is at the top level)".into();
            return;
        }
        self.block_sel = 0;
        self.block = Some(targets);
        self.message = "block: ↑/→ wider  ↓/← narrower  Enter/label select  ^B/Esc cancel".into();
    }

    pub fn block_cancel(&mut self) {
        self.block = None;
        self.message.clear();
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
            && let Some((p, i)) = targets.get(sel)
        {
            self.path = p.clone();
            self.select_anchor = Some(*i);
            self.select_path = p.clone();
            self.col = i + 1;
        }
        self.message.clear();
    }

    /// Select the ancestor behind a label key directly.
    pub fn block_to(&mut self, label: char) {
        let len = self.block.as_ref().map_or(0, Vec::len);
        if let Some(idx) = JUMP_LABELS
            .chars()
            .position(|c| c == label)
            .filter(|&i| i < len)
        {
            self.block_sel = idx;
            self.block_commit();
        } else {
            self.block_cancel();
        }
    }

    /// Vertical extents (rows above / below the marker's baseline row)
    /// of the boxes the display should paint: one entry for the active
    /// selection, or one per ^B target in ascending-open-column
    /// (= document) order. Computed by laying out the covered slice —
    /// the char grid alone cannot tell a block's rows apart from other
    /// content (e.g. a denominator centered under the same columns).
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
                .map(|(rank, (p, i))| {
                    // An Array fused into its delimiter has no isolated
                    // layout of its own: measure the parent Delim slice
                    // instead (the fused interior spans its full height).
                    let (p, i) = if self.fused_in_delim(p, *i) {
                        (&p[..p.len() - 1], p.last().unwrap().0)
                    } else {
                        (&p[..], *i)
                    };
                    // If the cursor is inside this block, lay the slice
                    // out in its editing view (matches the display).
                    let cur = (self.path.len() > p.len()
                        && self.path[..p.len()] == p[..]
                        && self.path[p.len()].0 == i)
                        .then(|| {
                            let mut rel = self.path[p.len()..].to_vec();
                            rel[0].0 = 0;
                            (rel, self.col)
                        });
                    // The gradient ranks by ancestry (innermost first).
                    extent(&row_at(&self.root, p)[i..i + 1], cur, rank)
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
            // The same rectangle the ops edit and decorated() marks —
            // one extent per cell, in the same row-major order.
            let sel: Vec<usize> = self
                .grid_rect()
                .map(|(r0, j0, r1, j1)| {
                    (r0..=r1)
                        .flat_map(|r| (j0..=j1).map(move |j| r * cols + j))
                        .collect()
                })
                .unwrap_or_default();
            let mut v: Vec<(usize, usize, usize)> = sel
                .iter()
                .map(|&cell| extent(&cells[cell], None, 0))
                .collect();
            // The frame box (whole-array extent) rides last; the
            // display reads it from the end.
            v.push(extent(
                &row_at(&self.root, &self.path[..k])[i..i + 1],
                None,
                0,
            ));
            v
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
            } => {
                use crate::symbols::Delim as D;
                let fuses = |d: D| matches!(d, D::Paren | D::Bracket | D::Ceil | D::Floor | D::Bar);
                *mids == 0 && fuses(*left) && fuses(*right)
            }
            _ => false,
        }
    }

    // ----- display decoration (jump labels / selection) -----

    /// A copy of the AST with display markers inserted, plus the cursor
    /// adjusted for the insertions (None while jump mode hides it).
    /// Markers are private-use `Sym`s that the TUI turns into colored
    /// labels / brackets; they never appear in a real document.
    pub fn decorated(&self) -> (Row, Option<CursorPos>) {
        let mut root = self.root.clone();
        // Nudge the cursor position past a marker inserted at slot `k`
        // of the row at `at`, so the cursor can stay threaded through
        // mode displays (keeping the editing-view geometry: ⬚ limit
        // slots, unfused matrices, expanded inline scripts).
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
        if let Some(targets) = &self.jump {
            let mut path = self.path.clone();
            let mut col = self.col;
            // A live selection is tracked through the marker insertions
            // like the cursor, then painted with the same SEL markers
            // as the normal branch.
            let mut sel = self
                .selection()
                .map(|(lo, hi)| ((self.path.clone(), lo), (self.path.clone(), hi)));
            // Reverse document order keeps not-yet-inserted positions valid.
            for (rank, (p, c)) in targets.iter().rev() {
                let mark = char::from_u32(JUMP_RANK_BASE + *rank as u32).unwrap();
                row_at_mut(&mut root, p).insert(*c, Node::Sym(mark));
                bump(&mut path, &mut col, p, *c);
                if let Some(((lp, lc), (hp, hc))) = &mut sel {
                    bump(lp, lc, p, *c);
                    bump(hp, hc, p, *c);
                }
            }
            if let Some(((lp, lo), (_, hi))) = sel {
                let row = row_at_mut(&mut root, &lp);
                row.insert(hi, Node::Sym(SEL_CLOSE));
                row.insert(lo, Node::Sym(SEL_OPEN));
                // The cursor may sit inside this row (or deeper): thread
                // it through both insertions like any other marker.
                bump(&mut path, &mut col, &lp, hi);
                bump(&mut path, &mut col, &lp, lo);
            }
            return (root, Some((path, col)));
        }
        if let Some(targets) = &self.block {
            let mut path = self.path.clone();
            let mut col = self.col;
            // Label sits immediately left of its block and a close marker
            // right after it, so the display can paint the block's extent.
            // Targets are innermost first = deepest first: inserting into
            // a deep row never shifts a shallower target's position.
            for (idx, (p, i)) in targets.iter().enumerate() {
                let mark = char::from_u32(JUMP_CHAR_BASE + idx as u32).unwrap();
                let row = row_at_mut(&mut root, p);
                row.insert(i + 1, Node::Sym(BLK_CLOSE));
                row.insert(*i, Node::Sym(mark));
                bump(&mut path, &mut col, p, i + 1);
                bump(&mut path, &mut col, p, *i);
            }
            return (root, Some((path, col)));
        }
        if let Some(gs) = self.grid
            && let Some((k, i, rows, cols, c)) = self.grid_info()
        {
            let mut path = self.path.clone();
            let mut col = self.col;
            let parent_path = self.path[..k].to_vec();
            // The frame pair around the Array node lets the display
            // recolor its lattice — the "grid mode is on" signal. A
            // fused matrix's frame IS its delimiter columns, so it
            // gets the FUSED pair (wider display scan).
            let fused = self.fused_in_delim(&parent_path, i);
            {
                let (op, cl) = if fused {
                    (FRAME_FUSED_OPEN, FRAME_FUSED_CLOSE)
                } else {
                    (FRAME_OPEN, FRAME_CLOSE)
                };
                let prow = row_at_mut(&mut root, &parent_path);
                prow.insert(i + 1, Node::Sym(cl));
                prow.insert(i, Node::Sym(op));
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
                    GridSel::Cells { .. } => (CELLS_OPEN, CELLS_CLOSE),
                    GridSel::Lanes { cols: true, .. } => (LANE_OPEN, LANE_CLOSE),
                    GridSel::Lanes { cols: false, .. } => (ROWLANE_OPEN, ROWLANE_CLOSE),
                };
                for r in r0..=r1 {
                    for j in j0..=j1 {
                        let cell = &mut cells[r * cols + j];
                        let hi = cell.len();
                        cell.insert(hi, Node::Sym(cl));
                        cell.insert(0, Node::Sym(op));
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
            // cell painted by the GRID_GAP mark). The ghost has real
            // width, so the parked cell's index shifts with it.
            let GridSel::Lanes {
                cols: cmode, pos, ..
            } = gs
            else {
                unreachable!()
            };
            let g = pos / 2;
            let mark = if cmode { GRID_GAP } else { GRID_GAP_ROW };
            let ghost = || vec![Node::Sym(mark), Node::Spacer];
            if cmode {
                for r in (0..rows).rev() {
                    cells.insert(r * cols + g.min(cols), ghost());
                }
                *nc = cols + 1;
            } else {
                for j in 0..cols {
                    cells.insert(g.min(rows) * cols + j, ghost());
                }
                *nr = rows + 1;
            }
            {
                let (r, j) = (c / cols, c % cols);
                let shifted = if cmode {
                    r * (cols + 1) + j + usize::from(j >= g.min(cols))
                } else {
                    (r + usize::from(r >= g.min(rows))) * cols + j
                };
                path[k].1 = Field::Cell(shifted);
            }
            return (root, Some((path, col)));
        }
        let mut path = self.path.clone();
        let mut col = self.col;
        // Deepest rows first: inserting into an ancestor row would shift
        // the node indices a deeper ghost path still needs. The cursor
        // is adjusted in one batch against the original coordinates
        // (incremental nudging breaks on nested ghosts).
        let mut ghosts: Vec<&Vec<(usize, Field)>> = self.ghost.iter().collect();
        ghosts.sort_by_key(|p| std::cmp::Reverse(p.len()));
        for p in ghosts {
            row_at_mut(&mut root, p).insert(0, Node::Sym(SLOT_GHOST));
        }
        let adjusted = ghost_adjust(&self.ghost, &path, col);
        path = adjusted.0;
        col = adjusted.1;
        if let Some((_, buf)) = &self.op_entry {
            // The in-progress name shows as an upright run (a display-
            // only Func atom: never quoted, spaces drawn as ␣) framed by
            // the selection markers, cursor right after the text.
            let row = row_at_mut(&mut root, &path);
            let node = if buf.is_empty() {
                Node::Sym('⬚')
            } else {
                Node::Func(buf.replace(' ', "␣"))
            };
            row.insert(col, Node::Sym(SEL_CLOSE));
            row.insert(col, node);
            row.insert(col, Node::Sym(SEL_OPEN));
            return (root, Some((path, col + 2)));
        }
        if let Some((lo, hi)) = self.selection() {
            let row = row_at_mut(&mut root, &path);
            row.insert(hi, Node::Sym(SEL_CLOSE));
            row.insert(lo, Node::Sym(SEL_OPEN));
            col = if col >= hi {
                col + 2
            } else if col > lo {
                col + 1
            } else {
                col
            };
        }
        (root, Some((path, col)))
    }

    /// Toggle grid edit mode (only meaningful inside a matrix).
    pub fn grid_mode_toggle(&mut self) {
        if self.grid.is_some() {
            self.grid = None;
        } else if self.enclosing_array().is_some() {
            self.grid = Some(GridSel::Cells { anchor: None });
            self.select_anchor = None;
        } else {
            self.message = "^O works inside a matrix/array".into();
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

    /// Delete in lane mode: remove the selected lane(s).
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

    /// A modal state is capturing keys (jump/block/free/minibuffer/op
    /// box) — undo/redo chords stay out of the way there.
    pub fn mode_active(&self) -> bool {
        self.jump.is_some()
            || self.block.is_some()
            || self.free.is_some()
            || self.minibuffer.is_some()
            || self.op_entry.is_some()
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
        let Some((root, path, col)) = self.undo.pop() else {
            self.message = "nothing to undo".into();
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
        let Some((root, path, col)) = self.redo.pop() else {
            self.message = "nothing to redo".into();
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
