//! Accent marks, as an enum: the AST stores `Accent`, and everything
//! about a mark — its command name, LaTeX spelling, side, and the
//! glyph its picture shows — is answered here. The old mark *chars*
//! (`^` `⇀` `¨` …) are gone: they appeared in no picture and no
//! output, so they were an untyped spelling of exactly this enum.

/// An accent mark. Over-marks stack above the base, under-marks below;
/// the two sets are disjoint, which is what makes a mark-on-base
/// column unambiguous for the parser.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Accent {
    Hat,
    Tilde,
    Bar,
    Vec,
    Dot,
    Ddot,
    Check,
    Ring,
    Underline,
    Utilde,
}

/// How a mark draws, both in the compact column and in the wide band.
/// The same glyph serves both: a `Center` mark sits in one centered
/// cell, a `Fill` repeats across the band's width (compact width = 1,
/// so the two coincide), and the ddot pair `․․` overhangs one cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawnForm {
    Center(char),
    Fill(char),
    Dots,
}

/// Everything about one mark, in one row — `info` is the single match,
/// so a variant's whole story reads in one place.
pub struct AccentInfo {
    /// The `\name` that applies this mark.
    pub name: &'static str,
    /// The LaTeX command for the single-char form.
    pub latex: &'static str,
    /// The LaTeX command for the stretchy (wide) form; marks without
    /// one use their plain command (\dot etc. accept groups).
    pub wide_latex: &'static str,
    /// Under-marks hug the base from below; everything else is over.
    pub under: bool,
    /// The glyph the picture shows. Every mark hugs its base: over
    /// marks draw low in their cell (bar as `_` like the √ overline,
    /// tilde as the low `˷`), under marks draw high (underline as `¯`,
    /// utilde as the high `˜`) — so the tilde/bar pairs swap between
    /// the over and under roles, and every drawn glyph names exactly
    /// one (mark, side).
    pub drawn: DrawnForm,
}

/// One row, positionally: (name, latex, wide_latex, under, drawn).
/// A macro rather than a const fn so the `&` still promotes to 'static.
macro_rules! mark {
    ($name:literal, $latex:literal, $wide:literal, $under:literal, $drawn:expr) => {
        &AccentInfo {
            name: $name,
            latex: $latex,
            wide_latex: $wide,
            under: $under,
            drawn: $drawn,
        }
    };
}

/// The input direction, same shape as the symbol `NAMES` table:
/// spelling -> variant, canonical first, aliases or-ed on (the LaTeX
/// spelling where it differs from the editor's name).
pub static ACCENT_NAMES: phf::Map<&'static str, Accent> = phf::phf_map! {
    "hat" => Accent::Hat,
    "tilde" => Accent::Tilde,
    "bar" => Accent::Bar,
    "vec" => Accent::Vec,
    "dot" => Accent::Dot,
    "ddot" => Accent::Ddot,
    "check" => Accent::Check,
    "ring" | "mathring" => Accent::Ring,
    "underline" => Accent::Underline,
    "utilde" => Accent::Utilde,
};

impl Accent {
    pub const ALL: [Accent; 10] = [
        Accent::Hat,
        Accent::Tilde,
        Accent::Bar,
        Accent::Vec,
        Accent::Dot,
        Accent::Ddot,
        Accent::Check,
        Accent::Ring,
        Accent::Underline,
        Accent::Utilde,
    ];

    /// The one row that says everything about this mark.
    #[rustfmt::skip]
    pub const fn info(self) -> &'static AccentInfo {
        match self {
            // Drawn glyphs: ˰ U+02F0 LOW UP ARROWHEAD, ˯ U+02EF LOW
            // DOWN ARROWHEAD, ˳ U+02F3 LOW RING, ․ U+2024 LEADER (not
            // the '.' atom), ￫ halfwidth U+FFEB (not the → atom),
            // ˷ U+02F7 LOW TILDE, ˜ U+02DC SMALL TILDE.
            Accent::Hat       => mark!("hat", "hat", "widehat", false, DrawnForm::Center('˰')),
            Accent::Tilde     => mark!("tilde", "tilde", "widetilde", false, DrawnForm::Fill('˷')),
            Accent::Bar       => mark!("bar", "bar", "overline", false, DrawnForm::Fill('_')),
            Accent::Vec       => mark!("vec", "vec", "overrightarrow", false, DrawnForm::Center('￫')),
            Accent::Dot       => mark!("dot", "dot", "dot", false, DrawnForm::Center('․')),
            Accent::Ddot      => mark!("ddot", "ddot", "ddot", false, DrawnForm::Dots),
            Accent::Check     => mark!("check", "check", "widecheck", false, DrawnForm::Center('˯')),
            Accent::Ring      => mark!("ring", "mathring", "mathring", false, DrawnForm::Center('˳')),
            Accent::Underline => mark!("underline", "underline", "underline", true, DrawnForm::Fill('¯')),
            Accent::Utilde    => mark!("utilde", "utilde", "utilde", true, DrawnForm::Fill('˜')),
        }
    }

    pub fn name(self) -> &'static str {
        self.info().name
    }

    /// The mark a `\name` applies (any spelling in its `ACCENT_NAMES`
    /// entry).
    pub fn of_name(name: &str) -> Option<Accent> {
        ACCENT_NAMES.get(name).copied()
    }

    pub fn latex(self) -> &'static str {
        self.info().latex
    }

    pub fn wide_latex(self) -> &'static str {
        self.info().wide_latex
    }

    pub fn under(self) -> bool {
        self.info().under
    }

    pub fn drawn(self) -> DrawnForm {
        self.info().drawn
    }

    /// The single glyph shown in a compact one-cell column (the ddot
    /// is the one mark that is wider than its base — handled apart).
    pub fn glyph(self) -> char {
        match self.drawn() {
            DrawnForm::Center(g) | DrawnForm::Fill(g) => g,
            DrawnForm::Dots => '․',
        }
    }

    /// Read a compact over-column glyph back to its mark (`․` is a
    /// dot; the caller upgrades a `․․` pair to the ddot).
    pub fn of_over_glyph(c: char) -> Option<Accent> {
        Accent::ALL
            .into_iter()
            .find(|a| !a.under() && a.drawn() != DrawnForm::Dots && a.glyph() == c)
    }

    pub fn of_under_glyph(c: char) -> Option<Accent> {
        Accent::ALL
            .into_iter()
            .find(|a| a.under() && a.glyph() == c)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every drawn glyph names exactly one (mark, side) — the baseline
    /// dive relies on the over/under classification being positionally
    /// unambiguous — and the compact readers invert `drawn`.
    #[test]
    fn drawn_glyphs_are_unambiguous() {
        for a in Accent::ALL {
            assert_eq!(Accent::of_name(a.name()), Some(a));
            assert_eq!(Accent::of_name(a.latex()), Some(a));
            match (a.under(), a.drawn()) {
                (_, DrawnForm::Dots) => assert_eq!(a, Accent::Ddot),
                (false, _) => assert_eq!(Accent::of_over_glyph(a.glyph()), Some(a)),
                (true, _) => assert_eq!(Accent::of_under_glyph(a.glyph()), Some(a)),
            }
        }
        // The tilde/bar pairs swap roles between the sides.
        assert_eq!(Accent::of_over_glyph('˷'), Some(Accent::Tilde));
        assert_eq!(Accent::of_under_glyph('˜'), Some(Accent::Utilde));
        assert_eq!(Accent::of_over_glyph('_'), Some(Accent::Bar));
        assert_eq!(Accent::of_under_glyph('¯'), Some(Accent::Underline));
    }
}
