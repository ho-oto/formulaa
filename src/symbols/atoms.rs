//! The atom vocabulary: which `\name` makes which character, which
//! characters are structure-reserved, and the allow-list gate every
//! parsed or typed character passes through.

use super::{alphabets, ext};

/// What a curated character *is*: `latex` is the one spelling the
/// serializer writes, and `kind` is how the editor materializes it —
/// a plain atom, or a ∑-class operator that comes in as a band. This
/// is the *output* table (char -> info, the char being the key); the
/// input direction (spelling -> char, including the extra spellings
/// like `\leq` for ≤) is `NAMES`, and the tests hold the two mirrors
/// together.
///
/// Most spellings also appear in the generated `ext` table. That
/// overlap is deliberate, not redundancy: it **pins** the meaning of
/// the names worth guaranteeing, so `ext` can be regenerated from
/// upstream without `\to` quietly changing what it produces. One
/// entry disagrees with upstream on purpose (`INTENTIONAL_OVERRIDES`),
/// and a test fails if a new divergence appears silently.
pub struct AtomSpec {
    pub latex: &'static str,
    pub kind: AtomKind,
}

/// How the editor materializes an atom when its name is typed.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AtomKind {
    /// An ordinary atom: insert the character.
    Sym,
    /// A ∑-class operator: insert the ┈-band with the cursor in the
    /// lower limit (the bare char is the collapsed form).
    BigOp,
}

const fn sym(latex: &'static str) -> AtomSpec {
    AtomSpec {
        latex,
        kind: AtomKind::Sym,
    }
}

const fn big(latex: &'static str) -> AtomSpec {
    AtomSpec {
        latex,
        kind: AtomKind::BigOp,
    }
}

