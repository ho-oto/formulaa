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
    /// Auto-scaling round brackets.
    Paren { inner: Row },
    /// rows×cols matrix, cells stored row-major.
    Matrix { rows: usize, cols: usize, cells: Vec<Row> },
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
    ParenInner,
    /// Row-major cell index of a Matrix.
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
            Node::Paren { .. } => vec![Field::ParenInner],
            Node::Matrix { cells, .. } => (0..cells.len()).map(Field::Cell).collect(),
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
            (Node::Paren { inner }, Field::ParenInner) => inner,
            (Node::Matrix { cells, .. }, Field::Cell(i)) => &cells[i],
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
            (Node::Paren { inner }, Field::ParenInner) => inner,
            (Node::Matrix { cells, .. }, Field::Cell(i)) => &mut cells[i],
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
            _ => out.push(node),
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
        Node::Paren { inner } => Node::Paren { inner: normalize(inner) },
        Node::Matrix { rows, cols, cells } => Node::Matrix {
            rows: *rows,
            cols: *cols,
            cells: cells.iter().map(normalize).collect(),
        },
    }
}
