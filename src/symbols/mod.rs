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

/// Every character the format accepts as an atom. Derived from the
/// tables themselves — a char is an atom exactly when some `\name`
/// produces it (or it is plain ASCII, which the keyboard types
/// directly), never a hand-kept list.
///
/// This is an allow-list on purpose. The layout model is a grid of
/// one-cell characters, so a full-width or combining char pasted into
/// a formula would silently shift every column; and a char outside the
/// tables has no LaTeX spelling to convert to. Both failures are
/// invisible until much later, so they are rejected at the door.
pub fn is_atom(c: char) -> bool {
    use std::sync::OnceLock;
    static ATOMS: OnceLock<std::collections::HashSet<char>> = OnceLock::new();
    if c.is_ascii() {
        // ASCII the keyboard types directly, minus the reserved glyphs
        // and the four that LaTeX would misread as syntax with no
        // sensible atom meaning (`^` `~` `` ` `` `\` — the symbols are
        // \sim, \backslash …). `# $ % &` stay: they are ordinary
        // characters that the serializer escapes.
        return c.is_ascii_graphic()
            && !is_reserved_glyph(c)
            && !matches!(c, '^' | '~' | '`' | '\\');
    }
    ATOMS
        .get_or_init(|| {
            SYMBOLS
                .iter()
                .map(|&(_, c)| c)
                .chain(BIG_OPS.iter().map(|&(_, c)| c))
                .chain(ext::EXT_SYMBOLS.values().copied())
                .chain(alphabets::ALPHABETS.iter().flat_map(|a| {
                    ('A'..='Z')
                        .chain('a'..='z')
                        .chain('0'..='9')
                        .filter_map(|l| {
                            alphabets::alphabet_char(&format!("{}{}", a.prefixes[0], l))
                        })
                }))
                .filter(|&c| !is_reserved_glyph(c))
                .collect()
        })
        .contains(&c)
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
/// - `spaced`: inner text for \operatorname when the name reads as
///   several words (`argmax` -> `arg\,max`); None = the name verbatim (\div is ÷ in LaTeX, \Re is
///   ℜ — the upright-operator reading needs \operatorname)
///
/// ∑-class *symbol* operators live in `BIG_OPS`; multi-word \op* bases
/// are assembled in `ast::op_words`.
pub struct FuncSpec {
    pub name: &'static str,
    pub limits: bool,
    pub spaced: Option<&'static str>,
}

const fn fun(name: &'static str, limits: bool) -> FuncSpec {
    FuncSpec {
        name,
        limits,
        spaced: None,
    }
}

/// A name that reads as several words in LaTeX (`arg\,max`).
const fn words(name: &'static str, spaced: &'static str) -> FuncSpec {
    FuncSpec {
        name,
        limits: true,
        spaced: Some(spaced),
    }
}

pub const FUNCS: &[FuncSpec] = &[
    fun("arccos", false),
    fun("arcsin", false),
    fun("arctan", false),
    fun("arctg", false),
    fun("arcctg", false),
    fun("arg", false),
    fun("ch", false),
    fun("cos", false),
    fun("cosec", false),
    fun("cosh", false),
    fun("cot", false),
    fun("cotg", false),
    fun("coth", false),
    fun("csc", false),
    fun("ctg", false),
    fun("cth", false),
    fun("deg", false),
    fun("det", true),
    fun("dim", false),
    fun("exp", false),
    fun("gcd", true),
    fun("hom", false),
    fun("inf", true),
    fun("ker", false),
    fun("lg", false),
    fun("lim", true),
    fun("ln", false),
    fun("log", false),
    fun("max", true),
    fun("min", true),
    fun("mod", false),
    fun("sec", false),
    fun("sh", false),
    fun("sin", false),
    fun("sinh", false),
    fun("sup", true),
    fun("tan", false),
    fun("tanh", false),
    fun("tg", false),
    fun("th", false),
    fun("Pr", true),
    fun("plim", true),
    fun("injlim", true),
    fun("projlim", true),
    // Names that read as several words in LaTeX.
    words("argmax", "arg\\,max"),
    words("argmin", "arg\\,min"),
    words("limsup", "lim\\,sup"),
    words("liminf", "lim\\,inf"),
    fun("asin", false),
    fun("acos", false),
    fun("atan", false),
    fun("acsc", false),
    fun("asec", false),
    fun("acot", false),
    fun("Tr", false),
    fun("tr", false),
    fun("rank", false),
    fun("erf", false),
    fun("Res", false),
    fun("res", false),
    fun("PV", false),
    fun("pv", false),
    fun("Re", false),
    fun("Im", false),
    fun("grad", false),
    fun("curl", false),
];

fn func_spec(name: &str) -> Option<&'static FuncSpec> {
    FUNCS.iter().find(|f| f.name == name)
}

