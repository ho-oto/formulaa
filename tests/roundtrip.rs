//! AA <-> AST roundtrip tests on realistic formulas from mathematics,
//! physics and statistics, plus a randomized property test.
//!
//! Invariant: for any AST x,
//!     parse(render(normalize(x))) == normalize(strip_spacers(normalize(x)))
//! (render(parse(aa)) == aa is NOT required — AA is source code and the
//! accepted set is wider than the canonical form; fmt tightens it.)
//!
//! The corpus includes the three formulas from MDN's MathML tutorial
//! "Three famous mathematical formulas" (Cardano, Cauchy–Schwarz,
//! Vandermonde determinant).

use mascii::ast::{Node, Row, normalize, strip_spacers};
use mascii::latex::row_to_latex;
use mascii::parse::parse;
use mascii::render::{RenderCtx, render_root};
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
    Node::Delim {
        left,
        right,
        mids,
        segs,
    }
}

fn paren(inner: Row) -> Node {
    delim('(', ')', vec![], vec![inner])
}

fn bigop(op: char, lower: Row, upper: Row) -> Node {
    Node::BigOp {
        base: vec![Node::Sym(op)],
        lower,
        upper,
    }
}

fn func(name: &str) -> Node {
    Node::Func(name.into())
}

fn acc(accent: char, base: char) -> Node {
    if mascii::symbols::is_under_mark(accent) {
        Node::Accent {
            overs: vec![],
            unders: vec![accent],
            base,
        }
    } else {
        Node::Accent {
            overs: vec![accent],
            unders: vec![],
            base,
        }
    }
}

fn array(rows: usize, cols: usize, cells: Vec<Row>) -> Node {
    assert_eq!(cells.len(), rows * cols);
    Node::Array { rows, cols, cells }
}

/// A [ ] matrix: a grid wrapped in bracket delimiters.
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
    let aa = render_root(&row, None, &ctx).to_text();
    // Formatting spacers survive in the AA but are invisible to the
    // parser, so the roundtrip target is the spacer-free normal form.
    let expected = normalize(&strip_spacers(&row));
    let parsed =
        parse(&aa).unwrap_or_else(|e| panic!("[{}] parse failed: {}\n--- AA ---\n{}", name, e, aa));
    assert_eq!(
        parsed,
        expected,
        "[{}] AST mismatch\n--- AA ---\n{}\n--- LaTeX (expected) ---\n{}\n--- LaTeX (parsed) ---\n{}",
        name,
        aa,
        row_to_latex(&expected),
        row_to_latex(&parsed)
    );
    // Spacer-free output is a parse fixpoint.
    let aa2 = render_root(&parsed, None, &ctx).to_text();
    let reparsed = parse(&aa2)
        .unwrap_or_else(|e| panic!("[{}] re-parse failed: {}\n--- AA ---\n{}", name, e, aa2));
    assert_eq!(reparsed, parsed, "[{}] re-render mismatch", name);
    // Exports must not panic and must be non-empty for non-empty input.
    if !expected.is_empty() {
        assert!(!row_to_latex(&expected).is_empty());
        assert!(!row_to_typst(&expected).is_empty());
    }
}

/// Hand-written input stacking non-accent content directly above/below a
/// baseline token is a parse error — never silently dropped.
#[test]
fn stray_stacked_content_is_an_error() {
    // The fraction pins the baseline; y sits right on top of / below x.
    assert!(parse("1   y\n─ + x\n2").is_err());
    assert!(parse("1\n─ + x\n2   y").is_err());
}

