//! Utility functions to easily map and unmap physical memory to virtual memory.
//!
//! It is useful as ACPI tables must me mapped before being read, but are not needed after that.

use x86_64::{
    PhysAddr, VirtAddr,
    structures::paging::{
        Mapper, Page, PageSize, PageTableFlags, PhysFrame, RecursivePageTable, Size4KiB,
    },
};

use crate::mem::{frame_alloc, page_alloc, page_table};

#[derive(Debug)]
/// Physical Mapping structure
///
/// Be careful to only used the original mapped length, as accessing outside
/// could result in undefined behavior if the memory is used by another mapping.
pub struct PhysicalMapping<S: PageSize = Size4KiB>
where
    for<'a> RecursivePageTable<'a>: Mapper<S>,
{
    start_frame: PhysFrame<S>,
    start_page: Page<S>,
    count: u64,
}

pub const FLAGS_MMIO: PageTableFlags = PageTableFlags::PRESENT
    .union(PageTableFlags::WRITABLE)
    .union(PageTableFlags::NO_EXECUTE)
    .union(PageTableFlags::NO_CACHE);

impl<S: PageSize> PhysicalMapping<S>
where
    for<'a> RecursivePageTable<'a>: Mapper<S>,
{
    /// Creates a new physical mapping.
    ///
    /// `flags` will be `OR`ed with `PageTableFlags::PRESENT` to ensure the page is present.
    #[must_use]
    pub fn new(start_paddr: PhysAddr, required_length: usize, flags: PageTableFlags) -> Self
    where
        S: core::fmt::Debug,
    {
        let end_paddr = start_paddr + u64::try_from(required_length).unwrap();

        let start_frame = PhysFrame::<S>::containing_address(start_paddr);
        let end_frame = PhysFrame::<S>::containing_address(end_paddr);

        let frame_range = PhysFrame::range_inclusive(start_frame, end_frame);

        let count = end_frame - start_frame + 1;

        let page_range = page_alloc::with_page_allocator(|page_allocator| {
            page_allocator.allocate_pages::<S>(count).unwrap()
        });

        frame_alloc::with_frame_allocator(|frame_allocator| {
            page_table::with_page_table(|page_table| {
                for (frame, page) in frame_range.zip(page_range) {
                    unsafe {
                        page_table.map_to_with_table_flags(
                            page,
                            frame,
                            flags | PageTableFlags::PRESENT,
                            PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
                            &mut *frame_allocator,
                        )
                    }
                    .unwrap()
                    .flush();
                }
            });
        });

        Self {
            start_frame,
            start_page: page_range.start,
            count,
        }
    }

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
}

impl<S: PageSize> Drop for PhysicalMapping<S>
where
    for<'a> RecursivePageTable<'a>: Mapper<S>,
{
    fn drop(&mut self) {
        // TODO: Is it possible to add frames to the frame allocator pool at some point?
        // We don't need to keep memory reserved for ACPI once we've read the tables.
        // Be careful as the frame could be used by another mapping.
        let page_range =
            Page::<S>::range_inclusive(self.start_page, self.start_page + self.count - 1);

        page_table::with_page_table(|page_table| {
            for page in page_range {
                let (_frame, tlb) = page_table.unmap(page).unwrap();
                tlb.flush();
            }
        });

        page_alloc::with_page_allocator(|page_allocator| {
            page_allocator.free_pages(page_range);
        });
    }
}
