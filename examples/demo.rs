//! The library in one page: AA in, AA and LaTeX out, and the LaTeX
//! road back. Run with: cargo run --example demo

use formulaa::ast::{Row, normalize};
use formulaa::render::{RenderCtx, render_root};
use formulaa::{from_latex, latex, parse};

/// A formula as the tools spell it: the canonical picture, then LaTeX.
fn show(title: &str, row: &Row) {
    println!("== {} ==", title);
    for line in render_root(row, None, &RenderCtx::canonical()).to_strings() {
        println!("  {}", line);
    }
    println!("  LaTeX: {}\n", latex::row_to_latex(row));
}

fn main() {
    // 1. AA -> AST. Input may be hand-written — plain ASCII letters
    //    for the math italics, loose spacing; the render below answers
    //    in canonical form, which is what `formulaa --format` writes.
    let row = parse::parse("x²+2x+1 = (x+1)²").expect("hand-written AA");
    show("parsed from hand-written AA", &row);

    // 2. A 2D picture is ordinary text too — this is the source, not a
    //    rendering of some hidden markup.
    let aa = " ∞    -𝑥²   ┌─\n┈∫┈┈ 𝑒   𝑑𝑥=√π\n -∞";
    let row = parse::parse(aa).expect("canonical AA");
    show("parsed from a 2D picture", &row);

    // …and it reads back to exactly the tree that drew it — the
    // contract the editor re-checks after every keystroke.
    let redrawn = render_root(&row, None, &RenderCtx::canonical()).to_text();
    assert_eq!(parse::parse(&redrawn).unwrap(), normalize(&row));

    // 3. The other road: LaTeX -> AST -> picture (best effort for
    //    outside dialects; everything `--aa2latex` emits comes back whole).
    let row = from_latex::row_from_latex(r"\frac{-b\pm\sqrt{b^2-4ac}}{2a}");
    show("parsed from LaTeX", &normalize(&row));
}
