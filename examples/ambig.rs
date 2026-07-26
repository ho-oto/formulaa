//! Regression demo for the over/under-limit ambiguity.
//!
//! Before the ┈-band notation, these two different ASTs rendered to the
//! same picture, so no parser could invert the output. The band makes the
//! horizontal extent of limits explicit and the two now differ.
//! Run: cargo run --example ambig

use mascii::ast::{Node, Row};
use mascii::parse::parse;
use mascii::render::{RenderCtx, render_row};

fn syms(s: &str) -> Row {
    s.chars().map(Node::Sym).collect()
}

fn show(title: &str, row: &Row) -> String {
    let text = render_row(row, None, false, &RenderCtx::canonical()).to_text();
    println!("{}:", title);
    for l in text.lines() {
        println!("  |{}|", l);
    }
    println!();
    text
}

fn main() {
    // AST 1: sum with upper limit containing an integral (no limits)
    let ast1 = vec![Node::BigOpSym {
        op: '∑',
        lower: syms("n=1"),
        upper: vec![Node::BigOpSym {
            op: '∫',
            lower: vec![],
            upper: vec![],
        }],
    }];

    // AST 2: integral whose LOWER limit is a sum with lower limit n=1
    let ast2 = vec![Node::BigOpSym {
        op: '∫',
        lower: vec![Node::BigOpSym {
            op: '∑',
            lower: syms("n=1"),
            upper: vec![],
        }],
        upper: vec![],
    }];

    let a = show("AST1  \\sum_{n=1}^{\\int}", &ast1);
    let b = show("AST2  \\int_{\\sum_{n=1}}", &ast2);

    assert_ne!(a, b, "the band notation must keep these distinguishable");
    assert_eq!(parse(&a).unwrap(), ast1);
    assert_eq!(parse(&b).unwrap(), ast2);
    println!("distinct pictures, both parse back to their own AST ✓");
}
