use super::{BinaryResult, LoadedBinary, TlsTemplate};
use crate::{mem::frame_alloc, process};
use beskar_core::arch::{
    commons::{
        VirtAddr,
        paging::{CacheFlush, Flags, FrameAllocator, M4KiB, Mapper, MemSize as _, Page},
    },
    x86_64::{paging::page_table::PageTable, userspace::Ring},
};
use beskar_core::process::binary::LoadError;
use xmas_elf::{
    ElfFile, P64,
    dynamic::Tag,
    header,
    program::{self, SegmentData, Type},
    sections::Rela,
};

macro_rules! faillible {
    ($expr:expr) => {
        $expr.map_err(|_| LoadError::InvalidBinary)?
    };
}

macro_rules! ensure {
    ($expr:expr) => {
        if !($expr) {
            return Err(LoadError::InvalidBinary);
        }
    };
}

/// Load an ELF binary into memory.
///
/// The input data is left untouched and will be copied properly into memory
/// so that it can safely be unmapped after calling this function.
pub fn load(input: &[u8]) -> BinaryResult<LoadedBinary> {
    let elf = faillible!(ElfFile::new(input));
    sanity_check(&elf)?;

    let total_size = elf
        .program_iter()
        .map(|ph| ph.virtual_addr() + ph.mem_size())
        .max()
        .unwrap_or(0);

    let page_count = total_size.div_ceil(M4KiB::SIZE);
    let page_range = process::current()
        .address_space()
        .with_pgalloc(|palloc| palloc.allocate_pages::<M4KiB>(page_count))
        .unwrap();
    let offset = page_range.start().start_address();
    // During the loading process, we will have to modify the page flags.
    // The pages we'll be writing to may not have the WRITABLE flag set anymore.
    // To overcome this problem, we will map a set of "working" pages, which will have the
    // correct flags set. These pages will be unmapped after the loading process.
    let working_page_range = process::current()
        .address_space()
        .with_pgalloc(|palloc| palloc.allocate_pages::<M4KiB>(page_count))
        .unwrap();
    let working_offset = working_page_range.start().start_address();

    frame_alloc::with_frame_allocator(|fralloc| {
        let mut dummy_flags = Flags::PRESENT | Flags::NO_EXECUTE | Flags::WRITABLE;
        if process::current().kind().ring() == Ring::User {
            dummy_flags = dummy_flags | Flags::USER_ACCESSIBLE;
        }
        process::current().address_space().with_page_table(|pt| {
            for (page, wp) in page_range.into_iter().zip(working_page_range) {
                let frame = fralloc.allocate_frame().unwrap();
                pt.map(page, frame, dummy_flags, fralloc).flush();
                pt.map(wp, frame, Flags::PRESENT | Flags::WRITABLE, fralloc)
                    .flush();
            }
        });
    });

    #[cfg(debug_assertions)]
    unsafe {
        working_offset.as_mut_ptr::<u8>().write_bytes(
            beskar_core::arch::x86_64::instructions::STACK_DEBUG_INSTR,
            usize::try_from(working_page_range.size()).unwrap(),
        );
    }

    let load_res = load_segments(&elf, offset, working_offset);

    // Unmap and free "working" pages
    process::current().address_space().with_page_table(|pt| {
        for page in working_page_range {
            // Note that we cannot free the frames as they are used by the binary!
            pt.unmap(page).unwrap().1.flush();
        }
    });
    process::current().address_space().with_pgalloc(|palloc| {
        palloc.free_pages(working_page_range);
    });

    if let Err(e) = load_res {
        frame_alloc::with_frame_allocator(|fralloc| {
            process::current().address_space().with_page_table(|pt| {
                for page in page_range {
                    let (frame, tlb) = pt.unmap(page).unwrap();
                    tlb.flush();
                    fralloc.free(frame);
                }
            });
        });
        process::current().address_space().with_pgalloc(|palloc| {
            palloc.free_pages(page_range);
        });
        return Err(e);
    }

    let entry_point = {
        let raw_entry_point = elf.header.pt2.entry_point().try_into().unwrap();
        let entry_point = unsafe { offset.as_ptr::<()>().byte_add(raw_entry_point) };
        unsafe { core::mem::transmute::<*const (), extern "C" fn()>(entry_point) }
    };

    Ok(LoadedBinary {
        entry_point,
        tls_template: load_res.unwrap().tls_template,
    })
}

