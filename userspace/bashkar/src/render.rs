//! Rendering: converts the terminal buffer into pixels and presents frames.

mod compositor;
mod rasterizer;

pub use compositor::Screen;
pub use rasterizer::Rasterizer;
