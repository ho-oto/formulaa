//! The matrix environments, in one table: the `\pmatrix`-family
//! minibuffer commands, the LaTeX `\begin{env}` names, and the
//! delimiter pair each wraps. The editor and both LaTeX directions
//! read this, so a new environment is added once.

use super::{ColDelim, Delim};

/// How a grid command wraps its lattice: a delimiter pair, the ‖ ‖
/// norm (\Vmatrix), or nothing (bare \array).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridWrap {
    Bare,
    Pair(Delim, Delim),
    Norm,
}

/// `matrix` and `array` are the bare lattice — a delimited grid spells
/// its pair with `\pmatrix` and friends. `cases`/`rcases` are
/// two-column environments; a wider grid falls back to the general
/// `\left\{ \begin{matrix} … \right.` shell.
pub const GRID_ENVS: &[(&str, GridWrap)] = &[
    ("matrix", GridWrap::Bare),
    ("array", GridWrap::Bare),
    ("smallmatrix", GridWrap::Bare),
    (
        "pmatrix",
        GridWrap::Pair(Delim::Col(ColDelim::Paren), Delim::Col(ColDelim::Paren)),
    ),
    (
        "bmatrix",
        GridWrap::Pair(Delim::Col(ColDelim::Bracket), Delim::Col(ColDelim::Bracket)),
    ),
    (
        "Bmatrix",
        GridWrap::Pair(Delim::Col(ColDelim::Brace), Delim::Col(ColDelim::Brace)),
    ),
    (
        "vmatrix",
        GridWrap::Pair(Delim::Col(ColDelim::Bar), Delim::Col(ColDelim::Bar)),
    ),
    ("Vmatrix", GridWrap::Norm),
    (
        "cases",
        GridWrap::Pair(Delim::Col(ColDelim::Brace), Delim::Col(ColDelim::Null)),
    ),
    (
        "rcases",
        GridWrap::Pair(Delim::Col(ColDelim::Null), Delim::Col(ColDelim::Brace)),
    ),
];
