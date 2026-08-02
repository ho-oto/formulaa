//! Every table the format rests on, one concern per file:
//!
//! - [`atoms`] — the whole symbol vocabulary, both directions:
//!   `NAMES` (spelling -> char, aliases or-ed on), `ATOMS`
//!   (char -> LaTeX + kind), the reserved glyphs and the `is_atom`
//!   allow-list gate
//! - [`alphabets`] — the styled families as rules (`\bbR`, `\Afrk`),
//!   including the char -> `\mathbb{R}` reverse spelling
//! - [`funcs`] — upright function names, limit-takers and ∑-class ops
//! - [`accents`] — accent marks and their LaTeX commands
//! - [`delims`] — the delimiter pairs (glyph columns, LaTeX)
//! - [`arrows`] — the stretchy labeled arrows
//! - [`radicals`] — the root signs
//! - [`scripts`] — the inline super/subscript bijections
//!
//! Everything is re-exported flat (`symbols::is_atom`), so callers
//! never need to know the file layout.

pub mod accents;
pub mod alphabets;
pub mod arrows;
pub mod atoms;
pub mod delims;
pub mod funcs;
pub mod radicals;
pub mod scripts;

pub use accents::*;
pub use alphabets::*;
pub use arrows::*;
pub use atoms::*;
pub use delims::*;
pub use funcs::*;
pub use radicals::*;
pub use scripts::*;

/// Resolve an input spelling to its character: the input table
/// (`NAMES`, canonical spellings and their aliases alike), then the
/// styled families (\bbR). No reserved-glyph filtering is needed —
/// the tables are static and the atoms test proves they never hand
/// out a structural glyph.
pub fn symbol_by_name(name: &str) -> Option<char> {
    named_char(name).or_else(|| alphabets::alphabet_char(name))
}

/// Is this char a ∑-class operator (band-promotable)?
pub fn is_bigop(c: char) -> bool {
    atom_of(c).is_some_and(|a| a.kind == AtomKind::BigOp)
}

/// The LaTeX spelling of a character — the *output* direction, kept
/// separate from input resolution: a char answers to many input
/// aliases but is written exactly one way.
pub fn latex_name(c: char) -> Option<&'static str> {
    atom_of(c).map(|a| a.latex)
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
    /// Every styled letter has a LaTeX spelling through its family, so
    /// the char -> LaTeX direction is total over the ~700 characters
    /// they generate. What is left over is measured, not assumed: a
    /// regression that widens the gap fails here.
    #[test]
    fn latex_spelling_covers_the_styled_families() {
        for (style, fam) in alphabets::ALPHABETS.entries() {
            for l in ('A'..='Z').chain('a'..='z') {
                let c = alphabets::alphabet_char(&format!("{}{}", style, l)).unwrap();
                let want = format!("\\{}{{{}}}", fam.latex, l);
                // A letterlike symbol the curated table names (ℑ = \Im)
                // keeps that name — it wins on purpose.
                let got = latex_of(c).unwrap_or_default();
                assert!(
                    got == want || latex_name(c).is_some(),
                    "{}{}: {:?}",
                    style,
                    l,
                    got
                );
            }
        }
        let gap: Vec<char> = (1..=0x2FFFFu32)
            .filter_map(char::from_u32)
            .filter(|&c| is_atom(c) && !c.is_ascii() && latex_of(c).is_none())
            .collect();
        // Typeable implies spellable: no atom is emitted as raw
        // Unicode, so the output never assumes unicode-math.
        assert!(gap.is_empty(), "atoms with no LaTeX spelling: {:?}", gap);
    }
}
