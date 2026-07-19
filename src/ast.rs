//! Math AST. A formula is a `Row` (horizontal sequence of nodes); every
//! editable slot in a structure node is itself a `Row`, so the cursor can
//! always be described as (path into nested rows, column in that row).

pub type Row = Vec<Node>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
    /// A single atom: letter, digit, operator or Unicode math symbol.
    /// Stored as the plain (ASCII where possible) character; styling such as
    /// math-italic letters is applied only at render time.
    Sym(char),
    /// Formatting space (the Space key): renders as one blank column in
    /// the AA, produces no LaTeX/Typst output, and vanishes on reparse —
    /// the roundtrip contract is parse∘render == strip_spacers∘normalize.
    /// (Use the ␣ atom, \space, for a *semantic* space.)
    Spacer,
    /// Named function/operator rendered upright (sin, cos, log, ...).
    /// Only names from `symbols::FUNCS` are valid (parse relies on this).
    Func(String),
    /// Roman/text run (\mathrm / \text), rendered as "…" with upright
    /// chars — the quotes are what keep hand-typed ASCII (lenient italic
    /// atoms) unambiguous. Interior spaces are drawn as ␣.
    Text(String),
    /// Accented base character (x̂ ẋ v̄ a⃗ …): over-marks stack upward and
    /// under-marks downward in the cells directly above/below the base,
    /// innermost first. Flat lists (not nesting) are deliberate: the
    /// picture cannot distinguish \hat{\underline{x}} from
    /// \underline{\hat{x}}, so the AST must not either.
    Accent { overs: Vec<char>, unders: Vec<char>, base: char },
    Frac { num: Row, den: Row },
    /// index 2 = √, 3 = ∛, 4 = ∜ (the only radical glyphs Unicode has).
    Sqrt { arg: Row, index: u8 },
    Sup { arg: Row },
    Sub { arg: Row },
    /// Big operator (∑ ∫ ∏ …) with optional limits typeset under/over it.
    BigOp { op: char, lower: Row, upper: Row },
    /// Stretchy labeled arrow (\xrightarrow / \xleftarrow): a ╌ body with
    /// the head char (→ or ←) at the pointing end, labels over/under
    /// spanning its extent (same range-band idea as ┄).
    Arrow { op: char, over: Row, under: Row },
    /// Auto-scaling delimiter block. `left`/`right`/`mids` hold delimiter
    /// *spec* chars: ( ) [ ] { } ⟨ ⟩ | and '.' (null delimiter, drawn as
    /// the thin ▏ ▕ markers). Middles ('|' only) separate the segments;
    /// segs.len() == mids.len() + 1. Segments are ordinary rows — a matrix
    /// is nothing more than a Delim whose segment contains an Array.
    Delim { left: char, right: char, mids: Vec<char>, segs: Vec<Row> },
    /// rows×cols grid (LaTeX array/matrix), cells stored row-major.
    /// Always drawn as a self-delimiting lattice (┌ ┬ ┐ / ├ ┼ ┤ / └ ┴ ┘
    /// junctions at every separator crossing including the outer edges),
    /// wherever it appears — delimiters simply wrap it.
    Array { rows: usize, cols: usize, cells: Vec<Row> },
    /// Struck-through content (\cancel): every cell of the rendered
    /// argument carries a combining long solidus overlay (U+0338).
    Cancel { arg: Row },
}

/// Valid delimiter spec chars for `Node::Delim` (`.` = null delimiter).
pub const DELIM_SPECS: &[char] = &['(', ')', '[', ']', '{', '}', '⟨', '⟩', '|', '.'];

/// Identifies one editable slot inside a structure node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    FracNum,
    FracDen,
    SqrtArg,
    SupArg,
    SubArg,
    OpLower,
    OpUpper,
    ArrowOver,
    ArrowUnder,
    /// Segment index of a Delim.
    Seg(usize),
    CancelArg,
    /// Row-major cell index of an Array.
    Cell(usize),
}