#[inline]
fn sanity_check(elf: &ElfFile) -> BinaryResult<()> {
    faillible!(header::sanity_check(elf));
    for ph in elf.program_iter() {
        faillible!(program::sanity_check(ph, elf));
    }

    ensure!(elf.header.pt1.class() == header::Class::SixtyFour);
    #[cfg(target_arch = "x86_64")]
    ensure!(elf.header.pt2.machine().as_machine() == header::Machine::X86_64);
    #[cfg(target_arch = "aarch64")]
    ensure!(elf.header.pt2.machine().as_machine() == header::Machine::AArch64);
    ensure!(elf.header.pt1.data() == header::Data::LittleEndian);
    ensure!(
        elf.header.pt1.os_abi() == header::OsAbi::SystemV
            || elf.header.pt1.os_abi() == header::OsAbi::Linux
    );
    ensure!(elf.header.pt2.entry_point() != 0);

    Ok(())
}

struct LoadedSegmentsInfo {
    tls_template: Option<TlsTemplate>,
}

fn load_segments(
    elf: &ElfFile,
    offset: VirtAddr,
    working_offset: VirtAddr,
) -> BinaryResult<LoadedSegmentsInfo> {
    let mut tls_template = None;

    for ph in elf.program_iter() {
        match faillible!(ph.get_type()) {
            Type::Load => {
                handle_segment_load(elf, ph, offset, working_offset)?;
            }
            Type::Tls => {
                if tls_template.is_some() {
                    return Err(LoadError::InvalidBinary);
                }
                tls_template = Some(TlsTemplate {
                    start: offset + ph.virtual_addr(),
                    file_size: ph.file_size(),
                    mem_size: ph.mem_size(),
                });
            }
            Type::Interp => {
                return Err(LoadError::InvalidBinary);
            }
            _ => {}
        }
    }

    // Relocate memory addresses
    for ph in elf.program_iter() {
        if faillible!(ph.get_type()) == Type::Dynamic {
            handle_segment_dynamic(elf, ph, offset, working_offset)?;
        }
    }

    // Handle GNU_RELRO segments
    for ph in elf.program_iter() {
        if faillible!(ph.get_type()) == Type::GnuRelro {
            handle_segment_gnurelro(ph, offset)?;
        }
    }

    Ok(LoadedSegmentsInfo { tls_template })
}

fn handle_segment_load(
    elf: &ElfFile,
    ph: program::ProgramHeader,
    offset: VirtAddr,
    working_offset: VirtAddr,
) -> BinaryResult<()> {
    let mut segment_flags = Flags::PRESENT;
    if ph.flags().is_write() {
        segment_flags = segment_flags | Flags::WRITABLE;
    }
    if !ph.flags().is_execute() {
        segment_flags = segment_flags | Flags::NO_EXECUTE;
    }
    if process::current().kind().ring() == Ring::User {
        segment_flags = segment_flags | Flags::USER_ACCESSIBLE;
    }

    process::current().address_space().with_page_table(|pt| {
        let segment_start_vaddr = offset + ph.virtual_addr();

        if ph.file_size() != 0 {
            let segment_start_page = Page::<M4KiB>::containing_address(segment_start_vaddr);
            let segment_end_page =
                Page::<M4KiB>::containing_address(segment_start_vaddr + ph.file_size() - 1);

            for page in Page::range_inclusive(segment_start_page, segment_end_page) {
                pt.update_flags(page, segment_flags).unwrap().flush();
            }

            // Copy the segment data from elf.input to the new location
            let dest = (working_offset + ph.virtual_addr()).as_mut_ptr::<u8>();
            let src = elf.input[usize::try_from(ph.offset()).unwrap()..].as_ptr();
            unsafe {
                dest.copy_from_nonoverlapping(src, usize::try_from(ph.file_size()).unwrap());
            }
        }

        if ph.mem_size() > ph.file_size() {
            zero_bss(ph, pt, offset, working_offset);
        }

        Ok(())
    })
}

