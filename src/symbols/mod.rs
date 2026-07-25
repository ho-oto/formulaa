//! Symbol / function / accent knowledge, all in one place:
//! - `SYMBOLS` (curated names) + `ext::EXT_SYMBOLS` (generated, 4000+)
//! - `FUNCS`: upright functions with their limits/LaTeX/Typst flags
//! - `BIG_OPS`: ∑-class symbol operators (band-promotable atoms)
//! - `ACCENTS`: accent marks (AST mark chars; drawn forms in render)
//! - `is_reserved_glyph`: the atom exclusion list (docs/aa-spec.md §2)

pub mod alphabets;
pub mod ext;

/// (command name, unicode char). The LaTeX command is `\name`.
pub const SYMBOLS: &[(&str, char)] = &[
    // Greek lowercase
    ("alpha", 'α'),
    ("beta", 'β'),
    ("gamma", 'γ'),
    ("delta", 'δ'),
    ("epsilon", 'ε'),
    ("zeta", 'ζ'),
    ("eta", 'η'),
    ("theta", 'θ'),
    ("iota", 'ι'),
    ("kappa", 'κ'),
    ("lambda", 'λ'),
    ("mu", 'μ'),
    ("nu", 'ν'),
    ("xi", 'ξ'),
    ("pi", 'π'),
    ("rho", 'ρ'),
    ("sigma", 'σ'),
    ("tau", 'τ'),
    ("upsilon", 'υ'),
    ("phi", 'φ'),
    ("chi", 'χ'),
    ("psi", 'ψ'),
    ("omega", 'ω'),
    ("varphi", 'ϕ'),
    ("vartheta", 'ϑ'),
    // Greek uppercase
    ("Gamma", 'Γ'),
    ("Delta", 'Δ'),
    ("Theta", 'Θ'),
    ("Lambda", 'Λ'),
    ("Xi", 'Ξ'),
    ("Pi", 'Π'),
    ("Sigma", 'Σ'),
    ("Upsilon", 'Υ'),
    ("Phi", 'Φ'),
    ("Psi", 'Ψ'),
    ("Omega", 'Ω'),
    // Binary operators / relations
    ("pm", '±'),
    ("mp", '∓'),
    ("times", '×'),
    ("div", '÷'),
    ("cdot", '⋅'),
    ("circ", '∘'),
    ("oplus", '⊕'),
    ("otimes", '⊗'),
    ("ast", '∗'),
    ("le", '≤'),
    ("leq", '≤'),
    ("ge", '≥'),
    ("geq", '≥'),
    ("ne", '≠'),
    ("neq", '≠'),
    ("approx", '≈'),
    ("equiv", '≡'),
    ("sim", '∼'),
    ("simeq", '≃'),
    ("propto", '∝'),
    ("ll", '≪'),
    ("gg", '≫'),
    // Arrows
    ("to", '→'),
    ("rightarrow", '→'),
    ("leftarrow", '←'),
    ("Rightarrow", '⇒'),
    ("Leftarrow", '⇐'),
    ("leftrightarrow", '↔'),
    ("Leftrightarrow", '⇔'),
    ("mapsto", '↦'),
    // Sets / logic
    ("in", '∈'),
    ("notin", '∉'),
    ("ni", '∋'),
    ("subset", '⊂'),
    ("subseteq", '⊆'),
    ("supset", '⊃'),
    ("supseteq", '⊇'),
    ("cup", '∪'),
    ("cap", '∩'),
    ("setminus", '∖'),
    ("emptyset", '∅'),
    ("forall", '∀'),
    ("exists", '∃'),
    ("neg", '¬'),
    ("land", '∧'),
    ("lor", '∨'),
    ("vdash", '⊢'),
    ("models", '⊨'),
    // Misc
    ("infty", '∞'),
    ("partial", '∂'),
    ("nabla", '∇'),
    ("hbar", 'ℏ'),
    ("ell", 'ℓ'),
    ("Re", 'ℜ'),
    ("Im", 'ℑ'),
    ("aleph", 'ℵ'),
    ("angle", '∠'),
    ("perp", '⊥'),
    ("parallel", '∥'),
    ("prime", '′'),
    ("degree", '°'),
    ("cdots", '⋯'),
    ("ldots", '…'),
    ("dots", '…'),
    ("vdots", '⋮'),
    ("ddots", '⋱'),
    // Explicit space atom (the Space key): visible ␣ in canonical AA so the
    // picture stays parseable (a real blank column is a sibling separator).
    ("space", '␣'),
];

