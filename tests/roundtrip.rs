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

use mascii::ast::{CancellableNode, Node, Row, normalize, strip_spacers};
use mascii::latex::row_to_latex;
use mascii::parse::parse;
use mascii::render::{RenderCtx, render_root};
use mascii::symbols::Radical;
use mascii::symbols::{Accent, Arrow};

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
    Node::Sqrt {
        arg,
        index: Radical::Sqrt,
    }
}

fn cbrt(arg: Row) -> Node {
    Node::Sqrt {
        arg,
        index: Radical::Cbrt,
    }
}

fn sup(arg: Row) -> Node {
    Node::Sup { arg }
}

fn sub(arg: Row) -> Node {
    Node::Sub { arg }
}

/// Spec-char convenience: the corpus spells pairs visually; the AST
/// stores the typed kinds.
fn delim(left: char, right: char, mids: Vec<char>, segs: Vec<Row>) -> Node {
    use mascii::symbols::Delim;
    Node::Delim {
        left: Delim::of_spec_side(left, true).unwrap(),
        right: Delim::of_spec_side(right, false).unwrap(),
        mids: mids.len(),
        segs,
    }
}

fn paren(inner: Row) -> Node {
    delim('(', ')', vec![], vec![inner])
}

fn bigop(op: char, lower: Row, upper: Row) -> Node {
    Node::BigOpSym { op, lower, upper }
}

fn opname(name: &str, lower: Row, upper: Row) -> Node {
    Node::BigOp {
        name: name.into(),
        lower,
        upper,
    }
}

fn func(name: &str) -> Node {
    Node::Func(name.into())
}

