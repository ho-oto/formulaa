//! Generates docs/examples.md: the roundtrip corpus rendered as AA with
//! its LaTeX and Typst translations.
//! Regenerate with: cargo run --example catalog > docs/examples.md
//! (Formula definitions mirror tests/roundtrip.rs.)

#![allow(dead_code)]
use mascii::ast::{normalize, Node, Row};
use mascii::latex::row_to_latex;
use mascii::render::{render_row, RenderCtx};
use mascii::typst::row_to_typst;

// ----- tiny DSL for building formulas -----

fn s(t: &str) -> Row {
    t.chars().map(Node::Sym).collect()
}

fn n(node: Node) -> Row {
    vec![node]
}

fn frac(num: Row, den: Row) -> Node {
    Node::Frac { num, den }
}

fn sqrt(arg: Row) -> Node {
    Node::Sqrt { arg, index: 2 }
}

fn cbrt(arg: Row) -> Node {
    Node::Sqrt { arg, index: 3 }
}

fn sup(arg: Row) -> Node {
    Node::Sup { arg }
}

fn sub(arg: Row) -> Node {
    Node::Sub { arg }
}

fn paren(inner: Row) -> Node {
    Node::Paren { inner }
}

fn bigop(op: char, lower: Row, upper: Row) -> Node {
    Node::BigOp { op, lower, upper }
}

fn func(name: &str) -> Node {
    Node::Func(name.into())
}

fn acc(accent: char, base: char) -> Node {
    Node::Accent { accent, base }
}

fn mat(rows: usize, cols: usize, cells: Vec<Row>) -> Node {
    assert_eq!(cells.len(), rows * cols);
    Node::Matrix { rows, cols, cells }
}

fn cancel(arg: Row) -> Node {
    Node::Cancel { arg }
}

fn cat(parts: &[Row]) -> Row {
    parts.concat()
}

// ----- roundtrip machinery -----

fn roundtrip(name: &str, row: &Row) {
    let row = normalize(row);
    let ctx = RenderCtx::canonical();
    let aa = render_row(&row, None, false, &ctx).to_text();
    println!("### {}\n", name);
    println!("```\n{}\n```\n", aa);
    println!("LaTeX:\n\n```latex\n{}\n```\n", row_to_latex(&row));
    println!("Typst:\n\n```typst\n{}\n```\n", row_to_typst(&row));
}

// ----- MDN: three famous mathematical formulas -----

/// ∛(−q/2 + √(q²/4 + p³/27)) + ∛(−q/2 − √(q²/4 + p³/27))
fn cardano_formula() {
    let discriminant = sqrt(cat(&[
        n(frac(cat(&[s("q"), n(sup(s("2")))]), s("4"))),
        s("+"),
        n(frac(cat(&[s("p"), n(sup(s("3")))]), s("27"))),
    ]));
    let half_q = |sign: &str| {
        cat(&[
            s(sign),
            n(frac(s("q"), s("2"))),
            s(if sign == "-" { "+" } else { "-" }),
            n(discriminant.clone()),
        ])
    };
    let row = cat(&[
        s("t="),
        n(cbrt(half_q("-"))),
        s("+"),
        n(cbrt(half_q(""))),
    ]);
    roundtrip("cardano", &row);
}

/// (∑_{k=1}^{n} u_k v̄_k)² ≤ (∑_{k=1}^{n} u_k²)(∑_{k=1}^{n} v_k²)
/// (MDN's Cauchy–Bunyakovsky–Schwarz inequality; |·| written with parens
/// since vertical-bar delimiters are not supported yet.)
fn cauchy_schwarz_inequality() {
    let sum = |body: Row| n(bigop('∑', s("k=1"), s("n"))).into_iter().chain(body).collect::<Row>();
    let row = cat(&[
        n(paren(sum(cat(&[
            s("u"),
            n(sub(s("k"))),
            n(acc('¯', 'v')),
            n(sub(s("k"))),
        ])))),
        n(sup(s("2"))),
        s("≤"),
        n(paren(sum(cat(&[s("u"), n(sub(s("k"))), n(sup(s("2")))])))),
        n(paren(sum(cat(&[s("v"), n(sub(s("k"))), n(sup(s("2")))])))),
    ]);
    roundtrip("cauchy-schwarz", &row);
}

