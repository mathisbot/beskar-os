use crate::{arch::cpuid, process::scheduler};
use beskar_core::{
    arch::{
        commons::{
            PhysAddr, VirtAddr,
            paging::{CacheFlush as _, M4KiB, Mapper as _, MemSize as _, Page},
        },
        x86_64::{
            paging::page_table::{Entries, Flags, PageTable},
            registers::{Cr3, Efer},
        },
    },
    boot::KernelInfo,
};

use super::{frame_alloc, page_alloc};
use hyperdrive::{locks::mcs::McsLock, once::Once};

static KERNEL_ADDRESS_SPACE: Once<AddressSpace> = Once::uninit();

static KERNEL_CODE_INFO: Once<KernelInfo> = Once::uninit();

const PROCESS_PGALLOC_VRANGES: usize = 128;

pub fn init(recursive_index: u16, kernel_info: &KernelInfo) {
    KERNEL_CODE_INFO.call_once(|| *kernel_info);

    let kernel_pt = {
        let bootloader_pt_vaddr = {
            let recursive_index = u64::from(recursive_index);
            let vaddr = (recursive_index << 39)
                | (recursive_index << 30)
                | (recursive_index << 21)
                | (recursive_index << 12);
            VirtAddr::new_extend(vaddr)
        };

        // Safety: The page table given by the bootloader is valid
        let bootloader_pt = unsafe { &mut *bootloader_pt_vaddr.as_mut_ptr() };

        PageTable::new(bootloader_pt)
    };

    if cpuid::check_feature(cpuid::CpuFeature::TCE) {
        unsafe { Efer::insert_flags(Efer::TRANSLATION_CACHE_EXTENSION) };
    }

    KERNEL_ADDRESS_SPACE.call_once(|| {
        let (frame, _flags) = Cr3::read();
        let pgalloc = McsLock::new(page_alloc::PageAllocator::new_from_pt(&kernel_pt));
        AddressSpace {
            pt: McsLock::new(kernel_pt),
            lvl4_paddr: frame.start_address(),
            pgalloc,
            pgalloc_pml4_idx: None,
        }
    });
}

// TODO: Free PT frames on drop? Useful for userland processes.
pub struct AddressSpace {
    /// Page table of the address space
    ///
    /// ## WARNING
    ///
    /// This field is only valid if the address space is active.
    pt: McsLock<PageTable<'static>>,
    /// Physical address of the level 4 page table
    lvl4_paddr: PhysAddr,
    // FIXME: Make it less than 1KiB!
    /// The process-specific page allocator
    pgalloc: McsLock<super::page_alloc::PageAllocator<PROCESS_PGALLOC_VRANGES>>,
    /// The index of the PML4 entry used by the page allocator
    ///
    /// Currently, this is only used to check if the address space owns any address
    /// in the syscall handler.
    pgalloc_pml4_idx: Option<u16>,
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
            let userspc_idx = curr_addr_space.pgalloc_pml4_idx.map(usize::from);

            for (i, pte) in cpt
                .entries()
                .iter_entries()
                .enumerate()
                .filter(|(i, _)| userspc_idx.map_or(true, |idx| *i != idx))
            {
                if pte.is_null() {
                    continue;
                }
                pt[i] = *pte;
            }
        });

        let (recursive_idx, pte) = pt
            .iter_entries_mut()
            .enumerate()
            .filter(|(_, e)| e.is_null())
            .next_back()
            .unwrap();

        pte.set(frame.start_address(), Flags::PRESENT | Flags::WRITABLE);

        // We will need another free index to give to the page allocator
        let free_idx = u16::try_from(
            pt.iter_entries_mut()
                .enumerate()
                .filter(|(_, e)| e.is_null())
                .next_back()
                .expect("No free index for the page allocator")
                .0,
        )
        .unwrap();

        unsafe { page.start_address().as_mut_ptr::<Entries>().write(pt) };

        // Unmap the page from the current address space as we're done with it
        curr_addr_space.with_page_table(|page_table| page_table.unmap(page).unwrap().1.flush());
        curr_addr_space.with_pgalloc(|page_allocator| {
            page_allocator.free_pages(Page::range_inclusive(page, page));
        });

        let lvl4_vaddr = {
            assert!(u16::try_from(recursive_idx).is_ok(), "Index is too large");
            let i = u64::try_from(recursive_idx).unwrap();
            VirtAddr::new_extend((i << 39) | (i << 30) | (i << 21) | (i << 12))
        };

        // Create a new process page allocator with a whole PLM4 index area free (256TiB)
        let pgalloc = {
            let start_page = Page::<M4KiB>::from_p4p3p2p1(free_idx, 0, 0, 0);
            let end_page = Page::<M4KiB>::from_p4p3p2p1(free_idx, 511, 511, 511);

            let start_vaddr = start_page.start_address();
            let end_vaddr = end_page.start_address() + (M4KiB::SIZE - 1);

            page_alloc::PageAllocator::new_range(start_vaddr, end_vaddr)
        };

        Self {
            pt: McsLock::new(PageTable::new(unsafe { &mut *lvl4_vaddr.as_mut_ptr() })),
            lvl4_paddr: frame.start_address(),
            pgalloc: McsLock::new(pgalloc),
            pgalloc_pml4_idx: Some(free_idx),
        }
    }

    #[must_use]
    #[inline]
    /// Returns whether a certain memory range is owned by the address space.
    pub fn is_addr_owned(&self, start: VirtAddr, end: VirtAddr) -> bool {
        let Some(idx) = self.pgalloc_pml4_idx else {
            video::warn!("`AddressSpace::is_addr_owned` called on a non-user address space");
            return false;
        };

        let start_page = Page::from_p4p3p2p1(idx, 0, 0, 0);
        let end_page = Page::from_p4p3p2p1(idx, 511, 511, 511);

        let start_vaddr = start_page.start_address();
        let end_vaddr = end_page.start_address() + (M4KiB::SIZE - 1);

        start >= start_vaddr && end <= end_vaddr
    }

    #[must_use]
    #[inline]
    pub fn is_active(&self) -> bool {
        let (frame, _) = Cr3::read();
        self.lvl4_paddr == frame.start_address()
    }

    #[must_use]
    #[inline]
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
    /// ## Panics
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
