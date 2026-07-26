//! Named operators: the upright functions (`\sin`), the limit-taking
//! names (`\lim`, `\argmax`) and the ∑-class big operators.

/// Upright function/operator names (`Node::Func`) — ONE entry per name
/// carries everything the rest of the crate needs to know:
/// - `limits`: the \lim class — the minibuffer command opens a ┈band┈
///   with an under-limit, and ↑/↓ re-promotes a bare one
///   (ast::promotable_base)
/// - `spaced`: inner text for \operatorname when the name reads as
///   several words (`argmax` -> `arg\,max`); None = the name verbatim (\div is ÷ in LaTeX, \Re is
///   ℜ — the upright-operator reading needs \operatorname)
///
/// ∑-class *symbol* operators live in `BIG_OPS`; multi-word \op* bases
/// are assembled in `ast::op_words`.
pub struct FuncSpec {
    pub name: &'static str,
    pub limits: bool,
    pub spaced: Option<&'static str>,
}

const fn fun(name: &'static str, limits: bool) -> FuncSpec {
    FuncSpec {
        name,
        limits,
        spaced: None,
    }
}

/// A name that reads as several words in LaTeX (`arg\,max`).
const fn words(name: &'static str, spaced: &'static str) -> FuncSpec {
    FuncSpec {
        name,
        limits: true,
        spaced: Some(spaced),
    }
}

pub const FUNCS: &[FuncSpec] = &[
    fun("arccos", false),
    fun("arcsin", false),
    fun("arctan", false),
    fun("arctg", false),
    fun("arcctg", false),
    fun("arg", false),
    fun("ch", false),
    fun("cos", false),
    fun("cosec", false),
    fun("cosh", false),
    fun("cot", false),
    fun("cotg", false),
    fun("coth", false),
    fun("csc", false),
    fun("ctg", false),
    fun("cth", false),
    fun("deg", false),
    fun("det", true),
    fun("dim", false),
    fun("exp", false),
    fun("gcd", true),
    fun("hom", false),
    fun("inf", true),
    fun("ker", false),
    fun("lg", false),
    fun("lim", true),
    fun("ln", false),
    fun("log", false),
    fun("max", true),
    fun("min", true),
    fun("mod", false),
    fun("sec", false),
    fun("sh", false),
    fun("sin", false),
    fun("sinh", false),
    fun("sup", true),
    fun("tan", false),
    fun("tanh", false),
    fun("tg", false),
    fun("th", false),
    fun("Pr", true),
    fun("plim", true),
    fun("injlim", true),
    fun("projlim", true),
    // Names that read as several words in LaTeX.
    words("argmax", "arg\\,max"),
    words("argmin", "arg\\,min"),
    words("limsup", "lim\\,sup"),
    words("liminf", "lim\\,inf"),
    fun("asin", false),
    fun("acos", false),
    fun("atan", false),
    fun("acsc", false),
    fun("asec", false),
    fun("acot", false),
    fun("Tr", false),
    fun("tr", false),
    fun("rank", false),
    fun("erf", false),
    fun("Res", false),
    fun("res", false),
    fun("PV", false),
    fun("pv", false),
    fun("Re", false),
    fun("Im", false),
    fun("grad", false),
    fun("curl", false),
];

fn func_spec(name: &str) -> Option<&'static FuncSpec> {
    FUNCS.iter().find(|f| f.name == name)
}

/// The \lim class: takes band limits; its bare form re-promotes.
pub fn func_takes_limits(name: &str) -> bool {
    func_spec(name).is_some_and(|f| f.limits)
}

/// Inner text for \operatorname: the name, or its spaced reading.
pub fn func_latex_text(name: &str) -> &str {
    func_spec(name).and_then(|f| f.spaced).unwrap_or(name)
}

pub fn is_func_name(name: &str) -> bool {
    func_spec(name).is_some()
}

/// Longest known function name that is a prefix of `s`.
pub fn func_prefix(s: &str) -> Option<&'static str> {
    FUNCS
        .iter()
        .filter(|f| s.starts_with(f.name))
        .max_by_key(|f| f.name.len())
        .map(|f| f.name)
}
