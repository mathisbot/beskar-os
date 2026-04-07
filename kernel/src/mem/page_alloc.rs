use beskar_core::{
    arch::{
        VirtAddr,
        paging::{M4KiB, MemSize, Page, PageRangeInclusive},
    },
    mem::ranges::{MemoryRange, MemoryRanges},
};
use bootloader_api::KERNEL_POOL_BASE;
use hyperdrive::locks::mcs::McsLock;

const KERNEL_PAGE_ALLOCATOR_SIZE: usize = 256;

/// The kernel page allocator.
static KERNEL_PAGE_ALLOCATOR: McsLock<PageAllocator<KERNEL_PAGE_ALLOCATOR_SIZE>> =
    McsLock::new(PageAllocator::new_range(KERNEL_POOL_BASE, VirtAddr::MAX));

#[derive(Debug)]
pub struct PageAllocator<const N: usize> {
    vranges: MemoryRanges<N>,
}

impl<const N: usize> PageAllocator<N> {
    #[must_use]
    #[inline]
    pub const fn new_range(start: VirtAddr, end: VirtAddr) -> Self {
        let vranges = MemoryRanges::from_single(MemoryRange::new(start.as_u64(), end.as_u64()));
        Self { vranges }
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

pub fn with_kernel_page_allocator<R>(
    f: impl FnOnce(&mut PageAllocator<KERNEL_PAGE_ALLOCATOR_SIZE>) -> R,
) -> R {
    KERNEL_PAGE_ALLOCATOR.with_locked(f)
}
