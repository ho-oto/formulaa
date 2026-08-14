//! Read LaTeX math back into the AST — the reverse of `output/latex`.
//! Two promises, in order:
//!
//! 1. Everything the serializer emits reads back to the tree it came
//!    from: `normalize(row_from_latex(row_to_latex(x))) ==
//!    normalize(strip_spacers(normalize(x)))` — the roundtrip harness
//!    pins this over the corpus and the random trees.
//! 2. Foreign LaTeX (the KaTeX / MathJax dialect) is read best-effort:
//!    an unknown command or an unrepresentable construct is skipped,
//!    never an error. The result is always a valid `Row`.
//!
//! The reader is a tokenizer plus a recursive descent over token
//! slices. Regions (groups, `\left…\right`, environments) are cut out
//! by index scanning with a depth counter before recursing, so a
//! sub-parse never runs past its own region.

use crate::ast::{Node, Row};
use crate::symbols::{Accent, Arrow, ColDelim, Delim, Radical, is_bigop, symbol_by_name};

/// Read a LaTeX math string into a Row, best-effort. Callers that care
/// about canonical trees normalize the result.
pub fn row_from_latex(s: &str) -> Row {
    let toks = tokenize(s);
    Parser { top: true }.row(&toks)
}

// ----- tokenizer -----

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    /// `\name`, or a single-char control like `\{` (name = "{").
    /// `\operatorname*` keeps its star.
    Cmd(String),
    Ch(char),
    Open,
    Close,
    Sup,
    Sub,
    Amp,
    /// `\\`
    RowSep,
    /// A run of spaces — skipped everywhere except inside `\text` and
    /// `\operatorname` (multi-word operator names keep them).
    Space,
}

fn tokenize(s: &str) -> Vec<Tok> {
    let mut out = Vec::new();
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '\\' => match it.peek().copied() {
                Some(n) if n.is_ascii_alphabetic() => {
                    let mut w = String::new();
                    while let Some(&n) = it.peek() {
                        if n.is_ascii_alphabetic() {
                            w.push(n);
                            it.next();
                        } else {
                            break;
                        }
                    }
                    if w == "operatorname" && it.peek() == Some(&'*') {
                        it.next();
                        w.push('*');
                    }
                    out.push(Tok::Cmd(w));
                }
                Some('\\') => {
                    it.next();
                    out.push(Tok::RowSep);
                }
                Some(n) => {
                    it.next();
                    out.push(Tok::Cmd(n.to_string()));
                }
                None => {}
            },
            '{' => out.push(Tok::Open),
            '}' => out.push(Tok::Close),
            '^' => out.push(Tok::Sup),
            '_' => out.push(Tok::Sub),
            '&' => out.push(Tok::Amp),
            // A raw % comments out the rest of the line.
            '%' => {
                for n in it.by_ref() {
                    if n == '\n' {
                        break;
                    }
                }
            }
            // Inline/display math fences of pasted fragments.
            '$' => {}
            // Combining strike overlays cannot be represented (the AA
            // grid is one char per cell); best effort drops them.
            '\u{338}' | '\u{336}' => {}
            ' ' => {
                if out.last() != Some(&Tok::Space) {
                    out.push(Tok::Space);
                }
            }
            c if c.is_whitespace() => {}
            c => out.push(Tok::Ch(c)),
        }
    }
    out
}

// ----- region scanning -----

/// Index in `t` of the token that closes the region opened right
/// before `t[0]` (a `{`, `\left` or `\begin`), or None. Nesting of
/// all three region kinds is tracked, so a nested group cannot hide a
/// `\right`.
fn find_match(t: &[Tok], is_close: impl Fn(&Tok) -> bool) -> Option<usize> {
    let mut group = 0i32;
    let mut lr = 0i32;
    let mut env = 0i32;
    for (i, tok) in t.iter().enumerate() {
        let at_top = group == 0 && lr == 0 && env == 0;
        if at_top && is_close(tok) {
            return Some(i);
        }
        match tok {
            Tok::Open => group += 1,
            Tok::Close => group -= 1,
            Tok::Cmd(c) if c == "left" => lr += 1,
            Tok::Cmd(c) if c == "right" => lr -= 1,
            Tok::Cmd(c) if c == "begin" => env += 1,
            Tok::Cmd(c) if c == "end" => env -= 1,
            _ => {}
        }
        // A stray closer above us: the region is unterminated.
        if group < 0 || lr < 0 || env < 0 {
            return None;
        }
    }
    None
}

