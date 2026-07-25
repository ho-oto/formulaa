//! The TUI color scheme, in one place. Every color the interface uses
//! is a named constant here — tweak freely; nothing else in the code
//! hard-codes a color.

use ratatui::style::Color;

// ----- chrome -----

/// Status messages on the bottom line.
pub const MESSAGE_FG: Color = Color::Green;
/// The usage/help bottom line and other secondary chrome.
pub const CHROME_FG: Color = Color::DarkGray;
/// The canvas border.
pub const BORDER_FG: Color = Color::DarkGray;
/// The visible space atom ␣.
pub const SPACE_FG: Color = Color::DarkGray;

// ----- cursor & selection -----

/// Selection box background.
pub const SELECTION_BG: Color = Color::Indexed(89);
/// The free cursor cell itself (^F): tinted so the mode is obvious.
pub const FREE_CURSOR_BG: Color = Color::Indexed(127);
/// Glyph color on the free cursor cell.
pub const FREE_CURSOR_FG: Color = Color::White;
/// The ^F snap-preview cell (the free cursor uses the caret style).
pub const FREE_BG: Color = Color::Indexed(24);
/// In-place minibuffer overlay (`\cmd` typed at the cursor).
pub const MINIBUF_BG: Color = Color::Indexed(94);
/// … the same overlay while the typed name is not a known command.
pub const MINIBUF_BAD_BG: Color = Color::Indexed(88);

// ----- jump / block markers -----

/// Marker label glyphs (jump labels, block labels).
pub const LABEL_FG: Color = Color::Black;
pub const LABEL_BG: Color = Color::Yellow;
/// Unlabeled jump markers (beyond the label alphabet).
pub const UNLABELED_BG: Color = Color::Indexed(238);
/// The arrow-selected jump / block marker.
pub const SELECTED_BG: Color = Color::Indexed(172);

/// Background palette for ^B ancestor boxes: a blue→cyan gradient from
/// the innermost parent outward, cycling if the nesting runs deeper.
/// The highlighted ancestor overrides with SELECTED_BG.
pub const DEPTH_BG: [Color; 5] = [
    Color::Indexed(17), // darkest blue (innermost parent)
    Color::Indexed(19),
    Color::Indexed(25),
    Color::Indexed(31),
    Color::Indexed(37), // cyan (outermost)
];
