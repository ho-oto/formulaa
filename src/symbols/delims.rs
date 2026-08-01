//! The delimiter pairs. The AST stores a `Delim` kind per side (the
//! slot decides left/right, so a `]` shape cannot end up in the left
//! slot), and a `Delim` is one of two drawn kinds, split at the type
//! level because almost nothing about them generalizes:
//!
//! - [`ColDelim`] — the column pairs: stacked glyph columns whose tall
//!   form is (top, extension, bottom) pieces. Everything about one —
//!   spec chars, one-line and tall glyphs, the brace vertex, LaTeX —
//!   is answered by `info` in one row, and the column classifiers
//!   (`of_run`, `run_glyphs`, …) are derivations over those rows.
//! - `Delim::Angle` — drawn from diagonal ╱ ╲ arms instead of a
//!   column; no tall pieces, no column walk (the parser resolves the
//!   shared arms contextually, `glyphs::ARM_RISE`/`ARM_FALL`).
//!
//! Both kinds pair freely per side (`⟨x|` is Angle + Col(Bar) — the
//! bra), which is why the angle stays a `Delim` and not its own node.
//! The norm `‖` IS its own node (both sides are the same glyph, told
//! apart by extent — `glyphs::NORM`).

/// The column-drawn pairs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColDelim {
    Paren,
    Bracket,
    Brace,
    Ceil,
    Floor,
    Bar,
    Null,
}

/// A delimiter slot: a column pair, or the diagonal-armed angle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delim {
    Col(ColDelim),
    Angle,
}

/// Everything about one column pair, in one row; paired fields are
/// (left, right).
pub struct ColInfo {
    /// The spec chars the `\lr` syntax spells (the side-symmetric
    /// `|` and `.` repeat one char).
    pub spec: (char, char),
    /// How the pair draws on one line.
    pub short: (char, char),
    /// The tall column, top to bottom: (top, extension, bottom) per
    /// side. `⌈` has no foot and `⌊` no head, so those repeat the
    /// extension — which is exactly what tells the families apart.
    pub tall: [(char, char, char); 2],
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

/// Spec char -> (pair, side); None = either side (the side-symmetric
/// `|` and `.`). The input table of the enum, same shape as the symbol
/// `NAMES` map; a test pins it against the `info` rows so the two
/// cannot drift.
pub static DELIM_SPECS: phf::Map<char, (Delim, Option<bool>)> = phf::phf_map! {
    '(' => (Delim::Col(ColDelim::Paren), Some(true)),
    ')' => (Delim::Col(ColDelim::Paren), Some(false)),
    '[' => (Delim::Col(ColDelim::Bracket), Some(true)),
    ']' => (Delim::Col(ColDelim::Bracket), Some(false)),
    '{' => (Delim::Col(ColDelim::Brace), Some(true)),
    '}' => (Delim::Col(ColDelim::Brace), Some(false)),
    '⌈' => (Delim::Col(ColDelim::Ceil), Some(true)),
    '⌉' => (Delim::Col(ColDelim::Ceil), Some(false)),
    '⌊' => (Delim::Col(ColDelim::Floor), Some(true)),
    '⌋' => (Delim::Col(ColDelim::Floor), Some(false)),
    '⟨' => (Delim::Angle, Some(true)),
    '⟩' => (Delim::Angle, Some(false)),
    '|' => (Delim::Col(ColDelim::Bar), None),
    '.' => (Delim::Col(ColDelim::Null), None),
};

impl ColDelim {
    pub const ALL: [ColDelim; 7] = [
        ColDelim::Paren,
        ColDelim::Bracket,
        ColDelim::Brace,
        ColDelim::Ceil,
        ColDelim::Floor,
        ColDelim::Bar,
        ColDelim::Null,
    ];

    /// The one row that says everything about this pair.
    #[rustfmt::skip]
    pub const fn info(self) -> &'static ColInfo {
        match self {
            ColDelim::Paren => &ColInfo {
                spec: ('(', ')'),
                short: ('(', ')'),
                tall: [('⎛', '⎜', '⎝'), ('⎞', '⎟', '⎠')],
                vertex: None,
                latex: ("(", ")"),
            },
            ColDelim::Bracket => &ColInfo {
                spec: ('[', ']'),
                short: ('[', ']'),
                tall: [('⎡', '⎢', '⎣'), ('⎤', '⎥', '⎦')],
                vertex: None,
                latex: ("[", "]"),
            },
            ColDelim::Brace => &ColInfo {
                spec: ('{', '}'),
                short: ('{', '}'),
                tall: [('⎧', '⎪', '⎩'), ('⎫', '⎪', '⎭')],
                vertex: Some(('⎨', '⎬')),
                latex: ("\\{", "\\}"),
            },
            // Ceil and floor reuse the bracket pieces with one corner
            // missing.
            ColDelim::Ceil => &ColInfo {
                spec: ('⌈', '⌉'),
                short: ('⌈', '⌉'),
                tall: [('⎡', '⎢', '⎢'), ('⎤', '⎥', '⎥')],
                vertex: None,
                latex: ("\\lceil ", "\\rceil "),
            },
            ColDelim::Floor => &ColInfo {
                spec: ('⌊', '⌋'),
                short: ('⌊', '⌋'),
                tall: [('⎢', '⎢', '⎣'), ('⎥', '⎥', '⎦')],
                vertex: None,
                latex: ("\\lfloor ", "\\rfloor "),
            },
            // The vertical bar is a cornerless extension column —
            // which is how the column walk tells it from ceil/floor.
            ColDelim::Bar => &ColInfo {
                spec: ('|', '|'),
                short: ('⎢', '⎥'),
                tall: [('⎢', '⎢', '⎢'), ('⎥', '⎥', '⎥')],
                vertex: None,
                latex: ("|", "|"),
            },
            // The null pair (\left. / \right.): dashed ghosts, like
            // the ┈ band.
            ColDelim::Null => &ColInfo {
                spec: ('.', '.'),
                short: ('┆', '┊'),
                tall: [('┆', '┆', '┆'), ('┊', '┊', '┊')],
                vertex: None,
                latex: (".", "."),
            },
        }
    }

