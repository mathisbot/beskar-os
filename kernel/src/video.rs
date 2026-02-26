//! Kernel video subsystem integration
//!
//! # Architecture
//!
//! - Compositor is stored in a lock (MUMcsLock) for safe concurrent access
//! - Surfaces are created via syscalls or kernel APIs
//! - The kernel decides when to render (via render policies)
//!
//! # Render policies
//!
//! - `Immediate`: Render on every change
//! - `OnRequest`: Render only when explicitly requested (default)
//! - `Deferred`: Render on timer/vsync

use beskar_core::video::FramebufferConfig;
use hyperdrive::locks::mcs::MUMcsLock;
use video::{backend::cpu::CpuBackend, compositor::Compositor};

static COMPOSITOR: MUMcsLock<Compositor<CpuBackend>> = MUMcsLock::uninit();

/// Initialize the video subsystem with a framebuffer configuration
pub fn init(config: &mut FramebufferConfig) {
    let info = config.info();
    let cpu_backend = unsafe { CpuBackend::new(config.buffer_ptr(), info) };
    let compositor = Compositor::new(config.clone(), cpu_backend);
    COMPOSITOR.init(compositor);
}

pub fn with_compositor<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut Compositor<CpuBackend>) -> R,
{
    COMPOSITOR.with_locked_if_init(f)
}

/// Guard for a surface that automatically destroys it when dropped
///
/// This is used to ensure surfaces are properly cleaned up when a process
/// terminates, preventing resource leaks.
#[derive(Clone, Debug)]
pub struct SurfaceGuard(pub beskar_core::video::SurfaceId);

impl Drop for SurfaceGuard {
    fn drop(&mut self) {
        // Destroy the surface when the guard is dropped
        let _ = with_compositor(|c| c.destroy_surface(self.0));
    }
}
