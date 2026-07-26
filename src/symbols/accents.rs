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

    /// The `\name` that applies this mark (also its LaTeX command,
    /// except `\ring` -> `\mathring`).
    pub fn name(self) -> &'static str {
        match self {
            Accent::Hat => "hat",
            Accent::Tilde => "tilde",
            Accent::Bar => "bar",
            Accent::Vec => "vec",
            Accent::Dot => "dot",
            Accent::Ddot => "ddot",
            Accent::Check => "check",
            Accent::Ring => "ring",
            Accent::Underline => "underline",
            Accent::Utilde => "utilde",
        }
    }

    pub fn of_name(name: &str) -> Option<Accent> {
        Accent::ALL.into_iter().find(|a| a.name() == name)
    }

    /// The LaTeX command for the single-char form.
    pub fn latex(self) -> &'static str {
        match self {
            Accent::Ring => "mathring",
            a => a.name(),
        }
    }

    /// The LaTeX command for the stretchy (wide) form; marks without
    /// one use their plain command (\dot etc. accept groups).
    pub fn wide_latex(self) -> &'static str {
        match self {
            Accent::Hat => "widehat",
            Accent::Tilde => "widetilde",
            Accent::Bar => "overline",
            Accent::Vec => "overrightarrow",
            Accent::Check => "widecheck",
            a => a.latex(),
        }
    }

    /// Under-marks hug the base from below; everything else is over.
    pub fn under(self) -> bool {
        matches!(self, Accent::Underline | Accent::Utilde)
    }

    /// The glyph the picture shows. Every mark hugs its base: over
    /// marks draw low in their cell (bar as `_` like the √ overline,
    /// tilde as the low `˷`), under marks draw high (underline as `¯`,
    /// utilde as the high `˜`) — so the tilde/bar pairs swap between
    /// the over and under roles, and every drawn glyph names exactly
    /// one (mark, side).
    pub fn drawn(self) -> DrawnForm {
        match self {
            Accent::Hat => DrawnForm::Center('˰'), // U+02F0 LOW UP ARROWHEAD
            Accent::Check => DrawnForm::Center('˯'), // U+02EF LOW DOWN ARROWHEAD
            Accent::Ring => DrawnForm::Center('˳'), // U+02F3 LOW RING
            Accent::Dot => DrawnForm::Center('․'), // U+2024 LEADER (not the '.' atom)
            // Halfwidth ￫ U+FFEB, distinct from the → atom.
            Accent::Vec => DrawnForm::Center('￫'),
            Accent::Ddot => DrawnForm::Dots,
            Accent::Bar => DrawnForm::Fill('_'),
            Accent::Underline => DrawnForm::Fill('¯'),
            Accent::Tilde => DrawnForm::Fill('˷'), // U+02F7 LOW TILDE
            Accent::Utilde => DrawnForm::Fill('˜'), // U+02DC SMALL TILDE
        }
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