/// ess sup_x f(x)  — \op*<name> (\operatorname*): a ┄band┄ whose base is
/// an arbitrary upright Text run instead of a dictionary Func.
#[test]
fn operatorname_star_band() {
    let row = cat(&[
        n(Node::BigOp {
            base: vec![Node::Text {
                t: "esssup".into(),
                math: true,
            }],
            lower: s("x"),
            upper: vec![],
        }),
        s("f"),
        n(paren(s("x"))),
    ]);
    roundtrip("operatorname-star", &row);
    assert_eq!(
        row_to_latex(&normalize(&row)),
        "\\operatorname*{esssup}_{x}f\\left(x\\right)"
    );
    // Multi-word \op* name: each word is its own band piece, dictionary
    // words as the Funcs they are (┄ess┄sup┄).
    let row = cat(&[n(Node::BigOp {
        base: vec![
            Node::Text {
                t: "ess".into(),
                math: true,
            },
            Node::Func("sup".into()),
        ],
        lower: s("x"),
        upper: vec![],
    })]);
    roundtrip("operatorname-star-words", &row);
    assert_eq!(
        row_to_latex(&normalize(&row)),
        "\\operatorname*{ess sup}_{x}"
    );
    // Empty limits: an \op* name keeps its band (a bare ∑ / lim would
    // collapse to the atom instead — promotable_base).
    let row = cat(&[
        n(Node::BigOp {
            base: vec![
                Node::Text {
                    t: "ess".into(),
                    math: true,
                },
                Node::Func("sup".into()),
            ],
            lower: vec![],
            upper: vec![],
        }),
        s("f"),
    ]);
    roundtrip("operatorname-star-bandless-limits", &row);
    assert_eq!(row_to_latex(&normalize(&row)), "\\operatorname*{ess sup}f");
    assert!(matches!(&normalize(&row)[0], Node::BigOp { .. }));
}

/// Roman differential: a lone upright letter drops its quotes exactly
/// when a neighbour glues it into the \mathrm reading, and keeps them
/// otherwise (the picture stays unambiguous either way).
#[test]
fn roman_differential_quotes() {
    let d = || Node::Text {
        t: "d".into(),
        math: true,
    };
    // Glued to a variable: bare d𝑥.
    let row = cat(&[n(d()), s("x")]);
    roundtrip("dx", &row);
    let aa = render_root(&normalize(&row), None, &RenderCtx::canonical()).to_text();
    assert_eq!(aa, "d𝑥");
    assert_eq!(row_to_latex(&normalize(&row)), "\\mathrm{d}x");
    // Standalone (or against a non-letter): quoted.
    let row = cat(&[n(d())]);
    roundtrip("d-alone", &row);
    assert_eq!(
        render_root(&normalize(&row), None, &RenderCtx::canonical()).to_text(),
        "'d'"
    );
    let row = cat(&[n(d()), s("+"), s("x")]);
    roundtrip("d-plus", &row);
    assert!(
        render_root(&normalize(&row), None, &RenderCtx::canonical())
            .to_text()
            .starts_with("'d'")
    );
    // Two roman letters never glue to each other (they would merge into
    // one run); the separation space keeps them quoted.
    let row = cat(&[n(d()), n(d())]);
    roundtrip("d-d", &row);
}

/// Dotted roman abbreviations (i.i.d., w.r.t.): one Text node, drawn
/// bare; the run lexer reads dots between letters (and one trailing dot
/// when an interior dot exists) back into the same token.
#[test]
fn dotted_roman_runs() {
    let t = |s: &str| Node::Text {
        t: s.into(),
        math: true,
    };
    let aa = |row: &Row| render_root(&normalize(row), None, &RenderCtx::canonical()).to_text();
    let row = cat(&[n(t("i.i.d.")), s("x")]);
    roundtrip("iid", &row);
    assert_eq!(aa(&row), "i.i.d.𝑥");
    assert_eq!(row_to_latex(&normalize(&row)), "\\mathrm{i.i.d.}x");
    // Adjacent letter run / period: the dotted run keeps a space so the
    // lexer cannot absorb them.
    let row = cat(&[n(t("i.i.d.")), n(t("ab"))]);
    roundtrip("iid-ab", &row);
    let row = cat(&[n(t("i.i")), n(Node::Sym('.'))]);
    roundtrip("iid-dot", &row);
    // `sin.` stays Func + period (a trailing dot needs an interior one).
    let row = cat(&[n(func("sin")), n(Node::Sym('.'))]);
    roundtrip("sin-dot", &row);
    assert_eq!(aa(&row), "sin.");
    // Ill-formed dot content falls back to the quoted form.
    let row = cat(&[n(t("x..y"))]);
    roundtrip("double-dot", &row);
    assert_eq!(aa(&row), "'x..y'");
    // Primes never touch dots (a '.' pair would read as a quote).
    let row = cat(&[n(Node::Sym('\'')), n(Node::Sym('.')), n(Node::Sym('\''))]);
    roundtrip("prime-dot-prime", &row);
}

