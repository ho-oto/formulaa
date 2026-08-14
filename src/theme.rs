//! The TUI color scheme, in one place. Every color the interface uses
//! is a named constant here — tweak freely; nothing else in the code
//! hard-codes a color.
//!
//! Two layers. **The palette** names the colors themselves — the only
//! place a `ratatui::Color` literal may appear — and stays inside the
//! 16 ANSI colors for portability (adr.md roadmap): terminal themes
//! can restyle every one of them, and nothing depends on the 256-color
//! table being honored. (ratatui's plain names are the dark ANSI
//! colors — the backend maps `Red`->DarkRed (1, maroon),
//! `Green`->DarkGreen (2), `Magenta`->DarkMagenta (5, purple); the
//! bright half is the `Light*` variants.) **The roles** below assign a
//! palette color to each use, so "what does purple mean here" and
//! "which purple is it" are separate questions.
//!
//! The ground rule: a fill is either a dark palette color with `WHITE`
//! glyphs on it, or reverse video — never a light color, never
//! dark-on-dark. The caret and the secondary marks (^B's outward-step
//! ring, the ^F snap preview) are reverse video with no color of their
//! own; both choices keep light terminals readable.
//!
//! What the colors mean:
//!
//! - **purple** — the selection layer: selected things, the cursor
//!   itself, mode commands in the minibuffer.
//! - **green** — OK / actionable: known commands, the grid frame,
//!   insert ghosts, normal messages.
//! - **maroon** — errors: unknown commands, error messages.
//! - **gray** — neutral: the ␣ glyph, the floating-box ground; never
//!   carries meaning.

use ratatui::style::Color;

// ----- the palette: color names -> ANSI colors -----

/// ANSI 5 (dark magenta).
const PURPLE: Color = Color::Magenta;
/// ANSI 2 (dark green).
const GREEN: Color = Color::Green;
/// ANSI 1 (dark red).
const MAROON: Color = Color::Red;
/// ANSI 8 (bright black).
const GRAY: Color = Color::DarkGray;
/// ANSI 15.
const WHITE: Color = Color::White;
/// The terminal's own default foreground.
const DEFAULT: Color = Color::Reset;

// ----- roles: every use, assigned through the palette -----

// Chrome.

/// Status messages on the bottom line.
pub const MESSAGE_FG: Color = GREEN;
/// … the same line when the message is an error.
pub const MESSAGE_ERR_FG: Color = MAROON;
/// The canvas border and the usage/help bottom line: the default
/// foreground, same as the formula text — the frame is part of the
/// page, not chrome.
pub const BORDER_FG: Color = DEFAULT;
/// The visible space atom ␣ (on a themed ground it turns `GROUND_FG`
/// like any other glyph).
pub const SPACE_FG: Color = GRAY;

// The selection layer.

/// Selection background — every "this is selected" surface: the linear
/// selection, grid cell rectangles and lanes (told apart by reach, not
/// color), the delimiter pair armed for unwrapping, and ^B's
/// highlighted ancestor. One color, because they are one idea.
pub const SELECTION_BG: Color = PURPLE;
/// The minibuffer overlay while the typed name is a *mode* command
/// (\f \b \t … — it moves a mode, not the formula): the selection
/// purple, so the three minibuffer grounds stay three ideas — green
/// runs an edit, purple changes where you stand, maroon runs nothing.
pub const MINIBUF_MODE_BG: Color = PURPLE;
/// The completion list's highlighted row: the one choice on the
/// neutral popup ground.
pub const POPUP_SEL_BG: Color = PURPLE;

// OK / actionable.

/// In-place minibuffer overlay (`\cmd` typed at the cursor) while the
/// name is a known command.
pub const MINIBUF_BG: Color = GREEN;
/// Grid lane-gap cursor: the ghost lane previewing an insertion
/// (Enter inserts here).
pub const GRID_INSERT_BG: Color = GREEN;
/// The edited matrix's lattice frame while grid mode is active — the
/// "you are in grid mode" signal.
pub const GRID_FRAME_FG: Color = GREEN;
/// The caret while a name box (\op \rm \text \latex) is open — green,
/// so the modal layer shows at the cursor itself.
pub const BOX_CURSOR_FG: Color = WHITE;
pub const BOX_CURSOR_BG: Color = GREEN;

// Errors.

/// The minibuffer overlay while the typed name is not a known command.
pub const MINIBUF_BAD_BG: Color = MAROON;

// Neutral grounds.

/// The ground a floating box sits on — the command preview and the
/// completion list alike. Neutral: the box is information, and the
/// formula underneath is what the user is working on.
pub const POPUP_BG: Color = GRAY;

/// The glyph color on any themed ground. The grounds are all dark, so
/// glyphs on them are white no matter what the terminal's own defaults
/// are (a black-on-white theme would otherwise paint black text into a
/// dark green box).
pub const GROUND_FG: Color = WHITE;