pub static ATOMS: phf::Map<char, AtomSpec> = phf::phf_map! {
    // Greek lowercase
    'α' => sym("alpha"),
    'β' => sym("beta"),
    'γ' => sym("gamma"),
    'δ' => sym("delta"),
    'ϵ' => sym("epsilon"),
    'ε' => sym("varepsilon"),
    'ζ' => sym("zeta"),
    'η' => sym("eta"),
    'θ' => sym("theta"),
    'ι' => sym("iota"),
    'κ' => sym("kappa"),
    'λ' => sym("lambda"),
    'μ' => sym("mu"),
    'ν' => sym("nu"),
    'ξ' => sym("xi"),
    'π' => sym("pi"),
    'ρ' => sym("rho"),
    'σ' => sym("sigma"),
    'τ' => sym("tau"),
    'υ' => sym("upsilon"),
    'ϕ' => sym("phi"),
    'χ' => sym("chi"),
    'ψ' => sym("psi"),
    'ω' => sym("omega"),
    'φ' => sym("varphi"),
    'ϑ' => sym("vartheta"),
    'ϰ' => sym("varkappa"),
    // Greek uppercase
    'Γ' => sym("Gamma"),
    'Δ' => sym("Delta"),
    'Θ' => sym("Theta"),
    'Λ' => sym("Lambda"),
    'Ξ' => sym("Xi"),
    'Π' => sym("Pi"),
    'Σ' => sym("Sigma"),
    'Υ' => sym("Upsilon"),
    'Φ' => sym("Phi"),
    'Ψ' => sym("Psi"),
    'Ω' => sym("Omega"),
    // Binary operators / relations
    '±' => sym("pm"),
    '∓' => sym("mp"),
    '×' => sym("times"),
    '÷' => sym("div"),
    '⋅' => sym("cdot"),
    '∘' => sym("circ"),
    '⊕' => sym("oplus"),
    '⊗' => sym("otimes"),
    '∗' => sym("ast"),
    '≤' => sym("le"),
    '≥' => sym("ge"),
    '≠' => sym("ne"),
    '≈' => sym("approx"),
    '≡' => sym("equiv"),
    '∼' => sym("sim"),
    '≃' => sym("simeq"),
    '∝' => sym("propto"),
    '≪' => sym("ll"),
    '≫' => sym("gg"),
    // Arrows
    '→' => sym("to"),
    '←' => sym("leftarrow"),
    '⇒' => sym("Rightarrow"),
    '⇐' => sym("Leftarrow"),
    '↔' => sym("leftrightarrow"),
    '⇔' => sym("Leftrightarrow"),
    '↦' => sym("mapsto"),
    // Sets / logic
    '∈' => sym("in"),
    '∉' => sym("notin"),
    '∋' => sym("ni"),
    '⊂' => sym("subset"),
    '⊆' => sym("subseteq"),
    '⊃' => sym("supset"),
    '⊇' => sym("supseteq"),
    '∪' => sym("cup"),
    '∩' => sym("cap"),
    '∖' => sym("setminus"),
    '∅' => sym("emptyset"),
    '∀' => sym("forall"),
    '∃' => sym("exists"),
    '¬' => sym("neg"),
    '∧' => sym("land"),
    '∨' => sym("lor"),
    '⊢' => sym("vdash"),
    '⊨' => sym("models"),
    // Misc
    '∞' => sym("infty"),
    '∂' => sym("partial"),
    '∇' => sym("nabla"),
    'ℏ' => sym("hbar"),
    'ℓ' => sym("ell"),
    'ℜ' => sym("Re"),
    'ℑ' => sym("Im"),
    'ℵ' => sym("aleph"),
    '∠' => sym("angle"),
    '⊥' => sym("perp"),
    '∥' => sym("parallel"),
    '′' => sym("prime"),
    '°' => sym("degree"),
    '⋯' => sym("cdots"),
    '…' => sym("ldots"),
    '⋮' => sym("vdots"),
    '⋱' => sym("ddots"),
    // Explicit space atom (the Space key): visible ␣ in canonical AA so the
    // picture stays parseable (a real blank column is a sibling separator).
    '␣' => sym("space"),
    // Arrows (amssymb tier)
    '↑' => sym("uparrow"),
    '↓' => sym("downarrow"),
    '↕' => sym("updownarrow"),
    '↖' => sym("nwarrow"),
    '↗' => sym("nearrow"),
    '↘' => sym("searrow"),
    '↙' => sym("swarrow"),
    '↞' => sym("twoheadleftarrow"),
    '↠' => sym("twoheadrightarrow"),
    '↢' => sym("leftarrowtail"),
    '↣' => sym("rightarrowtail"),
    '↩' => sym("hookleftarrow"),
    '↪' => sym("hookrightarrow"),
    '⇄' => sym("rightleftarrows"),
    '⇆' => sym("leftrightarrows"),
    '⇇' => sym("leftleftarrows"),
    '⇈' => sym("upuparrows"),
    '⇉' => sym("rightrightarrows"),
    '⇊' => sym("downdownarrows"),
    '⇑' => sym("Uparrow"),
    '⇓' => sym("Downarrow"),
    '⇕' => sym("Updownarrow"),
    '⇝' => sym("rightsquigarrow"),
    '⟵' => sym("longleftarrow"),
    '⟶' => sym("longrightarrow"),
    '⟷' => sym("longleftrightarrow"),
    '⟸' => sym("Longleftarrow"),
    '⟹' => sym("Longrightarrow"),
    '⟺' => sym("Longleftrightarrow"),
    '⟼' => sym("longmapsto"),
    // Relations and orders (amssymb tier)
    '≅' => sym("cong"),
    '≊' => sym("approxeq"),
    '≔' => sym("coloneqq"),
    '≕' => sym("eqqcolon"),
    '≦' => sym("leqq"),
    '≧' => sym("geqq"),
    '≨' => sym("lneqq"),
    '≩' => sym("gneqq"),
    '≲' => sym("lesssim"),
    '≳' => sym("gtrsim"),
    '≺' => sym("prec"),
    '≻' => sym("succ"),
    '≼' => sym("preccurlyeq"),
    '≽' => sym("succcurlyeq"),
    '⊊' => sym("subsetneq"),
    '⊋' => sym("supsetneq"),
    '⊏' => sym("sqsubset"),
    '⊐' => sym("sqsupset"),
    '⊑' => sym("sqsubseteq"),
    '⊒' => sym("sqsupseteq"),
    '⊣' => sym("dashv"),
    '⊤' => sym("top"),
    '⊩' => sym("Vdash"),
    '⊲' => sym("vartriangleleft"),
    '⊳' => sym("vartriangleright"),
    '⊴' => sym("trianglelefteq"),
    '⊵' => sym("trianglerighteq"),
    '⋦' => sym("lnsim"),
    '⋧' => sym("gnsim"),
    '⋘' => sym("lll"),
    '⋙' => sym("ggg"),
    '⪅' => sym("lessapprox"),
    '⪆' => sym("gtrapprox"),
    '⪇' => sym("lneq"),
    '⪈' => sym("gneq"),
    '⪉' => sym("lnapprox"),
    '⪊' => sym("gnapprox"),
    '⫅' => sym("subseteqq"),
    '⫆' => sym("supseteqq"),
    '⫋' => sym("subsetneqq"),
    '⫌' => sym("supsetneqq"),
    '∴' => sym("therefore"),
    '∵' => sym("because"),
    // Operators (amssymb tier)
    '†' => sym("dagger"),
    '‡' => sym("ddagger"),
    '∁' => sym("complement"),
    '∔' => sym("dotplus"),
    '∙' => sym("bullet"),
    '⊓' => sym("sqcap"),
    '⊔' => sym("sqcup"),
    '⊖' => sym("ominus"),
    '⊙' => sym("odot"),
    '⊻' => sym("veebar"),
    '⊼' => sym("barwedge"),
    '⋆' => sym("star"),
    '⋈' => sym("bowtie"),
    '⋉' => sym("ltimes"),
    '⋊' => sym("rtimes"),
    // ∑-class big operators (amsmath tier)
    '∭' => big("iiint"),
    '⨀' => big("bigodot"),
    '⨆' => big("bigsqcup"),
    // ∑-class big operators (band-promotable; see AtomKind::BigOp).
    '∑' => big("sum"),
    '∏' => big("prod"),
    '∐' => big("coprod"),
    '∫' => big("int"),
    '∬' => big("iint"),
    '∮' => big("oint"),
    '⋃' => big("bigcup"),
    '⋂' => big("bigcap"),
    '⨁' => big("bigoplus"),
    '⨂' => big("bigotimes"),
    '⋁' => big("bigvee"),
    '⋀' => big("bigwedge"),
};

