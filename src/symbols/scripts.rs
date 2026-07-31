//! The inline super/subscript bijections (`x²`, `aᵢ`): the two forward
//! tables and one merged reverse table, phf like every other symbol
//! table. A test pins them to each other, so a pair cannot be added to
//! one side only (phf itself rejects duplicate keys).

/// base -> superscript.
pub static SUPERSCRIPTS: phf::Map<char, char> = phf::phf_map! {
    '0' => '⁰', '1' => '¹', '2' => '²', '3' => '³', '4' => '⁴',
    '5' => '⁵', '6' => '⁶', '7' => '⁷', '8' => '⁸', '9' => '⁹',
    '+' => '⁺', '-' => '⁻', '=' => '⁼', '(' => '⁽', ')' => '⁾',
    'n' => 'ⁿ', 'i' => 'ⁱ',
};

/// base -> subscript.
pub static SUBSCRIPTS: phf::Map<char, char> = phf::phf_map! {
    '0' => '₀', '1' => '₁', '2' => '₂', '3' => '₃', '4' => '₄',
    '5' => '₅', '6' => '₆', '7' => '₇', '8' => '₈', '9' => '₉',
    '+' => '₊', '-' => '₋', '=' => '₌', '(' => '₍', ')' => '₎',
    'a' => 'ₐ', 'e' => 'ₑ', 'h' => 'ₕ', 'i' => 'ᵢ', 'j' => 'ⱼ',
    'k' => 'ₖ', 'l' => 'ₗ', 'm' => 'ₘ', 'n' => 'ₙ', 'o' => 'ₒ',
    'p' => 'ₚ', 'r' => 'ᵣ', 's' => 'ₛ', 't' => 'ₜ', 'u' => 'ᵤ',
    'v' => 'ᵥ', 'x' => 'ₓ',
};

/// superscript | subscript -> base (the merged reverse table; which
/// script a char is comes from the forward tables).
pub static UNSCRIPTS: phf::Map<char, char> = phf::phf_map! {
    '⁰' => '0', '¹' => '1', '²' => '2', '³' => '3', '⁴' => '4',
    '⁵' => '5', '⁶' => '6', '⁷' => '7', '⁸' => '8', '⁹' => '9',
    '⁺' => '+', '⁻' => '-', '⁼' => '=', '⁽' => '(', '⁾' => ')',
    'ⁿ' => 'n', 'ⁱ' => 'i',
    '₀' => '0', '₁' => '1', '₂' => '2', '₃' => '3', '₄' => '4',
    '₅' => '5', '₆' => '6', '₇' => '7', '₈' => '8', '₉' => '9',
    '₊' => '+', '₋' => '-', '₌' => '=', '₍' => '(', '₎' => ')',
    'ₐ' => 'a', 'ₑ' => 'e', 'ₕ' => 'h', 'ᵢ' => 'i', 'ⱼ' => 'j',
    'ₖ' => 'k', 'ₗ' => 'l', 'ₘ' => 'm', 'ₙ' => 'n', 'ₒ' => 'o',
    'ₚ' => 'p', 'ᵣ' => 'r', 'ₛ' => 's', 'ₜ' => 't', 'ᵤ' => 'u',
    'ᵥ' => 'v', 'ₓ' => 'x',
};

pub fn superscript_char(c: char) -> Option<char> {
    SUPERSCRIPTS.get(&c).copied()
}

pub fn subscript_char(c: char) -> Option<char> {
    SUBSCRIPTS.get(&c).copied()
}

pub fn unsuperscript_char(c: char) -> Option<char> {
    UNSCRIPTS
        .get(&c)
        .copied()
        .filter(|&b| SUPERSCRIPTS.get(&b) == Some(&c))
}

pub fn unsubscript_char(c: char) -> Option<char> {
    UNSCRIPTS
        .get(&c)
        .copied()
        .filter(|&b| SUBSCRIPTS.get(&b) == Some(&c))
}

#[cfg(test)]
mod tests {
    use super::*;
    /// The three tables are one bijection: every forward pair reverses,
    /// every reverse pair is claimed by exactly one forward table.
    #[test]
    fn script_tables_are_bijections() {
        for (fwd, other) in [(&SUPERSCRIPTS, &SUBSCRIPTS), (&SUBSCRIPTS, &SUPERSCRIPTS)] {
            for (&base, &script) in fwd.entries() {
                assert_eq!(UNSCRIPTS.get(&script), Some(&base), "{script} reverses");
                // The two scripts never share a glyph (the reverse
                // table could not tell them apart).
                assert!(
                    !other.values().any(|&v| v == script),
                    "{script} is in both scripts"
                );
            }
        }
        for (&script, &base) in UNSCRIPTS.entries() {
            assert!(
                SUPERSCRIPTS.get(&base) == Some(&script) || SUBSCRIPTS.get(&base) == Some(&script),
                "{script} -> {base} has no forward pair"
            );
        }
    }
}
