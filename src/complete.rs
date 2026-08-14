//! Tab completion for `\commands`.
//!
//! Everything here is *derived*: the spellings come from the symbol
//! tables' own keys, and what a row shows comes from `resolve` /
//! `preview_row` — the same functions that execute the command. The
//! tables are not touched, and this module can be lifted out whole
//! without leaving a hole in them.
//!
//! The one thing it must state itself is the list of structural
//! commands (`\frac`, `\op`, …), which live as literals in `resolve`'s
//! match rather than in a table; a test keeps that list honest by
//! resolving every entry.

use crate::ast::Node;
use crate::editor::{Edit, preview_row, resolve};
use crate::render::{RenderCtx, render_root};
use crate::symbols;

/// What accepting a row does. Every row is exactly one of these — a
/// pair of stringly fields ("commit, unless it is empty") is how a
/// row that pretends to be both slips in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Run this spelling as a command.
    Run(String),
    /// A step rather than an answer: the styled families are rules
    /// over hundreds of characters and a delimiter spec is written a
    /// token at a time, so the row carries the query it leaves behind.
    /// Taking it rewrites what has been typed and asks for the rest.
    Step(String),
}

/// One row of the popup: a symbol, the spellings that produce it, and
/// what accepting the row does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    /// What the command inserts, as one short line (`α`, `───`, `√⬚`).
    /// Blank when the result depends on where the cursor is.
    pub symbol: String,
    /// Every spelling that does this, collapsed: `al[p[ha]]`.
    pub names: String,
    pub action: Action,
}

impl Item {
    /// A row that cannot be executed — it continues the spelling.
    pub fn is_step(&self) -> bool {
        matches!(self.action, Action::Step(_))
    }

    /// The spelling accepting this row runs, if it runs one.
    pub fn commit(&self) -> Option<&str> {
        match &self.action {
            Action::Run(cmd) => Some(cmd),
            Action::Step(_) => None,
        }
    }

    /// The query accepting this row leaves behind, if it is a step.
    pub fn step_to(&self) -> Option<&str> {
        match &self.action {
            Action::Step(next) => Some(next),
            Action::Run(_) => None,
        }
    }
}

/// The open completion popup. The query it answers lives only in the
/// minibuffer — typing rebuilds the list from there.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Completion {
    pub items: Vec<Item>,
    /// Index into `items` of the highlighted row.
    pub sel: usize,
}

impl Completion {
    /// Build the popup for what has been typed so far. `None` when
    /// nothing matches, so the caller can leave the popup closed
    /// rather than showing an empty box.
    pub fn build(query: &str) -> Option<Completion> {
        let items = complete(query);
        (!items.is_empty()).then_some(Completion { items, sel: 0 })
    }

    /// The row the highlight is on, whatever kind it is — the list
    /// scrolls through everything, or rows below a run of shapes could
    /// never be reached.
    pub fn highlighted(&self) -> Option<&Item> {
        self.items.get(self.sel)
    }

    /// The row a commit would take — never a step row, which has no
    /// command behind it yet.
    pub fn selected(&self) -> Option<&Item> {
        self.highlighted().filter(|i| !i.is_step())
    }

    /// Move the highlight, wrapping at both ends (the list is short
    /// and cycling is what a completion popup does).
    pub fn step(&mut self, down: bool) {
        let n = self.items.len();
        if n == 0 {
            return;
        }
        self.sel = if down {
            (self.sel + 1) % n
        } else {
            (self.sel + n - 1) % n
        };
    }
}

/// The list's base size: enough to be worth scanning, few enough to
/// stay a popup. Not a hard cap — a hint family keeps all its rows
/// and the ordinary matches keep their MAX_TAIL, so a query like
/// `\lr` (sixteen step rows) runs past it and the popup scrolls.
pub const MAX_ITEMS: usize = 12;

/// Continuation hints (`\lr(`, `\frak{a…z}`) arrive as a family — a
/// bare `\lr` alone has a dozen tokens to offer — and they rank above
/// the loose matches, so left alone they take every row. This many
/// rows stay reserved for the ordinary matches, which is how `\lr`
/// still shows `longlr` and `delrow` at the bottom of its list.
const MAX_TAIL: usize = 4;

/// Structural commands: the ones `resolve` spells as literals rather
/// than reading from a table. `structural_commands_all_resolve` keeps
/// this list from drifting away from that match.
const STRUCTURAL: &[&str] = &[
    "frac",
    "norm",
    "overbrace",
    "underbrace",
    "ceil",
    "floor",
    "abs",
    "bra",
    "ket",
    "braket",
    "set",
    "mid",
    "addrow",
    "addcol",
    "delrow",
    "delcol",
    "op",
    "op*",
    "rm",
    "text",
    // The negation toggle, and the alternative spellings `resolve`
    // accepts as extra patterns on an arm. A spelling that resolves
    // but is missing here is worse than one nobody can complete: the
    // popup offers a *different* command under it, and Enter takes
    // that one.
    "!",
    "negate",
    "operatorname",
    "operatorname*",
    "limits",
    "latex",
];

