//! The stretchy labeled arrows (`\xto` and friends), as an enum: the
//! AST stores `Arrow`, and its direction, body rule and LaTeX command
//! are answered here. The head is always the ASCII `<` / `>` (box
//! rules and Unicode arrows do not line up across fonts), so only the
//! body distinguishes single from double.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arrow {
    /// `\xto` — `──>`
    To,
    /// `\xfrom` — `<──`
    From,
    /// `\xTo` — `══>` (LaTeX needs mathtools)
    DoubleTo,
    /// `\xFrom` — `<══`
    DoubleFrom,
}

/// Everything about one arrow, in one row — `info` is the single
/// match, so a variant's whole story reads in one place.
pub struct ArrowInfo {
    /// The editor's canonical `\name`.
    pub name: &'static str,
    /// True when the arrow points right.
    pub right: bool,
    /// The rule the body is drawn with.
    pub body: char,
    pub latex: &'static str,
}

/// The input direction, same shape as the symbol `NAMES` table:
/// spelling -> variant, canonical first, aliases or-ed on. New
/// spellings are one `|` here.
pub static ARROW_NAMES: phf::Map<&'static str, Arrow> = phf::phf_map! {
    "xto" | "xrightarrow" => Arrow::To,
    "xfrom" | "xleftarrow" => Arrow::From,
    "xTo" | "xRightarrow" => Arrow::DoubleTo,
    "xFrom" | "xLeftarrow" => Arrow::DoubleFrom,
};

impl Arrow {
    pub const ALL: [Arrow; 4] = [Arrow::To, Arrow::From, Arrow::DoubleTo, Arrow::DoubleFrom];

    /// The one row that says everything about this arrow.
    #[rustfmt::skip]
    pub const fn info(self) -> &'static ArrowInfo {
        match self {
            Arrow::To         => &ArrowInfo { name: "xto",   right: true,  body: '─', latex: "xrightarrow" },
            Arrow::From       => &ArrowInfo { name: "xfrom", right: false, body: '─', latex: "xleftarrow" },
            Arrow::DoubleTo   => &ArrowInfo { name: "xTo",   right: true,  body: '═', latex: "xRightarrow" },
            Arrow::DoubleFrom => &ArrowInfo { name: "xFrom", right: false, body: '═', latex: "xLeftarrow" },
        }
    }

    /// The arrow a `\name` builds (any spelling in its `ARROW_NAMES`
    /// entry).
    pub fn of_name(name: &str) -> Option<Arrow> {
        ARROW_NAMES.get(name).copied()
    }

    pub fn right(self) -> bool {
        self.info().right
    }

    /// The editor's canonical `\\name`.
    pub fn name(self) -> &'static str {
        self.info().name
    }

    pub fn body(self) -> char {
        self.info().body
    }

    pub fn latex(self) -> &'static str {
        self.info().latex
    }

    /// The arrow a body rule draws, in the given direction — the
    /// bijection the parser reads a body run back through.
    pub fn of_body(body: char, right: bool) -> Option<Arrow> {
        Arrow::ALL
            .into_iter()
            .find(|a| a.body() == body && a.right() == right)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arrows_are_a_bijection() {
        for a in Arrow::ALL {
            assert_eq!(Arrow::of_body(a.body(), a.right()), Some(a));
            assert!(a.latex().starts_with('x'));
            // Both spellings of the input row build the arrow.
            assert_eq!(Arrow::of_name(a.info().name), Some(a));
            assert_eq!(Arrow::of_name(a.latex()), Some(a));
        }
    }
}