/// Structure-reserved glyphs: never valid as atoms (see docs/aa-spec.md §2).
/// `symbol_by_name` filters these so no symbol table entry can inject one.
/// Accent marks, the √ overline `_`, and the inline super/subscript
/// codepoints are reserved too — they read back as structure, not atoms.
#[rustfmt::skip]
pub fn is_reserved_glyph(c: char) -> bool {
    matches!(c,
        // Rules and bands
        '─'                     // U+2500 BOX DRAWINGS LIGHT HORIZONTAL: fraction bar / arrow body
        | '┈'                   // U+2508 LIGHT QUADRUPLE DASH HORIZONTAL: op band / accent band / break row
        | '═'                   // U+2550 BOX DRAWINGS DOUBLE HORIZONTAL: double-arrow body
        // Radicals
        | '√' | '∛' | '∜'       // U+221A/221B/221C SQUARE/CUBE/FOURTH ROOT
        | '│'                   // U+2502 LIGHT VERTICAL: radical stem / delimiter middle / fused round column
        | '_'                   // U+005F LOW LINE: sqrt overline / drawn over-bar
        // Parentheses (U+239B-23A0 pieces)
        | '(' | ')' | '⎛' | '⎜' | '⎝' | '⎞' | '⎟' | '⎠'
        // Square brackets (U+23A1-23A6 pieces); ceil/floor reuse them
        | '[' | ']' | '⎡' | '⎢' | '⎣' | '⎤' | '⎥' | '⎦'
        | '⌈' | '⌉' | '⌊' | '⌋' // U+2308/2309/230A/230B CEILING / FLOOR
        // Curly braces (U+23A7-23AD pieces; min height 3 when tall)
        | '{' | '}' | '⎧' | '⎨' | '⎩' | '⎪' | '⎫' | '⎬' | '⎭'
        // Angles: U+27E8/27E9 one-line; arms U+2571/2572 diagonals
        | '⟨' | '⟩' | '╱' | '╲'
        // (the vertical bar \abs reuses the bracket extensions ⎢ ⎥;
        // ⎸ ⎹ U+23B8/23B9 are no longer part of the format)
        | '‖'                   // U+2016 DOUBLE VERTICAL LINE: norm (stacked when tall)
        | '┆'                   // U+2506 LIGHT TRIPLE DASH VERTICAL: null delimiter left
        | '┊'                   // U+250A LIGHT QUADRUPLE DASH VERTICAL: null delimiter right
        // Lattice markers (U+250C..2518 crossings) + fused junctions
        // (├ ┤ double as the fused-grid row junctions dug into
        // delimiter columns)
        | '┌' | '┬' | '┐' | '├' | '┼' | '┤' | '└' | '┴' | '┘'
        // Arcs U+256D/256E/2570/256F: brace range rows + fused round columns
        | '╭' | '╮' | '╰' | '╯'
        // Format spellings
        | '⬚'                   // U+2B1A DOTTED SQUARE: empty slot / script base
        | '▌'                   // U+258C LEFT HALF BLOCK: cursor (view layer only)
        | '"'                   // U+0022: \text quotes
        | '\''                  // U+0027: \mathrm quotes (the prime atom is ′ U+2032)
        // Drawn accent glyphs (the only accent chars that appear in
        // AA; the AST mark chars ^ ˇ ˜ ˙ ¨ ˚ ⇀ ‗ ˷ are internal
        // identifiers and NOT reserved)
        | '¯'                   // U+00AF MACRON: drawn under bar
        | '˜'                   // U+02DC SMALL TILDE: drawn under tilde
        | '˷'                   // U+02F7 LOW TILDE: drawn over tilde
        | '˰'                   // U+02F0 MODIFIER LETTER LOW UP ARROWHEAD: hat
        | '˯'                   // U+02EF MODIFIER LETTER LOW DOWN ARROWHEAD: check
        | '˳'                   // U+02F3 MODIFIER LETTER LOW RING: ring
        | '․'                   // U+2024 ONE DOT LEADER: dot / ddot (․․)
        | '￫'                   // U+FFEB HALFWIDTH RIGHTWARDS ARROW: vec
    ) || crate::render::unsuperscript_char(c).is_some()
        || crate::render::unsubscript_char(c).is_some()
}

