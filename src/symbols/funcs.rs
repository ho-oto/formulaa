//! Named operators: the upright functions (`\sin`), the limit-taking
//! names (`\lim`, `\argmax`) and the ∑-class big operators.

/// Upright function/operator names (`Node::Func`) — ONE entry per name
/// carries everything the rest of the crate needs to know:
/// - `limits`: the \lim class — the minibuffer command opens a ┈band┈
///   with an under-limit, and ↑/↓ re-promotes a bare one
/// - `spaced`: inner text for \operatorname when the name reads as
///   several words (`argmax` -> `arg\,max`); None = the name verbatim
///
/// ∑-class *symbol* operators live in `ATOMS` (kind BigOp).
pub struct FuncSpec {
    pub limits: bool,
    pub spaced: Option<&'static str>,
}

const FUN: FuncSpec = FuncSpec {
    limits: false,
    spaced: None,
};
const LIM: FuncSpec = FuncSpec {
    limits: true,
    spaced: None,
};

/// The named-operator table, same shape as the symbol `NAMES` map:
/// name -> spec, one row per name.
pub static FUNCS: phf::Map<&'static str, FuncSpec> = phf::phf_map! {
    "arccos" => FUN,
    "arcsin" => FUN,
    "arctan" => FUN,
    "arctg" => FUN,
    "arcctg" => FUN,
    "arg" => FUN,
    "ch" => FUN,
    "cos" => FUN,
    "cosec" => FUN,
    "cosh" => FUN,
    "cot" => FUN,
    "cotg" => FUN,
    "coth" => FUN,
    "csc" => FUN,
    "ctg" => FUN,
    "cth" => FUN,
    "deg" => FUN,
    "det" => LIM,
    "dim" => FUN,
    "exp" => FUN,
    "gcd" => LIM,
    "hom" => FUN,
    "inf" => LIM,
    "ker" => FUN,
    "lg" => FUN,
    "lim" => LIM,
    "ln" => FUN,
    "log" => FUN,
    "max" => LIM,
    "min" => LIM,
    "mod" => FUN,
    "sec" => FUN,
    "sh" => FUN,
    "sin" => FUN,
    "sinh" => FUN,
    "sup" => LIM,
    "tan" => FUN,
    "tanh" => FUN,
    "tg" => FUN,
    "th" => FUN,
    "Pr" => LIM,
    "plim" => LIM,
    "injlim" => LIM,
    "projlim" => LIM,
    // Names that read as several words in LaTeX.
    "argmax" => FuncSpec { limits: true, spaced: Some("arg\\,max") },
    "argmin" => FuncSpec { limits: true, spaced: Some("arg\\,min") },
    "limsup" => FuncSpec { limits: true, spaced: Some("lim\\,sup") },
    "liminf" => FuncSpec { limits: true, spaced: Some("lim\\,inf") },
    "asin" => FUN,
    "acos" => FUN,
    "atan" => FUN,
    "acsc" => FUN,
    "asec" => FUN,
    "acot" => FUN,
    "Tr" => FUN,
    "tr" => FUN,
    "rank" => FUN,
    "erf" => FUN,
    "Res" => FUN,
    "res" => FUN,
    "PV" => FUN,
    "pv" => FUN,
    "Re" => FUN,
    "Im" => FUN,
    "grad" => FUN,
    "curl" => FUN,
};

/// The \lim class: takes band limits; its bare form re-promotes.
pub fn func_takes_limits(name: &str) -> bool {
    FUNCS.get(name).is_some_and(|f| f.limits)
}

/// Inner text for \operatorname: the name, or its spaced reading.
pub fn func_latex_text(name: &str) -> &str {
    FUNCS.get(name).and_then(|f| f.spaced).unwrap_or(name)
}

pub fn is_func_name(name: &str) -> bool {
    FUNCS.contains_key(name)
}
