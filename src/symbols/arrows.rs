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

impl Arrow {
    pub const ALL: [Arrow; 4] = [Arrow::To, Arrow::From, Arrow::DoubleTo, Arrow::DoubleFrom];

    /// True when the arrow points right.
    pub fn right(self) -> bool {
        matches!(self, Arrow::To | Arrow::DoubleTo)
    }

    /// The rule the body is drawn with.
    pub fn body(self) -> char {
        match self {
            Arrow::To | Arrow::From => '─',
            Arrow::DoubleTo | Arrow::DoubleFrom => '═',
        }
    }

    pub fn latex(self) -> &'static str {
        match self {
            Arrow::To => "xrightarrow",
            Arrow::From => "xleftarrow",
            Arrow::DoubleTo => "xRightarrow",
            Arrow::DoubleFrom => "xLeftarrow",
        }
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
