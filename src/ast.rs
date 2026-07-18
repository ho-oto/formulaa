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
    /// Named function/operator rendered upright (sin, cos, log, ...).
    /// Only names from `symbols::FUNCS` are valid (parse relies on this).
    Func(String),
    /// Accent over a single base character (x̂ ẋ v̄ a⃗ …), typeset as a
    /// 2D mark in the cell directly above the base.
    Accent { accent: char, base: char },
    Frac { num: Row, den: Row },
    /// index 2 = √, 3 = ∛, 4 = ∜ (the only radical glyphs Unicode has).
    Sqrt { arg: Row, index: u8 },
    Sup { arg: Row },
    Sub { arg: Row },
    /// Big operator (∑ ∫ ∏ …) with optional limits typeset under/over it.
    BigOp { op: char, lower: Row, upper: Row },
    /// Auto-scaling delimiter block. `left`/`right`/`mids` hold delimiter
    /// *spec* chars: ( ) [ ] { } ⟨ ⟩ | and '.' (null delimiter, drawn as
    /// the thin ▏ ▕ markers). Middles ('|' only) separate the segments;
    /// segs.len() == mids.len() + 1. Canonical constraints (normalize):
    /// a plain [ ] pair with no mids always holds a single [Array] seg
    /// (that picture is the matrix), other segs hold an [Array] only when
    /// the grid has >= 2 cells.
    Delim { left: char, right: char, mids: Vec<char>, segs: Vec<Row> },
    /// rows×cols grid (LaTeX array/matrix), cells stored row-major.
    /// As the sole node of a Delim segment it renders as a blank-gap grid
    /// body (the delimiter gives the extent); anywhere else it renders as
    /// a self-delimiting ┼ lattice (markers at every separator crossing
    /// including the outer edges).
    Array { rows: usize, cols: usize, cells: Vec<Row> },
    /// Struck-through content (\cancel): every cell of the rendered
    /// argument carries a combining long solidus overlay (U+0338).
    Cancel { arg: Row },
}

/// Valid delimiter spec chars for `Node::Delim` (`.` = null delimiter).
pub const DELIM_SPECS: &[char] = &['(', ')', '[', ']', '{', '}', '⟨', '⟩', '|', '.'];

impl Node {
    /// A `[ … ]` pair with no middles: its interior is always a grid
    /// (this is the canonical matrix picture).
    pub fn is_plain_bracket(left: char, right: char, mids: &[char]) -> bool {
        left == '[' && right == ']' && mids.is_empty()
    }
}

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
            Node::Sym(_) | Node::Func(_) | Node::Accent { .. } => vec![],
            Node::Frac { .. } => vec![Field::FracNum, Field::FracDen],
            Node::Sqrt { .. } => vec![Field::SqrtArg],
            Node::Sup { .. } => vec![Field::SupArg],
            Node::Sub { .. } => vec![Field::SubArg],
            Node::BigOp { .. } => vec![Field::OpLower, Field::OpUpper],
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
        // Empty scripts/cancels do not exist in normal form: their lone ⬚
        // placeholder fuses with neighbouring script blocks in the picture,
        // so the renderer must never produce one.
        match &node {
            Node::Sup { arg } | Node::Sub { arg } | Node::Cancel { arg } if arg.is_empty() => {
                continue;
            }
            _ => {}
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
            _ => out.push(node),
        }
    }
    out
}

/// Remove every Cancel wrapper in the subtree, splicing its contents.
fn strip_cancels(row: &Row) -> Row {
    let mut out: Row = Vec::new();
    for n in row {
        match n {
            Node::Cancel { arg } => out.extend(strip_cancels(arg)),
            Node::Sym(_) | Node::Func(_) | Node::Accent { .. } => out.push(n.clone()),
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

fn normalize_node(node: &Node) -> Node {
    match node {
        Node::Sym(c) if crate::symbols::bigop_by_char(*c) => {
            // A bare big-operator symbol is the same picture as a BigOp
            // with empty limits; canonical form uses the BigOp node.
            Node::BigOp { op: *c, lower: vec![], upper: vec![] }
        }
        Node::Sym(_) | Node::Func(_) | Node::Accent { .. } => node.clone(),
        Node::Frac { num, den } => Node::Frac { num: normalize(num), den: normalize(den) },
        Node::Sqrt { arg, index } => Node::Sqrt { arg: normalize(arg), index: *index },
        Node::Sup { arg } => Node::Sup { arg: normalize(arg) },
        Node::Sub { arg } => Node::Sub { arg: normalize(arg) },
        Node::BigOp { op, lower, upper } => Node::BigOp {
            op: *op,
            lower: normalize(lower),
            upper: normalize(upper),
        },
        Node::Delim { left, right, mids, segs } => {
            let plain_bracket = Node::is_plain_bracket(*left, *right, mids);
            let segs = segs
                .iter()
                .map(|seg| {
                    // A sole Array stays a blank-gap grid body; Arrays in
                    // any other position render as self-delimiting ┼
                    // lattices and need no special casing.
                    let seg = match &seg[..] {
                        [Node::Array { rows, cols, cells }] => vec![Node::Array {
                            rows: *rows,
                            cols: *cols,
                            cells: cells.iter().map(normalize).collect(),
                        }],
                        _ => normalize(seg),
                    };
                    // Iterate to a fixpoint: splicing can surface a new
                    // sole Array (e.g. a 1×1 grid whose cell held a grid).
                    let mut seg = seg;
                    loop {
                        seg = match &seg[..] {
                            // Plain [ ]: the interior is always a grid.
                            [Node::Array { .. }] if plain_bracket => break seg,
                            _ if plain_bracket => {
                                break vec![Node::Array { rows: 1, cols: 1, cells: vec![seg] }]
                            }
                            // Elsewhere a 1×1 grid is the same picture as
                            // the cell itself rendered compact — canonical
                            // form is the plain row.
                            [Node::Array { rows: 1, cols: 1, cells }] => normalize(&cells[0]),
                            // A one-row grid has no separator row to prove
                            // its gridness in a blank-gap body; canonical
                            // form joins the cells with explicit ␣ spaces.
                            [Node::Array { rows: 1, cells, .. }] => {
                                let mut joined: Row = Vec::new();
                                for (i, cell) in cells.iter().enumerate() {
                                    if i > 0 {
                                        joined.push(Node::Sym('␣'));
                                    }
                                    joined.extend(cell.clone());
                                }
                                normalize(&joined)
                            }
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
