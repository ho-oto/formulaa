//! Styled character families. The alphabet families (`\bbR` `\calL`
//! `\frakg` `\bfsf3`) come first; the super/subscript modifier letters
//! (`\supA` `\Asup`) follow as an explicit table. Each alphabet family
//! family maps the 26+26 ASCII letters (and often the 10 digits) onto a
//! contiguous Unicode block, with a handful of exceptions where the
//! letterlike symbols live outside it (ℂ ℋ ℝ …). Storing the rule
//! instead of ~1700 pairs keeps the table readable and turns the lookup
//! into arithmetic; the style token may lead or trail (`\frakA` =
//! `\Afrk`), and the modifier spellings of one family (\bfcal =
//! \calbf = \scrbf …) are just extra prefixes on the same row.
//!
//! Exactly one spelling collides across the whole set (a test proves
//! it): `\bbf` is both bb+f and b+bf. **The leading style wins**, so
//! `\bbf` is the double-struck f; the bold b is `\bfb` (and the
//! double-struck f is also `\fbb`), so both stay reachable
//! unambiguously. The generated source resolved it the other way.

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
    /// The LaTeX macro that styles a plain letter the same way, so the
    /// serializer can spell any member (`𝔸` -> `\mathbb{A}`). These are
    /// the unicode-math names, which is the toolchain the raw-Unicode
    /// fallback already assumes.
    pub latex: &'static str,
}

pub const ALPHABETS: &[Alphabet] = &[
    Alphabet {
        prefixes: &["bb"],
        upper: 0x1D538,
        lower: 0x1D552,
        digits: Some(0x1D7D8),
        latex: "mathbb",
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
        latex: "mathbf",
        exceptions: &[],
    },
    Alphabet {
        prefixes: &["bfcal", "bfscr", "calbf", "scrbf"],
        upper: 0x1D4D0,
        lower: 0x1D4EA,
        digits: Some(0x1D7CE),
        latex: "mathbfcal",
        exceptions: &[],
    },
    Alphabet {
        prefixes: &["bffrk", "frkbf", "bffrak", "frakbf"],
        upper: 0x1D56C,
        lower: 0x1D586,
        digits: Some(0x1D7CE),
        latex: "mathbffrak",
        exceptions: &[],
    },
    Alphabet {
        prefixes: &["bfit", "itbf"],
        upper: 0x1D468,
        lower: 0x1D482,
        digits: Some(0x1D7CE),
        latex: "mathbfit",
        exceptions: &[],
    },
    Alphabet {
        prefixes: &["bfitsf", "bfsfit", "itbfsf", "itsfbf", "sfbfit", "sfitbf"],
        upper: 0x1D63C,
        lower: 0x1D656,
        digits: Some(0x1D7EC),
        latex: "mathbfsfit",
        exceptions: &[],
    },
    Alphabet {
        prefixes: &["bfsf", "sfbf"],
        upper: 0x1D5D4,
        lower: 0x1D5EE,
        digits: Some(0x1D7EC),
        latex: "mathbfsf",
        exceptions: &[],
    },
    Alphabet {
        prefixes: &["cal", "scr"],
        upper: 0x1D49C,
        lower: 0x1D4B6,
        digits: None,
        latex: "mathcal",
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
        latex: "mathfrak",
        exceptions: &[('C', 'ℭ'), ('H', 'ℌ'), ('I', 'ℑ'), ('R', 'ℜ'), ('Z', 'ℨ')],
    },
    Alphabet {
        prefixes: &["itsf", "sfit"],
        upper: 0x1D608,
        lower: 0x1D622,
        digits: Some(0x1D7E2),
        latex: "mathsfit",
        exceptions: &[],
    },
    Alphabet {
        prefixes: &["sf"],
        upper: 0x1D5A0,
        lower: 0x1D5BA,
        digits: Some(0x1D7E2),
        latex: "mathsf",
        exceptions: &[],
    },
    Alphabet {
        prefixes: &["tt"],
        upper: 0x1D670,
        lower: 0x1D68A,
        digits: Some(0x1D7F6),
        latex: "mathtt",
        exceptions: &[],
    },
];

/// The styled character of `\<style><char>` or `\<char><style>` — the
/// style token may lead or trail (`\frakA` = `\Afrk`). The leading
/// form is tried first, which decides the one colliding spelling
/// (`\bbf`, see the module docs); `alphabet_spellings_agree` proves
/// every other spelling has a single reading.
pub fn alphabet_char(name: &str) -> Option<char> {
    let of = |fam: &Alphabet, ch: char| -> Option<char> {
        if let Some(&(_, c)) = fam.exceptions.iter().find(|&&(l, _)| l == ch) {
            return Some(c);
        }
        let (base, first) = match ch {
            'A'..='Z' => (fam.upper, b'A'),
            'a'..='z' => (fam.lower, b'a'),
            '0'..='9' => (fam.digits?, b'0'),
            _ => return None,
        };
        char::from_u32(base + (ch as u8 - first) as u32)
    };
    let family = |s: &str| ALPHABETS.iter().find(|a| a.prefixes.contains(&s));
    let first = name.chars().next()?;
    let last = name.chars().next_back()?;
    // <style><char>
    if let Some(fam) = family(&name[..name.len() - last.len_utf8()])
        && let Some(c) = of(fam, last)
    {
        return Some(c);
    }
    // <char><style>
    of(family(&name[first.len_utf8()..])?, first)
}

/// Any styled character (today: the alphabet families).
pub fn styled_char(name: &str) -> Option<char> {
    alphabet_char(name)
}

/// The LaTeX spelling of a styled letter (`𝔸` -> `\mathbb{A}`), found
/// by walking the families back — this is what keeps the char -> LaTeX
/// direction total for the ~700 characters they generate.
pub fn styled_latex(c: char) -> Option<String> {
    for fam in ALPHABETS {
        if let Some(&(l, _)) = fam.exceptions.iter().find(|&&(_, x)| x == c) {
            return Some(format!("\\{}{{{}}}", fam.latex, l));
        }
        for (base, first, span) in [
            (Some(fam.upper), b'A', 26),
            (Some(fam.lower), b'a', 26),
            (fam.digits, b'0', 10),
        ] {
            let Some(base) = base else { continue };
            if !(base..base + span).contains(&(c as u32)) {
                continue;
            }
            let l = (first + (c as u32 - base) as u8) as char;
            // …unless that slot is an exception, in which case this
            // codepoint belongs to no family after all.
            if fam.exceptions.iter().any(|&(e, _)| e == l) {
                continue;
            }
            return Some(format!("\\{}{{{}}}", fam.latex, l));
        }
    }
    None
}