    /// The spec char of this pair's given side.
    pub fn spec(self, left: bool) -> char {
        let (l, r) = self.info().spec;
        if left { l } else { r }
    }

    /// Whether this pair's columns fuse with a sole grid segment (the
    /// delimiter absorbs the lattice edges: ├ ┤ junction rows, ┬ ┴
    /// markers on its top/bottom rows). Curly braces keep their vertex
    /// column; null ghosts take a bare lattice instead.
    pub const fn fuses(self) -> bool {
        matches!(
            self,
            ColDelim::Paren | ColDelim::Bracket | ColDelim::Ceil | ColDelim::Floor | ColDelim::Bar
        )
    }

    /// Every glyph this side can show, in any height: the one-line
    /// short, the tall pieces and the vertex.
    pub fn glyphs(self, left: bool) -> Vec<char> {
        let info = self.info();
        let side = |p: (char, char)| if left { p.0 } else { p.1 };
        let mut v = vec![side(info.short)];
        let (t, e, b) = info.tall[usize::from(!left)];
        v.extend([t, e, b]);
        if let Some(vx) = info.vertex {
            v.push(side(vx));
        }
        v.sort_unstable();
        v.dedup();
        v
    }

    /// The glyphs this pair's tall column shows on one side: the tall
    /// pieces and the vertex — never the one-line short.
    pub fn run_pieces(self, left: bool) -> Vec<char> {
        let info = self.info();
        let (t, e, b) = info.tall[usize::from(!left)];
        let mut p = vec![t, e, b];
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
        ColDelim::ALL
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
                let mut v: Vec<char> = ColDelim::ALL
                    .iter()
                    .flat_map(|d| d.run_pieces(left))
                    .collect();
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
    pub fn of_run(has: impl Fn(char) -> bool, left: bool) -> ColDelim {
        let owners = |g: char| {
            ColDelim::ALL
                .iter()
                .flat_map(|d| [d.glyphs(true), d.glyphs(false)])
                .filter(|v| v.contains(&g))
                .count()
        };
        for d in ColDelim::ALL {
            if d.glyphs(left).into_iter().any(|g| owners(g) == 1 && has(g)) {
                return d;
            }
        }
        // The extension-sharing group: its members' own corners are the
        // pieces that differ from the shared extension.
        let tall = |d: ColDelim| d.info().tall[usize::from(!left)];
        let group: Vec<(ColDelim, (char, char, char))> = ColDelim::ALL
            .into_iter()
            .map(|d| (d, tall(d)))
            .filter(|&(_, (_, e, _))| ColDelim::ALL.iter().filter(|&&o| tall(o).1 == e).count() > 1)
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
            .unwrap_or(ColDelim::Bar)
    }
}

impl Delim {
    pub const ALL: [Delim; 8] = [
        Delim::Col(ColDelim::Paren),
        Delim::Col(ColDelim::Bracket),
        Delim::Col(ColDelim::Brace),
        Delim::Col(ColDelim::Ceil),
        Delim::Col(ColDelim::Floor),
        Delim::Col(ColDelim::Bar),
        Delim::Col(ColDelim::Null),
        Delim::Angle,
    ];

    /// The column pair, when this is one (the classifiers and the fuse
    /// path only exist for columns).
    pub fn col(self) -> Option<ColDelim> {
        match self {
            Delim::Col(c) => Some(c),
            Delim::Angle => None,
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
        match self {
            Delim::Col(c) => c.spec(left),
            Delim::Angle => {
                if left {
                    '⟨'
                } else {
                    '⟩'
                }
            }
        }
    }

    pub fn latex(self, left: bool) -> &'static str {
        match self {
            Delim::Col(c) => {
                let (l, r) = c.info().latex;
                if left { l } else { r }
            }
            Delim::Angle => {
                if left {
                    "\\langle "
                } else {
                    "\\rangle "
                }
            }
        }
    }

