//! Serialize the AST to a LaTeX string.
//!
//! Every symbol the format can spell has a LaTeX name (curated in
//! `symbols::atoms`, or derived for a styled letter) — the "typeable
//! implies spellable" invariant, pinned by a test in `symbols`. The
//! raw-emit fallback below is therefore unreachable for atoms.

use crate::ast::{Node, Row};

pub fn row_to_latex(row: &Row) -> String {
    let mut s = String::new();
    for (i, n) in row.iter().enumerate() {
        // A node that could absorb the following script node — a
        // ∑-class atom (`\sum_{i}` means band limits), a band with that
        // limit still empty, a brace with no label yet — is braced, so
        // the script stays the separate side-script the picture shows.
        let next_script = match row.get(i + 1) {
            Some(Node::Sup { .. }) => Some(true),
            Some(Node::Sub { .. }) => Some(false),
            _ => None,
        };
        let absorbs = match n {
            Node::Sym(c) if crate::symbols::bigop_by_char(*c) => next_script.is_some(),
            Node::BigOpSym { lower, upper, .. } | Node::BigOp { lower, upper, .. } => {
                match next_script {
                    Some(true) => upper.is_empty(),
                    Some(false) => lower.is_empty(),
                    None => false,
                }
            }
            Node::Brace { over, label, .. } => label.is_empty() && next_script == Some(*over),
            _ => false,
        };
        if absorbs {
            s.push('{');
            s.push_str(node_to_latex(n).trim_end());
            s.push('}');
        } else {
            s.push_str(&node_to_latex(n));
        }
    }
    s
}

fn braced(row: &Row) -> String {
    format!("{{{}}}", row_to_latex(row))
}

/// Characters LaTeX reads as syntax wherever they appear — a raw % would
/// comment out the rest of the document, a stray { would swallow it.
fn escaped(c: char) -> Option<String> {
    match c {
        '#' | '$' | '%' | '&' | '{' | '}' | '_' => Some(format!("\\{}", c)),
        '\\' => Some("\\textbackslash{}".into()),
        _ => None,
    }
}

/// Text-mode run (`\text{…}`): the same escapes, everything else
/// verbatim.
fn text_to_latex(t: &str) -> String {
    t.chars()
        .map(|c| escaped(c).unwrap_or_else(|| c.to_string()))
        .collect()
}

fn sym_to_latex(c: char) -> String {
    if c == '␣' {
        return "\\ ".into();
    }
    if let Some(e) = escaped(c) {
        return e;
    }
    // A curated name, or a styled letter spelled through its family;
    // anything left over is emitted raw (unicode-math renders it).
    crate::symbols::latex_of(c).unwrap_or_else(|| c.to_string())
}

/// `base_{lower}^{upper}`, skipping empty limits.
fn limited(base: &str, lower: &Row, upper: &Row) -> String {
    let mut s = base.to_string();
    if !lower.is_empty() {
        s.push_str(&format!("_{}", braced(lower)));
    }
    if !upper.is_empty() {
        s.push_str(&format!("^{}", braced(upper)));
    }
    s
}

