//! AA <-> AST roundtrip tests on realistic formulas from mathematics,
//! physics and statistics, plus a randomized property test.
//!
//! Invariant: for any AST x,
//!     parse(render(normalize(x))) == normalize(x)
//!     render(parse(aa)) == aa           for canonical aa
//!
//! The corpus includes the three formulas from MDN's MathML tutorial
//! "Three famous mathematical formulas" (Cardano, Cauchy–Schwarz,
//! Vandermonde determinant).

use mascii::ast::{normalize, Node, Row};
use mascii::latex::row_to_latex;
use mascii::parse::parse;
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

fn delim(left: char, right: char, mids: Vec<char>, segs: Vec<Row>) -> Node {
    Node::Delim { left, right, mids, segs }
}

fn paren(inner: Row) -> Node {
    delim('(', ')', vec![], vec![inner])
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

fn array(rows: usize, cols: usize, cells: Vec<Row>) -> Node {
    assert_eq!(cells.len(), rows * cols);
    Node::Array { rows, cols, cells }
}

/// A [ ] matrix (grid wrapped in brackets, the old Matrix node).
fn mat(rows: usize, cols: usize, cells: Vec<Row>) -> Node {
    delim('[', ']', vec![], vec![vec![array(rows, cols, cells)]])
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
    let parsed = parse(&aa)
        .unwrap_or_else(|e| panic!("[{}] parse failed: {}\n--- AA ---\n{}", name, e, aa));
    assert_eq!(
        parsed, row,
        "[{}] AST mismatch\n--- AA ---\n{}\n--- LaTeX (expected) ---\n{}\n--- LaTeX (parsed) ---\n{}",
        name,
        aa,
        row_to_latex(&row),
        row_to_latex(&parsed)
    );
    let aa2 = render_row(&parsed, None, false, &ctx).to_text();
    assert_eq!(aa2, aa, "[{}] re-render mismatch", name);
    // Exports must not panic and must be non-empty for non-empty input.
    if !row.is_empty() {
        assert!(!row_to_latex(&row).is_empty());
        assert!(!row_to_typst(&row).is_empty());
    }
}

// ----- MDN: three famous mathematical formulas -----

/// ∛(−q/2 + √(q²/4 + p³/27)) + ∛(−q/2 − √(q²/4 + p³/27))
#[test]
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
#[test]
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
#[test]
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
#[test]
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
#[test]
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
#[test]
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
#[test]
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
#[test]
fn euler_identity() {
    let row = cat(&[s("e"), n(sup(s("iπ"))), s("+1=0")]);
    roundtrip("euler", &row);
}

// ----- statistics -----

/// f(x) = (1/√(2πσ²)) e^{−(x−μ)²/(2σ²)}   (normal distribution PDF)
#[test]
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
#[test]
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
#[test]
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
#[test]
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
#[test]
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
#[test]
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
#[test]
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

/// Generalized delimiters: |x|, ⟨x|y⟩, {x | P(x)}, cases, bare arrays,
/// pmatrix, mismatched pairs.
#[test]
fn delimiter_blocks() {
    // |−x| = |x|
    let abs = |r: Row| delim('|', '|', vec![], vec![r]);
    roundtrip(
        "abs",
        &cat(&[n(abs(s("-x"))), s("="), n(abs(s("x")))]),
    );
    // ⟨ψ|H|ψ⟩ (two mids)
    roundtrip(
        "braket",
        &n(delim('⟨', '⟩', vec!['|', '|'], vec![s("ψ"), s("H"), s("ψ")])),
    );
    // {x | x² > 0} with a tall member
    roundtrip(
        "set-builder",
        &n(delim(
            '{',
            '}',
            vec!['|'],
            vec![s("x"), cat(&[s("x"), n(sup(s("2"))), s(">"), n(frac(s("1"), s("2")))])],
        )),
    );
    // cases: |x| = { x (x≥0) / −x (x<0)
    roundtrip(
        "cases",
        &cat(&[
            n(abs(s("x"))),
            s("="),
            n(delim(
                '{',
                '.',
                vec![],
                vec![n(array(2, 2, vec![s("x"), s("x≥0"), s("-x"), s("x<0")]))],
            )),
        ]),
    );
    // Bare array: self-delimiting ┌┬┐ lattice, also adjacent pairs and
    // inside a superscript.
    roundtrip(
        "bare-array",
        &n(array(2, 2, vec![s("a"), s("b"), s("c"), s("d")])),
    );
    roundtrip(
        "adjacent-lattices",
        &cat(&[
            n(array(2, 1, vec![s("a"), s("b")])),
            s("x"),
            n(array(2, 1, vec![s("c"), s("d")])),
        ]),
    );
    roundtrip(
        "lattice-in-sup",
        &cat(&[s("e"), n(sup(n(array(1, 2, vec![s("0"), s("t")]))))]),
    );
    // Explicit ▏ ▕ null pair still available via \delim..
    roundtrip(
        "null-delim-grid",
        &n(delim('.', '.', vec![], vec![n(array(2, 2, vec![s("a"), s("b"), s("c"), s("d")]))])),
    );
    roundtrip(
        "pmatrix",
        &n(delim('(', ')', vec![], vec![n(array(1, 2, vec![s("a+b"), s("c")]))])),
    );
    // Mismatched pair (half-open interval) and nested delimiters.
    roundtrip("interval", &n(delim('(', ']', vec![], vec![s("0,1")])));
    roundtrip(
        "nested-delims",
        &n(delim(
            '{',
            '}',
            vec![],
            vec![n(delim('⟨', '⟩', vec!['|'], vec![s("u"), n(abs(s("v")))]))],
        )),
    );
}

/// Explicit ␣ space atoms: manual spacing survives the roundtrip, also
/// inside matrix cells (␣ is a non-blank atom, so cell splitting holds).
#[test]
fn explicit_space_atoms() {
    let row = cat(&[s("f"), s("␣"), n(paren(s("x"))), s("␣␣"), s("dx")]);
    roundtrip("space-atoms", &row);
    let row = cat(&[
        n(mat(1, 2, vec![cat(&[s("a"), s("␣"), s("b")]), s("c")])),
        s("␣"),
        n(frac(s("␣"), s("x"))),
    ]);
    roundtrip("space-in-matrix", &row);
}

/// Continued fraction (deep vertical nesting).
#[test]
fn continued_fraction() {
    let mut row = s("x");
    for _ in 0..4 {
        row = cat(&[s("1+"), n(frac(s("1"), row))]);
    }
    roundtrip("continued-fraction", &row);
}

/// Limits that themselves contain big operators and fractions.
#[test]
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

// ----- hand-written (lenient) input -----

#[test]
fn parses_handwritten_2d_input() {
    let aa = r#"
        2
       x  + 1
x  =  ────────
       √2π
"#;
    // Note: the sqrt above has no overline, so write it canonically:
    let aa = aa.replace("√2π", "2π"); // keep the fraction test simple
    let row = parse(&aa).unwrap();
    assert_eq!(row_to_latex(&row), "x=\\frac{x^{2}+1}{2\\pi }");
}

// ----- randomized property test -----

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        // xorshift64*
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545F4914F6CDD1D)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

