//! Utility functions to easily map and unmap physical memory to virtual memory.
//!
//! It is useful as ACPI tables must me mapped before being read, but are not needed after that.

use crate::{mem::frame_alloc, process};
use beskar_core::arch::{
    PhysAddr, VirtAddr,
    paging::{CacheFlush as _, Flags as FlagsTrait, Frame, M4KiB, Mapper, MemSize, Page},
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
    /// Creates a new physical mapping.
    ///
    /// `flags` will be `OR`ed with `PageTableFlags::PRESENT` to ensure the page is present.
    #[must_use]
    pub fn new(start_paddr: PhysAddr, required_length: usize, flags: Flags) -> Self
    where
        S: core::fmt::Debug,
    {
        let end_paddr = start_paddr + u64::try_from(required_length).unwrap();

        let start_frame = Frame::<S>::containing_address(start_paddr);
        let end_frame = Frame::<S>::containing_address(end_paddr);

        let frame_range = Frame::range_inclusive(start_frame, end_frame);

        let count = end_frame - start_frame + 1;

        let page_range = process::current()
            .address_space()
            .with_pgalloc(|page_allocator| page_allocator.allocate_pages::<S>(count).unwrap());

        frame_alloc::with_frame_allocator(|frame_allocator| {
            process::current()
                .address_space()
                .with_page_table(|page_table| {
                    for (frame, page) in frame_range.into_iter().zip(page_range) {
                        page_table
                            .map(page, frame, flags | Flags::PRESENT, frame_allocator)
                            .flush();
                    }
                });
        });

        Self {
            start_frame,
            start_page: page_range.start(),
            count,
        }
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
        let page_range =
            Page::<S>::range_inclusive(self.start_page, self.start_page + self.count - 1);

        process::current()
            .address_space()
            .with_page_table(|page_table| {
                for page in page_range {
                    let (_frame, tlb) = page_table.unmap(page).unwrap();
                    tlb.flush();
                }
            });

        process::current()
            .address_space()
            .with_pgalloc(|page_allocator| {
                page_allocator.free_pages(page_range);
            });
    }
}
