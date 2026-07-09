//! Serialize the AST to Typst math syntax. Typst math accepts Unicode
//! symbols verbatim, so only structures need translation.

use crate::ast::{Node, Row};
use crate::symbols::accent_info;

pub fn row_to_typst(row: &Row) -> String {
    // Adjacent digits form one number token ("27", not "2 7").
    let mut parts: Vec<String> = Vec::new();
    let mut prev_digit = false;
    for node in row {
        let digit = matches!(node, Node::Sym(c) if c.is_ascii_digit() || *c == '.');
        let s = node_to_typst(node);
        if digit && prev_digit {
            parts.last_mut().unwrap().push_str(&s);
        } else {
            parts.push(s);
        }
        prev_digit = digit;
    }
    parts.join(" ")
}

/// Group as a parenthesized argument when the row is not a single token.
fn grouped(row: &Row) -> String {
    let s = row_to_typst(row);
    if row.len() == 1 && matches!(row[0], Node::Sym(_)) {
        s
    } else {
        format!("({})", s)
    }
}

/// Typst accent function names.
fn accent_fn(mark: char) -> &'static str {
    match accent_info(mark) {
        Some((_, "bar")) => "macron",
        Some((_, "vec")) => "arrow",
        Some((_, "ddot")) => "dot.double",
        Some((_, "mathring")) => "circle",
        Some((_, "underline")) => "underline",
        Some((_, latex)) => match latex {
            "hat" => "hat",
            "tilde" => "tilde",
            "dot" => "dot",
            "check" => "caron",
            "breve" => "breve",
            _ => "hat",
        },
        None => "hat",
    }
}

fn node_to_typst(node: &Node) -> String {
    match node {
        Node::Sym(c) => match c {
            '\'' => "'".into(),
            c => c.to_string(),
        },
        Node::Func(name) => name.clone(),
        Node::Accent { accent, base } => {
            format!("{}({})", accent_fn(*accent), base)
        }
        Node::Frac { num, den } => format!("{}/{}", grouped(num), grouped(den)),
        Node::Sqrt { arg, index } => match index {
            2 => format!("sqrt({})", row_to_typst(arg)),
            i => format!("root({}, {})", i, row_to_typst(arg)),
        },
        Node::Sup { arg } => format!("^{}", grouped(arg)),
        Node::Sub { arg } => format!("_{}", grouped(arg)),
        Node::BigOp { op, lower, upper } => {
            let mut s = op.to_string();
            if !lower.is_empty() {
                s.push_str(&format!("_{}", grouped(lower)));
            }
            if !upper.is_empty() {
                s.push_str(&format!("^{}", grouped(upper)));
            }
            s
        }
        Node::Paren { inner } => format!("({})", row_to_typst(inner)),
        Node::Cancel { arg } => format!("cancel({})", row_to_typst(arg)),
        Node::Matrix { cols, cells, .. } => {
            let body = cells
                .chunks(*cols)
                .map(|row| row.iter().map(row_to_typst).collect::<Vec<_>>().join(", "))
                .collect::<Vec<_>>()
                .join("; ");
            format!("mat(delim: \"[\", {})", body)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_typst() {
        let root = vec![
            Node::Sym('x'),
            Node::Sym('='),
            Node::Frac {
                num: vec![Node::Sym('1')],
                den: vec![Node::Sym('n'), Node::Sup { arg: vec![Node::Sym('2')] }],
            },
        ];
        assert_eq!(row_to_typst(&root), "x = 1/(n ^2)");
    }
}
