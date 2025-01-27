use beskar_core::arch::x86_64::registers::Cr3;
use uefi::{
    boot::MemoryType,
    mem::memory_map::{MemoryMap, MemoryMapOwned},
};
use x86_64::{
    VirtAddr,
    structures::paging::{
        FrameAllocator, OffsetPageTable, PageSize, PageTable, PhysFrame, Size4KiB,
    },
};

mod phys;
pub use phys::EarlyFrameAllocator;

mod virt;
pub use virt::{Level4Entries, Mappings};
use xmas_elf::ElfFile;

use crate::{debug, info};

#[must_use]
pub fn init(
    memory_map: MemoryMapOwned,
    kernel_elf: &ElfFile,
) -> (EarlyFrameAllocator, PageTables, Mappings) {
    let total_mem_size = compute_total_memory_kib(&memory_map);
    debug!("Detected memory size: {} MiB", total_mem_size / 1024);

    let mut frame_allocator = EarlyFrameAllocator::new(memory_map);

    let mut page_tables = create_page_tables(&mut frame_allocator);

    let mappings = virt::make_mappings(kernel_elf, &mut frame_allocator, &mut page_tables);

    (frame_allocator, page_tables, mappings)
}

/// Provides access to the page tables of the bootloader and kernel address space.
pub struct PageTables {
    /// Provides access to the page tables of the bootloader address space.
    pub bootloader: OffsetPageTable<'static>,
    /// Provides access to the page tables of the kernel address space (not active).
    pub kernel: OffsetPageTable<'static>,
    /// The physical frame where the level 4 page table of the kernel address space is stored.
    ///
    /// Must be the page table that the `kernel` field of this struct refers to.
    pub kernel_level_4_frame: PhysFrame,
}

#[must_use]
fn compute_total_memory_kib(memory_map: &MemoryMapOwned) -> u64 {
    memory_map
        .entries()
        .filter_map(|entry| match entry.ty {
            MemoryType::CONVENTIONAL
            | MemoryType::LOADER_CODE
            | MemoryType::LOADER_DATA
            | MemoryType::BOOT_SERVICES_CODE
            | MemoryType::BOOT_SERVICES_DATA
            | MemoryType::RUNTIME_SERVICES_CODE
            | MemoryType::RUNTIME_SERVICES_DATA => Some(entry.page_count),
            _ => None,
        })
        .sum::<u64>()
        * (Size4KiB::SIZE / 1024)
}

pub fn create_page_tables(frame_allocator: &mut EarlyFrameAllocator) -> PageTables {
    // All memory is identity mapped by UEFI
    let physical_offset = VirtAddr::new(0);

    // TODO: Don't
    let bootloader_page_table = {
        let old_table = {
            let (old_frame, _) = Cr3::read();
            let ptr: *const PageTable =
                (physical_offset + old_frame.start_address().as_u64()).as_ptr();

            // ## Safety
            // We are reading a page table from a valid physical address mapped
            // in the virtual address space.
            unsafe { &*ptr }
        };

        let frame = frame_allocator
            .allocate_frame()
            .expect("Failed to allocate a frame");

        let table = {
            let ptr: *mut PageTable =
                (physical_offset + frame.start_address().as_u64()).as_mut_ptr();

            // ## Safety
            // We are writing a page table to a valid physical address
            // mapped in the virtual address space.
            unsafe {
                ptr.write(PageTable::new());
                &mut *ptr
            }
        };

        // Copy indexes for identity mapped memory
        let end_vaddr = VirtAddr::new(frame_allocator.max_physical_address().as_u64() - 1);
        for p4_index in 0..=usize::from(end_vaddr.p4_index()) {
            table[p4_index] = old_table[p4_index].clone();
        }

        // Copy indexes for framebuffer (which is not necessarily identity mapped)
        let (start_vaddr, end_vaddr) = crate::video::with_physical_framebuffer(|fb| {
            let start_vaddr = fb.start_addr_as_virtual();
            let end_vaddr = start_vaddr + u64::try_from(fb.info().size()).unwrap();
            (start_vaddr, end_vaddr)
        });
        for p4_index in usize::from(start_vaddr.p4_index())..=usize::from(end_vaddr.p4_index()) {
            table[p4_index] = old_table[p4_index].clone();
        }

        info!("Switching to a new level 4 page table");

        unsafe {
            Cr3::write(core::mem::transmute(frame), 0);
            OffsetPageTable::new(&mut *table, physical_offset)
        }
    };

    // Create a new page table hierarchy for the kernel
    let (kernel_page_table, kernel_level_4_frame) = {
        let frame = frame_allocator
            .allocate_frame()
            .expect("Failed to allocate a frame");

        debug!(
            "Kernel level 4 page table is at {:#x}",
            frame.start_address().as_u64()
        );

        let ptr: *mut PageTable = (physical_offset + frame.start_address().as_u64()).as_mut_ptr();

        // Safety:
        // We are writing a page table to a valid physical address
        // mapped in the virtual address space.
        let table = unsafe {
            ptr.write(PageTable::new());
            &mut *ptr
        };

        (
            unsafe { OffsetPageTable::new(table, physical_offset) },
            frame,
        )
    };

    PageTables {
        bootloader: bootloader_page_table,
        kernel: kernel_page_table,
        kernel_level_4_frame,
    }
}
