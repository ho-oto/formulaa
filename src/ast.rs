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
    Frac { num: Row, den: Row },
    Sqrt { arg: Row },
    Sup { arg: Row },
    Sub { arg: Row },
    /// Big operator (∑ ∏ ∫ …) with optional limits typeset under/over it.
    BigOp { op: char, lower: Row, upper: Row },
    /// Auto-scaling round brackets.
    Paren { inner: Row },
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
}

impl Node {
    /// Editable fields in cursor-traversal order (empty for atoms).
    pub fn fields(&self) -> &'static [Field] {
        match self {
            Node::Sym(_) => &[],
            Node::Frac { .. } => &[Field::FracNum, Field::FracDen],
            Node::Sqrt { .. } => &[Field::SqrtArg],
            Node::Sup { .. } => &[Field::SupArg],
            Node::Sub { .. } => &[Field::SubArg],
            Node::BigOp { .. } => &[Field::OpLower, Field::OpUpper],
            Node::Paren { .. } => &[Field::ParenInner],
        }
    }

    pub fn field(&self, f: Field) -> &Row {
        match (self, f) {
            (Node::Frac { num, .. }, Field::FracNum) => num,
            (Node::Frac { den, .. }, Field::FracDen) => den,
            (Node::Sqrt { arg }, Field::SqrtArg) => arg,
            (Node::Sup { arg }, Field::SupArg) => arg,
            (Node::Sub { arg }, Field::SubArg) => arg,
            (Node::BigOp { lower, .. }, Field::OpLower) => lower,
            (Node::BigOp { upper, .. }, Field::OpUpper) => upper,
            (Node::Paren { inner }, Field::ParenInner) => inner,
            _ => panic!("field {:?} does not belong to node {:?}", f, self),
        }
    }

    pub fn field_mut(&mut self, f: Field) -> &mut Row {
        match (self, f) {
            (Node::Frac { num, .. }, Field::FracNum) => num,
            (Node::Frac { den, .. }, Field::FracDen) => den,
            (Node::Sqrt { arg }, Field::SqrtArg) => arg,
            (Node::Sup { arg }, Field::SupArg) => arg,
            (Node::Sub { arg }, Field::SubArg) => arg,
            (Node::BigOp { lower, .. }, Field::OpLower) => lower,
            (Node::BigOp { upper, .. }, Field::OpUpper) => upper,
            (Node::Paren { inner }, Field::ParenInner) => inner,
            (node, f) => panic!("field {:?} does not belong to node {:?}", f, node),
        }
    }

    pub fn is_empty_structure(&self) -> bool {
        !self.fields().is_empty() && self.fields().iter().all(|&f| self.field(f).is_empty())
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