/// The curated row for a character.
pub fn atom_of(c: char) -> Option<&'static AtomSpec> {
    ATOMS.get(&c)
}

/// The input direction, spelled out: every `\name` that types a
/// curated character, alternative spellings or-ed onto the canonical
/// one (which comes first) in the same entry. Deliberately a
/// hand-written mirror of `ATOMS` rather than derived from it, so each
/// direction reads on its own; a spelling claimed twice fails the
/// build (phf), and the tests pin the two tables together (same chars,
/// every canonical spelling present). Command aliases (`\sqrt3` =
/// `\cbrt`) are not names of a character, so they live as extra
/// patterns in `resolve`'s match.
pub static NAMES: phf::Map<&'static str, char> = phf::phf_map! {
    // Greek lowercase
    "alpha" => 'α',
    "beta" => 'β',
    "gamma" => 'γ',
    "delta" => 'δ',
    "epsilon" => 'ϵ',
    "varepsilon" => 'ε',
    "zeta" => 'ζ',
    "eta" => 'η',
    "theta" => 'θ',
    "iota" => 'ι',
    "kappa" => 'κ',
    "lambda" => 'λ',
    "mu" => 'μ',
    "nu" => 'ν',
    "xi" => 'ξ',
    "pi" => 'π',
    "rho" => 'ρ',
    "sigma" => 'σ',
    "tau" => 'τ',
    "upsilon" => 'υ',
    "phi" => 'ϕ',
    "chi" => 'χ',
    "psi" => 'ψ',
    "omega" => 'ω',
    "varphi" => 'φ',
    "vartheta" => 'ϑ',
    "varkappa" => 'ϰ',
    // Greek uppercase
    "Gamma" => 'Γ',
    "Delta" => 'Δ',
    "Theta" => 'Θ',
    "Lambda" => 'Λ',
    "Xi" => 'Ξ',
    "Pi" => 'Π',
    "Sigma" => 'Σ',
    "Upsilon" => 'Υ',
    "Phi" => 'Φ',
    "Psi" => 'Ψ',
    "Omega" => 'Ω',
    // Binary operators / relations
    "pm" => '±',
    "mp" => '∓',
    "times" => '×',
    "div" => '÷',
    "cdot" => '⋅',
    "circ" => '∘',
    "oplus" => '⊕',
    "otimes" => '⊗',
    "ast" => '∗',
    "le" | "leq" => '≤',
    "ge" | "geq" => '≥',
    "ne" | "neq" => '≠',
    "approx" => '≈',
    "equiv" => '≡',
    "sim" => '∼',
    "simeq" => '≃',
    "propto" => '∝',
    "ll" => '≪',
    "gg" => '≫',
    // Arrows
    "to" | "rightarrow" => '→',
    "leftarrow" => '←',
    "Rightarrow" => '⇒',
    "Leftarrow" => '⇐',
    "leftrightarrow" => '↔',
    "Leftrightarrow" => '⇔',
    "mapsto" => '↦',
    // Sets / logic
    "in" => '∈',
    "notin" => '∉',
    "ni" => '∋',
    "subset" => '⊂',
    "subseteq" => '⊆',
    "supset" => '⊃',
    "supseteq" => '⊇',
    "cup" => '∪',
    "cap" => '∩',
    "setminus" => '∖',
    "emptyset" => '∅',
    "forall" => '∀',
    "exists" => '∃',
    "neg" => '¬',
    "land" => '∧',
    "lor" => '∨',
    "vdash" => '⊢',
    "models" => '⊨',
    // Misc
    "infty" => '∞',
    "partial" => '∂',
    "nabla" => '∇',
    "hbar" => 'ℏ',
    "ell" => 'ℓ',
    "Re" => 'ℜ',
    "Im" => 'ℑ',
    "aleph" => 'ℵ',
    "angle" => '∠',
    "perp" => '⊥',
    "parallel" => '∥',
    "prime" => '′',
    "degree" => '°',
    "cdots" => '⋯',
    "ldots" | "dots" => '…',
    "vdots" => '⋮',
    "ddots" => '⋱',
    "space" => '␣',
    // Arrows (amssymb tier)
    "uparrow" => '↑',
    "downarrow" => '↓',
    "updownarrow" => '↕',
    "nwarrow" => '↖',
    "nearrow" => '↗',
    "searrow" => '↘',
    "swarrow" => '↙',
    "twoheadleftarrow" => '↞',
    "twoheadrightarrow" => '↠',
    "leftarrowtail" => '↢',
    "rightarrowtail" => '↣',
    "hookleftarrow" => '↩',
    "hookrightarrow" => '↪',
    "rightleftarrows" => '⇄',
    "leftrightarrows" => '⇆',
    "leftleftarrows" => '⇇',
    "upuparrows" => '⇈',
    "rightrightarrows" => '⇉',
    "downdownarrows" => '⇊',
    "Uparrow" => '⇑',
    "Downarrow" => '⇓',
    "Updownarrow" => '⇕',
    "rightsquigarrow" => '⇝',
    "longleftarrow" => '⟵',
    "longrightarrow" => '⟶',
    "longleftrightarrow" => '⟷',
    "Longleftarrow" => '⟸',
    "Longrightarrow" => '⟹',
    "Longleftrightarrow" => '⟺',
    "longmapsto" => '⟼',
    // Relations and orders (amssymb tier)
    "cong" => '≅',
    "approxeq" => '≊',
    "coloneqq" => '≔',
    "eqqcolon" => '≕',
    "leqq" => '≦',
    "geqq" => '≧',
    "lneqq" => '≨',
    "gneqq" => '≩',
    "lesssim" => '≲',
    "gtrsim" => '≳',
    "prec" => '≺',
    "succ" => '≻',
    "preccurlyeq" => '≼',
    "succcurlyeq" => '≽',
    "subsetneq" => '⊊',
    "supsetneq" => '⊋',
    "sqsubset" => '⊏',
    "sqsupset" => '⊐',
    "sqsubseteq" => '⊑',
    "sqsupseteq" => '⊒',
    "dashv" => '⊣',
    "top" => '⊤',
    "Vdash" => '⊩',
    "vartriangleleft" => '⊲',
    "vartriangleright" => '⊳',
    "trianglelefteq" => '⊴',
    "trianglerighteq" => '⊵',
    "lnsim" => '⋦',
    "gnsim" => '⋧',
    "lll" => '⋘',
    "ggg" => '⋙',
    "lessapprox" => '⪅',
    "gtrapprox" => '⪆',
    "lneq" => '⪇',
    "gneq" => '⪈',
    "lnapprox" => '⪉',
    "gnapprox" => '⪊',
    "subseteqq" => '⫅',
    "supseteqq" => '⫆',
    "subsetneqq" => '⫋',
    "supsetneqq" => '⫌',
    "therefore" => '∴',
    "because" => '∵',
    // Operators (amssymb tier)
    "dagger" => '†',
    "ddagger" => '‡',
    "complement" => '∁',
    "dotplus" => '∔',
    "bullet" => '∙',
    "sqcap" => '⊓',
    "sqcup" => '⊔',
    "ominus" => '⊖',
    "odot" => '⊙',
    "veebar" => '⊻',
    "barwedge" => '⊼',
    "star" => '⋆',
    "bowtie" => '⋈',
    "ltimes" => '⋉',
    "rtimes" => '⋊',
    // ∑-class big operators (amsmath tier)
    "iiint" => '∭',
    "bigodot" => '⨀',
    "bigsqcup" => '⨆',
    // ∑-class big operators
    "sum" => '∑',
    "prod" => '∏',
    "coprod" => '∐',
    "int" => '∫',
    "iint" => '∬',
    "oint" => '∮',
    "bigcup" => '⋃',
    "bigcap" => '⋂',
    "bigoplus" => '⨁',
    "bigotimes" => '⨂',
    "bigvee" => '⋁',
    "bigwedge" => '⋀',
};