/// Accent marks: (command name, mark char, is_under, latex command).
/// Mark chars are RESERVED — they never occur as atoms, and over-marks are
/// disjoint from under-marks; this is what makes a two-character column
/// (mark stacked on base) unambiguous for the parser.
/// AST mark chars; the hugging glyphs actually drawn are mapped in
/// render (over_glyph/under_glyph) and parse (over_mark_at/under_mark_at).
pub const ACCENTS: &[(&str, char, bool, &str)] = &[
    ("hat", '^', false, "hat"),            // U+005E, drawn ˰ U+02F0
    ("tilde", '˜', false, "tilde"),        // U+02DC (not the atom '~'), drawn ˷ U+02F7
    ("bar", '¯', false, "bar"),            // U+00AF MACRON, drawn _ U+005F
    ("vec", '⇀', false, "vec"),            // U+21C0 (not the atom '→'), drawn ￫ U+FFEB
    ("dot", '˙', false, "dot"),            // U+02D9, drawn ․ U+2024
    ("ddot", '¨', false, "ddot"),          // U+00A8, drawn ․․ (overhangs right)
    ("check", 'ˇ', false, "check"),        // U+02C7 CARON, drawn ˯ U+02EF
    ("ring", '˚', false, "mathring"),      // U+02DA, drawn ˳ U+02F3
    ("underline", '‗', true, "underline"), // U+2017 DOUBLE LOW LINE, drawn ¯
    // Under tilde: the AST marks form a swapped pair with the drawn
    // glyphs — over tilde ˜ draws as the low ˷, under tilde ˷ draws as
    // the high ˜ (both hug the base).
    ("utilde", '˷', true, "utilde"), // U+02F7 LOW TILDE, drawn ˜
];

pub fn accent_by_name(name: &str) -> Option<(char, bool)> {
    ACCENTS
        .iter()
        .find(|(n, ..)| *n == name)
        .map(|&(_, c, under, _)| (c, under))
}

pub fn accent_info(mark: char) -> Option<(bool, &'static str)> {
    ACCENTS
        .iter()
        .find(|&&(_, c, ..)| c == mark)
        .map(|&(_, _, under, latex)| (under, latex))
}

/// LaTeX command for a stretchy (multi-char) accent; marks without a
/// wide variant use their plain command (\dot etc. accept groups).
pub fn wide_accent_latex(mark: char) -> &'static str {
    match mark {
        '^' => "widehat",
        '˜' => "widetilde",
        '¯' => "overline",
        '⇀' => "overrightarrow",
        'ˇ' => "widecheck",
        m => accent_info(m).map(|(_, l)| l).unwrap_or("widehat"),
    }
}

pub fn is_over_mark(c: char) -> bool {
    ACCENTS.iter().any(|&(_, m, under, _)| m == c && !under)
}

pub fn is_under_mark(c: char) -> bool {
    ACCENTS.iter().any(|&(_, m, under, _)| m == c && under)
}

/// Multi-word operator names: the command inserts a band with one piece
/// per word (`\argmax` -> ┈arg┈max┈, `\limsup` -> ┈lim┈sup┈), so the
/// spacing matches the words. `latex`/`typst` flag whether the joined
/// name is native there (\limsup); otherwise the serializers group the
/// pieces (\mathop{\arg\max} / op("arg max")).
pub struct WordOp {
    pub name: &'static str,
    pub words: [&'static str; 2],
    pub latex: bool,
    pub typst: bool,
}

pub const WORD_OPS: &[WordOp] = &[
    WordOp {
        name: "argmax",
        words: ["arg", "max"],
        latex: false,
        typst: false,
    },
    WordOp {
        name: "argmin",
        words: ["arg", "min"],
        latex: false,
        typst: false,
    },
    WordOp {
        name: "limsup",
        words: ["lim", "sup"],
        latex: true,
        typst: true,
    },
    WordOp {
        name: "liminf",
        words: ["lim", "inf"],
        latex: true,
        typst: true,
    },
];

pub fn word_op(name: &str) -> Option<&'static WordOp> {
    WORD_OPS.iter().find(|w| w.name == name)
}