/// Split `t` at top-level occurrences of tokens matching `sep`.
fn split_top(t: &[Tok], sep: impl Fn(&Tok) -> bool) -> Vec<&[Tok]> {
    let mut parts = Vec::new();
    let (mut group, mut lr, mut env) = (0i32, 0i32, 0i32);
    let mut start = 0;
    for (i, tok) in t.iter().enumerate() {
        if group == 0 && lr == 0 && env == 0 && sep(tok) {
            parts.push(&t[start..i]);
            start = i + 1;
            continue;
        }
        match tok {
            Tok::Open => group += 1,
            Tok::Close => group -= 1,
            Tok::Cmd(c) if c == "left" => lr += 1,
            Tok::Cmd(c) if c == "right" => lr -= 1,
            Tok::Cmd(c) if c == "begin" => env += 1,
            Tok::Cmd(c) if c == "end" => env -= 1,
            _ => {}
        }
    }
    parts.push(&t[start..]);
    parts
}

// ----- parser -----

struct Parser {
    /// Top level: `\\` is a formula line break (`Node::Break`).
    top: bool,
}

/// What the node just parsed can still absorb: `\sum` and
/// `\operatorname*` take `_` `^` as band limits, `\overbrace` takes
/// its label. Cleared by any other content.
enum Host {
    None,
    Limits(usize),
    Brace(usize),
}

impl Parser {
    fn sub(&self) -> Parser {
        Parser { top: false }
    }

    fn row(&self, mut t: &[Tok]) -> Row {
        let mut out: Row = Vec::new();
        let mut host = Host::None;
        while let Some(tok) = t.first() {
            match tok {
                // TeX primitives that split the enclosing group into a
                // fraction: everything so far is the numerator, the
                // rest of the group the denominator (\atop draws a bar
                // too — formulAA has no barless stack; best effort).
                Tok::Cmd(w) if w == "over" || w == "atop" => {
                    let num = std::mem::take(&mut out);
                    let den = self.sub().row(&t[1..]);
                    out.push(Node::Frac { num, den });
                    return out;
                }
                Tok::Space => t = &t[1..],
                Tok::Amp => {
                    host = Host::None;
                    t = &t[1..];
                }
                Tok::RowSep => {
                    if self.top {
                        out.push(Node::Break);
                    }
                    host = Host::None;
                    t = &t[1..];
                }
                Tok::Sup | Tok::Sub => {
                    let up = matches!(tok, Tok::Sup);
                    let (arg, rest) = self.arg(&t[1..]);
                    t = rest;
                    match host {
                        Host::Limits(i) => {
                            let (lower, upper) = match &mut out[i] {
                                Node::BigOpSym { lower, upper, .. }
                                | Node::BigOp { lower, upper, .. } => (lower, upper),
                                _ => unreachable!(),
                            };
                            let slot = if up { upper } else { lower };
                            if slot.is_empty() {
                                *slot = arg;
                                continue;
                            }
                            // A script the host could not take ends its
                            // reach: it now sits between them, so a
                            // later script belongs to the script, not
                            // back to the band.
                            host = Host::None;
                            out.push(if up {
                                Node::Sup { arg }
                            } else {
                                Node::Sub { arg }
                            });
                        }
                        Host::Brace(i) => {
                            let Node::Brace { over, label, .. } = &mut out[i] else {
                                unreachable!()
                            };
                            // The label rides the marked side: ^ for
                            // \overbrace, _ for \underbrace.
                            if *over == up && label.is_empty() {
                                *label = arg;
                                host = Host::None;
                                continue;
                            }
                            host = Host::None;
                            out.push(if up {
                                Node::Sup { arg }
                            } else {
                                Node::Sub { arg }
                            });
                        }
                        Host::None => {
                            out.push(if up {
                                Node::Sup { arg }
                            } else {
                                Node::Sub { arg }
                            });
                        }
                    }
                }
                _ => {
                    // A brace group is transparent for content but not
                    // for scripts: `{\sum}_{i}` keeps its side-script
                    // instead of becoming band limits.
                    let grouped = matches!(tok, Tok::Open);
                    let before = out.len();
                    t = self.item(t, &mut out);
                    host = match out.last() {
                        _ if grouped => Host::None,
                        // A skipped/no-op token (\limits, \displaystyle,
                        // an unknown command) sits between the host and
                        // its scripts without ending its reach —
                        // \sum\limits_{i} explicitly ASKS for the band.
                        _ if out.len() == before => host,
                        Some(n @ (Node::BigOpSym { .. } | Node::BigOp { .. }))
                            if is_limit_host(n) =>
                        {
                            Host::Limits(out.len() - 1)
                        }
                        Some(Node::Brace { .. }) => Host::Brace(out.len() - 1),
                        _ => Host::None,
                    };
                }
            }
        }
        out
    }

