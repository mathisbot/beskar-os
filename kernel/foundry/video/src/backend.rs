//! Backend rendering trait and error types
use beskar_core::video::{Point, Rect};

pub mod cpu;

/// Backend rendering operations
///
/// Backends are responsible for low-level framebuffer access.
pub trait RenderBackend {
    /// Bitblit (copy) operation from source to destination
    ///
    /// Copies a rectangle from source surface to destination with given position.
    ///
    /// # Safety
    ///
    /// Callers must ensure pointers remain valid for the duration of the operation.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation is not supported or if parameters are invalid.
    unsafe fn bitblt(
        &mut self,
        src_pixels: *const u8,
        src_stride: u16,
        src_rect: Rect,
        dst_pos: Point,
    ) -> Result<(), RenderError>;

    /// Fill a rectangle with a solid color
    ///
    /// # Safety
    ///
    /// Callers must ensure the destination pointer remains valid.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation is not supported or if parameters are invalid.
    unsafe fn fill(&mut self, rect: Rect, color: u32) -> Result<(), RenderError>;
}

/// Rendering errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderError {
    InvalidFormat,
    OutOfBounds,
    OperationFailed,
    SurfaceNotFound,
}

impl core::fmt::Display for RenderError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidFormat => write!(f, "Invalid pixel format"),
            Self::OutOfBounds => write!(f, "Operation out of bounds"),
            Self::OperationFailed => write!(f, "Render operation failed"),
            Self::SurfaceNotFound => write!(f, "Surface not found"),
        }
    }
}
impl core::error::Error for RenderError {}
