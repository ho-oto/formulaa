//! Serialize the AST to a LaTeX string.
//!
//! Symbols with a curated LaTeX name become `\name`; other Unicode symbols
//! (from the large generated table) are emitted verbatim, which assumes a
//! unicode-math toolchain (lualatex/xelatex).

use crate::ast::{Node, Row};
use crate::symbols::accent_info;

pub fn row_to_latex(row: &Row) -> String {
    row.iter().map(node_to_latex).collect()
}

fn braced(row: &Row) -> String {
    format!("{{{}}}", row_to_latex(row))
}

fn sym_to_latex(c: char) -> String {
    if c == '␣' {
        return "\\ ".into();
    }
    // Ordinary characters that LaTeX reads as syntax (a raw % would
    // comment out the rest of the document).
    if matches!(c, '#' | '$' | '%' | '&') {
        return format!("\\{}", c);
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
            // order the compact Accent uses.
            let mut s = row_to_latex(base);
            for &m in unders.iter().chain(overs.iter()) {
                s = format!("\\{}{{{}}}", crate::symbols::wide_accent_latex(m), s);
            }
            s
        }
        Node::Accent {
            overs,
            unders,
            base,
        } => {
            // Canonical nesting: under-marks innermost, then over-marks.
            let mut s = sym_to_latex(*base).trim_end().to_string();
            for &m in unders.iter().chain(overs.iter()) {
                let cmd = accent_info(m).map(|(_, l)| l).unwrap_or("hat");
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
        Node::Text(t) => format!("\\text{{{}}}", t),
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
            let cmd = match op {
                '←' => "xleftarrow",
                '⇒' => "xRightarrow",
                '⇐' => "xLeftarrow",
                _ => "xrightarrow",
            };
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
            if mids.is_empty()
                && let [seg] = &segs[..]
                && let [Node::Array { cols, cells, .. }] = &seg[..]
            {
                let env = match (left, right) {
                    ('(', ')') => Some("pmatrix"),
                    ('[', ']') => Some("bmatrix"),
                    ('{', '}') => Some("Bmatrix"),
                    ('|', '|') => Some("vmatrix"),
                    ('‖', '‖') => Some("Vmatrix"),
                    ('.', '.') => Some("matrix"),
                    // cases is a two-column environment; wider grids fall
                    // through to \left\{ \begin{matrix} … \right.
                    ('{', '.') if *cols <= 2 => Some("cases"),
                    // mathtools' mirror image (\usepackage{mathtools}).
                    ('.', '}') if *cols <= 2 => Some("rcases"),
                    _ => None,
                };
                if let Some(env) = env {
                    return format!(
                        "\\begin{{{env}}} {} \\end{{{env}}}",
                        array_body(*cols, cells)
                    );
                }
            }
            let mut s = format!("\\left{}", delim_latex(*left));
            for (k, seg) in segs.iter().enumerate() {
                if k > 0 {
                    s.push_str(&format!("\\middle{}", delim_latex(mids[k - 1])));
                }
                s.push_str(&row_to_latex(seg));
            }
            s.push_str(&format!("\\right{}", delim_latex(*right)));
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

fn delim_latex(spec: char) -> String {
    match spec {
        '{' => "\\{".into(),
        '}' => "\\}".into(),
        '⟨' => "\\langle ".into(),
        '⟩' => "\\rangle ".into(),
        '⌈' => "\\lceil ".into(),
        '⌉' => "\\rceil ".into(),
        '⌊' => "\\lfloor ".into(),
        '⌋' => "\\rfloor ".into(),
        '‖' => "\\|".into(),
        '|' => "|".into(),
        '.' => ".".into(),
        c => c.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
                overs: vec!['⇀'],
                unders: vec![],
                base: 'v',
            },
            Node::Delim {
                left: '[',
                right: ']',
                mids: vec![],
                segs: vec![vec![Node::Array {
                    rows: 1,
                    cols: 2,
                    cells: vec![vec![Node::Sym('a')], vec![Node::Sym('b')]],
                }]],
            },
            Node::Sqrt {
                arg: vec![Node::Sym('x')],
                index: crate::ast::Radical::Cbrt,
            },
        ];
        assert_eq!(
            row_to_latex(&root),
            "\\operatorname{sin}\\vec{v}\\begin{bmatrix} a & b \\end{bmatrix}\\sqrt[3]{x}"
        );
    }
}
