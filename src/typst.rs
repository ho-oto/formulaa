//! Serialize the AST to Typst math syntax. Typst math accepts Unicode
//! symbols verbatim, so only structures need translation.

use crate::ast::{Node, Row};
use crate::symbols::accent_info;

pub fn row_to_typst(row: &Row) -> String {
    // Adjacent digits form one number token ("27", not "2 7").
    let mut parts: Vec<String> = Vec::new();
    let mut prev_digit = false;
    for node in row {
        if matches!(node, Node::Spacer) {
            prev_digit = false;
            continue;
        }
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
        Node::Spacer => String::new(),
        Node::Sym(c) => match c {
            '\'' => "'".into(),
            '␣' => "space".into(),
            c => c.to_string(),
        },
        Node::Func(name) => name.clone(),
        Node::Accent { overs, unders, base } => {
            let mut s = base.to_string();
            for &m in unders.iter().chain(overs.iter()) {
                s = format!("{}({})", accent_fn(m), s);
            }
            s
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
        Node::Text(t) => format!("\"{}\"", t),
        Node::Arrow { op, over, under } => {
            let head = match op {
                '←' => "<-",
                '⇒' => "=>",
                '⇐' => "<=",
                _ => "->",
            };
            let mut s = format!("attach(stretch({})", head);
            if !over.is_empty() {
                s.push_str(&format!(", t: ({})", row_to_typst(over)));
            }
            if !under.is_empty() {
                s.push_str(&format!(", b: ({})", row_to_typst(under)));
            }
            s.push(')');
            s
        }
        Node::Delim { left, right, mids, segs } => {
            if mids.is_empty() {
                if let [seg] = &segs[..] {
                    if let [Node::Array { cols, cells, .. }] = &seg[..] {
                        let body = mat_body(*cols, cells);
                        return match (left, right) {
                            ('{', '.') => {
                                // cases(): one argument per grid row.
                                let rows = cells
                                    .chunks(*cols)
                                    .map(|row| {
                                        row.iter()
                                            .map(row_to_typst)
                                            .collect::<Vec<_>>()
                                            .join(" & ")
                                    })
                                    .collect::<Vec<_>>()
                                    .join(", ");
                                format!("cases({})", rows)
                            }
                            ('(', ')') => format!("mat(delim: \"(\", {})", body),
                            ('[', ']') => format!("mat(delim: \"[\", {})", body),
                            ('{', '}') => format!("mat(delim: \"{{\", {})", body),
                            ('|', '|') => format!("mat(delim: \"|\", {})", body),
                            ('.', '.') => format!("mat(delim: #none, {})", body),
                            _ => format!(
                                "lr({} mat(delim: #none, {}) {})",
                                delim_typst(*left),
                                body,
                                delim_typst(*right)
                            ),
                        };
                    }
                }
            }
            let mut s = String::from("lr(");
            s.push_str(&delim_typst(*left));
            for (k, seg) in segs.iter().enumerate() {
                if k > 0 {
                    s.push_str(&format!(" mid({})", delim_typst(mids[k - 1])));
                }
                s.push(' ');
                s.push_str(&row_to_typst(seg));
            }
            s.push(' ');
            s.push_str(&delim_typst(*right));
            s.push(')');
            s
        }
        Node::Array { cols, cells, .. } => {
            format!("mat(delim: #none, {})", mat_body(*cols, cells))
        }
        Node::Cancel { arg } => format!("cancel({})", row_to_typst(arg)),
    }
}

fn mat_body(cols: usize, cells: &[Row]) -> String {
    cells
        .chunks(cols)
        .map(|row| row.iter().map(row_to_typst).collect::<Vec<_>>().join(", "))
        .collect::<Vec<_>>()
        .join("; ")
}

fn delim_typst(spec: char) -> String {
    match spec {
        '{' => "{".into(),
        '}' => "}".into(),
        '⟨' => "angle.l".into(),
        '⟩' => "angle.r".into(),
        '|' => "|".into(),
        '.' => "".into(),
        c => c.to_string(),
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