/// Vandermonde determinant (matrix with ⋮ ⋱ ⋯ cells) = ∏_{1≤i<j≤n}(x_j−x_i)
fn vandermonde_determinant() {
    let x = |i: &str, p: Option<&str>| -> Row {
        let mut row = cat(&[s("x"), n(sub(s(i)))]);
        if let Some(p) = p {
            row.push(sup(s(p)));
        }
        row
    };
    let m = mat(
        4,
        5,
        vec![
            s("1"), x("1", None), x("1", Some("2")), s("⋯"), x("1", Some("n-1")),
            s("1"), x("2", None), x("2", Some("2")), s("⋯"), x("2", Some("n-1")),
            s("⋮"), s("⋮"), s("⋮"), s("⋱"), s("⋮"),
            s("1"), x("n", None), x("n", Some("2")), s("⋯"), x("n", Some("n-1")),
        ],
    );
    let row = cat(&[
        n(m),
        s("="),
        n(bigop('∏', s("1≤i<j≤n"), vec![])),
        n(paren(cat(&[
            s("x"),
            n(sub(s("j"))),
            s("-"),
            s("x"),
            n(sub(s("i"))),
        ]))),
    ]);
    roundtrip("vandermonde", &row);
}

// ----- physics -----

/// ∫_{−∞}^{∞} e^{−x²} dx = √π
fn gaussian_integral() {
    let row = cat(&[
        n(bigop('∫', s("-∞"), s("∞"))),
        s("e"),
        n(sup(cat(&[s("-x"), n(sup(s("2")))]))),
        s("dx="),
        n(sqrt(s("π"))),
    ]);
    roundtrip("gaussian", &row);
}

/// iℏ ∂Ψ/∂t = −(ℏ²/2m) ∂²Ψ/∂x² + V(x)Ψ
fn schroedinger_equation() {
    let row = cat(&[
        s("iℏ"),
        n(frac(s("∂Ψ"), s("∂t"))),
        s("=-"),
        n(frac(cat(&[s("ℏ"), n(sup(s("2")))]), s("2m"))),
        n(frac(
            cat(&[s("∂"), n(sup(s("2"))), s("Ψ")]),
            cat(&[s("∂x"), n(sup(s("2")))]),
        )),
        s("+V"),
        n(paren(s("x"))),
        s("Ψ"),
    ]);
    roundtrip("schroedinger", &row);
}

/// ∮ E⃗ ⋅ dA⃗ = Q/ε₀   (Gauss's law, with ⇀ accents)
fn gauss_law() {
    let row = cat(&[
        n(bigop('∮', vec![], vec![])),
        n(acc('⇀', 'E')),
        s("⋅d"),
        n(acc('⇀', 'A')),
        s("="),
        n(frac(s("Q"), cat(&[s("ε"), n(sub(s("0")))]))),
    ]);
    roundtrip("gauss-law", &row);
}

/// f(a) = (1/2πi) ∮ f(z)/(z−a) dz   (Cauchy integral formula)
fn cauchy_integral_formula() {
    let row = cat(&[
        s("f"),
        n(paren(s("a"))),
        s("="),
        n(frac(s("1"), s("2πi"))),
        n(bigop('∮', vec![], vec![])),
        n(frac(cat(&[s("f"), n(paren(s("z")))]), cat(&[s("z-a")]))),
        s("dz"),
    ]);
    roundtrip("cauchy-integral", &row);
}

/// e^{iπ} + 1 = 0
fn euler_identity() {
    let row = cat(&[s("e"), n(sup(s("iπ"))), s("+1=0")]);
    roundtrip("euler", &row);
}

// ----- statistics -----

/// f(x) = (1/√(2πσ²)) e^{−(x−μ)²/(2σ²)}   (normal distribution PDF)
fn normal_pdf() {
    let row = cat(&[
        s("f"),
        n(paren(s("x"))),
        s("="),
        n(frac(
            s("1"),
            n(sqrt(cat(&[s("2πσ"), n(sup(s("2")))]))),
        )),
        s("e"),
        n(sup(cat(&[
            s("-"),
            n(frac(
                cat(&[n(paren(s("x-μ"))), n(sup(s("2")))]),
                cat(&[s("2σ"), n(sup(s("2")))]),
            )),
        ]))),
    ]);
    roundtrip("normal-pdf", &row);
}

/// σ² = (1/n) ∑_{i=1}^{n} (x_i − μ)²
fn variance() {
    let row = cat(&[
        s("σ"),
        n(sup(s("2"))),
        s("="),
        n(frac(s("1"), s("n"))),
        n(bigop('∑', s("i=1"), s("n"))),
        n(paren(cat(&[s("x"), n(sub(s("i"))), s("-μ")]))),
        n(sup(s("2"))),
    ]);
    roundtrip("variance", &row);
}

