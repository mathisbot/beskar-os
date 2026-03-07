//! The software compositor managing surfaces, scene graph, and rendering
use crate::backend::{RenderBackend, RenderError};
use beskar_core::video::{FramebufferConfig, Rect, Surface, SurfaceId};
use core::fmt;

/// The software compositor managing surfaces, scene graph, and rendering
pub struct Compositor<B: RenderBackend> {
    framebuffer: FramebufferConfig,
    surfaces: hashbrown::HashMap<SurfaceId, Surface>,
    next_surface_id: u32,
    backend: B,
}

unsafe impl<B: RenderBackend + Send> Send for Compositor<B> {}

impl<B: RenderBackend> Compositor<B> {
    /// Create a new compositor with a framebuffer and rendering backend
    #[must_use]
    pub fn new(framebuffer: FramebufferConfig, backend: B) -> Self {
        Self {
            framebuffer,
            surfaces: hashbrown::HashMap::new(),
            next_surface_id: 1,
            backend,
        }
    }

    #[must_use]
    #[inline]
    pub const fn config(&self) -> &FramebufferConfig {
        &self.framebuffer
    }

    /// Allocate a new offscreen surface with an external buffer
    ///
    /// The kernel is responsible for providing the buffer pointer (e.g., from mmap).
    ///
    /// # Safety
    ///
    /// Caller must ensure:
    /// - `buffer` points to valid, kernel-managed memory
    /// - Buffer remains valid for the lifetime of this Surface
    /// - Buffer has at least `width * height * format.bytes_per_pixel()` bytes
    pub unsafe fn create_surface_with_buffer(
        &mut self,
        x: u16,
        y: u16,
        width: u16,
        height: u16,
        buffer: *mut u8,
    ) -> SurfaceId {
        let id = SurfaceId(self.next_surface_id);
        self.next_surface_id += 1;

        let surface = unsafe {
            Surface::new_with_buffer(
                id,
                x,
                y,
                width,
                height,
                self.framebuffer.info().pixel_format(),
                buffer,
            )
        };
        self.surfaces.insert(id, surface);

        id
    }

    /// Get a mutable reference to a surface
    #[must_use]
    pub fn surface_mut(&mut self, id: SurfaceId) -> Option<&mut Surface> {
        self.surfaces.get_mut(&id)
    }

    /// Get a reference to a surface
    #[must_use]
    pub fn surface(&self, id: SurfaceId) -> Option<&Surface> {
        self.surfaces.get(&id)
    }

    /// Destroy a surface
    ///
    /// # Note
    ///
    /// This does NOT deallocate the buffer - that's the kernel's responsibility.
    /// This only removes the surface from compositor tracking.
    pub fn destroy_surface(&mut self, id: SurfaceId) -> Option<Surface> {
        let surface = self.surfaces.remove(&id)?;
        Some(surface)
    }

    /// Mark a region on a surface as dirty
    ///
    /// This is the primary method for surfaces to notify the compositor of changes.
    /// The dirty region is in surface-local coordinates (0,0 = surface origin).
    ///
    /// # Errors
    ///
    /// Returns `SurfaceNotFound` if the surface with the given ID does not exist.
    pub fn mark_surface_dirty(&mut self, id: SurfaceId, rect: Rect) -> Result<(), CompositorError> {
        let surface = self
            .surface_mut(id)
            .ok_or(CompositorError::SurfaceNotFound)?;

        surface.mark_dirty(rect);

        Ok(())
    }

    /// Mark entire surface as dirty
    ///
    /// # Errors
    ///
    /// Returns `SurfaceNotFound` if the surface with the given ID does not exist
    /// or if backend rendering fails in immediate mode.
    pub fn mark_surface_all_dirty(&mut self, id: SurfaceId) -> Result<(), CompositorError> {
        let surface = self
            .surface_mut(id)
            .ok_or(CompositorError::SurfaceNotFound)?;

        surface.mark_all_dirty();

        Ok(())
    }