/// Every spelling the command layer knows, straight from the tables'
/// own keys. Alphabet families are rules over ~700 characters rather
/// than spellings, so they are represented by their prefixes (`\bb` …)
/// instead of being expanded — the popup is for finding a name, not
/// for listing every letter.
fn all_names() -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    let tables = [
        symbols::NAMES.keys().copied().collect::<Vec<_>>(),
        symbols::FUNCS.keys().copied().collect(),
        symbols::ACCENT_NAMES.keys().copied().collect(),
        symbols::ARROW_NAMES.keys().copied().collect(),
        symbols::RADICAL_NAMES.keys().copied().collect(),
        symbols::DELIM_NAMES.keys().copied().collect(),
        symbols::GRID_ENVS.keys().copied().collect(),
        STRUCTURAL.to_vec(),
    ];
    names.extend(tables.into_iter().flatten().map(str::to_string));
    // `\!x` spells the slashed relation of `\x` (∈ -> ∉). Those
    // spellings are formed on resolve rather than stored, so they are
    // generated the same way here instead of being listed.
    let negatable: Vec<String> = symbols::NAMES
        .entries()
        .filter(|(_, c)| symbols::negated(**c).is_some())
        .map(|(n, _)| format!("!{}", n))
        .collect();
    names.extend(negatable);
    names.sort_unstable();
    names.dedup();
    names
}

/// How well `name` answers `query` — lower ranks first, `None` is no
/// match. The tiers are exact, prefix, substring, then subsequence:
/// `in` finds `int` by prefix, `!in` by substring and `liminf` by
/// subsequence, in that order.
/// The score tiers, named so a caller can say which ones it wants.
const PREFIX: u32 = 100;
const SUBSTRING: u32 = 10_000;
const SUBSEQUENCE: u32 = 1_000_000;

fn score(name: &str, query: &str) -> Option<u32> {
    if name == query {
        return Some(0);
    }
    if name.starts_with(query) {
        return Some(PREFIX + (name.len() - query.len()) as u32);
    }
    if let Some(at) = name.find(query) {
        return Some(SUBSTRING + (at * 100 + name.len()) as u32);
    }
    subsequence(name, query).map(|spread| SUBSEQUENCE + spread)
}

/// The query's characters in order somewhere inside `name`, scored by
/// how spread out they are (a tight run ranks above a scattered one).
/// This is the whole fuzzy pass — deliberately not a ranking system.
fn subsequence(name: &str, query: &str) -> Option<u32> {
    let mut chars = name.char_indices();
    let mut spread = 0u32;
    let mut last: Option<usize> = None;
    for q in query.chars() {
        let (at, _) = chars.find(|&(_, c)| c == q)?;
        if let Some(prev) = last {
            spread += (at - prev) as u32;
        }
        last = Some(at);
    }
    Some(spread * 10 + name.len() as u32)
}