/// The \lim class: takes band limits; its bare form re-promotes.
pub fn func_takes_limits(name: &str) -> bool {
    func_spec(name).is_some_and(|f| f.limits)
}

/// Inner text for \operatorname: the name, or its spaced reading.
pub fn func_latex_text(name: &str) -> &str {
    func_spec(name).and_then(|f| f.spaced).unwrap_or(name)
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

/// Lookup order: curated table, then the styled families (\bbR \supA),
/// then the large generated table (from ho-oto/mathematical-symbols,
/// a compile-time perfect hash).
pub fn symbol_by_name(name: &str) -> Option<char> {
    SYMBOLS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|&(_, c)| c)
        .or_else(|| alphabets::styled_char(name))
        .or_else(|| ext::EXT_SYMBOLS.get(name).copied())
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

/// The LaTeX spelling of an atom: a curated `\name`, a styled letter
/// (`𝔸` -> `\mathbb{A}`), or None when only the raw character is left.
pub fn latex_of(c: char) -> Option<String> {
    latex_name(c)
        .map(|n| format!("\\{} ", n))
        .or_else(|| alphabets::styled_latex(c))
}

#[cfg(test)]
mod tests {
    use super::*;

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
        // Both spelling orders, letters and digits.
        for (a, b) in [("frakA", "Afrk"), ("bfsf3", "3bfsf"), ("ttz", "ztt")] {
            assert_eq!(symbol_by_name(a), symbol_by_name(b), "{} vs {}", a, b);
            assert!(symbol_by_name(a).is_some(), "{}", a);
        }
    }

    /// The style token may lead or trail, so a name like `\bbb` has two
    /// readings. Exhaustively check that they agree (and that every
    /// spelling resolves), so the lookup is well-defined.
    #[test]
    fn alphabet_spellings_agree() {
        for fam in alphabets::ALPHABETS {
            for style in fam.prefixes {
                for ch in ('A'..='Z')
                    .chain('a'..='z')
                    .chain(fam.digits.iter().flat_map(|_| '0'..='9'))
                {
                    let pre = alphabets::alphabet_char(&format!("{}{}", style, ch));
                    let suf = alphabets::alphabet_char(&format!("{}{}", ch, style));
                    assert!(pre.is_some(), "\\{}{} missing", style, ch);
                    // …the trailing spelling agrees, except for the one
                    // collision pinned down below.
                    if format!("{}{}", ch, style) != "bbf" {
                        assert_eq!(pre, suf, "\\{}{} vs \\{}{}", style, ch, ch, style);
                    }
                }
            }
        }
        // \bbf is the only spelling both readings claim (bb+f vs b+bf):
        // the leading style wins, and both chars stay reachable.
        for a in alphabets::ALPHABETS {
            for lead in a.prefixes {
                for b in alphabets::ALPHABETS {
                    for trail in b.prefixes {
                        if lead.len() != trail.len() || lead[1..] != trail[..trail.len() - 1] {
                            continue;
                        }
                        let name = format!("{}{}", lead, &trail[trail.len() - 1..]);
                        let x = name.chars().next().unwrap();
                        let y = name.chars().next_back().unwrap();
                        let (as_lead, as_trail) = (
                            alphabets::alphabet_char(&format!("{}{}", lead, y)),
                            alphabets::alphabet_char(&format!("{}{}", x, trail)),
                        );
                        if as_lead != as_trail {
                            assert_eq!(name, "bbf", "new collision: \\{}", name);
                            assert_eq!(alphabets::alphabet_char("bbf"), as_lead);
                            assert_eq!(alphabets::alphabet_char("fbb"), as_lead);
                            assert_eq!(alphabets::alphabet_char("bfb"), as_trail);
                        }
                    }
                }
            }
        }
    }

    /// Every atom is exactly one cell wide and stands alone — that is
    /// what the whole grid layout rests on, so it is checked over the
    /// entire accepted set rather than trusted.
    #[test]
    fn every_atom_is_one_narrow_char() {
        let wide = |c: char| {
            // The East Asian Wide/Fullwidth blocks, plus emoji.
            matches!(c as u32,
                0x1100..=0x115F | 0x2E80..=0xA4CF | 0xA960..=0xA97F
                | 0xAC00..=0xD7A3 | 0xF900..=0xFAFF | 0xFE10..=0xFE19
                | 0xFE30..=0xFE6F | 0xFF00..=0xFF60 | 0xFFE0..=0xFFE6
                | 0x1F300..=0x1FAFF | 0x20000..=0x3FFFD)
        };
        let mut n = 0;
        for c in char::from_u32(0)
            .into_iter()
            .chain((1..=0x2FFFFu32).filter_map(char::from_u32))
        {
            if !is_atom(c) {
                continue;
            }
            n += 1;
            assert!(!wide(c), "{:?} (U+{:04X}) is not narrow", c, c as u32);
            assert!(
                !matches!(c as u32, 0x300..=0x36F) && c != '\u{200B}',
                "{:?} is a combining/zero-width char",
                c
            );
        }
        assert!(n > 500, "the atom set looks too small: {}", n);
        // The halfwidth arrow is a *drawn* accent glyph, not an atom.
        assert!(!is_atom('￫'));
        for c in ['😀', '漢', '\u{0301}', '─', '┈'] {
            assert!(!is_atom(c), "{:?} must be rejected", c);
        }
    }

    /// Every styled letter has a LaTeX spelling through its family, so
    /// the char -> LaTeX direction is total over the ~700 characters
    /// they generate. What is left over is measured, not assumed: a
    /// regression that widens the gap fails here.
    #[test]
    fn latex_spelling_covers_the_styled_families() {
        for fam in alphabets::ALPHABETS {
            for l in ('A'..='Z').chain('a'..='z') {
                let c = alphabets::alphabet_char(&format!("{}{}", fam.prefixes[0], l)).unwrap();
                let want = format!("\\{}{{{}}}", fam.latex, l);
                // A letterlike symbol the curated table names (ℑ = \Im)
                // keeps that name — it wins on purpose.
                let got = latex_of(c).unwrap_or_default();
                assert!(
                    got == want || latex_name(c).is_some(),
                    "{}{}: {:?}",
                    fam.prefixes[0],
                    l,
                    got
                );
            }
        }
        let gap = (1..=0x2FFFFu32)
            .filter_map(char::from_u32)
            .filter(|&c| is_atom(c) && !c.is_ascii() && latex_of(c).is_none())
            .count();
        // The rest are emitted raw, which unicode-math renders — the
        // toolchain this crate already assumes for exotic symbols.
        assert!(gap <= 121, "the LaTeX gap grew to {}", gap);
    }

    /// The style token never swallows an ordinary name that starts with it.
    #[test]
    fn script_letters_resolve_without_eating_names() {
        // The sup/sub modifier letters are gone (a superscript is
        // structure: `\^h`), so the names they used to shadow keep
        // their own meaning and the look-alike atoms are rejected.
        for (name, want) in [
            ("supset", '⊃'),
            ("subset", '⊂'),
            ("supseteq", '⊇'),
            ("subseteq", '⊆'),
        ] {
            assert_eq!(symbol_by_name(name), Some(want), "\\{}", name);
        }
        assert_eq!(symbol_by_name("supA"), None);
        assert!(!is_atom('ᴬ'), "a modifier letter is not an atom");
    }
}
