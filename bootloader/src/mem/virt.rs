use x86_64::{
    PhysAddr, VirtAddr,
    registers::segmentation::{self, Segment},
    structures::{
        gdt::GlobalDescriptorTable,
        paging::{
            FrameAllocator, Mapper, Page, PageSize, PageTableFlags, PageTableIndex, PhysFrame,
            Size4KiB,
        },
    },
};
use xmas_elf::{ElfFile, program::ProgramHeader};

use crate::{KERNEL_STACK_SIZE, chg_ctx, debug, info, kernel_elf, structs::TlsTemplate};

use super::{EarlyFrameAllocator, PageTables};

/// Keeps track of used entries in the level 4 page table.
pub struct Level4Entries([bool; 512]);

impl Level4Entries {
    #[must_use]
    #[allow(clippy::missing_panics_doc)]
    pub fn new(max_phys_addr: PhysAddr) -> Self {
        let mut usage = [false; 512];

        // Mark identity-mapped memory as used
        let start_page = Page::<Size4KiB>::containing_address(VirtAddr::new(0));
        let end_page = Page::<Size4KiB>::containing_address(VirtAddr::new(max_phys_addr.as_u64()));

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
            let end = start + u64::try_from(fb.info().size()).unwrap() - 1;
            (start, end)
        });

        let start_page = Page::<Size4KiB>::containing_address(start);
        let end_page = Page::<Size4KiB>::containing_address(end);

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

            let start_page = Page::<Size4KiB>::containing_address(start);
            let end_page = Page::<Size4KiB>::containing_address(end);

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
    pub fn get_free_entries(&mut self, num: usize) -> PageTableIndex {
        // TODO: ASLR
        let index = self
            .internal_entries()
            .windows(num)
            .position(|entries| entries.iter().all(|used| !used))
            .expect("No suitable level 4 entries found");

        for i in 0..num {
            self.0[index + i] = true;
        }

        PageTableIndex::new(u16::try_from(index).unwrap())
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
        let needed_lvl4_entries = size.div_ceil(512 * 512 * 512 * Size4KiB::SIZE);

        // TODO: ASLR (add random offset, need to manage alignment)
        Page::from_page_table_indices_1gib(
            self.get_free_entries(usize::try_from(needed_lvl4_entries).unwrap()),
            PageTableIndex::new(0),
        )
        .start_address()
    }
}

#[allow(clippy::too_many_lines)]
#[must_use]
/// This function initializes the memory mappings.
///
/// These mappings will be sent to and used by the kernel.
pub fn make_mappings(
    kernel: &ElfFile,
    frame_allocator: &mut EarlyFrameAllocator,
    page_tables: &mut PageTables,
) -> Mappings {
    let mut level_4_entries = Level4Entries::new(frame_allocator.max_physical_address());

    let kernel_paddr = PhysAddr::new(kernel.input.as_ptr() as u64);
    let kernel_len = u64::try_from(kernel.input.len()).unwrap();

    let kernel_elf::KernelInfo {
        image_offset: kernel_vaddr,
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
    debug!("Kernel image offset: {:#x}", kernel_vaddr.as_u64());
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
        let (start_frame, end_frame, start_page) = crate::video::with_physical_framebuffer(|fb| {
            let start_frame = PhysFrame::<Size4KiB>::containing_address(fb.start_addr());
            let end_frame = PhysFrame::<Size4KiB>::containing_address(
                fb.start_addr() + (u64::try_from(fb.info().size()).unwrap() - 1),
            );

            let start_page = Page::<Size4KiB>::from_start_address(
                level_4_entries.get_free_address(u64::try_from(fb.info().size()).unwrap()),
            )
            .unwrap();

            (start_frame, end_frame, start_page)
        });

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

        kernel_addr: kernel_paddr,
        kernel_len,
        kernel_vaddr,
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
    kernel_vaddr: VirtAddr,
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
    pub(crate) fn level_4_entries_mut(&mut self) -> &mut Level4Entries {
        &mut self.level_4_entries
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
    pub const fn kernel_vaddr(&self) -> VirtAddr {
        self.kernel_vaddr
    }
}