/// The char a curated `\name` types (any spelling in its `NAMES` entry).
pub fn named_char(name: &str) -> Option<char> {
    NAMES.get(name).copied()
}

/// Names where the curated table deliberately differs from the
/// generated one: the TeX convention is not what upstream picked, and
/// the curated spelling wins.
pub const INTENTIONAL_OVERRIDES: &[&str] = &["hbar"];

/// Structure-reserved glyphs: never valid as atoms (see docs/aa-spec.md §2).
/// `symbol_by_name` filters these so no symbol table entry can inject one.
/// Accent marks, the √ overline `_`, and the inline super/subscript
/// codepoints are reserved too — they read back as structure, not atoms.
#[rustfmt::skip]
pub fn is_reserved_glyph(c: char) -> bool {
    matches!(
        c,
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
        | '￫' // U+FFEB HALFWIDTH RIGHTWARDS ARROW: vec
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
    static ATOM_SET: OnceLock<std::collections::HashSet<char>> = OnceLock::new();
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
    ATOM_SET
        .get_or_init(|| {
            ATOMS
                .keys()
                .copied()
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

#[cfg(test)]
mod tests {
    use crate::symbols::*;
    /// The two hand-written tables mirror each other: every atom's
    /// canonical `latex` spelling is claimed by exactly one char and
    /// types it back through `NAMES` itself (not the ext fallback) —
    /// \le and \leq type the same ≤ but it always prints as \le — and
    /// every `NAMES` entry targets a curated atom. A spelling written
    /// twice is a phf build error, so uniqueness needs no test.
    #[test]
    fn atom_rows_are_canonical() {
        let mut names = std::collections::HashSet::new();
        for (&ch, a) in ATOMS.entries() {
            assert!(names.insert(a.latex), "\\{} is claimed twice", a.latex);
            assert_eq!(latex_name(ch), Some(a.latex));
            assert_eq!(named_char(a.latex), Some(ch), "\\{}", a.latex);
            assert_eq!(symbol_by_name(a.latex), Some(ch), "\\{}", a.latex);
        }
        for (&name, &ch) in NAMES.entries() {
            assert!(atom_of(ch).is_some(), "\\{} targets a non-atom", name);
            assert_eq!(symbol_by_name(name), Some(ch), "\\{}", name);
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
    /// The curated table pins the names worth guaranteeing against the
    /// generated one. Every disagreement must be a listed override —
    /// otherwise a regeneration could change what `\to` means and
    /// nothing would notice.
    #[test]
    fn curated_names_pin_the_generated_table() {
        let spellings = NAMES.entries().map(|(&n, &c)| (n, c));
        let mut diverged: Vec<&str> = spellings
            .filter(|(n, c)| ext::EXT_SYMBOLS.get(n).is_some_and(|e| e != c))
            .map(|(n, _)| n)
            .collect();
        diverged.sort_unstable();
        let mut want = INTENTIONAL_OVERRIDES.to_vec();
        want.sort_unstable();
        assert_eq!(diverged, want, "undocumented divergence from ext");
        // …and an override is a name the curated table actually wins.
        for &n in INTENTIONAL_OVERRIDES {
            let curated = ATOMS.entries().find(|(_, a)| a.latex == n).map(|(&c, _)| c);
            assert_eq!(symbol_by_name(n), curated, "\\{}", n);
        }
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