fn zero_bss(
    ph: program::ProgramHeader,
    pt: &mut PageTable<'_>,
    offset: VirtAddr,
    working_offset: VirtAddr,
) {
    let zero_start = offset + ph.virtual_addr() + ph.file_size();
    let zero_end = offset + ph.virtual_addr() + ph.mem_size() - 1;

    let working_zero_start = working_offset + ph.virtual_addr() + ph.file_size();

    let unaligned = zero_start.as_u64() % M4KiB::SIZE;
    if unaligned != 0 {
        let len =
            usize::try_from((M4KiB::SIZE - unaligned).min(ph.mem_size() - ph.file_size())).unwrap();
        for i in 0..len {
            unsafe {
                working_zero_start
                    .as_mut_ptr::<u8>()
                    .byte_add(i)
                    .write_volatile(0);
            }
        }
    }

    let zero_start_page =
        Page::<M4KiB>::from_start_address(zero_start.align_up(M4KiB::SIZE)).unwrap();
    let zero_end_page = Page::<M4KiB>::containing_address(zero_end);

    let mut segment_flags = Flags::PRESENT;
    if ph.flags().is_write() {
        segment_flags = segment_flags | Flags::WRITABLE;
    }
    if !ph.flags().is_execute() {
        segment_flags = segment_flags | Flags::NO_EXECUTE;
    }
    if process::current().kind().ring() == Ring::User {
        segment_flags = segment_flags | Flags::USER_ACCESSIBLE;
    }

    for page in Page::range_inclusive(zero_start_page, zero_end_page) {
        // FIXME: Free these pages on binary unload
        crate::mem::frame_alloc::with_frame_allocator(|fralloc| {
            let frame = fralloc.allocate_frame().unwrap();
            // We need to zero the frame, so start by setting the page as writable
            pt.map(page, frame, Flags::PRESENT | Flags::WRITABLE, fralloc)
                .flush();
        });
        unsafe {
            page.start_address().as_mut_ptr::<usize>().write_bytes(
                0,
                usize::try_from(M4KiB::SIZE).unwrap() / size_of::<usize>(),
            );
        }
        // Finally, set the page with the correct flags (potentially without WRITABLE)
        pt.update_flags(page, segment_flags).unwrap().flush();
    }
}

fn handle_segment_dynamic(
    elf: &ElfFile,
    ph: program::ProgramHeader,
    offset: VirtAddr,
    working_offset: VirtAddr,
) -> BinaryResult<()> {
    let SegmentData::Dynamic64(data) = faillible!(ph.get_data(elf)) else {
        return Err(LoadError::InvalidBinary);
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
                ensure!(prev.is_none());
            }
            Tag::RelaSize => {
                let value = rel.get_val().unwrap();
                let prev = relasz.replace(value);
                ensure!(prev.is_none());
            }
            Tag::RelaEnt => {
                let value = rel.get_val().unwrap();
                let prev = relaent.replace(value);
                ensure!(prev.is_none());
            }
            _ => {}
        }
    }

    let Some(relocation_table_offset) = rela else {
        ensure!(relasz.is_none() && relaent.is_none());
        // No relocation needed, job is done
        return Ok(());
    };

    ensure!(relasz.is_some() && relaent.is_some());
    let relasz = relasz.unwrap();
    let relaent = relaent.unwrap();
    ensure!(relaent == u64::try_from(size_of::<Rela<P64>>()).unwrap());

    let num_entries = relasz / relaent;
    ensure!(num_entries * relaent == relasz);

    for i in 0..num_entries {
        let rela = {
            let offset = relocation_table_offset + i * relaent;
            unsafe {
                elf.input
                    .as_ptr()
                    .byte_add(usize::try_from(offset).unwrap())
                    .cast::<Rela<P64>>()
                    .read_unaligned()
            }
        };

        ensure!(rela.get_symbol_table_index() == 0);
        ensure!(rela.get_type() == 8);

        ensure!(elf.program_iter().any(|ph| {
            ph.get_type().unwrap() == Type::Load
                && ph.virtual_addr() <= rela.get_offset()
                && rela.get_offset() < ph.virtual_addr() + ph.mem_size()
        }));

        let addr = working_offset + rela.get_offset();
        let value = offset + rela.get_addend();

        unsafe { addr.as_mut_ptr::<u64>().write_unaligned(value.as_u64()) };
    }

    Ok(())
}

fn handle_segment_gnurelro(ph: program::ProgramHeader, offset: VirtAddr) -> BinaryResult<()> {
    process::current().address_space().with_page_table(|pt| {
        let start_vaddr = offset + ph.virtual_addr();

        let start_page = Page::<M4KiB>::containing_address(start_vaddr);
        let end_page = Page::<M4KiB>::containing_address(start_vaddr + ph.mem_size() - 1);

        for page in Page::range_inclusive(start_page, end_page) {
            let (_frame, flags) = pt.translate(page).unwrap();

            if flags.contains(Flags::WRITABLE) {
                pt.update_flags(page, flags.without(Flags::WRITABLE))
                    .unwrap()
                    .flush();
            }
        }

        Ok(())
    })
}
