use beskar_core::{
    arch::{
        PhysAddr, VirtAddr,
        paging::{
            CacheFlush as _, Frame, FrameAllocator as _, M1GiB, M4KiB, Mapper, MemSize as _, Page,
        },
    },
    boot::{KernelInfo, RamdiskInfo},
};
use beskar_hal::{
    paging::page_table::Flags,
    registers::{CS, SS},
    structures::{GdtDescriptor, GlobalDescriptorTable},
};
use xmas_elf::{ElfFile, program::ProgramHeader};

use crate::{KERNEL_STACK_NB_PAGES, arch::chg_ctx, debug, info, kernel_elf};

use super::{EarlyFrameAllocator, PageTables};

const KERNEL_STACK_SIZE: u64 = KERNEL_STACK_NB_PAGES * M4KiB::SIZE;

/// Keeps track of used entries in the level 4 page table.
pub struct Level4Entries([bool; 512]);

impl Level4Entries {
    #[must_use]
    pub fn new(max_phys_addr: PhysAddr) -> Self {
        let mut usage = [false; 512];

        // Mark identity-mapped memory as used
        let start_page = Page::<M4KiB>::containing_address(VirtAddr::new(0));
        let end_page = Page::<M4KiB>::containing_address(VirtAddr::new(max_phys_addr.as_u64()));

        for used_page in usage
            .iter_mut()
            .take(usize::from(end_page.p4_index()) + 1)
            .skip(usize::from(start_page.p4_index()))
        {
            *used_page = true;
        }

        // Mark framebuffer as used
        let (start, end) = crate::video::with_physical_framebuffer(|fb| {
            let start = VirtAddr::new(fb.start_addr().as_u64());
            let end = start + u64::from(fb.info().size()) - 1;
            (start, end)
        });

        let start_page = Page::<M4KiB>::containing_address(start);
        let end_page = Page::<M4KiB>::containing_address(end);

        for used_page in usage
            .iter_mut()
            .take(usize::from(end_page.p4_index()) + 1)
            .skip(usize::from(start_page.p4_index()))
        {
            *used_page = true;
        }

        Self(usage)
    }

    #[must_use]
    #[inline]
    const fn internal_entries(&self) -> &[bool; 512] {
        &self.0
    }