/// Stretchy accents: a band whose limit region holds only the mark.
#[test]
fn wide_accents() {
    let wa =
        |over: Option<char>, under: Option<char>, base: Row| Node::WideAccent { over, under, base };
    let row = n(wa(Some('^'), None, s("abc")));
    roundtrip("widehat", &row);
    let aa = render_root(&normalize(&row), None, &RenderCtx::canonical()).to_text();
    assert_eq!(aa, "┄┄˰┄┄\n 𝑎𝑏𝑐");
    assert_eq!(row_to_latex(&normalize(&row)), "\\widehat{abc}");
    // Both sides at once.
    let row = n(wa(Some('⇀'), Some('‗'), s("AB")));
    roundtrip("vec-underline", &row);
    assert_eq!(
        row_to_latex(&normalize(&row)),
        "\\underline{\\overrightarrow{AB}}"
    );
    // Under tilde (\utilde): the ˜/˷ pair swaps between AST mark and
    // drawn glyph; the wide band fills with the high ˜ below.
    let row = n(wa(None, Some('˷'), s("AB")));
    roundtrip("wide-utilde", &row);
    let aa = render_root(&normalize(&row), None, &RenderCtx::canonical()).to_text();
    assert_eq!(aa, " 𝐴𝐵\n┄˜˜┄");
    assert_eq!(row_to_latex(&normalize(&row)), "\\utilde{AB}");
    assert_eq!(row_to_typst(&normalize(&row)), "attach(A B, b: sym.tilde)");
    let row = n(wa(Some('˜'), Some('˷'), s("xy")));
    roundtrip("tilde-utilde", &row);
    // Stacked hats: the outer band's ╱ can sit directly above the inner
    // band's ╲, which must not read as an angle-delimiter turn (the
    // angle-turn guard checks the ╱'s right neighbor for its partner ╲).
    let inner = wa(Some('^'), Some('‗'), s("Aβ"));
    let row = n(wa(Some('^'), None, cat(&[n(inner), s("1")])));
    roundtrip("stacked-hats", &row);
    // A one-char base with one over mark is the compact Accent.
    let row = n(wa(Some('^'), None, s("x")));
    assert_eq!(
        normalize(&row),
        vec![Node::Accent {
            overs: vec!['^'],
            unders: vec![],
            base: 'x'
        }]
    );
    // A markless wide accent is just its base (spliced).
    let row = n(wa(None, None, s("ab")));
    assert_eq!(normalize(&row), s("ab"));
    // The old base-banded picture reads leniently as a BigOp whose
    // upper limit is the bare mark atom — distinct from the accent
    // band, which never sits on the baseline.
}

/// Ceil / floor / double-bar norm delimiters.
#[test]
fn ceil_floor_norm() {
    let row = cat(&[n(delim(
        '⌈',
        '⌉',
        vec![],
        vec![cat(&[s("x"), n(frac(s("1"), s("2")))])],
    ))]);
    roundtrip("ceil", &row);
    assert_eq!(
        row_to_latex(&normalize(&row)),
        "\\left\\lceil x\\frac{1}{2}\\right\\rceil "
    );
    let row = cat(&[n(delim('⌊', '⌋', vec![], vec![s("n")]))]);
    roundtrip("floor", &row);
    let row = cat(&[n(delim(
        '‖',
        '‖',
        vec![],
        vec![cat(&[s("v"), n(frac(s("a"), s("b")))])],
    ))]);
    roundtrip("norm", &row);
    assert_eq!(
        row_to_latex(&normalize(&row)),
        "\\left\\|v\\frac{a}{b}\\right\\|"
    );
    assert!(mascii::typst::row_to_typst(&normalize(&row)).starts_with("norm("));
    // Two sibling norms stay siblings (parity per row).
    let row = cat(&[
        n(delim('‖', '‖', vec![], vec![s("v")])),
        s("+"),
        n(delim('‖', '‖', vec![], vec![s("w")])),
    ]);
    roundtrip("norm-siblings", &row);
}

/// \text keeps real spaces inside its quotes; KaTeX function names and
/// declared operators serialize correctly.
#[test]
fn text_spaces_and_operator_names() {
    let row = cat(&[n(Node::Text {
        t: "if x holds".into(),
        math: false,
    })]);
    roundtrip("text-spaces", &row);
    assert_eq!(
        render_root(&normalize(&row), None, &RenderCtx::canonical()).to_text(),
        "\"if x holds\""
    );
    assert_eq!(row_to_latex(&normalize(&row)), "\\text{if x holds}");
    // KaTeX-known names emit \name; declared operators \operatorname.
    assert_eq!(row_to_latex(&vec![func("arcctg")]), "\\arcctg ");
    assert_eq!(row_to_latex(&vec![func("Tr")]), "\\operatorname{Tr}");
    assert_eq!(row_to_latex(&vec![func("Re")]), "\\operatorname{Re}");
    roundtrip("tr-run", &cat(&[n(func("Tr")), n(paren(s("A")))]));
    // plim & friends band like lim.
    let row = cat(&[n(Node::BigOp {
        base: vec![func("plim")],
        lower: s("n"),
        upper: vec![],
    })]);
    roundtrip("plim", &row);
    assert_eq!(row_to_latex(&normalize(&row)), "\\plim _{n}");
}

