use x86_64::{
    registers::control::{Cr3, Cr3Flags},
    structures::paging::{Mapper, Page, PageTable, PageTableFlags, Size4KiB},
    PhysAddr, VirtAddr,
};

use super::{frame_alloc, page_alloc, page_table};
use crate::utils::once::Once;

static KERNEL_ADDRESS_SPACE: Once<AddressSpace> = Once::uninit();

/// This function should only be called once BY THE BSP on startup.
pub fn init(recursive_index: u16) {
    KERNEL_ADDRESS_SPACE.call_once(|| {
        let (frame, flags) = Cr3::read();
        let vaddr = {
            let recursive_index = u64::from(recursive_index);
            let vaddr = recursive_index << 39
                | recursive_index << 30
                | recursive_index << 21
                | recursive_index << 12;
            VirtAddr::new(vaddr)
        };
        AddressSpace::new_raw(vaddr, frame.start_address(), flags)
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

impl AddressSpace {
    #[must_use]
    #[inline]
    pub const fn new_raw(lvl4_vaddr: VirtAddr, lvl4_paddr: PhysAddr, cr3: Cr3Flags) -> Self {
        Self {
            lvl4_vaddr,
            lvl4_paddr,
            cr3,
        }
    }

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

        // TODO: Copy the kernel's page table entries to the new address space

        let (index, pte) = pt
            .iter_mut()
            .enumerate()
            .filter(|(_, e)| e.is_unused())
            .last()
            .unwrap();
        assert_ne!(index, 0, "No free PTEs in the new page table");

        pte.set_frame(frame, PageTableFlags::PRESENT | PageTableFlags::WRITABLE);

        unsafe { page.start_address().as_mut_ptr::<PageTable>().write(pt) };

        page_table::with_page_table(|page_table| page_table.unmap(page).unwrap().1.flush());

        page_alloc::with_page_allocator(|page_allocator| {
            page_allocator.free_pages(Page::range_inclusive(page, page));
        });

        let lvl4_vaddr = {
            assert!(u16::try_from(index).is_ok(), "Index is too large");
            let i = u64::try_from(index).unwrap();
            VirtAddr::new(i << 39 | i << 30 | i << 21 | i << 12)
        };

        Self::new_raw(lvl4_vaddr, frame.start_address(), Cr3Flags::empty())
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
}

pub fn get_kernel_address_space() -> &'static AddressSpace {
    KERNEL_ADDRESS_SPACE.get().unwrap()
}