/// The word op a run of band pieces spells, if any.
pub fn word_op_of(words: &[&str]) -> Option<&'static WordOp> {
    WORD_OPS.iter().find(|w| w.words == words)
}

/// Big operators available as `\name` (typeset with under/over limits).
pub const BIG_OPS: &[(&str, char)] = &[
    ("sum", '∑'),
    ("prod", '∏'),
    ("coprod", '∐'),
    ("int", '∫'),
    ("iint", '∬'),
    ("oint", '∮'),
    ("bigcup", '⋃'),
    ("bigcap", '⋂'),
    ("bigoplus", '⨁'),
    ("bigotimes", '⨂'),
    ("bigvee", '⋁'),
    ("bigwedge", '⋀'),
];

/// Upright function/operator names (`Node::Func`) — ONE entry per name
/// carries everything the rest of the crate needs to know:
/// - `limits`: the \lim class — the minibuffer command opens a ┈band┈
///   with an under-limit, and ↑/↓ re-promotes a bare one
///   (ast::promotable_base)
/// - `latex`: LaTeX/KaTeX define \name natively; otherwise the
///   serializer emits \operatorname{name} (\div is ÷ in LaTeX, \Re is
///   ℜ — the upright-operator reading needs \operatorname)
/// - `typst`: Typst's math mode predefines it; otherwise op("name")
///
/// ∑-class *symbol* operators live in `BIG_OPS`; multi-word \op* bases
/// are assembled in `ast::op_words`.
pub struct FuncSpec {
    pub name: &'static str,
    pub limits: bool,
    pub latex: bool,
    pub typst: bool,
}

const fn fun(name: &'static str, limits: bool, latex: bool, typst: bool) -> FuncSpec {
    FuncSpec {
        name,
        limits,
        latex,
        typst,
    }
}

pub const FUNCS: &[FuncSpec] = &[
    fun("arccos", false, true, true),
    fun("arcsin", false, true, true),
    fun("arctan", false, true, true),
    fun("arctg", false, true, false),
    fun("arcctg", false, true, false),
    fun("arg", false, true, true),
    fun("ch", false, true, false),
    fun("cos", false, true, true),
    fun("cosec", false, true, false),
    fun("cosh", false, true, true),
    fun("cot", false, true, true),
    fun("cotg", false, true, false),
    fun("coth", false, true, true),
    fun("csc", false, true, true),
    fun("ctg", false, true, true),
    fun("cth", false, true, false),
    fun("deg", false, true, true),
    fun("det", true, true, true),
    fun("dim", false, true, true),
    fun("exp", false, true, true),
    fun("gcd", true, true, true),
    fun("hom", false, true, true),
    fun("inf", true, true, true),
    fun("ker", false, true, true),
    fun("lg", false, true, true),
    fun("lim", true, true, true),
    fun("ln", false, true, true),
    fun("log", false, true, true),
    fun("max", true, true, true),
    fun("min", true, true, true),
    fun("mod", false, true, true),
    fun("sec", false, true, true),
    fun("sh", false, true, false),
    fun("sin", false, true, true),
    fun("sinh", false, true, true),
    fun("sup", true, true, true),
    fun("tan", false, true, true),
    fun("tanh", false, true, true),
    fun("tg", false, true, true),
    fun("th", false, true, false),
    fun("Pr", true, true, true),
    fun("plim", true, true, false),
    fun("injlim", true, true, false),
    fun("projlim", true, true, false),
    fun("asin", false, false, false),
    fun("acos", false, false, false),
    fun("atan", false, false, false),
    fun("acsc", false, false, false),
    fun("asec", false, false, false),
    fun("acot", false, false, false),
    fun("Tr", false, false, false),
    fun("tr", false, false, true),
    fun("rank", false, false, false),
    fun("erf", false, false, false),
    fun("Res", false, false, false),
    fun("res", false, false, false),
    fun("PV", false, false, false),
    fun("pv", false, false, false),
    fun("Re", false, false, false),
    fun("Im", false, false, false),
    fun("grad", false, false, false),
    fun("curl", false, false, false),
];

fn func_spec(name: &str) -> Option<&'static FuncSpec> {
    FUNCS.iter().find(|f| f.name == name)
}

/// The \lim class: takes band limits; its bare form re-promotes.
pub fn func_takes_limits(name: &str) -> bool {
    func_spec(name).is_some_and(|f| f.limits)
}

