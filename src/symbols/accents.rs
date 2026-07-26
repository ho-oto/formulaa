//! Accent marks: the AST mark chars, their over/under side and their
//! LaTeX commands. The drawn forms (¯ ˰ ￫ …) belong to render — this
//! is the vocabulary, not the picture.

/// Accent marks: (command name, mark char, is_under, latex command).
/// Mark chars are RESERVED — they never occur as atoms, and over-marks are
/// disjoint from under-marks; this is what makes a two-character column
/// (mark stacked on base) unambiguous for the parser.
/// AST mark chars; the hugging glyphs actually drawn are mapped in
/// render (over_glyph/under_glyph) and parse (over_mark_at/under_mark_at).
pub const ACCENTS: &[(&str, char, bool, &str)] = &[
    ("hat", '^', false, "hat"),            // U+005E, drawn ˰ U+02F0
    ("tilde", '˜', false, "tilde"),        // U+02DC (not the atom '~'), drawn ˷ U+02F7
    ("bar", '¯', false, "bar"),            // U+00AF MACRON, drawn _ U+005F
    ("vec", '⇀', false, "vec"),            // U+21C0 (not the atom '→'), drawn ￫ U+FFEB
    ("dot", '˙', false, "dot"),            // U+02D9, drawn ․ U+2024
    ("ddot", '¨', false, "ddot"),          // U+00A8, drawn ․․ (overhangs right)
    ("check", 'ˇ', false, "check"),        // U+02C7 CARON, drawn ˯ U+02EF
    ("ring", '˚', false, "mathring"),      // U+02DA, drawn ˳ U+02F3
    ("underline", '‗', true, "underline"), // U+2017 DOUBLE LOW LINE, drawn ¯
    // Under tilde: the AST marks form a swapped pair with the drawn
    // glyphs — over tilde ˜ draws as the low ˷, under tilde ˷ draws as
    // the high ˜ (both hug the base).
    ("utilde", '˷', true, "utilde"), // U+02F7 LOW TILDE, drawn ˜
];

pub fn accent_by_name(name: &str) -> Option<(char, bool)> {
    ACCENTS
        .iter()
        .find(|(n, ..)| *n == name)
        .map(|&(_, c, under, _)| (c, under))
}

pub fn accent_info(mark: char) -> Option<(bool, &'static str)> {
    ACCENTS
        .iter()
        .find(|&&(_, c, ..)| c == mark)
        .map(|&(_, _, under, latex)| (under, latex))
}

/// LaTeX command for a stretchy (multi-char) accent; marks without a
/// wide variant use their plain command (\dot etc. accept groups).
pub fn wide_accent_latex(mark: char) -> &'static str {
    match mark {
        '^' => "widehat",
        '˜' => "widetilde",
        '¯' => "overline",
        '⇀' => "overrightarrow",
        'ˇ' => "widecheck",
        m => accent_info(m).map(|(_, l)| l).unwrap_or("widehat"),
    }
}

pub fn is_over_mark(c: char) -> bool {
    ACCENTS.iter().any(|&(_, m, under, _)| m == c && !under)
}

pub fn is_under_mark(c: char) -> bool {
    ACCENTS.iter().any(|&(_, m, under, _)| m == c && under)
}
