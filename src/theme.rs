//! The TUI color scheme, in one place. Every color the interface uses
//! is a named constant here — tweak freely; nothing else in the code
//! hard-codes a color.
//!
//! The palette is deliberately small, and it stays inside the **16
//! ANSI colors** for portability (adr.md roadmap): terminal themes can
//! restyle every one of them, and nothing depends on the 256-color
//! table being honored. (ratatui's plain names are the dark ANSI
//! colors — the backend maps `Red`->DarkRed (1, maroon),
//! `Green`->DarkGreen (2), `Magenta`->DarkMagenta (5, purple); the
//! bright half is the `Light*` variants. ratatui has no `DarkGreen`
//! name of its own.) The caret and
//! the secondary marks use reverse video instead of any fixed color,
//! and text on a fixed themed ground takes a fixed white foreground
//! (`GROUND_FG`) — both for light-terminal safety.
//!
//! - **purple (selection)** — the one selection ground ("this is
//!   selected / the cursor itself"). The secondary kind — the ^F snap
//!   target, ^B's one-step-outward ring — carries no color at all: it
//!   is reverse video, which reads on light and dark terminals alike.
//! - **green (OK / actionable)** — known commands, the grid frame,
//!   insert ghosts, previews, normal messages.
//! - **maroon (error)** — unknown commands, error messages.
//! - **neutral gray (chrome)** — help line, borders, the ␣ glyph, the
//!   ground under a floating box; never carries meaning.

use ratatui::style::Color;

// ----- the palette -----

/// Selection main: selected things and the cursor itself — ANSI 5
/// (purple), the strongest of the grounds.
const PURPLE: Color = Color::Magenta;

/// OK / actionable ground: ANSI 2 (green).
const GREEN_BG: Color = Color::Green;
/// Errors: ANSI 1 (maroon).
const RED_BG: Color = Color::Red;
/// Neutral ground: ANSI 8 (grey).
const GRAY_BG: Color = Color::DarkGray;

// ----- chrome -----

/// Status messages on the bottom line.
pub const MESSAGE_FG: Color = Color::Green;
/// … the same line when the message is an error.
pub const MESSAGE_ERR_FG: Color = Color::Red;
/// The usage/help bottom line and other secondary chrome.
pub const CHROME_FG: Color = Color::DarkGray;
/// The canvas border: the terminal's default foreground, same as the
/// formula text — the frame is part of the page, not chrome.
pub const BORDER_FG: Color = Color::Reset;
/// The visible space atom ␣.
pub const SPACE_FG: Color = Color::DarkGray;

// ----- 紫: cursor & selection -----

/// Selection background — every "this is selected" surface: the linear
/// selection, grid cell rectangles and lanes (told apart by reach, not
/// color), the delimiter pair armed for unwrapping, and ^B's
/// highlighted ancestor. One color, because they are one idea.
pub const SELECTION_BG: Color = PURPLE;

// ----- 緑: OK / actionable -----

/// In-place minibuffer overlay (`\cmd` typed at the cursor) while the
/// name is a known command.
pub const MINIBUF_BG: Color = GREEN_BG;
/// Grid lane-gap cursor: the ghost lane previewing an insertion
/// (Enter inserts here).
pub const GRID_INSERT_BG: Color = GREEN_BG;
/// The edited matrix's lattice frame while grid mode is active — the
/// "you are in grid mode" signal.
pub const GRID_FRAME_FG: Color = Color::Green;
/// The caret while a name box (\op \rm \text \tex) is open — green,
/// so the modal layer shows at the cursor itself. White on dark, like
/// every ground (black on ANSI dark green is unreadable).
pub const BOX_CURSOR_FG: Color = GROUND_FG;
pub const BOX_CURSOR_BG: Color = Color::Green;

/// The minibuffer overlay while the typed name is a *mode* command
/// (\f \b \t \c \q — it moves a mode, not the formula): the
/// selection purple, so the three grounds stay three ideas — green
/// runs an edit, purple changes where/how you stand, red runs nothing.
pub const MINIBUF_MODE_BG: Color = PURPLE;

// ----- 赤: error -----

/// The minibuffer overlay while the typed name is not a known command.
pub const MINIBUF_BAD_BG: Color = RED_BG;

// ----- floating boxes (preview / completion) -----

/// The ground a floating box sits on — the command preview and the
/// completion list alike. Neutral: the box is information, and the
/// formula underneath is what the user is working on.
pub const POPUP_BG: Color = GRAY_BG;

/// …with the list's highlighted row picked out of it. The only thing
/// on that ground that is a choice, so the only thing colored.
pub const POPUP_SEL_BG: Color = PURPLE;

/// The rule of the ground layer: a fill is either one of the dark
/// ANSI colors with white glyphs on it, or reverse video — never a
/// light color, never dark-on-dark. The glyph color on any themed
/// ground: The grounds are all
/// dark, so glyphs on them are white no matter what the terminal's own
/// defaults are (a black-on-white theme would otherwise paint black
/// text into a dark green box).
pub const GROUND_FG: Color = Color::White;
