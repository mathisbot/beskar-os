use beskar_core::arch::commons::{
    PhysAddr, VirtAddr,
    paging::{CacheFlush as _, M4KiB, Mapper as _, Page},
};
use beskar_core::arch::x86_64::{
    paging::page_table::{Entries, Flags, PageTable},
    registers::Cr3,
};

use super::{frame_alloc, page_alloc, page_table};
use hyperdrive::once::Once;

static KERNEL_ADDRESS_SPACE: Once<AddressSpace> = Once::uninit();

static KERNEL_CODE_ADDRESS: Once<VirtAddr> = Once::uninit();

/// This function should only be called once BY THE BSP on startup.
pub fn init(recursive_index: u16, kernel_vaddr: VirtAddr) {
    KERNEL_CODE_ADDRESS.call_once(|| kernel_vaddr);

    KERNEL_ADDRESS_SPACE.call_once(|| {
        let (frame, flags) = Cr3::read();
        let vaddr = {
            let recursive_index = u64::from(recursive_index);
            let vaddr = (recursive_index << 39)
                | (recursive_index << 30)
                | (recursive_index << 21)
                | (recursive_index << 12);
            VirtAddr::new(vaddr)
        };
        AddressSpace {
            lvl4_vaddr: vaddr,
            lvl4_paddr: frame.start_address(),
            cr3_flags: flags,
        }
    });
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
// TODO: Free address space? Useful for userland processes.
pub struct AddressSpace {
    lvl4_vaddr: VirtAddr,
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
    pub fn new() -> Self {
        let frame =
            frame_alloc::with_frame_allocator(super::frame_alloc::FrameAllocator::alloc).unwrap();

        // The page is in the CURRENT address space.
        let page = page_alloc::with_page_allocator(|page_allocator| {
            page_allocator.allocate_pages::<M4KiB>(1)
        })
        .unwrap()
        .start;

        frame_alloc::with_frame_allocator(|frame_allocator| {
            page_table::with_page_table(|page_table| {
                page_table
                    .map(
                        page,
                        frame,
                        Flags::PRESENT | Flags::WRITABLE | Flags::NO_EXECUTE,
                        frame_allocator,
                    )
                    .flush();
            });
        });

        let mut pt = Entries::new();

        // Copy the kernel's page table entries to the new address space
        let kernel_start_page =
            Page::<M4KiB>::containing_address(*KERNEL_CODE_ADDRESS.get().unwrap());
        let kernel_page_range = kernel_start_page.p4_index().into()..512_usize;

        let current_page_table = KERNEL_ADDRESS_SPACE.get().unwrap().get_recursive_pt();
        let current_pt = current_page_table.entries();
        for (i, pte) in current_pt
            .iter()
            .enumerate()
            .skip(kernel_page_range.start)
            .take(512 - kernel_page_range.start)
        {
            if pte.is_null() {
                continue;
            }
            pt[i].set(pte.addr(), pte.flags());
        }

        let (index, pte) = pt
            .iter_mut()
            .enumerate()
            .filter(|(_, e)| e.is_null())
            .next_back()
            .unwrap();
        assert_ne!(index, 0, "No free PTEs in the new page table");

        pte.set(frame.start_address(), Flags::PRESENT | Flags::WRITABLE);

        unsafe { page.start_address().as_mut_ptr::<Entries>().write(pt) };

        // Unmap the page from the current address space as we're done with it
        page_table::with_page_table(|page_table| page_table.unmap(page).unwrap().1.flush());
        page_alloc::with_page_allocator(|page_allocator| {
            page_allocator.free_pages(Page::range_inclusive(page, page));
        });

        let lvl4_vaddr = {
            assert!(u16::try_from(index).is_ok(), "Index is too large");
            let i = u64::try_from(index).unwrap();
            VirtAddr::new((i << 39) | (i << 30) | (i << 21) | (i << 12))
        };

        Self {
            lvl4_vaddr,
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

    pub fn cr3_raw(&self) -> usize {
        usize::try_from(self.lvl4_paddr.as_u64() | u64::from(self.cr3_flags())).unwrap()
    }

    fn get_recursive_pt(&self) -> PageTable<'static> {
        assert!(self.is_active(), "Address space is not active");
        PageTable::new(unsafe { &mut *self.lvl4_vaddr.as_mut_ptr() })
    }
}

pub fn get_kernel_address_space() -> &'static AddressSpace {
    KERNEL_ADDRESS_SPACE.get().unwrap()
}