fn acc(accent: Accent, base: char) -> Node {
    if accent.under() {
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

/// Strike every token of the row (the atom-only Cancel model: a strike
/// on a structure means striking the tokens inside it).
fn cancel(mut arg: Row) -> Row {
    mascii::ast::cancel_all(&mut arg);
    arg
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
    // normalize must be a fixpoint of itself on every tree the
    // generator can produce, not just the hand-picked cases.
    assert_eq!(
        normalize(&expected),
        expected,
        "[{}] normalize is not idempotent",
        name
    );
    let parsed = parse(&aa).unwrap_or_else(|e| {
        panic!(
            "[{}] parse failed: {}\n--- AA ---\n{}\n--- AST ---\n{:?}",
            name, e, aa, row
        )
    });
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
    }
    // The LaTeX we emit reads back to the tree it came from (the
    // second road: AST -> LaTeX -> AST), spacers excepted.
    let tex = row_to_latex(&expected);
    let from_tex = normalize(&mascii::from_latex::row_from_latex(&tex));
    assert_eq!(
        from_tex, expected,
        "[{}] LaTeX roundtrip mismatch\n--- LaTeX ---\n{}",
        name, tex
    );
    // The export form (⬚ slot marks blanked when safe) reads back to
    // the same tree — over the whole corpus and the random trees.
    let exported = mascii::render::export_aa(&row);
    let reparsed = parse(&exported).unwrap_or_else(|e| {
        panic!(
            "[{}] export parse failed: {}\n--- AA ---\n{}",
            name, e, exported
        )
    });
    assert_eq!(
        reparsed, expected,
        "[{}] export mismatch\n--- AA ---\n{}",
        name, exported
    );
}

/// A formula with holes exports the holes as holes: the ⬚ marks blank
/// out when other ink carries the structure, and stay put when they
/// are the only thing making the picture readable.
/// Malformed pictures are errors, never panics: `parse` is the public
/// entry for the CLI and the wasm bindings, where an abort is fatal.
#[test]
fn malformed_pictures_do_not_panic() {
    for aa in [
        // A fused-grid row junction on the very first row: the cell
        // above it is empty, and its edge sits one row "above" zero.
        "├   ┤\n⎝ 𝑏 ⎠",
        "⎛ 𝑎 ⎞\n├   ┤",
        // A radical stem in the region's last column: no overline run
        // to measure, so the radicand is empty.
        " ┌\n𝑥│\n √",
        " ┌\n │\n √",
        // Assorted truncated pictures.
        "┌\n√",
        "⎡\n⎣",
        "┬",
        "╱\n╲",
    ] {
        let got = parse(aa);
        // Either reading is fine; aborting is not.
        assert!(got.is_ok() || got.is_err(), "{:?}", aa);
    }
}

/// Foreign-LaTeX constructs the reader must not misread: \limits asks
/// for the band, \over splits its group into a fraction, \not negates
/// a relation when Unicode has the codepoint (and keeps the bare
/// relation when it does not), and a spec-less \begin{array} keeps its
/// first cell.
#[test]
fn foreign_latex_reads_right() {
    use mascii::from_latex::row_from_latex;
    let tex = |t: &str| row_to_latex(&normalize(&row_from_latex(t)));
    assert_eq!(tex(r"{1 \over 2} + x"), r"\frac{1}{2}+x");
    assert_eq!(tex(r"a \atop b"), r"\frac{a}{b}");
    assert_eq!(tex(r"a \not= b"), r"a\ne b");
    assert_eq!(tex(r"a \not\in B"), r"a\notin B");
    assert_eq!(tex(r"a \not\leq b"), r"a\nleq b");
    // No Unicode negation -> the bare relation survives, not garbage.
    assert_eq!(tex(r"a \not\equiv b"), r"a\equiv b");
    // Our own \cancel{\sum } must strike the bare ∑ (an empty-limit
    // big operator is its symbol), and the spliced atom must not
    // become a limits host for a following script.
    assert_eq!(tex(r"\cancel{\sum }"), r"\cancel{\sum }");
    assert_eq!(tex(r"\cancel{\sum }^{2}"), r"\cancel{\sum }^{2}");
    assert_eq!(tex(r"\cancel{\operatorname{d}}"), r"\cancel{\mathrm{d}}");
    assert_eq!(
        tex(r"\sum\limits_{i=1}^{n} a"),
        tex(r"\sum_{i=1}^{n} a"),
        "\\limits keeps the band"
    );
    assert_eq!(
        tex(r"\begin{array} a & b \\ c \end{array}"),
        tex(r"\begin{array}{cc} a & b \\ c \end{array}"),
        "a spec-less array keeps its first cell"
    );
}

/// `normalize` must be idempotent: it runs again after every merge, so
/// a rule that lands on a shape another rule would rewrite (a band
/// collapsing to a one-letter `Func`) makes two exports of one document
/// disagree.
#[test]
fn normalize_is_idempotent() {
    use mascii::ast::{Field, Node};
    let cases: Vec<Node> = vec![
        Node::BigOp {
            name: "T".into(),
            lower: vec![],
            upper: vec![],
        },
        Node::BigOp {
            name: "lim".into(),
            lower: vec![],
            upper: vec![],
        },
        Node::Func("T".into()),
        Node::Func("".into()),
        // Roman is a lone upright LETTER; digits and dots canonicalize
        // to plain atoms (their bare picture reads back that way).
        Node::Roman('1'),
        Node::Roman('.'),
        Node::WideAccent {
            overs: vec![],
            unders: vec![],
            base: vec![Node::Sym('x')],
        },
        Node::WideAccent {
            overs: vec![],
            unders: vec![],
            base: vec![],
        },
        Node::Sup { arg: vec![] },
    ];
    for n in cases {
        let once = normalize(&vec![n.clone()]);
        assert_eq!(normalize(&once), once, "not idempotent: {:?}", n);
        // …and nested in an inset, where the same rules run again.
        let mut host = Node::Sqrt {
            index: mascii::symbols::Radical::Sqrt,
            arg: vec![],
        };
        *host.field_mut(Field::SqrtArg) = vec![n.clone()];
        let once = normalize(&vec![host.clone()]);
        assert_eq!(
            normalize(&once),
            once,
            "not idempotent in an inset: {:?}",
            n
        );
    }
}

#[test]
fn export_blanks_slot_marks_when_safe() {
    use mascii::render::export_aa;
    // The fraction bar carries the structure: ⬚ blanks away.
    let row = vec![frac(vec![], s("2"))];
    let ex = export_aa(&row);
    assert!(!ex.contains('⬚'), "{}", ex);
    assert_eq!(parse(&ex).unwrap(), normalize(&row));
    // A complete formula never contains ⬚ in the first place.
    assert!(!export_aa(&s("x+1")).contains('⬚'));
    // A bare tall script's base ⬚ is the only ink on the baseline —
    // blanking it would leave nothing to read, so it stays.
    let row = vec![sup(n(frac(s("1"), s("2"))))];
    let ex = export_aa(&row);
    assert!(ex.contains('⬚'), "{}", ex);
    assert_eq!(parse(&ex).unwrap(), normalize(&row));
}

/// Hand-written input stacking non-accent content directly above/below a
/// baseline token is a parse error — never silently dropped.
#[test]
fn stray_stacked_content_is_an_error() {
    // The fraction pins the baseline; y sits right on top of / below x.
    assert!(parse("1   y\n─ + x\n2").is_err());
    assert!(parse("1\n─ + x\n2   y").is_err());
}

/// ess sup_x f(x)  — \op*<name> (\operatorname*): a ┈band┈ whose base is
/// an arbitrary upright Text run instead of a dictionary Func.
#[test]
fn operatorname_star_band() {
    let row = cat(&[
        n(opname("esssup", s("x"), vec![])),
        s("f"),
        n(paren(s("x"))),
    ]);
    roundtrip("operatorname-star", &row);
    assert_eq!(
        row_to_latex(&normalize(&row)),
        "\\operatorname*{esssup}_{x}f\\left(x\\right)"
    );
    // Empty limits bare the band, and the bare form is exactly one Func
    // — the two spellings correspond one to one.
    let row = cat(&[n(opname("esssup", vec![], vec![])), s("f")]);
    roundtrip("operatorname-star-bandless-limits", &row);
    assert_eq!(normalize(&row)[0], func("esssup"));
    assert_eq!(row_to_latex(&normalize(&row)), "\\operatorname{esssup}f");
}

/// Multi-word operators: one band piece per word, native joined names
/// where the target format has them.
#[test]
fn word_operators() {
    // A band name is one piece; LaTeX spaces the ones that read as
    // several words (\operatorname*{arg\,max}).
    let row = n(opname("limsup", s("n"), vec![]));
    roundtrip("limsup", &row);
    let aa = render_root(&normalize(&row), None, &RenderCtx::canonical()).to_text();
    assert!(aa.contains("┈limsup┈"), "{}", aa);
    assert_eq!(
        row_to_latex(&normalize(&row)),
        "\\operatorname*{lim\\,sup}_{n}"
    );
    let row = n(opname("argmax", s("x"), vec![]));
    roundtrip("argmax", &row);
    assert_eq!(
        row_to_latex(&normalize(&row)),
        "\\operatorname*{arg\\,max}_{x}"
    );
}

/// Roman differential: a lone upright letter drops its quotes exactly
/// when a neighbour glues it into the \mathrm reading, and keeps them
/// otherwise (the picture stays unambiguous either way).
#[test]
fn roman_differential_quotes() {
    let d = || Node::Roman('d');
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
    let t = |s: &str| Node::Func(s.into());
    let aa = |row: &Row| render_root(&normalize(row), None, &RenderCtx::canonical()).to_text();
    let row = cat(&[n(t("i.i.d.")), s("x")]);
    roundtrip("iid", &row);
    assert_eq!(aa(&row), "i.i.d.𝑥");
    assert_eq!(row_to_latex(&normalize(&row)), "\\operatorname{i.i.d.}x");
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
    // The prime atom ′ is unrelated to the ' quote delimiter.
    let row = cat(&[s("x"), n(Node::Sym('′')), n(Node::Sym('′'))]);
    roundtrip("primes", &row);
    assert_eq!(aa(&row), "𝑥′′");
}

/// Stretchy accents: a band whose limit region holds only the mark.
#[test]
fn wide_accents() {
    let wa = |over: Option<Accent>, under: Option<Accent>, base: Row| Node::WideAccent {
        overs: over.into_iter().collect(),
        unders: under.into_iter().collect(),
        base,
    };
    let row = n(wa(Some(Accent::Hat), None, s("abc")));
    roundtrip("widehat", &row);
    let aa = render_root(&normalize(&row), None, &RenderCtx::canonical()).to_text();
    assert_eq!(aa, "┈┈˰┈┈\n 𝑎𝑏𝑐");
    assert_eq!(row_to_latex(&normalize(&row)), "\\widehat{abc}");
    // Both sides at once.
    let row = n(wa(Some(Accent::Vec), Some(Accent::Underline), s("AB")));
    roundtrip("vec-underline", &row);
    assert_eq!(
        row_to_latex(&normalize(&row)),
        "\\overrightarrow{\\underline{AB}}"
    );
    // Under tilde (\utilde): the ˜/˷ pair swaps between AST mark and
    // drawn glyph; the wide band fills with the high ˜ below.
    let row = n(wa(None, Some(Accent::Utilde), s("AB")));
    roundtrip("wide-utilde", &row);
    let aa = render_root(&normalize(&row), None, &RenderCtx::canonical()).to_text();
    assert_eq!(aa, " 𝐴𝐵\n┈˜˜┈");
    assert_eq!(row_to_latex(&normalize(&row)), "\\utilde{AB}");
    let row = n(wa(Some(Accent::Tilde), Some(Accent::Utilde), s("xy")));
    roundtrip("tilde-utilde", &row);
    // Stacked hats: the outer band's ╱ can sit directly above the inner
    // band's ╲, which must not read as an angle-delimiter turn (the
    // angle-turn guard checks the ╱'s right neighbor for its partner ╲).
    let inner = wa(Some(Accent::Hat), Some(Accent::Underline), s("Aβ"));
    let row = n(wa(Some(Accent::Hat), None, cat(&[n(inner), s("1")])));
    roundtrip("stacked-hats", &row);
    // A one-char base with one over mark is the compact Accent.
    let row = n(wa(Some(Accent::Hat), None, s("x")));
    assert_eq!(
        normalize(&row),
        vec![Node::Accent {
            overs: vec![Accent::Hat],
            unders: vec![],
            base: 'x'
        }]
    );
    // A struck accent still carries a band, so it needs the same
    // separation space as a bare one (the band has no closing glyph).
    let row = cat(&[
        n(sup(n(wa(None, Some(Accent::Underline), s("px"))))),
        cancel(n(wa(Some(Accent::Tilde), None, s("bc")))),
    ]);
    roundtrip("band-next-to-struck-band", &row);
    // A markless wide accent is just its base (spliced).
    let row = n(wa(None, None, s("ab")));
    assert_eq!(normalize(&row), s("ab"));
    // The old base-banded picture reads leniently as a BigOp whose
    // upper limit is the bare mark atom — distinct from the accent
    // band, which never sits on the baseline.
}

/// Fused grids use the ⎛/⎡ column glyphs with light ├ ┤ row junctions;
/// curly braces do not fuse; height-2 curly uses the ⎰⎱ sections.
#[test]
fn box_drawing_delim_forms() {
    let arr = |rows: usize, cols: usize, cells: Vec<Row>| Node::Array { rows, cols, cells };
    // Fused grids: 1 column (junctions ├ ┤ dug into the delimiter
    // columns), 2x2 (interior ┼ row), 1 row (┬ ┴ marker rows), and a
    // mixed ( ] pair.
    let row = n(delim(
        '(',
        ')',
        vec![],
        vec![n(arr(2, 1, vec![s("a"), s("b")]))],
    ));
    roundtrip("fused-grid-1col", &row);
    let aa = render_root(&normalize(&row), None, &RenderCtx::canonical()).to_text();
    assert_eq!(aa, "⎛ 𝑎 ⎞\n├   ┤\n⎝ 𝑏 ⎠");
    let row = n(delim(
        '(',
        ')',
        vec![],
        vec![n(arr(2, 2, vec![s("a"), s("b"), s("c"), s("d")]))],
    ));
    roundtrip("fused-grid-2x2", &row);
    let row = n(delim(
        '(',
        ')',
        vec![],
        vec![n(arr(1, 2, vec![s("a"), s("b")]))],
    ));
    roundtrip("fused-grid-1row", &row);
    let row = n(delim(
        '(',
        ']',
        vec![],
        vec![n(arr(2, 1, vec![s("a"), s("b")]))],
    ));
    roundtrip("fused-grid-mixed", &row);
    let row = n(delim(
        '[',
        ']',
        vec![],
        vec![n(arr(2, 1, vec![s("a"), s("b")]))],
    ));
    roundtrip("fused-grid-bracket", &row);
    let aa = render_root(&normalize(&row), None, &RenderCtx::canonical()).to_text();
    assert_eq!(aa, "⎡ 𝑎 ⎤\n├   ┤\n⎣ 𝑏 ⎦");
    // Curly braces do not fuse: the grid keeps its bare-lattice frame.
    let row = n(delim(
        '{',
        '}',
        vec![],
        vec![n(arr(2, 1, vec![s("a"), s("b")]))],
    ));
    roundtrip("curly-wraps-lattice", &row);
    let aa = render_root(&normalize(&row), None, &RenderCtx::canonical()).to_text();
    assert!(
        aa.contains('┌') && aa.contains('⎨'),
        "no fusion for curly:\n{}",
        aa
    );
    // A 2-row body still gets the full 3-row ⎧⎨⎩ column (the vertex
    // needs hook + ⎨ + hook; the extent widens past the body).
    let row = n(delim(
        '{',
        '}',
        vec![],
        vec![cat(&[s("a"), n(sup(s("α+")))])],
    ));
    roundtrip("curly-min-height", &row);
    let aa = render_root(&normalize(&row), None, &RenderCtx::canonical()).to_text();
    assert_eq!(aa, "⎧ α+⎫\n⎨𝑎  ⎬\n⎩   ⎭");
    // Null pair ┆ ┊ and a tall norm as stacked ‖.
    let row = n(delim('.', ')', vec![], vec![n(frac(s("1"), s("2")))]));
    roundtrip("null-left", &row);
    let aa = render_root(&normalize(&row), None, &RenderCtx::canonical()).to_text();
    assert!(aa.contains('┆') && aa.contains('⎞'), "{}", aa);
    let row = n(Node::Norm {
        arg: n(frac(s("1"), s("2"))),
    });
    roundtrip("tall-norm-stacked", &row);
    let aa = render_root(&normalize(&row), None, &RenderCtx::canonical()).to_text();
    assert_eq!(
        aa.matches('‖').count(),
        6,
        "stacked ‖ on both sides:\n{}",
        aa
    );
}

/// The sqrt ┌─ overline and its separation from aligned ─ neighbours.
#[test]
fn sqrt_box_overline() {
    let row = n(sqrt(cat(&[s("1+"), n(frac(s("1"), s("2")))])));
    roundtrip("sqrt-box", &row);
    let aa = render_root(&normalize(&row), None, &RenderCtx::canonical()).to_text();
    assert!(aa.starts_with("┌────"), "{}", aa);
    // A sup whose arrow body lands on the overline row must not merge
    // into the greedy ─ scan (geometric separation space).
    let row = cat(&[
        n(sqrt(s("2"))),
        n(sup(n(Node::Arrow {
            op: Arrow::To,
            over: vec![],
            under: vec![],
        }))),
    ]);
    roundtrip("sqrt-then-arrow-sup", &row);
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
    let row = cat(&[n(Node::Norm {
        arg: cat(&[s("v"), n(frac(s("a"), s("b")))]),
    })]);
    roundtrip("norm", &row);
    assert_eq!(
        row_to_latex(&normalize(&row)),
        "\\left\\|v\\frac{a}{b}\\right\\|"
    );
    // Two sibling norms stay siblings (parity per row).
    let row = cat(&[
        n(Node::Norm { arg: s("v") }),
        s("+"),
        n(Node::Norm { arg: s("w") }),
    ]);
    roundtrip("norm-siblings", &row);
}

/// \text keeps real spaces inside its quotes; KaTeX function names and
/// declared operators serialize correctly.
#[test]
fn text_spaces_and_operator_names() {
    let row = cat(&[n(Node::Text("if x holds".into()))]);
    roundtrip("text-spaces", &row);
    assert_eq!(
        render_root(&normalize(&row), None, &RenderCtx::canonical()).to_text(),
        "\"if x holds\""
    );
    assert_eq!(row_to_latex(&normalize(&row)), "\\text{if x holds}");
    // A literal quote (or backslash) inside \text is escaped in the AA.
    let row = cat(&[n(Node::Text("a\"b\\c".into()))]);
    roundtrip("text-escapes", &row);
    assert_eq!(
        render_root(&normalize(&row), None, &RenderCtx::canonical()).to_text(),
        "\"a\\\"b\\\\c\""
    );
    // …and so is every char LaTeX would read as syntax: a raw % would
    // comment out the rest of the document it is pasted into.
    let row = cat(&[n(Node::Text("50% {of} $x_1$".into()))]);
    roundtrip("text-syntax-chars", &row);
    assert_eq!(
        row_to_latex(&normalize(&row)),
        "\\text{50\\% \\{of\\} \\$x\\_1\\$}"
    );
    // KaTeX-known names emit \name; declared operators \operatorname.
    assert_eq!(
        row_to_latex(&vec![func("arcctg")]),
        "\\operatorname{arcctg}"
    );
    assert_eq!(row_to_latex(&vec![func("Tr")]), "\\operatorname{Tr}");
    assert_eq!(row_to_latex(&vec![func("Re")]), "\\operatorname{Re}");
    roundtrip("tr-run", &cat(&[n(func("Tr")), n(paren(s("A")))]));
    // plim & friends band like lim.
    let row = cat(&[n(Node::BigOp {
        name: "plim".into(),
        lower: s("n"),
        upper: vec![],
    })]);
    roundtrip("plim", &row);
    assert_eq!(row_to_latex(&normalize(&row)), "\\operatorname*{plim}_{n}");
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
    // The legacy single-column ╱⟨╲ form still parses when the content
    // fits the vertex row…
    assert!(parse("╱     ╲\n⟨𝑥 + 𝑦⟩\n╲     ╱").is_ok());
    // …but taller content used to be SILENTLY DROPPED (the fraction's
    // rows are outside the ⟨ column's extent); that is an error now.
    let legacy = "╱     1 ╲\n⟨𝑥 + ───⟩\n╲     2 ╱";
    assert!(
        parse(legacy).is_err(),
        "content outside the vertex row must not be dropped: {:?}",
        parse(legacy)
    );
}

/// Multi-line formula: Breaks stack the lines with a lone-┈ continuation
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
        aa.lines().filter(|l| l.trim_end() == "┈").count(),
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
            n(acc(Accent::Bar, 'v')),
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
        n(acc(Accent::Vec, 'E')),
        s("⋅d"),
        n(acc(Accent::Vec, 'A')),
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

/// ∭_Ω ∇⋅F dV — the triple integral is ∑-class (band-promotable).
#[test]
fn triple_integral_band() {
    let row = cat(&[n(bigop('∭', s("Ω"), vec![])), s("∇⋅F␣dV")]);
    roundtrip("triple-integral", &row);
}

/// \Vmatrix (a ‖ ‖ norm around a grid) and a bare Array standing
/// mid-row — the grid shapes the editor's newer commands produce.
#[test]
fn norm_grid_and_bare_array_roundtrip() {
    let row = vec![Node::Norm {
        arg: vec![array(2, 2, vec![s("a"), s("b"), vec![], s("d")])],
    }];
    roundtrip("vmatrix", &row);
    assert_eq!(
        row_to_latex(&normalize(&row)),
        "\\begin{Vmatrix} a & b \\\\  & d \\end{Vmatrix}"
    );
    // A cell-clipboard paste at the top level drops a bare Array
    // between ordinary siblings.
    let row = cat(&[s("x+"), n(array(1, 2, vec![s("a"), s("b")])), s("=y")]);
    roundtrip("bare-array-mid-row", &row);
}

/// The amssymb tier: relations, orders and operators that now carry a
/// LaTeX spelling (a ≼ b ⪅ c ⋆ d ⊓ e ⟹ a ≺ e).
#[test]
fn amssymb_tier_atoms() {
    roundtrip("amssymb-relations", &s("a≼b⪅c⋆d⊓e⟹a≺e"));
    // TeX-aligned Greek variants: ϵ = \epsilon, ε = \varepsilon,
    // ϕ = \phi, φ = \varphi, ϰ = \varkappa.
    roundtrip("greek-variants", &s("ϵ≠ε␣ϕ≠φ␣ϰ≠κ"));
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
        n(frac(cat(&[s("x"), cancel(s("y"))]), cancel(s("y")))),
        s("="),
        s("x"),
    ]);
    roundtrip("cancel-simple", &row);
    // cancel over a whole fraction, next to an uncancelled sibling
    let row = cat(&[
        cancel(cat(&[n(frac(s("a+b"), s("c"))), s("d")])),
        s("+"),
        s("e"),
    ]);
    roundtrip("cancel-frac", &row);
    // cancel inside a superscript
    let row = cat(&[s("e"), n(sup(cat(&[s("x"), cancel(s("2α"))])))]);
    roundtrip("cancel-in-sup", &row);
    // A struck Text keeps its real space; the space cell has no glyph
    // to strike, and the parser must absorb it with the token.
    let row = vec![Node::Cancel(CancellableNode::Text("a b".into()))];
    roundtrip("cancel-text-space", &row);
    // A struck one-letter Func collapses to Roman through the strike
    // (reachable via \cancel{\operatorname{d}} in the LaTeX reader).
    let row = vec![Node::Cancel(CancellableNode::Func("d".into()))];
    roundtrip("cancel-func-one-letter", &row);
    // A struck dotted run before a dot atom keeps its separating
    // space, or the dot would be absorbed into the run on reparse.
    let row = vec![
        Node::Cancel(CancellableNode::Func("w.r.t".into())),
        Node::Cancel(CancellableNode::Sym('.')),
    ];
    roundtrip("cancel-dot-run", &row);
    // A struck big-operator atom (the bare ∑ is a Sym).
    let row = vec![Node::Cancel(CancellableNode::Sym('∑'))];
    roundtrip("cancel-bigop-atom", &row);
    // A bare one-char Func lands on its final shape in one pass
    // (normalize idempotence: Func("1") must go straight to Sym).
    let row = vec![Node::Func("1".into()), Node::Func(".".into())];
    roundtrip("bare-one-char-func", &row);
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
    // Explicit ┆ ┊ null pair still available via \delim..
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
            n(Node::Func("dx".into())),
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
                    n(Node::Text("other wise".into())),
                ],
            ))],
        )),
    );
}