const ATOMS: &[char] = &[
    'a', 'b', 'c', 'x', 'y', 'z', 'A', 'B', 'N', '0', '1', '2', '7', '+',
    '-', '=', '<', 'α', 'β', 'π', 'λ', '∞', '∂', '⋅', '±', '∈', '→', '␣', '~',
];

fn gen_row(rng: &mut Rng, depth: usize, max_len: usize) -> Row {
    let len = rng.below(max_len + 1);
    (0..len).map(|_| gen_node(rng, depth)).collect()
}

fn gen_node(rng: &mut Rng, depth: usize) -> Node {
    let structural = depth > 0 && rng.below(100) < 45;
    if !structural {
        return match rng.below(10) {
            0 => Node::Func(["sin", "cos", "log", "exp"][rng.below(4)].into()),
            1 => Node::Accent {
                accent: ['^', '¯', '˙', '⇀', '˜', '‗'][rng.below(6)],
                base: ['x', 'v', 'a', 'E'][rng.below(4)],
            },
            _ => Node::Sym(ATOMS[rng.below(ATOMS.len())]),
        };
    }
    let d = depth - 1;
    match rng.below(10) {
        0 => Node::Frac { num: gen_row(rng, d, 3), den: gen_row(rng, d, 3) },
        1 => Node::Sqrt { arg: gen_row(rng, d, 3), index: [2, 2, 3, 4][rng.below(4)] },
        2 => Node::Sup { arg: gen_row(rng, d, 2) },
        3 => Node::Sub { arg: gen_row(rng, d, 2) },
        4 => Node::BigOp {
            op: ['∑', '∏', '∫', '⋃'][rng.below(4)],
            lower: gen_row(rng, d, 3),
            upper: gen_row(rng, d, 2),
        },
        5 => {
            // Random delimiter block: any pair (mismatched allowed), with
            // an occasional │ middle. normalize repairs constraint slips.
            let pairs = [
                ('(', ')'),
                ('[', ']'),
                ('{', '}'),
                ('⟨', '⟩'),
                ('|', '|'),
                ('.', '.'),
                ('(', ']'),
                ('{', '.'),
            ];
            let (l, r) = pairs[rng.below(pairs.len())];
            let nsegs = 1 + rng.below(2); // 1 or 2 segs
            let segs = (0..nsegs).map(|_| gen_row(rng, d, 3)).collect::<Vec<_>>();
            Node::Delim { left: l, right: r, mids: vec!['|'; nsegs - 1], segs }
        }
        7 => Node::Cancel { arg: gen_row(rng, d, 3) },
        6 => {
            // Grid inside a random known pair (bracket matrix most often).
            let pairs = [('[', ']'), ('[', ']'), ('(', ')'), ('.', '.'), ('{', '.')];
            let (l, r) = pairs[rng.below(pairs.len())];
            let (rows, cols) = [(2, 2), (1, 2), (2, 1), (1, 1)][rng.below(4)];
            let cells = (0..rows * cols).map(|_| gen_row(rng, d, 2)).collect();
            Node::Delim {
                left: l,
                right: r,
                mids: vec![],
                segs: vec![vec![Node::Array { rows, cols, cells }]],
            }
        }
        8 => {
            // Stray Array: normalize must wrap it in the null delimiter.
            let (rows, cols) = [(2, 2), (1, 2)][rng.below(2)];
            let cells = (0..rows * cols).map(|_| gen_row(rng, d, 2)).collect();
            Node::Array { rows, cols, cells }
        }
        _ => Node::Sym(ATOMS[rng.below(ATOMS.len())]),
    }
}

/// Case count / seed overridable for stress runs:
/// MASCII_PROP_N=30000 MASCII_PROP_SEED=1234 cargo test property_
#[test]
fn property_random_asts_roundtrip() {
    let n: usize = std::env::var("MASCII_PROP_N")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(2000);
    let seed: u64 = std::env::var("MASCII_PROP_SEED")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0x8bad_f00d_dead_beef);
    let mut rng = Rng(seed);
    for i in 0..n {
        let depth = 1 + rng.below(4);
        let row = gen_row(&mut rng, depth, 5);
        roundtrip(&format!("random-{}", i), &row);
    }
}
