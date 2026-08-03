//! The TUI color scheme, in one place. Every color the interface uses
//! is a named constant here — tweak freely; nothing else in the code
//! hard-codes a color.
//!
//! The palette is deliberately small (docs/design.md カラーパレット):
//!
//! - **紫 (selection)** — one main purple for "this is selected / the
//!   cursor itself", two softer purples for secondary selection info
//!   (the ^F snap target, the ^B ancestor alternation).
//! - **緑 (OK / actionable)** — known commands, jump markers, the grid
//!   frame, insert ghosts, previews, normal messages.
//! - **赤 (error)** — unknown commands, error messages.
//! - **中立灰 (chrome)** — help line, borders, the ␣ glyph, unlabeled
//!   markers; never carries meaning.
//!
//! Label glyphs are black on the bright green marker background.

use ratatui::style::Color;

// ----- the palette -----

/// 紫メイン: selected things and the cursor itself — the deepest of
/// the three, so "selected" always reads strongest.
const PURPLE: Color = Color::Indexed(54);
/// 紫サブ (明): secondary selection info.
const PURPLE_SOFT: Color = Color::Indexed(97);
/// 紫サブ (淡): secondary selection info, the quieter alternation
/// partner.
const PURPLE_DIM: Color = Color::Indexed(60);
/// 緑 (背景): OK / actionable ground.
const GREEN_BG: Color = Color::Indexed(22);
/// 緑 (背景・明): the marker labels' ground — pops over GREEN_BG.
const GREEN_BRIGHT_BG: Color = Color::Indexed(28);
/// 赤 (背景): errors.
const RED_BG: Color = Color::Indexed(88);
/// 中立灰 (背景).
const GRAY_BG: Color = Color::Indexed(238);

// ----- chrome -----

/// Status messages on the bottom line.
pub const MESSAGE_FG: Color = Color::Green;
/// … the same line when the message is an error.
pub const MESSAGE_ERR_FG: Color = Color::Red;
/// The usage/help bottom line and other secondary chrome.
pub const CHROME_FG: Color = Color::DarkGray;
/// The canvas border.
pub const BORDER_FG: Color = Color::DarkGray;
/// The visible space atom ␣.
pub const SPACE_FG: Color = Color::DarkGray;

/// Struck (\cancel) glyphs: the strike is the terminal's own
/// crossed-out attribute plus this tint — combining overlays (U+0338)
/// stay out of the TUI cells (fonts draw astral char + combining pairs
/// wider than one cell and the overflow smears under partial repaints).
pub const CANCEL_FG: Color = Color::Red;

// ----- 紫: cursor & selection -----

/// Selection background — every "this is selected" surface: the linear
/// selection, grid cell rectangles and lanes (told apart by reach, not
/// color), the arrow-selected marker.
pub const SELECTION_BG: Color = PURPLE;
/// The free cursor cell itself (^F): tinted so the mode is obvious.
pub const FREE_CURSOR_BG: Color = PURPLE;
/// Glyph color on the free cursor cell.
pub const FREE_CURSOR_FG: Color = Color::White;
/// The ^F snap-preview cell (the free cursor uses the caret style).
pub const FREE_BG: Color = PURPLE_SOFT;
/// The arrow-selected jump / block marker.
pub const SELECTED_BG: Color = PURPLE;
/// Background alternation for ^B ancestor boxes, innermost parent
/// first, cycling outward. The highlighted ancestor overrides with
/// SELECTED_BG.
pub const DEPTH_BG: [Color; 2] = [PURPLE_DIM, PURPLE_SOFT];

// ----- 緑: OK / actionable -----

/// In-place minibuffer overlay (`\cmd` typed at the cursor) while the
/// name is a known command.
pub const MINIBUF_BG: Color = GREEN_BG;
/// Marker label glyphs (jump labels, block labels).
pub const LABEL_FG: Color = Color::Black;
pub const LABEL_BG: Color = GREEN_BRIGHT_BG;
/// Grid lane-gap cursor: the ghost lane previewing an insertion
/// (Enter inserts here).
pub const GRID_INSERT_BG: Color = GREEN_BG;
/// The edited matrix's lattice frame while grid mode is active — the
/// "you are in grid mode" signal.
pub const GRID_FRAME_FG: Color = Color::Green;
/// The caret while a name box (\op \rm \text \tex) is open — green,
/// so the modal layer shows at the cursor itself.
pub const BOX_CURSOR_FG: Color = Color::Black;
pub const BOX_CURSOR_BG: Color = Color::Green;

// ----- 赤: error -----

/// The minibuffer overlay while the typed name is not a known command.
pub const MINIBUF_BAD_BG: Color = RED_BG;

// ----- 中立灰 -----

/// Unlabeled jump markers (beyond the label alphabet).
pub const UNLABELED_BG: Color = GRAY_BG;
/// The live preview of what committing the command would insert —
/// neutral ground: it is information, not yet an action.
pub const PREVIEW_BG: Color = GRAY_BG;