/// Labeled stretchy arrows (\xrightarrow / \xleftarrow).
#[test]
fn labeled_arrows() {
    let arrow = |op: Arrow, over: Row, under: Row| Node::Arrow { op, over, under };
    // A --f--> B, with an under label too, and a left arrow.
    roundtrip(
        "xrightarrow",
        &cat(&[s("A"), n(arrow(Arrow::To, s("f"), vec![])), s("B")]),
    );
    roundtrip(
        "xarrow-both",
        &cat(&[
            s("X"),
            n(arrow(Arrow::To, cat(&[s("g"), n(sup(s("2")))]), s("n→∞"))),
            s("Y"),
            n(arrow(Arrow::From, vec![], s("h"))),
            s("Z"),
        ]),
    );
    // Double arrows, and a fraction next to an arrow atom (space-separated).
    roundtrip(
        "double-arrows",
        &cat(&[
            n(arrow(Arrow::DoubleTo, s("f"), vec![])),
            n(arrow(Arrow::DoubleFrom, vec![], s("g"))),
        ]),
    );
    roundtrip(
        "frac-then-arrow-atom",
        &cat(&[n(frac(s("1"), s("2"))), s("→"), n(frac(s("3"), s("4")))]),
    );
    // Adjacent arrows must not fuse their bodies.
    roundtrip(
        "adjacent-arrows",
        &cat(&[
            n(arrow(Arrow::From, s("a"), vec![])),
            n(arrow(Arrow::To, s("b"), vec![])),
        ]),
    );
}

