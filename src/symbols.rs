//! Named symbol table: LyX/TeX command name -> Unicode char, plus the
//! reverse mapping used by the LaTeX serializer.

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
pub fn is_reserved_glyph(c: char) -> bool {
    matches!(
        c,
        '─' | '┄'
            | '═'
            | '√'
            | '∛'
            | '∜'
            | '│'
            | '⬚'
            | '▌'
            | '"'
            | '_'
            | '⎛'
            | '⎜'
            | '⎝'
            | '⎞'
            | '⎟'
            | '⎠'
            | '⎡'
            | '⎢'
            | '⎣'
            | '⎤'
            | '⎥'
            | '⎦'
            | '('
            | ')'
            | '['
            | ']'
            | '{'
            | '}'
            | '⟨'
            | '⟩'
            | '⎧'
            | '⎨'
            | '⎩'
            | '⎪'
            | '⎫'
            | '⎬'
            | '⎭'
            | '╱'
            | '╲'
            | '⎸'
            | '⎹'
            | '▏'
            | '▕'
            | '┌'
            | '┬'
            | '┐'
            | '├'
            | '┼'
            | '┤'
            | '└'
            | '┴'
            | '┘'
            | '┠'
            | '┨'
            | '╭'
            | '╮'
            | '╰'
            | '╯'
    ) || is_over_mark(c)
        || is_under_mark(c)
        || crate::render::unsuperscript_char(c).is_some()
        || crate::render::unsubscript_char(c).is_some()
}

/// Accent marks: (command name, mark char, is_under, latex command).
/// Mark chars are RESERVED — they never occur as atoms, and over-marks are
/// disjoint from under-marks; this is what makes a two-character column
/// (mark stacked on base) unambiguous for the parser.
pub const ACCENTS: &[(&str, char, bool, &str)] = &[
    ("hat", '^', false, "hat"),
    ("tilde", '˜', false, "tilde"), // U+02DC, distinct from the atom '~'
    ("bar", '¯', false, "bar"),
    ("vec", '⇀', false, "vec"), // U+21C0, distinct from the atom '→'
    ("dot", '˙', false, "dot"),
    ("ddot", '¨', false, "ddot"),
    ("check", 'ˇ', false, "check"),
    ("breve", '˘', false, "breve"),
    ("ring", '˚', false, "mathring"),
    ("underline", '‗', true, "underline"), // U+2017
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

pub fn is_over_mark(c: char) -> bool {
    ACCENTS.iter().any(|&(_, m, under, _)| m == c && !under)
}

pub fn is_under_mark(c: char) -> bool {
    ACCENTS.iter().any(|&(_, m, under, _)| m == c && under)
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

/// Function/operator names rendered upright (Node::Func). Sorted so that
/// longer names are matched first by the parser (greedy longest match).
pub const FUNCS: &[&str] = &[
    "arccos", "arcsin", "arctan", "arg", "cos", "cosh", "cot", "coth", "csc", "deg", "det", "dim",
    "exp", "gcd", "hom", "inf", "ker", "lg", "lim", "liminf", "limsup", "ln", "log", "max", "min",
    "mod", "sec", "sin", "sinh", "sup", "tan", "tanh", "Pr",
];

pub fn is_func_name(name: &str) -> bool {
    FUNCS.contains(&name)
}

/// Longest known function name that is a prefix of `s`.
pub fn func_prefix(s: &str) -> Option<&'static str> {
    FUNCS
        .iter()
        .filter(|f| s.starts_with(**f))
        .max_by_key(|f| f.len())
        .copied()
}

/// Lookup order: curated table first, then the large generated table
/// (from https://github.com/ho-oto/mathematical-symbols).
pub fn symbol_by_name(name: &str) -> Option<char> {
    SYMBOLS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|&(_, c)| c)
        .or_else(|| {
            crate::symbols_ext::EXT_SYMBOLS
                .iter()
                .find(|(n, _)| *n == name)
                .map(|&(_, c)| c)
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