impl Node {
    /// Editable fields in cursor-traversal order (empty for atoms).
    pub fn fields(&self) -> Vec<Field> {
        match self {
            Node::Sym(_) | Node::Spacer | Node::Func(_) | Node::Text(_) | Node::Accent { .. } => vec![],
            Node::Frac { .. } => vec![Field::FracNum, Field::FracDen],
            Node::Sqrt { .. } => vec![Field::SqrtArg],
            Node::Sup { .. } => vec![Field::SupArg],
            Node::Sub { .. } => vec![Field::SubArg],
            Node::BigOp { .. } => vec![Field::OpLower, Field::OpUpper],
            Node::Arrow { .. } => vec![Field::ArrowOver, Field::ArrowUnder],
            Node::Delim { segs, .. } => (0..segs.len()).map(Field::Seg).collect(),
            Node::Cancel { .. } => vec![Field::CancelArg],
            Node::Array { cells, .. } => (0..cells.len()).map(Field::Cell).collect(),
        }
    }

    pub fn field(&self, f: Field) -> &Row {
        match (self, f) {
            (Node::Frac { num, .. }, Field::FracNum) => num,
            (Node::Frac { den, .. }, Field::FracDen) => den,
            (Node::Sqrt { arg, .. }, Field::SqrtArg) => arg,
            (Node::Sup { arg }, Field::SupArg) => arg,
            (Node::Sub { arg }, Field::SubArg) => arg,
            (Node::BigOp { lower, .. }, Field::OpLower) => lower,
            (Node::BigOp { upper, .. }, Field::OpUpper) => upper,
            (Node::Arrow { over, .. }, Field::ArrowOver) => over,
            (Node::Arrow { under, .. }, Field::ArrowUnder) => under,
            (Node::Delim { segs, .. }, Field::Seg(i)) => &segs[i],
            (Node::Cancel { arg }, Field::CancelArg) => arg,
            (Node::Array { cells, .. }, Field::Cell(i)) => &cells[i],
            _ => panic!("field {:?} does not belong to node {:?}", f, self),
        }
    }

    pub fn field_mut(&mut self, f: Field) -> &mut Row {
        match (self, f) {
            (Node::Frac { num, .. }, Field::FracNum) => num,
            (Node::Frac { den, .. }, Field::FracDen) => den,
            (Node::Sqrt { arg, .. }, Field::SqrtArg) => arg,
            (Node::Sup { arg }, Field::SupArg) => arg,
            (Node::Sub { arg }, Field::SubArg) => arg,
            (Node::BigOp { lower, .. }, Field::OpLower) => lower,
            (Node::BigOp { upper, .. }, Field::OpUpper) => upper,
            (Node::Arrow { over, .. }, Field::ArrowOver) => over,
            (Node::Arrow { under, .. }, Field::ArrowUnder) => under,
            (Node::Delim { segs, .. }, Field::Seg(i)) => &mut segs[i],
            (Node::Cancel { arg }, Field::CancelArg) => arg,
            (Node::Array { cells, .. }, Field::Cell(i)) => &mut cells[i],
            (node, f) => panic!("field {:?} does not belong to node {:?}", f, node),
        }
    }

    pub fn is_empty_structure(&self) -> bool {
        let fields = self.fields();
        !fields.is_empty() && fields.iter().all(|&f| self.field(f).is_empty())
    }
}

/// Walk `path` down from `root` and return the row the cursor lives in.
pub fn row_at<'a>(root: &'a Row, path: &[(usize, Field)]) -> &'a Row {
    let mut row = root;
    for &(i, f) in path {
        row = row[i].field(f);
    }
    row
}

pub fn row_at_mut<'a>(root: &'a mut Row, path: &[(usize, Field)]) -> &'a mut Row {
    let mut row = root;
    for &(i, f) in path {
        row = row[i].field_mut(f);
    }
    row
}

