//! The delimiter pairs, as an enum: the AST stores a `Delim` kind per
//! side (the slot decides left/right, so a `]` shape cannot end up in
//! the left slot), and everything about a pair — its spec chars, the
//! one-line and tall glyph columns, the brace vertex and the LaTeX
//! spelling — is answered by `info` in one row. parse/render/latex all
//! ask here, so a new pair is added in exactly one place. Angles are
//! the one family drawn from diagonal arms instead of a stacked
//! column, so their `tall` is None and the renderer draws the arms
//! itself. The norm `‖` is not a pair (both sides are the same glyph,
//! told apart by extent) — it stays its own node.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delim {
    Paren,
    Bracket,
    Brace,
    Ceil,
    Floor,
    Angle,
    Bar,
    Null,
}

/// Everything about one pair, in one row; paired fields are
/// (left, right).
pub struct DelimInfo {
    /// The spec chars the `\lr` syntax spells (the side-symmetric
    /// `|` and `.` repeat one char).
    pub spec: (char, char),
    /// How the pair draws on one line.
    pub short: (char, char),
    /// The tall column, top to bottom: (top, extension, bottom) per
    /// side. `⌈` has no foot and `⌊` no head, so those repeat the
    /// extension — which is exactly what tells the families apart.
    /// None for the angles (diagonal arms, no stacked column).
    pub tall: Option<[(char, char, char); 2]>,
    /// The vertex a brace puts on the baseline row (no other pair has
    /// one, and it takes precedence over the corners).
    pub vertex: Option<(char, char)>,
    /// The LaTeX spelling per side.
    pub latex: (&'static str, &'static str),
}

/// The `\lr` spec's `\name` tokens, same shape as the symbol `NAMES`
/// table: spelling -> spec char, aliases or-ed on.
pub static DELIM_NAMES: phf::Map<&'static str, char> = phf::phf_map! {
    "lparen" => '(',
    "rparen" => ')',
    "lbrack" => '[',
    "rbrack" => ']',
    "lbrace" => '{',
    "rbrace" => '}',
    "langle" => '⟨',
    "rangle" => '⟩',
    "vert" | "mid" => '|',
    "dot" | "none" => '.',
};

impl Delim {
    pub const ALL: [Delim; 8] = [
        Delim::Paren,
        Delim::Bracket,
        Delim::Brace,
        Delim::Ceil,
        Delim::Floor,
        Delim::Angle,
        Delim::Bar,
        Delim::Null,
    ];