    /// An accent's argument, with the opacity of a doubly-braced base:
    /// `\dot{{\hat{x}}}` is the serializer's spelling for a band over
    /// an accented block, told apart from the stacked `\dot{\hat{x}}`.
    fn accent_arg<'a>(&self, t: &'a [Tok]) -> (Row, bool, &'a [Tok]) {
        let mut i = 0;
        while t.get(i) == Some(&Tok::Space) {
            i += 1;
        }
        if t.get(i) != Some(&Tok::Open) {
            let (row, rest) = self.arg(&t[i..]);
            return (row, false, rest);
        }
        let Some(end) = find_match(&t[i + 1..], |k| *k == Tok::Close) else {
            return (self.sub().row(&t[i + 1..]), false, &t[t.len()..]);
        };
        let inner = &t[i + 1..i + 1 + end];
        let rest = &t[i + end + 2..];
        // Opaque exactly when the whole argument is one nested group.
        let opaque = inner.first() == Some(&Tok::Open)
            && find_match(&inner[1..], |k| *k == Tok::Close).is_some_and(|j| j + 2 == inner.len());
        (self.sub().row(inner), opaque, rest)
    }

    /// One argument: a `{…}` group, or a single item.
    fn arg<'a>(&self, t: &'a [Tok]) -> (Row, &'a [Tok]) {
        match t.first() {
            Some(Tok::Space) => self.arg(&t[1..]),
            Some(Tok::Open) => match find_match(&t[1..], |k| *k == Tok::Close) {
                Some(i) => (self.sub().row(&t[1..1 + i]), &t[i + 2..]),
                None => (self.sub().row(&t[1..]), &t[t.len()..]),
            },
            Some(_) => {
                let mut row = Vec::new();
                let rest = self.item(t, &mut row);
                (row, rest)
            }
            None => (Vec::new(), t),
        }
    }

    /// Parse one construct starting at `t[0]` (not `^`/`_`/space),
    /// appending zero or more nodes. Returns the remaining tokens.
    fn item<'a>(&self, t: &'a [Tok], out: &mut Row) -> &'a [Tok] {
        match &t[0] {
            Tok::Open => match find_match(&t[1..], |k| *k == Tok::Close) {
                Some(i) => {
                    // A group is transparent: parse and splice.
                    out.extend(self.sub().row(&t[1..1 + i]));
                    &t[i + 2..]
                }
                None => {
                    out.extend(self.sub().row(&t[1..]));
                    &t[t.len()..]
                }
            },
            // A stray closer: skip (best effort).
            Tok::Close => &t[1..],
            Tok::Ch(c) => self.plain_char(*c, t, out),
            Tok::Cmd(name) => self.command(name, &t[1..], out),
            Tok::Sup | Tok::Sub | Tok::Amp | Tok::RowSep | Tok::Space => &t[1..],
        }
    }

    /// A plain character: an atom, a prime, or a bare delimiter pair
    /// read best-effort (`(x)` without `\left`).
    fn plain_char<'a>(&self, c: char, t: &'a [Tok], out: &mut Row) -> &'a [Tok] {
        if let Some((open, close)) = match c {
            '(' => Some(('(', ')')),
            '[' => Some(('[', ']')),
            _ => None,
        } {
            let mut depth = 0i32;
            for (i, tok) in t[1..].iter().enumerate() {
                match tok {
                    Tok::Ch(x) if *x == open => depth += 1,
                    Tok::Ch(x) if *x == close => {
                        if depth == 0 {
                            let inner = self.sub().row(&t[1..1 + i]);
                            out.push(Node::Delim {
                                left: Delim::of_spec_side(open, true).unwrap(),
                                right: Delim::of_spec_side(close, false).unwrap(),
                                mids: 0,
                                segs: vec![inner],
                            });
                            return &t[i + 2..];
                        }
                        depth -= 1;
                    }
                    _ => {}
                }
            }
            // Unmatched: drop the bracket, keep going.
            return &t[1..];
        }
        match c {
            '\'' => out.push(Node::Sym('′')),
            c if crate::symbols::is_atom(c) => out.push(Node::Sym(c)),
            // Unrepresentable character: skip.
            _ => {}
        }
        &t[1..]
    }

    /// A control sequence. `t` starts right after the command token.
    fn command<'a>(&self, name: &str, t: &'a [Tok], out: &mut Row) -> &'a [Tok] {
        match name {
            // \not negates the following relation through the shared
            // symbols::negated table; with no slashed codepoint the
            // \not is dropped and the relation stays bare (best
            // effort).
            "not" => {
                let negated = crate::symbols::negated;
                let mut i = 0;
                while t.get(i) == Some(&Tok::Space) {
                    i += 1;
                }
                let (rel, next) = match t.get(i) {
                    Some(Tok::Ch(c)) => (Some(*c), i + 1),
                    Some(Tok::Cmd(w)) => (crate::symbols::symbol_by_name(w), i + 1),
                    // \not{=}: a one-token group is its relation.
                    Some(Tok::Open) => match find_match(&t[i + 1..], |k| *k == Tok::Close) {
                        Some(end) => {
                            let rel = match &t[i + 1..i + 1 + end] {
                                [Tok::Ch(c)] => Some(*c),
                                [Tok::Cmd(w)] => crate::symbols::symbol_by_name(w),
                                _ => None,
                            };
                            (rel, i + 2 + end)
                        }
                        None => (None, i),
                    },
                    _ => (None, i),
                };
                if let Some(neg) = rel.and_then(negated) {
                    out.push(Node::Sym(neg));
                    return &t[next..];
                }
                t
            }
            "frac" | "dfrac" | "tfrac" | "cfrac" => {
                let (num, t) = self.arg(t);
                let (den, t) = self.arg(t);
                out.push(Node::Frac { num, den });
                t
            }
            "sqrt" => {
                let (index, t) = self.radical_index(t);
                let (arg, t) = self.arg(t);
                out.push(Node::Sqrt { arg, index });
                t
            }
            "left" => self.left_right(t, out),
            // Stray structure words (a fragment cut mid-formula).
            "right" | "middle" | "end" => self.skip_delim_token(t),
            "begin" => self.environment(t, out),
            "operatorname" | "operatorname*" | "mathop" => {
                let star = name.ends_with('*');
                let (text, t) = self.group_text(t);
                let name = op_name(&text);
                if name.is_empty() {
                    return t;
                }
                out.push(if star {
                    Node::BigOp {
                        name,
                        lower: vec![],
                        upper: vec![],
                    }
                } else {
                    Node::Func(name)
                });
                t
            }
            "text" | "textrm" | "mbox" | "textnormal" => {
                let (text, t) = self.group_text(t);
                if !text.is_empty() {
                    out.push(Node::Text(text));
                }
                t
            }
            "mathrm" => {
                let (text, rest) = self.group_text(t);
                let clean: String = text.chars().filter(|c| !c.is_whitespace()).collect();
                if !clean.is_empty() && clean.chars().all(|c| c.is_ascii_alphanumeric() || c == '.')
                {
                    match clean.chars().count() {
                        1 => out.push(Node::Roman(clean.chars().next().unwrap())),
                        _ => out.push(Node::Func(clean)),
                    }
                    rest
                } else {
                    // Not an upright run — read the group as math.
                    let (row, rest) = self.arg(t);
                    out.extend(row);
                    rest
                }
            }
            "overbrace" | "underbrace" => {
                let (arg, t) = self.arg(t);
                out.push(Node::Brace {
                    over: name == "overbrace",
                    arg,
                    label: vec![],
                });
                t
            }
            "cancel" => {
                // No struck form exists (adr.md §67): keep the
                // content, drop the line.
                let (arg, t) = self.arg(t);
                out.extend(arg);
                t
            }
            // Formatting space -> the formatting Spacer (vanishes on
            // reparse of the AA, exactly like in LaTeX source).
            "," | ";" | "!" | ":" | "quad" | "qquad" | "enspace" | "thinspace" | "medspace"
            | "thickspace" | "phantom" | "hphantom" => {
                // \phantom{…}: skip the argument too.
                let t = if name.contains("phantom") {
                    self.arg(t).1
                } else {
                    t
                };
                out.push(Node::Spacer);
                t
            }
            // Escaped backslash-space: the semantic ␣ atom.
            " " => {
                out.push(Node::Sym('␣'));
                t
            }
            // \# \$ \% \& — ordinary atoms the serializer escapes.
            "#" | "$" | "%" | "&" => {
                out.push(Node::Sym(name.chars().next().unwrap()));
                t
            }
            // A bare \{ … \} pair (no \left) reads as the brace
            // delimiter, best-effort; unmatched, it is dropped.
            "{" => {
                let mut depth = 0i32;
                for (i, tok) in t.iter().enumerate() {
                    match tok {
                        Tok::Cmd(c) if c == "{" => depth += 1,
                        Tok::Cmd(c) if c == "}" => {
                            if depth == 0 {
                                let inner = self.sub().row(&t[..i]);
                                out.push(Node::Delim {
                                    left: Delim::Col(ColDelim::Brace),
                                    right: Delim::Col(ColDelim::Brace),
                                    mids: 0,
                                    segs: vec![inner],
                                });
                                return &t[i + 1..];
                            }
                            depth -= 1;
                        }
                        _ => {}
                    }
                }
                t
            }
            "}" => t,
            _ => {
                if let Some((mark, _)) = accent_command(name) {
                    let (arg, opaque, rest) = self.accent_arg(t);
                    if let Some(n) = apply_accent(mark, opaque, arg) {
                        out.push(n);
                    }
                    return rest;
                }
                if let Some(op) = Arrow::of_name(name) {
                    let (under, t) = self.optional_bracket(t);
                    let (over, t) = self.arg(t);
                    out.push(Node::Arrow { op, over, under });
                    return t;
                }
                if crate::symbols::func_takes_limits(name) {
                    out.push(Node::BigOp {
                        name: name.to_string(),
                        lower: vec![],
                        upper: vec![],
                    });
                    return t;
                }
                if crate::symbols::is_func_name(name) {
                    out.push(Node::Func(name.to_string()));
                    return t;
                }
                if let Some(row) = self.styled_letters(name, t) {
                    let (nodes, t) = row;
                    out.extend(nodes);
                    return t;
                }
                if let Some(c) = symbol_by_name(name) {
                    out.push(if is_bigop(c) {
                        Node::BigOpSym {
                            op: c,
                            lower: vec![],
                            upper: vec![],
                        }
                    } else {
                        Node::Sym(c)
                    });
                    return t;
                }
                // Unknown command: skip it, keep its arguments as
                // ordinary content (best effort).
                t
            }
        }
    }

    /// `\sqrt[3]` — the only indices the picture can draw are 3 and 4;
    /// any other index is dropped (best effort).
    fn radical_index<'a>(&self, t: &'a [Tok]) -> (Radical, &'a [Tok]) {
        let mut i = 0;
        while t.get(i) == Some(&Tok::Space) {
            i += 1;
        }
        if t.get(i) != Some(&Tok::Ch('[')) {
            return (Radical::Sqrt, t);
        }
        let mut idx = String::new();
        let mut j = i + 1;
        while let Some(tok) = t.get(j) {
            match tok {
                Tok::Ch(']') => {
                    let r = match idx.trim() {
                        "3" => Radical::Cbrt,
                        "4" => Radical::Qdrt,
                        _ => Radical::Sqrt,
                    };
                    return (r, &t[j + 1..]);
                }
                Tok::Ch(c) => idx.push(*c),
                _ => {}
            }
            j += 1;
        }
        (Radical::Sqrt, t)
    }

    /// `[…]` optional argument (the \xrightarrow under-label).
    fn optional_bracket<'a>(&self, t: &'a [Tok]) -> (Row, &'a [Tok]) {
        let mut i = 0;
        while t.get(i) == Some(&Tok::Space) {
            i += 1;
        }
        if t.get(i) != Some(&Tok::Ch('[')) {
            return (Vec::new(), t);
        }
        let (mut depth, mut group, mut lr, mut env) = (0i32, 0i32, 0i32, 0i32);
        // The token right after \left / \right / \middle is that
        // fence's delimiter — never the closer of this optional arg.
        let mut fence_delim = false;
        for (j, tok) in t[i + 1..].iter().enumerate() {
            if fence_delim {
                fence_delim = matches!(tok, Tok::Space);
                if !fence_delim {
                    continue;
                }
            }
            let at_top = group == 0 && lr == 0 && env == 0;
            match tok {
                Tok::Ch('[') if at_top => depth += 1,
                Tok::Ch(']') if at_top && depth == 0 => {
                    let row = self.sub().row(&t[i + 1..i + 1 + j]);
                    return (row, &t[i + j + 2..]);
                }
                Tok::Ch(']') if at_top => depth -= 1,
                Tok::Open => group += 1,
                Tok::Close => group -= 1,
                Tok::Cmd(c) if c == "left" || c == "right" || c == "middle" => {
                    if c == "left" {
                        lr += 1;
                    } else if c == "right" {
                        lr -= 1;
                    }
                    fence_delim = true;
                }
                Tok::Cmd(c) if c == "begin" => env += 1,
                Tok::Cmd(c) if c == "end" => env -= 1,
                _ => {}
            }
        }
        (Vec::new(), t)
    }

    /// The raw text of a `{…}` group (for \operatorname, \text …):
    /// tokens are spelled back verbatim, so `x\y` and `a"b` survive.
    fn group_text<'a>(&self, t: &'a [Tok]) -> (String, &'a [Tok]) {
        let mut i = 0;
        while t.get(i) == Some(&Tok::Space) {
            i += 1;
        }
        if t.get(i) != Some(&Tok::Open) {
            // Single-token argument (\text x).
            return match t.get(i) {
                Some(Tok::Ch(c)) => (c.to_string(), &t[i + 1..]),
                _ => (String::new(), t),
            };
        }
        let Some(end) = find_match(&t[i + 1..], |k| *k == Tok::Close) else {
            return (String::new(), &t[t.len()..]);
        };
        let mut s = String::new();
        let body = &t[i + 1..i + 1 + end];
        let mut k = 0;
        while k < body.len() {
            let tok = &body[k];
            k += 1;
            match tok {
                Tok::Ch(c) => s.push(*c),
                Tok::Space => s.push(' '),
                // An escaped syntax char (\% \{ …) is that char; a real
                // command keeps its backslash (best effort — \text is
                // not a math context, so nothing else expands here).
                Tok::Cmd(w) if w.chars().count() == 1 && !w.starts_with(char::is_alphabetic) => {
                    s.push_str(w)
                }
                Tok::Cmd(w)
                    if w == "textbackslash" || w == "textasciicircum" || w == "textasciitilde" =>
                {
                    s.push(match w.as_str() {
                        "textasciicircum" => '^',
                        "textasciitilde" => '~',
                        _ => '\\',
                    });
                    // …and its empty group, which only exists to end
                    // the command name.
                    if body.get(k) == Some(&Tok::Open) && body.get(k + 1) == Some(&Tok::Close) {
                        k += 2;
                    }
                }
                Tok::Cmd(w) => {
                    s.push('\\');
                    s.push_str(w);
                }
                Tok::RowSep => s.push_str("\\\\"),
                Tok::Open => s.push('{'),
                Tok::Close => s.push('}'),
                Tok::Sup => s.push('^'),
                Tok::Sub => s.push('_'),
                Tok::Amp => s.push('&'),
            }
        }
        (s, &t[i + end + 2..])
    }

    /// `\mathbb{R}`-style styled letters: every char in the group that
    /// the family knows becomes its styled atom.
    fn styled_letters<'a>(&self, name: &str, t: &'a [Tok]) -> Option<(Row, &'a [Tok])> {
        let (style, _) = crate::symbols::alphabets::ALPHABETS
            .entries()
            .find(|(_, f)| f.latex == name)?;
        let (text, rest) = self.group_text(t);
        let row: Row = text
            .chars()
            .filter_map(|c| {
                crate::symbols::alphabets::alphabet_char(&format!("{}{}", style, c)).map(Node::Sym)
            })
            .collect();
        Some((row, rest))
    }

    /// `\left X … \middle| … \right Y`.
    fn left_right<'a>(&self, t: &'a [Tok], out: &mut Row) -> &'a [Tok] {
        let (l, t) = delim_token(t);
        let Some(end) = find_match(t, |k| matches!(k, Tok::Cmd(c) if c == "right")) else {
            // No \right: drop the \left and continue (best effort).
            return t;
        };
        let inner = &t[..end];
        let (r, rest) = delim_token(&t[end + 1..]);
        let parts = split_top(inner, |k| matches!(k, Tok::Cmd(c) if c == "middle"));
        // Each part after a \middle starts with its delimiter token
        // (always `|` in our output); drop it.
        let mut segs: Vec<Row> = Vec::new();
        for (k, part) in parts.iter().enumerate() {
            let part = if k > 0 { delim_token(part).1 } else { *part };
            segs.push(self.sub().row(part));
        }
        match (l, r) {
            (Some('‖'), Some('‖')) if segs.len() == 1 => {
                out.push(Node::Norm {
                    arg: segs.into_iter().next().unwrap(),
                });
            }
            _ => {
                // A `‖` paired with something else degrades to the bar.
                let kind = |spec: Option<char>, left: bool| match spec {
                    Some('‖') => Some(Delim::Col(ColDelim::Bar)),
                    Some(c) => {
                        Delim::of_spec_side(c, left).or_else(|| Delim::of_spec(c).map(|(d, _)| d))
                    }
                    None => None,
                };
                let (Some(left), Some(right)) = (kind(l, true), kind(r, false)) else {
                    // Unreadable pair: keep the contents, drop the shell.
                    for seg in segs {
                        out.extend(seg);
                    }
                    return rest;
                };
                out.push(Node::Delim {
                    left,
                    right,
                    mids: segs.len() - 1,
                    segs,
                });
            }
        }
        rest
    }

    fn skip_delim_token<'a>(&self, t: &'a [Tok]) -> &'a [Tok] {
        delim_token(t).1
    }

    /// `\begin{env} … \end{env}`.
    fn environment<'a>(&self, t: &'a [Tok], out: &mut Row) -> &'a [Tok] {
        let (name, t) = self.group_text(t);
        // \begin{array}{ccc} — the column spec is dropped, but only
        // when a brace group actually follows: without one the first
        // cell must not be eaten in its place.
        let t = if name == "array" {
            let mut i = 0;
            while t.get(i) == Some(&Tok::Space) {
                i += 1;
            }
            if t.get(i) == Some(&Tok::Open) {
                self.group_text(t).1
            } else {
                t
            }
        } else {
            t
        };
        let Some(end) = find_match(t, |k| matches!(k, Tok::Cmd(c) if c == "end")) else {
            return t;
        };
        let inner = &t[..end];
        let rest = self.group_text(&t[end + 1..]).1;
        let array = self.array_of(inner);
        let wrap = |l: Delim, r: Delim, array: Node| Node::Delim {
            left: l,
            right: r,
            mids: 0,
            segs: vec![vec![array]],
        };
        // GRID_ENVS lives in symbols::grids; the editor re-exports it.
        use crate::editor::{GRID_ENVS, GridWrap};
        match GRID_ENVS.get(name.as_str()).copied() {
            Some(GridWrap::Bare) => out.push(array),
            Some(GridWrap::Pair(l, r)) => out.push(wrap(l, r, array)),
            Some(GridWrap::Norm) => out.push(Node::Norm { arg: vec![array] }),
            // Unknown environment: keep its contents inline, with
            // `\\` as formula breaks at top level (aligned & co).
            None => out.extend(self.row(inner)),
        }
        rest
    }

    /// Rows split by `\\`, cells by `&`, padded row-major.
    fn array_of(&self, t: &[Tok]) -> Node {
        let rows: Vec<Vec<Row>> = split_top(t, |k| *k == Tok::RowSep)
            .into_iter()
            .map(|r| {
                split_top(r, |k| *k == Tok::Amp)
                    .into_iter()
                    .map(|c| self.sub().row(c))
                    .collect()
            })
            .collect();
        let nrows = rows.len().max(1);
        let ncols = rows.iter().map(Vec::len).max().unwrap_or(1).max(1);
        let mut cells = Vec::with_capacity(nrows * ncols);
        for r in 0..nrows {
            for c in 0..ncols {
                cells.push(
                    rows.get(r)
                        .and_then(|row| row.get(c))
                        .cloned()
                        .unwrap_or_default(),
                );
            }
        }
        Node::Array {
            rows: nrows,
            cols: ncols,
            cells,
        }
    }
}

