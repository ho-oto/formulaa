//! Math AST. A formula is a `Row` (horizontal sequence of nodes); every
//! editable slot in a structure node is itself a `Row`, so the cursor can
//! always be described as (path into nested rows, column in that row).

pub type Row = Vec<Node>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
    /// A single atom: letter, digit, operator or Unicode math symbol.
    /// Stored as the plain (ASCII where possible) character; styling such as
    /// math-italic letters is applied only at render time.
    Sym(char),
    /// Formatting space (the Space key): renders as one blank column in
    /// the AA, produces no LaTeX output, and vanishes on reparse —
    /// the roundtrip contract is parse∘render == strip_spacers∘normalize.
    /// (Use the ␣ atom, \space, for a *semantic* space.)
    Spacer,
    /// Line break of a multi-line formula (top-level row only). Renders
    /// as a vertical stack: the continuation line starts with the `┈ `
    /// marker at its baseline (a lone ┈ has no other reading — a band
    /// always sandwiches its pieces without spaces). LaTeX: `\\`,
    /// Typst: `\ `.
    Break,
    /// Named function/operator rendered upright (sin, cos, log, ...).
    /// Any upright multi-letter name — a dictionary word (`sin`) or an
    /// ad-hoc one (`vol`); the dictionary only decides whether the name
    /// takes limits and how the ASCII run lexer splits it.
    Func(String),
    /// Upright run. `math: true` is \mathrm — drawn bare when the
    /// picture reads back unambiguously: >= 2 ASCII letters and not a
    /// dictionary word, or a single letter glued to an alphabetic
    /// neighbour (d𝑦, the roman differential); otherwise
    /// 'single-quoted'. The quotes appear/disappear automatically as
    /// the context changes. `math: false` is \text, always
    /// "double-quoted". Interior spaces are drawn as ␣ inside quotes.
    /// `\text{…}`: prose inside a formula, drawn in "double quotes"
    /// (a literal `"` or `\` inside is escaped with a backslash).
    Text(String),
    /// A single upright letter — the differential `d` of `d𝑥`
    /// (`\mathrm{d}`). Longer upright runs are `Func` instead, so the
    /// two never overlap.
    Roman(char),
    /// Accented base character (x̂ ẋ v̄ a⃗ …): over-marks stack upward and
    /// under-marks downward in the cells directly above/below the base,
    /// innermost first. Flat lists (not nesting) are deliberate: the
    /// picture cannot distinguish \hat{\underline{x}} from
    /// \underline{\hat{x}}, so the AST must not either.
    Accent {
        overs: Vec<char>,
        unders: Vec<char>,
        base: char,
    },
    /// Stretchy accent over/under a multi-character base: a ┈band┈
    /// whose limit region holds nothing but the mark character —
    /// reserved marks cannot be atoms, so a marks-only limit row is
    /// unambiguous (\widehat{abc}, \overline{xy}, \underline{...}).
    /// The base is a flat one-line row like a BigOp base and is not
    /// cursor-editable (wrap a selection; re-edit by deleting).
    /// A one-char base with a single over mark normalizes to Accent.
    WideAccent {
        over: Option<char>,
        under: Option<char>,
        base: Row,
    },
    Frac {
        num: Row,
        den: Row,
    },
    /// index 2 = √, 3 = ∛, 4 = ∜ (the only radical glyphs Unicode has).
    Sqrt {
        arg: Row,
        index: Radical,
    },
    Sup {
        arg: Row,
    },
    Sub {
        arg: Row,
    },
    /// A ∑-class symbol with under/over limits: the atom sandwiched in
    /// ┈ (`┈∑┈`). With both limits empty it normalizes to the bare atom
    /// — the picture `∑` reads back as `Sym`, and ↑/↓ lifts it again.
    BigOpSym {
        op: char,
        lower: Row,
        upper: Row,
    },
    /// A named operator with under/over limits (`┈lim┈`, `┈argmax┈`).
    /// The name is one piece: no blanks, so the bare picture reads back
    /// as exactly one `Func` and the two forms correspond one to one.
    /// With both limits empty it normalizes to that `Func`.
    BigOp {
        name: String,
        lower: Row,
        upper: Row,
    },
    /// `‖x‖`: both sides are the same glyph, so the parser tells the
    /// pair apart by column extent rather than by the side-distinct
    /// glyphs a `Delim` relies on — its own node keeps that exception
    /// out of the general delimiter machinery. Its one slot is
    /// `Field::Seg(0)`, so cursor paths look like a one-segment Delim.
    Norm {
        arg: Row,
    },
    /// Horizontal brace over/under its argument (\overbrace/\underbrace):
    /// a ╭──╮ / ╰──╯ row hugging the argument block, with an optional
    /// label beyond it. Same range-row idea as Frac, anchored off-baseline.
    Brace {
        over: bool,
        arg: Row,
        label: Row,
    },
    /// Stretchy labeled arrow (\xrightarrow / \xleftarrow and the ⇒/⇐
    /// doubles): a ─ (or ═) body with an ASCII head (< or >) at the
    /// pointing end, labels over/under spanning its extent (same
    /// range-band idea as ┈). `op` is → ← ⇒ or ⇐.
    Arrow {
        op: char,
        over: Row,
        under: Row,
    },
    /// Auto-scaling delimiter block. `left`/`right`/`mids` hold delimiter
    /// *spec* chars: ( ) [ ] { } ⟨ ⟩ | and '.' (null delimiter, drawn as
    /// the dashed ┆ ┊ ghosts). Middles ('|' only) separate the segments;
    /// segs.len() == mids.len() + 1. Segments are ordinary rows — a matrix
    /// is nothing more than a Delim whose segment contains an Array.
    Delim {
        left: char,
        right: char,
        mids: Vec<char>,
        segs: Vec<Row>,
    },
    /// rows×cols grid (LaTeX array/matrix), cells stored row-major.
    /// Always drawn as a self-delimiting lattice (┌ ┬ ┐ / ├ ┼ ┤ / └ ┴ ┘
    /// junctions at every separator crossing including the outer edges),
    /// wherever it appears — delimiters simply wrap it.
    Array {
        rows: usize,
        cols: usize,
        cells: Vec<Row>,
    },
    /// Struck-through content (\cancel): every cell of the rendered
    /// argument carries a combining long solidus overlay (U+0338).
    Cancel {
        arg: Row,
    },
}

