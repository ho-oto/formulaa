//! The atom vocabulary: which `\name` makes which character, which
//! characters are structure-reserved, and the allow-list gate every
//! parsed or typed character passes through.

use super::alphabets;

/// What a curated character *is*: `latex` is the one spelling the
/// serializer writes, and `kind` is how the editor materializes it —
/// a plain atom, or a ∑-class operator that comes in as a band. This
/// is the *output* table (char -> info, the char being the key); the
/// input direction (spelling -> char, including the extra spellings
/// like `\leq` for ≤) is `NAMES`, and the tests hold the two mirrors
/// together.
///
/// Every non-ASCII char the format accepts appears here or in the
/// styled families — there is no generated long tail anymore, so
/// "typeable" implies "spells its LaTeX" by construction (the mod
/// test measures the gap at zero).
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
    '⋤' => sym("sqsubsetneq"),
    '⋥' => sym("sqsupsetneq"),
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
    '∯' => big("oiint"),
    '∰' => big("oiiint"),
    '⨀' => big("bigodot"),
    '⨅' => big("bigsqcap"),
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
/// character, alternative spellings or-ed onto the canonical one
/// (which comes first) in the same entry — the LaTeX names, the ASCII
/// pictures (`\->`, `\:=`, `\|x|`) and the abbreviations alike, all
/// absorbed from the once-generated ext table. Deliberately a
/// hand-written mirror of `ATOMS` rather than derived from it, so each
/// direction reads on its own; a spelling claimed twice fails the
/// build (phf), and the tests pin the two tables together (every
/// canonical spelling present, every target spells its LaTeX).
/// Command aliases (`\sqrt3` = `\cbrt`) are not names of a character,
/// so they live as extra patterns in `resolve`'s match.
pub static NAMES: phf::Map<&'static str, char> = phf::phf_map! {
    // Greek lowercase
    "alpha" | "al" | "alp" => 'α',
    "beta" | "be" | "bet" => 'β',
    "gamma" | "ga" | "gam" | "gm" | "gmm" => 'γ',
    "delta" | "de" | "del" | "dl" | "dlt" => 'δ',
    "epsilon" | "ep" | "eps" => 'ϵ',
    "varepsilon" | "varep" | "vareps" | "vep" | "veps" | "vepsilon" => 'ε',
    "zeta" | "ze" | "zet" => 'ζ',
    "eta" | "et" => 'η',
    "theta" | "th" | "the" => 'θ',
    "iota" | "io" | "iot" => 'ι',
    "kappa" | "ka" | "kap" | "kp" | "kpp" => 'κ',
    "lambda" | "la" | "lam" | "lm" => 'λ',
    "mu" => 'μ',
    "nu" => 'ν',
    "xi" => 'ξ',
    "pi" => 'π',
    "rho" | "rh" => 'ρ',
    "sigma" | "sg" | "sgm" | "si" | "sig" => 'σ',
    "tau" | "ta" => 'τ',
    "upsilon" | "ups" => 'υ',
    "phi" | "ph" => 'ϕ',
    "chi" | "ch" => 'χ',
    "psi" | "ps" => 'ψ',
    "omega" | "om" | "ome" | "omg" => 'ω',
    "varphi" | "varph" | "vph" | "vphi" => 'φ',
    "vartheta" | "varth" | "varthe" | "vth" | "vthe" | "vtheta" => 'ϑ',
    "varkappa" | "varka" | "varkap" | "varkp" | "varkpp" | "vka" | "vkap" | "vkappa" | "vkp" | "vkpp" => 'ϰ',
    // Greek uppercase
    "Gamma" | "Ga" | "Gam" | "Gm" | "Gmm" => 'Γ',
    "Delta" | "De" | "Del" | "Dl" | "Dlt" => 'Δ',
    "Theta" | "Th" | "The" => 'Θ',
    "Lambda" | "La" | "Lam" | "Lm" => 'Λ',
    "Xi" => 'Ξ',
    "Pi" => 'Π',
    "Sigma" | "Sg" | "Sgm" | "Si" | "Sig" => 'Σ',
    "Upsilon" | "Ups" => 'Υ',
    "Phi" | "Ph" => 'Φ',
    "Psi" | "Ps" => 'Ψ',
    "Omega" | "Om" | "Ome" | "Omg" => 'Ω',
    // Binary operators / relations
    "pm" | "+-" => '±',
    "mp" | "-+" => '∓',
    "times" | "x" => '×',
    "div" => '÷',
    "cdot" => '⋅',
    "circ" => '∘',
    "oplus" => '⊕',
    "otimes" => '⊗',
    "ast" => '∗',
    "le" | "leq" => '≤',
    "ge" | "geq" => '≥',
    "ne" | "neq" | "/=" | "=/" | "=/=" => '≠',
    "approx" | "~~" => '≈',
    "equiv" | "-=" | "=-" => '≡',
    "sim" => '∼',
    "simeq" | "-~" | "~-" => '≃',
    "propto" | "oc" | "prop" => '∝',
    "ll" | "<<" => '≪',
    "gg" | ">>" => '≫',
    // Arrows
    "to" | "rightarrow" | "right" | "->" => '→',
    "leftarrow" | "left" | "<-" => '←',
    "Rightarrow" | "Right" | "=>" => '⇒',
    "Leftarrow" | "Left" | "<=" => '⇐',
    "leftrightarrow" | "leftright" | "lr" | "<->" => '↔',
    "Leftrightarrow" | "LR" | "Leftrigh" | "Lr" | "<=>" => '⇔',
    "mapsto" | "|->" => '↦',
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
    "setminus" | "setm" | "sm" => '∖',
    "emptyset" | "empty" => '∅',
    "forall" | "A" | "all" => '∀',
    "exists" | "E" | "exist" => '∃',
    "neg" | "not" => '¬',
    "land" | "and" | "wedge" => '∧',
    "lor" | "or" | "vee" => '∨',
    "vdash" => '⊢',
    "models" | "vDash" => '⊨',
    // Misc
    "infty" | "oo" => '∞',
    "partial" | "p" | "par" => '∂',
    "nabla" => '∇',
    "hbar" | "hslash" => 'ℏ',
    "ell" => 'ℓ',
    "Re" => 'ℜ',
    "Im" => 'ℑ',
    "aleph" => 'ℵ',
    "angle" => '∠',
    "perp" | "bot" => '⊥',
    "parallel" => '∥',
    "prime" => '′',
    "degree" => '°',
    "cdots" | "---" => '⋯',
    "ldots" | "dots" | "..." => '…',
    "vdots" => '⋮',
    "ddots" => '⋱',
    "space" => '␣',
    // Arrows (amssymb tier)
    "uparrow" | "up" => '↑',
    "downarrow" | "dn" | "down" => '↓',
    "updownarrow" | "ud" | "updn" | "updown" => '↕',
    "nwarrow" => '↖',
    "nearrow" => '↗',
    "searrow" => '↘',
    "swarrow" => '↙',
    "twoheadleftarrow" | "twoheadleft" | "<<-" => '↞',
    "twoheadrightarrow" | "twoheadright" | "->>" => '↠',
    "leftarrowtail" | "lefttail" | "<-<" => '↢',
    "rightarrowtail" | "righttail" | ">->" => '↣',
    "hookleftarrow" | "hookleft" | "<-C" | "<-c" => '↩',
    "hookrightarrow" | "hookright" | "C->" | "c->" => '↪',
    "rightleftarrows" => '⇄',
    "leftrightarrows" => '⇆',
    "leftleftarrows" | "leftleft" | "<-<-" => '⇇',
    "upuparrows" | "upup" => '⇈',
    "rightrightarrows" | "rightright" | "->->" => '⇉',
    "downdownarrows" | "dndn" | "downdown" => '⇊',
    "Uparrow" | "Up" => '⇑',
    "Downarrow" | "Dn" | "Down" => '⇓',
    "Updownarrow" | "UD" | "Ud" | "Updn" | "Updown" => '⇕',
    "rightsquigarrow" | "leadsto" | "~>" => '⇝',
    "longleftarrow" | "longleft" | "<--" => '⟵',
    "longrightarrow" | "longright" | "-->" => '⟶',
    "longleftrightarrow" | "longleftright" | "longlr" | "<-->" => '⟷',
    "Longleftarrow" | "Longleft" | "<==" => '⟸',
    "Longrightarrow" | "Longright" | "==>" => '⟹',
    "Longleftrightarrow" | "Longleftright" | "Longlr" | "iff" | "<==>" => '⟺',
    "longmapsto" | "|-->" => '⟼',
    // Relations and orders (amssymb tier)
    "cong" | "simeqq" | "=~" | "~=" => '≅',
    "approxeq" | "-~~" | "~~-" => '≊',
    "coloneqq" | "ceq" | "coloneq" | ":=" => '≔',
    "eqqcolon" | "eqc" | "eqcolon" | "=:" => '≕',
    "leqq" => '≦',
    "geqq" => '≧',
    "lneqq" => '≨',
    "gneqq" => '≩',
    "lesssim" | "lsim" => '≲',
    "gtrsim" | "gsim" => '≳',
    "prec" => '≺',
    "succ" => '≻',
    "preccurlyeq" | "preceq" => '≼',
    "succcurlyeq" | "succeq" => '≽',
    "subsetneq" => '⊊',
    "supsetneq" => '⊋',
    "sqsubset" => '⊏',
    "sqsupset" => '⊐',
    "sqsubseteq" => '⊑',
    "sqsupseteq" => '⊒',
    "dashv" => '⊣',
    "top" => '⊤',
    "Vdash" => '⊩',
    "vartriangleleft" | "<|" => '⊲',
    "vartriangleright" | "|>" => '⊳',
    "trianglelefteq" | "-<|" | "<|-" => '⊴',
    "trianglerighteq" | "-|>" | "|>-" => '⊵',
    "sqsubsetneq" => '⋤',
    "sqsupsetneq" => '⋥',
    "lnsim" => '⋦',
    "gnsim" => '⋧',
    "lll" | "<<<" => '⋘',
    "ggg" | ">>>" => '⋙',
    "lessapprox" | "lapprox" => '⪅',
    "gtrapprox" | "gapprox" => '⪆',
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
    "dagger" | "+" => '†',
    "ddagger" | "++" => '‡',
    "complement" => '∁',
    "dotplus" => '∔',
    "bullet" => '∙',
    "sqcap" => '⊓',
    "sqcup" => '⊔',
    "ominus" => '⊖',
    "odot" => '⊙',
    "veebar" | "xor" => '⊻',
    "barwedge" | "nand" => '⊼',
    "star" => '⋆',
    "bowtie" | "|x|" => '⋈',
    "ltimes" | "|x" => '⋉',
    "rtimes" | "x|" => '⋊',
    // ∑-class big operators (amsmath tier)
    "iiint" | "III" => '∭',
    "oiint" | "oII" => '∯',
    "oiiint" | "oIII" => '∰',
    "bigodot" => '⨀',
    "bigsqcap" => '⨅',
    "bigsqcup" => '⨆',
    // ∑-class big operators
    "sum" | "S" => '∑',
    "prod" | "P" => '∏',
    "coprod" => '∐',
    "int" | "I" => '∫',
    "iint" | "II" => '∬',
    "oint" | "oI" => '∮',
    "bigcup" => '⋃',
    "bigcap" => '⋂',
    "bigoplus" => '⨁',
    "bigotimes" => '⨂',
    "bigvee" => '⋁',
    "bigwedge" => '⋀',
    // Blackboard shortcuts (\RR = \bbR; the LaTeX spelling comes
    // from the styled families, not a curated row)
    "CC" => 'ℂ',
    "HH" => 'ℍ',
    "NN" => 'ℕ',
    "PP" => 'ℙ',
    "QQ" => 'ℚ',
    "RR" => 'ℝ',
    "ZZ" => 'ℤ',
};

