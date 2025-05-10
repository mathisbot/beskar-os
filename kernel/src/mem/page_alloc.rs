use super::address_space;
use beskar_core::{
    arch::{
        VirtAddr,
        paging::{M1GiB, M2MiB, M4KiB, MemSize, Page, PageRangeInclusive},
    },
    mem::ranges::{MemoryRange, MemoryRanges},
};
use beskar_hal::paging::page_table::{Entries, Flags, PageTable};

pub mod pmap;

/// This is the maximum valid address that 4 level paging can map.
const MAX_VALID_VADDR: u64 = 0xFFFF_FFFF_FFFF; // 256 TiB

pub fn init() {
    address_space::get_kernel_address_space().with_pgalloc(|page_allocator| {
        // Make sure identity-mapped page for the AP trampoline code is reserved
        reserve_tramp_page(page_allocator);
    });
}

/// Recursively remove already mapped pages from the available ranges.
fn remove_mapping<const N: usize>(
    level: u8,
    page_table: &Entries,
    level_indices: &[u16; 4],
    vaddrs: &mut MemoryRanges<N>,
) {
    for (i, pte) in page_table.iter_entries().enumerate() {
        if !pte.flags().contains(Flags::PRESENT) {
            continue;
        }

        let mut level_indices = *level_indices;
        level_indices.rotate_left(1);
        level_indices[3] = u16::try_from(i).unwrap();

        if pte.flags().contains(Flags::HUGE_PAGE) {
            match level {
                3 => {
                    let l4 = u64::from(level_indices[2]);
                    let l3 = u64::from(level_indices[3]);
                    let vaddr = (l4 << 39) | (l3 << 30);
                    vaddrs.remove(MemoryRange::new(vaddr, vaddr + (M1GiB::SIZE - 1)));
                }
                2 => {
                    let l4 = u64::from(level_indices[1]);
                    let l3 = u64::from(level_indices[2]);
                    let l2 = u64::from(level_indices[3]);
                    let vaddr = (l4 << 39) | (l3 << 30) | (l2 << 21);
                    vaddrs.remove(MemoryRange::new(vaddr, vaddr + (M2MiB::SIZE - 1)));
                }
                1 => {
                    unreachable!("Huge page in level 1");
                }
                _ => unreachable!("Invalid level"),
            }
        }

        if level == 1 {
            if pte.frame_start().is_some() {
                let l4 = u64::from(level_indices[0]);
                let l3 = u64::from(level_indices[1]);
                let l2 = u64::from(level_indices[2]);
                let l1 = u64::from(level_indices[3]);
                let vaddr = (l4 << 39) | (l3 << 30) | (l2 << 21) | (l1 << 12);
                vaddrs.remove(MemoryRange::new(vaddr, vaddr + (M4KiB::SIZE - 1)));
            }
        } else {
            let l4 = u64::from(level_indices[0]);
            let l3 = u64::from(level_indices[1]);
            let l2 = u64::from(level_indices[2]);
            let l1 = u64::from(level_indices[3]);
            let vaddr = VirtAddr::new_extend((l4 << 39) | (l3 << 30) | (l2 << 21) | (l1 << 12));

            let entry: &Entries = unsafe { &*(vaddr.as_ptr()) };
            remove_mapping(level - 1, entry, &level_indices, vaddrs);
        }
    }
}

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
        vaddrs.insert(MemoryRange::new(
            start.as_u64() & MAX_VALID_VADDR,
            end.as_u64() & MAX_VALID_VADDR,
        ));
        Self { vranges: vaddrs }
    }

    #[must_use]
    /// Create a new page allocator by walking through the page table
    pub fn new_from_pt(pt: &PageTable) -> Self {
        let mut vaddrs = MemoryRanges::<N>::new();
        // Skip the first two pages
        vaddrs.insert(MemoryRange::new(2 * M4KiB::SIZE, MAX_VALID_VADDR));

        let recursive_index = pt.recursive_index();

        remove_mapping(4, pt.entries(), &[recursive_index; 4], &mut vaddrs);

        let pte_start = u64::from(recursive_index) << 39;
        let pte_end = (u64::from(recursive_index) << 39)
            // Fill in bits with all 1s
            | (0x1FF << 30)
            | (0x1FF << 21)
            | (0x1FF << 12);

        vaddrs.remove(MemoryRange::new(pte_start, pte_end));

        Self { vranges: vaddrs }
    }

    pub fn allocate_pages<S: MemSize>(&mut self, count: u64) -> Option<PageRangeInclusive<S>> {
        let start_vaddr = self.vranges.allocate(S::SIZE * count, S::SIZE)?;

        let first_page =
            Page::from_start_address(VirtAddr::new_extend(u64::try_from(start_vaddr).unwrap()))
                .unwrap();

        Some(Page::range_inclusive(first_page, first_page + (count - 1)))
    }

    fn allocate_specific<S: MemSize>(&mut self, page: Page<S>) -> Option<Page<S>> {
        if page.start_address().as_u64() == 0 {
            return None; // Can't allocate the null page (not the first two pages)
        }

        if self
            .vranges
            .try_remove(MemoryRange::new(
                page.start_address().as_u64(),
                page.start_address().as_u64() + (S::SIZE - 1),
            ))
            .is_some()
        {
            Some(page)
        } else {
            None // Page already used
        }
    }

    /// Returns a tuple with the range of pages and the guard pages
    pub fn allocate_guarded(
        &mut self,
        count: u64,
    ) -> Option<(Page<M4KiB>, PageRangeInclusive<M4KiB>, Page<M4KiB>)> {
        let size = M4KiB::SIZE * (count + 2);
        let alignment = M4KiB::SIZE;

        let start_vaddr =
            VirtAddr::new_extend(u64::try_from(self.vranges.allocate(size, alignment)?).unwrap());

        let guard_page_start = Page::<M4KiB>::from_start_address(start_vaddr).unwrap();

        let usable_pages = Page::range_inclusive(
            Page::<M4KiB>::from_start_address(start_vaddr + M4KiB::SIZE).unwrap(),
            Page::<M4KiB>::from_start_address(start_vaddr + M4KiB::SIZE * count).unwrap(),
        );

        let guard_page_end =
            Page::<M4KiB>::from_start_address(start_vaddr + M4KiB::SIZE * (count + 1)).unwrap();

        Some((guard_page_start, usable_pages, guard_page_end))
    }

    pub fn free_pages<S: MemSize>(&mut self, pages: PageRangeInclusive<S>) {
        self.vranges.insert(MemoryRange::new(
            pages.start().start_address().as_u64(),
            pages.end().start_address().as_u64() + (S::SIZE - 1),
        ));
    }
}

/// Reserve a page for the AP trampoline code
///
/// It is easier to allocate the page at the beginning of memory initialization,
/// because we are sure that the needed region is available.
fn reserve_tramp_page<const N: usize>(allocator: &mut PageAllocator<N>) {
    let vaddr = VirtAddr::new(crate::arch::ap::AP_TRAMPOLINE_PADDR);

    let page = Page::<M4KiB>::from_start_address(vaddr).unwrap();

    assert!(
        allocator.allocate_specific(page).is_some(),
        "Failed to allocate AP page"
    );
}