/// Canonical form: merge adjacent same-kind scripts (x^{a}^{b} == x^{ab} in
/// the picture, so the parser can only ever return the merged form).
/// `parse(render(x)) == normalize(x)` is the roundtrip invariant.
pub fn normalize(row: &Row) -> Row {
    let mut out: Row = Vec::with_capacity(row.len());
    for node in row {
        let node = normalize_node(node);
        // Leading spacers are dropped: they would rob a row-initial script
        // of its explicit ⬚ base (and trim eats them anyway).
        if out.is_empty() && matches!(node, Node::Spacer) {
            continue;
        }
        // Empty scripts/cancels do not exist in normal form: their lone ⬚
        // placeholder fuses with neighbouring script blocks in the picture,
        // so the renderer must never produce one.
        match &node {
            Node::Sup { arg } | Node::Sub { arg } | Node::Cancel { arg } if arg.is_empty() => {
                continue;
            }
            // An empty text run has no picture of its own worth keeping.
            Node::Text(t) if t.is_empty() => continue,
            _ => {}
        }
        // Scripts and cancels merge *across* spacers: the blank column a
        // spacer renders is internal to the script run in the picture, so
        // the parser reads one merged node — canonical form matches (the
        // spacers die in the merge, and are restored when no merge fires).
        let mut tail: Row = Vec::new();
        if matches!(node, Node::Sup { .. } | Node::Sub { .. } | Node::Cancel { .. }) {
            while matches!(out.last(), Some(Node::Spacer)) {
                tail.push(out.pop().unwrap());
            }
        }
        match (out.last_mut(), &node) {
            // Re-normalize after merging: the concatenation can create new
            // same-kind adjacencies inside the argument (idempotence).
            (Some(Node::Sup { arg: a }), Node::Sup { arg: b }) => {
                a.extend(b.clone());
                *a = normalize(a);
            }
            (Some(Node::Sub { arg: a }), Node::Sub { arg: b }) => {
                a.extend(b.clone());
                *a = normalize(a);
            }
            // Adjacent struck-through blocks are one picture.
            (Some(Node::Cancel { arg: a }), Node::Cancel { arg: b }) => {
                a.extend(b.clone());
                *a = normalize(a);
            }
            // "Maximal cancel": a fully struck script right after a cancel
            // is the same picture as the script living inside it.
            (Some(Node::Cancel { arg: a }), Node::Sup { arg: b })
                if matches!(b[..], [Node::Cancel { .. }]) =>
            {
                let Node::Cancel { arg: inner } = &b[0] else { unreachable!() };
                a.push(Node::Sup { arg: inner.clone() });
                *a = normalize(a);
            }
            (Some(Node::Cancel { arg: a }), Node::Sub { arg: b })
                if matches!(b[..], [Node::Cancel { .. }]) =>
            {
                let Node::Cancel { arg: inner } = &b[0] else { unreachable!() };
                a.push(Node::Sub { arg: inner.clone() });
                *a = normalize(a);
            }
            _ => {
                out.extend(tail);
                out.push(node);
            }
        }
    }
    // Trailing spacers pad nothing visible (and a trailing one inside a
    // \cancel argument would meet the ragged-cancel spacer and fake a cell
    // gap); spacers are only meaningful *between* siblings.
    while matches!(out.last(), Some(Node::Spacer)) {
        out.pop();
    }
    out
}

/// Remove every Cancel wrapper in the subtree, splicing its contents.
fn strip_cancels(row: &Row) -> Row {
    let mut out: Row = Vec::new();
    for n in row {
        match n {
            Node::Cancel { arg } => out.extend(strip_cancels(arg)),
            Node::Sym(_) | Node::Spacer | Node::Func(_) | Node::Text(_) | Node::Accent { .. } => {
                out.push(n.clone())
            }
            Node::Frac { num, den } => out.push(Node::Frac {
                num: strip_cancels(num),
                den: strip_cancels(den),
            }),
            Node::Sqrt { arg, index } => {
                out.push(Node::Sqrt { arg: strip_cancels(arg), index: *index })
            }
            Node::Sup { arg } => out.push(Node::Sup { arg: strip_cancels(arg) }),
            Node::Sub { arg } => out.push(Node::Sub { arg: strip_cancels(arg) }),
            Node::BigOp { op, lower, upper } => out.push(Node::BigOp {
                op: *op,
                lower: strip_cancels(lower),
                upper: strip_cancels(upper),
            }),
            Node::Arrow { op, over, under } => out.push(Node::Arrow {
                op: *op,
                over: strip_cancels(over),
                under: strip_cancels(under),
            }),
            Node::Delim { left, right, mids, segs } => out.push(Node::Delim {
                left: *left,
                right: *right,
                mids: mids.clone(),
                segs: segs.iter().map(strip_cancels).collect(),
            }),
            Node::Array { rows, cols, cells } => out.push(Node::Array {
                rows: *rows,
                cols: *cols,
                cells: cells.iter().map(strip_cancels).collect(),
            }),
        }
    }
    out
}

