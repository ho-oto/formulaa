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
        overs: Vec<crate::symbols::Accent>,
        unders: Vec<crate::symbols::Accent>,
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
        overs: Vec<crate::symbols::Accent>,
        unders: Vec<crate::symbols::Accent>,
        base: Row,
    },
    Frac {
        num: Row,
        den: Row,
    },
    /// index 2 = √, 3 = ∛, 4 = ∜ (the only radical glyphs Unicode has).
    Sqrt {
        arg: Row,
        index: crate::symbols::Radical,
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
        op: crate::symbols::Arrow,
        over: Row,
        under: Row,
    },
    /// Auto-scaling delimiter block. `left`/`right` are pair kinds
    /// (`Delim`) — the slot decides the side, so `\lr(]` stores
    /// Paren/Bracket and a right-shaped glyph cannot open. `mids`
    /// counts the │ middles separating the segments;
    /// segs.len() == mids + 1. Segments are ordinary rows — a matrix
    /// is nothing more than a Delim whose segment contains an Array.
    Delim {
        left: crate::symbols::Delim,
        right: crate::symbols::Delim,
        mids: usize,
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
}

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
            (Node::Norm { arg }, Field::Seg(0)) => arg,
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
            (Node::Norm { arg }, Field::Seg(0)) => arg,
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

/// The canonical node for an upright name standing on its own: a
/// one-letter run is `Roman` (its picture — 'd', or a glued d𝑥 — reads
/// back that way), so `Func` always holds 2+ letters.
fn bare_upright(name: String) -> Node {
    let mut cs = name.chars();
    match (cs.next(), cs.next()) {
        // One char: Roman for a letter; a digit or dot has no
        // italic/upright distinction, so it lands straight on the
        // plain atom (idempotence: the Roman arm below must not need
        // a second pass to finish the job).
        (Some(c), None) if c.is_alphabetic() => Node::Roman(c),
        (Some(c), None) => Node::Sym(c),
        _ => Node::Func(name),
    }
}

/// Canonical form: merge adjacent same-kind scripts (x^{a}^{b} == x^{ab} in
/// the picture, so the parser can only ever return the merged form).
/// `parse(render(x)) == normalize(x)` is the roundtrip invariant, and
/// this must be idempotent — it runs again after every merge, so a rule
/// has to land on the shape the next pass would leave alone.
pub fn normalize(row: &Row) -> Row {
    // A band with no limits collapses to its bare base — but only when
    // the editor can lift that base back into a band (∑-class atoms and
    // lim-class functions, via ↑/↓). An \op* name (Text piece or a
    // multi-piece base) has no such way back, so its band survives
    // empty limits: ┈ess┈sup┈ stays a band.
    let mut pre: Row = Vec::with_capacity(row.len());
    for node in row {
        match normalize_node(node) {
            // The bare form is one-to-one with the band (the editor
            // keeps the band in its own tree while the cursor is
            // inside).
            Node::BigOpSym { op, lower, upper } if lower.is_empty() && upper.is_empty() => {
                pre.push(Node::Sym(op))
            }
            // …and the bare form is then normalized in turn: a
            // one-letter name is Roman, not Func (this pass runs once,
            // so the collapse must land on the final shape or
            // normalize would not be idempotent).
            Node::BigOp { name, lower, upper } if lower.is_empty() && upper.is_empty() => {
                pre.push(bare_upright(name))
            }
            // A markless wide accent is just its base.
            Node::WideAccent {
                overs,
                unders,
                base,
            } if overs.is_empty() && unders.is_empty() => pre.extend(base),
            Node::Func(t) => pre.push(bare_upright(t)),
            // Roman marks a lone upright LETTER; a digit or dot has no
            // italic form to be upright against, so its bare picture
            // reads back as a plain atom — canonicalize it to one.
            Node::Roman(c) if !c.is_alphabetic() => pre.push(Node::Sym(c)),
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
        // Empty scripts do not exist in normal form: their lone ⬚
        // placeholder fuses with neighbouring script blocks in the picture,
        // so the renderer must never produce one.
        match &node {
            Node::Sup { arg } | Node::Sub { arg } if arg.is_empty() => {
                continue;
            }
            // An empty text run has no picture of its own worth
            // keeping, struck or not.
            Node::Text(t) | Node::Func(t) if t.is_empty() => continue,
            _ => {}
        }
        // Scripts merge *across* spacers: the blank column a spacer
        // renders is internal to the script run in the picture, so the
        // parser reads one merged node — canonical form matches (the
        // spacers die in the merge, and are restored when no merge fires).
        let mut tail: Row = Vec::new();
        if matches!(node, Node::Sup { .. } | Node::Sub { .. }) {
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
            _ => {
                out.extend(tail);
                out.push(node);
            }
        }
    }
    // Trailing spacers pad nothing visible; spacers are only meaningful
    // *between* siblings.
    while matches!(out.last(), Some(Node::Spacer | Node::Break)) {
        out.pop();
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
            Node::WideAccent {
                overs,
                unders,
                base,
            } => out.push(Node::WideAccent {
                overs: overs.clone(),
                unders: unders.clone(),
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
                mids: *mids,
                segs: segs.iter().map(strip_spacers).collect(),
            }),
            Node::Array { rows, cols, cells } => out.push(Node::Array {
                rows: *rows,
                cols: *cols,
                cells: cells.iter().map(strip_spacers).collect(),
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
        Node::WideAccent {
            overs,
            unders,
            base,
        } => {
            let base = normalize(base);
            // A one-char base is the compact stacked Accent (x̂): the
            // two would draw the same picture otherwise. Marks stack
            // the same way in both, so the lists carry straight over.
            match &base[..] {
                // …and a markless one is just that base, or the arm
                // above would have to run a second time (normalize is
                // a single pass and must be idempotent).
                [Node::Sym(c)] if overs.is_empty() && unders.is_empty() => Node::Sym(*c),
                [Node::Sym(c)] => Node::Accent {
                    overs: overs.clone(),
                    unders: unders.clone(),
                    base: *c,
                },
                _ => Node::WideAccent {
                    overs: overs.clone(),
                    unders: unders.clone(),
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
                mids: *mids,
                segs,
            }
        }
        Node::Array { rows, cols, cells } => Node::Array {
            rows: *rows,
            cols: *cols,
            cells: cells.iter().map(normalize).collect(),
        },
    }
}
