//! Serialize the AST to a LaTeX string.
//!
//! Symbols with a curated LaTeX name become `\name`; other Unicode symbols
//! (from the large generated table) are emitted verbatim, which assumes a
//! unicode-math toolchain (lualatex/xelatex or Typst-side conversion).

use crate::ast::{Node, Row};
use crate::symbols::{accent_info, latex_name};

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
    match latex_name(c) {
        Some(name) => format!("\\{} ", name),
        None => c.to_string(),
    }
}

fn node_to_latex(node: &Node) -> String {
    match node {
        Node::Spacer => String::new(),
        // Line break of a multi-line formula (gather/aligned-style).
        Node::Break => " \\\\ ".into(),
        Node::Sym(c) => sym_to_latex(*c),
        Node::Func(name) => {
            if crate::symbols::latex_knows_func(name) {
                format!("\\{} ", name)
            } else {
                format!("\\operatorname{{{}}}", name)
            }
        }
        Node::WideAccent { over, under, base } => {
            let mut s = row_to_latex(base);
            for m in over.iter().chain(under.iter()) {
                s = format!("\\{}{{{}}}", crate::symbols::wide_accent_latex(*m), s);
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
        Node::Sqrt { arg, index } => match index {
            2 => format!("\\sqrt{}", braced(arg)),
            i => format!("\\sqrt[{}]{}", i, braced(arg)),
        },
        Node::Sup { arg } => format!("^{}", braced(arg)),
        Node::Sub { arg } => format!("_{}", braced(arg)),
        Node::BigOp { base, lower, upper } => {
            // \sum_{..}^{..}, \lim_{..}, \arg\max_{..} — the base row's
            // own serialization already yields the operator commands.
            // Word pieces with a Text among them are an \op* name
            // (┈ess┈sup┈): \operatorname* keeps the limits underneath
            // (plain \mathrm would set them aside).
            let mut s = match crate::ast::op_words(base) {
                Some((ws, true)) => {
                    format!("\\operatorname*{{{}}}", ws.join(" "))
                }
                // ┈lim┈sup┈ is \limsup where LaTeX knows the joined
                // name; other multi-piece bases (┈arg┈min┈) group under
                // \mathop so the limits center under the whole thing —
                // \arg \min_{θ} would hang θ under \min alone.
                Some((ws, false)) if crate::symbols::word_op_of(&ws).is_some_and(|w| w.latex) => {
                    format!("\\{}", crate::symbols::word_op_of(&ws).unwrap().name)
                }
                _ if base.len() > 1 => format!("\\mathop{{{}}}", row_to_latex(base).trim_end()),
                _ => row_to_latex(base),
            };
            if !lower.is_empty() {
                s.push_str(&format!("_{}", braced(lower)));
            }
            if !upper.is_empty() {
                s.push_str(&format!("^{}", braced(upper)));
            }
            s
        }
        Node::Text { t, math } => {
            if *math {
                format!("\\mathrm{{{}}}", t)
            } else {
                format!("\\text{{{}}}", t)
            }
        }
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
        let root = vec![Node::BigOp {
            base: vec![Node::Sym('∑')],
            lower: vec![Node::Sym('i')],
            upper: vec![Node::Sym('n')],
        }];
        assert_eq!(row_to_latex(&root), "\\sum _{i}^{n}");
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
                index: 3,
            },
        ];
        assert_eq!(
            row_to_latex(&root),
            "\\sin \\vec{v}\\begin{bmatrix} a & b \\end{bmatrix}\\sqrt[3]{x}"
        );
    }
}