/// Stacked accents: marks pile outward above/below one base.
#[test]
fn stacked_accents() {
    // \hat{\vec{a}} and a bar over an underlined x.
    let row = cat(&[
        n(Node::Accent {
            overs: vec![Accent::Vec, Accent::Hat],
            unders: vec![],
            base: 'a',
        }),
        s("+"),
        n(Node::Accent {
            overs: vec![Accent::Bar],
            unders: vec![Accent::Underline],
            base: 'x',
        }),
    ]);
    roundtrip("stacked-accents", &row);
    // Triple stack next to a fraction (baseline stripping goes deep).
    let row = cat(&[
        n(Node::Accent {
            overs: vec![Accent::Dot, Accent::Bar, Accent::Hat],
            unders: vec![],
            base: 'v',
        }),
        n(frac(s("1"), s("2"))),
    ]);
    roundtrip("triple-accent", &row);
    // Marks with a low variant hug the base: over bar draws as _ ,
    // hat as ˰ , tilde as ˷ , check as ˯ , ring as ˳ , dot as the
    // leader ․ , vec as the halfwidth ￫ , under bar as ¯. The ddot
    // draws as ․․ overhanging one blank-baseline column to the right.
    let row = cat(&[
        n(Node::Accent {
            overs: vec![Accent::Bar],
            unders: vec![],
            base: 'x',
        }),
        n(Node::Accent {
            overs: vec![Accent::Hat],
            unders: vec![],
            base: 'v',
        }),
        n(Node::Accent {
            overs: vec![Accent::Tilde],
            unders: vec![],
            base: 'w',
        }),
        n(Node::Accent {
            overs: vec![Accent::Check],
            unders: vec![],
            base: 'c',
        }),
        n(Node::Accent {
            overs: vec![Accent::Ring],
            unders: vec![],
            base: 'r',
        }),
        n(Node::Accent {
            overs: vec![Accent::Dot],
            unders: vec![],
            base: 'd',
        }),
        n(Node::Accent {
            overs: vec![Accent::Ddot],
            unders: vec![],
            base: 'e',
        }),
        n(Node::Accent {
            overs: vec![Accent::Vec],
            unders: vec![],
            base: 'u',
        }),
        n(Node::Accent {
            overs: vec![],
            unders: vec![Accent::Underline],
            base: 'y',
        }),
    ]);
    let aa = render_root(&normalize(&row), None, &RenderCtx::canonical()).to_text();
    assert_eq!(aa, "_˰˷˯˳․․․￫\n𝑥𝑣𝑤𝑐𝑟𝑑𝑒 𝑢𝑦\n         ¯");
    roundtrip("hugging-marks", &row);
    // The ddot's blank spill column breaks physical adjacency: a lone
    // \mathrm letter after it must keep its quotes (glue check is per
    // edge), and adjacent dotted atoms keep their own dots apart.
    let row = cat(&[
        n(Node::Accent {
            overs: vec![Accent::Ddot, Accent::Vec],
            unders: vec![],
            base: 'E',
        }),
        n(Node::Roman('e')),
    ]);
    roundtrip("ddot-then-mathrm", &row);
    let row = cat(&[
        n(Node::Accent {
            overs: vec![Accent::Ddot],
            unders: vec![],
            base: 'x',
        }),
        n(Node::Accent {
            overs: vec![Accent::Ddot],
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
            index: Radical::Sqrt,
        }),
        n(Node::Accent {
            overs: vec![Accent::Bar],
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
/// the same ┈band┈ notation as big operators.
#[test]
fn limit_functions() {
    let row = cat(&[
        n(Node::BigOp {
            name: "lim".into(),
            lower: cat(&[s("x"), s("→"), s("0")]),
            upper: vec![],
        }),
        s("f"),
        n(paren(s("x"))),
    ]);
    roundtrip("lim", &row);
    let row = cat(&[
        n(Node::BigOp {
            name: "argmax".into(),
            lower: s("x∈S"),
            upper: vec![],
        }),
        s("f"),
        n(paren(s("x"))),
    ]);
    roundtrip("argmax", &row);
    // Empty-limit bands normalize away: base splices into the row.
    let row = vec![Node::BigOpSym {
        op: '∮',
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
    'π', 'λ', '∞', '∂', '⋅', '±', '∈', '→', '␣', '∼', '′', '.', '%', '&',
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
                let marks = [
                    Accent::Hat,
                    Accent::Bar,
                    Accent::Dot,
                    Accent::Ddot,
                    Accent::Vec,
                    Accent::Tilde,
                    Accent::Underline,
                    Accent::Utilde,
                ];
                let base = ['x', 'v', 'a', 'E'][rng.below(4)];
                let (mut overs, mut unders) = (vec![], vec![]);
                for _ in 0..1 + rng.below(2) {
                    let m = marks[rng.below(marks.len())];
                    if m.under() {
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
            // The roman differential (quoted or bare depending on the
            // rendered neighbours) and upright runs.
            3 => match rng.below(6) {
                0 => Node::Roman('d'),
                1 => Node::Roman('e'),
                2 => Node::Roman('D'),
                3 => Node::Func("i.i.d.".into()),
                4 => Node::Func("w.r.t".into()),
                _ => Node::Func("a.e".into()),
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
            index: [Radical::Sqrt, Radical::Sqrt, Radical::Cbrt, Radical::Qdrt][rng.below(4)],
        },
        2 => Node::Sup {
            arg: gen_row(rng, d, 2),
        },
        3 => Node::Sub {
            arg: gen_row(rng, d, 2),
        },
        4 => {
            let (lower, upper) = (gen_row(rng, d, 3), gen_row(rng, d, 2));
            match rng.below(5) {
                // Named bands, including ad-hoc \op* names (and the
                // shortest legal one — two letters).
                0 => opname(["lim", "Tr"][rng.below(2)], lower, upper),
                1 => opname("max", lower, upper),
                2 => opname("argmax", lower, upper),
                3 => opname("esssup", lower, upper),
                _ => Node::BigOpSym {
                    op: ['∑', '∏', '∫', '⋃'][rng.below(4)],
                    lower,
                    upper,
                },
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
            ];
            let (l, r) = pairs[rng.below(pairs.len())];
            let nsegs = 1 + rng.below(2); // 1 or 2 segs
            let segs = (0..nsegs).map(|_| gen_row(rng, d, 3)).collect::<Vec<_>>();
            delim(l, r, vec!['|'; nsegs - 1], segs)
        }
        7 => match rng.below(4) {
            // Norms nest (the outer pair renders two rows taller).
            0 => Node::Norm {
                arg: gen_row(rng, d, 3),
            },
            // A strike wraps one atom-shaped token.
            _ => Node::Cancel(match rng.below(4) {
                0 => CancellableNode::Sym(['x', 'y', 'α', 'B'][rng.below(4)]),
                1 => CancellableNode::Func(["sin", "w.r.t"][rng.below(2)].into()),
                2 => CancellableNode::Roman(['d', 'A'][rng.below(2)]),
                // The spaced text pins the blank-cell strike rule.
                _ => CancellableNode::Text(["ok", "a b"][rng.below(2)].into()),
            }),
        },
        13 => {
            // Stretchy accent; the band rides over any base block.
            let overs = [
                Accent::Hat,
                Accent::Tilde,
                Accent::Bar,
                Accent::Vec,
                Accent::Dot,
                Accent::Ddot,
                Accent::Check,
                Accent::Ring,
            ];
            let base: Row = if rng.below(3) == 0 {
                gen_row(rng, d, 3)
            } else {
                (0..1 + rng.below(3))
                    .map(|_| Node::Sym(ATOMS[rng.below(ATOMS.len())]))
                    .collect()
            };
            let unders = [Accent::Underline, Accent::Utilde];
            // Marks stack, so generate 0-2 per side.
            let pick = |rng: &mut Rng, pool: &[Accent], n: usize| -> Vec<Accent> {
                let mut v = Vec::new();
                for _ in 0..n {
                    v.push(pool[rng.below(pool.len())]);
                }
                v
            };
            let (no, nu) = match rng.below(4) {
                0 => (1, 0),
                1 => (0, 1),
                2 => (1, 1),
                _ => (1 + rng.below(2), rng.below(2)),
            };
            Node::WideAccent {
                overs: pick(rng, &overs, no),
                unders: pick(rng, &unders, nu),
                base,
            }
        }
        6 => {
            // Grid inside a random known pair (bracket matrix most often).
            let pairs = [('[', ']'), ('[', ']'), ('(', ')'), ('.', '.'), ('{', '.')];
            let (l, r) = pairs[rng.below(pairs.len())];
            let (rows, cols) = [(2, 2), (1, 2), (2, 1), (1, 1)][rng.below(4)];
            let cells = (0..rows * cols).map(|_| gen_row(rng, d, 2)).collect();
            delim(l, r, vec![], vec![vec![Node::Array { rows, cols, cells }]])
        }
        8 => {
            // Bare Array: renders as a self-delimiting lattice.
            let (rows, cols) = [(2, 2), (1, 2), (1, 1), (3, 2), (1, 3)][rng.below(5)];
            let cells = (0..rows * cols).map(|_| gen_row(rng, d, 2)).collect();
            Node::Array { rows, cols, cells }
        }
        10 => {
            if rng.below(2) == 0 {
                Node::Func(["dx", "abc", "T", "sin"][rng.below(4)].into())
            } else {
                // \text content may need the \" \\ escapes.
                Node::Text(["if", "if x", "a\"b", "x\\y"][rng.below(4)].into())
            }
        }
        11 => Node::Brace {
            over: rng.below(2) == 0,
            arg: gen_row(rng, d, 3),
            label: gen_row(rng, d, 2),
        },
        9 => Node::Arrow {
            op: Arrow::ALL[rng.below(Arrow::ALL.len())],
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
