use beskar_core::{
    arch::{
        VirtAddr,
        paging::{M4KiB, MemSize, Page, PageRangeInclusive},
    },
    mem::ranges::{MemoryRange, MemoryRanges},
};

pub mod pmap;

#[derive(Debug)]
pub struct PageAllocator<const N: usize> {
    vranges: MemoryRanges<N>,
}

impl<const N: usize> PageAllocator<N> {
    #[must_use]
    #[inline]
    pub fn new_empty() -> Self {
        Self {
            vranges: MemoryRanges::default(),
        }
    }

    #[must_use]
    #[inline]
    pub fn new_range(start: VirtAddr, end: VirtAddr) -> Self {
        let mut vaddrs = MemoryRanges::new();
        vaddrs.insert(MemoryRange::new(start.as_u64(), end.as_u64()));
        Self { vranges: vaddrs }
    }

    pub fn allocate_pages<S: MemSize>(&mut self, count: u64) -> Option<PageRangeInclusive<S>> {
        let start_vaddr = self.vranges.allocate(S::SIZE * count, S::ALIGNMENT)?;

        let first_page = Page::containing_address(VirtAddr::new_extend(start_vaddr));

        Some(Page::range_inclusive(first_page, first_page + (count - 1)))
    }

    /// Returns a tuple with the range of pages and the guard pages
    pub fn allocate_guarded(
        &mut self,
        count: u64,
    ) -> Option<(Page<M4KiB>, PageRangeInclusive<M4KiB>, Page<M4KiB>)> {
        let size = M4KiB::SIZE * (count + 2);
        let alignment = M4KiB::ALIGNMENT;

        let start_vaddr = self.vranges.allocate(size, alignment)?;
        let start_vaddr = VirtAddr::new_extend(start_vaddr);

        let guard_page_start = Page::<M4KiB>::containing_address(start_vaddr);
        let usable_pages = Page::range_inclusive(
            Page::<M4KiB>::containing_address(start_vaddr + M4KiB::SIZE),
            Page::<M4KiB>::containing_address(start_vaddr + M4KiB::SIZE * count),
        );
        let guard_page_end =
            Page::<M4KiB>::containing_address(start_vaddr + M4KiB::SIZE * (count + 1));

        Some((guard_page_start, usable_pages, guard_page_end))
    }

    pub fn free_pages<S: MemSize>(&mut self, pages: PageRangeInclusive<S>) {
        self.vranges.insert(MemoryRange::new(
            pages.start().start_address().as_u64(),
            pages.end().start_address().as_u64() + (S::SIZE - 1),
        ));
    }
}
