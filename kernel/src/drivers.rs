#![expect(dead_code, reason = "Drivers are not fully implemented yet")]
pub mod acpi;
pub mod hpet;
pub mod keyboard;
pub mod nic;
mod pci;
pub mod ps2;
pub mod storage;
pub mod tsc;
pub mod usb;

use crate::mem::vmm;
use beskar_core::arch::paging::{Frame, M4KiB, MemSize as _, Page};
use beskar_core::drivers::{DriverError, DriverResult};
use beskar_hal::paging::page_table::Flags;

pub extern "C" fn init() -> ! {
    let pci_init_result = pci::init();
    if pci_init_result.is_err() {
        crate::warn!("PCI initialization failed");
    }

    // TODO: Start each driver's process when needed

    let _ = keyboard::init();

    #[cfg(target_arch = "x86_64")]
    let _ = ps2::init();

    let _ = storage::init();
    let _ = usb::init();
    let _ = nic::init();

    unsafe { crate::process::scheduler::exit_current_thread() };
}

struct DmaPage {
    page: Page<M4KiB>,
    frame: Frame<M4KiB>,
    size: usize,
}

impl DmaPage {
    pub fn new(size: usize) -> DriverResult<Self> {
        if size > usize::try_from(M4KiB::SIZE).unwrap() {
            return Err(DriverError::Invalid);
        }

        let Some(page) = vmm::kernel::reserve_pages::<M4KiB>(1).map(|range| range.start()) else {
            return Err(DriverError::Unknown);
        };
        let page_range = Page::range_inclusive(page, page);

        let Some(frame) = vmm::kernel::alloc_frame::<M4KiB>() else {
            vmm::kernel::free_pages(page_range);
            return Err(DriverError::Unknown);
        };

        if vmm::kernel::map_frame(page, frame, Flags::MMIO_SUITABLE).is_err() {
            vmm::kernel::free_frame(frame);
            vmm::kernel::free_pages(page_range);
            return Err(DriverError::Unknown);
        }

        let dma = Self { page, frame, size };
        dma.clear();

        Ok(dma)
    }

    #[must_use]
    #[inline]
    pub const fn len(&self) -> usize {
        self.size
    }

    #[must_use]
    #[inline]
    pub const fn frame(&self) -> Frame<M4KiB> {
        self.frame
    }

    #[must_use]
    #[inline]
    pub const fn phys_addr(&self) -> beskar_core::arch::PhysAddr {
        self.frame.start_address()
    }

    #[must_use]
    #[inline]
    pub const fn as_ptr<T>(&self) -> *const T {
        self.page.start_address().as_ptr::<T>()
    }

    #[must_use]
    #[inline]
    pub const fn as_mut_ptr<T>(&self) -> *mut T {
        self.page.start_address().as_mut_ptr::<T>()
    }

    #[inline]
    pub const fn clear(&self) {
        unsafe { core::ptr::write_bytes(self.as_mut_ptr::<u8>(), 0, self.size) };
    }

    pub fn copy_from_slice(&self, src: &[u8]) {
        debug_assert_eq!(src.len(), self.size);
        unsafe {
            core::ptr::copy_nonoverlapping(
                src.as_ptr(),
                self.as_mut_ptr::<u8>(),
                self.size.min(src.len()),
            );
        }
    }

    pub fn copy_to_slice(&self, dst: &mut [u8]) {
        debug_assert_eq!(dst.len(), self.size);
        unsafe {
            core::ptr::copy_nonoverlapping(
                self.as_mut_ptr::<u8>().cast_const(),
                dst.as_mut_ptr(),
                self.size.min(dst.len()),
            );
        }
    }

    #[must_use]
    #[inline]
    pub const fn as_bytes(&self) -> &[u8] {
        unsafe { core::slice::from_raw_parts(self.as_ptr::<u8>(), self.size) }
    }
}

impl Drop for DmaPage {
    fn drop(&mut self) {
        let res = vmm::kernel::unmap_page(self.page);
        debug_assert_eq!(res, Ok(self.frame));
        vmm::kernel::free_frame(self.frame);
        vmm::kernel::free_pages(Page::range_inclusive(self.page, self.page));
    }
}
