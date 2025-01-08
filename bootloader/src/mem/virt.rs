use x86_64::{
    VirtAddr,
    structures::paging::{Page, PageSize, PageTableIndex, Size4KiB},
};
use xmas_elf::program::ProgramHeader;

use crate::PhysicalFrameBuffer;

/// Keeps track of used entries in the level 4 page table.
pub struct Level4Entries([bool; 512]);

impl Level4Entries {
    #[must_use]
    #[allow(clippy::missing_panics_doc)]
    pub fn new(framebuffer: &PhysicalFrameBuffer, max_phys_addr: u64) -> Self {
        let mut usage = [false; 512];

        // Mark identity-mapped memory as used
        let start_page = Page::<Size4KiB>::containing_address(VirtAddr::new(0));
        let end_page = Page::<Size4KiB>::containing_address(VirtAddr::new(max_phys_addr));

        for used_page in usage
            .iter_mut()
            .take(usize::from(end_page.p4_index()) + 1)
            .skip(usize::from(start_page.p4_index()))
        {
            *used_page = true;
        }

        // Mark framebuffer as used
        let start = VirtAddr::new(framebuffer.buffer_start().as_u64());
        let end = start + u64::try_from(framebuffer.info().size).unwrap() - 1;

        let start_page = Page::<Size4KiB>::containing_address(start);
        let end_page = Page::<Size4KiB>::containing_address(end);

        for used_page in usage
            .iter_mut()
            .take(usize::from(end_page.p4_index()) + 1)
            .skip(usize::from(start_page.p4_index()))
        {
            *used_page = true;
        }

        Self(usage)
    }

    #[must_use]
    #[inline]
    const fn internal_entries(&self) -> &[bool; 512] {
        &self.0
    }

    /// Marks the virtual address range of all segments as used.
    pub fn mark_segments<'a>(
        &mut self,
        segments: impl Iterator<Item = ProgramHeader<'a>>,
        virtual_address_offset: u64,
    ) {
        for segment in segments.filter(|s| s.mem_size() > 0) {
            let start = VirtAddr::new(virtual_address_offset + segment.virtual_addr());
            let end = start + segment.mem_size() - 1;

            let start_page = Page::<Size4KiB>::containing_address(start);
            let end_page = Page::<Size4KiB>::containing_address(end);

            for i in usize::from(start_page.p4_index())..=usize::from(end_page.p4_index()) {
                self.0[i] = true;
            }
        }
    }

    /// Returns the first index of `num` contiguous unused level 4 entries.
    ///
    /// ## Note
    ///
    /// Marks each returned index as used.
    ///
    /// ## Panics
    ///
    /// Panics if no contiguous free entries are found.
    pub fn get_free_entries(&mut self, num: usize) -> PageTableIndex {
        // TODO: ASLR
        let index = self
            .internal_entries()
            .windows(num)
            .position(|entries| entries.iter().all(|used| !used))
            .expect("No suitable level 4 entries found");

        for i in 0..num {
            self.0[index + i] = true;
        }

        PageTableIndex::new(u16::try_from(index).unwrap())
    }

    /// Returns a virtual address that is not used.
    ///
    /// ## Note
    ///
    /// Marks associated entries indices as used.
    ///
    /// ## Panics
    ///
    /// Panics if no contiguous free memory is found.
    pub fn get_free_address(&mut self, size: u64) -> VirtAddr {
        let needed_lvl4_entries = size.div_ceil(512 * 512 * 512 * Size4KiB::SIZE);

        // TODO: ASLR (add random offset, need to manage alignment)
        Page::from_page_table_indices_1gib(
            self.get_free_entries(usize::try_from(needed_lvl4_entries).unwrap()),
            PageTableIndex::new(0),
        )
        .start_address()
    }
}
