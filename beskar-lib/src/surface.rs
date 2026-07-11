//! Compositor surface API
//!
//! This module provides a high-level interface to create and manage
//! compositor surfaces for graphics output.
//!
//! # Usage
//!
//! ```rust,no_run
//! let width = 800;
//! let height = 600;
//!
//! let mut buffer = vec![Pixel::BLACK; width * height];
//! let mut surface = Surface::create(width, height, 0, 0, buffer.as_mut_ptr())?;
//!
//! // Modify pixels in buffer...
//! buffer[100] = Pixel::WHITE;
//!
//! // Notify compositor of changes
//! surface.present_region(1, 1, 100, 0)?;
//! ```

use beskar_core::video::Pixel;
use core::mem::MaybeUninit;

use crate::error::{SurfaceError, SurfaceErrorKind};

/// An offscreen rendering surface managed by the kernel compositor
///
/// Surfaces hold a buffer of pixels and communicate with the kernel
/// compositor to get rendered to the screen.
pub struct Surface {
    width: u16,
    height: u16,
}

impl Surface {
    /// Create a new surface with the given dimensions and buffer
    ///
    /// # Errors
    ///
    /// Returns an error if the syscall fails or arguments are invalid
    ///
    /// # Safety
    ///
    /// Caller must ensure the buffer pointer is valid and contains
    /// at least `width * height * 4` bytes of valid memory.
    pub unsafe fn create(
        width: u16,
        height: u16,
        x: u16,
        y: u16,
        buffer: *mut Pixel,
    ) -> Result<Self, SurfaceError> {
        if width == 0 || height == 0 {
            return Err(SurfaceError::new(SurfaceErrorKind::InvalidDimensions));
        }

        let res = crate::sys::sc_surface_create(width, height, x, y, buffer as *const _);
        if !res.is_success() {
            return Err(SurfaceError::new(SurfaceErrorKind::SyscallFailed));
        }

        Ok(Self { width, height })
    }

    /// Get the surface width in pixels
    #[must_use]
    #[inline]
    pub const fn width(&self) -> u16 {
        self.width
    }

    /// Get the surface height in pixels
    #[must_use]
    #[inline]
    pub const fn height(&self) -> u16 {
        self.height
    }

    #[inline]
    /// Mark a rectangular region of the surface as dirty/modified.
    ///
    /// # Errors
    ///
    /// Returns an error if the syscall fails or the region is invalid.
    pub fn mark_dirty(&self, x: u16, y: u16, width: u16, height: u16) -> Result<(), SurfaceError> {
        if x.saturating_add(width) > self.width || y.saturating_add(height) > self.height {
            return Err(SurfaceError::new(SurfaceErrorKind::InvalidDimensions));
        }

        let code = crate::sys::sc_surface_dirty(width, height, x, y);

        if code == beskar_core::syscall::SyscallExitCode::Success {
            Ok(())
        } else {
            Err(SurfaceError::new(SurfaceErrorKind::SyscallFailed))
        }
    }

    #[inline]
    /// Mark the entire surface as dirty/modified.
    ///
    /// # Errors
    ///
    /// Returns an error if the syscall fails.
    pub fn mark_all_dirty(&self) -> Result<(), SurfaceError> {
        self.mark_dirty(0, 0, self.width, self.height)
    }

    #[inline]
    /// Present only the dirty regions of the surface to the screen.
    ///
    /// # Errors
    ///
    /// Returns an error if the syscall fails.
    pub fn present_dirty(&self) -> Result<(), SurfaceError> {
        let code = crate::sys::sc_surface_present(false);
        if code == beskar_core::syscall::SyscallExitCode::Success {
            Ok(())
        } else {
            Err(SurfaceError::new(SurfaceErrorKind::SyscallFailed))
        }
    }

    #[inline]
    /// Mark a region of the surface as dirty and present it to the screen.
    ///
    /// # Errors
    ///
    /// Returns an error if the syscall fails or the region is invalid.
    pub fn present_region(
        &self,
        x: u16,
        y: u16,
        width: u16,
        height: u16,
    ) -> Result<(), SurfaceError> {
        self.mark_dirty(x, y, width, height)?;
        self.present_dirty()
    }

    #[inline]
    /// Mark the entire surface as dirty and present it to the screen.
    ///
    /// # Errors
    ///
    /// Returns an error if the syscall fails.
    pub fn present_all(&self) -> Result<(), SurfaceError> {
        let code = crate::sys::sc_surface_present(true);
        if code == beskar_core::syscall::SyscallExitCode::Success {
            Ok(())
        } else {
            Err(SurfaceError::new(SurfaceErrorKind::SyscallFailed))
        }
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        let _ = crate::sys::sc_surface_destroy();
    }
}

/// Query the screen information.
///
/// # Errors
///
/// Returns an error if the syscall fails.
pub fn query_screen_info() -> Result<beskar_core::video::Info, SurfaceError> {
    let mut info = MaybeUninit::<beskar_core::video::Info>::uninit();
    let code = crate::sys::sc_query_config(
        beskar_core::syscall::consts::QUERY_FRAMEBUFFER,
        info.as_mut_ptr().cast(),
        core::mem::size_of::<beskar_core::video::Info>() as u64,
    );
    if code == beskar_core::syscall::SyscallExitCode::Success {
        Ok(unsafe { info.assume_init() })
    } else {
        Err(SurfaceError::new(SurfaceErrorKind::SyscallFailed))
    }
}
