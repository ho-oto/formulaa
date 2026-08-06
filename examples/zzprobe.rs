use mascii::complete::complete;
use mascii::editor::{Edit, resolve};
use mascii::symbols;
use std::collections::HashMap;

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

fn main() {
    let names = all_names();
    println!("names: {}", names.len());
    let nonascii: Vec<&String> = names.iter().filter(|n| !n.is_ascii()).collect();
    println!("non-ascii names: {:?}", nonascii);
    let unresolved: Vec<&String> = names.iter().filter(|n| resolve(n).is_none()).collect();
    println!("unresolved ({}): {:?}", unresolved.len(), unresolved);

    // Debug-string collisions between genuinely different edits.
    let mut by_key: HashMap<String, Vec<(String, Edit)>> = HashMap::new();
    for n in &names {
        let Some(e) = resolve(n) else { continue };
        by_key
            .entry(format!("{:?}", e))
            .or_default()
            .push((n.clone(), e));
    }
    let mut collisions = 0;
    for (k, v) in &by_key {
        let first = &v[0].1;
        if v.iter().any(|(_, e)| e != first) {
            collisions += 1;
            println!(
                "COLLISION key={} names={:?}",
                k,
                v.iter().map(|(n, _)| n).collect::<Vec<_>>()
            );
        }
    }
    println!("debug-key collisions: {}", collisions);

    // commit spelling must resolve to the group's edit
    let mut bad_commit = 0;
    for v in by_key.values() {
        let commit = v
            .iter()
            .map(|(n, _)| n)
            .max_by_key(|n| (n.len(), n.as_str()))
            .unwrap();
        if resolve(commit).as_ref() != Some(&v[0].1) {
            bad_commit += 1;
            println!("BAD COMMIT {} -> {:?}", commit, resolve(commit));
        }
    }
    println!("bad commits: {}", bad_commit);

    for q in ["bb", "cal", "frak", "hat", "op", "matrix", "sqrt", "lr"] {
        println!("== {:?} resolve={:?}", q, resolve(q).is_some());
        for it in complete(q).iter().take(4) {
            println!(
                "   {:>6} | {:<40} commit={}",
                it.symbol, it.names, it.commit
            );
        }
    }
    // Look at some real popups.
    for q in [
        "", "in", "al", "m", "to", "rm", "b", "s", "l", "int", "!", "o", "e",
    ] {
        println!("--- query {:?}", q);
        for it in complete(q) {
            println!(
                "   {:>6} | {:<40} commit={}",
                it.symbol, it.names, it.commit
            );
        }
    }
}
