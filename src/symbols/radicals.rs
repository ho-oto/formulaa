//! The root signs, as an enum: the AST stores a `Radical`, the input
//! direction is a phf map (spelling -> variant, aliases or-ed on) and
//! everything about one — its glyph and LaTeX index — is answered by
//! `info` in one row, same two-layer shape as the arrows and accents.
//! Only the three radicals Unicode has a glyph for exist; a general
//! `\sqrt[n]` has no one-cell spelling in the picture.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Radical {
    /// `√` — the square root, drawn without an index.
    Sqrt,
    /// `∛`
    Cbrt,
    /// `∜`
    Qdrt,
}

/// Everything about one root sign, in one row.
pub struct RadicalInfo {
    /// The editor's canonical `\name`.
    pub name: &'static str,
    /// The root glyph, which is also how the picture spells the index.
    pub glyph: char,
    /// The LaTeX index (`\sqrt[3]`); the square root has none.
    pub latex_index: Option<u8>,
}

/// The input direction: spelling -> variant, canonical first, the
/// LaTeX-flavored spelling or-ed on as an alias.
pub static RADICAL_NAMES: phf::Map<&'static str, Radical> = phf::phf_map! {
    "sqrt" => Radical::Sqrt,
    "cbrt" | "sqrt3" => Radical::Cbrt,
    "qdrt" | "sqrt4" => Radical::Qdrt,
};

/// The radical stem column: │ runs from the overline down to the root
/// glyph, which is its bottom cell.
pub const STEM: char = '│'; // U+2502 (shared glyph with the \lr mid separator)

/// The overline's corner, sitting directly above the stem (the lattice
/// ┌ has a blank gap below instead — that adjacency is the
/// disambiguator).
pub const OVERLINE_CORNER: char = '┌'; // U+250C

/// A glyph the stem column may show: the stem or a root sign.
pub fn is_stem_glyph(c: char) -> bool {
    c == STEM || Radical::of_glyph(c).is_some()
}

impl Radical {
    pub const ALL: [Radical; 3] = [Radical::Sqrt, Radical::Cbrt, Radical::Qdrt];

    /// The one row that says everything about this root sign.
    pub const fn info(self) -> &'static RadicalInfo {
        match self {
            Radical::Sqrt => &RadicalInfo {
                name: "sqrt",
                glyph: '√',
                latex_index: None,
            },
            Radical::Cbrt => &RadicalInfo {
                name: "cbrt",
                glyph: '∛',
                latex_index: Some(3),
            },
            Radical::Qdrt => &RadicalInfo {
                name: "qdrt",
                glyph: '∜',
                latex_index: Some(4),
            },
        }
    }

    /// The radical a `\name` builds (any spelling in its
    /// `RADICAL_NAMES` entry).
    pub fn of_name(name: &str) -> Option<Radical> {
        RADICAL_NAMES.get(name).copied()
    }

    pub fn glyph(self) -> char {
        self.info().glyph
    }

    pub fn of_glyph(c: char) -> Option<Radical> {
        Radical::ALL.into_iter().find(|r| r.glyph() == c)
    }

    pub fn latex_index(self) -> Option<u8> {
        self.info().latex_index
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn radicals_are_a_bijection() {
        for r in Radical::ALL {
            assert_eq!(Radical::of_glyph(r.glyph()), Some(r));
            assert_eq!(Radical::of_name(r.info().name), Some(r));
        }
        // Both spellings of an indexed root build it.
        assert_eq!(Radical::of_name("sqrt3"), Some(Radical::Cbrt));
        assert_eq!(Radical::of_name("sqrt4"), Some(Radical::Qdrt));
    }
}
