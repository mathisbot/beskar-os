//! Generic ELF binary loader.

use crate::{
    Result,
    error::ElfLoadError,
    mapper::{MappedRegion, MemoryMapper, PageFlags},
    segments::{LoadedBinary, TlsTemplate},
};
use beskar_core::{arch::VirtAddr, mem::ranges::MemoryRange};
use xmas_elf::{
    ElfFile, P64,
    dynamic::Tag,
    header,
    program::{self, SegmentData, Type},
    sections::Rela,
};

#[cfg(target_arch = "x86_64")]
const R_RELATIVE: u32 = 8;
#[cfg(target_arch = "aarch64")]
const R_RELATIVE: u32 = 1027;

const PAGE_SIZE: u64 = 4096;

const MAX_LOAD_SEGMENTS: usize = 16;

/// Generic ELF binary loader with pluggable memory mapper.
pub struct ElfLoader;

impl ElfLoader {
    /// Load an ELF binary using the provided memory mapper.
    ///
    /// # Errors
    ///
    /// Returns any errors encountered during loading.
    pub fn load<M: MemoryMapper>(input: &[u8], mapper: &mut M) -> Result<LoadedBinary> {
        let elf = ElfFile::new(input).map_err(|_| ElfLoadError::InvalidBinary)?;

        // Validate ELF format
        Self::sanity_check(&elf)?;

        // Calculate address range for all allocatable segments
        let addr_range = Self::calculate_address_range(&elf)?;

        // Map binary into memory
        let region = mapper
            .map_region(addr_range.size(), PageFlags::rw())
            .map_err(|()| ElfLoadError::MapperError)?;

        // Load segments and collect TLS template
        let tls_template = match Self::load_segments(
            &elf,
            region.virt_addr,
            VirtAddr::new_extend(addr_range.start()),
            mapper,
        ) {
            Ok(template) => template,
            Err(e) => {
                mapper.unmap_region(region).ok();
                mapper.rollback();
                return Err(e);
            }
        };

        // Get entry point
        let entry_point = {
            let entry_vaddr = elf.header.pt2.entry_point();
            let runtime_addr = region.virt_addr + (entry_vaddr - addr_range.start());
            let entry_ptr = runtime_addr.as_ptr();
            unsafe { core::mem::transmute::<*const (), extern "C" fn()>(entry_ptr) }
        };

        Ok(LoadedBinary {
            entry_point,
            tls_template,
            image_size: addr_range.size(),
        })
    }

    /// Sanity check the ELF file format.
    fn sanity_check(elf: &ElfFile) -> Result<()> {
        header::sanity_check(elf).map_err(|_| ElfLoadError::InvalidBinary)?;

        if elf
            .program_iter()
            .any(|ph| program::sanity_check(ph, elf).is_err())
        {
            return Err(ElfLoadError::InvalidBinary);
        }

        if elf.header.pt1.class() != header::Class::SixtyFour {
            return Err(ElfLoadError::InvalidBinary);
        }

        if (cfg!(target_arch = "x86_64")
            && elf.header.pt2.machine().as_machine() != header::Machine::X86_64)
            || (cfg!(target_arch = "aarch64")
                && elf.header.pt2.machine().as_machine() != header::Machine::AArch64)
        {
            return Err(ElfLoadError::InvalidBinary);
        }

        if elf.header.pt1.data() != header::Data::LittleEndian {
            return Err(ElfLoadError::InvalidBinary);
        }

        let os_abi = elf.header.pt1.os_abi();
        if !matches!(os_abi, header::OsAbi::SystemV | header::OsAbi::Linux) {
            return Err(ElfLoadError::InvalidBinary);
        }

        let elf_type = elf.header.pt2.type_().as_type();
        if !matches!(
            elf_type,
            header::Type::Executable | header::Type::SharedObject
        ) {
            return Err(ElfLoadError::InvalidBinary);
        }

        Ok(())
    }