/// rcases: the mirror of cases (null left, brace right).
#[test]
fn rcases_grid() {
    let row = n(delim(
        '.',
        '}',
        vec![],
        vec![vec![array(2, 2, vec![s("a"), s("b"), s("c"), s("d")])]],
    ));
    roundtrip("rcases", &row);
    assert_eq!(
        row_to_latex(&normalize(&row)),
        "\\begin{rcases} a & b \\\\ c & d \\end{rcases}"
    );
}

/// The session file writes formatting spacers as explicit ␠ so they
/// survive the restore parse.
#[test]
fn session_spacers_roundtrip() {
    let row = cat(&[
        s("a"),
        vec![Node::Spacer],
        s("b+"),
        vec![Node::Spacer, Node::Spacer],
        s("c"),
    ]);
    let row = normalize(&row);
    let marked = render_root(
        &mascii::ast::mark_spacers(&row),
        None,
        &RenderCtx::canonical(),
    )
    .to_text();
    assert!(marked.contains('␠'), "spacers visible: {}", marked);
    assert_eq!(parse(&marked).unwrap(), row, "restore keeps spacers");
}

/// Tall middles: a braket whose content is taller than one row keeps the
/// full-height │ separators (multi-row \middle| example).
#[test]
fn tall_middle_braket() {
    let row = cat(&[n(delim(
        '⟨',
        '⟩',
        vec!['|', '|'],
        vec![
            s("ψ"),
            vec![Node::Frac {
                num: s("H"),
                den: s("2"),
            }],
            s("ψ"),
        ],
    ))]);
    roundtrip("tall-middle", &row);
    assert_eq!(
        row_to_latex(&normalize(&row)),
        "\\left\\langle \\psi \\middle|\\frac{H}{2}\\middle|\\psi \\right\\rangle "
    );
    // Tall angles are pure diagonals (even height, the turn is a
    // same-column ╱╲ / ╲╱ pair, upper turn row = baseline); the ⟨ ⟩
    // glyphs appear only in the one-line form.
    let aa = render_root(&normalize(&row), None, &RenderCtx::canonical()).to_text();
    assert_eq!(aa, " ╱ │ 𝐻 │ ╲\n╱ ψ│───│ψ ╲\n╲  │ 2 │  ╱\n ╲ │   │ ╱");
    // The legacy single-column ╱⟨╲ form still parses.
    let legacy = "╱     1 ╲\n⟨𝑥 + ───⟩\n╲     2 ╱";
    assert!(parse(legacy).is_ok());
}

/// Multi-line formula: Breaks stack the lines with a lone-┄ continuation
/// marker on each following baseline.
#[test]
fn multi_line_formula() {
    let row = cat(&[
        s("y="),
        n(paren(cat(&[s("x+1")]))),
        n(sup(s("2"))),
        n(Node::Break),
        s("=x"),
        n(sup(s("2"))),
        s("+2x+1"),
        n(Node::Break),
        n(frac(s("a"), s("b"))),
    ]);
    roundtrip("multi-line", &row);
    let aa = render_root(&normalize(&row), None, &RenderCtx::canonical()).to_text();
    assert_eq!(
        aa.lines().filter(|l| l.trim_end() == "┄").count(),
        2,
        "two separator rows:\n{}",
        aa
    );
    assert!(
        row_to_latex(&normalize(&row)).contains("\\\\"),
        "latex line break"
    );
    // Empty middle line is legal.
    roundtrip(
        "empty-line",
        &cat(&[s("a"), n(Node::Break), n(Node::Break), s("b")]),
    );
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
    let row = cat(&[s("t="), n(cbrt(half_q("-"))), s("+"), n(cbrt(half_q("")))]);
    roundtrip("cardano", &row);
}

