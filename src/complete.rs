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

use crate::editor::{Edit, preview_row, resolve};
use crate::render::{RenderCtx, render_root};
use crate::symbols;

/// One row of the popup: a symbol, the spellings that produce it, and
/// the spelling accepting the row commits to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    /// What the command inserts, as one short line (`α`, `───`, `√⬚`).
    pub symbol: String,
    /// Every spelling that does this, collapsed: `al/p/ha`.
    pub names: String,
    /// The spelling accepting this row types into the minibuffer.
    pub commit: String,
}

/// The open completion popup.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Completion {
    /// The query the list was built for; typing rebuilds it.
    pub query: String,
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
        (!items.is_empty()).then(|| Completion {
            query: query.to_string(),
            items,
            sel: 0,
        })
    }

    pub fn selected(&self) -> Option<&Item> {
        self.items.get(self.sel)
    }

    /// Move the highlight, wrapping at both ends (the list is short
    /// and cycling is what a completion popup does).
    pub fn step(&mut self, down: bool) {
        if self.items.is_empty() {
            return;
        }
        let n = self.items.len();
        self.sel = if down {
            (self.sel + 1) % n
        } else {
            (self.sel + n - 1) % n
        };
    }
}

/// How many rows the popup offers. Enough to be worth scanning, few
/// enough to stay a popup.
pub const MAX_ITEMS: usize = 12;

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
    "langle",
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
    "tex",
    "array",
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
fn score(name: &str, query: &str) -> Option<u32> {
    if query.is_empty() {
        return Some(name.len() as u32);
    }
    if name == query {
        return Some(0);
    }
    if name.starts_with(query) {
        return Some(100 + (name.len() - query.len()) as u32);
    }
    if let Some(at) = name.find(query) {
        return Some(10_000 + (at * 100 + name.len()) as u32);
    }
    subsequence(name, query).map(|spread| 1_000_000 + spread)
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
/// `ab abc abcde A` reads `ab/c/de, A`. Each chain is a name and the
/// tails that extend it, so the popup shows how far you may stop
/// typing instead of repeating the same stem.
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
                s.push('/');
                s.push_str(&pair[1][pair[0].len()..]);
            }
            s
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// The one-line shape a command inserts: the baseline row of what it
/// would preview. A symbol is its character, `\frac` its bar, `\sqrt`
/// its sign over the empty slot — always whatever the command really
/// does, because it comes from the same preview the minibuffer draws.
fn shape(cmd: &str) -> String {
    match resolve(cmd) {
        // An accent hangs its mark above the baseline, where a
        // one-line cell cannot show it: the mark itself is the shape.
        Some(Edit::Accent(mark)) => return mark.glyph().to_string(),
        // A grid's baseline row is a slice of lattice that says little
        // and is wider than a popup row; its pair around a filler says
        // "matrix" in the space there is.
        Some(Edit::Grid { wrap, .. }) => {
            return match wrap {
                symbols::GridWrap::Bare => "⋱".into(),
                symbols::GridWrap::Norm => "‖⋱‖".into(),
                symbols::GridWrap::Pair(l, r) => {
                    format!("{}⋱{}", l.spec(true), r.spec(false))
                }
            };
        }
        // The name boxes have nothing to insert until they are filled.
        Some(Edit::OpenBox(_)) => return "…".into(),
        _ => {}
    }
    if let Some(row) = preview_row(cmd) {
        let block = render_root(&row, None, &RenderCtx::canonical());
        if let Some(line) = block
            .lines
            .get(block.baseline.min(block.height().saturating_sub(1)))
        {
            let s: String = line.iter().collect();
            let s = s.trim().to_string();
            if !s.is_empty() {
                return s;
            }
        }
    }
    // Grid surgery (\addrow …) changes a grid rather than inserting.
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
                let item = Item {
                    symbol: shape(&commit),
                    names: collapse(names.iter().map(String::as_str).collect()),
                    commit,
                };
                (item, names)
            })
            .collect()
    })
}

/// The rows for `query`, best first. A row is offered when any of its
/// spellings matches, and ranks by its best one — so `\al` finds the
/// α row and the row still shows every way to spell it.
pub fn complete(query: &str) -> Vec<Item> {
    let mut hits: Vec<(u32, &Item)> = rows()
        .iter()
        .filter_map(|(item, names)| {
            let best = names.iter().filter_map(|n| score(n, query)).min()?;
            Some((best, item))
        })
        .collect();
    hits.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.commit.cmp(&b.1.commit)));
    hits.into_iter()
        .take(MAX_ITEMS)
        .map(|(_, item)| item.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The structural list is the only hand-written part of the
    /// vocabulary: every entry must still be a command.
    #[test]
    fn structural_commands_all_resolve() {
        for &cmd in STRUCTURAL {
            assert!(resolve(cmd).is_some(), "\\{} no longer resolves", cmd);
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
        assert_eq!(first.names, "al/p/ha");
        assert_eq!(first.commit, "alpha");

        // Substring: `in` reaches the negated \!in as well as \int,
        // and the prefix match outranks the substring one.
        let items = complete("in");
        let names: Vec<&str> = items.iter().map(|i| i.names.as_str()).collect();
        let notin = items.iter().position(|i| i.symbol == "∉");
        let int = items.iter().position(|i| i.commit == "int");
        assert!(notin.is_some(), "no ∉ among {:?}", names);
        assert!(int < notin, "\\int should outrank \\!in: {:?}", names);
    }

    /// The chain collapse is the display rule the user reads names by.
    #[test]
    fn collapse_shows_where_a_name_may_stop() {
        assert_eq!(collapse(vec!["abcde", "abc", "ab", "A"]), "ab/c/de, A");
        assert_eq!(collapse(vec!["to"]), "to");
        // Unrelated spellings stay separate, longest family first.
        assert_eq!(collapse(vec!["xto", "xrightarrow"]), "xrightarrow, xto");
    }

    /// A structural command shows its shape, not an empty cell.
    #[test]
    fn rows_show_what_they_insert() {
        let frac = complete("frac").into_iter().next().unwrap();
        assert!(frac.symbol.contains('─'), "frac shape: {:?}", frac.symbol);
        let sqrt = complete("sqrt").into_iter().next().unwrap();
        assert!(sqrt.symbol.contains('√'), "sqrt shape: {:?}", sqrt.symbol);
        // Every row says something.
        for item in complete("m") {
            assert!(!item.names.is_empty(), "{:?}", item);
        }
    }

    /// Nothing matches nothing: the popup stays closed.
    #[test]
    fn no_matches_means_no_popup() {
        assert!(Completion::build("qqzzxx").is_none());
        assert!(Completion::build("alpha").is_some());
    }
}