/// Valid delimiter spec chars for `Node::Delim` (`.` = null delimiter,
/// `‖` = the double-bar norm — same spec char on both sides).
/// Which root sign a `Sqrt` draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Radical {
    /// √ — the square root, drawn without an index.
    Sqrt,
    /// ∛
    Cbrt,
    /// ∜
    Qdrt,
}

impl Radical {
    pub const ALL: [Radical; 3] = [Radical::Sqrt, Radical::Cbrt, Radical::Qdrt];

    /// The root glyph, which is also how the picture spells the index.
    pub fn glyph(self) -> char {
        match self {
            Radical::Sqrt => '√',
            Radical::Cbrt => '∛',
            Radical::Qdrt => '∜',
        }
    }

    pub fn of_glyph(c: char) -> Option<Radical> {
        Radical::ALL.into_iter().find(|r| r.glyph() == c)
    }

    /// The LaTeX index (`\sqrt[3]`); the square root has none.
    pub fn latex_index(self) -> Option<u8> {
        match self {
            Radical::Sqrt => None,
            Radical::Cbrt => Some(3),
            Radical::Qdrt => Some(4),
        }
    }
}

pub const DELIM_SPECS: &[char] = &[
    '(', ')', '[', ']', '{', '}', '⟨', '⟩', '|', '.', '⌈', '⌉', '⌊', '⌋',
];

/// Identifies one editable slot inside a structure node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    FracNum,
    FracDen,
    SqrtArg,
    SupArg,
    SubArg,
    OpLower,
    OpUpper,
    ArrowOver,
    ArrowUnder,
    BraceArg,
    BraceLabel,
    /// Segment index of a Delim.
    Seg(usize),
    CancelArg,
    /// Row-major cell index of an Array.
    Cell(usize),
}

impl Node {
    /// Editable fields in cursor-traversal order (empty for atoms).
    pub fn fields(&self) -> Vec<Field> {
        match self {
            Node::Sym(_)
            | Node::Spacer
            | Node::Break
            | Node::Func(_)
            | Node::Text(_)
            | Node::Roman(_)
            | Node::Accent { .. }
            | Node::WideAccent { .. } => {
                vec![]
            }
            Node::Frac { .. } => vec![Field::FracNum, Field::FracDen],
            Node::Sqrt { .. } => vec![Field::SqrtArg],
            Node::Sup { .. } => vec![Field::SupArg],
            Node::Sub { .. } => vec![Field::SubArg],
            Node::BigOp { .. } | Node::BigOpSym { .. } => {
                vec![Field::OpLower, Field::OpUpper]
            }
            Node::Arrow { .. } => vec![Field::ArrowOver, Field::ArrowUnder],
            Node::Brace { .. } => vec![Field::BraceArg, Field::BraceLabel],
            Node::Delim { segs, .. } => (0..segs.len()).map(Field::Seg).collect(),
            Node::Norm { .. } => vec![Field::Seg(0)],
            Node::Cancel { .. } => vec![Field::CancelArg],
            Node::Array { cells, .. } => (0..cells.len()).map(Field::Cell).collect(),
        }
    }