/// Collapse spellings that share a prefix into one cell:
/// `ab abc abcde A` reads `ab[c[de]], A`. Each chain is a name and the
/// tails that extend it, bracketed to show what may be left off — so
/// the row says how far you have to type, not just which spellings
/// exist. A long chain would end in a wall of brackets, so a closing
/// run of more than two folds to `]…]`.
fn collapse(mut names: Vec<&str>) -> String {
    names.sort_unstable_by_key(|n| (n.len(), *n));
    names.dedup();
    let mut chains: Vec<Vec<&str>> = Vec::new();
    for n in names {
        match chains
            .iter_mut()
            .find(|c| n.starts_with(c.last().copied().unwrap_or_default()))
        {
            Some(chain) => chain.push(n),
            None => chains.push(vec![n]),
        }
    }
    // The richest family leads: it is the one the stem belongs to.
    chains.sort_by_key(|c| (std::cmp::Reverse(c.len()), c[0]));
    chains
        .iter()
        .map(|chain| {
            let mut s = chain[0].to_string();
            for pair in chain.windows(2) {
                s.push('[');
                s.push_str(&pair[1][pair[0].len()..]);
            }
            let closes = chain.len() - 1;
            match closes {
                0 => {}
                1 | 2 => s.push_str(&"]".repeat(closes)),
                _ => s.push_str("]…]"),
            }
            s
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// How wide the symbol column is allowed to get.
const SHAPE_MAX: usize = 9;

/// What a command does, for the ones that cannot show it. These act
/// on their surroundings rather than inserting, so the symbol column
/// is blank and the row would otherwise be a bare name.
fn gloss(edit: &Edit) -> Option<&'static str> {
    Some(match edit {
        Edit::Mid => "a │ segment, or the ∣ atom outside a pair",
        Edit::Negate => "slash the symbol before the cursor",
        Edit::AddRow => "row below",
        Edit::AddCol => "column to the right",
        Edit::DelRow => "delete this row",
        Edit::DelCol => "delete this column",
        _ => return None,
    })
}

/// The one-line shape a command inserts: the baseline row of what it
/// would preview. A symbol is its character, `\frac` its bar, `\sqrt`
/// its sign over the empty slot — always whatever the command really
/// does, because it comes from the same preview the minibuffer draws.
fn shape(cmd: &str) -> String {
    match resolve(cmd) {
        // An accent shows its spacing form (`ˆ`, `˙`) — the same
        // one-line stand-in the minibuffer preview uses.
        Some(Edit::Accent(mark)) => return mark.info().preview.to_string(),
        // Commands whose result depends on where the cursor is — the
        // grid surgery, and \mid, which adds a │ segment inside a
        // delimiter but inserts the ∣ atom anywhere else — get no
        // symbol at all. `resolve` does not know the context, so
        // anything drawn here would be a guess, and a guess that is
        // wrong half the time is worse than a blank: \mid used to
        // promise │ and hand you ∣.
        Some(
            Edit::Mid | Edit::Negate | Edit::AddRow | Edit::AddCol | Edit::DelRow | Edit::DelCol,
        ) => return String::new(),
        _ => {}
    }
    if let Some(row) = preview_row(cmd) {
        let block = render_root(&row, None, &RenderCtx::canonical());
        // The column is one line tall. A radical still reads whole
        // from its baseline row alone (√⬚ — the overline is the only
        // thing above it), so it gets that row; anything else must fit
        // the line in full, because a fragment (`\frac`'s bare ───,
        // one strip of a matrix) reads as if it were the result. Those
        // rows show nothing — the preview under the minibuffer shows
        // the real shape.
        let at = if matches!(row[..], [Node::Sqrt { .. }]) {
            Some(block.baseline)
        } else {
            (block.height() == 1).then_some(0)
        };
        if let Some(line) = at.and_then(|y| block.lines.get(y)) {
            let s: String = line.iter().collect();
            let s = s.trim();
            if !s.is_empty() {
                // A wide one-liner would set the symbol column's width
                // for every other row, so it is cut instead.
                return match s.chars().count() > SHAPE_MAX {
                    true => s.chars().take(SHAPE_MAX - 1).chain(['…']).collect(),
                    false => s.to_string(),
                };
            }
        }
    }
    String::new()
}

/// Every command the popup can offer, one row per *edit*: the aliases
/// of a symbol are one row, and the row already knows how it looks.
/// Built once — the tables are static, so grouping them is not work to
/// repeat on every keystroke; a query only scores what is here.
fn rows() -> &'static [(Item, Vec<String>)] {
    use std::collections::HashMap;
    use std::sync::OnceLock;
    static ROWS: OnceLock<Vec<(Item, Vec<String>)>> = OnceLock::new();
    ROWS.get_or_init(|| {
        // Keyed by the edit's own shape. `Edit` is not hashable and
        // comparing every pair would be quadratic over the whole
        // vocabulary, so the debug form stands in for it: equal edits
        // print alike, and different ones do not collide.
        let mut by_edit: HashMap<String, Vec<String>> = HashMap::new();
        let mut order: Vec<String> = Vec::new();
        for name in all_names() {
            let Some(edit) = resolve(&name) else { continue };
            let key = format!("{:?}", edit);
            by_edit.entry(key.clone()).or_insert_with(|| {
                order.push(key.clone());
                Vec::new()
            });
            by_edit.get_mut(&key).expect("just inserted").push(name);
        }
        order
            .into_iter()
            .map(|key| {
                let names = by_edit.remove(&key).expect("one entry per key");
                // Commit the fullest spelling: completion should leave
                // a name that reads back, not the shorthand that
                // happened to sort first.
                let commit = names
                    .iter()
                    .max_by_key(|n| (n.len(), n.as_str()))
                    .cloned()
                    .unwrap_or_default();
                let mut names_shown = collapse(names.iter().map(String::as_str).collect());
                if let Some(g) = resolve(&commit).as_ref().and_then(gloss) {
                    names_shown.push_str(&format!(" ({})", g));
                }
                let item = Item {
                    symbol: shape(&commit),
                    names: names_shown,
                    action: Action::Run(commit),
                };
                (item, names)
            })
            .collect()
    })
}

/// The styled families are rules over some seven hundred characters,
/// so they cannot be listed spelling by spelling. Each family offers
/// its *shape* instead: the range it maps, with the range it produces
/// beside it, split by case so the transformation is legible
/// (`𝔞…𝔷  frak{a…z}`). These rows cannot be committed — there is no
/// one command to run — so the selection walks past them.
fn family_hints(query: &str) -> Vec<(u32, Item)> {
    let mut out = Vec::new();
    for (prefix, family) in symbols::ALPHABETS.entries() {
        // Only when the query is the family's own start. A style token
        // reached by a loose fuzzy match (`\frak` finding `bffrak`)
        // would bury the commands under shapes nobody asked for.
        let Some(score) = score(prefix, query).filter(|&s| s < SUBSTRING) else {
            continue;
        };
        // Lower, upper, digits — the order the ranges read in.
        for (rank, (lo, hi, digits)) in [('a', 'z', false), ('A', 'Z', false), ('0', '9', true)]
            .into_iter()
            .enumerate()
        {
            if digits && family.digits.is_none() {
                continue;
            }
            let styled = |c: char| symbols::alphabet_char(&format!("{}{}", prefix, c));
            let (Some(first), Some(last)) = (styled(lo), styled(hi)) else {
                continue;
            };
            out.push((
                score + rank as u32,
                Item {
                    symbol: format!("{}…{}", first, last),
                    names: format!("{}{{{}…{}}}", prefix, lo, hi),
                    action: Action::Step(prefix.to_string()),
                },
            ));
        }
    }
    out
}

/// `\lr…` builds a pair from a spec written in visual order, so it is
/// not one command but a family of them. While the spec is still being
/// written the list shows the tokens that could come next — the point
/// of typing `\lr\` is wanting to know what the names are — each with
/// the glyph it stands for. Nothing here can be committed either.
fn delim_hints(query: &str) -> Vec<(u32, Item)> {
    let Some(spec) = query
        .strip_prefix("delim")
        .or_else(|| query.strip_prefix("lr"))
    else {
        return Vec::new();
    };
    // The settled tokens, and the name still being typed after them.
    // Note there is no "already resolves, so stop" guard: `\lr(|`
    // resolves (`|` closes) *and* continues (`|` may be a middle) —
    // whether anything can still follow is the grammar's question, not
    // `resolve`'s, and treating them as exclusive was how picking a
    // middle dead-ended the token-by-token flow.
    let Some((settled, tail)) = crate::editor::lr_split(spec) else {
        return Vec::new();
    };
    let naming = settled.len() != spec.len();
    // Before the first token only an opener can come; after it, only a
    // `|` middle or the closer that ends the spec. A spec that already
    // broke one of those rules — or already closed — gets no
    // continuation at all: `\lr)` must be allowed to become an error
    // instead of growing another token, and `\lr()` is finished.
    let Some(opening) = crate::editor::lr_spec_more(settled) else {
        return Vec::new();
    };
    let prefix = if query.starts_with("delim") {
        "delim"
    } else {
        "lr"
    };
    // The side-symmetric `|` and `.` pass both tests, so they stay
    // offered throughout.
    let mut out: Vec<(u32, Item)> = symbols::DELIM_NAMES
        .entries()
        .filter(|(name, glyph)| {
            name.starts_with(tail)
                && symbols::Delim::of_spec_side(**glyph, opening).is_some()
                // `mid` and `dot` are the │ separator's and the null
                // delimiter's *other* names; offering both spellings of
                // one glyph is noise, and "mid" in the opening slot
                // reads as though it were a middle.
                && !matches!(**name, "mid" | "dot")
        })
        .map(|(name, &glyph)| {
            let spec = format!("{}{}\\{}", prefix, settled, name);
            (
                PREFIX + (name.len() - tail.len()) as u32,
                Item {
                    symbol: glyph.to_string(),
                    names: format!("{}…", spec),
                    action: Action::Step(spec),
                },
            )
        })
        .collect();
    // …and the bare spec chars, which is how most pairs are written.
    // A typed backslash says the user wants a name, so they drop out.
    if !naming {
        for (glyph, _) in symbols::DELIM_SPECS.entries() {
            if symbols::Delim::of_spec_side(*glyph, opening).is_none() {
                continue;
            }
            let spec = format!("{}{}{}", prefix, settled, glyph);
            out.push((
                0,
                Item {
                    symbol: glyph.to_string(),
                    names: format!("{}…", spec),
                    action: Action::Step(spec),
                },
            ));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.names.cmp(&b.1.names)));
    out
}

/// A grid is a family too: `\matrix34` is three rows by four columns,
/// so the bare name runs nothing and the list shows the shape instead.
/// A whole env name also brings its siblings, since the names end in
/// one another (`pmatrix`, `bmatrix`, `smallmatrix` all end in
/// `matrix`; `rcases` in `cases`) and that is how the delimited grids
/// are found. Only a whole name does — plain substring matching would
/// answer `\a` with every environment there is.
fn grid_hints(query: &str) -> Vec<(u32, Item)> {
    // No symbol column: a grid is lines tall, so anything one line
    // could show would be a fragment, and the env names already tell
    // the family apart.
    symbols::GRID_ENVS
        .keys()
        // `smallmatrix` reads from LaTeX but is not a command: the AA
        // draws it exactly like `matrix`, so the smallness would be
        // silently lost on the roundtrip — a spelling that lies.
        .filter(|env| **env != "smallmatrix")
        .filter_map(|env| {
            // A name with half its size typed keeps its row up: the
            // rows digit is chosen, the columns one still owed, and
            // the moment between them must not read as "unknown
            // command" (nothing resolves, so the list went blank).
            if let Some(rest) = query.strip_prefix(env)
                && matches!(rest.as_bytes(), [b'1'..=b'9'])
            {
                return Some((
                    1,
                    Item {
                        symbol: String::new(),
                        names: format!("{}{{1…9}}  (columns)", query),
                        action: Action::Step(query.to_string()),
                    },
                ));
            }
            let score = score(env, query).filter(|&s| s < SUBSTRING).or_else(|| {
                let sibling = symbols::GRID_ENVS.contains_key(query) && env.ends_with(query);
                sibling.then_some(SUBSTRING)
            })?;
            Some((
                score + 1,
                Item {
                    symbol: String::new(),
                    names: format!("{}{{1…9}}{{1…9}}  (rows, columns)", env),
                    action: Action::Step(env.to_string()),
                },
            ))
        })
        .collect()
}

/// The mode commands (^F, ^B, ^G, ^Y, ^Q) have minibuffer spellings
/// for terminals that steal those chords; the popup lists them like
/// anything else, told apart by the symbol column: `[^F]`, drawn
/// bold — a chord where the edits show a glyph. (A row tint was
/// tried and dropped: a tinted row under the selection highlight
/// stops reading as either.)
fn mode_hints(query: &str) -> Vec<(u32, Item)> {
    crate::editor::MODE_COMMANDS
        .iter()
        .filter_map(|(names, _, chord, gloss)| {
            let best = names.iter().filter_map(|n| score(n, query)).min()?;
            let commit = names
                .iter()
                .max_by_key(|n| (n.len(), **n))
                .expect("no spellings");
            let mut shown = collapse(names.to_vec());
            shown.push_str(&format!(" ({})", gloss));
            Some((
                best,
                Item {
                    symbol: format!("[{}]", chord),
                    names: shown,
                    action: Action::Run(commit.to_string()),
                },
            ))
        })
        .collect()
}

/// The rows for `query`, best first. A row is offered when any of its
/// spellings matches, and ranks by its best one — so `\al` finds the
/// α row and the row still shows every way to spell it.
pub fn complete(query: &str) -> Vec<Item> {
    // An empty query matches everything and ranks nothing: what came
    // out was an arbitrary sample of the vocabulary (the shortest
    // names, as it happened, plus every alphabet family). No list is
    // honest — the popup waits for a first letter to have an opinion.
    if query.is_empty() {
        return Vec::new();
    }
    let hits: Vec<(u32, &Item)> = rows()
        .iter()
        .filter_map(|(item, names)| {
            let best = names.iter().filter_map(|n| score(n, query)).min()?;
            Some((best, item))
        })
        .collect();
    let mut hits: Vec<(u32, Item)> = hits
        .into_iter()
        .map(|(s, i)| (s, i.clone()))
        .chain(mode_hints(query))
        .chain(family_hints(query))
        .chain(delim_hints(query))
        .chain(grid_hints(query))
        .collect();
    hits.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.names.cmp(&b.1.names)));
    // Two budgets rather than one: the hints keep their whole family
    // uncapped — a truncated family teaches that the dropped sibling
    // spellings don't exist — and the matches keep a tail. Only when
    // the hints are few does the list stay the plain MAX_ITEMS it
    // looks like.
    let hint_count = hits.iter().filter(|(_, i)| i.is_step()).count();
    let mut room = (
        usize::MAX,
        MAX_TAIL.max(MAX_ITEMS.saturating_sub(hint_count)),
    );
    let mut items: Vec<Item> = hits
        .into_iter()
        .filter_map(|(_, i)| {
            let left = if i.is_step() {
                &mut room.0
            } else {
                &mut room.1
            };
            (*left > 0).then(|| {
                *left -= 1;
                i
            })
        })
        .collect();
    let cap = items.len().max(1);
    // A query that is already a command leads the list. Commands whose
    // meaning comes from a suffix — `\rmx`, `\matrix34`, `\lr(]`,
    // `\^z` — are built by parsing, not by lookup, so no table can
    // hold them; without this the row that happens to rank first is
    // what Enter commits, and pressing Tab quietly changes what the
    // command does (`\rmx` used to complete to `\argmax`).
    if let Some(edit) = resolve(query)
        && !items.iter().any(|i| {
            i.commit()
                .is_some_and(|c| resolve(c).as_ref() == Some(&edit))
        })
    {
        items.insert(
            0,
            Item {
                symbol: shape(query),
                names: query.to_string(),
                action: Action::Run(query.to_string()),
            },
        );
        items.truncate(cap);
    }
    items
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The structural list is the only hand-written part of the
    /// vocabulary, so it is checked in both directions. List -> resolve
    /// keeps dead entries out. Resolve -> list is the one that matters
    /// to a user: completing a command must never change which command
    /// it is, and a spelling the popup does not know gets silently
    /// replaced by whatever ranked first.
    #[test]
    fn structural_commands_all_resolve() {
        for &cmd in STRUCTURAL {
            let edit = resolve(cmd);
            assert!(edit.is_some(), "\\{} no longer resolves", cmd);
            let offered = complete(cmd);
            let first = offered
                .first()
                .unwrap_or_else(|| panic!("\\{} completes to nothing", cmd));
            assert_eq!(
                first.commit().and_then(resolve),
                edit,
                "\\{} + Tab + Enter would run {:?}",
                cmd,
                first.action
            );
        }
    }

    /// Completing a command must never change which command it is.
    /// The spellings here are written out rather than read from
    /// `STRUCTURAL`, because a list that checks itself cannot notice
    /// one of its own entries going missing — which is exactly how
    /// `\\!` came to offer ∄.
    #[test]
    fn completing_a_command_keeps_its_meaning() {
        for cmd in [
            "!",
            "negate",
            "bra",
            "ket",
            "operatorname",
            "operatorname*",
            "limits",
            "latex",
            "frac",
            "sqrt",
            "cbrt",
            "norm",
            "mid",
            "op*",
            "text",
            "pmatrix22",
            "alpha",
            "int",
            "xto",
        ] {
            let edit = resolve(cmd);
            assert!(edit.is_some(), "\\{} is not a command", cmd);
            let first = complete(cmd)
                .into_iter()
                .next()
                .unwrap_or_else(|| panic!("\\{} completes to nothing", cmd));
            assert_eq!(
                first.commit().and_then(resolve),
                edit,
                "\\{} + Tab + Enter would run {:?} instead",
                cmd,
                first.action
            );
        }
    }

    /// Families and delimiter specs are not commands but shapes: the
    /// list shows what they map and what they map to, and the
    /// selection walks past them because there is nothing to run.
    #[test]
    fn shapes_are_shown_but_cannot_be_committed() {
        let frak = complete("frak");
        // Lower, upper, digits — in the order the ranges read.
        assert_eq!(frak[0].names, "frak{a…z}");
        assert_eq!(frak[0].symbol, "𝔞…𝔷");
        assert_eq!(frak[1].names, "frak{A…Z}");
        assert!(frak.iter().take(2).all(|i| i.is_step()), "{:?}", frak);

        // A half-written delimiter spec offers the tokens that could
        // come next, with the glyph each stands for — and only the
        // ones that fit the side being written.
        let after_open = complete("lr(\\r");
        assert!(!after_open.is_empty(), "no hint after an opener");
        assert!(
            after_open
                .iter()
                .all(|i| i.is_step() && i.names.starts_with("lr(")),
            "{:?}",
            after_open
        );
        assert!(
            complete("lr\\l").iter().any(|i| i.names == "lr\\lceil…"),
            "\\lceil is offered as an opener"
        );
        // …but a spec that already builds a pair is a command again.
        let done = complete("lr(]");
        assert_eq!(done[0].action, Action::Run("lr(]".into()), "{:?}", done[0]);

        // A list of nothing but shapes has nothing to select, so Enter
        // falls through to whatever was typed.
        let list = Completion::build("frak").expect("shapes are listed");
        assert!(list.selected().is_none(), "a shape was selectable");
    }

    /// A grid's size is part of the command, so the bare name runs
    /// nothing and the list answers with the shape — the way `\lr`
    /// answers with tokens rather than building an empty pair.
    #[test]
    fn a_grid_asks_for_its_size() {
        for env in ["matrix", "pmatrix", "cases"] {
            assert!(resolve(env).is_none(), "\\{} still builds something", env);
            let rows = complete(env);
            let shape = format!("{}{{1…9}}{{1…9}}", env);
            let row = rows
                .iter()
                .find(|r| r.names.starts_with(&shape))
                .unwrap_or_else(|| panic!("no shape row for \\{}: {:?}", env, rows));
            assert!(row.is_step(), "the shape row was committable");
        }
        // A name that carries a size is a command, and nothing else.
        let sized = complete("matrix34");
        assert_eq!(sized.len(), 1, "{:?}", sized);
        assert!(!sized[0].is_step());
        assert!(resolve("matrix34").is_some());
    }

    /// Commands built from a suffix cannot be in any table, so the
    /// list has to offer the query itself: `\\rmx` used to complete to
    /// `\\argmax` (r-m-x is a subsequence of it) and Enter took that.
    #[test]
    fn a_parametric_command_leads_its_own_list() {
        for cmd in [
            "rmx",       // \mathrm{x}
            "rmabc",     // an upright run
            "textfoo",   // \text{foo}
            "matrix34",  // a 3x4 grid
            "pmatrix12", // …in parentheses
            "lr(]",      // a mismatched delimiter pair
            "^z",        // a real superscript
            "_i",
        ] {
            let edit = resolve(cmd);
            assert!(edit.is_some(), "\\{} is not a command", cmd);
            let first = complete(cmd)
                .into_iter()
                .next()
                .unwrap_or_else(|| panic!("\\{} completes to nothing", cmd));
            assert_eq!(
                first.commit().and_then(resolve),
                edit,
                "\\{} + Tab + Enter would run {:?} instead",
                cmd,
                first.action
            );
        }
    }

    /// Prefix first, then substring, then subsequence — and a row
    /// gathers every spelling of one symbol.
    #[test]
    fn ranking_and_grouping() {
        let items = complete("alpha");
        let first = &items[0];
        assert_eq!(first.symbol, "α");
        // \alpha, \al and \alp are one row, shown as one stem.
        assert_eq!(first.names, "al[p[ha]]");
        assert_eq!(first.action, Action::Run("alpha".into()));

        // Substring: `in` reaches the negated \!in as well as \int,
        // and the prefix match outranks the substring one.
        let items = complete("in");
        let names: Vec<&str> = items.iter().map(|i| i.names.as_str()).collect();
        // `None < Some(_)` in Rust, so both positions must be pinned
        // before they are compared — otherwise a missing \int passes.
        let notin = items
            .iter()
            .position(|i| i.symbol == "∉")
            .unwrap_or_else(|| panic!("no ∉ among {:?}", names));
        let int = items
            .iter()
            .position(|i| i.commit() == Some("int"))
            .unwrap_or_else(|| panic!("no \\int among {:?}", names));
        assert!(int < notin, "\\int should outrank \\!in: {:?}", names);
    }

    /// The chain collapse is the display rule the user reads names by.
    #[test]
    fn collapse_shows_where_a_name_may_stop() {
        assert_eq!(collapse(vec!["abcde", "abc", "ab", "A"]), "ab[c[de]], A");
        // A deep chain folds its closing run.
        assert_eq!(
            collapse(vec!["a", "al", "alp", "alph", "alpha"]),
            "a[l[p[h[a]…]"
        );
        assert_eq!(collapse(vec!["to"]), "to");
        // Unrelated spellings stay separate, longest family first.
        assert_eq!(collapse(vec!["xto", "xrightarrow"]), "xrightarrow, xto");
    }

    /// A one-line command shows its shape; a taller one shows nothing,
    /// unless a natural one-line form exists (a radical's √⬚).
    #[test]
    fn rows_show_what_they_insert() {
        for tall in ["frac", "matrix34"] {
            let row = complete(tall).into_iter().next().unwrap();
            assert_eq!(row.symbol, "", "\\\\{} is taller than the column", tall);
        }
        // A radical sheds only its overline: √⬚ is still the whole
        // shape, where ─── without its numerator is not.
        let sqrt = complete("sqrt").into_iter().next().unwrap();
        assert_eq!(sqrt.symbol, "√⬚");
        // An accent shows its spacing stand-in, not the mark hung on
        // a two-line ⬚ and not the drawn speck (`˰`).
        let hat = complete("hat").into_iter().next().unwrap();
        assert_eq!(hat.symbol, "ˆ");
        let vec = complete("vec").into_iter().next().unwrap();
        assert_eq!(vec.symbol, "→");
        let cbrt = complete("cbrt").into_iter().next().unwrap();
        assert_eq!(cbrt.symbol, "∛⬚");
        let abs = complete("abs").into_iter().next().unwrap();
        assert_eq!(abs.symbol, "⎢⬚⎥");
        let braket = complete("braket").into_iter().next().unwrap();
        assert!(braket.symbol.contains('⟨'), "{:?}", braket.symbol);
        // A row's symbol may be blank — the grid surgery and \mid
        // depend on the cursor, so `resolve` cannot know — but a row
        // that inserts something must show it.
        let sym = complete("alpha").into_iter().next().unwrap();
        assert_eq!(sym.symbol, "α");
        assert!(
            complete("mid")
                .into_iter()
                .next()
                .unwrap()
                .symbol
                .is_empty()
        );
    }

    /// Nothing matches nothing: the popup stays closed.
    #[test]
    fn no_matches_means_no_popup() {
        assert!(Completion::build("qqzzxx").is_none());
        assert!(Completion::build("alpha").is_some());
    }
    /// A hint family must not evict every ordinary match: `\lr` opens
    /// a dozen delimiter tokens, and `lr` is also a substring of
    /// `longlr` and `delrow`. The list keeps a tail for them.
    #[test]
    fn a_hint_family_leaves_room_for_matches() {
        let rows = complete("lr");
        assert!(
            rows.iter().filter(|r| r.is_step()).count() >= 8,
            "the \\lr tokens are all there: {:?}",
            rows
        );
        let tail: Vec<&Item> = rows.iter().filter(|r| !r.is_step()).collect();
        assert!(!tail.is_empty(), "no ordinary match survived: {:?}", rows);
        assert!(
            tail.iter().any(|r| r.names.contains("longlr")),
            "{:?}",
            tail
        );
    }

    /// The delimited grids are found by typing the part of the name
    /// they share: a whole env name offers the whole family.
    #[test]
    fn the_grid_family_is_found_through_its_shared_name() {
        let rows = complete("matrix");
        for env in ["pmatrix", "bmatrix", "Bmatrix"] {
            let row = rows
                .iter()
                .find(|r| r.names.starts_with(env))
                .unwrap_or_else(|| panic!("no {} row: {:?}", env, rows));
            assert!(row.is_step() && row.symbol.is_empty(), "{:?}", row);
        }
    }
    /// A spec that can no longer become one is offered nothing. `\lr)`
    /// opens with a closer and `\lrx` with a letter: handing either
    /// another token lets the spelling grow forever without ever
    /// reaching a command, and the usage error never gets a chance to
    /// be the answer.
    #[test]
    fn a_broken_spec_gets_no_continuation() {
        for q in ["lr)", "lr(]x", "delim}", "lrx"] {
            assert!(resolve(q).is_none(), "\\{} resolves", q);
            let offered: Vec<Item> = complete(q).into_iter().filter(|i| i.is_step()).collect();
            assert!(offered.is_empty(), "\\{} was offered {:?}", q, offered);
        }
        // A name that is fully spelled is a token, not a name still
        // being typed — otherwise `\lr\langle` offers itself back and
        // taking the row changes nothing, forever.
        let rows = complete("lr\\langle");
        assert!(!rows.is_empty(), "the closers should be offered");
        for row in &rows {
            let ext = row.step_to().expect("a step row");
            assert!(
                ext.len() > "lr\\langle".len() && ext.starts_with("lr\\langle"),
                "{:?} does not move the spelling on",
                row
            );
        }
    }
    /// `\lr(|` both runs (`|` closes the pair) and continues (`|` may
    /// be a middle): the command row leads and the continuations still
    /// follow, so picking a middle no longer dead-ends the
    /// token-by-token flow. The split is token-aware too — in
    /// `\lr\vert|` the name ended at its `t`, and reading everything
    /// after the last backslash as a half-typed name silenced it.
    #[test]
    fn a_middle_keeps_the_flow_alive() {
        for q in ["lr(|", "lr\\vert|", "lr(||"] {
            let rows = complete(q);
            assert_eq!(rows[0].action, Action::Run(q.into()), "{:?}", rows);
            let steps: Vec<&Item> = rows.iter().filter(|i| i.is_step()).collect();
            assert!(!steps.is_empty(), "\\{} dead-ends: {:?}", q, rows);
            assert!(
                steps.iter().all(|i| i
                    .step_to()
                    .is_some_and(|s| s.starts_with(q) && s.len() > q.len())),
                "{:?}",
                steps
            );
        }
        // …but a closed spec is finished: nothing follows `\lr()`.
        assert!(complete("lr()").iter().all(|i| !i.is_step()));
    }

    /// An empty query builds no list: it matches everything, so any
    /// list would be an arbitrary sample of the vocabulary.
    #[test]
    fn an_empty_query_builds_no_list() {
        assert!(Completion::build("").is_none());
    }

    /// Between a grid's two digits nothing resolves and the env name
    /// no longer matches, so a dedicated row bridges the gap — the
    /// spelling is on course and the list should say so.
    #[test]
    fn a_half_sized_grid_keeps_its_row() {
        let rows = complete("matrix3");
        assert_eq!(rows.len(), 1, "{:?}", rows);
        assert_eq!(rows[0].names, "matrix3{1…9}  (columns)");
        assert_eq!(rows[0].action, Action::Step("matrix3".into()));
        // A zero row count can never become a command; no bridge.
        assert!(complete("matrix0").is_empty());
    }
    /// The mode commands are listed, glossed, and lead their exact
    /// spellings — a chord-less terminal has to be able to *find*
    /// them, not just know them.
    #[test]
    fn mode_commands_are_listed() {
        let rows = complete("f");
        assert_eq!(rows[0].names, "f[ree], F (free cursor)");
        assert_eq!(rows[0].symbol, "[^F]");
        assert_eq!(rows[0].commit(), Some("free"));
        let rows = complete("blocksel");
        assert!(
            rows.iter().any(|r| r.commit() == Some("blockselect")),
            "{:?}",
            rows
        );
    }

    /// The hint budget must not truncate a family: `\\lr` offers every
    /// named opener, or a missing sibling reads as a spelling that
    /// does not exist (`lr\\lceil` offered, `lr\\lfloor` absent).
    #[test]
    fn a_hint_family_is_never_truncated() {
        let rows = complete("lr");
        for want in [
            "none", "vert", "lceil", "lfloor", "langle", "lbrace", "lbrack", "lparen",
        ] {
            assert!(
                rows.iter()
                    .any(|i| i.step_to().is_some_and(|s| s == format!("lr\\{want}"))),
                "lr\\{want} missing: {:?}",
                rows.iter().filter_map(|i| i.step_to()).collect::<Vec<_>>()
            );
        }
    }
}
