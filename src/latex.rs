//! Serialize the AST to a LaTeX string.
//!
//! Symbols with a curated LaTeX name become `\name`; other Unicode symbols
//! (from the large generated table) are emitted verbatim, which assumes a
//! unicode-math toolchain (lualatex/xelatex or Typst-side conversion).

use crate::ast::{Node, Row};
use crate::symbols::{accent_info, is_func_name, latex_name};

pub fn row_to_latex(row: &Row) -> String {
    row.iter().map(node_to_latex).collect()
}

fn braced(row: &Row) -> String {
    format!("{{{}}}", row_to_latex(row))
}

fn sym_to_latex(c: char) -> String {
    match latex_name(c) {
        Some(name) => format!("\\{} ", name),
        None => c.to_string(),
    }
}

fn node_to_latex(node: &Node) -> String {
    match node {
        Node::Sym(c) => sym_to_latex(*c),
        Node::Func(name) => {
            if is_func_name(name) {
                format!("\\{} ", name)
            } else {
                format!("\\operatorname{{{}}}", name)
            }
        }
        Node::Accent { accent, base } => {
            let cmd = accent_info(*accent).map(|(_, l)| l).unwrap_or("hat");
            format!("\\{}{{{}}}", cmd, sym_to_latex(*base).trim_end())
        }
        Node::Frac { num, den } => format!("\\frac{}{}", braced(num), braced(den)),
        Node::Sqrt { arg, index } => match index {
            2 => format!("\\sqrt{}", braced(arg)),
            i => format!("\\sqrt[{}]{}", i, braced(arg)),
        },
        Node::Sup { arg } => format!("^{}", braced(arg)),
        Node::Sub { arg } => format!("_{}", braced(arg)),
        Node::BigOp { op, lower, upper } => {
            let mut s = match latex_name(*op) {
                Some(name) => format!("\\{} ", name),
                None => op.to_string(),
            };
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
        // Requires \usepackage{cancel}.
        Node::Cancel { arg } => format!("\\cancel{}", braced(arg)),
        Node::Matrix { cols, cells, .. } => {
            let body = cells
                .chunks(*cols)
                .map(|row| row.iter().map(row_to_latex).collect::<Vec<_>>().join(" & "))
                .collect::<Vec<_>>()
                .join(" \\\\ ");
            format!("\\begin{{bmatrix}} {} \\end{{bmatrix}}", body)
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

    #[test]
    fn serializes_matrix_func_accent() {
        let root = vec![
            Node::Func("sin".into()),
            Node::Accent { accent: '⇀', base: 'v' },
            Node::Matrix {
                rows: 1,
                cols: 2,
                cells: vec![vec![Node::Sym('a')], vec![Node::Sym('b')]],
            },
            Node::Sqrt { arg: vec![Node::Sym('x')], index: 3 },
        ];
        assert_eq!(
            row_to_latex(&root),
            "\\sin \\vec{v}\\begin{bmatrix} a & b \\end{bmatrix}\\sqrt[3]{x}"
        );
    }
}
