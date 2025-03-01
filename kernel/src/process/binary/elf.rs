use beskar_core::arch::{
    commons::{
        VirtAddr,
        paging::{CacheFlush, Flags, FrameAllocator, M4KiB, Mapper, MemSize as _, Page},
    },
    x86_64::paging::page_table::PageTable,
};
use xmas_elf::{
    ElfFile, header,
    program::{self, Type},
};

use crate::process::scheduler;

use super::{BinaryResult, LoadError, LoadedBinary};

macro_rules! faillible {
    ($expr:expr) => {
        $expr.map_err(|_| LoadError::InvalidBinary)?
    };
}

pub fn load(input: &[u8]) -> BinaryResult<LoadedBinary> {
    let elf = faillible!(ElfFile::new(input));
    sanity_check(&elf)?;

    load_segments(&elf)?;

    let entry_point = {
        let raw_entry_point = elf.header.pt2.entry_point();
        // Get the right memory layout
        let entry_point = raw_entry_point as *const ();
        unsafe { core::mem::transmute(entry_point) }
    };

    Ok(LoadedBinary { entry_point })
}

#[inline]
fn sanity_check(elf: &ElfFile) -> BinaryResult<()> {
    faillible!(header::sanity_check(elf));
    for ph in elf.program_iter() {
        faillible!(program::sanity_check(ph, elf));
    }

    if elf.header.pt1.class() != header::Class::SixtyFour {
        return Err(LoadError::InvalidBinary);
    }
    if elf.header.pt2.machine().as_machine() != header::Machine::X86_64 {
        return Err(LoadError::InvalidBinary);
    }

    Ok(())
}

fn load_segments(elf: &ElfFile) -> BinaryResult<()> {
    let binary_offset = VirtAddr::new(elf.input.as_ptr() as u64);

    for ph in elf.program_iter() {
        match faillible!(ph.get_type()) {
            Type::Load => {
                handle_segment_load(ph, binary_offset)?;
            }
            Type::Tls => {
                crate::warn!("TLS segment found, but not supported");
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
            handle_segment_dynamic(ph, binary_offset)?;
        }
    }

    // Relocate memory addresses
    for ph in elf.program_iter() {
        if faillible!(ph.get_type()) == Type::GnuRelro {
            handle_segment_gnurelro(ph, binary_offset)?;
        }
    }

    Ok(())
}

fn handle_segment_load(ph: program::ProgramHeader, offset: VirtAddr) -> BinaryResult<()> {
    let mut pt = scheduler::current_process()
        .address_space()
        .get_recursive_pt();

    let segment_start_vaddr = offset + ph.virtual_addr();

    if ph.file_size() != 0 {
        let segment_start_page = Page::<M4KiB>::containing_address(segment_start_vaddr);
        let segment_end_page =
            Page::<M4KiB>::containing_address(segment_start_vaddr + ph.file_size() - 1);

        // TODO: User Accessible if userspace
        let mut segment_flags = Flags::PRESENT;
        if ph.flags().is_write() {
            segment_flags = segment_flags | Flags::WRITABLE;
        }
        if !ph.flags().is_execute() {
            segment_flags = segment_flags | Flags::NO_EXECUTE;
        }

        for page in Page::range_inclusive(segment_start_page, segment_end_page) {
            pt.update_flags(page, segment_flags)
                .expect("Failed to update flags")
                .flush();
        }
    }

    if ph.mem_size() > ph.file_size() {
        zero_bss(segment_start_vaddr, ph, &mut pt).unwrap();
    }

    Ok(())
}

fn zero_bss<'a>(
    vaddr: VirtAddr,
    ph: program::ProgramHeader,
    pt: &mut PageTable<'a>,
) -> BinaryResult<()> {
    let zero_start = vaddr + ph.file_size();
    let zero_end = vaddr + ph.mem_size() - 1;

    let unaligned = zero_start.as_u64() % M4KiB::SIZE;
    if unaligned != 0 {
        todo!("Unaligned BSS");
    }

    let zero_start_page =
        Page::<M4KiB>::from_start_address(zero_start.align_up(M4KiB::SIZE)).unwrap();
    let zero_end_page = Page::<M4KiB>::containing_address(zero_end);

    // TODO: User Accessible if userspace
    let mut segment_flags = Flags::PRESENT;
    if ph.flags().is_write() {
        segment_flags = segment_flags | Flags::WRITABLE;
    }
    if !ph.flags().is_execute() {
        segment_flags = segment_flags | Flags::NO_EXECUTE;
    }

    for page in Page::range_inclusive(zero_start_page, zero_end_page) {
        // FIXME: Free these pages on binary unload
        crate::mem::frame_alloc::with_frame_allocator(|fralloc| {
            let frame = fralloc.allocate_frame().unwrap();
            pt.map(page, frame, segment_flags, fralloc).flush();
        });
        unsafe {
            page.start_address()
                .as_mut_ptr::<usize>()
                .write_bytes(0, M4KiB::SIZE as usize / size_of::<usize>())
        };
    }

    Ok(())
}

fn handle_segment_dynamic(ph: program::ProgramHeader, offset: VirtAddr) -> BinaryResult<()> {
    todo!("Dynamic segment")
}

fn handle_segment_gnurelro(ph: program::ProgramHeader, offset: VirtAddr) -> BinaryResult<()> {
    let mut pt = scheduler::current_process()
        .address_space()
        .get_recursive_pt();

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
}
