//! The atom vocabulary: which `\name` makes which character, which
//! characters are structure-reserved, and the allow-list gate every
//! parsed or typed character passes through.

use super::{alphabets, ext};

/// One curated character per row: the char itself is the canonical
/// identity, `latex` is the one spelling the serializer writes (and,
/// inverted, the canonical input spelling), and `kind` is how the
/// editor materializes it — a plain atom, or a ∑-class operator that
/// comes in as a band. Extra input spellings (`\leq` for ≤) are not
/// listed here: every alternative spelling lives in `ALIASES`, the
/// same table that spells commands (`\sqrt3` = `\cbrt`).
///
/// Most spellings also appear in the generated `ext` table. That
/// overlap is deliberate, not redundancy: it **pins** the meaning of
/// the names worth guaranteeing, so `ext` can be regenerated from
/// upstream without `\to` quietly changing what it produces. Four
/// entries disagree with upstream on purpose (`INTENTIONAL_OVERRIDES`),
/// and a test fails if a new divergence appears silently.
pub struct AtomSpec {
    pub ch: char,
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

const fn atom(ch: char, latex: &'static str, kind: AtomKind) -> AtomSpec {
    AtomSpec { ch, latex, kind }
}

pub const ATOMS: &[AtomSpec] = &[
    // Greek lowercase
    atom('α', "alpha", AtomKind::Sym),
    atom('β', "beta", AtomKind::Sym),
    atom('γ', "gamma", AtomKind::Sym),
    atom('δ', "delta", AtomKind::Sym),
    atom('ε', "epsilon", AtomKind::Sym),
    atom('ζ', "zeta", AtomKind::Sym),
    atom('η', "eta", AtomKind::Sym),
    atom('θ', "theta", AtomKind::Sym),
    atom('ι', "iota", AtomKind::Sym),
    atom('κ', "kappa", AtomKind::Sym),
    atom('λ', "lambda", AtomKind::Sym),
    atom('μ', "mu", AtomKind::Sym),
    atom('ν', "nu", AtomKind::Sym),
    atom('ξ', "xi", AtomKind::Sym),
    atom('π', "pi", AtomKind::Sym),
    atom('ρ', "rho", AtomKind::Sym),
    atom('σ', "sigma", AtomKind::Sym),
    atom('τ', "tau", AtomKind::Sym),
    atom('υ', "upsilon", AtomKind::Sym),
    atom('φ', "phi", AtomKind::Sym),
    atom('χ', "chi", AtomKind::Sym),
    atom('ψ', "psi", AtomKind::Sym),
    atom('ω', "omega", AtomKind::Sym),
    atom('ϕ', "varphi", AtomKind::Sym),
    atom('ϑ', "vartheta", AtomKind::Sym),
    // Greek uppercase
    atom('Γ', "Gamma", AtomKind::Sym),
    atom('Δ', "Delta", AtomKind::Sym),
    atom('Θ', "Theta", AtomKind::Sym),
    atom('Λ', "Lambda", AtomKind::Sym),
    atom('Ξ', "Xi", AtomKind::Sym),
    atom('Π', "Pi", AtomKind::Sym),
    atom('Σ', "Sigma", AtomKind::Sym),
    atom('Υ', "Upsilon", AtomKind::Sym),
    atom('Φ', "Phi", AtomKind::Sym),
    atom('Ψ', "Psi", AtomKind::Sym),
    atom('Ω', "Omega", AtomKind::Sym),
    // Binary operators / relations
    atom('±', "pm", AtomKind::Sym),
    atom('∓', "mp", AtomKind::Sym),
    atom('×', "times", AtomKind::Sym),
    atom('÷', "div", AtomKind::Sym),
    atom('⋅', "cdot", AtomKind::Sym),
    atom('∘', "circ", AtomKind::Sym),
    atom('⊕', "oplus", AtomKind::Sym),
    atom('⊗', "otimes", AtomKind::Sym),
    atom('∗', "ast", AtomKind::Sym),
    atom('≤', "le", AtomKind::Sym),
    atom('≥', "ge", AtomKind::Sym),
    atom('≠', "ne", AtomKind::Sym),
    atom('≈', "approx", AtomKind::Sym),
    atom('≡', "equiv", AtomKind::Sym),
    atom('∼', "sim", AtomKind::Sym),
    atom('≃', "simeq", AtomKind::Sym),
    atom('∝', "propto", AtomKind::Sym),
    atom('≪', "ll", AtomKind::Sym),
    atom('≫', "gg", AtomKind::Sym),
    // Arrows
    atom('→', "to", AtomKind::Sym),
    atom('←', "leftarrow", AtomKind::Sym),
    atom('⇒', "Rightarrow", AtomKind::Sym),
    atom('⇐', "Leftarrow", AtomKind::Sym),
    atom('↔', "leftrightarrow", AtomKind::Sym),
    atom('⇔', "Leftrightarrow", AtomKind::Sym),
    atom('↦', "mapsto", AtomKind::Sym),
    // Sets / logic
    atom('∈', "in", AtomKind::Sym),
    atom('∉', "notin", AtomKind::Sym),
    atom('∋', "ni", AtomKind::Sym),
    atom('⊂', "subset", AtomKind::Sym),
    atom('⊆', "subseteq", AtomKind::Sym),
    atom('⊃', "supset", AtomKind::Sym),
    atom('⊇', "supseteq", AtomKind::Sym),
    atom('∪', "cup", AtomKind::Sym),
    atom('∩', "cap", AtomKind::Sym),
    atom('∖', "setminus", AtomKind::Sym),
    atom('∅', "emptyset", AtomKind::Sym),
    atom('∀', "forall", AtomKind::Sym),
    atom('∃', "exists", AtomKind::Sym),
    atom('¬', "neg", AtomKind::Sym),
    atom('∧', "land", AtomKind::Sym),
    atom('∨', "lor", AtomKind::Sym),
    atom('⊢', "vdash", AtomKind::Sym),
    atom('⊨', "models", AtomKind::Sym),
    // Misc
    atom('∞', "infty", AtomKind::Sym),
    atom('∂', "partial", AtomKind::Sym),
    atom('∇', "nabla", AtomKind::Sym),
    atom('ℏ', "hbar", AtomKind::Sym),
    atom('ℓ', "ell", AtomKind::Sym),
    atom('ℜ', "Re", AtomKind::Sym),
    atom('ℑ', "Im", AtomKind::Sym),
    atom('ℵ', "aleph", AtomKind::Sym),
    atom('∠', "angle", AtomKind::Sym),
    atom('⊥', "perp", AtomKind::Sym),
    atom('∥', "parallel", AtomKind::Sym),
    atom('′', "prime", AtomKind::Sym),
    atom('°', "degree", AtomKind::Sym),
    atom('⋯', "cdots", AtomKind::Sym),
    atom('…', "ldots", AtomKind::Sym),
    atom('⋮', "vdots", AtomKind::Sym),
    atom('⋱', "ddots", AtomKind::Sym),
    // Explicit space atom (the Space key): visible ␣ in canonical AA so the
    // picture stays parseable (a real blank column is a sibling separator).
    atom('␣', "space", AtomKind::Sym),
    // ∑-class big operators (band-promotable; see AtomKind::BigOp).
    atom('∑', "sum", AtomKind::BigOp),
    atom('∏', "prod", AtomKind::BigOp),
    atom('∐', "coprod", AtomKind::BigOp),
    atom('∫', "int", AtomKind::BigOp),
    atom('∬', "iint", AtomKind::BigOp),
    atom('∮', "oint", AtomKind::BigOp),
    atom('⋃', "bigcup", AtomKind::BigOp),
    atom('⋂', "bigcap", AtomKind::BigOp),
    atom('⨁', "bigoplus", AtomKind::BigOp),
    atom('⨂', "bigotimes", AtomKind::BigOp),
    atom('⋁', "bigvee", AtomKind::BigOp),
    atom('⋀', "bigwedge", AtomKind::BigOp),
];

/// The curated row for a character.
pub fn atom_of(c: char) -> Option<&'static AtomSpec> {
    ATOMS.iter().find(|a| a.ch == c)
}