/// The delimiter after `\left` / `\right` / `\middle`, as a spec char
/// (`‖` marks `\|`).
fn delim_token(t: &[Tok]) -> (Option<char>, &[Tok]) {
    let mut i = 0;
    while t.get(i) == Some(&Tok::Space) {
        i += 1;
    }
    match t.get(i) {
        Some(Tok::Ch(c)) if "()[]|.<>⟨⟩⌈⌉⌊⌋".contains(*c) => {
            let c = match c {
                '<' => '⟨',
                '>' => '⟩',
                c => *c,
            };
            (Some(c), &t[i + 1..])
        }
        Some(Tok::Cmd(w)) => {
            let c = match w.as_str() {
                "{" => Some('{'),
                "}" => Some('}'),
                "|" | "Vert" => Some('‖'),
                "vert" => Some('|'),
                "langle" => Some('⟨'),
                "rangle" => Some('⟩'),
                "lceil" => Some('⌈'),
                "rceil" => Some('⌉'),
                "lfloor" => Some('⌊'),
                "rfloor" => Some('⌋'),
                _ => None,
            };
            match c {
                Some(c) => (Some(c), &t[i + 1..]),
                None => (None, t),
            }
        }
        _ => (None, t),
    }
}

fn is_limit_host(n: &Node) -> bool {
    match n {
        Node::BigOpSym { lower, upper, .. } | Node::BigOp { lower, upper, .. } => {
            lower.is_empty() || upper.is_empty()
        }
        _ => false,
    }
}

