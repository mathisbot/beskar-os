//! Responsible for initializiing graphical output

use beskar_core::{
    arch::commons::{PhysAddr, VirtAddr},
    video::{Info, Pixel},
};
use hyperdrive::locks::mcs::MUMcsLock;

pub mod gop;

static PHYSICAL_FB: MUMcsLock<PhysicalFrameBuffer> = MUMcsLock::uninit();

pub fn init() {
    let p_fb = gop::init();
    PHYSICAL_FB.init(p_fb);
}

#[derive(Debug)]
/// Represents a framebuffer with its physical address.
pub struct PhysicalFrameBuffer {
    start_addr: PhysAddr,
    info: Info,
}

impl PhysicalFrameBuffer {
    #[must_use]
    #[inline]
    pub const fn start_addr(&self) -> PhysAddr {
        self.start_addr
    }

    #[must_use]
    #[inline]
    pub const fn info(&self) -> Info {
        self.info
    }

    #[must_use]
    pub const fn start_addr_as_virtual(&self) -> VirtAddr {
        VirtAddr::new(self.start_addr.as_u64())
    }

    #[must_use]
    /// Access the raw bytes of the framebuffer as a mutable slice.
    pub const fn buffer_mut(&mut self) -> &mut [Pixel] {
        unsafe {
            core::slice::from_raw_parts_mut(
                self.start_addr_as_virtual().as_mut_ptr::<Pixel>(),
                self.info().size() / self.info().bytes_per_pixel(),
            )
        }
    }

    #[must_use]
    #[inline]
    /// Converts the physical framebuffer to a (virtual) framebuffer.
    ///
    /// ## Safety
    ///
    /// The provided framebuffer must only be used to transfer the framebuffer to the kernel.
    pub const unsafe fn to_framebuffer(&self, vaddr: VirtAddr) -> FrameBuffer {
        FrameBuffer {
            buffer_start: vaddr,
            info: self.info,
        }
    }
}

/// Represents a frambuffer.
///
/// This is the struct that is sent to the kernel.
#[derive(Debug)]
pub struct FrameBuffer {
    buffer_start: VirtAddr,
    info: Info,
}

impl FrameBuffer {
    #[must_use]
    #[inline]
    /// Creates a new framebuffer instance.
    ///
    /// ## Safety
    ///
    /// The given start address and info must describe a valid framebuffer.
    pub const unsafe fn new(start_addr: VirtAddr, info: Info) -> Self {
        Self {
            buffer_start: start_addr,
            info,
        }
    }

    #[must_use]
    #[inline]
    /// Returns layout and pixel format information of the framebuffer.
    pub const fn info(&self) -> Info {
        self.info
    }

    #[must_use]
    #[inline]
    /// Access the raw bytes of the framebuffer as a mutable slice.
    pub const fn buffer_mut(&mut self) -> &mut [u8] {
        unsafe {
            core::slice::from_raw_parts_mut(
                self.buffer_start.as_mut_ptr::<u8>(),
                self.info().size(),
            )
        }
    }
}

#[inline]
pub fn clear_screen() {
    with_physical_framebuffer(|fb| {
        fb.buffer_mut().fill(Pixel::BLACK);
    });
}

pub fn with_physical_framebuffer<T, F: FnOnce(&mut PhysicalFrameBuffer) -> T>(f: F) -> T {
    PHYSICAL_FB.with_locked(f)
}
