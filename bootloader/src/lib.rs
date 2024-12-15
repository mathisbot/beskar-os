#![no_main]
#![no_std]
#![warn(clippy::pedantic, clippy::nursery)]
#![allow(clippy::missing_panics_doc, clippy::similar_names)]

#[cfg(not(target_arch = "x86_64"))]
compile_error!("BeskarOS bootloader only supports x86_64 architecture");

use info::{MemoryRegion, TlsTemplate};
use log::{debug, info};
use mem::{EarlyFrameAllocator, Level4Entries, PageTables};
use x86_64::{
    instructions::segmentation,
    registers::{self, segmentation::Segment},
    structures::{
        gdt::GlobalDescriptorTable,
        paging::{
            FrameAllocator, Mapper, Page, PageSize, PageTableFlags, PageTableIndex, PhysFrame,
            Size4KiB,
        },
    },
    PhysAddr, VirtAddr,
};
use xmas_elf::ElfFile;

pub mod mem;

mod framebuffer;
pub use framebuffer::{FrameBuffer, FrameBufferInfo, FrameBufferWriter, PixelBitmask, PixelFormat};

pub mod info;
pub use info::{BootInfo, EarlySystemInfo};

pub mod logging;

mod kernel_elf;

const KERNEL_STACK_SIZE: u64 = 64 * 4096; // 256 KiB

#[macro_export]
macro_rules! entry_point {
    ($path:path) => {
        #[export_name = "_start"]
        pub extern "C" fn __kernel_entry(boot_info: &'static mut $crate::BootInfo) -> ! {
            ($path)(boot_info)
        }
    };
}

pub fn map_memory_and_jump(
    kernel: &ElfFile,
    mut frame_allocator: EarlyFrameAllocator,
    mut page_tables: PageTables,
    mut system_info: EarlySystemInfo,
) -> ! {
    let mut mappings = make_mappings(kernel, &mut frame_allocator, &mut page_tables, &system_info);

    let boot_info = create_boot_info(
        frame_allocator,
        &mut page_tables,
        &mut mappings,
        &system_info,
    );

    info!("Jumping to kernel, see you on the other side!");

    // Optionally stall here for debugging
    // loop {
    //     x86_64::instructions::hlt();
    // }

    // Clear screen before jumping to the kernel
    system_info.framebuffer.buffer_mut().fill(0);

    unsafe {
        chg_ctx(
            page_tables.kernel_level_4_frame.start_address().as_u64(),
            mappings.stack_top().as_u64(),
            mappings.entry_point().as_u64(),
            core::ptr::from_ref(boot_info) as u64,
        );
    };
}

/// Change context and jump to the kernel entry point.
///
/// ## Safety
///
/// The caller must ensure that the four adresses are valid.
unsafe fn chg_ctx(
    level4_frame_addr: u64,
    stack_top: u64,
    entry_point_addr: u64,
    boot_info_addr: u64,
) -> ! {
    // Safety:
    // We are resetting the stack, which is safe if we do not intend to return.
    // We are setting the CR3 register to a valid page table, which is safe.
    // We are also putting boot info into rdi, according to the C calling convention.
    // Finally, jumping to the kernel entry point is safe, as it is valid.
    unsafe {
        core::arch::asm!(
            r#"
            xor rbp, rbp
            mov cr3, {}
            mov rsp, {}
            jmp {}
            "#,
            in(reg) level4_frame_addr,
            in(reg) stack_top,
            in(reg) entry_point_addr,
            in("rdi") boot_info_addr,
            options(noreturn)
        );
    };
}