/// An accent command, by its narrow or wide LaTeX name; the bool says
/// the *command* is the stretchy form.
fn accent_command(name: &str) -> Option<(Accent, bool)> {
    Accent::ALL
        .into_iter()
        .find_map(|a| (a.latex() == name).then_some((a, false)))
        .or_else(|| {
            Accent::ALL
                .into_iter()
                .find_map(|a| (a.wide_latex() == name).then_some((a, true)))
        })
}

/// Apply a mark to a parsed base row: a single plain char takes the
/// compact accent, a bare sole accent absorbs the mark into its (flat,
/// innermost first) list, and anything else — including an `opaque`
/// (doubly-braced) base — takes the wide band over the block.
fn apply_accent(mark: Accent, opaque: bool, base: Row) -> Option<Node> {
    match base.as_slice() {
        // An empty base is a legal (normalized) wide accent — the band
        // rides an empty slot.
        [] => Some(Node::WideAccent {
            overs: if mark.under() { vec![] } else { vec![mark] },
            unders: if mark.under() { vec![mark] } else { vec![] },
            base: vec![],
        }),
        _ if opaque => Some(Node::WideAccent {
            overs: if mark.under() { vec![] } else { vec![mark] },
            unders: if mark.under() { vec![mark] } else { vec![] },
            base,
        }),
        [Node::Sym(c)] => Some(Node::Accent {
            overs: if mark.under() { vec![] } else { vec![mark] },
            unders: if mark.under() { vec![mark] } else { vec![] },
            base: *c,
        }),
        [
            Node::Accent {
                overs,
                unders,
                base,
            },
        ] => {
            let (mut overs, mut unders) = (overs.clone(), unders.clone());
            if mark.under() {
                unders.push(mark);
            } else {
                overs.push(mark);
            }
            Some(Node::Accent {
                overs,
                unders,
                base: *base,
            })
        }
        [
            Node::WideAccent {
                overs,
                unders,
                base,
            },
        ] => {
            let (mut overs, mut unders) = (overs.clone(), unders.clone());
            if mark.under() {
                unders.push(mark);
            } else {
                overs.push(mark);
            }
            Some(Node::WideAccent {
                overs,
                unders,
                base: base.clone(),
            })
        }
        _ => Some(Node::WideAccent {
            overs: if mark.under() { vec![] } else { vec![mark] },
            unders: if mark.under() { vec![mark] } else { vec![] },
            base,
        }),
    }
}

