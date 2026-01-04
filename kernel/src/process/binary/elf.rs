use super::LoadedBinary;
use crate::{mem::frame_alloc, process};
use beskar_core::arch::{
    VirtAddr,
    paging::{CacheFlush, FrameAllocator, M4KiB, Mapper, MappingError, MemSize as _, Page},
};
use beskar_core::process::binary::BinaryResult;
use beskar_hal::{paging::page_table::Flags, userspace::Ring};
use elf::{ElfLoader, MemoryMapper, PageFlags, mapper::MappedRegion};

/// Load an ELF binary into memory using the generic ELF loader.
pub fn load(input: &[u8]) -> BinaryResult<LoadedBinary> {
    let mut mapper = ElfMemoryMapper::default();

    ElfLoader::load(input, &mut mapper)
        .map(|bin| LoadedBinary {
            entry_point: bin.entry_point,
            tls_template: bin.tls_template.map(Into::into),
        })
        .map_err(|_| beskar_core::process::binary::LoadError::InvalidBinary)
}

#[derive(Debug, Default)]
struct ElfMemoryMapper {
    /// Allocated page ranges for rollback on error.
    ///
    /// (u64: start address, u64: size)
    allocated_regions: alloc::vec::Vec<(VirtAddr, u64)>,
}

impl MemoryMapper for ElfMemoryMapper {
    fn map_region(&mut self, size: u64, flags: PageFlags) -> Result<MappedRegion, ()> {
        if size == 0 {
            return Err(());
        }

        let page_count = size.div_ceil(M4KiB::SIZE);
        let page_range = process::current()
            .address_space()
            .with_pgalloc(|palloc| palloc.allocate_pages::<M4KiB>(page_count))
            .ok_or(())?;

        let start_page = page_range.start();
        let end_page = start_page + (page_count - 1);
        let base_addr = start_page.start_address();

        let initial_flags = convert_flags(flags, process::current().kind().ring());

        let map_result: Result<(), MappingError<M4KiB>> =
            frame_alloc::with_frame_allocator(|fralloc| {
                process::current().address_space().with_page_table(|pt| {
                    for page in Page::range_inclusive(start_page, end_page) {
                        let frame = fralloc
                            .allocate_frame()
                            .ok_or(MappingError::FrameAllocationFailed)?;
                        pt.map(page, frame, initial_flags, fralloc)?.flush();
                    }
                    Ok(())
                })
            });

        if map_result.is_err() {
            // Best-effort cleanup for partially mapped regions
            release_region(base_addr, page_count * M4KiB::SIZE);
            return Err(());
        }

        self.allocated_regions.push((base_addr, size));

        Ok(MappedRegion {
            virt_addr: base_addr,
            size,
        })
    }

    fn update_flags(&mut self, region: MappedRegion, flags: PageFlags) -> Result<(), ()> {
        if region.size == 0 {
            return Ok(());
        }

        let start_page = Page::<M4KiB>::containing_address(region.virt_addr);
        let end_addr = region.virt_addr + (region.size - 1);
        let end_page = Page::<M4KiB>::containing_address(end_addr);
        let kernel_flags = convert_flags(flags, process::current().kind().ring());

        process::current().address_space().with_page_table(|pt| {
            for page in Page::range_inclusive(start_page, end_page) {
                let tlb_flush = pt.update_flags(page, kernel_flags).map_err(|_| ())?;
                tlb_flush.flush();
            }
            Ok(())
        })
    }

    fn copy_data(&mut self, dest: VirtAddr, src: &[u8]) -> core::result::Result<(), ()> {
        let dest_ptr = dest.as_mut_ptr();
        unsafe {
            core::ptr::copy_nonoverlapping(src.as_ptr(), dest_ptr, src.len());
        }
        Ok(())
    }

    fn zero_region(&mut self, dest: VirtAddr, size: u64) -> core::result::Result<(), ()> {
        let dest_ptr = dest.as_mut_ptr::<u8>();
        unsafe {
            core::ptr::write_bytes(dest_ptr, 0, usize::try_from(size).unwrap());
        }
        Ok(())
    }

    fn unmap_region(&mut self, region: MappedRegion) -> Result<(), ()> {
        if let Some((idx, _)) = self
            .allocated_regions
            .iter()
            .enumerate()
            .find(|(_, (base, _))| *base == region.virt_addr)
        {
            let (_base, recorded_size) = self.allocated_regions.remove(idx);
            let to_free = region.size.min(recorded_size);
            release_region(region.virt_addr, to_free);
        }

        Ok(())
    }

    fn rollback(&mut self) {
        // Cleanup all allocated regions on error
        for (addr, size) in self.allocated_regions.drain(..) {
            release_region(addr, size);
        }
    }
}

/// Convert generic PageFlags to kernel-specific Flags.
fn convert_flags(flags: PageFlags, ring: Ring) -> Flags {
    let mut kernel_flags = Flags::EMPTY;

    if flags.is_present() {
        kernel_flags |= Flags::PRESENT;
    }
    if flags.is_writable() {
        kernel_flags |= Flags::WRITABLE;
    }
    if !flags.is_executable() {
        kernel_flags |= Flags::NO_EXECUTE;
    }
    if flags.is_user_accessible() {
        kernel_flags |= Flags::USER_ACCESSIBLE;
    }

    if ring == Ring::User {
        kernel_flags |= Flags::USER_ACCESSIBLE;
    }

    kernel_flags
}

/// Unmap a region and release frames/pages.
fn release_region(base: VirtAddr, size: u64) {
    if size == 0 {
        return;
    }

    let page_count = size.div_ceil(M4KiB::SIZE);
    let start_page = Page::<M4KiB>::containing_address(base);
    let end_page = start_page + (page_count - 1);
    let page_range = Page::range_inclusive(start_page, end_page);

    frame_alloc::with_frame_allocator(|fralloc| {
        process::current().address_space().with_page_table(|pt| {
            for page in page_range {
                if let Ok((frame, tlb)) = pt.unmap(page) {
                    tlb.flush();
                    fralloc.free(frame);
                }
            }
        });
    });

    process::current().address_space().with_pgalloc(|palloc| {
        palloc.free_pages(Page::range_inclusive(start_page, end_page));
    });
}

impl From<::elf::segments::TlsTemplate> for super::TlsTemplate {
    fn from(tls: ::elf::segments::TlsTemplate) -> Self {
        Self {
            start: tls.start,
            file_size: tls.file_size,
            mem_size: tls.mem_size,
        }
    }
}
