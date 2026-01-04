#![no_std]
#![forbid(unsafe_op_in_unsafe_fn)]
#![warn(clippy::pedantic, clippy::nursery)]
#![allow(clippy::missing_panics_doc, clippy::similar_names)]

use beskar_core::{
    arch::{
        VirtAddr,
        paging::{CacheFlush as _, FrameAllocator as _, Mapper, Page},
    },
    mem::ranges::MemoryRange,
};
use beskar_hal::paging::page_table::Flags;
use bootloader_api::{BOOT_INFO_BASE, BootInfo};
use core::alloc::Layout;
use mem::{EarlyFrameAllocator, Mappings, PageTables};

pub mod arch;
pub mod fs;
pub mod mem;
pub mod system;
pub mod video;

mod kernel_elf;

const KERNEL_STACK_NB_PAGES: u64 = 64; // 256 KiB

#[must_use]
pub fn create_boot_info(
    mut frame_allocator: EarlyFrameAllocator,
    page_tables: &mut PageTables,
    mappings: &mut Mappings,
) -> VirtAddr {
    let max_region_count = frame_allocator.mem_map_max_region_count();

    let (layout, memory_regions_offset) = Layout::new::<BootInfo>()
        .extend(Layout::array::<MemoryRange>(max_region_count).unwrap())
        .unwrap();

    let boot_info_addr = BOOT_INFO_BASE;

    let memory_map_regions_addr = boot_info_addr + u64::try_from(memory_regions_offset).unwrap();
    let memory_map_regions_end = boot_info_addr + u64::try_from(layout.size()).unwrap();

    let start_page = Page::containing_address(boot_info_addr);
    let end_page = Page::containing_address(memory_map_regions_end - 1);

    for page in Page::range_inclusive(start_page, end_page) {
        let flags = Flags::PRESENT | Flags::WRITABLE | Flags::NO_EXECUTE;

        let frame = frame_allocator
            .allocate_frame()
            .expect("Failed to allocate a frame");

        for table in [&mut page_tables.kernel, &mut page_tables.bootloader] {
            table
                .map(page, frame, flags, &mut frame_allocator)
                .expect("Failed to map boot information")
                .flush();
        }
    }

    // Safety: We just allocated enough memory for a slice of enough `MemoryRange` structs.
    let memory_regions =
        unsafe { frame_allocator.construct_memory_map(memory_map_regions_addr, max_region_count) };

    // Safety: We are writing to a valid memory region, and converting its pointer to a mutable reference.
    unsafe {
        boot_info_addr.as_mut_ptr::<BootInfo>().write(BootInfo {
            memory_regions,
            framebuffer: crate::video::with_physical_framebuffer(|fb| {
                fb.to_framebuffer(mappings.framebuffer())
            }),
            recursive_index: mappings.recursive_index(),
            rsdp_paddr: crate::arch::acpi::rsdp_paddr(),
            kernel_info: mappings.kernel_info(),
            ramdisk_info: mappings.ramdisk_info(),
            cpu_count: crate::system::core_count(),
        });

        info!("Boot info created");
        debug!("Boot info written to {:#x}", boot_info_addr.as_u64());

        boot_info_addr
    }
}