/// P(A|B) = P(B|A)P(A) / P(B)
fn bayes_theorem() {
    let p = |arg: &str| cat(&[s("P"), n(paren(s(arg)))]);
    let row = cat(&[
        p("A|B"),
        s("="),
        n(frac(cat(&[p("B|A"), p("A")]), p("B"))),
    ]);
    roundtrip("bayes", &row);
}

// ----- structural stress tests -----

/// Rotation matrix with upright function names.
fn rotation_matrix() {
    let row = cat(&[
        s("R="),
        n(mat(
            2,
            2,
            vec![
                cat(&[n(func("cos")), s("θ")]),
                cat(&[s("-"), n(func("sin")), s("θ")]),
                cat(&[n(func("sin")), s("θ")]),
                cat(&[n(func("cos")), s("θ")]),
            ],
        )),
    ]);
    roundtrip("rotation", &row);
}

/// Matrix inside a superscript: e^{Jt} with J spelled out.
fn matrix_exponential() {
    let row = cat(&[
        s("e"),
        n(sup(cat(&[
            n(mat(2, 2, vec![s("0"), s("1"), s("-1"), s("0")])),
            s("t"),
        ]))),
    ]);
    roundtrip("matrix-exponential", &row);
}

/// Matrix whose cells are fractions and nested matrices.
fn nested_matrices() {
    let inner = mat(2, 2, vec![s("a"), s("b"), s("c"), s("d")]);
    let row = n(mat(
        2,
        2,
        vec![
            n(frac(s("1"), s("2"))),
            s("0"),
            n(inner),
            cat(&[s("x"), n(sup(s("2")))]),
        ],
    ));
    roundtrip("nested-matrices", &row);
}

/// Cancellation: (x̸ y̸ / y̸ z̸) style strikes, including a struck fraction.
fn cancel_strikes() {
    // x·y/y = x with the y's cancelled
    let row = cat(&[
        n(frac(
            cat(&[s("x"), n(cancel(s("y")))]),
            n(cancel(s("y"))),
        )),
        s("="),
        s("x"),
    ]);
    roundtrip("cancel-simple", &row);
    // cancel over a whole fraction, next to an uncancelled sibling
    let row = cat(&[
        n(cancel(cat(&[n(frac(s("a+b"), s("c"))), s("d")]))),
        s("+"),
        s("e"),
    ]);
    roundtrip("cancel-frac", &row);
    // cancel inside a superscript
    let row = cat(&[
        s("e"),
        n(sup(cat(&[s("x"), n(cancel(s("2α")))]))),
    ]);
    roundtrip("cancel-in-sup", &row);
}

/// Continued fraction (deep vertical nesting).
fn continued_fraction() {
    let mut row = s("x");
    for _ in 0..4 {
        row = cat(&[s("1+"), n(frac(s("1"), row))]);
    }
    roundtrip("continued-fraction", &row);
}

/// Limits that themselves contain big operators and fractions.
fn nested_limits() {
    let row = cat(&[
        n(bigop(
            '∑',
            cat(&[s("i∈"), n(bigop('⋃', s("k"), vec![])), s("S"), n(sub(s("k")))]),
            n(frac(s("n"), s("2"))),
        )),
        s("a"),
        n(sub(s("i"))),
    ]);
    roundtrip("nested-limits", &row);
}


fn main() {
    println!("# 数式コーパス カタログ\n");
    println!("`tests/roundtrip.rs` のラウンドトリップ検証済み数式の対照表。");
    println!("`cargo run --example catalog > docs/examples.md` で再生成する。\n");
    println!("すべての式で `parse(render(normalize(x))) == normalize(x)` と");
    println!("`render(parse(aa)) == aa` が成立している(AA はそのまま `mascii aa2tex` に入力可能)。\n");
    println!("## MDN「三つの有名な数式」\n");
    cardano_formula();
    cauchy_schwarz_inequality();
    vandermonde_determinant();
    println!("## 物理\n");
    gaussian_integral();
    schroedinger_equation();
    gauss_law();
    cauchy_integral_formula();
    euler_identity();
    println!("## 統計\n");
    normal_pdf();
    variance();
    bayes_theorem();
    println!("## 構造ストレステスト\n");
    rotation_matrix();
    matrix_exponential();
    nested_matrices();
    cancel_strikes();
    continued_fraction();
    nested_limits();
}
