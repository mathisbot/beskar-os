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

use beskar_core::video::{FramebufferConfig, SurfaceId};
use core::sync::atomic::{AtomicU64, Ordering};
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

pub struct AtomicOptionSurfaceGuard(AtomicU64);

impl AtomicOptionSurfaceGuard {
    const OPTION_MASK: u64 = 0x1_0000_0000;
    const VALUE_MASK: u64 = 0xFFFF_FFFF;

    #[must_use]
    #[inline]
    fn encode_raw(option: Option<SurfaceId>) -> u64 {
        option.map_or(0, |id| u64::from(id.0) | Self::OPTION_MASK)
    }
    #[must_use]
    #[inline]
    fn decode_raw(raw: u64) -> Option<SurfaceId> {
        debug_assert_eq!(raw & !(Self::OPTION_MASK | Self::VALUE_MASK), 0);
        if raw & Self::OPTION_MASK != 0 {
            let id = u32::try_from(raw & Self::VALUE_MASK).unwrap();
            Some(SurfaceId(id))
        } else {
            None
        }
    }

    #[must_use]
    #[inline]
    pub fn new(surface: Option<SurfaceId>) -> Self {
        let raw = Self::encode_raw(surface);
        Self(AtomicU64::new(raw))
    }

    #[must_use]
    #[inline]
    pub fn load(&self, order: Ordering) -> Option<SurfaceId> {
        let raw = self.0.load(order);
        Self::decode_raw(raw)
    }

    // #[inline]
    // pub fn store(&self, surface: Option<SurfaceId>, order: Ordering) {
    //     let raw = Self::encode_raw(surface);
    //     self.0.store(raw, order);
    // }

    #[inline]
    pub fn swap(&self, surface: Option<SurfaceId>, order: Ordering) -> Option<SurfaceId> {
        let raw = Self::encode_raw(surface);
        let raw = self.0.swap(raw, order);
        Self::decode_raw(raw)
    }

    pub fn compare_exchange(
        &self,
        current: Option<SurfaceId>,
        new: Option<SurfaceId>,
        success: Ordering,
        failure: Ordering,
    ) -> Result<Option<SurfaceId>, Option<SurfaceId>> {
        let current_raw = Self::encode_raw(current);
        let new_raw = Self::encode_raw(new);
        let res = self
            .0
            .compare_exchange(current_raw, new_raw, success, failure);
        match res {
            Ok(_) => Ok(current),
            Err(actual) => Err(Self::decode_raw(actual)),
        }
    }
}

impl Drop for AtomicOptionSurfaceGuard {
    fn drop(&mut self) {
        if let Some(surface_id) = self.load(Ordering::Acquire) {
            drop(SurfaceGuard(surface_id));
        }
    }
}
