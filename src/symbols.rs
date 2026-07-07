//! Named symbol table: LyX/TeX command name -> Unicode char, plus the
//! reverse mapping used by the LaTeX serializer.

/// (command name, unicode char). The LaTeX command is `\name`.
pub const SYMBOLS: &[(&str, char)] = &[
    // Greek lowercase
    ("alpha", 'α'), ("beta", 'β'), ("gamma", 'γ'), ("delta", 'δ'),
    ("epsilon", 'ε'), ("zeta", 'ζ'), ("eta", 'η'), ("theta", 'θ'),
    ("iota", 'ι'), ("kappa", 'κ'), ("lambda", 'λ'), ("mu", 'μ'),
    ("nu", 'ν'), ("xi", 'ξ'), ("pi", 'π'), ("rho", 'ρ'),
    ("sigma", 'σ'), ("tau", 'τ'), ("upsilon", 'υ'), ("phi", 'φ'),
    ("chi", 'χ'), ("psi", 'ψ'), ("omega", 'ω'),
    ("varphi", 'ϕ'), ("vartheta", 'ϑ'),
    // Greek uppercase
    ("Gamma", 'Γ'), ("Delta", 'Δ'), ("Theta", 'Θ'), ("Lambda", 'Λ'),
    ("Xi", 'Ξ'), ("Pi", 'Π'), ("Sigma", 'Σ'), ("Upsilon", 'Υ'),
    ("Phi", 'Φ'), ("Psi", 'Ψ'), ("Omega", 'Ω'),
    // Binary operators / relations
    ("pm", '±'), ("mp", '∓'), ("times", '×'), ("div", '÷'), ("cdot", '⋅'),
    ("circ", '∘'), ("oplus", '⊕'), ("otimes", '⊗'), ("ast", '∗'),
    ("le", '≤'), ("leq", '≤'), ("ge", '≥'), ("geq", '≥'), ("ne", '≠'),
    ("neq", '≠'), ("approx", '≈'), ("equiv", '≡'), ("sim", '∼'),
    ("simeq", '≃'), ("propto", '∝'), ("ll", '≪'), ("gg", '≫'),
    // Arrows
    ("to", '→'), ("rightarrow", '→'), ("leftarrow", '←'),
    ("Rightarrow", '⇒'), ("Leftarrow", '⇐'), ("leftrightarrow", '↔'),
    ("Leftrightarrow", '⇔'), ("mapsto", '↦'),
    // Sets / logic
    ("in", '∈'), ("notin", '∉'), ("ni", '∋'), ("subset", '⊂'),
    ("subseteq", '⊆'), ("supset", '⊃'), ("supseteq", '⊇'),
    ("cup", '∪'), ("cap", '∩'), ("setminus", '∖'), ("emptyset", '∅'),
    ("forall", '∀'), ("exists", '∃'), ("neg", '¬'), ("land", '∧'),
    ("lor", '∨'), ("vdash", '⊢'), ("models", '⊨'),
    // Misc
    ("infty", '∞'), ("partial", '∂'), ("nabla", '∇'), ("hbar", 'ℏ'),
    ("ell", 'ℓ'), ("Re", 'ℜ'), ("Im", 'ℑ'), ("aleph", 'ℵ'),
    ("angle", '∠'), ("perp", '⊥'), ("parallel", '∥'), ("prime", '′'),
    ("degree", '°'), ("cdots", '⋯'), ("ldots", '…'), ("dots", '…'),
    ("langle", '⟨'), ("rangle", '⟩'),
];

/// Big operators available as `\name` (typeset with under/over limits).
pub const BIG_OPS: &[(&str, char)] = &[
    ("sum", '∑'), ("prod", '∏'), ("coprod", '∐'),
    ("int", '∫'), ("iint", '∬'), ("oint", '∮'),
    ("bigcup", '⋃'), ("bigcap", '⋂'),
    ("bigoplus", '⨁'), ("bigotimes", '⨂'), ("bigvee", '⋁'), ("bigwedge", '⋀'),
];

pub fn symbol_by_name(name: &str) -> Option<char> {
    SYMBOLS.iter().find(|(n, _)| *n == name).map(|&(_, c)| c)
}

pub fn bigop_by_name(name: &str) -> Option<char> {
    BIG_OPS.iter().find(|(n, _)| *n == name).map(|&(_, c)| c)
}

/// Reverse lookup for the LaTeX serializer: char -> command name.
pub fn latex_name(c: char) -> Option<&'static str> {
    SYMBOLS
        .iter()
        .chain(BIG_OPS.iter())
        .find(|&&(_, ch)| ch == c)
        .map(|&(n, _)| n)
}

/// Characters treated as binary operators / relations for render spacing.
pub fn is_spaced_op(c: char) -> bool {
    matches!(
        c,
        '+' | '-' | '=' | '<' | '>' | '±' | '∓' | '×' | '÷' | '⋅' | '∘'
            | '⊕' | '⊗' | '≤' | '≥' | '≠' | '≈' | '≡' | '∼' | '≃' | '∝'
            | '≪' | '≫' | '→' | '←' | '⇒' | '⇐' | '↔' | '⇔' | '↦'
            | '∈' | '∉' | '∋' | '⊂' | '⊆' | '⊃' | '⊇' | '∪' | '∩' | '∖'
            | '∧' | '∨' | '⊢' | '⊨'
    )
}
