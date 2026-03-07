use beskar_core::video::PixelComponents;

use crate::theme;

/// Visual style applied to a single character cell.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Style {
    pub fg: PixelComponents,
    pub bg: PixelComponents,
}

impl Style {
    pub const DEFAULT: Self = Self {
        fg: theme::FG_PRIMARY,
        bg: theme::BG_PRIMARY,
    };
}

/// A single character cell in the terminal grid.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    pub ch: char,
    pub style: Style,
}

impl Cell {
    pub const BLANK: Self = Self {
        ch: ' ',
        style: Style::DEFAULT,
    };

    #[must_use]
    #[inline]
    pub const fn new(ch: char, style: Style) -> Self {
        Self { ch, style }
    }
}