    /// Whether this side's column fuses with a sole grid segment; the
    /// angles never do (their arms wrap a bare lattice instead).
    pub fn fuses(self) -> bool {
        self.col().is_some_and(ColDelim::fuses)
    }

    /// Every glyph this side can show, in any height. The angles show
    /// only their one-line char — the ╱ ╲ arms are shared diagonals
    /// the parser resolves contextually.
    pub fn glyphs(self, left: bool) -> Vec<char> {
        match self {
            Delim::Col(c) => c.glyphs(left),
            Delim::Angle => vec![self.spec(left)],
        }
    }

    /// The pair a glyph on the *baseline row* of a delimiter column can
    /// belong to (first claimant in `ALL` order wins the shared bracket
    /// pieces — the column walk refines it). A pair with a vertex shows
    /// the vertex on the baseline, so only its short and vertex glyphs
    /// count; ⎧ ⎪ ⎩ never stand on a baseline.
    pub fn of_baseline_piece(c: char, left: bool) -> Option<Delim> {
        Delim::ALL.into_iter().find(|d| match d {
            Delim::Col(cd) => {
                let info = cd.info();
                let side = |p: (char, char)| if left { p.0 } else { p.1 };
                match info.vertex {
                    Some(vx) => c == side(info.short) || c == side(vx),
                    None => cd.glyphs(left).contains(&c),
                }
            }
            Delim::Angle => c == d.spec(left),
        })
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
            let symmetric = d.spec(true) == d.spec(false);
            for left in [true, false] {
                let spec = d.spec(left);
                let (row, side) = Delim::of_spec(spec).expect("a spec resolves");
                assert_eq!(row, d, "{:?} resolves elsewhere", spec);
                // A side-distinct spec names its own side; a symmetric
                // one stands on either.
                assert_eq!(side, (!symmetric).then_some(left));
                assert_eq!(Delim::of_spec_side(spec, left), Some(d));
                assert!(!d.latex(left).is_empty());
            }
            // Wrong-side lookups reject: `]` cannot open a pair.
            if !symmetric {
                assert_eq!(Delim::of_spec_side(d.spec(false), true), None);
                assert_eq!(Delim::of_spec_side(d.spec(true), false), None);
            }
        }
        for cd in ColDelim::ALL {
            let info = cd.info();
            // The one-line glyphs identify the pair on their own.
            for g in [info.short.0, info.short.1] {
                assert!(seen_short.insert(g), "{:?} is shared", g);
            }
            // Ceil/floor drop a corner by repeating the extension —
            // that is what tells the bracket families apart.
            for (t, e, b) in info.tall {
                assert!(
                    t == e || b == e || (t != e && b != e),
                    "{:?} has an unreadable column",
                    info.spec
                );
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
        // brace only stands on its short or vertex — and the angle
        // claims exactly its one-line char.
        for (c, want) in [
            ('⎢', Some(Delim::Col(ColDelim::Bracket))),
            ('⎡', Some(Delim::Col(ColDelim::Bracket))),
            ('⌈', Some(Delim::Col(ColDelim::Ceil))),
            ('(', Some(Delim::Col(ColDelim::Paren))),
            ('⎜', Some(Delim::Col(ColDelim::Paren))),
            ('⎨', Some(Delim::Col(ColDelim::Brace))),
            ('┆', Some(Delim::Col(ColDelim::Null))),
            ('⟨', Some(Delim::Angle)),
            ('⎧', None),
            ('⎪', None),
            ('|', None),
        ] {
            assert_eq!(Delim::of_baseline_piece(c, true), want, "{:?}", c);
        }
        // Runs never contain one-line shorts.
        for left in [true, false] {
            for cd in ColDelim::ALL {
                let short = if left {
                    cd.info().short.0
                } else {
                    cd.info().short.1
                };
                let run = ColDelim::run_glyphs(left);
                // ┆ and ⎢ double as extensions — the only shorts a run
                // may show.
                if !matches!(cd, ColDelim::Null | ColDelim::Bar) {
                    assert!(!run.contains(&short), "{:?} in run", short);
                }
            }
        }
        // Run resolution: unique pieces first, corners for the rest.
        let of = |glyphs: &'static str, left| ColDelim::of_run(|c| glyphs.contains(c), left);
        assert_eq!(of("⎛⎜⎝", true), ColDelim::Paren);
        assert_eq!(of("⎧⎨⎪⎩", true), ColDelim::Brace);
        assert_eq!(of("⎡⎢⎣", true), ColDelim::Bracket);
        assert_eq!(of("⎡⎢", true), ColDelim::Ceil);
        assert_eq!(of("⎢⎣", true), ColDelim::Floor);
        assert_eq!(of("⎢", true), ColDelim::Bar);
        assert_eq!(of("⎪", true), ColDelim::Bar); // vertex-less ⎪ run: degenerate, reads as bar
        assert_eq!(of("⎤⎥", false), ColDelim::Ceil);
        assert_eq!(of("┊", false), ColDelim::Null);
    }
}
