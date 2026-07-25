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

/// Super/subscript modifier letters (`\supA` = `\Asup` = `\^A` =
/// `\A^` = ᴬ). Unlike the alphabet families these are not a contiguous
/// block — Unicode has no capital form for every letter — so the table
/// is explicit. The editor shadows the `^` / `_` spellings with the
/// script *commands* (`\^z` builds a real superscript node), so these
/// normally reach a formula through `sup` / `sub`.
///
/// The style token is matched only against this table's arguments, so
/// the ordinary names it happens to prefix (`\supset` ⊃, `\subseteq`
/// ⊆) keep their own meaning.
pub struct Script {
    /// Style tokens that select it, leading or trailing.
    pub names: &'static [&'static str],
    /// (argument name, styled char) — a letter as a one-char name, or
    /// a symbol name (`alpha`).
    pub chars: &'static [(&'static str, char)],
}

pub const SCRIPTS: &[Script] = &[
    Script {
        names: &["sup", "^"],
        chars: &[
            ("A", 'ᴬ'),
            ("B", 'ᴮ'),
            ("D", 'ᴰ'),
            ("E", 'ᴱ'),
            ("G", 'ᴳ'),
            ("H", 'ᴴ'),
            ("I", 'ᴵ'),
            ("J", 'ᴶ'),
            ("K", 'ᴷ'),
            ("L", 'ᴸ'),
            ("M", 'ᴹ'),
            ("N", 'ᴺ'),
            ("O", 'ᴼ'),
            ("P", 'ᴾ'),
            ("R", 'ᴿ'),
            ("T", 'ᵀ'),
            ("U", 'ᵁ'),
            ("V", 'ⱽ'),
            ("W", 'ᵂ'),
            ("Z", 'ᶻ'),
            ("a", 'ᵃ'),
            ("alpha", 'ᵅ'),
            ("b", 'ᵇ'),
            ("beta", 'ᵝ'),
            ("c", 'ᶜ'),
            ("chi", 'ᵡ'),
            ("d", 'ᵈ'),
            ("delta", 'ᵟ'),
            ("e", 'ᵉ'),
            ("f", 'ᶠ'),
            ("g", 'ᵍ'),
            ("gamma", 'ᵞ'),
            ("h", 'ʰ'),
            ("j", 'ʲ'),
            ("k", 'ᵏ'),
            ("l", 'ˡ'),
            ("m", 'ᵐ'),
            ("o", 'ᵒ'),
            ("p", 'ᵖ'),
            ("phi", 'ᵠ'),
            ("r", 'ʳ'),
            ("s", 'ˢ'),
            ("t", 'ᵗ'),
            ("theta", 'ᶿ'),
            ("u", 'ᵘ'),
            ("v", 'ᵛ'),
            ("w", 'ʷ'),
            ("x", 'ˣ'),
            ("y", 'ʸ'),
            ("z", 'ᶻ'),
        ],
    },
    Script {
        names: &["sub", "_"],
        chars: &[("beta", 'ᵦ'), ("chi", 'ᵪ'), ("phi", 'ᵩ'), ("rho", 'ᵨ')],
    },
];

/// `\<style><arg>` or `\<arg><style>` for the super/subscript styles.
pub fn script_char(name: &str) -> Option<char> {
    SCRIPTS.iter().find_map(|s| {
        s.names.iter().find_map(|tok| {
            let arg = name
                .strip_prefix(tok)
                .or_else(|| name.strip_suffix(tok))
                .filter(|a| !a.is_empty())?;
            s.chars.iter().find(|&&(k, _)| k == arg).map(|&(_, c)| c)
        })
    })
}

/// Any styled character: an alphabet family or a super/subscript.
pub fn styled_char(name: &str) -> Option<char> {
    alphabet_char(name).or_else(|| script_char(name))
}