    pub fn field(&self, f: Field) -> &Row {
        match (self, f) {
            (Node::Frac { num, .. }, Field::FracNum) => num,
            (Node::Frac { den, .. }, Field::FracDen) => den,
            (Node::Sqrt { arg, .. }, Field::SqrtArg) => arg,
            (Node::Sup { arg }, Field::SupArg) => arg,
            (Node::Sub { arg }, Field::SubArg) => arg,
            (Node::BigOp { lower, .. } | Node::BigOpSym { lower, .. }, Field::OpLower) => lower,
            (Node::BigOp { upper, .. } | Node::BigOpSym { upper, .. }, Field::OpUpper) => upper,
            (Node::Arrow { over, .. }, Field::ArrowOver) => over,
            (Node::Arrow { under, .. }, Field::ArrowUnder) => under,
            (Node::Brace { arg, .. }, Field::BraceArg) => arg,
            (Node::Brace { label, .. }, Field::BraceLabel) => label,
            (Node::Delim { segs, .. }, Field::Seg(i)) => &segs[i],
            (Node::Cancel { arg }, Field::CancelArg) => arg,
            (Node::Array { cells, .. }, Field::Cell(i)) => &cells[i],
            _ => panic!("field {:?} does not belong to node {:?}", f, self),
        }
    }

    pub fn field_mut(&mut self, f: Field) -> &mut Row {
        match (self, f) {
            (Node::Frac { num, .. }, Field::FracNum) => num,
            (Node::Frac { den, .. }, Field::FracDen) => den,
            (Node::Sqrt { arg, .. }, Field::SqrtArg) => arg,
            (Node::Sup { arg }, Field::SupArg) => arg,
            (Node::Sub { arg }, Field::SubArg) => arg,
            (Node::BigOp { lower, .. } | Node::BigOpSym { lower, .. }, Field::OpLower) => lower,
            (Node::BigOp { upper, .. } | Node::BigOpSym { upper, .. }, Field::OpUpper) => upper,
            (Node::Arrow { over, .. }, Field::ArrowOver) => over,
            (Node::Arrow { under, .. }, Field::ArrowUnder) => under,
            (Node::Brace { arg, .. }, Field::BraceArg) => arg,
            (Node::Brace { label, .. }, Field::BraceLabel) => label,
            (Node::Delim { segs, .. }, Field::Seg(i)) => &mut segs[i],
            (Node::Cancel { arg }, Field::CancelArg) => arg,
            (Node::Array { cells, .. }, Field::Cell(i)) => &mut cells[i],
            (node, f) => panic!("field {:?} does not belong to node {:?}", f, node),
        }
    }

    pub fn is_empty_structure(&self) -> bool {
        let fields = self.fields();
        !fields.is_empty() && fields.iter().all(|&f| self.field(f).is_empty())
    }
}

/// Walk `path` down from `root` and return the row the cursor lives in.
pub fn row_at<'a>(root: &'a Row, path: &[(usize, Field)]) -> &'a Row {
    let mut row = root;
    for &(i, f) in path {
        row = row[i].field(f);
    }
    row
}

pub fn row_at_mut<'a>(root: &'a mut Row, path: &[(usize, Field)]) -> &'a mut Row {
    let mut row = root;
    for &(i, f) in path {
        row = row[i].field_mut(f);
    }
    row
}