#[allow(clippy::too_many_lines)]
#[must_use]
/// This function initializes the memory mappings.
///
/// These mappings will be sent to and used by the kernel.
fn make_mappings(
    kernel: &ElfFile,
    frame_allocator: &mut EarlyFrameAllocator,
    page_tables: &mut PageTables,
    system_info: &EarlySystemInfo,
) -> Mappings {
    let mut level_4_entries = Level4Entries::new(
        &system_info.framebuffer,
        frame_allocator.max_physical_address().as_u64(),
    );

    // Enable support for no execute pages.
    unsafe {
        x86_64::registers::control::Efer::update(|efer| {
            efer.insert(registers::control::EferFlags::NO_EXECUTE_ENABLE);
        });
    };

    // Enable support for write protection in Ring-0.
    unsafe {
        x86_64::registers::control::Cr0::update(|cr0| {
            cr0.insert(registers::control::Cr0Flags::WRITE_PROTECT);
        });
    };

    let kernel_start = PhysAddr::new(kernel.input.as_ptr() as u64);
    let kernel_len = u64::try_from(kernel.input.len()).unwrap();

    let kernel_elf::KernelInfo {
        image_offset: kernel_image_offset,
        entry_point: kernel_entry_point,
        tls_template,
    } = crate::kernel_elf::load_kernel_elf(kernel_elf::KernelLoadingUtils::new(
        kernel,
        &mut level_4_entries,
        &mut page_tables.kernel,
        frame_allocator,
    ));

    info!("Kernel loaded");
    debug!("Kernel entry point at {:#x}", kernel_entry_point.as_u64());
    debug!("Kernel image offset: {:#x}", kernel_image_offset.as_u64());
    if tls_template.is_some() {
        info!("TLS template found");
    }

    let stack_start_page = {
        let guard_page = Page::<Size4KiB>::from_start_address(
            // Allocate a guard page
            level_4_entries.get_free_address(Size4KiB::SIZE + KERNEL_STACK_SIZE),
        )
        .unwrap();

        guard_page + 1
    };
    let stack_end_addr = (stack_start_page.start_address() + KERNEL_STACK_SIZE).align_down(16_u64);
    let stack_end_page = Page::containing_address(stack_end_addr - 1);

    for page in Page::range_inclusive(stack_start_page, stack_end_page) {
        let frame = frame_allocator
            .allocate_frame()
            .expect("Failed to allocate a frame");

        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_EXECUTE;

        unsafe {
            page_tables
                .kernel
                .map_to(page, frame, flags, frame_allocator)
        }
        .expect("Failed to map stack page")
        .flush();
    }

    info!("Setup stack");
    debug!("Stack top at {:#x}", stack_end_addr);

    let chg_ctx_function_addr = PhysAddr::new(chg_ctx as *const () as u64);
    let chg_ctx_function_frame = PhysFrame::<Size4KiB>::containing_address(chg_ctx_function_addr);

    for frame in PhysFrame::range_inclusive(chg_ctx_function_frame, chg_ctx_function_frame + 1) {
        let page = Page::containing_address(VirtAddr::new(frame.start_address().as_u64()));

        unsafe {
            page_tables.kernel.map_to_with_table_flags(
                page,
                frame,
                PageTableFlags::PRESENT,
                PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
                frame_allocator,
            )
        }
        .expect("Failed to map chg_ctx page")
        .flush();
    }

    info!("Mapped jump code");
    debug!("chg_ctx function at {:#x}", chg_ctx_function_addr.as_u64());

    let gdt_frame = frame_allocator
        .allocate_frame()
        .expect("Failed to allocate a frame");

    let gdt_virt_addr = VirtAddr::new(gdt_frame.start_address().as_u64());
    let ptr: *mut GlobalDescriptorTable = gdt_virt_addr.as_mut_ptr();
    let mut gdt = GlobalDescriptorTable::new();

    let code_selector = gdt.append(x86_64::structures::gdt::Descriptor::kernel_code_segment());
    let data_selector = gdt.append(x86_64::structures::gdt::Descriptor::kernel_data_segment());

    unsafe {
        ptr.write(gdt);
        &*ptr
    }
    .load();

    unsafe {
        segmentation::CS::set_reg(code_selector);
        segmentation::SS::set_reg(data_selector);
    }

    let gdt_page = Page::containing_address(VirtAddr::new(gdt_frame.start_address().as_u64()));
    unsafe {
        page_tables.kernel.map_to_with_table_flags(
            gdt_page,
            gdt_frame,
            PageTableFlags::PRESENT,
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
            frame_allocator,
        )
    }
    .expect("Failed to map GDT page")
    .flush();

    info!("Mapped GDT");
    debug!("GDT at {:#x}", gdt_frame.start_address().as_u64());

    let framebuffer_virt_addr = {
        let start_frame = PhysFrame::<Size4KiB>::containing_address(PhysAddr::new(
            system_info.framebuffer.buffer_start(),
        ));
        let end_frame = PhysFrame::<Size4KiB>::containing_address(PhysAddr::new(
            system_info.framebuffer.buffer_start()
                + u64::try_from(system_info.framebuffer.info().size).unwrap()
                - 1,
        ));

        let start_page = Page::<Size4KiB>::from_start_address(
            level_4_entries
                .get_free_address(u64::try_from(system_info.framebuffer.info().size).unwrap()),
        )
        .unwrap();

        for (i, frame) in PhysFrame::range_inclusive(start_frame, end_frame).enumerate() {
            let page = start_page + u64::try_from(i).unwrap();
            let flags =
                PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_EXECUTE;
            unsafe {
                page_tables
                    .kernel
                    .map_to(page, frame, flags, frame_allocator)
            }
            .expect("Failed to map framebuffer page")
            .flush();
        }

        info!("Mapped framebuffer");
        debug!("Framebuffer at {:#x}", start_page.start_address().as_u64());

        start_page.start_address()
    };

    let recursive_index = {
        let index = level_4_entries.get_free_entries(1);

        let entry = &mut page_tables.kernel.level_4_table_mut()[index];
        assert!(
            entry.is_unused(),
            "Recursive mapping entry is already in use"
        );

        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_EXECUTE;

        entry.set_frame(page_tables.kernel_level_4_frame, flags);

        index
    };

    info!("Mapped recursive index");
    debug!(
        "Recursive page table index is {}",
        u16::from(recursive_index)
    );

    Mappings {
        stack_top: stack_end_addr,
        entry_point: kernel_entry_point,
        level_4_entries,
        framebuffer: framebuffer_virt_addr,
        recursive_index,
        tls_template,

        kernel_addr: kernel_start,
        kernel_len,
        kernel_image_offset,
    }
}

