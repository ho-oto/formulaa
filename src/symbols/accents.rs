//! Accent marks, as an enum: the AST stores `Accent`, and everything
//! about a mark — its command name, LaTeX spelling, side, and the
//! glyph its picture shows — is answered here. Mark chars appear in
//! no picture and no output (adr.md §62).

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
    /// The one-line stand-in for previews (the minibuffer preview and
    /// the completion's symbol column): the accent applied to `⬚` is
    /// two lines tall, and its drawn glyph alone (`˰`, `․`) reads as a
    /// stray speck — the spacing form (`ˆ`, `˙`) is the character that
    /// *means* the mark on its own.
    pub preview: char,
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
    pub const fn info(self) -> &'static AccentInfo {
        match self {
            // Drawn glyphs: ˰ U+02F0 LOW UP ARROWHEAD, ˯ U+02EF LOW
            // DOWN ARROWHEAD, ˳ U+02F3 LOW RING, ․ U+2024 LEADER (not
            // the '.' atom), ￫ halfwidth U+FFEB (not the → atom),
            // ˷ U+02F7 LOW TILDE, ˜ U+02DC SMALL TILDE.
            Accent::Hat => &AccentInfo {
                name: "hat",
                latex: "hat",
                wide_latex: "widehat",
                under: false,
                drawn: DrawnForm::Center('˰'),
                preview: 'ˆ',
            },
            Accent::Tilde => &AccentInfo {
                name: "tilde",
                latex: "tilde",
                wide_latex: "widetilde",
                under: false,
                drawn: DrawnForm::Fill('˷'),
                preview: '˜',
            },
            Accent::Bar => &AccentInfo {
                name: "bar",
                latex: "bar",
                wide_latex: "overline",
                under: false,
                drawn: DrawnForm::Fill('_'),
                preview: '¯',
            },
            Accent::Vec => &AccentInfo {
                name: "vec",
                latex: "vec",
                wide_latex: "overrightarrow",
                under: false,
                drawn: DrawnForm::Center('￫'),
                preview: '→',
            },
            Accent::Dot => &AccentInfo {
                name: "dot",
                latex: "dot",
                wide_latex: "dot",
                under: false,
                drawn: DrawnForm::Center('․'),
                preview: '˙',
            },
            Accent::Ddot => &AccentInfo {
                name: "ddot",
                latex: "ddot",
                wide_latex: "ddot",
                under: false,
                drawn: DrawnForm::Dots,
                preview: '¨',
            },
            Accent::Check => &AccentInfo {
                name: "check",
                latex: "check",
                wide_latex: "widecheck",
                under: false,
                drawn: DrawnForm::Center('˯'),
                preview: 'ˇ',
            },
            Accent::Ring => &AccentInfo {
                name: "ring",
                latex: "mathring",
                wide_latex: "mathring",
                under: false,
                drawn: DrawnForm::Center('˳'),
                preview: '˚',
            },
            Accent::Underline => &AccentInfo {
                name: "underline",
                latex: "underline",
                wide_latex: "underline",
                under: true,
                drawn: DrawnForm::Fill('¯'),
                preview: '_',
            },
            Accent::Utilde => &AccentInfo {
                name: "utilde",
                latex: "utilde",
                wide_latex: "utilde",
                under: true,
                drawn: DrawnForm::Fill('˜'),
                preview: '˷',
            },
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

    /// The cells this mark occupies in a compact column, in order —
    /// the ddot is the one mark wider than its base (`․․` overhanging
    /// one column right); every other mark is its single glyph.
    pub fn cells(self) -> Vec<char> {
        match self.drawn() {
            DrawnForm::Dots => vec!['․', '․'],
            _ => vec![self.glyph()],
        }
    }

    /// The mark whose compact cells are this mark's glyph followed by
    /// `next` — how the reader upgrades a length-sensitive pair
    /// (`․․` = a dot next to a dot is the ddot).
    pub fn widen(self, next: char) -> Option<Accent> {
        Accent::ALL
            .into_iter()
            .find(|a| a.cells() == [self.glyph(), next])
    }

    /// A glyph some mark draws with (band material and compact columns
    /// alike).
    pub fn is_mark_material(c: char) -> bool {
        Accent::ALL.iter().any(|a| a.glyph() == c)
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

    /// Every mark's one-line preview is its own: two accents sharing
    /// a stand-in would be indistinguishable in the completion list.
    #[test]
    fn previews_are_distinct() {
        let mut seen = std::collections::HashSet::new();
        for a in Accent::ALL {
            assert!(
                seen.insert(a.info().preview),
                "{:?} reuses the preview {:?}",
                a,
                a.info().preview
            );
        }
    }

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
