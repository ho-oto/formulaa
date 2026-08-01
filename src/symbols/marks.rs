//! The shared baseline marks — glyphs several nodes spell their
//! structure with, none of them atoms. Node-specific vocabularies live
//! with their node's module (delims, lattice, radicals, braces); these
//! four are the cross-cutting ones.

/// Placeholder for an empty mandatory slot, and explicit base of a script
/// that starts a row (so `[Sup(x)]` is distinguishable from `[Sym(x)]`).
pub const PLACEHOLDER: char = '⬚'; // U+2B1A DOTTED SQUARE

/// Fraction bar. Distinct from the '-' atom and the big-op band. Also
/// the single-arrow body (a bar run directly capped by a `>` head *is*
/// the arrow), the radical overline run and the over/underbrace body.
pub const FRAC_BAR: char = '─'; // U+2500 BOX DRAWINGS LIGHT HORIZONTAL

/// Big-operator band: marks the horizontal extent of over/under limits.
/// A lone band row is also the top-level formula-line separator
/// (`Node::Break`).
pub const OP_BAND: char = '┈'; // U+2508 BOX DRAWINGS LIGHT QUADRUPLE DASH HORIZONTAL

/// Double-arrow body (`══>`); ═ has no other use. Heads render as
/// ASCII < > (never the Unicode arrows — those stay ordinary atoms).
pub const DOUBLE_BODY: char = '═'; // U+2550 BOX DRAWINGS DOUBLE HORIZONTAL
