//! The stretchy labeled arrows (\xto and friends): kind, direction,
//! body rule and LaTeX command, as one bijective table.

/// A stretchy labeled arrow: the four kinds, with the body they draw
/// and the LaTeX command they serialize to. The head is always the
/// ASCII `<` / `>` (box rules and Unicode arrows do not line up across
/// fonts), so only the body distinguishes single from double.
pub struct ArrowSpec {
    /// The AST's operator char.
    pub op: char,
    /// True when the arrow points right.
    pub right: bool,
    /// The rule the body is drawn with.
    pub body: char,
    /// The LaTeX command (the doubles need mathtools).
    pub latex: &'static str,
}

pub const ARROWS: &[ArrowSpec] = &[
    ArrowSpec {
        op: '→',
        right: true,
        body: '─',
        latex: "xrightarrow",
    },
    ArrowSpec {
        op: '←',
        right: false,
        body: '─',
        latex: "xleftarrow",
    },
    ArrowSpec {
        op: '⇒',
        right: true,
        body: '═',
        latex: "xRightarrow",
    },
    ArrowSpec {
        op: '⇐',
        right: false,
        body: '═',
        latex: "xLeftarrow",
    },
];

pub fn arrow_of(op: char) -> Option<&'static ArrowSpec> {
    ARROWS.iter().find(|a| a.op == op)
}

/// The arrow a body rule draws, in the given direction.
pub fn arrow_by_body(body: char, right: bool) -> Option<&'static ArrowSpec> {
    ARROWS.iter().find(|a| a.body == body && a.right == right)
}

#[cfg(test)]
mod tests {
    use super::*;
    /// The arrows are a bijection between (body, direction) and kind,
    /// which is what lets the parser read a body run back to its op.
    #[test]
    fn arrow_table_is_a_bijection() {
        for a in ARROWS {
            assert_eq!(arrow_of(a.op).map(|x| x.op), Some(a.op));
            let back = arrow_by_body(a.body, a.right).expect("body resolves");
            assert_eq!(back.op, a.op, "{:?} is ambiguous", a.body);
            assert!(a.latex.starts_with('x'), "{:?}", a.latex);
        }
        assert_eq!(ARROWS.len(), 4);
    }
}
