use crate::mem::EarlyFrameAllocator;
use beskar_core::arch::{
    PhysAddr, VirtAddr,
    paging::{CacheFlush, Frame, FrameAllocator, M4KiB, Mapper as _, MemSize, Page},
};
use beskar_hal::paging::page_table::{Flags, OffsetPageTable};
use bootloader_api::KERNEL_IMAGE_BASE;
use xmas_elf::{
    ElfFile,
    dynamic::Tag,
    header,
    program::{self, ProgramHeader, Type},
    sections::Rela,
};

pub struct KernelLoadingUtils<'a> {
    kernel: &'a ElfFile<'a>,
    page_table: &'a mut OffsetPageTable<'static>,
    frame_allocator: &'a mut EarlyFrameAllocator,
}

impl<'a> KernelLoadingUtils<'a> {
    #[must_use]
    #[inline]
    pub const fn new(
        kernel: &'a ElfFile<'a>,
        page_table: &'a mut OffsetPageTable<'static>,
        frame_allocator: &'a mut EarlyFrameAllocator,
    ) -> Self {
        Self {
            kernel,
            page_table,
            frame_allocator,
        }
    }
}

pub fn load_kernel_elf(mut klu: KernelLoadingUtils) -> LoadedKernelInfo {
    // Assert that the kernel is page aligned
    assert!(
        PhysAddr::new_truncate(core::ptr::from_ref::<u8>(&klu.kernel.input[0]) as u64)
            .is_aligned(M4KiB::ALIGNMENT),
        "Kernel is not page aligned"
    );

    // Make sure that the ELF file is valid
    header::sanity_check(klu.kernel).expect("ELF header is invalid");
    klu.kernel.program_iter().for_each(|program_header| {
        program::sanity_check(program_header, klu.kernel).expect("Program header is invalid");
    });

    // Make sure it is suitable to run on x86_64
    assert_eq!(
        klu.kernel.header.pt1.class(),
        header::Class::SixtyFour,
        "Kernel is unexpectedly not 64-bit."
    );
    assert_eq!(
        klu.kernel.header.pt2.machine().as_machine(),
        header::Machine::X86_64,
        "Kernel is unexpectedly not x86_64."
    );

    // Get the offset of the kernel image in the virtual address space
    let virtual_address_offset = {
        let (min_addr, max_addr) = klu
            .kernel
            .program_iter()
            .filter_map(|header| {
                if header.get_type() == Ok(xmas_elf::program::Type::Load) {
                    Some((
                        header.virtual_addr(),
                        header.virtual_addr() + header.mem_size(),
                    ))
                } else {
                    None
                }
            })
            .fold((u64::MAX, 0), |(min, max), (start, end)| {
                (min.min(start), max.max(end))
            });

        assert!(min_addr <= max_addr, "No loadable segments");

        KERNEL_IMAGE_BASE - min_addr
    }
    .as_u64();

    let _lsi = load_segments(&mut klu, virtual_address_offset);

    let total_size = klu
        .kernel
        .program_iter()
        .map(|ph| ph.virtual_addr() + ph.mem_size())
        .max()
        .unwrap_or(0);

    LoadedKernelInfo {
        entry_point: VirtAddr::new_extend(
            virtual_address_offset + klu.kernel.header.pt2.entry_point(),
        ),
        image_offset: VirtAddr::new_extend(virtual_address_offset),
        kernel_size: total_size,
    }
}

struct LoadedSegmentsInfo {}

fn load_segments(klu: &mut KernelLoadingUtils, vao: u64) -> LoadedSegmentsInfo {
    for program_header in klu.kernel.program_iter() {
        match program_header.get_type().unwrap() {
            Type::Load => {
                handle_segment_load(program_header, klu, vao);
            }
            Type::Tls => {
                crate::warn!("TLS segment found, but not used.");
            }
            Type::Interp => {
                panic!("Found unexpected interpreter segment");
            }
            _ => {}
        }
    }

    // Relocate memory addresses
    for program_header in klu.kernel.program_iter() {
        if program_header.get_type().unwrap() == Type::Dynamic {
            handle_segment_dynamic(program_header, klu, vao);
        }
    }

    // Mark memory as read-only after relocation
    for program_header in klu.kernel.program_iter() {
        if program_header.get_type().unwrap() == Type::GnuRelro {
            handle_segment_gnurelro(program_header, klu, vao);
        }
    }

    // Remove PageTableFlag::BIT_9 from the kernel page table
    // so that kernel can use it for other purposes.
    // It is currently use to help managing bss section.
    for program_header in klu.kernel.program_iter() {
        if program_header.get_type().unwrap() == Type::Load {
            let start = VirtAddr::new_extend(vao + program_header.virtual_addr());
            let end = VirtAddr::new_extend(
                vao + program_header.virtual_addr() + program_header.mem_size(),
            );

            let start_page = Page::<M4KiB>::containing_address(start);
            let end_page = Page::<M4KiB>::containing_address(end - 1);

            for page in Page::range_inclusive(start_page, end_page) {
                let (_, flags) = klu
                    .page_table
                    .translate(end_page)
                    .expect("Last page of segment is not mapped");

                if flags.contains(Flags::BIT_9) {
                    let new_flags = flags.without(Flags::BIT_9);
                    unsafe {
                        klu.page_table
                            .update_flags(page, new_flags)
                            .unwrap()
                            .ignore_flush();
                    }
                }
            }
        }
    }

    LoadedSegmentsInfo {}
}

