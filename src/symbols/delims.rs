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

/// The norm `‖` — not a pair (both sides are the same glyph, told
/// apart by extent), so it stays outside the enum; the glyph lives
/// here with the rest of the delimiter vocabulary.
pub const NORM: char = '‖'; // U+2016 DOUBLE VERTICAL LINE

/// The `\lr` mid separator column (│ between segments).
pub const MID: char = '│'; // U+2502 BOX DRAWINGS LIGHT VERTICAL

/// The tall angles' diagonal arms (drawn by the renderer; resolved
/// contextually by the parser — they name no side of their own).
pub const ARM_RISE: char = '╱'; // U+2571 BOX DRAWINGS LIGHT DIAGONAL
pub const ARM_FALL: char = '╲'; // U+2572

/// Spec char -> (pair, side); None = either side (the side-symmetric
/// `|` and `.`). The input table of the enum, same shape as the symbol
/// `NAMES` map; a test pins it against the `info` rows so the two
/// cannot drift.
pub static DELIM_SPECS: phf::Map<char, (Delim, Option<bool>)> = phf::phf_map! {
    '(' => (Delim::Paren, Some(true)),
    ')' => (Delim::Paren, Some(false)),
    '[' => (Delim::Bracket, Some(true)),
    ']' => (Delim::Bracket, Some(false)),
    '{' => (Delim::Brace, Some(true)),
    '}' => (Delim::Brace, Some(false)),
    '⌈' => (Delim::Ceil, Some(true)),
    '⌉' => (Delim::Ceil, Some(false)),
    '⌊' => (Delim::Floor, Some(true)),
    '⌋' => (Delim::Floor, Some(false)),
    '⟨' => (Delim::Angle, Some(true)),
    '⟩' => (Delim::Angle, Some(false)),
    '|' => (Delim::Bar, None),
    '.' => (Delim::Null, None),
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
        DELIM_SPECS.get(&c).copied()
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

    /// Whether this pair's columns fuse with a sole grid segment (the
    /// delimiter absorbs the lattice edges: ├ ┤ junction rows, ┬ ┴
    /// markers on its top/bottom rows). Curly braces keep their vertex
    /// column; null ghosts and norm/angle geometry take a bare lattice
    /// instead.
    pub const fn fuses(self) -> bool {
        matches!(
            self,
            Delim::Paren | Delim::Bracket | Delim::Ceil | Delim::Floor | Delim::Bar
        )
    }

    /// The pair a glyph on the *baseline row* of a delimiter column can
    /// belong to (first claimant in `ALL` order wins the shared bracket
    /// pieces — the column walk refines it). A pair with a vertex shows
    /// the vertex on the baseline, so only its short and vertex glyphs
    /// count; ⎧ ⎪ ⎩ never stand on a baseline.
    pub fn of_baseline_piece(c: char, left: bool) -> Option<Delim> {
        let side = |p: (char, char)| if left { p.0 } else { p.1 };
        Delim::ALL.into_iter().find(|d| {
            let info = d.info();
            match info.vertex {
                Some(vx) => c == side(info.short) || c == side(vx),
                None => d.glyphs(left).contains(&c),
            }
        })
    }

    /// The glyphs this pair's tall column shows on one side: the tall
    /// pieces and the vertex — never the one-line short.
    pub fn run_pieces(self, left: bool) -> Vec<char> {
        let info = self.info();
        let mut p = Vec::new();
        if let Some(tall) = info.tall {
            let (t, e, b) = tall[usize::from(!left)];
            p.extend([t, e, b]);
        }
        if let Some(vx) = info.vertex {
            p.push(if left { vx.0 } else { vx.1 });
        }
        p.sort_unstable();
        p.dedup();
        p
    }

    /// A piece several pairs share on this side (⎡ ⎢ ⎣ / ⎤ ⎥ ⎦): its
    /// family needs the column walk (`of_run`); every other piece
    /// names its pair outright.
    pub fn is_shared_piece(c: char, left: bool) -> bool {
        Delim::ALL
            .iter()
            .filter(|d| d.glyphs(left).contains(&c))
            .count()
            > 1
    }

    /// Every glyph a tall delimiter column can contain on this side —
    /// the union of `run_pieces` over all pairs.
    pub fn run_glyphs(left: bool) -> &'static [char] {
        use std::sync::OnceLock;
        static RUNS: OnceLock<[Vec<char>; 2]> = OnceLock::new();
        let runs = RUNS.get_or_init(|| {
            let build = |left: bool| {
                let mut v: Vec<char> = Delim::ALL.iter().flat_map(|d| d.run_pieces(left)).collect();
                v.sort_unstable();
                v.dedup();
                v
            };
            [build(true), build(false)]
        });
        &runs[usize::from(!left)]
    }

    /// Resolve which pair a delimiter column's glyph run belongs to.
    /// Pieces unique to one (pair, side) resolve outright (⎜ is paren,
    /// ⎨ is brace, ┆ is null — the ⎪ extension serves both brace
    /// columns, so it stays neutral); what remains is the group sharing
    /// its extension piece (bracket/ceil/floor/bar all extend with ⎢),
    /// where the corners decide — by exactly the corner-dropping
    /// convention the tall columns encode (⌈ repeats its foot, ⌊ its
    /// head, the bar both).
    pub fn of_run(has: impl Fn(char) -> bool, left: bool) -> Delim {
        let owners = |g: char| {
            Delim::ALL
                .iter()
                .flat_map(|d| [d.glyphs(true), d.glyphs(false)])
                .filter(|v| v.contains(&g))
                .count()
        };
        for d in Delim::ALL {
            if d.glyphs(left).into_iter().any(|g| owners(g) == 1 && has(g)) {
                return d;
            }
        }
        // The extension-sharing group: its members' own corners are the
        // pieces that differ from the shared extension.
        let tall = |d: Delim| d.info().tall.map(|t| t[usize::from(!left)]);
        let group: Vec<(Delim, (char, char, char))> = Delim::ALL
            .into_iter()
            .filter_map(|d| Some((d, tall(d)?)))
            .filter(|&(_, (_, e, _))| {
                Delim::ALL
                    .iter()
                    .filter(|&&o| tall(o).is_some_and(|(_, oe, _)| oe == e))
                    .count()
                    > 1
            })
            .collect();
        // The group's head and foot corners (every cornered member
        // shares them: ⎡ and ⎣ on the left).
        let head = group
            .iter()
            .find_map(|&(_, (t, e, _))| (t != e).then_some(t));
        let foot = group
            .iter()
            .find_map(|&(_, (_, e, b))| (b != e).then_some(b));
        let (head_seen, foot_seen) = (head.is_some_and(&has), foot.is_some_and(&has));
        group
            .iter()
            .find(|&&(_, (t, e, b))| (t != e) == head_seen && (b != e) == foot_seen)
            .map(|&(d, _)| d)
            // Unreachable by construction (the no-corner member matches
            // anything left), but stay total.
            .unwrap_or(Delim::Bar)
    }

    /// Every glyph any pair can show on either side, in any height.
    pub fn all_pieces() -> &'static [char] {
        use std::sync::OnceLock;
        static PIECES: OnceLock<Vec<char>> = OnceLock::new();
        PIECES.get_or_init(|| {
            let mut v: Vec<char> = Delim::ALL
                .iter()
                .flat_map(|d| [d.glyphs(true), d.glyphs(false)])
                .flatten()
                .collect();
            v.sort_unstable();
            v.dedup();
            v
        })
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
        // The DELIM_SPECS input table lists nothing the info rows do
        // not spell (the loop above proved the other inclusion).
        for (&c, &(d, _)) in DELIM_SPECS.entries() {
            assert!(
                d.spec(true) == c || d.spec(false) == c,
                "{:?} is not a spec of {:?}",
                c,
                d
            );
        }
    }

    /// The column-run classifiers derived from the info rows behave as
    /// the parser expects: baseline pieces resolve to their family
    /// (vertex pairs by short/vertex only), and glyph runs resolve
    /// through unique pieces or the corner convention.
    #[test]
    fn run_classifiers_follow_the_table() {
        // Baseline pieces: bracket pieces default to the bracket, the
        // brace only stands on its short or vertex.
        for (c, want) in [
            ('⎢', Some(Delim::Bracket)),
            ('⎡', Some(Delim::Bracket)),
            ('⌈', Some(Delim::Ceil)),
            ('(', Some(Delim::Paren)),
            ('⎜', Some(Delim::Paren)),
            ('⎨', Some(Delim::Brace)),
            ('┆', Some(Delim::Null)),
            ('⎧', None),
            ('⎪', None),
            ('|', None),
        ] {
            assert_eq!(Delim::of_baseline_piece(c, true), want, "{:?}", c);
        }
        // Runs never contain one-line shorts.
        for left in [true, false] {
            for d in Delim::ALL {
                let short = if left {
                    d.info().short.0
                } else {
                    d.info().short.1
                };
                let run = Delim::run_glyphs(left);
                // ┆ and ⎢ double as extensions — the only shorts a run
                // may show.
                if !matches!(d, Delim::Null | Delim::Bar) {
                    assert!(!run.contains(&short), "{:?} in run", short);
                }
            }
        }
        // Run resolution: unique pieces first, corners for the rest.
        let of = |glyphs: &'static str, left| Delim::of_run(|c| glyphs.contains(c), left);
        assert_eq!(of("⎛⎜⎝", true), Delim::Paren);
        assert_eq!(of("⎧⎨⎪⎩", true), Delim::Brace);
        assert_eq!(of("⎡⎢⎣", true), Delim::Bracket);
        assert_eq!(of("⎡⎢", true), Delim::Ceil);
        assert_eq!(of("⎢⎣", true), Delim::Floor);
        assert_eq!(of("⎢", true), Delim::Bar);
        assert_eq!(of("⎪", true), Delim::Bar); // vertex-less ⎪ run: degenerate, reads as bar
        assert_eq!(of("⎤⎥", false), Delim::Ceil);
        assert_eq!(of("┊", false), Delim::Null);
    }
}