/// Canonical form: merge adjacent same-kind scripts (x^{a}^{b} == x^{ab} in
/// the picture, so the parser can only ever return the merged form).
/// `parse(render(x)) == normalize(x)` is the roundtrip invariant.
pub fn normalize(row: &Row) -> Row {
    // A band with no limits collapses to its bare base — but only when
    // the editor can lift that base back into a band (∑-class atoms and
    // lim-class functions, via ↑/↓). An \op* name (Text piece or a
    // multi-piece base) has no such way back, so its band survives
    // empty limits: ┈ess┈sup┈ stays a band.
    let mut pre: Row = Vec::with_capacity(row.len());
    for node in row {
        match normalize_node(node) {
            // An empty-limit band is its bare form: ┈∑┈ -> ∑ and
            // ┈lim┈ -> lim, one to one in both directions (↑/↓ lifts
            // them back, and the editor keeps the band in its own tree
            // while the cursor is inside).
            Node::BigOpSym { op, lower, upper } if lower.is_empty() && upper.is_empty() => {
                pre.push(Node::Sym(op))
            }
            Node::BigOp { name, lower, upper } if lower.is_empty() && upper.is_empty() => {
                pre.push(Node::Func(name))
            }
            // A markless wide accent is just its base.
            Node::WideAccent {
                over: None,
                under: None,
                base,
            } => pre.extend(base),
            // A one-letter upright run is Roman: its picture ('d' or a
            // glued d𝑥) reads back that way, so Func stays 2+ letters.
            Node::Func(t) if t.chars().count() == 1 => {
                pre.push(Node::Roman(t.chars().next().unwrap()))
            }
            n => pre.push(n),
        }
    }
    let mut out: Row = Vec::with_capacity(pre.len());
    for node in pre {
        // Leading spacers are dropped: they would rob a row-initial script
        // of its explicit ⬚ base (and trim eats them anyway). A Break
        // starts a new line, so the same applies at every line edge —
        // without this a line-initial script chunk sits on a spacer base
        // and its baseline row renders fully blank (unparseable).
        if out.is_empty() && matches!(node, Node::Spacer | Node::Break) {
            continue;
        }
        if matches!(node, Node::Spacer) && matches!(out.last(), Some(Node::Break)) {
            continue;
        }
        if matches!(node, Node::Break) {
            while matches!(out.last(), Some(Node::Spacer)) {
                out.pop();
            }
            if out.is_empty() {
                continue;
            }
        }
        // Empty scripts/cancels do not exist in normal form: their lone ⬚
        // placeholder fuses with neighbouring script blocks in the picture,
        // so the renderer must never produce one.
        match &node {
            Node::Sup { arg } | Node::Sub { arg } | Node::Cancel { arg } if arg.is_empty() => {
                continue;
            }
            // An empty text run has no picture of its own worth keeping.
            Node::Text(t) | Node::Func(t) if t.is_empty() => continue,
            _ => {}
        }
        // Scripts and cancels merge *across* spacers: the blank column a
        // spacer renders is internal to the script run in the picture, so
        // the parser reads one merged node — canonical form matches (the
        // spacers die in the merge, and are restored when no merge fires).
        let mut tail: Row = Vec::new();
        if matches!(
            node,
            Node::Sup { .. } | Node::Sub { .. } | Node::Cancel { .. }
        ) {
            while matches!(out.last(), Some(Node::Spacer)) {
                tail.push(out.pop().unwrap());
            }
        }
        match (out.last_mut(), &node) {
            // Re-normalize after merging: the concatenation can create new
            // same-kind adjacencies inside the argument (idempotence).
            (Some(Node::Sup { arg: a }), Node::Sup { arg: b }) => {
                a.extend(b.clone());
                *a = normalize(a);
            }
            (Some(Node::Sub { arg: a }), Node::Sub { arg: b }) => {
                a.extend(b.clone());
                *a = normalize(a);
            }
            // Adjacent struck-through blocks are one picture.
            (Some(Node::Cancel { arg: a }), Node::Cancel { arg: b }) => {
                a.extend(b.clone());
                *a = normalize(a);
            }
            // "Maximal cancel": a fully struck script right after a cancel
            // is the same picture as the script living inside it.
            (Some(Node::Cancel { arg: a }), Node::Sup { arg: b })
                if matches!(b[..], [Node::Cancel { .. }]) =>
            {
                let Node::Cancel { arg: inner } = &b[0] else {
                    unreachable!()
                };
                a.push(Node::Sup { arg: inner.clone() });
                *a = normalize(a);
            }
            (Some(Node::Cancel { arg: a }), Node::Sub { arg: b })
                if matches!(b[..], [Node::Cancel { .. }]) =>
            {
                let Node::Cancel { arg: inner } = &b[0] else {
                    unreachable!()
                };
                a.push(Node::Sub { arg: inner.clone() });
                *a = normalize(a);
            }
            _ => {
                out.extend(tail);
                out.push(node);
            }
        }
    }
    // Trailing spacers pad nothing visible (and a trailing one inside a
    // \cancel argument would meet the ragged-cancel spacer and fake a cell
    // gap); spacers are only meaningful *between* siblings.
    while matches!(out.last(), Some(Node::Spacer | Node::Break)) {
        out.pop();
    }
    out
}