fn handle_segment_load(load_segment: ProgramHeader, klu: &mut KernelLoadingUtils, vao: u64) {
    let phys_start = PhysAddr::new_truncate(core::ptr::from_ref::<u8>(&klu.kernel.input[0]) as u64)
        + load_segment.offset();

    let start_frame = Frame::<M4KiB>::containing_address(phys_start);
    let end_frame = Frame::<M4KiB>::containing_address(phys_start + load_segment.file_size() - 1);

    let virt_start = VirtAddr::new_extend(vao + load_segment.virtual_addr());
    let start_page = Page::<M4KiB>::containing_address(virt_start);

    let mut segment_flags = Flags::PRESENT | Flags::GLOBAL;
    if load_segment.flags().is_write() {
        segment_flags = segment_flags.union(Flags::WRITABLE);
    }
    if !load_segment.flags().is_execute() {
        segment_flags = segment_flags.union(Flags::NO_EXECUTE);
    }

    for frame in Frame::range_inclusive(start_frame, end_frame) {
        let page = start_page + (frame - start_frame);

        unsafe {
            klu.page_table
                .map(page, frame, segment_flags, klu.frame_allocator)
                .expect("Failed to map kernel ELF segment")
                .ignore_flush();
        }
    }

    // Map a zeroed-out section for the BSS segment
    if load_segment.mem_size() > load_segment.file_size() {
        zero_bss(virt_start, load_segment, klu);
    }
}

fn zero_bss(virt_start: VirtAddr, load_segment: ProgramHeader, klu: &mut KernelLoadingUtils) {
    let zero_start = virt_start + load_segment.file_size();
    let zero_end = virt_start + load_segment.mem_size();

    // Zeroing whole areas of memory is slow, so we use a trick to zero aligned pages

    // First, handle unaligned start
    let before_aligned = zero_start.as_u64() % M4KiB::SIZE;
    if before_aligned != 0 {
        let last_page = Page::<M4KiB>::containing_address(zero_start);

        let new_frame = {
            let (paddr, flags) = klu
                .page_table
                .translate(last_page)
                .expect("Last page of segment is not mapped to a 4KiB frame");

            // Use bit 9 to mark already kernel-space-mapped pages
            if flags.contains(Flags::BIT_9) {
                paddr
            } else {
                let new_frame = klu
                    .frame_allocator
                    .allocate_frame()
                    .expect("Failed to allocate frame");
                unsafe {
                    core::ptr::copy_nonoverlapping(
                        paddr.start_address().as_u64() as *const u8,
                        new_frame.start_address().as_u64() as *mut u8,
                        usize::try_from(M4KiB::SIZE).unwrap(),
                    );
                }

                unsafe { klu.page_table.unmap(last_page).unwrap().1.ignore_flush() };

                unsafe {
                    klu.page_table
                        .map(
                            last_page,
                            new_frame,
                            flags.union(Flags::BIT_9),
                            klu.frame_allocator,
                        )
                        .expect("Failed to map BSS page")
                        .ignore_flush();
                }

                new_frame
            }
        };

        unsafe {
            core::ptr::write_bytes(
                (new_frame.start_address().as_u64() as *mut u8)
                    .add(usize::try_from(before_aligned).unwrap()),
                0,
                usize::try_from(M4KiB::SIZE - before_aligned).unwrap(),
            );
        }
    }

    let mut segment_flags = Flags::PRESENT | Flags::GLOBAL;
    if load_segment.flags().is_write() {
        segment_flags = segment_flags.union(Flags::WRITABLE);
    }
    if !load_segment.flags().is_execute() {
        segment_flags = segment_flags.union(Flags::NO_EXECUTE);
    }

    let start_page = Page::<M4KiB>::containing_address(zero_start.aligned_up(M4KiB::ALIGNMENT));
    let end_page = Page::containing_address(zero_end - 1);

    // Then zero aligned pages
    for page in Page::range_inclusive(start_page, end_page) {
        let frame = klu
            .frame_allocator
            .allocate_frame()
            .expect("Failed to allocate frame");

        #[expect(clippy::cast_possible_truncation, reason = "Size is known")]
        {
            let frame_ptr = frame.start_address().as_u64()
                as *mut [u64; M4KiB::SIZE as usize / size_of::<u64>()];
            unsafe {
                frame_ptr.write([0_u64; M4KiB::SIZE as usize / size_of::<u64>()]);
            }
        }

        unsafe {
            klu.page_table
                .map(page, frame, segment_flags, klu.frame_allocator)
                .expect("Failed to map BSS")
                .ignore_flush();
        }
    }
}