/// Command spellings that mean another command. Keeping them in one
/// table lets the dispatch name each command exactly once, and makes
/// "what else is this called" answerable in one place.
pub const ALIASES: &[(&str, &str)] = &[
    // LaTeX names for what the editor calls something shorter.
    ("sqrt3", "cbrt"),
    ("sqrt4", "qdrt"),
    ("Vert", "norm"),
    ("xrightarrow", "xto"),
    ("xleftarrow", "xfrom"),
    ("xRightarrow", "xTo"),
    ("xLeftarrow", "xFrom"),
    ("operatorname", "op"),
    ("operatorname*", "op*"),
    ("limits", "op*"),
    ("delim", "lr"),
    // Symbol spellings: aliases for a character's canonical (LaTeX)
    // name, resolved by the same one-hop rule as the command aliases.
    ("leq", "le"),
    ("geq", "ge"),
    ("neq", "ne"),
    ("rightarrow", "to"),
    ("dots", "ldots"),
];

/// One-hop alias resolution: the spelling's canonical form, or itself.
pub fn unalias(name: &str) -> &str {
    ALIASES
        .iter()
        .find_map(|&(from, to)| (from == name).then_some(to))
        .unwrap_or(name)
}

/// Names where the curated table deliberately differs from the
/// generated one: the TeX convention is not what upstream picked, and
/// the curated spelling wins.
pub const INTENTIONAL_OVERRIDES: &[&str] = &["epsilon", "phi", "varphi", "hbar"];

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
                .iter()
                .map(|a| a.ch)
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
    /// One row per character: the char is unique, every spelling is
    /// claimed once (canonical names and aliases together), and the
    /// output is the row's `latex` — \le and \leq type the same ≤ but
    /// it always prints as \le.
    #[test]
    fn atom_rows_are_canonical() {
        let mut chars = std::collections::HashSet::new();
        let mut names = std::collections::HashSet::new();
        for a in ATOMS {
            assert!(chars.insert(a.ch), "{:?} has two rows", a.ch);
            assert!(names.insert(a.latex), "\\{} is claimed twice", a.latex);
            assert_eq!(latex_name(a.ch), Some(a.latex));
            assert_eq!(symbol_by_name(a.latex), Some(a.ch), "\\{}", a.latex);
        }
        for &(from, to) in ALIASES {
            assert!(names.insert(from), "\\{} is claimed twice", from);
            // An alias points at a canonical spelling, never another
            // alias (resolution is one hop).
            assert_eq!(unalias(to), to, "\\{} chains", from);
            assert_eq!(symbol_by_name(from), symbol_by_name(to), "\\{}", from);
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
        let spellings = ATOMS.iter().map(|a| (a.latex, a.ch)).chain(
            ALIASES
                .iter()
                .filter_map(|&(from, to)| Some((from, atom_of(symbol_by_name(to)?)?.ch))),
        );
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
            let curated = ATOMS.iter().find(|a| a.latex == n).map(|a| a.ch);
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