/// Remove every Cancel wrapper in the subtree, splicing its contents.
fn strip_cancels(row: &Row) -> Row {
    let mut out: Row = Vec::new();
    for n in row {
        match n {
            Node::Cancel { arg } => out.extend(strip_cancels(arg)),
            Node::Sym(_)
            | Node::Spacer
            | Node::Break
            | Node::Func(_)
            | Node::Text(_)
            | Node::Roman(_)
            | Node::Accent { .. } => out.push(n.clone()),
            Node::WideAccent { over, under, base } => out.push(Node::WideAccent {
                over: *over,
                under: *under,
                base: strip_cancels(base),
            }),
            Node::Frac { num, den } => out.push(Node::Frac {
                num: strip_cancels(num),
                den: strip_cancels(den),
            }),
            Node::Sqrt { arg, index } => out.push(Node::Sqrt {
                arg: strip_cancels(arg),
                index: *index,
            }),
            Node::Sup { arg } => out.push(Node::Sup {
                arg: strip_cancels(arg),
            }),
            Node::Sub { arg } => out.push(Node::Sub {
                arg: strip_cancels(arg),
            }),
            Node::BigOp { name, lower, upper } => out.push(Node::BigOp {
                name: name.clone(),
                lower: strip_cancels(lower),
                upper: strip_cancels(upper),
            }),
            Node::BigOpSym { op, lower, upper } => out.push(Node::BigOpSym {
                op: *op,
                lower: strip_cancels(lower),
                upper: strip_cancels(upper),
            }),
            Node::Arrow { op, over, under } => out.push(Node::Arrow {
                op: *op,
                over: strip_cancels(over),
                under: strip_cancels(under),
            }),
            Node::Brace { over, arg, label } => out.push(Node::Brace {
                over: *over,
                arg: strip_cancels(arg),
                label: strip_cancels(label),
            }),
            Node::Norm { arg } => out.push(Node::Norm {
                arg: strip_cancels(arg),
            }),
            Node::Delim {
                left,
                right,
                mids,
                segs,
            } => out.push(Node::Delim {
                left: *left,
                right: *right,
                mids: mids.clone(),
                segs: segs.iter().map(strip_cancels).collect(),
            }),
            Node::Array { rows, cols, cells } => out.push(Node::Array {
                rows: *rows,
                cols: *cols,
                cells: cells.iter().map(strip_cancels).collect(),
            }),
        }
    }
    out
}

/// Remove every formatting `Spacer` in the subtree — exactly what the
/// parser cannot see (blank columns are structural). The roundtrip
/// contract is `parse(render(normalize(x))) == normalize(strip_spacers(normalize(x)))`.
pub fn strip_spacers(row: &Row) -> Row {
    let mut out: Row = Vec::new();
    for n in row {
        match n {
            Node::Spacer => {}
            Node::Sym(_)
            | Node::Break
            | Node::Func(_)
            | Node::Text(_)
            | Node::Roman(_)
            | Node::Accent { .. } => out.push(n.clone()),
            Node::WideAccent { over, under, base } => out.push(Node::WideAccent {
                over: *over,
                under: *under,
                base: strip_spacers(base),
            }),
            Node::Frac { num, den } => out.push(Node::Frac {
                num: strip_spacers(num),
                den: strip_spacers(den),
            }),
            Node::Sqrt { arg, index } => out.push(Node::Sqrt {
                arg: strip_spacers(arg),
                index: *index,
            }),
            Node::Sup { arg } => out.push(Node::Sup {
                arg: strip_spacers(arg),
            }),
            Node::Sub { arg } => out.push(Node::Sub {
                arg: strip_spacers(arg),
            }),
            Node::BigOp { name, lower, upper } => out.push(Node::BigOp {
                name: name.clone(),
                lower: strip_spacers(lower),
                upper: strip_spacers(upper),
            }),
            Node::BigOpSym { op, lower, upper } => out.push(Node::BigOpSym {
                op: *op,
                lower: strip_spacers(lower),
                upper: strip_spacers(upper),
            }),
            Node::Arrow { op, over, under } => out.push(Node::Arrow {
                op: *op,
                over: strip_spacers(over),
                under: strip_spacers(under),
            }),
            Node::Brace { over, arg, label } => out.push(Node::Brace {
                over: *over,
                arg: strip_spacers(arg),
                label: strip_spacers(label),
            }),
            Node::Norm { arg } => out.push(Node::Norm {
                arg: strip_spacers(arg),
            }),
            Node::Delim {
                left,
                right,
                mids,
                segs,
            } => out.push(Node::Delim {
                left: *left,
                right: *right,
                mids: mids.clone(),
                segs: segs.iter().map(strip_spacers).collect(),
            }),
            Node::Array { rows, cols, cells } => out.push(Node::Array {
                rows: *rows,
                cols: *cols,
                cells: cells.iter().map(strip_spacers).collect(),
            }),
            Node::Cancel { arg } => out.push(Node::Cancel {
                arg: strip_spacers(arg),
            }),
        }
    }
    out
}

