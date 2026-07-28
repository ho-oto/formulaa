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
    /// True when the arrow points right.
    pub right: bool,
    /// The rule the body is drawn with.
    pub body: char,
    pub latex: &'static str,
}

impl Arrow {
    pub const ALL: [Arrow; 4] = [Arrow::To, Arrow::From, Arrow::DoubleTo, Arrow::DoubleFrom];

    /// The one row that says everything about this arrow.
    #[rustfmt::skip]
    pub const fn info(self) -> &'static ArrowInfo {
        match self {
            Arrow::To         => &ArrowInfo { right: true,  body: '─', latex: "xrightarrow" },
            Arrow::From       => &ArrowInfo { right: false, body: '─', latex: "xleftarrow" },
            Arrow::DoubleTo   => &ArrowInfo { right: true,  body: '═', latex: "xRightarrow" },
            Arrow::DoubleFrom => &ArrowInfo { right: false, body: '═', latex: "xLeftarrow" },
        }
    }

    pub fn right(self) -> bool {
        self.info().right
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
        }
    }
}
