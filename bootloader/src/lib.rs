#![no_main]
#![no_std]
#![warn(clippy::pedantic, clippy::nursery)]
#![allow(clippy::missing_panics_doc, clippy::similar_names)]

use beskar_core::{boot::BootInfo, mem::MemoryRegion};
use mem::{EarlyFrameAllocator, Mappings, PageTables};
use x86_64::structures::paging::{FrameAllocator, Mapper, Page, PageTableFlags};

pub mod arch;
pub mod fs;
pub mod log;
pub mod mem;
pub mod system;
pub mod video;

mod kernel_elf;

// The amount of pages should be kept in sync with the stack size allocated by the bootloader
const KERNEL_STACK_NB_PAGES: u64 = 64; // 256 KiB

#[macro_export]
macro_rules! entry_point {
    ($path:path) => {
        #[unsafe(export_name = "_start")]
        pub extern "C" fn __kernel_entry(
            boot_info: &'static mut ::beskar_core::boot::BootInfo,
        ) -> ! {
            ($path)(boot_info)
        }
    };
}

#[must_use]
pub fn create_boot_info(
    mut frame_allocator: EarlyFrameAllocator,
    page_tables: &mut PageTables,
    mappings: &mut Mappings,
) -> x86_64::VirtAddr {
    let (layout, memory_regions_offset) = core::alloc::Layout::new::<BootInfo>()
        .extend(
            core::alloc::Layout::array::<MemoryRegion>(
                frame_allocator.memory_map_max_region_count(),
            )
            .unwrap(),
        )
        .unwrap();

    let boot_info_addr = mappings
        .level_4_entries_mut()
        .get_free_address(u64::try_from(layout.size()).unwrap());

    let memory_map_regions_addr = boot_info_addr + u64::try_from(memory_regions_offset).unwrap();
    let memory_map_regions_end = boot_info_addr + u64::try_from(layout.size()).unwrap();

    let start_page = Page::containing_address(boot_info_addr);
    let end_page = Page::containing_address(memory_map_regions_end - 1);

    for page in Page::range_inclusive(start_page, end_page) {
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_EXECUTE;

        let frame = frame_allocator
            .allocate_frame()
            .expect("Failed to allocate a frame");

        for table in [&mut page_tables.kernel, &mut page_tables.bootloader] {
            unsafe {
                table
                    .map_to(page, frame, flags, &mut frame_allocator)
                    .expect("Failed to map boot info page")
                    .flush();
            }
        }
    }

    let max_region_count = frame_allocator.memory_map_max_region_count();

    let memory_regions = frame_allocator.construct_memory_map(
        unsafe {
            core::slice::from_raw_parts_mut(memory_map_regions_addr.as_mut_ptr(), max_region_count)
        },
        mappings.kernel_info().paddr(),
        mappings.kernel_info().size(),
    );

    // ## Safety
    // We are writing to a valid memory region, and converting its pointer to a mutable reference.
    unsafe {
        boot_info_addr.as_mut_ptr::<BootInfo>().write(BootInfo {
            memory_regions,
            framebuffer: crate::video::with_physical_framebuffer(|fb| {
                fb.to_framebuffer(core::mem::transmute(mappings.framebuffer()))
            }),
            recursive_index: u16::from(mappings.recursive_index()),
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