    /// Render a single surface only
    ///
    /// This renders only the specified surface to the framebuffer, without
    /// accessing any other surfaces.
    ///
    /// # Errors
    ///
    /// Returns an error if the surface doesn't exist or backend rendering fails.
    pub fn render_surface(&mut self, id: SurfaceId, rect: Rect) -> Result<(), RenderError> {
        let surface = self.surface(id).ok_or(RenderError::SurfaceNotFound)?;

        // Calculate destination on framebuffer
        let dst_pos = beskar_core::video::Point::new(
            surface.x() + rect.top_left.x,
            surface.y() + rect.top_left.y,
        );

        // Bitblt from surface to framebuffer
        unsafe {
            self.backend
                .bitblt(surface.buffer_ptr(), surface.stride_bytes(), rect, dst_pos)?;
        }

        Ok(())
    }

    /// Render only the dirty region of a surface
    ///
    /// # Errors
    ///
    /// Returns an error if the surface doesn't exist or backend rendering fails.
    pub fn render_surface_dirty(&mut self, id: SurfaceId) -> Result<(), RenderError> {
        let surface = self.surface_mut(id).ok_or(RenderError::SurfaceNotFound)?;

        let Some(rect) = surface.take_dirty() else {
            return Ok(()); // No damage to render
        };

        // Calculate destination on framebuffer
        let dst_pos = beskar_core::video::Point::new(
            surface.x() + rect.top_left.x,
            surface.y() + rect.top_left.y,
        );

        let surface_ptr = surface.buffer_ptr();
        let stride = surface.stride_bytes();

        // Bitblt from surface to framebuffer
        unsafe { self.backend.bitblt(surface_ptr, stride, rect, dst_pos) }?;

        Ok(())
    }
}

/// Compositor errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompositorError {
    SurfaceNotFound,
    RenderError(RenderError),
}

impl fmt::Display for CompositorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SurfaceNotFound => write!(f, "Surface not found"),
            Self::RenderError(e) => write!(f, "Render error: {e}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::cpu::CpuBackend;
    use beskar_core::arch::VirtAddr;
    use beskar_core::video::PixelFormat;

    #[test]
    fn test_compositor_surface_lifecycle() {
        let fb_config =
            FramebufferConfig::new(600, 600, 600, PixelFormat::Argb8888, VirtAddr::ZERO);
        let fb_info = fb_config.info();

        let backend = unsafe { CpuBackend::new(VirtAddr::ZERO.as_mut_ptr(), fb_info) };
        let mut comp = Compositor::new(fb_config, backend);

        // Create a test buffer
        let mut buffer = [0u8; 100 * 100 * 4];
        let sid = unsafe { comp.create_surface_with_buffer(0, 0, 100, 100, buffer.as_mut_ptr()) };

        assert!(comp.surface(sid).is_some());

        comp.destroy_surface(sid);
        assert!(comp.surface(sid).is_none());
    }

    #[test]
    fn test_damage_tracking() {
        let fb_config =
            FramebufferConfig::new(600, 600, 600, PixelFormat::Argb8888, VirtAddr::ZERO);
        let fb_info = fb_config.info();

        let backend = unsafe { CpuBackend::new(VirtAddr::ZERO.as_mut_ptr(), fb_info) };
        let mut comp = Compositor::new(fb_config, backend);

        let mut buffer = [0u8; 100 * 100 * 4];
        let sid = unsafe { comp.create_surface_with_buffer(0, 0, 100, 100, buffer.as_mut_ptr()) };

        // Mark damage
        comp.mark_surface_dirty(sid, Rect::new(0, 0, 50, 50))
            .unwrap();
        assert_eq!(
            comp.surface(sid).unwrap().dirty_rect(),
            Some(Rect::new(0, 0, 50, 50))
        );

        comp.mark_surface_dirty(sid, Rect::new(50, 50, 50, 50))
            .unwrap();
        assert_eq!(
            comp.surface(sid).unwrap().dirty_rect(),
            Some(Rect::new(0, 0, 100, 100))
        );
    }
}