    /// Marks the virtual address range of all segments as used.
    pub fn mark_segments<'a>(
        &mut self,
        segments: impl Iterator<Item = ProgramHeader<'a>>,
        virtual_address_offset: u64,
    ) {
        for segment in segments.filter(|s| s.mem_size() > 0) {
            let start = VirtAddr::new(virtual_address_offset + segment.virtual_addr());
            let end = start + segment.mem_size() - 1;

            let start_page = Page::<M4KiB>::containing_address(start);
            let end_page = Page::<M4KiB>::containing_address(end);

            for i in usize::from(start_page.p4_index())..=usize::from(end_page.p4_index()) {
                self.0[i] = true;
            }
        }
    }

    /// Returns the first index of `num` contiguous unused level 4 entries.
    ///
    /// ## Note
    ///
    /// Marks each returned index as used.
    ///
    /// ## Panics
    ///
    /// Panics if no contiguous free entries are found.
    pub fn get_free_entries(&mut self, num: usize) -> u16 {
        // TODO: ASLR
        let index = self
            .internal_entries()
            .windows(num)
            .position(|entries| entries.iter().all(|used| !used))
            .expect("No suitable level 4 entries found");

        for i in 0..num {
            self.0[index + i] = true;
        }

        u16::try_from(index).unwrap()
    }

    /// Returns a virtual address that is not used.
    ///
    /// ## Note
    ///
    /// Marks associated entries indices as used.
    ///
    /// ## Panics
    ///
    /// Panics if no contiguous free memory is found.
    pub fn get_free_address(&mut self, size: u64) -> VirtAddr {
        let needed_lvl4_entries = size.div_ceil(512 * M1GiB::SIZE);

        // TODO: ASLR (add random offset, need to manage alignment)
        Page::from_p4p3(
            self.get_free_entries(usize::try_from(needed_lvl4_entries).unwrap()),
            0,
        )
        .start_address()
    }
}

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
    let mut level_4_entries = Level4Entries::new(frame_allocator.max_physical_address());

    // Get the recursive index
    let recursive_index = {
        let index = level_4_entries.get_free_entries(1);

        let entry = &mut page_tables.kernel.entries_mut()[usize::from(index)];
        assert!(entry.is_null(), "Recursive mapping entry is already in use");

        let flags = Flags::PRESENT | Flags::WRITABLE | Flags::NO_EXECUTE;

        entry.set(page_tables.kernel_level_4_frame.start_address(), flags);

        index
    };
    debug!("Recursive page table index is {}", recursive_index);

    // Map kernel
    let (kernel_entry_point, kernel_info) = {
        let kernel_paddr = PhysAddr::new(kernel.input.as_ptr() as u64);

        let kernel_elf::LoadedKernelInfo {
            image_offset: kernel_vaddr,
            entry_point: kernel_entry_point,
            kernel_size,
        } = crate::kernel_elf::load_kernel_elf(kernel_elf::KernelLoadingUtils::new(
            kernel,
            &mut level_4_entries,
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

    // Map ramdisk
    let ramdisk_info = ramdisk.map(|ramdisk| {
        let size = u64::try_from(ramdisk.len()).unwrap();

        let ramdisk_paddr = PhysAddr::new(ramdisk.as_ptr() as u64);
        let start_frame = Frame::from_start_address(ramdisk_paddr).unwrap();
        let end_frame = start_frame + (size / M4KiB::SIZE);

        let start_page = {
            let start_addr = level_4_entries.get_free_address(size);
            Page::<M4KiB>::from_start_address(start_addr).unwrap()
        };
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
        let stack_start_page = {
            let guard_page = Page::<M4KiB>::from_start_address(
                // Allocate a guard page
                level_4_entries.get_free_address(M4KiB::SIZE + KERNEL_STACK_SIZE),
            )
            .unwrap();

            guard_page + 1
        };
        let stack_end_addr =
            (stack_start_page.start_address() + KERNEL_STACK_SIZE).align_down(16_u64);
        let stack_end_page = Page::containing_address(stack_end_addr - 1);

        for page in Page::range_inclusive(stack_start_page, stack_end_page) {
            let frame = frame_allocator
                .allocate_frame()
                .expect("Failed to allocate a frame");

            let flags = Flags::PRESENT | Flags::WRITABLE | Flags::NO_EXECUTE;

            page_tables
                .kernel
                .map(page, frame, flags, frame_allocator)
                .flush();
        }

        info!("Kernel stack is setup");
        debug!("Kernel stack top at {:#x}", stack_end_addr.as_u64());

        stack_end_addr
    };

    // Identity map the jump code
    {
        let chg_ctx_function_addr = PhysAddr::new(u64::try_from(chg_ctx as usize).unwrap());
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

    // Handle GDT
    {
        let gdt_frame = frame_allocator
            .allocate_frame()
            .expect("Failed to allocate a frame");
        // Identity-map
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

    // Map framebuffer
    let framebuffer_virt_addr = {
        let (start_frame, end_frame, start_page) = crate::video::with_physical_framebuffer(|fb| {
            let start_frame = Frame::<M4KiB>::containing_address(fb.start_addr());
            let end_frame = Frame::<M4KiB>::containing_address(
                fb.start_addr() + (u64::from(fb.info().size()) - 1),
            );

            let start_page = Page::<M4KiB>::from_start_address(
                level_4_entries.get_free_address(u64::from(fb.info().size())),
            )
            .unwrap();

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

    Mappings {
        stack_top: stack_end_addr,
        entry_point: kernel_entry_point,
        level_4_entries,
        framebuffer: framebuffer_virt_addr,
        recursive_index,
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
    /// List of used entries in the level 4 page table.
    level_4_entries: Level4Entries,
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
    pub const fn level_4_entries(&self) -> &Level4Entries {
        &self.level_4_entries
    }

    #[must_use]
    #[inline]
    pub(crate) const fn level_4_entries_mut(&mut self) -> &mut Level4Entries {
        &mut self.level_4_entries
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