    /// The one row that says everything about this pair.
    #[rustfmt::skip]
    pub const fn info(self) -> &'static DelimInfo {
        match self {
            Delim::Paren => &DelimInfo {
                spec: ('(', ')'),
                short: ('(', ')'),
                tall: Some([('⎛', '⎜', '⎝'), ('⎞', '⎟', '⎠')]),
                vertex: None,
                latex: ("(", ")"),
            },
            Delim::Bracket => &DelimInfo {
                spec: ('[', ']'),
                short: ('[', ']'),
                tall: Some([('⎡', '⎢', '⎣'), ('⎤', '⎥', '⎦')]),
                vertex: None,
                latex: ("[", "]"),
            },
            Delim::Brace => &DelimInfo {
                spec: ('{', '}'),
                short: ('{', '}'),
                tall: Some([('⎧', '⎪', '⎩'), ('⎫', '⎪', '⎭')]),
                vertex: Some(('⎨', '⎬')),
                latex: ("\\{", "\\}"),
            },
            // Ceil and floor reuse the bracket pieces with one corner
            // missing.
            Delim::Ceil => &DelimInfo {
                spec: ('⌈', '⌉'),
                short: ('⌈', '⌉'),
                tall: Some([('⎡', '⎢', '⎢'), ('⎤', '⎥', '⎥')]),
                vertex: None,
                latex: ("\\lceil ", "\\rceil "),
            },
            Delim::Floor => &DelimInfo {
                spec: ('⌊', '⌋'),
                short: ('⌊', '⌋'),
                tall: Some([('⎢', '⎢', '⎣'), ('⎥', '⎥', '⎦')]),
                vertex: None,
                latex: ("\\lfloor ", "\\rfloor "),
            },
            // Angles: U+27E8/27E9 one-line; tall forms are diagonal
            // arm glyphs ╱ ╲ the renderer draws itself.
            Delim::Angle => &DelimInfo {
                spec: ('⟨', '⟩'),
                short: ('⟨', '⟩'),
                tall: None,
                vertex: None,
                latex: ("\\langle ", "\\rangle "),
            },
            // The vertical bar is a cornerless extension column —
            // which is how the column walk tells it from ceil/floor.
            Delim::Bar => &DelimInfo {
                spec: ('|', '|'),
                short: ('⎢', '⎥'),
                tall: Some([('⎢', '⎢', '⎢'), ('⎥', '⎥', '⎥')]),
                vertex: None,
                latex: ("|", "|"),
            },
            // The null pair (\left. / \right.): dashed ghosts, like
            // the ┈ band.
            Delim::Null => &DelimInfo {
                spec: ('.', '.'),
                short: ('┆', '┊'),
                tall: Some([('┆', '┆', '┆'), ('┊', '┊', '┊')]),
                vertex: None,
                latex: (".", "."),
            },
        }
    }

    /// The pair a spec char belongs to, and which side the char spells
    /// (None = either; the side-symmetric `|` and `.`).
    pub fn of_spec(c: char) -> Option<(Delim, Option<bool>)> {
        Delim::ALL.into_iter().find_map(|d| {
            let (l, r) = d.info().spec;
            if l == r && c == l {
                Some((d, None))
            } else if c == l {
                Some((d, Some(true)))
            } else if c == r {
                Some((d, Some(false)))
            } else {
                None
            }
        })
    }

    /// The pair a spec char spells when it stands on the given side —
    /// None when it cannot (a `]` has no business in the left slot).
    pub fn of_spec_side(c: char, left: bool) -> Option<Delim> {
        match Delim::of_spec(c)? {
            (d, None) => Some(d),
            (d, Some(side)) if side == left => Some(d),
            _ => None,
        }
    }

    /// The spec char of this pair's given side.
    pub fn spec(self, left: bool) -> char {
        let (l, r) = self.info().spec;
        if left { l } else { r }
    }

    pub fn latex(self, left: bool) -> &'static str {
        let (l, r) = self.info().latex;
        if left { l } else { r }
    }

    /// Every glyph this side can show, in any height (the angles show
    /// only their one-line char — the ╱ ╲ arms are shared diagonals
    /// the parser resolves contextually).
    pub fn glyphs(self, left: bool) -> Vec<char> {
        let info = self.info();
        let side = |p: (char, char)| if left { p.0 } else { p.1 };
        let mut v = vec![side(info.short)];
        if let Some(tall) = info.tall {
            let (t, e, b) = tall[usize::from(!left)];
            v.extend([t, e, b]);
        }
        if let Some(vx) = info.vertex {
            v.push(side(vx));
        }
        v.sort_unstable();
        v.dedup();
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    /// The info rows are the single description of each pair, so the
    /// pieces they list have to be consistent: distinct sides,
    /// distinct one-line glyphs, and specs that resolve back to their
    /// own row and side.
    #[test]
    fn delimiter_table_is_consistent() {
        let mut seen_short = std::collections::HashSet::new();
        for d in Delim::ALL {
            let info = d.info();
            for left in [true, false] {
                let spec = d.spec(left);
                let (row, side) = Delim::of_spec(spec).expect("a spec resolves");
                assert_eq!(row, d, "{:?} resolves elsewhere", spec);
                // A side-distinct spec names its own side; a symmetric
                // one stands on either.
                assert_eq!(side, (info.spec.0 != info.spec.1).then_some(left));
                assert_eq!(Delim::of_spec_side(spec, left), Some(d));
                assert!(!d.latex(left).is_empty());
            }
            // Wrong-side lookups reject: `]` cannot open a pair.
            if info.spec.0 != info.spec.1 {
                assert_eq!(Delim::of_spec_side(info.spec.1, true), None);
                assert_eq!(Delim::of_spec_side(info.spec.0, false), None);
            }
            // The one-line glyphs identify the pair on their own.
            for g in [info.short.0, info.short.1] {
                assert!(seen_short.insert(g), "{:?} is shared", g);
            }
            // Ceil/floor drop a corner by repeating the extension —
            // that is what tells the bracket families apart.
            if let Some(tall) = info.tall {
                for (t, e, b) in tall {
                    assert!(
                        t == e || b == e || (t != e && b != e),
                        "{:?} has an unreadable column",
                        info.spec
                    );
                }
            }
        }
        // Every \lr name token spells a spec some row owns.
        for (&name, &c) in DELIM_NAMES.entries() {
            assert!(Delim::of_spec(c).is_some(), "\\{} names no pair", name);
        }
    }
}
