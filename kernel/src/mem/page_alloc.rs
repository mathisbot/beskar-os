use x86_64::{
    VirtAddr,
    structures::paging::{
        Page, PageSize, PageTable, PageTableFlags, PageTableIndex, Size1GiB, Size2MiB, Size4KiB,
        page::PageRangeInclusive, page_table::FrameError,
    },
};

use crate::mem::{page_table, ranges::MemoryRange};
use hyperdrive::locks::mcs::MUMcsLock;

use super::ranges::MemoryRanges;

pub mod pmap;

static KPAGE_ALLOC: MUMcsLock<PageAllocator> = MUMcsLock::uninit();

/// This is the maximum valid address that 4 level paging can map.
const MAX_VALID_VADDR: u64 = 0xFFFF_FFFF_FFFF; // 256 TiB

const MAX_VRANGES: usize = 128;

pub fn init(recursive_index: u16) {
    /// Recursively remove already mapped pages from the available ranges.
    fn remove_mapping(
        level: u8,
        page_table: &PageTable,
        level_indices: &[PageTableIndex; 4],
        vaddrs: &mut MemoryRanges<MAX_VRANGES>,
    ) {
        for (i, pte) in page_table.iter().enumerate() {
            if !pte.flags().contains(PageTableFlags::PRESENT) {
                continue;
            }

            let mut level_indices = *level_indices;
            level_indices.rotate_left(1);
            level_indices[3] = PageTableIndex::new(u16::try_from(i).unwrap());

            if pte.flags().contains(PageTableFlags::HUGE_PAGE) {
                match level {
                    3 => {
                        let l4 = u64::from(level_indices[2]);
                        let l3 = u64::from(level_indices[3]);
                        let vaddr = (l4 << 39) | (l3 << 30);
                        vaddrs.remove(MemoryRange::new(vaddr, vaddr + (Size1GiB::SIZE - 1)));
                    }
                    2 => {
                        let l4 = u64::from(level_indices[1]);
                        let l3 = u64::from(level_indices[2]);
                        let l2 = u64::from(level_indices[3]);
                        let vaddr = (l4 << 39) | (l3 << 30) | (l2 << 21);
                        vaddrs.remove(MemoryRange::new(vaddr, vaddr + (Size2MiB::SIZE - 1)));
                    }
                    1 => {
                        unreachable!("Huge page in level 1");
                    }
                    _ => unreachable!("Invalid level"),
                }
            }

            if level == 1 {
                match pte.frame() {
                    Ok(_) => {
                        let l4 = u64::from(level_indices[0]);
                        let l3 = u64::from(level_indices[1]);
                        let l2 = u64::from(level_indices[2]);
                        let l1 = u64::from(level_indices[3]);
                        let vaddr = (l4 << 39) | (l3 << 30) | (l2 << 21) | (l1 << 12);
                        vaddrs.remove(MemoryRange::new(vaddr, vaddr + (Size4KiB::SIZE - 1)));
                    }
                    Err(FrameError::FrameNotPresent) => {}
                    Err(FrameError::HugeFrame) => {
                        unreachable!("Huge page in level 1");
                    }
                }
            } else {
                let l4 = u64::from(level_indices[0]);
                let l3 = u64::from(level_indices[1]);
                let l2 = u64::from(level_indices[2]);
                let l1 = u64::from(level_indices[3]);
                let vaddr = (l4 << 39) | (l3 << 30) | (l2 << 21) | (l1 << 12);

                let entry: &PageTable = unsafe { &*(vaddr as *const PageTable) };
                remove_mapping(level - 1, entry, &level_indices, vaddrs);
            }
        }
    }

    let mut vaddrs = MemoryRanges::new();
    // Skip the first two pages
    vaddrs.insert(MemoryRange::new(2 * Size4KiB::SIZE, MAX_VALID_VADDR));

    page_table::with_page_table(|pt| {
        remove_mapping(
            4,
            pt.level_4_table(),
            &[PageTableIndex::new(recursive_index); 4],
            &mut vaddrs,
        );
    });

    let pte_start = u64::from(recursive_index) << 39;
    let pte_end = (u64::from(recursive_index) << 39)
        // Fill in bits with all 1s
        | (0x1FF << 30)
        | (0x1FF << 21)
        | (0x1FF << 12);

    vaddrs.remove(MemoryRange::new(pte_start, pte_end));

    let mut page_allocator = PageAllocator { vranges: vaddrs };

    // Make sure identity-mapped page for the AP trampoline code is reserved
    reserve_tramp_page(&mut page_allocator);

    KPAGE_ALLOC.init(page_allocator);
}

