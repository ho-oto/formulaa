//! The inline super/subscript bijections (`x²`, `aᵢ`): one aligned
//! table per script, read in both directions, so a pair cannot be
//! added to one side only.

/// The inline super/subscript characters, as the bijections they are:
/// one table per script, read in both directions. Written as two
/// aligned strings so a new pair cannot be added to one side only.
const SUPERSCRIPTS: (&str, &str) = ("0123456789+-=()ni", "⁰¹²³⁴⁵⁶⁷⁸⁹⁺⁻⁼⁽⁾ⁿⁱ");
const SUBSCRIPTS: (&str, &str) = (
    "0123456789+-=()aehijklmnoprstuvx",
    "₀₁₂₃₄₅₆₇₈₉₊₋₌₍₎ₐₑₕᵢⱼₖₗₘₙₒₚᵣₛₜᵤᵥₓ",
);

fn map_script(table: (&str, &str), c: char, up: bool) -> Option<char> {
    let (from, to) = if up { table } else { (table.1, table.0) };
    from.chars()
        .position(|x| x == c)
        .and_then(|i| to.chars().nth(i))
}

pub fn superscript_char(c: char) -> Option<char> {
    map_script(SUPERSCRIPTS, c, true)
}

pub fn subscript_char(c: char) -> Option<char> {
    map_script(SUBSCRIPTS, c, true)
}

pub fn unsuperscript_char(c: char) -> Option<char> {
    map_script(SUPERSCRIPTS, c, false)
}

pub fn unsubscript_char(c: char) -> Option<char> {
    map_script(SUBSCRIPTS, c, false)
}