/// The char a curated `\name` types (any spelling in its `NAMES` entry).
pub fn named_char(name: &str) -> Option<char> {
    NAMES.get(name).copied()
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
        // ASCII the keyboard types directly, minus the glyphs the
        // format reserves for drawing/quoting (( ) [ ] { } _ " ') and
        // the four LaTeX would misread as syntax with no sensible atom
        // meaning (`^` `~` `` ` `` `\` — the symbols are \sim,
        // \backslash …). `# $ % &` stay: they are ordinary characters
        // that the serializer escapes. The test pins this set against
        // the full reserved-glyph table.
        return c.is_ascii_graphic()
            && !matches!(
                c,
                '(' | ')' | '[' | ']' | '{' | '}' | '_' | '"' | '\'' | '^' | '~' | '`' | '\\'
            );
    }
    ATOM_SET
        .get_or_init(|| {
            NAMES
                .values()
                .copied()
                .chain(alphabets::ALPHABETS.keys().flat_map(|style| {
                    ('A'..='Z')
                        .chain('a'..='z')
                        .chain('0'..='9')
                        .filter_map(move |l| alphabets::alphabet_char(&format!("{}{}", style, l)))
                }))
                .collect()
        })
        .contains(&c)
}

#[cfg(test)]
mod tests {
    use crate::symbols::*;
    /// Structure-reserved glyphs: never valid as atoms (see
    /// docs/aa-spec.md §2). Accent marks, the √ overline `_`, and the
    /// inline super/subscript codepoints are reserved too — they read
    /// back as structure, not atoms. The tables are static, so this is
    /// a test predicate rather than a runtime filter: the test below
    /// proves no table can hand one out, and `is_atom` needs no check.
    #[rustfmt::skip]
    fn is_reserved_glyph(c: char) -> bool {
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
            // ⎸ ⎹ U+23B8/23B9 are not part of the format)
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
            // Drawn accent glyphs — the only accent chars a picture
            // can contain (accents are the Accent enum in the AST, so
            // nothing else needs reserving)
            | '¯'                   // U+00AF MACRON: drawn under bar
            | '˜'                   // U+02DC SMALL TILDE: drawn under tilde
            | '˷'                   // U+02F7 LOW TILDE: drawn over tilde
            | '˰'                   // U+02F0 MODIFIER LETTER LOW UP ARROWHEAD: hat
            | '˯'                   // U+02EF MODIFIER LETTER LOW DOWN ARROWHEAD: check
            | '˳'                   // U+02F3 MODIFIER LETTER LOW RING: ring
            | '․'                   // U+2024 ONE DOT LEADER: dot / ddot (․․)
            | '￫' // U+FFEB HALFWIDTH RIGHTWARDS ARROW: vec
        ) || crate::symbols::scripts::unsuperscript_char(c).is_some()
            || crate::symbols::scripts::unsubscript_char(c).is_some()
    }

    /// Every character the tables hand out is an honest atom: no
    /// symbol name, styled family or ASCII key can produce a reserved
    /// glyph, and the `is_atom` ASCII gate matches the reserved table
    /// exactly.
    #[test]
    fn reserved_glyphs_stay_out_of_the_tables() {
        for (&name, &ch) in NAMES.entries() {
            assert!(
                !is_reserved_glyph(ch),
                "\\{} hands out a reserved glyph",
                name
            );
        }
        for style in alphabets::ALPHABETS.keys() {
            for l in ('A'..='Z').chain('a'..='z').chain('0'..='9') {
                if let Some(c) = alphabets::alphabet_char(&format!("{}{}", style, l)) {
                    assert!(!is_reserved_glyph(c), "{:?}", c);
                }
            }
        }
        // The ASCII gate is the reserved table, spelled inline.
        for c in (0u8..=127).map(char::from) {
            let expect = c.is_ascii_graphic()
                && !is_reserved_glyph(c)
                && !matches!(c, '^' | '~' | '`' | '\\');
            assert_eq!(is_atom(c), expect, "{:?}", c);
        }
        // …and no reserved glyph anywhere is an atom.
        for c in (1..=0x2FFFFu32).filter_map(char::from_u32) {
            if is_reserved_glyph(c) {
                assert!(!is_atom(c), "{:?}", c);
            }
        }
    }

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
            // Every target spells its LaTeX — a curated row, or a
            // styled letter for the \RR-style blackboard shortcuts.
            assert!(
                crate::symbols::latex_of(ch).is_some(),
                "\\{} targets a char with no LaTeX spelling",
                name
            );
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
    /// The style token never swallows an ordinary name that starts with it.
    #[test]
    fn script_letters_resolve_without_eating_names() {
        // The sup/sub modifier letters stay out (a superscript is
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