/// The name inside \operatorname{…}: `arg\,max` reads back as the one
/// band piece `argmax`.
fn op_name(text: &str) -> String {
    // Spaces stay: our own writer spells `'a␣b'` as \operatorname{a b}.
    text.replace("\\,", "")
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | ' '))
        .collect::<String>()
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::normalize;
    use crate::output::latex::row_to_latex;

    fn tex(s: &str) -> Row {
        normalize(&row_from_latex(s))
    }

    /// Foreign KaTeX/MathJax-style input reads best-effort: known
    /// constructs land, junk is skipped, nothing errors.
    #[test]
    fn foreign_latex_reads_best_effort() {
        // Whitespace and $ fences are noise; plain parens pair up.
        assert_eq!(
            row_to_latex(&tex("$ f(x) = x^2 $")),
            "f\\left(x\\right)=x^{2}"
        );
        assert_eq!(row_to_latex(&tex("e^{i\\pi}")), "e^{i\\pi }");
        // \lim takes band limits; \, is a formatting spacer (vanishes).
        assert_eq!(
            row_to_latex(&tex("\\lim_{x \\to 0} x\\,dx")),
            "\\operatorname*{lim}_{x\\to 0}xdx"
        );
        // Unknown commands are skipped, their brace groups are content.
        assert_eq!(row_to_latex(&tex("\\unknowncmd{x}+y")), "x+y");
        // Styled letters spell through their family.
        assert_eq!(row_to_latex(&tex("\\mathbb{R}")), "\\mathbb{R}");
        // A pmatrix is a parenthesized grid.
        let row = tex("\\begin{pmatrix} a & b \\\\ c & d \\end{pmatrix}");
        assert!(matches!(
            &row[..],
            [Node::Delim { left: Delim::Col(ColDelim::Paren), right: Delim::Col(ColDelim::Paren), segs, .. }]
                if matches!(&segs[0][..], [Node::Array { rows: 2, cols: 2, .. }])
        ));
        // Primes and bare \{ \} pairs.
        assert_eq!(
            row_to_latex(&tex("f'\\{0\\}")),
            "f\\prime \\left\\{0\\right\\}"
        );
    }

    /// The reader never panics on garbage — it just returns what it
    /// could read.
    #[test]
    fn garbage_never_errors() {
        for junk in [
            "",
            "}}}{{{",
            "\\left(",
            "\\begin{pmatrix} a",
            "^",
            "_^_^",
            "\\\\",
            "&&&",
            "\\frac",
            "\\sqrt[",
            "x^",
            "\\right)",
            "\\end{matrix}",
            "\\operatorname",
        ] {
            let _ = tex(junk);
        }
    }
}