    /// Calculate the virtual address range for all allocatable segments.
    fn calculate_address_range(elf: &ElfFile) -> Result<MemoryRange> {
        let mut min_vaddr = u64::MAX;
        let mut max_vaddr = 0u64;

        // Consider LOAD and TLS segments
        for ph in elf.program_iter().filter(|ph| {
            ph.get_type()
                .is_ok_and(|pt| matches!(pt, Type::Load | Type::Tls))
        }) {
            let seg_start = ph.virtual_addr();
            let seg_end = seg_start
                .checked_add(ph.mem_size())
                .ok_or(ElfLoadError::Overflow)?;

            let seg_start_aligned = align_down(seg_start, PAGE_SIZE);
            let seg_end_aligned = align_up(seg_end, PAGE_SIZE)? - 1;

            min_vaddr = min_vaddr.min(seg_start_aligned);
            max_vaddr = max_vaddr.max(seg_end_aligned);
        }

        if min_vaddr == u64::MAX {
            return Err(ElfLoadError::InvalidBinary);
        }

        Ok(MemoryRange::new(min_vaddr, max_vaddr))
    }

    /// Load all segments into the mapped region.
    fn load_segments<M: MemoryMapper>(
        elf: &ElfFile,
        region_addr: VirtAddr,
        min_vaddr: VirtAddr,
        mapper: &mut M,
    ) -> Result<Option<TlsTemplate>> {
        let mut tls_template = None;

        for ph in elf.program_iter() {
            let pt = ph.get_type().map_err(|_| ElfLoadError::InvalidBinary)?;

            match pt {
                Type::Load => {
                    Self::load_segment(elf, ph, region_addr, min_vaddr, mapper)?;
                }
                Type::Tls => {
                    Self::load_tls_segment(ph, elf.input, region_addr, min_vaddr, mapper)?;
                    tls_template = Some(TlsTemplate {
                        start: region_addr + (ph.virtual_addr() - min_vaddr.as_u64()),
                        file_size: ph.file_size(),
                        mem_size: ph.mem_size(),
                    });
                }
                Type::Dynamic => {
                    Self::process_relocations(elf, ph, region_addr, min_vaddr, mapper)?;
                }
                Type::GnuRelro => {
                    Self::process_gnu_relro(ph, region_addr, min_vaddr, mapper)?;
                }
                Type::Interp => {
                    return Err(ElfLoadError::UnsupportedFeature);
                }
                _ => {}
            }
        }

        Ok(tls_template)
    }

    /// Load a LOAD segment.
    fn load_segment<M: MemoryMapper>(
        elf: &ElfFile,
        ph: xmas_elf::program::ProgramHeader,
        region_addr: VirtAddr,
        min_vaddr: VirtAddr,
        mapper: &mut M,
    ) -> Result<()> {
        let segment_vaddr = ph.virtual_addr();
        let file_size = ph.file_size();
        let mem_size = ph.mem_size();
        if mem_size < file_size {
            return Err(ElfLoadError::InvalidSegment);
        }

        let dest_offset = segment_vaddr
            .checked_sub(min_vaddr.as_u64())
            .ok_or(ElfLoadError::Overflow)?;
        let dest_addr = region_addr
            .as_u64()
            .checked_add(dest_offset)
            .ok_or(ElfLoadError::Overflow)?;

        // Copy file data
        if file_size > 0 {
            let offset = usize::try_from(ph.offset()).map_err(|_| ElfLoadError::InvalidSegment)?;
            let file_data = elf
                .input
                .get(
                    offset
                        ..offset
                            + usize::try_from(file_size)
                                .map_err(|_| ElfLoadError::InvalidSegment)?,
                )
                .ok_or(ElfLoadError::InvalidBinary)?;

            mapper
                .copy_data(VirtAddr::new_extend(dest_addr), file_data)
                .map_err(|()| ElfLoadError::MapperError)?;
        }

        // Zero BSS section
        if mem_size > file_size {
            let bss_size = mem_size - file_size;
            let bss_addr = dest_addr
                .checked_add(file_size)
                .ok_or(ElfLoadError::Overflow)?;
            mapper
                .zero_region(VirtAddr::new_extend(bss_addr), bss_size)
                .map_err(|()| ElfLoadError::MapperError)?;
        }

        // Set appropriate page flags
        let flags = Self::compute_segment_flags(&ph);
        let region = MappedRegion {
            virt_addr: VirtAddr::new_extend(dest_addr),
            size: mem_size,
        };
        mapper
            .update_flags(region, flags)
            .map_err(|()| ElfLoadError::MapperError)?;

        Ok(())
    }

