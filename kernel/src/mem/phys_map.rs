//! Physical-to-virtual mapping helpers for MMIO and firmware-owned regions.

use crate::mem::vmm;
use beskar_core::arch::{
    PhysAddr, VirtAddr,
    paging::{Frame, M4KiB, Mapper, MappingError, MemSize, Page},
};
use beskar_hal::paging::page_table::{Flags, PageTable};

#[derive(Debug)]
/// Physical Mapping structure
///
/// Be careful to only use the original mapped length, as accessing outside
/// could result in undefined behavior if the memory is used by another mapping.
pub struct PhysicalMapping<S: MemSize = M4KiB>
where
    for<'a> PageTable<'a>: Mapper<S, Flags>,
{
    start_frame: Frame<S>,
    start_page: Page<S>,
    count: u64,
}

impl<S: MemSize> PhysicalMapping<S>
where
    for<'a> PageTable<'a>: Mapper<S, Flags>,
{
    const EMPTY: Self = Self {
        start_frame: Frame::containing_address(PhysAddr::ZERO),
        start_page: Page::containing_address(VirtAddr::ZERO),
        count: 0,
    };

    /// Creates a new physical mapping.
    ///
    /// `flags` will be `OR`ed with `PageTableFlags::PRESENT` to ensure the page is present.
    pub fn new(
        start_paddr: PhysAddr,
        required_length: usize,
        flags: Flags,
    ) -> Result<Self, MappingError<S>> {
        if required_length == 0 {
            return Ok(Self::EMPTY);
        }

        let end_paddr = start_paddr + (u64::try_from(required_length).unwrap() - 1);

        let start_frame = Frame::<S>::containing_address(start_paddr);
        let end_frame = Frame::<S>::containing_address(end_paddr);

        let frame_range = Frame::range_inclusive(start_frame, end_frame);

        let count = end_frame - start_frame + 1;

        let page_range =
            vmm::kernel::reserve_pages::<S>(count).ok_or(MappingError::FrameAllocationFailed)?;

        for (frame, page) in frame_range.clone().into_iter().zip(page_range) {
            let res = vmm::kernel::map_frame(page, frame, flags);

            if let Err(e) = res {
                // Unmap any pages that were successfully mapped before the failure.
                for (_prev_frame, prev_page) in frame_range
                    .into_iter()
                    .zip(page_range)
                    .take_while(|(f, _)| *f <= frame)
                {
                    let _ = vmm::kernel::unmap_page(prev_page);
                }

                return Err(e);
            }
        }

        Ok(Self {
            start_frame,
            start_page: page_range.start(),
            count,
        })
    }

    /// Translate a physical address to a virtual address within the mapping.
    pub fn translate(&self, addr: PhysAddr) -> Option<VirtAddr> {
        if addr < self.start_frame.start_address() {
            return None;
        }

        let offset = addr - self.start_frame.start_address();
        if offset >= self.count * S::SIZE {
            return None;
        }

        Some(self.start_page.start_address() + offset)
    }

    /// Translate a physical range to a virtual address within the mapping.
    /// Unlike `Self::translate`, this function has the additional guarantee that the
    /// whole range is mapped.
    pub fn translate_range(&self, addr: PhysAddr, length: u64) -> Option<VirtAddr> {
        if addr < self.start_frame.start_address() {
            return None;
        }

        let offset = addr - self.start_frame.start_address();
        if offset + length > self.count * S::SIZE {
            return None;
        }

        Some(self.start_page.start_address() + offset)
    }

    #[must_use]
    #[inline]
    pub const fn start_frame(&self) -> Frame<S> {
        self.start_frame
    }
}

impl<S: MemSize> Drop for PhysicalMapping<S>
where
    for<'a> PageTable<'a>: Mapper<S, Flags>,
{
    fn drop(&mut self) {
        if self.count == 0 {
            return;
        }

        let page_range =
            Page::<S>::range_inclusive(self.start_page, self.start_page + self.count - 1);
        for page in page_range {
            let _ = vmm::kernel::unmap_page(page);
        }
        vmm::kernel::free_pages(page_range);
    }
}

impl<S: MemSize + core::fmt::Debug> driver_api::PhysicalMapper<S> for PhysicalMapping<S>
where
    for<'a> PageTable<'a>: Mapper<S, Flags>,
{
    #[inline]
    fn new(start_paddr: PhysAddr, required_length: usize, flags: Flags) -> Self {
        Self::new(start_paddr, required_length, flags).unwrap()
    }

    #[inline]
    fn translate(&self, paddr: PhysAddr) -> Option<VirtAddr> {
        self.translate(paddr)
    }
}
