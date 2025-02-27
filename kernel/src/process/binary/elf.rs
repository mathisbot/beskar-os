use xmas_elf::{
    ElfFile, header,
    program::{self, Type},
};

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
    for ph in elf.program_iter() {
        match faillible!(ph.get_type()) {
            Type::Load => {
                handle_segment_load(ph, elf)?;
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
            handle_segment_dynamic(ph, elf)?;
        }
    }

    // Relocate memory addresses
    for ph in elf.program_iter() {
        if faillible!(ph.get_type()) == Type::GnuRelro {
            handle_segment_gnurelro(ph, elf)?;
        }
    }

    Ok(())
}

fn handle_segment_load(ph: program::ProgramHeader, elf: &ElfFile) -> BinaryResult<()> {
    todo!()
}

fn handle_segment_dynamic(ph: program::ProgramHeader, elf: &ElfFile) -> BinaryResult<()> {
    todo!()
}

fn handle_segment_gnurelro(ph: program::ProgramHeader, elf: &ElfFile) -> BinaryResult<()> {
    todo!()
}
