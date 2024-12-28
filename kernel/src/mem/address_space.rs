use x86_64::{
    registers::control::{Cr3, Cr3Flags},
    structures::paging::{Mapper, Page, PageTable, PageTableFlags, RecursivePageTable, Size4KiB},
    PhysAddr, VirtAddr,
};

use super::{frame_alloc, page_alloc, page_table};
use crate::utils::once::Once;

static KERNEL_ADDRESS_SPACE: Once<AddressSpace> = Once::uninit();

static KERNEL_CODE_ADDRESS: Once<VirtAddr> = Once::uninit();

/// This function should only be called once BY THE BSP on startup.
pub fn init(recursive_index: u16, kernel_vaddr: u64) {
    KERNEL_CODE_ADDRESS.call_once(|| VirtAddr::new(kernel_vaddr));

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
            cr3: flags,
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
    cr3: Cr3Flags,
}

impl Default for AddressSpace {
    fn default() -> Self {
        Self::new()
    }
}

impl AddressSpace {
    #[must_use]
    pub fn new() -> Self {
        let frame = frame_alloc::with_frame_allocator(|frame_allocator| {
            frame_allocator.alloc::<Size4KiB>()
        })
        .unwrap();

        // The page is in the CURRENT address space.
        let page = page_alloc::with_page_allocator(|page_allocator| {
            page_allocator.allocate_pages::<Size4KiB>(1)
        })
        .unwrap()
        .start;

        frame_alloc::with_frame_allocator(|frame_allocator| {
            page_table::with_page_table(|page_table| {
                unsafe {
                    page_table.map_to(
                        page,
                        frame,
                        PageTableFlags::PRESENT
                            | PageTableFlags::WRITABLE
                            | PageTableFlags::NO_EXECUTE,
                        frame_allocator,
                    )
                }
                .unwrap()
                .flush();
            });
        });

        let mut pt = PageTable::new();

        // Copy the kernel's page table entries to the new address space
        let kernel_start_page =
            Page::<Size4KiB>::containing_address(*KERNEL_CODE_ADDRESS.get().unwrap());
        let kernel_page_range = kernel_start_page.p4_index().into()..512_usize;

        let current_page_table = KERNEL_ADDRESS_SPACE.get().unwrap().get_recursive_pt();
        let current_pt = current_page_table.level_4_table();
        for (i, pte) in current_pt
            .iter()
            .enumerate()
            .skip(kernel_page_range.start)
            .take(512 - kernel_page_range.start)
        {
            if pte.is_unused() {
                continue;
            }
            pt[i].set_addr(pte.addr(), pte.flags());
        }

        let (index, pte) = pt
            .iter_mut()
            .enumerate()
            .filter(|(_, e)| e.is_unused())
            .last()
            .unwrap();
        assert_ne!(index, 0, "No free PTEs in the new page table");

        pte.set_frame(frame, PageTableFlags::PRESENT | PageTableFlags::WRITABLE);

        unsafe { page.start_address().as_mut_ptr::<PageTable>().write(pt) };

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
            cr3: Cr3Flags::empty(),
        }
    }

    pub fn is_active(&self) -> bool {
        let (frame, _) = Cr3::read();
        self.lvl4_paddr == frame.start_address()
    }

    pub fn cr3_flags(&self) -> Cr3Flags {
        if self.is_active() {
            Cr3::read().1
        } else {
            self.cr3
        }
    }

    fn get_recursive_pt(&self) -> RecursivePageTable<'static> {
        assert!(self.is_active(), "Address space is not active");
        unsafe { RecursivePageTable::new(&mut *self.lvl4_vaddr.as_mut_ptr()) }.unwrap()
    }
}

pub fn get_kernel_address_space() -> &'static AddressSpace {
    KERNEL_ADDRESS_SPACE.get().unwrap()
}