fn normalize_node(node: &Node) -> Node {
    match node {
        // A markless accent is just its base.
        Node::Accent {
            overs,
            unders,
            base,
        } if overs.is_empty() && unders.is_empty() => {
            let _ = (overs, unders);
            Node::Sym(*base)
        }
        Node::Sym(_)
        | Node::Spacer
        | Node::Break
        | Node::Func(_)
        | Node::Text(_)
        | Node::Roman(_)
        | Node::Accent { .. } => node.clone(),
        Node::WideAccent { over, under, base } => {
            let base = normalize(base);
            // Markless: splice would need row context; degrade to the
            // bare base wrapped in nothing — callers splice via the
            // pre-pass below? Simplest: keep as a one-char Accent when
            // possible, else keep the node.
            match (&base[..], over, under) {
                // One-char base with a single over mark is the compact
                // stacked Accent (x̂) — one picture per formula.
                ([Node::Sym(c)], Some(m), None) => Node::Accent {
                    overs: vec![*m],
                    unders: vec![],
                    base: *c,
                },
                ([Node::Sym(c)], None, Some(m)) => Node::Accent {
                    overs: vec![],
                    unders: vec![*m],
                    base: *c,
                },
                _ => Node::WideAccent {
                    over: *over,
                    under: *under,
                    base,
                },
            }
        }
        Node::Frac { num, den } => Node::Frac {
            num: normalize(num),
            den: normalize(den),
        },
        Node::Sqrt { arg, index } => Node::Sqrt {
            arg: normalize(arg),
            index: *index,
        },
        Node::Sup { arg } => Node::Sup {
            arg: normalize(arg),
        },
        Node::Sub { arg } => Node::Sub {
            arg: normalize(arg),
        },
        Node::BigOp { name, lower, upper } => Node::BigOp {
            name: name.clone(),
            lower: normalize(lower),
            upper: normalize(upper),
        },
        Node::BigOpSym { op, lower, upper } => Node::BigOpSym {
            op: *op,
            lower: normalize(lower),
            upper: normalize(upper),
        },
        Node::Arrow { op, over, under } => Node::Arrow {
            op: *op,
            over: normalize(over),
            under: normalize(under),
        },
        Node::Brace { over, arg, label } => Node::Brace {
            over: *over,
            arg: normalize(arg),
            label: normalize(label),
        },
        Node::Norm { arg } => Node::Norm {
            arg: normalize(arg),
        },
        Node::Delim {
            left,
            right,
            mids,
            segs,
        } => {
            let segs = segs
                .iter()
                .map(|seg| {
                    // A sole 1×1 grid is indistinguishable from its cell in
                    // the fused picture — canonical form is the plain row.
                    // Iterate: splicing can surface a new sole Array.
                    let mut seg = normalize(seg);
                    loop {
                        seg = match &seg[..] {
                            [
                                Node::Array {
                                    rows: 1,
                                    cols: 1,
                                    cells,
                                },
                            ] => normalize(&cells[0]),
                            _ => break seg,
                        };
                    }
                })
                .collect();
            Node::Delim {
                left: *left,
                right: *right,
                mids: mids.clone(),
                segs,
            }
        }
        Node::Cancel { arg } => {
            // A cancel strikes every cell of its subtree, so any Cancel
            // nested anywhere inside it (even deep in a fraction) is the
            // same picture; dissolve them all.
            Node::Cancel {
                arg: normalize(&strip_cancels(&normalize(arg))),
            }
        }
        Node::Array { rows, cols, cells } => Node::Array {
            rows: *rows,
            cols: *cols,
            cells: cells.iter().map(normalize).collect(),
        },
    }
}
