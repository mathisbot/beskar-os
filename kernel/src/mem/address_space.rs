use super::page_alloc;
use crate::mem::vmm;
use beskar_core::arch::{
    PhysAddr, VirtAddr,
    paging::{M4KiB, MemSize, Page},
};
use beskar_hal::{
    paging::page_table::{Entries, Flags, PageTable},
    registers::Cr3,
};
use bootloader_api::{KERNEL_AS_BASE, KERNEL_PT_START_ENTRY, USER_PT_END_ENTRY};
use hyperdrive::locks::mcs::McsLock;

const PROCESS_PGALLOC_VRANGES: usize = 64;

// TODO: Free PT frames on drop? Useful for userland processes.
pub struct AddressSpace {
    /// Page table of the address space
    ///
    /// # WARNING
    ///
    /// This field is only valid if the address space is active.
    pt: McsLock<PageTable<'static>>,
    /// Physical address of the level 4 page table
    lvl4_paddr: PhysAddr,
    // FIXME: Make it less than 1KiB!
    /// The process-specific page allocator
    pgalloc: McsLock<super::page_alloc::PageAllocator<PROCESS_PGALLOC_VRANGES>>,
}

impl Default for AddressSpace {
    fn default() -> Self {
        Self::new()
    }
}

impl AddressSpace {
    #[must_use]
    /// Create a new address space.
    pub fn new() -> Self {
        // Prepare memory for the new PML4.
        let page = vmm::kernel::reserve_pages::<M4KiB>(1).unwrap().start();
        let frame = vmm::kernel::alloc_frame::<M4KiB>().unwrap();
        vmm::kernel::map_frame(
            page,
            frame,
            Flags::PRESENT | Flags::WRITABLE | Flags::NO_EXECUTE,
        )
        .unwrap();

        // Prepare the new PML4
        let mut pt = Entries::new();
        let recursive_index = vmm::kernel::recursive_index();

        // Safety: `skip(usize::from(KERNEL_PT_START_ENTRY))` guarantees we only read kernel entries.
        unsafe {
            vmm::kernel::with_kernel_pt(|kpt| {
                for (kpte, pte) in kpt
                    .entries()
                    .iter_entries()
                    .zip(pt.iter_entries_mut())
                    .skip(usize::from(KERNEL_PT_START_ENTRY))
                {
                    *pte = *kpte;
                }
            });
        }
        pt[usize::from(recursive_index)]
            .set(frame.start_address(), Flags::PRESENT | Flags::WRITABLE);

        // Write the new PML4 to the reserved page
        unsafe { page.start_address().as_mut_ptr::<Entries>().write(pt) };

        // Unmap the page from the current address space as we're done with it
        let unmapped_frame = vmm::kernel::unmap_page(page).unwrap();
        debug_assert_eq!(unmapped_frame, frame);
        vmm::kernel::free_pages(Page::range_inclusive(page, page));

        // Create a new process page allocator with 256 PLM4 index free (128TiB)
        let pgalloc = {
            let start_page = Page::<M4KiB>::from_p4p3p2p1(0, 0, 0, 1);
            let end_page = Page::<M4KiB>::from_p4p3p2p1(USER_PT_END_ENTRY, 511, 511, 511);

            let start_vaddr = start_page.start_address();
            let end_vaddr = end_page.start_address() + (M4KiB::SIZE - 1);

            page_alloc::PageAllocator::new_range(start_vaddr, end_vaddr)
        };
        let lvl4_vaddr = {
            let i = recursive_index;
            VirtAddr::from_pt_indices(i, i, i, i, 0)
        };

        Self {
            pt: McsLock::new(PageTable::new(unsafe { &mut *lvl4_vaddr.as_mut_ptr() })),
            lvl4_paddr: frame.start_address(),
            pgalloc: McsLock::new(pgalloc),
        }
    }

    #[must_use]
    #[inline]
    #[expect(clippy::unused_self, reason = "Might be used in the future")]
    /// Returns whether a certain memory range is owned by the address space.
    pub fn is_addr_owned(&self, start: VirtAddr, end: VirtAddr) -> bool {
        debug_assert!(start <= end);
        start <= end && end < KERNEL_AS_BASE
    }

    #[must_use]
    #[inline]
    pub fn is_active(&self) -> bool {
        let (frame, _) = Cr3::read();
        self.lvl4_paddr == frame.start_address()
    }

    #[must_use]
    #[inline]
    #[expect(clippy::unused_self, reason = "CR3 flags are constant")]
    pub const fn cr3_flags(&self) -> u16 {
        // The only two valid CR3 flags are CACHE_WRITETHROUGH and CACHE_DISABLE
        // These two are better set at the page table entry level
        0
    }

    #[must_use]
    #[inline]
    pub fn cr3_raw(&self) -> u64 {
        self.lvl4_paddr.as_u64() | u64::from(self.cr3_flags())
    }

    #[inline]
    /// Activate the address space by writing to CR3.
    ///
    /// # Safety
    ///
    /// The caller must ensure the CPU's state allows switching to that address space.
    pub(crate) unsafe fn activate(&self) {
        unsafe { Cr3::write_raw(self.cr3_raw()) };
    }

    /// Operate on the page table of the address space.
    ///
    /// # Panics
    ///
    /// Panics if the address space is not active.
    ///
    /// # Safety
    ///
    /// The caller must only modify the userland portion of the page table.
    pub(super) unsafe fn with_page_table<R>(
        &self,
        f: impl FnOnce(&mut PageTable<'static>) -> R,
    ) -> R {
        assert!(self.is_active(), "Address space must be active");
        self.pt.with_locked(f)
    }

    #[inline]
    /// Operate on the process' page allocator.
    pub(super) fn with_pgalloc<R>(
        &self,
        f: impl FnOnce(&mut super::page_alloc::PageAllocator<PROCESS_PGALLOC_VRANGES>) -> R,
    ) -> R {
        self.pgalloc.with_locked(f)
    }
}

impl Drop for AddressSpace {
    fn drop(&mut self) {
        // We recall that the address space's page table is not active anymore
        debug_assert!(
            !self.is_active(),
            "Address space is suspiciously still active on drop"
        );
        // TODO: Free frames in userland
    }
}