/// (∑_{k=1}^{n} u_k v̄_k)² ≤ (∑_{k=1}^{n} u_k²)(∑_{k=1}^{n} v_k²)
/// (MDN's Cauchy–Bunyakovsky–Schwarz inequality, kept with parens as in
/// the original corpus entry; |·| delimiters exist too — see \abs.)
#[test]
fn cauchy_schwarz_inequality() {
    let sum = |body: Row| {
        n(bigop('∑', s("k=1"), s("n")))
            .into_iter()
            .chain(body)
            .collect::<Row>()
    };
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
            s("1"),
            x("1", None),
            x("1", Some("2")),
            s("⋯"),
            x("1", Some("n-1")),
            s("1"),
            x("2", None),
            x("2", Some("2")),
            s("⋯"),
            x("2", Some("n-1")),
            s("⋮"),
            s("⋮"),
            s("⋮"),
            s("⋱"),
            s("⋮"),
            s("1"),
            x("n", None),
            x("n", Some("2")),
            s("⋯"),
            x("n", Some("n-1")),
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
        n(frac(s("1"), n(sqrt(cat(&[s("2πσ"), n(sup(s("2")))]))))),
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
    let row = cat(&[p("A|B"), s("="), n(frac(cat(&[p("B|A"), p("A")]), p("B")))]);
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
        n(frac(cat(&[s("x"), n(cancel(s("y")))]), n(cancel(s("y"))))),
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
    let row = cat(&[s("e"), n(sup(cat(&[s("x"), n(cancel(s("2α")))])))]);
    roundtrip("cancel-in-sup", &row);
}

