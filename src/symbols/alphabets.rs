//! Styled character families (`\bbR` `\calL` `\frakg` `\bfsf3`). Each
//! alphabet family maps the 26+26 ASCII letters (and often the 10
//! digits) onto a contiguous Unicode block, with a handful of
//! exceptions where the letterlike symbols live outside it (ℂ ℋ ℝ …).
//! Storing the rule instead of ~1700 pairs keeps the table readable and
//! turns the lookup into arithmetic; the style token may lead or trail
//! (`\frakA` = `\Afrk`), and alias spellings (\bfcal = \calbf = \scrbf
//! …) are or-ed keys of the same row, like every other symbol table.
//!
//! Exactly one spelling collides across the whole set (a test proves
//! it): `\bbf` is both bb+f and b+bf. **The leading style wins**, so
//! `\bbf` is the double-struck f; the bold b is `\bfb` (and the
//! double-struck f is also `\fbb`), so both stay reachable
//! unambiguously.

pub struct Alphabet {
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

/// Style token -> family, aliases or-ed on (\bfcal = \calbf = \scrbf).
pub static ALPHABETS: phf::Map<&'static str, Alphabet> = phf::phf_map! {
    "bb" => Alphabet {
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
    "bf" => Alphabet {
        upper: 0x1D400,
        lower: 0x1D41A,
        digits: Some(0x1D7CE),
        latex: "mathbf",
        exceptions: &[],
    },
    "bfcal" | "bfscr" | "calbf" | "scrbf" => Alphabet {
        upper: 0x1D4D0,
        lower: 0x1D4EA,
        digits: Some(0x1D7CE),
        latex: "mathbfcal",
        exceptions: &[],
    },
    "bffrk" | "frkbf" | "bffrak" | "frakbf" => Alphabet {
        upper: 0x1D56C,
        lower: 0x1D586,
        digits: Some(0x1D7CE),
        latex: "mathbffrak",
        exceptions: &[],
    },
    "bfit" | "itbf" => Alphabet {
        upper: 0x1D468,
        lower: 0x1D482,
        digits: Some(0x1D7CE),
        latex: "mathbfit",
        exceptions: &[],
    },
    "bfitsf" | "bfsfit" | "itbfsf" | "itsfbf" | "sfbfit" | "sfitbf" => Alphabet {
        upper: 0x1D63C,
        lower: 0x1D656,
        digits: Some(0x1D7EC),
        latex: "mathbfsfit",
        exceptions: &[],
    },
    "bfsf" | "sfbf" => Alphabet {
        upper: 0x1D5D4,
        lower: 0x1D5EE,
        digits: Some(0x1D7EC),
        latex: "mathbfsf",
        exceptions: &[],
    },
    "cal" | "scr" => Alphabet {
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
    "frk" | "frak" => Alphabet {
        upper: 0x1D504,
        lower: 0x1D51E,
        digits: None,
        latex: "mathfrak",
        exceptions: &[('C', 'ℭ'), ('H', 'ℌ'), ('I', 'ℑ'), ('R', 'ℜ'), ('Z', 'ℨ')],
    },
    "itsf" | "sfit" => Alphabet {
        upper: 0x1D608,
        lower: 0x1D622,
        digits: Some(0x1D7E2),
        latex: "mathsfit",
        exceptions: &[],
    },
    "sf" => Alphabet {
        upper: 0x1D5A0,
        lower: 0x1D5BA,
        digits: Some(0x1D7E2),
        latex: "mathsf",
        exceptions: &[],
    },
    "tt" => Alphabet {
        upper: 0x1D670,
        lower: 0x1D68A,
        digits: Some(0x1D7F6),
        latex: "mathtt",
        exceptions: &[],
    },
};

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
    let first = name.chars().next()?;
    let last = name.chars().next_back()?;
    // <style><char>
    if let Some(fam) = ALPHABETS.get(&name[..name.len() - last.len_utf8()])
        && let Some(c) = of(fam, last)
    {
        return Some(c);
    }
    // <char><style>
    of(ALPHABETS.get(&name[first.len_utf8()..])?, first)
}

/// Any styled character (today: the alphabet families).
pub fn styled_char(name: &str) -> Option<char> {
    alphabet_char(name)
}

/// The LaTeX spelling of a styled letter (`𝔸` -> `\mathbb{A}`), found
/// by walking the families back — this is what keeps the char -> LaTeX
/// direction total for the ~700 characters they generate. The digit
/// blocks are shared between families (bold digits serve \mathbf and
/// \mathbfcal alike): the family with the shortest LaTeX name owns
/// them, so the plainest spelling wins deterministically.
pub fn styled_latex(c: char) -> Option<String> {
    let mut best: Option<(&Alphabet, char)> = None;
    for fam in ALPHABETS.values() {
        let letter = if let Some(&(l, _)) = fam.exceptions.iter().find(|&&(_, x)| x == c) {
            Some(l)
        } else {
            [
                (Some(fam.upper), b'A', 26),
                (Some(fam.lower), b'a', 26),
                (fam.digits, b'0', 10),
            ]
            .into_iter()
            .find_map(|(base, first, span)| {
                let base = base?;
                if !(base..base + span).contains(&(c as u32)) {
                    return None;
                }
                let l = (first + (c as u32 - base) as u8) as char;
                // …unless that slot is an exception, in which case this
                // codepoint belongs to no family after all.
                (!fam.exceptions.iter().any(|&(e, _)| e == l)).then_some(l)
            })
        };
        if let Some(l) = letter
            && best.is_none_or(|(b, _)| fam.latex.len() < b.latex.len())
        {
            best = Some((fam, l));
        }
    }
    best.map(|(fam, l)| format!("\\{}{{{}}}", fam.latex, l))
}

#[cfg(test)]
mod tests {
    use crate::symbols::*;
    /// The alphabet families cover both cases, including the letterlike
    /// exceptions that sit outside their block.
    #[test]
    fn alphabet_families_resolve() {
        for (name, want) in [
            ("bbR", 'ℝ'),
            ("bbA", '𝔸'),
            ("bbf", '𝕗'),
            ("calL", 'ℒ'),
            ("scrL", 'ℒ'),
            ("frakg", '𝔤'),
            ("frakH", 'ℌ'),
            ("bfa", '𝐚'),
            ("ttZ", '𝚉'),
            ("sfbfitq", '𝙦'),
            ("calbfA", '𝓐'),
            ("scrbfA", '𝓐'),
            ("bfsf3", '𝟯'),
            ("tt7", '𝟽'),
            ("frkZ", 'ℨ'),
        ] {
            assert_eq!(symbol_by_name(name), Some(want), "\\{}", name);
        }
        // Every family maps all 52 letters (and its digits) to a
        // distinct char, under every alias spelling.
        for (style, fam) in alphabets::ALPHABETS.entries() {
            let mut seen = std::collections::HashSet::new();
            let chars: Vec<char> = ('A'..='Z')
                .chain('a'..='z')
                .chain(fam.digits.iter().flat_map(|_| '0'..='9'))
                .collect();
            for &l in &chars {
                let c = alphabets::alphabet_char(&format!("{}{}", style, l))
                    .unwrap_or_else(|| panic!("{}{} missing", style, l));
                assert!(seen.insert(c), "{}{} duplicates {:?}", style, l, c);
            }
        }
        assert_eq!(symbol_by_name("nosuchfamilyX"), None);
        // Both spelling orders, letters and digits.
        for (a, b) in [("frakA", "Afrk"), ("bfsf3", "3bfsf"), ("ttz", "ztt")] {
            assert_eq!(symbol_by_name(a), symbol_by_name(b), "{} vs {}", a, b);
            assert!(symbol_by_name(a).is_some(), "{}", a);
        }
    }
    /// The style token may lead or trail, so a name like `\bbb` has two
    /// readings. Exhaustively check that they agree (and that every
    /// spelling resolves), so the lookup is well-defined.
    #[test]
    fn alphabet_spellings_agree() {
        for (style, fam) in alphabets::ALPHABETS.entries() {
            for ch in ('A'..='Z')
                .chain('a'..='z')
                .chain(fam.digits.iter().flat_map(|_| '0'..='9'))
            {
                let pre = alphabets::alphabet_char(&format!("{}{}", style, ch));
                let suf = alphabets::alphabet_char(&format!("{}{}", ch, style));
                assert!(pre.is_some(), "\\{}{} missing", style, ch);
                // …the trailing spelling agrees, except for the one
                // collision pinned down below.
                if format!("{}{}", ch, style) != "bbf" {
                    assert_eq!(pre, suf, "\\{}{} vs \\{}{}", style, ch, ch, style);
                }
            }
        }
        // \bbf is the only spelling both readings claim (bb+f vs b+bf):
        // the leading style wins, and both chars stay reachable.
        for lead in alphabets::ALPHABETS.keys() {
            for trail in alphabets::ALPHABETS.keys() {
                if lead.len() != trail.len() || lead[1..] != trail[..trail.len() - 1] {
                    continue;
                }
                let name = format!("{}{}", lead, &trail[trail.len() - 1..]);
                let x = name.chars().next().unwrap();
                let y = name.chars().next_back().unwrap();
                let (as_lead, as_trail) = (
                    alphabets::alphabet_char(&format!("{}{}", lead, y)),
                    alphabets::alphabet_char(&format!("{}{}", x, trail)),
                );
                if as_lead != as_trail {
                    assert_eq!(name, "bbf", "new collision: \\{}", name);
                    assert_eq!(alphabets::alphabet_char("bbf"), as_lead);
                    assert_eq!(alphabets::alphabet_char("fbb"), as_lead);
                    assert_eq!(alphabets::alphabet_char("bfb"), as_trail);
                }
            }
        }
    }
}