pub struct PageAllocator {
    vranges: MemoryRanges<MAX_VRANGES>,
}

impl PageAllocator {
    pub fn allocate_pages<S: PageSize>(&mut self, count: u64) -> Option<PageRangeInclusive<S>> {
        let start_vaddr = self.vranges.allocate::<1>(
            S::SIZE * count,
            S::SIZE,
            &super::ranges::MemoryRangeRequest::DontCare,
        )?;

        let first_page =
            Page::from_start_address(VirtAddr::new(u64::try_from(start_vaddr).unwrap())).unwrap();

        Some(Page::range_inclusive(first_page, first_page + (count - 1)))
    }

    pub fn allocate_specific_page<S: PageSize>(&mut self, page: Page<S>) -> Option<Page<S>> {
        if page.start_address().as_u64() == 0 {
            return None; // Can't allocate the null page (not the first two pages)
        }

        if self
            .vranges
            .try_remove(MemoryRange::new(
                page.start_address().as_u64(),
                page.start_address().as_u64() + S::SIZE - 1,
            ))
            .is_some()
        {
            Some(page)
        } else {
            None // Page already used
        }
    }

    /// Returns a tuple with the range of pages and the guard page
    pub fn allocate_guarded<S: PageSize>(
        &mut self,
        count: u64,
    ) -> Option<(PageRangeInclusive<S>, Page<Size4KiB>)> {
        let size = S::SIZE * count + Size4KiB::SIZE;
        let alignment = S::SIZE;

        let mut start_vaddr = VirtAddr::new(
            u64::try_from(self.vranges.allocate::<1>(
                size,
                alignment,
                &super::ranges::MemoryRangeRequest::DontCare,
            )?)
            .unwrap(),
        );

        // Guard page is the first page
        let guard_page = Page::<Size4KiB>::from_start_address(start_vaddr).unwrap();
        start_vaddr += Size4KiB::SIZE;

        let first_page = Page::<S>::from_start_address(start_vaddr).unwrap();

        Some((
            Page::range_inclusive(first_page, first_page + (count - 1)),
            guard_page,
        ))
    }

    pub fn free_pages<S: PageSize>(&mut self, pages: PageRangeInclusive<S>) {
        self.vranges.insert(MemoryRange::new(
            pages.start.start_address().as_u64(),
            pages.end.start_address().as_u64() + S::SIZE - 1,
        ));
    }
}

/// Reserve a page for the AP trampoline code
///
/// It is easier to allocate the page at the beginning of memory initialization,
/// because we are sure that the needed region is available.
fn reserve_tramp_page(allocator: &mut PageAllocator) {
    let vaddr = VirtAddr::new(crate::arch::ap::AP_TRAMPOLINE_PADDR);

    let page = Page::<Size4KiB>::from_start_address(vaddr).unwrap();

    assert!(
        allocator.allocate_specific_page(page).is_some(),
        "Failed to allocate AP page"
    );
}

#[inline]
/// Perform a single operation on the kernel frame allocator
pub fn with_page_allocator<F, R>(f: F) -> R
where
    F: FnOnce(&mut PageAllocator) -> R,
{
    KPAGE_ALLOC.with_locked(f)
}
