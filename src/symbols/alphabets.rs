//! Styled alphabet families: `\bbR` `\calL` `\frakg` `\bfsf3` … Each
//! family maps the 26+26 ASCII letters (and often the 10 digits) onto a
//! contiguous Unicode block, with a handful of exceptions where the
//! letterlike symbols live outside it (ℂ ℋ ℝ …). Storing the rule
//! instead of ~1700 pairs keeps the table readable and turns the lookup
//! into arithmetic; the modifier spellings of one family (\bfcal =
//! \calbf = \scrbf …) are just extra prefixes on the same row.
//!
//! (The generated source has one typo — `\bbf` listed as the bold b
//! of `\bfb` — which the rule form made obvious; it is not carried
//! over here.)

pub struct Alphabet {
    /// Command prefixes that select this family (\calL, \scrL).
    pub prefixes: &'static [&'static str],
    /// Codepoint of the family's 'A' and 'a'.
    pub upper: u32,
    pub lower: u32,
    /// Codepoint of its '0', when the family styles digits too.
    pub digits: Option<u32>,
    /// Characters that sit outside the block (letterlike symbols).
    pub exceptions: &'static [(char, char)],
}

pub const ALPHABETS: &[Alphabet] = &[
    Alphabet {
        prefixes: &["bb"],
        upper: 0x1D538,
        lower: 0x1D552,
        digits: Some(0x1D7D8),
        exceptions: &[
            ('C', 'ℂ'),
            ('H', 'ℍ'),
            ('N', 'ℕ'),
            ('P', 'ℙ'),
            ('Q', 'ℚ'),
            ('R', 'ℝ'),
            ('Z', 'ℤ'),
        ],
    },
    Alphabet {
        prefixes: &["bf"],
        upper: 0x1D400,
        lower: 0x1D41A,
        digits: Some(0x1D7CE),
        exceptions: &[],
    },
    Alphabet {
        prefixes: &["bfcal", "bfscr", "calbf", "scrbf"],
        upper: 0x1D4D0,
        lower: 0x1D4EA,
        digits: Some(0x1D7CE),
        exceptions: &[],
    },
    Alphabet {
        prefixes: &["bffrk", "frkbf", "bffrak", "frakbf"],
        upper: 0x1D56C,
        lower: 0x1D586,
        digits: Some(0x1D7CE),
        exceptions: &[],
    },
    Alphabet {
        prefixes: &["bfit", "itbf"],
        upper: 0x1D468,
        lower: 0x1D482,
        digits: Some(0x1D7CE),
        exceptions: &[],
    },
    Alphabet {
        prefixes: &["bfitsf", "bfsfit", "itbfsf", "itsfbf", "sfbfit", "sfitbf"],
        upper: 0x1D63C,
        lower: 0x1D656,
        digits: Some(0x1D7EC),
        exceptions: &[],
    },
    Alphabet {
        prefixes: &["bfsf", "sfbf"],
        upper: 0x1D5D4,
        lower: 0x1D5EE,
        digits: Some(0x1D7EC),
        exceptions: &[],
    },
    Alphabet {
        prefixes: &["cal", "scr"],
        upper: 0x1D49C,
        lower: 0x1D4B6,
        digits: None,
        exceptions: &[
            ('B', 'ℬ'),
            ('E', 'ℰ'),
            ('F', 'ℱ'),
            ('H', 'ℋ'),
            ('I', 'ℐ'),
            ('L', 'ℒ'),
            ('M', 'ℳ'),
            ('R', 'ℛ'),
            ('e', 'ℯ'),
            ('g', 'ℊ'),
            ('o', 'ℴ'),
        ],
    },
    Alphabet {
        prefixes: &["frk", "frak"],
        upper: 0x1D504,
        lower: 0x1D51E,
        digits: None,
        exceptions: &[('C', 'ℭ'), ('H', 'ℌ'), ('I', 'ℑ'), ('R', 'ℜ'), ('Z', 'ℨ')],
    },
    Alphabet {
        prefixes: &["itsf", "sfit"],
        upper: 0x1D608,
        lower: 0x1D622,
        digits: Some(0x1D7E2),
        exceptions: &[],
    },
    Alphabet {
        prefixes: &["sf"],
        upper: 0x1D5A0,
        lower: 0x1D5BA,
        digits: Some(0x1D7E2),
        exceptions: &[],
    },
    Alphabet {
        prefixes: &["tt"],
        upper: 0x1D670,
        lower: 0x1D68A,
        digits: Some(0x1D7F6),
        exceptions: &[],
    },
];

/// `\<prefix><letter or digit>` -> the styled character.
pub fn alphabet_char(name: &str) -> Option<char> {
    let last = name
        .chars()
        .next_back()
        .filter(char::is_ascii_alphanumeric)?;
    let fam = ALPHABETS
        .iter()
        .find(|a| a.prefixes.contains(&&name[..name.len() - 1]))?;
    if let Some(&(_, c)) = fam.exceptions.iter().find(|&&(l, _)| l == last) {
        return Some(c);
    }
    let (base, first) = match last {
        'A'..='Z' => (fam.upper, b'A'),
        'a'..='z' => (fam.lower, b'a'),
        _ => (fam.digits?, b'0'),
    };
    char::from_u32(base + (last as u8 - first) as u32)
}