/// Remove every formatting `Spacer` in the subtree — exactly what the
/// parser cannot see (blank columns are structural). The roundtrip
/// contract is `parse(render(normalize(x))) == normalize(strip_spacers(normalize(x)))`.
pub fn strip_spacers(row: &Row) -> Row {
    let mut out: Row = Vec::new();
    for n in row {
        match n {
            Node::Spacer => {}
            Node::Sym(_) | Node::Func(_) | Node::Text(_) | Node::Accent { .. } => {
                out.push(n.clone())
            }
            Node::Frac { num, den } => out.push(Node::Frac {
                num: strip_spacers(num),
                den: strip_spacers(den),
            }),
            Node::Sqrt { arg, index } => {
                out.push(Node::Sqrt { arg: strip_spacers(arg), index: *index })
            }
            Node::Sup { arg } => out.push(Node::Sup { arg: strip_spacers(arg) }),
            Node::Sub { arg } => out.push(Node::Sub { arg: strip_spacers(arg) }),
            Node::BigOp { op, lower, upper } => out.push(Node::BigOp {
                op: *op,
                lower: strip_spacers(lower),
                upper: strip_spacers(upper),
            }),
            Node::Arrow { op, over, under } => out.push(Node::Arrow {
                op: *op,
                over: strip_spacers(over),
                under: strip_spacers(under),
            }),
            Node::Delim { left, right, mids, segs } => out.push(Node::Delim {
                left: *left,
                right: *right,
                mids: mids.clone(),
                segs: segs.iter().map(strip_spacers).collect(),
            }),
            Node::Array { rows, cols, cells } => out.push(Node::Array {
                rows: *rows,
                cols: *cols,
                cells: cells.iter().map(strip_spacers).collect(),
            }),
            Node::Cancel { arg } => out.push(Node::Cancel { arg: strip_spacers(arg) }),
        }
    }
    out
}

fn normalize_node(node: &Node) -> Node {
    match node {
        Node::Sym(c) if crate::symbols::bigop_by_char(*c) => {
            // A bare big-operator symbol is the same picture as a BigOp
            // with empty limits; canonical form uses the BigOp node.
            Node::BigOp { op: *c, lower: vec![], upper: vec![] }
        }
        // A markless accent is just its base.
        Node::Accent { overs, unders, base } if overs.is_empty() && unders.is_empty() => {
            let _ = (overs, unders);
            Node::Sym(*base)
        }
        Node::Sym(_) | Node::Spacer | Node::Func(_) | Node::Text(_) | Node::Accent { .. } => {
            node.clone()
        }
        Node::Frac { num, den } => Node::Frac { num: normalize(num), den: normalize(den) },
        Node::Sqrt { arg, index } => Node::Sqrt { arg: normalize(arg), index: *index },
        Node::Sup { arg } => Node::Sup { arg: normalize(arg) },
        Node::Sub { arg } => Node::Sub { arg: normalize(arg) },
        Node::BigOp { op, lower, upper } => Node::BigOp {
            op: *op,
            lower: normalize(lower),
            upper: normalize(upper),
        },
        Node::Arrow { op, over, under } => Node::Arrow {
            op: *op,
            over: normalize(over),
            under: normalize(under),
        },
        Node::Delim { left, right, mids, segs } => {
            let segs = segs
                .iter()
                .map(|seg| {
                    // A sole 1×1 grid is indistinguishable from its cell in
                    // the fused picture — canonical form is the plain row.
                    // Iterate: splicing can surface a new sole Array.
                    let mut seg = normalize(seg);
                    loop {
                        seg = match &seg[..] {
                            [Node::Array { rows: 1, cols: 1, cells }] => normalize(&cells[0]),
                            _ => break seg,
                        };
                    }
                })
                .collect();
            Node::Delim { left: *left, right: *right, mids: mids.clone(), segs }
        }
        Node::Cancel { arg } => {
            // A cancel strikes every cell of its subtree, so any Cancel
            // nested anywhere inside it (even deep in a fraction) is the
            // same picture; dissolve them all.
            Node::Cancel { arg: normalize(&strip_cancels(&normalize(arg))) }
        }
        Node::Array { rows, cols, cells } => Node::Array {
            rows: *rows,
            cols: *cols,
            cells: cells.iter().map(normalize).collect(),
        },
    }
}