    /// Load a TLS segment.
    fn load_tls_segment<M: MemoryMapper>(
        ph: xmas_elf::program::ProgramHeader,
        input: &[u8],
        region_addr: VirtAddr,
        min_vaddr: VirtAddr,
        mapper: &mut M,
    ) -> Result<()> {
        let file_size = ph.file_size();
        if file_size == 0 {
            return Ok(());
        }

        let mem_size = ph.mem_size();
        if mem_size < file_size {
            return Err(ElfLoadError::InvalidSegment);
        }

        let offset = usize::try_from(ph.offset()).map_err(|_| ElfLoadError::InvalidSegment)?;
        let file_data = input
            .get(
                offset
                    ..offset
                        + usize::try_from(file_size).map_err(|_| ElfLoadError::InvalidSegment)?,
            )
            .ok_or(ElfLoadError::InvalidBinary)?;

        let dest_offset = ph
            .virtual_addr()
            .checked_sub(min_vaddr.as_u64())
            .ok_or(ElfLoadError::Overflow)?;
        let dest_addr = region_addr
            .as_u64()
            .checked_add(dest_offset)
            .ok_or(ElfLoadError::Overflow)?;
        mapper
            .copy_data(VirtAddr::new_extend(dest_addr), file_data)
            .map_err(|()| ElfLoadError::MapperError)?;

        // Zero TLS BSS
        if mem_size > file_size {
            let bss_addr = dest_addr
                .checked_add(file_size)
                .ok_or(ElfLoadError::Overflow)?;
            let bss_size = mem_size - file_size;
            mapper
                .zero_region(VirtAddr::new_extend(bss_addr), bss_size)
                .map_err(|()| ElfLoadError::MapperError)?;
        }

        Ok(())
    }

    /// Compute page flags from segment header flags.
    fn compute_segment_flags(ph: &xmas_elf::program::ProgramHeader) -> PageFlags {
        let write = ph.flags().is_write();
        let exec = ph.flags().is_execute();

        match (write, exec) {
            (true, true) => PageFlags::rwx(),
            (true, false) => PageFlags::rw(),
            (false, true) => PageFlags::rx(),
            (false, false) => PageFlags::r(),
        }
    }

