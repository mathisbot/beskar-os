use super::{EarlyFrameAllocator, PageTables};
use crate::{KERNEL_STACK_NB_PAGES, arch::chg_ctx, debug, info, kernel_elf};
use beskar_core::arch::{
    PhysAddr, VirtAddr,
    paging::{CacheFlush as _, Frame, FrameAllocator as _, M4KiB, Mapper, MemSize as _, Page},
};
use beskar_hal::{
    paging::page_table::Flags,
    registers::{CS, SS},
    structures::{GdtDescriptor, GlobalDescriptorTable},
};
use bootloader_api::{
    FRAMEBUFFER_BASE, KERNEL_PT_RECURSIVE_INDEX, KERNEL_STACK_BASE, KernelInfo, RAMDISK_BASE,
    RamdiskInfo,
};
use xmas_elf::ElfFile;

#[expect(clippy::too_many_lines, reason = "Many mappings to do")]
#[must_use]
/// This function initializes the memory mappings.
///
/// These mappings will be sent to and used by the kernel.
pub fn make_mappings(
    kernel: &ElfFile,
    ramdisk: Option<&[u8]>,
    frame_allocator: &mut EarlyFrameAllocator,
    page_tables: &mut PageTables,
) -> Mappings {
    // Assert recursive mapping
    page_tables.kernel.entries_mut()[usize::from(KERNEL_PT_RECURSIVE_INDEX)].set(
        page_tables.kernel_level_4_frame.start_address(),
        Flags::PRESENT | Flags::WRITABLE | Flags::NO_EXECUTE,
    );

    // Map kernel
    let (kernel_entry_point, kernel_info) = {
        let kernel_paddr = PhysAddr::new(kernel.input.as_ptr() as u64);

        let kernel_elf::LoadedKernelInfo {
            image_offset: kernel_vaddr,
            entry_point: kernel_entry_point,
            kernel_size,
        } = crate::kernel_elf::load_kernel_elf(kernel_elf::KernelLoadingUtils::new(
            kernel,
            &mut page_tables.kernel,
            frame_allocator,
        ));

        info!("Kernel loaded");
        debug!("Kernel code at {:#x}", kernel_vaddr.as_u64());
        debug!("Kernel entry point at {:#x}", kernel_entry_point.as_u64());

        (
            kernel_entry_point,
            KernelInfo::new(kernel_paddr, kernel_vaddr, kernel_size),
        )
    };

    // Map framebuffer
    let framebuffer_virt_addr = {
        let (start_frame, end_frame, start_page) = crate::video::with_physical_framebuffer(|fb| {
            let start_frame = Frame::<M4KiB>::containing_address(fb.start_addr());
            let end_frame = Frame::<M4KiB>::containing_address(
                fb.start_addr() + (u64::from(fb.info().size()) - 1),
            );
            let start_page = Page::<M4KiB>::from_start_address(FRAMEBUFFER_BASE).unwrap();
            (start_frame, end_frame, start_page)
        });
        for (i, frame) in Frame::range_inclusive(start_frame, end_frame)
            .into_iter()
            .enumerate()
        {
            let page = start_page + u64::try_from(i).unwrap();
            let flags = Flags::PRESENT | Flags::WRITABLE | Flags::NO_EXECUTE;
            page_tables
                .kernel
                .map(page, frame, flags, frame_allocator)
                .flush();
        }
        info!("Mapped framebuffer");
        debug!("Framebuffer at {:#x}", start_page.start_address().as_u64());
        start_page.start_address()
    };

    // Map ramdisk en higher-half (si présent)
    let ramdisk_info = ramdisk.map(|ramdisk| {
        let size = u64::try_from(ramdisk.len()).unwrap();
        let ramdisk_paddr = PhysAddr::new(ramdisk.as_ptr() as u64);
        let start_frame = Frame::from_start_address(ramdisk_paddr).unwrap();
        let end_frame = start_frame + (size / M4KiB::SIZE);
        let start_page = Page::<M4KiB>::from_start_address(RAMDISK_BASE).unwrap();
        let end_page = start_page + (size / M4KiB::SIZE);
        for (page, frame) in Page::range_inclusive(start_page, end_page)
            .into_iter()
            .zip(Frame::range_inclusive(start_frame, end_frame))
        {
            let flags = Flags::PRESENT | Flags::NO_EXECUTE;
            page_tables
                .kernel
                .map(page, frame, flags, frame_allocator)
                .flush();
        }
        RamdiskInfo::new(start_page.start_address(), size)
    });

    let stack_end_addr = {
        let stack_range = {
            let guard_page = Page::<M4KiB>::from_start_address(KERNEL_STACK_BASE).unwrap();
            let start_page = guard_page + 1;
            let end_page = start_page + KERNEL_STACK_NB_PAGES - 1;
            Page::range_inclusive(start_page, end_page)
        };
        for page in stack_range {
            let frame = frame_allocator
                .allocate_frame()
                .expect("Failed to allocate a frame");
            let flags = Flags::PRESENT | Flags::WRITABLE | Flags::NO_EXECUTE;
            page_tables
                .kernel
                .map(page, frame, flags, frame_allocator)
                .flush();
        }
        let stack_end_addr = stack_range.end().start_address() + M4KiB::SIZE;
        info!("Kernel stack is setup");
        debug!("Kernel stack top at {:#x}", stack_end_addr.as_u64());
        stack_end_addr
    };

    // Identity map the jump code
    {
        let chg_ctx_function_addr =
            PhysAddr::new(u64::try_from(chg_ctx as *const () as usize).unwrap());
        let chg_ctx_function_frame = Frame::<M4KiB>::containing_address(chg_ctx_function_addr);
        for frame in Frame::range_inclusive(chg_ctx_function_frame, chg_ctx_function_frame + 1) {
            let page = Page::containing_address(VirtAddr::new(frame.start_address().as_u64()));
            page_tables
                .kernel
                .map(page, frame, Flags::PRESENT, frame_allocator)
                .flush();
        }
        info!("Mapped jump code");
        debug!(
            "Context switch function at {:#x}",
            chg_ctx_function_addr.as_u64()
        );
    }

    #[cfg(target_arch = "x86_64")]
    // Handle minimal GDT, kernel should create its own
    {
        let gdt_frame = frame_allocator
            .allocate_frame()
            .expect("Failed to allocate a frame");
        let gdt_virt_addr = VirtAddr::new(gdt_frame.start_address().as_u64());
        let mut gdt = GlobalDescriptorTable::empty();
        let code_selector = gdt.append(GdtDescriptor::kernel_code_segment());
        let data_selector = gdt.append(GdtDescriptor::kernel_data_segment());
        let ptr = gdt_virt_addr.as_mut_ptr::<GlobalDescriptorTable>();
        let gdt = unsafe {
            ptr.write(gdt);
            &*ptr
        };
        gdt.load();
        unsafe {
            CS::set(code_selector);
            SS::set(data_selector);
        }
        let gdt_page = Page::from_start_address(gdt_virt_addr).unwrap();
        page_tables
            .kernel
            .map(gdt_page, gdt_frame, Flags::PRESENT, frame_allocator)
            .flush();
        info!("Mapped GDT");
        debug!("GDT at {:#x}", gdt_page.start_address().as_u64());
    }

    Mappings {
        stack_top: stack_end_addr,
        entry_point: kernel_entry_point,
        framebuffer: framebuffer_virt_addr,
        recursive_index: KERNEL_PT_RECURSIVE_INDEX,
        kernel_info,
        ramdisk_info,
    }
}

/// Represents the memory mappings that will be used by the kernel.
pub struct Mappings {
    // Memory information
    /// The top of the stack.
    stack_top: VirtAddr,
    /// The address of the entry point of the kernel.
    entry_point: VirtAddr,
    /// The start of the framebuffer.
    framebuffer: VirtAddr,
    /// The recursive mapping index in the level 4 page table.
    recursive_index: u16,
    /// Various kernel information.
    kernel_info: KernelInfo,
    /// Various ramdisk information.
    ramdisk_info: Option<RamdiskInfo>,
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
    pub const fn framebuffer(&self) -> VirtAddr {
        self.framebuffer
    }

    #[must_use]
    #[inline]
    pub const fn recursive_index(&self) -> u16 {
        self.recursive_index
    }

    #[must_use]
    #[inline]
    pub const fn kernel_info(&self) -> KernelInfo {
        self.kernel_info
    }

    #[must_use]
    #[inline]
    pub const fn ramdisk_info(&self) -> Option<RamdiskInfo> {
        self.ramdisk_info
    }
}