#[must_use]
fn create_boot_info(
    mut frame_allocator: EarlyFrameAllocator,
    page_tables: &mut PageTables,
    mappings: &mut Mappings,
    system_info: &EarlySystemInfo,
) -> &'static mut BootInfo {
    let (layout, memory_regions_offset) = core::alloc::Layout::new::<BootInfo>()
        .extend(
            core::alloc::Layout::array::<MemoryRegion>(
                frame_allocator.memory_map_max_region_count(),
            )
            .unwrap(),
        )
        .unwrap();

    let boot_info_addr = mappings
        .level_4_entries
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
        mappings.kernel_addr(),
        mappings.kernel_len(),
    );

    // ## Safety
    // We are writing to a valid memory region, and converting its pointer to a mutable reference.
    unsafe {
        boot_info_addr.as_mut_ptr::<BootInfo>().write(BootInfo {
            memory_regions,
            // Framebuffer in the kernel has to be accessed using the virtual address found in the mappings
            // instead of the physical address.
            framebuffer: FrameBuffer::new(
                mappings.framebuffer.as_u64(),
                system_info.framebuffer.info(),
            ),
            recursive_index: u16::from(mappings.recursive_index()),
            rsdp_paddr: system_info.rsdp_paddr.map(x86_64::PhysAddr::as_u64), // RSDP address is physical because it is not mapped
            kernel_vaddr: mappings.kernel_addr().as_u64(),
            kernel_len: mappings.kernel_len(),
            kernel_image_offset: mappings.kernel_image_offset().as_u64(),
            tls_template: mappings.tls_template(),
            cpu_count: system_info.cpu_count,
        });

        info!("Boot info created");
        debug!("Boot info written to {:#x}", boot_info_addr.as_u64());

        &mut *boot_info_addr.as_mut_ptr()
    }
}

/// Represents the memory mappings that will be used by the kernel.
pub struct Mappings {
    // Memory information
    /// The top of the stack.
    stack_top: VirtAddr,
    /// The address of the entry point of the kernel.
    entry_point: VirtAddr,
    /// List of used entries in the level 4 page table.
    level_4_entries: Level4Entries,
    /// The start of the framebuffer.
    framebuffer: VirtAddr,
    /// The recursive mapping index in the level 4 page table.
    recursive_index: PageTableIndex,
    /// An optional thread local storage template.
    tls_template: Option<TlsTemplate>,

    // Kernel-related
    /// The address of the kernel ELF in memory.
    kernel_addr: PhysAddr,
    /// The size of the kernel ELF in memory.
    kernel_len: u64,
    /// The offset of the kernel image.
    kernel_image_offset: VirtAddr,
}

impl Mappings {
    #[must_use]
    #[inline]
    pub const fn stack_top(&self) -> VirtAddr {
        self.stack_top
    }

    #[must_use]
    #[inline]
    pub const fn entry_point(&self) -> VirtAddr {
        self.entry_point
    }

    #[must_use]
    #[inline]
    pub const fn level_4_entries(&self) -> &Level4Entries {
        &self.level_4_entries
    }

    #[must_use]
    #[inline]
    pub const fn framebuffer(&self) -> VirtAddr {
        self.framebuffer
    }

    #[must_use]
    #[inline]
    pub const fn recursive_index(&self) -> PageTableIndex {
        self.recursive_index
    }

    #[must_use]
    #[inline]
    pub const fn tls_template(&self) -> Option<TlsTemplate> {
        self.tls_template
    }

    #[must_use]
    #[inline]
    pub const fn kernel_addr(&self) -> PhysAddr {
        self.kernel_addr
    }

    #[must_use]
    #[inline]
    pub const fn kernel_len(&self) -> u64 {
        self.kernel_len
    }

    #[must_use]
    #[inline]
    pub const fn kernel_image_offset(&self) -> VirtAddr {
        self.kernel_image_offset
    }
}
