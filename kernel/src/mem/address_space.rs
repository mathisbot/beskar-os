use super::{frame_alloc, page_alloc};
use crate::{arch::cpuid, process::scheduler};
use beskar_core::arch::{
    PhysAddr, VirtAddr,
    paging::{CacheFlush as _, M4KiB, Mapper as _, MemSize as _, Page},
};
use beskar_hal::{
    paging::page_table::{Entries, Flags, PageTable},
    registers::{Cr3, Efer},
};
use bootloader_api::{
    KERNEL_AS_BASE, KERNEL_POOL_BASE, KERNEL_PT_RECURSIVE_INDEX, KERNEL_PT_START_ENTRY, KernelInfo,
};
use hyperdrive::{locks::mcs::McsLock, once::Once};

static KERNEL_ADDRESS_SPACE: Once<AddressSpace> = Once::uninit();

static KERNEL_CODE_INFO: Once<KernelInfo> = Once::uninit();

const PROCESS_PGALLOC_VRANGES: usize = 64;

pub fn init(recursive_index: u16, kernel_info: &KernelInfo) {
    KERNEL_CODE_INFO.call_once(|| *kernel_info);

    let kernel_pt = {
        let vaddr = VirtAddr::from_pt_indices(
            recursive_index,
            recursive_index,
            recursive_index,
            recursive_index,
            0,
        );
        // Safety: The page table given by the bootloader is valid
        let raw_pt = unsafe { &mut *vaddr.as_mut_ptr::<Entries>() };
        PageTable::new(raw_pt)
    };

    if cpuid::check_feature(cpuid::CpuFeature::TCE) {
        unsafe { Efer::insert_flags(Efer::TRANSLATION_CACHE_EXTENSION) };
    }

    KERNEL_ADDRESS_SPACE.call_once(|| {
        let (frame, _flags) = Cr3::read();
        let pgalloc = McsLock::new(page_alloc::PageAllocator::new_range(
            KERNEL_POOL_BASE,
            VirtAddr::MAX,
        ));
        AddressSpace {
            pt: McsLock::new(kernel_pt),
            lvl4_paddr: frame.start_address(),
            pgalloc,
        }
    });
}

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
        let curr_process = scheduler::current_process();
        let curr_addr_space = curr_process.address_space();

        let page = curr_addr_space
            .with_pgalloc(|page_allocator| page_allocator.allocate_pages::<M4KiB>(1))
            .unwrap()
            .start();

        let frame = frame_alloc::with_frame_allocator(|frame_allocator| {
            let frame = frame_allocator.alloc().unwrap();
            curr_addr_space.with_page_table(|page_table| {
                page_table
                    .map(
                        page,
                        frame,
                        Flags::PRESENT | Flags::WRITABLE | Flags::NO_EXECUTE,
                        frame_allocator,
                    )
                    .flush();
            });
            frame
        });

        let mut pt = Entries::new();

        curr_addr_space.with_page_table(|cpt| {
            for (i, pte) in cpt
                .entries()
                .iter_entries()
                .enumerate()
                .skip(usize::from(KERNEL_PT_START_ENTRY))
            {
                pt[i] = *pte;
            }
        });
        pt[usize::from(KERNEL_PT_RECURSIVE_INDEX)]
            .set(frame.start_address(), Flags::PRESENT | Flags::WRITABLE);

        unsafe { page.start_address().as_mut_ptr::<Entries>().write(pt) };

        // Unmap the page from the current address space as we're done with it
        curr_addr_space.with_page_table(|page_table| page_table.unmap(page).unwrap().1.flush());
        curr_addr_space.with_pgalloc(|page_allocator| {
            page_allocator.free_pages(Page::range_inclusive(page, page));
        });

        let lvl4_vaddr = {
            let i = KERNEL_PT_RECURSIVE_INDEX;
            VirtAddr::from_pt_indices(i, i, i, i, 0)
        };

        // Create a new process page allocator with a whole PLM4 index area free (256TiB)
        let pgalloc = {
            let start_page = Page::<M4KiB>::from_p4p3p2p1(0, 0, 0, 0);
            let end_page = Page::<M4KiB>::from_p4p3p2p1(KERNEL_PT_START_ENTRY - 1, 511, 511, 511);

            let start_vaddr = start_page.start_address();
            let end_vaddr = end_page.start_address() + (M4KiB::SIZE - 1);

            page_alloc::PageAllocator::new_range(start_vaddr, end_vaddr)
        };

        Self {
            pt: McsLock::new(PageTable::new(unsafe { &mut *lvl4_vaddr.as_mut_ptr() })),
            lvl4_paddr: frame.start_address(),
            pgalloc: McsLock::new(pgalloc),
        }
    }

    #[must_use]
    #[inline]
    /// Returns whether a certain memory range is owned by the address space.
    pub fn is_addr_owned(&self, _start: VirtAddr, end: VirtAddr) -> bool {
        end < KERNEL_AS_BASE
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

    /// Operate on the page table of the address space.
    ///
    /// # Panics
    ///
    /// Panics if the address space is not active.
    pub fn with_page_table<R>(&self, f: impl FnOnce(&mut PageTable<'static>) -> R) -> R {
        assert!(self.is_active(), "Address space must be active");
        self.pt.with_locked(f)
    }

    #[inline]
    /// Operate on the process' page allocator.
    pub fn with_pgalloc<R>(
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
    }
}

#[must_use]
#[inline]
pub fn get_kernel_address_space() -> &'static AddressSpace {
    KERNEL_ADDRESS_SPACE.get().unwrap()
}

#[inline]
pub fn with_kernel_pgalloc<R>(
    f: impl FnOnce(&mut super::page_alloc::PageAllocator<PROCESS_PGALLOC_VRANGES>) -> R,
) -> R {
    get_kernel_address_space().with_pgalloc(f)
}

#[inline]
pub fn with_kernel_pt<R>(f: impl FnOnce(&mut PageTable<'static>) -> R) -> R {
    get_kernel_address_space().with_page_table(f)
}
