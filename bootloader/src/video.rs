//! Responsible for initializiing graphical output

use beskar_core::{
    arch::{PhysAddr, VirtAddr},
    video::{FrameBuffer, Info, Pixel},
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
        unsafe { FrameBuffer::new(vaddr, self.info) }
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