fn node_to_latex(node: &Node) -> String {
    match node {
        Node::Spacer => String::new(),
        // Line break of a multi-line formula (gather/aligned-style).
        Node::Break => " \\\\ ".into(),
        Node::Sym(c) => sym_to_latex(*c),
        // Every upright run is \operatorname: one spelling for
        // dictionary and ad-hoc names alike (\sin and \operatorname{sin}
        // typeset identically, and this keeps the mapping total).
        Node::Func(name) => format!(
            "\\operatorname{{{}}}",
            crate::symbols::func_latex_text(name)
        ),
        Node::WideAccent {
            overs,
            unders,
            base,
        } => {
            // Under-marks innermost, then over-marks — the same nesting
            // order the compact Accent uses. A base that is itself a
            // sole accent is braced: `\dot{{\hat{x}}}` is a band over
            // an accented block, `\dot{\hat{x}}` is the stacked marks
            // — without the braces the two would read alike.
            let sole_accent = matches!(base[..], [Node::Accent { .. } | Node::WideAccent { .. }]);
            let mut s = if sole_accent {
                format!("{{{}}}", row_to_latex(base))
            } else {
                row_to_latex(base)
            };
            for &m in unders.iter().chain(overs.iter()) {
                s = format!("\\{}{{{}}}", m.wide_latex(), s);
            }
            s
        }
        Node::Accent {
            overs,
            unders,
            base,
        } => {
            // Canonical nesting: under-marks innermost, then over-marks.
            // (The trim drops a \name's trailing space; the ␣ base is
            // exactly its trailing space, so it keeps it — trimming
            // would leave a bare backslash that eats the brace.)
            let mut s = if *base == '␣' {
                sym_to_latex(*base)
            } else {
                sym_to_latex(*base).trim_end().to_string()
            };
            for &m in unders.iter().chain(overs.iter()) {
                let cmd = m.latex();
                s = format!("\\{}{{{}}}", cmd, s);
            }
            s
        }
        Node::Frac { num, den } => format!("\\frac{}{}", braced(num), braced(den)),
        Node::Sqrt { arg, index } => match index.latex_index() {
            None => format!("\\sqrt{}", braced(arg)),
            Some(i) => format!("\\sqrt[{}]{}", i, braced(arg)),
        },
        Node::Norm { arg } => {
            // A norm around a sole grid is Vmatrix, like the other pairs.
            if let [Node::Array { cols, cells, .. }] = &arg[..] {
                return format!(
                    "\\begin{{Vmatrix}} {} \\end{{Vmatrix}}",
                    array_body(*cols, cells)
                );
            }
            format!("\\left\\|{}\\right\\|", row_to_latex(arg))
        }
        Node::Sup { arg } => format!("^{}", braced(arg)),
        Node::Sub { arg } => format!("_{}", braced(arg)),
        Node::BigOpSym { op, lower, upper } => limited(sym_to_latex(*op).trim_end(), lower, upper),
        // \operatorname* keeps the limits underneath, which is what the
        // band means (\operatorname{lim}_{x} would set them aside).
        Node::BigOp { name, lower, upper } => limited(
            &format!(
                "\\operatorname*{{{}}}",
                crate::symbols::func_latex_text(name)
            ),
            lower,
            upper,
        ),
        Node::Roman(c) => format!("\\mathrm{{{}}}", c),
        Node::Text(t) => format!("\\text{{{}}}", text_to_latex(t)),
        Node::Brace { over, arg, label } => {
            let (cmd, att) = if *over {
                ("overbrace", '^')
            } else {
                ("underbrace", '_')
            };
            let mut s = format!("\\{}{}", cmd, braced(arg));
            if !label.is_empty() {
                s.push_str(&format!("{}{}", att, braced(label)));
            }
            s
        }
        Node::Arrow { op, over, under } => {
            // \xRightarrow / \xLeftarrow need mathtools.
            let cmd = op.latex();
            let mut s = format!("\\{}", cmd);
            if !under.is_empty() {
                s.push_str(&format!("[{}]", row_to_latex(under)));
            }
            s.push_str(&braced(over));
            s
        }
        Node::Delim {
            left,
            right,
            mids,
            segs,
        } => {
            // Single grid seg with a well-known pair -> a matrix environment.
            if *mids == 0
                && let [seg] = &segs[..]
                && let [Node::Array { cols, cells, .. }] = &seg[..]
            {
                let env = crate::editor::grid_env_name(*left, *right, *cols);
                if let Some(env) = env {
                    return format!(
                        "\\begin{{{env}}} {} \\end{{{env}}}",
                        array_body(*cols, cells)
                    );
                }
            }
            let mut s = format!("\\left{}", left.latex(true));
            for (k, seg) in segs.iter().enumerate() {
                if k > 0 {
                    // Middles are always the vertical bar.
                    s.push_str("\\middle|");
                }
                s.push_str(&row_to_latex(seg));
            }
            s.push_str(&format!("\\right{}", right.latex(false)));
            s
        }
        Node::Array { cols, cells, .. } => {
            format!(
                "\\begin{{matrix}} {} \\end{{matrix}}",
                array_body(*cols, cells)
            )
        }
        // Requires \usepackage{cancel}.
        Node::Cancel { arg } => format!("\\cancel{}", braced(arg)),
    }
}

fn array_body(cols: usize, cells: &[Row]) -> String {
    cells
        .chunks(cols)
        .map(|row| row.iter().map(row_to_latex).collect::<Vec<_>>().join(" & "))
        .collect::<Vec<_>>()
        .join(" \\\\ ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::symbols::Accent;

    #[test]
    fn serializes_basic_formula() {
        let root = vec![
            Node::Sym('x'),
            Node::Sup {
                arg: vec![Node::Sym('2')],
            },
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
        let root = vec![Node::BigOpSym {
            op: '∑',
            lower: vec![Node::Sym('i')],
            upper: vec![Node::Sym('n')],
        }];
        assert_eq!(row_to_latex(&root), "\\sum_{i}^{n}");
    }

    #[test]
    fn serializes_matrix_func_accent() {
        let root = vec![
            Node::Func("sin".into()),
            Node::Accent {
                overs: vec![Accent::Vec],
                unders: vec![],
                base: 'v',
            },
            Node::Delim {
                left: crate::symbols::Delim::Col(crate::symbols::ColDelim::Bracket),
                right: crate::symbols::Delim::Col(crate::symbols::ColDelim::Bracket),
                mids: 0,
                segs: vec![vec![Node::Array {
                    rows: 1,
                    cols: 2,
                    cells: vec![vec![Node::Sym('a')], vec![Node::Sym('b')]],
                }]],
            },
            Node::Sqrt {
                arg: vec![Node::Sym('x')],
                index: crate::symbols::Radical::Cbrt,
            },
        ];
        assert_eq!(
            row_to_latex(&root),
            "\\operatorname{sin}\\vec{v}\\begin{bmatrix} a & b \\end{bmatrix}\\sqrt[3]{x}"
        );
    }
}