fn handle_segment_dynamic(dynamic_segment: ProgramHeader, klu: &mut KernelLoadingUtils, vao: u64) {
    let xmas_elf::program::SegmentData::Dynamic64(data) =
        dynamic_segment.get_data(klu.kernel).unwrap()
    else {
        panic!("Failed to get dynamic segment data");
    };

    // Locate the RELA table
    let mut rela = None;
    let mut relasz = None;
    let mut relaent = None;
    for rel in data {
        match rel.get_tag().unwrap() {
            Tag::Rela => {
                let ptr = rel.get_ptr().unwrap();
                let prev = rela.replace(ptr);
                assert!(prev.is_none(), "Multiple RELA entries");
            }
            Tag::RelaSize => {
                let value = rel.get_val().unwrap();
                let prev = relasz.replace(value);
                assert!(prev.is_none(), "Multiple RELASZ entries");
            }
            Tag::RelaEnt => {
                let value = rel.get_val().unwrap();
                let prev = relaent.replace(value);
                assert!(prev.is_none(), "Multiple RELAENT entries");
            }
            _ => {}
        }
    }

    let Some(relocation_table_offset) = rela else {
        assert!(
            relasz.is_none() && relaent.is_none(),
            "Missing RELA entry but RELASIZE or RELAENT is present"
        );
        // No relocation needed, job is done
        return;
    };

    let total_size = relasz.expect("Missing RELASIZE entry");
    let entry_size = relaent.expect("Missing RELAENT entry");

    assert_eq!(entry_size, size_of::<Rela<u64>>() as u64);

    let num_entries = total_size / entry_size;
    for i in 0..num_entries {
        let rela = {
            let offset = relocation_table_offset + size_of::<Rela<u64>>() as u64 * i;
            let value = vao + offset;
            let addr = VirtAddr::try_new(value)
                .expect("Invalid address: outside of virtual address space");

            let mut buf = [0; size_of::<Rela<u64>>()];
            copy_from_krnlspc(klu, addr, &mut buf);

            unsafe { buf.as_ptr().cast::<Rela<u64>>().read_unaligned() }
        };

        assert_eq!(
            rela.get_symbol_table_index(),
            0,
            "Unexpected non-null symbol index"
        );
        assert_eq!(rela.get_type(), 8, "Unexpected relocation type");

        // Make sure segment is loaded
        assert!(
            klu.kernel.program_iter().any(|ph| {
                ph.get_type().unwrap() == Type::Load
                    && ph.virtual_addr() <= rela.get_offset()
                    && rela.get_offset() < ph.virtual_addr() + ph.mem_size()
            }),
            "Address is not loaded"
        );

        let addr = VirtAddr::new_extend(vao + rela.get_offset());
        let value = vao + rela.get_addend();

        copy_to_krnlspc(klu, addr, &value.to_ne_bytes());
    }
}

