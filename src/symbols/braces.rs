//! The over/underbrace glyphs (`Node::Brace`): a ╭──╮ row hugging the
//! argument (╰──╯ underneath for \underbrace), body shared with the
//! fraction bar.

pub const BRACE_TL: char = '╭'; // U+256D
pub const BRACE_TR: char = '╮'; // U+256E
pub const BRACE_BL: char = '╰'; // U+2570
pub const BRACE_BR: char = '╯'; // U+256F

/// The corner pair of the brace row: (left, right) for the given
/// direction.
pub const fn brace_corners(over: bool) -> (char, char) {
    if over {
        (BRACE_TL, BRACE_TR)
    } else {
        (BRACE_BL, BRACE_BR)
    }
}

/// Any of the four brace corners.
pub fn is_brace_corner(c: char) -> bool {
    matches!(c, BRACE_TL | BRACE_TR | BRACE_BL | BRACE_BR)
}
