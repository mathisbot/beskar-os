//! Terminal buffer: a grid of styled cells with scrollback.

mod cell;
mod grid;

pub use cell::{Cell, Style};
pub use grid::TermBuffer;