// Kernel space doesn't exactly map to the physical address space
// so the process of copying arrays of data is not trivial.
fn copy_from_krnlspc(klu: &KernelLoadingUtils, addr: VirtAddr, buf: &mut [u8]) {
    let end_addr = {
        let offset = u64::try_from(buf.len() - 1).unwrap();
        addr + offset
    };

    let start_page = Page::<M4KiB>::containing_address(addr);
    let end_page = Page::<M4KiB>::containing_address(end_addr);

    for page in Page::range_inclusive(start_page, end_page) {
        let (frame, _) = klu.page_table.translate(page).expect("Page is not mapped");
        let paddr = frame.start_address();

        // Find the address range to copy
        let page_start = page.start_address();
        let page_end = page.start_address() + M4KiB::SIZE - 1;

        // Special case of first and last pages
        let start_copy = page_start.max(addr);
        let end_copy = page_end.min(end_addr);

        let start_offset_in_frame = start_copy - page_start;

        let copy_len = usize::try_from(end_copy - start_copy + 1).unwrap();

        let start_paddr = paddr + start_offset_in_frame;

        let start_offset_buffer = {
            let mut steps = start_copy.as_u64().checked_sub(addr.as_u64()).unwrap();
            steps &= 0xffff_ffff_ffff;
            usize::try_from(steps).unwrap()
        };

        let src =
            unsafe { core::slice::from_raw_parts(start_paddr.as_u64() as *const u8, copy_len) };

        let dest = &mut buf[start_offset_buffer..][..copy_len];

        dest.copy_from_slice(src);
    }
}

// Kernel space doesn't exactly map to the physical address space
// so the process of copying arrays of data is not trivial.
fn copy_to_krnlspc(klu: &mut KernelLoadingUtils, addr: VirtAddr, buf: &[u8]) {
    let end_addr = {
        let offset = u64::try_from(buf.len() - 1).unwrap();
        addr + offset
    };

    let start_page = Page::<M4KiB>::containing_address(addr);
    let end_page = Page::<M4KiB>::containing_address(end_addr);

    for page in Page::range_inclusive(start_page, end_page) {
        let phys_addr = {
            let (frame, flags) = klu
                .page_table
                .translate(page)
                .expect("Last page of segment is not mapped to a 4KiB frame");

            if flags.contains(Flags::BIT_9) {
                frame
            } else {
                let new_frame = klu
                    .frame_allocator
                    .allocate_frame()
                    .expect("Failed to allocate frame");

                unsafe {
                    core::ptr::copy_nonoverlapping(
                        frame.start_address().as_u64() as *const u8,
                        new_frame.start_address().as_u64() as *mut u8,
                        usize::try_from(M4KiB::SIZE).unwrap(),
                    );
                }

                unsafe { klu.page_table.unmap(page).unwrap().1.ignore_flush() };

                unsafe {
                    klu.page_table
                        .map(
                            page,
                            new_frame,
                            flags.union(Flags::BIT_9),
                            klu.frame_allocator,
                        )
                        .expect("Failed to copy memory to kernel space")
                        .ignore_flush();
                }

                new_frame
            }
        };

        // Find the address range to copy
        let page_start = page.start_address();
        let page_end = page.start_address() + M4KiB::SIZE - 1;

        // Special case of first and last pages
        let start_copy = page_start.max(addr);
        let end_copy = page_end.min(end_addr);

        let start_offset_in_frame = start_copy - page_start;

        let copy_len = usize::try_from(end_copy - start_copy + 1).unwrap();

        let start_paddr = phys_addr.start_address() + start_offset_in_frame;

        let start_offset_buffer = {
            let mut steps = start_copy.as_u64().checked_sub(addr.as_u64()).unwrap();
            steps &= 0xffff_ffff_ffff;
            usize::try_from(steps).unwrap()
        };

        let dest =
            unsafe { core::slice::from_raw_parts_mut(start_paddr.as_u64() as *mut u8, copy_len) };

        let src = &buf[start_offset_buffer..][..copy_len];

        dest.copy_from_slice(src);
    }
}

fn handle_segment_gnurelro(
    gnurelro_segment: ProgramHeader,
    klu: &mut KernelLoadingUtils,
    vao: u64,
) {
    let start = VirtAddr::new_extend(vao + gnurelro_segment.virtual_addr());
    let start_page = Page::<M4KiB>::containing_address(start);
    let end_page = Page::<M4KiB>::containing_address(start + gnurelro_segment.mem_size() - 1);

    for page in Page::range_inclusive(start_page, end_page) {
        let (_, flags) = klu
            .page_table
            .translate(page)
            .expect("Last page of segment is not mapped");

        if flags.contains(Flags::WRITABLE) {
            let new_flags = flags.without(Flags::WRITABLE);
            unsafe {
                klu.page_table
                    .update_flags(page, new_flags)
                    .unwrap()
                    .ignore_flush();
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct LoadedKernelInfo {
    pub entry_point: VirtAddr,
    pub image_offset: VirtAddr,
    pub kernel_size: u64,
}
