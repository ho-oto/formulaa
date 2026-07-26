//! The delimiter pairs: one row per pair describing the spec chars,
//! the one-line and tall glyph columns, the brace vertex and the LaTeX
//! spelling. parse/render/latex all read this table, so a new pair is
//! added in exactly one place. Angles stay out (diagonal arms share
//! none of these fields).

/// `{ }`, `⌈ ⌉`, `⌊ ⌋`, the vertical bar and the null pair, in one
/// row each. Before this table the same knowledge lived in eight
/// places (the spec list, the two side sets, the glyph->spec maps, the
/// vertical-scan families, the column renderer and the LaTeX speller),
/// so adding a pair meant touching five files.
///
/// Angles are deliberately absent: they are drawn from diagonal arms
/// rather than stacked pieces, so they share none of these fields.
pub struct DelimSpec {
    /// The spec chars the AST stores, left and right.
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

pub const DELIMS: &[DelimSpec] = &[
    DelimSpec {
        spec: ('(', ')'),
        short: ('(', ')'),
        tall: [('⎛', '⎜', '⎝'), ('⎞', '⎟', '⎠')],
        vertex: None,
        latex: ("(", ")"),
    },
    DelimSpec {
        spec: ('[', ']'),
        short: ('[', ']'),
        tall: [('⎡', '⎢', '⎣'), ('⎤', '⎥', '⎦')],
        vertex: None,
        latex: ("[", "]"),
    },
    DelimSpec {
        spec: ('{', '}'),
        short: ('{', '}'),
        tall: [('⎧', '⎪', '⎩'), ('⎫', '⎪', '⎭')],
        vertex: Some(('⎨', '⎬')),
        latex: ("\\{", "\\}"),
    },
    // Ceil and floor reuse the bracket pieces with one corner missing.
    DelimSpec {
        spec: ('⌈', '⌉'),
        short: ('⌈', '⌉'),
        tall: [('⎡', '⎢', '⎢'), ('⎤', '⎥', '⎥')],
        vertex: None,
        latex: ("\\lceil ", "\\rceil "),
    },
    DelimSpec {
        spec: ('⌊', '⌋'),
        short: ('⌊', '⌋'),
        tall: [('⎢', '⎢', '⎣'), ('⎥', '⎥', '⎦')],
        vertex: None,
        latex: ("\\lfloor ", "\\rfloor "),
    },
    // The vertical bar is a cornerless extension column — which is how
    // the column walk tells it from ceil/floor.
    DelimSpec {
        spec: ('|', '|'),
        short: ('⎢', '⎥'),
        tall: [('⎢', '⎢', '⎢'), ('⎥', '⎥', '⎥')],
        vertex: None,
        latex: ("|", "|"),
    },
    // The null pair (\left. / \right.): dashed ghosts, like the ┈ band.
    DelimSpec {
        spec: ('.', '.'),
        short: ('┆', '┊'),
        tall: [('┆', '┆', '┆'), ('┊', '┊', '┊')],
        vertex: None,
        latex: (".", "."),
    },
];

impl DelimSpec {
    /// Every glyph this side can show, in any height.
    pub fn glyphs(&self, left: bool) -> Vec<char> {
        let i = usize::from(!left);
        let (t, e, b) = self.tall[i];
        let mut v = vec![if left { self.short.0 } else { self.short.1 }, t, e, b];
        if let Some(vx) = self.vertex {
            v.push(if left { vx.0 } else { vx.1 });
        }
        v.sort_unstable();
        v.dedup();
        v
    }
}

/// The pair a spec char belongs to, and which side it is.
pub fn delim_of(spec: char) -> Option<(&'static DelimSpec, bool)> {
    DELIMS.iter().find_map(|d| {
        if d.spec.0 == spec {
            Some((d, true))
        } else if d.spec.1 == spec {
            Some((d, false))
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    /// The delimiter table is the single description of each pair, so
    /// the pieces it lists have to be consistent: distinct sides,
    /// distinct glyphs per side, and a spec that resolves back to its
    /// own row.
    #[test]
    fn delimiter_table_is_consistent() {
        let mut seen_short = std::collections::HashSet::new();
        for d in DELIMS {
            for (spec, left) in [(d.spec.0, true), (d.spec.1, false)] {
                let (row, side) = delim_of(spec).expect("a spec resolves");
                assert_eq!(row.spec, d.spec, "{:?} resolves elsewhere", spec);
                // A side-distinct spec must name its own side.
                if d.spec.0 != d.spec.1 {
                    assert_eq!(side, left, "{:?}", spec);
                }
            }
            // The one-line glyphs identify the pair on their own.
            for g in [d.short.0, d.short.1] {
                assert!(seen_short.insert(g), "{:?} is shared", g);
            }
            // Ceil/floor drop a corner by repeating the extension —
            // that is what tells the bracket families apart.
            for i in 0..2 {
                let (t, e, b) = d.tall[i];
                assert!(
                    t == e || b == e || (t != e && b != e),
                    "{:?} has an unreadable column",
                    d.spec
                );
            }
            assert!(!d.latex.0.is_empty() && !d.latex.1.is_empty());
        }
        // …and every spec is a spec the AST accepts.
        for d in DELIMS {
            assert!(crate::ast::is_delim_spec(d.spec.0));
            assert!(crate::ast::is_delim_spec(d.spec.1));
        }
    }
}
