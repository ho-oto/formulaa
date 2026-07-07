//! Serialize the AST to a LaTeX string (the persistent representation).

use crate::ast::{Node, Row};
use crate::symbols::latex_name;

pub fn row_to_latex(row: &Row) -> String {
    row.iter().map(node_to_latex).collect()
}

fn braced(row: &Row) -> String {
    format!("{{{}}}", row_to_latex(row))
}

fn node_to_latex(node: &Node) -> String {
    match node {
        Node::Sym(c) => match latex_name(*c) {
            Some(name) => format!("\\{} ", name),
            None => c.to_string(),
        },
        Node::Frac { num, den } => format!("\\frac{}{}", braced(num), braced(den)),
        Node::Sqrt { arg } => format!("\\sqrt{}", braced(arg)),
        Node::Sup { arg } => format!("^{}", braced(arg)),
        Node::Sub { arg } => format!("_{}", braced(arg)),
        Node::BigOp { op, lower, upper } => {
            let name = latex_name(*op).unwrap_or("sum");
            let mut s = format!("\\{} ", name);
            if !lower.is_empty() {
                s.push_str(&format!("_{}", braced(lower)));
            }
            if !upper.is_empty() {
                s.push_str(&format!("^{}", braced(upper)));
            }
            s
        }
        Node::Paren { inner } => {
            format!("\\left({}\\right)", row_to_latex(inner))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_basic_formula() {
        let root = vec![
            Node::Sym('x'),
            Node::Sup { arg: vec![Node::Sym('2')] },
            Node::Sym('+'),
            Node::Frac {
                num: vec![Node::Sym('α')],
                den: vec![Node::Sym('2')],
            },
        ];
        assert_eq!(row_to_latex(&root), "x^{2}+\\frac{\\alpha }{2}");
    }

    #[test]
    fn serializes_bigop_limits() {
        let root = vec![Node::BigOp {
            op: '∑',
            lower: vec![Node::Sym('i')],
            upper: vec![Node::Sym('n')],
        }];
        assert_eq!(row_to_latex(&root), "\\sum _{i}^{n}");
    }
}
