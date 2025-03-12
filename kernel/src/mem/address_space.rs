use beskar_core::arch::x86_64::{
    paging::page_table::{Entries, Flags, PageTable},
    registers::Cr3,
};
use beskar_core::{
    arch::commons::{
        PhysAddr, VirtAddr,
        paging::{CacheFlush as _, M4KiB, Mapper as _, Page},
    },
    boot::KernelInfo,
};

use super::{frame_alloc, page_alloc};
use hyperdrive::{locks::mcs::McsLock, once::Once};

static KERNEL_ADDRESS_SPACE: Once<AddressSpace> = Once::uninit();

static KERNEL_CODE_INFO: Once<KernelInfo> = Once::uninit();

pub fn init(recursive_index: u16, kernel_info: &KernelInfo) {
    KERNEL_CODE_INFO.call_once(|| *kernel_info);

    let kernel_pt = {
        let bootloader_pt_vaddr = {
            let recursive_index = u64::from(recursive_index);
            let vaddr = (recursive_index << 39)
                | (recursive_index << 30)
                | (recursive_index << 21)
                | (recursive_index << 12);
            VirtAddr::new(vaddr)
        };

        // Safety: The page table given by the bootloader is valid
        let bootloader_pt = unsafe { &mut *bootloader_pt_vaddr.as_mut_ptr() };

        PageTable::new(bootloader_pt)
    };

    KERNEL_ADDRESS_SPACE.call_once(|| {
        let (frame, flags) = Cr3::read();
        AddressSpace {
            pt: McsLock::new(kernel_pt),
            lvl4_paddr: frame.start_address(),
            cr3_flags: flags,
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
    /// # WARNING
    /// Only updated when the address space is loaded.
    cr3_flags: u16,
}

impl Default for AddressSpace {
    fn default() -> Self {
        Self::new()
    }
}

impl AddressSpace {
    #[must_use]
    /// Create a new address space.
    ///
    /// ## Panics
    ///
    /// Panics if the kernel address space is not active.
    pub fn new() -> Self {
        // TODO: Find a way to avoid this panic
        assert!(
            KERNEL_ADDRESS_SPACE.get().unwrap().is_active(),
            "Kernel address must be active"
        );

        let page = page_alloc::with_page_allocator(|page_allocator| {
            page_allocator.allocate_pages::<M4KiB>(1)
        })
        .unwrap()
        .start;

        let frame = frame_alloc::with_frame_allocator(|frame_allocator| {
            let frame = frame_allocator.alloc().unwrap();
            with_kernel_pt(|page_table| {
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

        // Copy the kernel's page table entries to the new address space
        with_kernel_pt(|current_pt| {
            // FIXME: Is it safe to copy the whole PML4?
            // In fact we should only need kernel code, heap, fb, and stack.
            // maybe more?

            // let kcode_info = KERNEL_CODE_INFO.get().unwrap();
            // let kernel_start_page = Page::<M4KiB>::containing_address(kcode_info.vaddr());
            // let kernel_end_page =
            //     Page::<M4KiB>::containing_address(kcode_info.vaddr() + kcode_info.size());

            for (i, pte) in current_pt.entries().iter().enumerate()
            // .take(kernel_end_page.p4_index().into())
            // .skip(kernel_start_page.p4_index().into())
            {
                if pte.is_null() {
                    continue;
                }
                pt[i] = *pte;
            }
        });

        let (index, pte) = pt
            .iter_mut()
            .enumerate()
            .filter(|(_, e)| e.is_null())
            .next_back()
            .unwrap();

        pte.set(frame.start_address(), Flags::PRESENT | Flags::WRITABLE);

        unsafe { page.start_address().as_mut_ptr::<Entries>().write(pt) };

        // Unmap the page from the current address space as we're done with it

        with_kernel_pt(|page_table| page_table.unmap(page).unwrap().1.flush());
        page_alloc::with_page_allocator(|page_allocator| {
            page_allocator.free_pages(Page::range_inclusive(page, page));
        });

        let lvl4_vaddr = {
            assert!(u16::try_from(index).is_ok(), "Index is too large");
            let i = u64::try_from(index).unwrap();
            VirtAddr::new_extend((i << 39) | (i << 30) | (i << 21) | (i << 12))
        };

        Self {
            pt: McsLock::new(PageTable::new(unsafe { &mut *lvl4_vaddr.as_mut_ptr() })),
            lvl4_paddr: frame.start_address(),
            cr3_flags: 0,
        }
    }

    pub fn is_active(&self) -> bool {
        let (frame, _) = Cr3::read();
        self.lvl4_paddr == frame.start_address()
    }

    pub fn cr3_flags(&self) -> u16 {
        if self.is_active() {
            Cr3::read().1
        } else {
            self.cr3_flags
        }
    }

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
        self.pt.with_locked(|pt| f(pt))
    }
}

pub fn get_kernel_address_space() -> &'static AddressSpace {
    KERNEL_ADDRESS_SPACE.get().unwrap()
}

pub fn with_kernel_pt<R>(f: impl FnOnce(&mut PageTable<'static>) -> R) -> R {
    get_kernel_address_space().with_page_table(f)
}
