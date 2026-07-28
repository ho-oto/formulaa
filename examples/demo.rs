//! Prints a few formulas rendered by the layout engine, without the TUI.
//! Run with: cargo run --example demo

use mascii::ast::{Node, Radical, Row};
use mascii::latex;
use mascii::render::{RenderCtx, render_row};

fn syms(s: &str) -> Row {
    s.chars().map(Node::Sym).collect()
}

fn show(title: &str, row: &Row) {
    println!("== {} ==", title);
    println!("LaTeX: {}", latex::row_to_latex(row));
    let block = render_row(row, None, false, &RenderCtx::canonical());
    for line in block.to_strings() {
        println!("  {}", line);
    }
    println!();
}

fn main() {
    // Gaussian integral: ∫_{-∞}^{∞} e^{-x²} dx = √π
    let gaussian = vec![
        Node::BigOpSym {
            op: '∫',
            lower: vec![Node::Sym('-'), Node::Sym('∞')],
            upper: vec![Node::Sym('∞')],
        },
        Node::Sym('e'),
        Node::Sup {
            arg: vec![Node::Sym('-'), Node::Sym('x'), Node::Sup { arg: syms("2") }],
        },
        Node::Sym('d'),
        Node::Sym('x'),
        Node::Sym('='),
        Node::Sqrt {
            arg: vec![Node::Sym('π')],
            index: Radical::Sqrt,
        },
    ];
    show("Gaussian integral", &gaussian);

    // Quadratic formula: x = (-b ± √(b²-4ac)) / 2a
    let quadratic = vec![
        Node::Sym('x'),
        Node::Sym('='),
        Node::Frac {
            num: vec![
                Node::Sym('-'),
                Node::Sym('b'),
                Node::Sym('±'),
                Node::Sqrt {
                    index: Radical::Sqrt,
                    arg: vec![
                        Node::Sym('b'),
                        Node::Sup { arg: syms("2") },
                        Node::Sym('-'),
                        Node::Sym('4'),
                        Node::Sym('a'),
                        Node::Sym('c'),
                    ],
                },
            ],
            den: syms("2a"),
        },
    ];
    show("Quadratic formula", &quadratic);

    // Basel problem: ∑_{n=1}^{∞} 1/n² = π²/6
    let basel = vec![
        Node::BigOpSym {
            op: '∑',
            lower: syms("n=1"),
            upper: vec![Node::Sym('∞')],
        },
        Node::Frac {
            num: syms("1"),
            den: vec![Node::Sym('n'), Node::Sup { arg: syms("2") }],
        },
        Node::Sym('='),
        Node::Frac {
            num: vec![Node::Sym('π'), Node::Sup { arg: syms("2") }],
            den: syms("6"),
        },
    ];
    show("Basel problem", &basel);

    // Nested: f(x) = (1 + 1/(1 + 1/x))
    let nested = vec![
        Node::Sym('f'),
        Node::Delim {
            left: mascii::symbols::Delim::Paren,
            right: mascii::symbols::Delim::Paren,
            mids: 0,
            segs: vec![syms("x")],
        },
        Node::Sym('='),
        Node::Delim {
            left: mascii::symbols::Delim::Paren,
            right: mascii::symbols::Delim::Paren,
            mids: 0,
            segs: vec![vec![
                Node::Sym('1'),
                Node::Sym('+'),
                Node::Frac {
                    num: syms("1"),
                    den: vec![
                        Node::Sym('1'),
                        Node::Sym('+'),
                        Node::Frac {
                            num: syms("1"),
                            den: syms("x"),
                        },
                    ],
                },
            ]],
        },
    ];
    show("Nested fractions in parens", &nested);
}
