//! Color palette for the terminal.

use beskar_core::video::PixelComponents;

pub const BG_PRIMARY: PixelComponents = PixelComponents::new(0x08, 0x0C, 0x14, 0xFF);
pub const BG_STATUS_BAR: PixelComponents = PixelComponents::new(0x10, 0x18, 0x28, 0xFF);
pub const BG_CURSOR: PixelComponents = PixelComponents::new(0xB0, 0xD0, 0xE8, 0xFF);

pub const FG_PRIMARY: PixelComponents = PixelComponents::new(0xC8, 0xD8, 0xE8, 0xFF);
pub const FG_DIMMED: PixelComponents = PixelComponents::new(0x58, 0x68, 0x78, 0xFF);

pub const ACCENT_CYAN: PixelComponents = PixelComponents::new(0x5C, 0xC4, 0xD4, 0xFF);
pub const ACCENT_AMBER: PixelComponents = PixelComponents::new(0xD4, 0x9C, 0x3C, 0xFF);
pub const ACCENT_RED: PixelComponents = PixelComponents::new(0xD4, 0x44, 0x44, 0xFF);

pub const PROMPT_USER: PixelComponents = ACCENT_CYAN;
pub const PROMPT_SEPARATOR: PixelComponents = FG_DIMMED;
pub const PROMPT_CHEVRON: PixelComponents = PixelComponents::new(0x40, 0x90, 0xA0, 0xFF);

pub const STATUS_FG: PixelComponents = PixelComponents::new(0x78, 0x90, 0xA8, 0xFF);
pub const STATUS_LABEL: PixelComponents = ACCENT_CYAN;