/// LaTeX/KaTeX know \name natively (else \operatorname{name}).
pub fn latex_knows_func(name: &str) -> bool {
    func_spec(name).is_some_and(|f| f.latex)
}

/// Typst predefines the name in math mode (else op("name")).
pub fn typst_knows_func(name: &str) -> bool {
    func_spec(name).is_some_and(|f| f.typst)
}

pub fn is_func_name(name: &str) -> bool {
    func_spec(name).is_some()
}

/// Longest known function name that is a prefix of `s`.
pub fn func_prefix(s: &str) -> Option<&'static str> {
    FUNCS
        .iter()
        .filter(|f| s.starts_with(f.name))
        .max_by_key(|f| f.name.len())
        .map(|f| f.name)
}

/// Lookup order: curated table, then the alphabet families (\bbR),
/// then the large generated table (from ho-oto/mathematical-symbols,
/// sorted so this is a binary search).
pub fn symbol_by_name(name: &str) -> Option<char> {
    SYMBOLS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|&(_, c)| c)
        .or_else(|| alphabets::alphabet_char(name))
        .or_else(|| {
            ext::EXT_SYMBOLS
                .binary_search_by_key(&name, |&(n, _)| n)
                .ok()
                .map(|i| ext::EXT_SYMBOLS[i].1)
        })
        .filter(|&c| !is_reserved_glyph(c))
}

pub fn bigop_by_name(name: &str) -> Option<char> {
    BIG_OPS.iter().find(|(n, _)| *n == name).map(|&(_, c)| c)
}

pub fn bigop_by_char(c: char) -> bool {
    BIG_OPS.iter().any(|&(_, op)| op == c)
}

/// Reverse lookup for the LaTeX serializer: char -> command name.
pub fn latex_name(c: char) -> Option<&'static str> {
    SYMBOLS
        .iter()
        .chain(BIG_OPS.iter())
        .find(|&&(_, ch)| ch == c)
        .map(|&(n, _)| n)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `symbol_by_name` binary-searches EXT_SYMBOLS, so a regeneration
    /// must keep the table sorted (and free of duplicates).
    #[test]
    fn ext_table_is_sorted() {
        for w in ext::EXT_SYMBOLS.windows(2) {
            assert!(w[0].0 < w[1].0, "unsorted: {:?} then {:?}", w[0].0, w[1].0);
        }
    }

    /// The alphabet families cover both cases, including the letterlike
    /// exceptions that sit outside their block.
    #[test]
    fn alphabet_families_resolve() {
        for (name, want) in [
            ("bbR", 'ℝ'),
            ("bbA", '𝔸'),
            ("bbf", '𝕗'),
            ("calL", 'ℒ'),
            ("scrL", 'ℒ'),
            ("frakg", '𝔤'),
            ("frakH", 'ℌ'),
            ("bfa", '𝐚'),
            ("ttZ", '𝚉'),
            ("sfbfitq", '𝙦'),
            ("calbfA", '𝓐'),
            ("scrbfA", '𝓐'),
            ("bfsf3", '𝟯'),
            ("tt7", '𝟽'),
            ("frkZ", 'ℨ'),
        ] {
            assert_eq!(symbol_by_name(name), Some(want), "\\{}", name);
        }
        // Every family maps all 52 letters (and its digits) to a
        // distinct char, under every alias spelling.
        for fam in alphabets::ALPHABETS {
            let mut seen = std::collections::HashSet::new();
            let chars: Vec<char> = ('A'..='Z')
                .chain('a'..='z')
                .chain(fam.digits.iter().flat_map(|_| '0'..='9'))
                .collect();
            for &l in &chars {
                let c = alphabets::alphabet_char(&format!("{}{}", fam.prefixes[0], l))
                    .unwrap_or_else(|| panic!("{}{} missing", fam.prefixes[0], l));
                assert!(
                    seen.insert(c),
                    "{}{} duplicates {:?}",
                    fam.prefixes[0],
                    l,
                    c
                );
                for alias in fam.prefixes {
                    assert_eq!(
                        alphabets::alphabet_char(&format!("{}{}", alias, l)),
                        Some(c),
                        "alias {}{}",
                        alias,
                        l
                    );
                }
            }
        }
        assert_eq!(symbol_by_name("nosuchfamilyX"), None);
    }
}