/// Generalized delimiters: |x|, ⟨x|y⟩, {x | P(x)}, cases, bare arrays,
/// pmatrix, mismatched pairs.
#[test]
fn delimiter_blocks() {
    // |−x| = |x|
    let abs = |r: Row| delim('|', '|', vec![], vec![r]);
    roundtrip("abs", &cat(&[n(abs(s("-x"))), s("="), n(abs(s("x")))]));
    // ⟨ψ|H|ψ⟩ (two mids)
    roundtrip(
        "braket",
        &n(delim(
            '⟨',
            '⟩',
            vec!['|', '|'],
            vec![s("ψ"), s("H"), s("ψ")],
        )),
    );
    // {x | x² > 0} with a tall member
    roundtrip(
        "set-builder",
        &n(delim(
            '{',
            '}',
            vec!['|'],
            vec![
                s("x"),
                cat(&[s("x"), n(sup(s("2"))), s(">"), n(frac(s("1"), s("2")))]),
            ],
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
    // Explicit ╎ ┆ null pair still available via \delim..
    roundtrip(
        "null-delim-grid",
        &n(delim(
            '.',
            '.',
            vec![],
            vec![n(array(2, 2, vec![s("a"), s("b"), s("c"), s("d")]))],
        )),
    );
    roundtrip(
        "pmatrix",
        &n(delim(
            '(',
            ')',
            vec![],
            vec![n(array(1, 2, vec![s("a+b"), s("c")]))],
        )),
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

/// \overbrace / \underbrace with labels, incl. next to < > atoms.
#[test]
fn braces_over_under() {
    let brace = |over, arg: Row, label: Row| Node::Brace { over, arg, label };
    roundtrip(
        "overbrace",
        &cat(&[
            n(brace(true, cat(&[s("a"), s("+"), s("b")]), s("n"))),
            s("+"),
            n(brace(false, s("c"), s("m"))),
        ]),
    );
    // Unlabeled, with tall content, and adjacent to comparison atoms.
    roundtrip(
        "brace-tall",
        &cat(&[
            s("x"),
            s("<"),
            n(brace(
                true,
                cat(&[n(frac(s("1"), s("2"))), s("+y")]),
                vec![],
            )),
        ]),
    );
}

/// Quoted roman/text runs (\mathrm / \text).
#[test]
fn text_runs() {
    // ∫f(x)"dx" and a cases with a worded condition.
    roundtrip(
        "mathrm-dx",
        &cat(&[
            n(bigop('∫', vec![], vec![])),
            s("f"),
            n(paren(s("x"))),
            n(Node::Text {
                t: "dx".into(),
                math: true,
            }),
        ]),
    );
    roundtrip(
        "text-otherwise",
        &n(delim(
            '{',
            '.',
            vec![],
            vec![n(array(
                2,
                2,
                vec![
                    s("x"),
                    s("x≥0"),
                    s("-x"),
                    n(Node::Text {
                        t: "other wise".into(),
                        math: false,
                    }),
                ],
            ))],
        )),
    );
}

/// Labeled stretchy arrows (\xrightarrow / \xleftarrow).
#[test]
fn labeled_arrows() {
    let arrow = |op: char, over: Row, under: Row| Node::Arrow { op, over, under };
    // A --f--> B, with an under label too, and a left arrow.
    roundtrip(
        "xrightarrow",
        &cat(&[s("A"), n(arrow('→', s("f"), vec![])), s("B")]),
    );
    roundtrip(
        "xarrow-both",
        &cat(&[
            s("X"),
            n(arrow('→', cat(&[s("g"), n(sup(s("2")))]), s("n→∞"))),
            s("Y"),
            n(arrow('←', vec![], s("h"))),
            s("Z"),
        ]),
    );
    // Double arrows, and a fraction next to an arrow atom (space-separated).
    roundtrip(
        "double-arrows",
        &cat(&[n(arrow('⇒', s("f"), vec![])), n(arrow('⇐', vec![], s("g")))]),
    );
    roundtrip(
        "frac-then-arrow-atom",
        &cat(&[n(frac(s("1"), s("2"))), s("→"), n(frac(s("3"), s("4")))]),
    );
    // Adjacent arrows must not fuse their bodies.
    roundtrip(
        "adjacent-arrows",
        &cat(&[n(arrow('←', s("a"), vec![])), n(arrow('→', s("b"), vec![]))]),
    );
}

/// Stacked accents: marks pile outward above/below one base.
#[test]
fn stacked_accents() {
    // \hat{\vec{a}} and a bar over an underlined x.
    let row = cat(&[
        n(Node::Accent {
            overs: vec!['⇀', '^'],
            unders: vec![],
            base: 'a',
        }),
        s("+"),
        n(Node::Accent {
            overs: vec!['¯'],
            unders: vec!['‗'],
            base: 'x',
        }),
    ]);
    roundtrip("stacked-accents", &row);
    // Triple stack next to a fraction (baseline stripping goes deep).
    let row = cat(&[
        n(Node::Accent {
            overs: vec!['˙', '¯', '^'],
            unders: vec![],
            base: 'v',
        }),
        n(frac(s("1"), s("2"))),
    ]);
    roundtrip("triple-accent", &row);
    // Marks with a low variant hug the base: over bar draws as _ ,
    // hat as ˰ , tilde as ˷ , check as ˯ , ring as ˳ , dot as the
    // leader ․ , under bar as ¯. The ddot draws as ․․ overhanging one
    // blank-baseline column to the right.
    let row = cat(&[
        n(Node::Accent {
            overs: vec!['¯'],
            unders: vec![],
            base: 'x',
        }),
        n(Node::Accent {
            overs: vec!['^'],
            unders: vec![],
            base: 'v',
        }),
        n(Node::Accent {
            overs: vec!['˜'],
            unders: vec![],
            base: 'w',
        }),
        n(Node::Accent {
            overs: vec!['ˇ'],
            unders: vec![],
            base: 'c',
        }),
        n(Node::Accent {
            overs: vec!['˚'],
            unders: vec![],
            base: 'r',
        }),
        n(Node::Accent {
            overs: vec!['˙'],
            unders: vec![],
            base: 'd',
        }),
        n(Node::Accent {
            overs: vec!['¨'],
            unders: vec![],
            base: 'e',
        }),
        n(Node::Accent {
            overs: vec![],
            unders: vec!['‗'],
            base: 'y',
        }),
    ]);
    let aa = render_root(&normalize(&row), None, &RenderCtx::canonical()).to_text();
    assert_eq!(aa, "_˰˷˯˳․․․\n𝑥𝑣𝑤𝑐𝑟𝑑𝑒 𝑦\n        ¯");
    roundtrip("hugging-marks", &row);
    // The ddot's blank spill column breaks physical adjacency: a lone
    // \mathrm letter after it must keep its quotes (glue check is per
    // edge), and adjacent dotted atoms keep their own dots apart.
    let row = cat(&[
        n(Node::Accent {
            overs: vec!['¨', '⇀'],
            unders: vec![],
            base: 'E',
        }),
        n(Node::Text {
            t: "e".into(),
            math: true,
        }),
    ]);
    roundtrip("ddot-then-mathrm", &row);
    let row = cat(&[
        n(Node::Accent {
            overs: vec!['¨'],
            unders: vec![],
            base: 'x',
        }),
        n(Node::Accent {
            overs: vec!['¨'],
            unders: vec![],
            base: 'y',
        }),
        n(Node::Sup {
            arg: vec![Node::Sym('.')],
        }),
    ]);
    roundtrip("ddot-chain-sup-dot", &row);
    // A sqrt's greedy _ overline must not merge with a neighbouring bar
    // accent's _ on the same row (a separating space keeps them apart).
    let row = cat(&[
        n(Node::Sqrt {
            arg: vec![],
            index: 2,
        }),
        n(Node::Accent {
            overs: vec!['¯'],
            unders: vec![],
            base: 'x',
        }),
    ]);
    roundtrip("sqrt-then-bar", &row);
}

/// Formatting spacers: blank columns in the AA that vanish on reparse.
#[test]
fn formatting_spacers() {
    let sp = || Node::Spacer;
    // Between siblings, around structures, inside sub-rows.
    let row = cat(&[
        s("f"),
        n(sp()),
        n(paren(s("x"))),
        n(sp()),
        n(sp()),
        n(frac(cat(&[s("1"), n(sp()), s("+"), s("x")]), s("2"))),
    ]);
    roundtrip("spacers", &row);
    // Across-script spacer merges; leading/trailing die.
    let row = cat(&[
        n(sp()),
        s("x"),
        n(sup(s("a"))),
        n(sp()),
        n(sup(s("b"))),
        n(sp()),
    ]);
    roundtrip("spacers-scripts", &row);
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

/// Generalized bands: \lim, \argmax and friends take under-limits with
/// the same ┄band┄ notation as big operators.
#[test]
fn limit_functions() {
    let row = cat(&[
        n(Node::BigOp {
            base: vec![func("lim")],
            lower: cat(&[s("x"), s("→"), s("0")]),
            upper: vec![],
        }),
        s("f"),
        n(paren(s("x"))),
    ]);
    roundtrip("lim", &row);
    let row = cat(&[
        n(Node::BigOp {
            base: vec![func("arg"), func("max")],
            lower: s("x∈S"),
            upper: vec![],
        }),
        s("f"),
        n(paren(s("x"))),
    ]);
    roundtrip("argmax", &row);
    // Empty-limit bands normalize away: base splices into the row.
    let row = vec![Node::BigOp {
        base: vec![Node::Sym('∮')],
        lower: vec![],
        upper: vec![],
    }];
    roundtrip("bare-op", &row);
}

/// Limits that themselves contain big operators and fractions.
#[test]
fn nested_limits() {
    let row = cat(&[
        n(bigop(
            '∑',
            cat(&[
                s("i∈"),
                n(bigop('⋃', s("k"), vec![])),
                s("S"),
                n(sub(s("k"))),
            ]),
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
    'a', 'b', 'c', 'x', 'y', 'z', 'A', 'B', 'N', '0', '1', '2', '7', '+', '-', '=', '<', 'α', 'β',
    'π', 'λ', '∞', '∂', '⋅', '±', '∈', '→', '␣', '~', '\'', '.',
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
            1 => {
                // 1–2 marks, mixing over and under stacks.
                let marks = ['^', '¯', '˙', '¨', '⇀', '˜', '‗', '˷'];
                let base = ['x', 'v', 'a', 'E'][rng.below(4)];
                let (mut overs, mut unders) = (vec![], vec![]);
                for _ in 0..1 + rng.below(2) {
                    let m = marks[rng.below(marks.len())];
                    if m == '‗' || m == '˷' {
                        unders.push(m)
                    } else {
                        overs.push(m)
                    }
                }
                Node::Accent {
                    overs,
                    unders,
                    base,
                }
            }
            2 => Node::Spacer,
            // Single-letter \mathrm (the roman differential): quoted or
            // bare depending on the rendered neighbours.
            3 => Node::Text {
                t: ["d", "e", "D", "i.i.d.", "w.r.t", "a.e"][rng.below(6)].into(),
                math: true,
            },
            _ => Node::Sym(ATOMS[rng.below(ATOMS.len())]),
        };
    }
    let d = depth - 1;
    match rng.below(14) {
        0 => Node::Frac {
            num: gen_row(rng, d, 3),
            den: gen_row(rng, d, 3),
        },
        1 => Node::Sqrt {
            arg: gen_row(rng, d, 3),
            index: [2, 2, 3, 4][rng.below(4)],
        },
        2 => Node::Sup {
            arg: gen_row(rng, d, 2),
        },
        3 => Node::Sub {
            arg: gen_row(rng, d, 2),
        },
        4 => {
            let base: Row = match rng.below(5) {
                0 => vec![Node::Func("lim".into())],
                1 => vec![Node::Func("max".into())],
                2 => vec![Node::Func("arg".into()), Node::Func("max".into())],
                // \op* names: arbitrary upright operator words.
                3 => match rng.below(2) {
                    0 => vec![Node::Text {
                        t: "esssup".into(),
                        math: true,
                    }],
                    _ => vec![
                        Node::Text {
                            t: "ess".into(),
                            math: true,
                        },
                        Node::Func("sup".into()),
                    ],
                },
                _ => vec![Node::Sym(['∑', '∏', '∫', '⋃'][rng.below(4)])],
            };
            Node::BigOp {
                base,
                lower: gen_row(rng, d, 3),
                upper: gen_row(rng, d, 2),
            }
        }
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
                ('.', '}'),
                ('⌈', '⌉'),
                ('⌊', '⌋'),
                ('‖', '‖'),
            ];
            let (l, r) = pairs[rng.below(pairs.len())];
            let nsegs = 1 + rng.below(2); // 1 or 2 segs
            let mut segs = (0..nsegs).map(|_| gen_row(rng, d, 3)).collect::<Vec<_>>();
            // Direct norm-in-norm is unsupported (the picture is
            // ambiguous — same ‖ on both sides); rewrite inner norms.
            if l == '‖' {
                fn denorm(row: &mut Row) {
                    for n in row.iter_mut() {
                        if let Node::Delim { left, right, .. } = n
                            && *left == '‖'
                        {
                            *left = '|';
                            *right = '|';
                        }
                        // WideAccent bases are not cursor fields; walk
                        // them explicitly.
                        if let Node::WideAccent { base, .. } = n {
                            denorm(base);
                        }
                        for f in n.fields() {
                            denorm(n.field_mut(f));
                        }
                    }
                }
                for seg in &mut segs {
                    denorm(seg);
                }
            }
            Node::Delim {
                left: l,
                right: r,
                mids: vec!['|'; nsegs - 1],
                segs,
            }
        }
        7 => Node::Cancel {
            arg: gen_row(rng, d, 3),
        },
        13 => {
            // Stretchy accent; the band rides over any base block.
            let overs = ['^', '˜', '¯', '⇀', '˙', '¨', 'ˇ', '˚'];
            let base: Row = if rng.below(3) == 0 {
                gen_row(rng, d, 3)
            } else {
                (0..1 + rng.below(3))
                    .map(|_| Node::Sym(ATOMS[rng.below(ATOMS.len())]))
                    .collect()
            };
            let unders = ['‗', '˷'];
            let (over, under) = match rng.below(3) {
                0 => (Some(overs[rng.below(overs.len())]), None),
                1 => (None, Some(unders[rng.below(2)])),
                _ => (
                    Some(overs[rng.below(overs.len())]),
                    Some(unders[rng.below(2)]),
                ),
            };
            Node::WideAccent { over, under, base }
        }
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
            // Bare Array: renders as a self-delimiting lattice.
            let (rows, cols) = [(2, 2), (1, 2)][rng.below(2)];
            let cells = (0..rows * cols).map(|_| gen_row(rng, d, 2)).collect();
            Node::Array { rows, cols, cells }
        }
        10 => {
            let t = ["dx", "if", "abc", "T", "if x", "sin", "d"][rng.below(7)];
            Node::Text {
                t: t.into(),
                math: rng.below(2) == 0,
            }
        }
        11 => Node::Brace {
            over: rng.below(2) == 0,
            arg: gen_row(rng, d, 3),
            label: gen_row(rng, d, 2),
        },
        9 => Node::Arrow {
            op: ['→', '←', '⇒', '⇐'][rng.below(4)],
            over: gen_row(rng, d, 3),
            under: gen_row(rng, d, 2),
        },
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
        let mut row = gen_row(&mut rng, depth, 5);
        // Multi-line roots: occasionally append further segments.
        for _ in 0..rng.below(3) {
            row.push(Node::Break);
            let d = 1 + rng.below(3);
            row.extend(gen_row(&mut rng, d, 4));
        }
        roundtrip(&format!("random-{}", i), &row);
    }
}