    /// Process RELA relocations from the dynamic segment.
    fn process_relocations<M: MemoryMapper>(
        elf: &ElfFile,
        ph: xmas_elf::program::ProgramHeader,
        region_addr: VirtAddr,
        min_vaddr: VirtAddr,
        _mapper: &mut M,
    ) -> Result<()> {
        let SegmentData::Dynamic64(data) =
            ph.get_data(elf).map_err(|_| ElfLoadError::InvalidBinary)?
        else {
            return Err(ElfLoadError::InvalidBinary);
        };

        // Find relocation table metadata
        let mut rela_vaddr = None;
        let mut rela_size = None;
        let mut rela_ent = None;

        for entry in data {
            match entry.get_tag().map_err(|_| ElfLoadError::InvalidBinary)? {
                Tag::Rela => {
                    rela_vaddr = Some(entry.get_ptr().map_err(|_| ElfLoadError::InvalidBinary)?);
                }
                Tag::RelaSize => {
                    rela_size = Some(entry.get_val().map_err(|_| ElfLoadError::InvalidBinary)?);
                }
                Tag::RelaEnt => {
                    rela_ent = Some(entry.get_val().map_err(|_| ElfLoadError::InvalidBinary)?);
                }
                _ => {}
            }
        }

        let Some(reloc_vaddr) = rela_vaddr else {
            // No relocations needed
            return Ok(());
        };

        let reloc_size = rela_size.ok_or(ElfLoadError::InvalidBinary)?;
        let reloc_ent = rela_ent.ok_or(ElfLoadError::InvalidBinary)?;

        // Validate relocation entry size
        if reloc_ent != size_of::<Rela<P64>>() as u64 {
            return Err(ElfLoadError::RelocationError);
        }

        // Find relocation data in input
        let reloc_file_offset =
            Self::vaddr_to_file_offset(elf, reloc_vaddr).ok_or(ElfLoadError::InvalidBinary)?;

        let num_entries = reloc_size / reloc_ent;
        let reloc_entry_end = reloc_file_offset
            .checked_add(reloc_size)
            .ok_or(ElfLoadError::Overflow)?;
        if usize::try_from(reloc_entry_end).unwrap() > elf.input.len() {
            return Err(ElfLoadError::InvalidBinary);
        }

        // Pre-collect LOAD segment ranges for O(1) validation
        let mut load_segments = [None; MAX_LOAD_SEGMENTS];
        let mut load_count = 0;

        for seg_ph in elf.program_iter() {
            if seg_ph.get_type() == Ok(Type::Load) {
                if load_count >= MAX_LOAD_SEGMENTS {
                    return Err(ElfLoadError::InvalidBinary);
                }
                let seg_start = seg_ph.virtual_addr();
                let seg_end = seg_start
                    .checked_add(seg_ph.mem_size())
                    .ok_or(ElfLoadError::Overflow)?;
                load_segments[load_count] = Some((seg_start, seg_end));
                load_count += 1;
            }
        }

        // Process each relocation
        for i in 0..num_entries {
            let reloc_offset = reloc_file_offset + i * reloc_ent;
            let reloc_offset_usize =
                usize::try_from(reloc_offset).map_err(|_| ElfLoadError::InvalidBinary)?;

            let rela = unsafe {
                elf.input
                    .as_ptr()
                    .byte_add(reloc_offset_usize)
                    .cast::<Rela<P64>>()
                    .read_unaligned()
            };

            // Only handle RELATIVE relocations
            if rela.get_symbol_table_index() != 0 || rela.get_type() != R_RELATIVE {
                continue;
            }

            // Validate target address is in a LOAD segment
            let rela_offset_vaddr = rela.get_offset();
            let in_load_segment = load_segments[..load_count].iter().any(|seg| {
                if let Some((start, end)) = seg {
                    rela_offset_vaddr >= *start && rela_offset_vaddr < *end
                } else {
                    false
                }
            });

            if !in_load_segment {
                return Err(ElfLoadError::RelocationError);
            }

            // Apply relocation
            let target_delta = rela_offset_vaddr
                .checked_sub(min_vaddr.as_u64())
                .ok_or(ElfLoadError::Overflow)?;
            let target_addr = region_addr
                .as_u64()
                .checked_add(target_delta)
                .ok_or(ElfLoadError::Overflow)?;
            let relocated_value = region_addr
                .as_u64()
                .checked_add(rela.get_addend())
                .ok_or(ElfLoadError::Overflow)?;

            unsafe {
                (target_addr as *mut u64).write_unaligned(relocated_value);
            }
        }

        Ok(())
    }

    /// Process `GNU_RELRO` segments.
    fn process_gnu_relro<M: MemoryMapper>(
        ph: xmas_elf::program::ProgramHeader,
        region_addr: VirtAddr,
        min_vaddr: VirtAddr,
        mapper: &mut M,
    ) -> Result<()> {
        let virt_addr = region_addr + (ph.virtual_addr() - min_vaddr.as_u64());
        let size = ph.mem_size();

        let flags = PageFlags::r();
        let region = MappedRegion { virt_addr, size };
        mapper
            .update_flags(region, flags)
            .map_err(|()| ElfLoadError::MapperError)?;

        Ok(())
    }

    /// Convert a virtual address to a file offset using LOAD segments.
    fn vaddr_to_file_offset(elf: &ElfFile, vaddr: u64) -> Option<u64> {
        for ph in elf.program_iter() {
            if ph.get_type() != Ok(Type::Load) {
                continue;
            }

            let seg_vaddr = ph.virtual_addr();
            let seg_file_size = ph.file_size();

            if vaddr >= seg_vaddr && vaddr < seg_vaddr + seg_file_size {
                let offset_in_segment = vaddr - seg_vaddr;
                return Some(ph.offset() + offset_in_segment);
            }
        }

        None
    }
}

#[must_use]
#[inline]
fn align_down(value: u64, align: u64) -> u64 {
    debug_assert!(align.is_power_of_two());
    value & !(align - 1)
}

#[inline]
fn align_up(value: u64, align: u64) -> Result<u64> {
    debug_assert!(align.is_power_of_two());
    value
        .checked_add(align - 1)
        .map(|v| v & !(align - 1))
        .ok_or(ElfLoadError::Overflow)
}
