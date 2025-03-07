use beskar_core::arch::commons::{
    PhysAddr,
    paging::{Frame, M4KiB, MemSize as _},
};
use beskar_core::mem::{MemoryRegion, MemoryRegionUsage};
use uefi::{
    boot::{MemoryDescriptor, MemoryType},
    mem::memory_map::{MemoryMap, MemoryMapOwned},
};
use x86_64::structures::paging::{FrameAllocator, PhysFrame, Size4KiB};

/// A physical frame allocator based on a UEFI provided memory map.
///
/// Its purpose is to keep track of used regions, to avoid overwriting them in the kernel.
///
/// Allocation is done in a linear fashion, starting from the first usable frame.
pub struct EarlyFrameAllocator {
    memory_map: MemoryMapOwned,
    current_entry_index: usize,
    /// Keeps track of the current maximum allocated frame.
    next_frame: Frame,
    /// Keeps track of the first allocated frame.
    min_frame: Frame,
    /// The largest detected physical memory address.
    max_physical_address: PhysAddr,
}

impl EarlyFrameAllocator {
    #[must_use]
    #[inline]
    /// Creates a new frame allocator based on the given memory map.
    ///
    /// It is assumed but not mandatory that the memory map is sorted by physical address.
    pub fn new(memory_map: MemoryMapOwned) -> Self {
        // Skip the lower 1 MiB of frames by convention.
        // This also skips `0x00` because Rust assumes that null references are not valid.
        let frame = Frame::containing_address(PhysAddr::new(0x10_0000));

        let max = memory_map
            .entries()
            .map(|r| PhysAddr::new(r.phys_start) + r.page_count * M4KiB::SIZE)
            .max()
            .unwrap();

        // First 4 GiB of physical memory contain important info, such as ACPI tables.
        // Therefore, they should always be covered.
        let max_physical_address = max.max(PhysAddr::new(0x1_0000_0000));

        Self {
            memory_map,
            current_entry_index: 0,
            next_frame: frame,
            min_frame: frame,
            max_physical_address,
        }
    }

    #[must_use]
    fn allocate_frame_from_descriptor(&mut self, descriptor: &MemoryDescriptor) -> Option<Frame> {
        let start_paddr = PhysAddr::new(descriptor.phys_start);
        let end_paddr = start_paddr + descriptor.page_count * M4KiB::SIZE;

        let start_frame = Frame::containing_address(start_paddr);
        let end_frame = Frame::containing_address(end_paddr - 1u64);

        if self.next_frame < start_frame {
            self.next_frame = start_frame;
        }

        if self.next_frame <= end_frame {
            let ret = self.next_frame;
            self.next_frame = self.next_frame + 1;

            Some(ret)
        } else {
            None
        }
    }

    #[must_use]
    #[inline]
    /// Returns the largest detected physical memory address.
    pub const fn max_physical_address(&self) -> PhysAddr {
        self.max_physical_address
    }

    #[must_use]
    #[inline]
    pub fn memory_map_max_region_count(&self) -> usize {
        // There are 2 used regions (kernel, and bootloader heap)
        // that can split into up 2 extra spaces.
        self.memory_map.len() + 2 * 2
    }

    #[must_use]
    /// Convert the allocator into a memory map understandable by the kernel.
    pub fn construct_memory_map(
        self,
        regions: &mut [MemoryRegion],
        kernel_slice_start: PhysAddr,
        kernel_slice_len: u64,
    ) -> &mut [MemoryRegion] {
        let used_slices = [
            (
                self.min_frame.start_address().as_u64(), // Is aligned to 0x1000
                self.next_frame.start_address().as_u64(), // Is aligned to 0x1000
            ),
            (
                kernel_slice_start.align_down(0x1000_u64).as_u64(),
                (kernel_slice_start + kernel_slice_len)
                    .align_up(0x1000_u64)
                    .as_u64(),
            ),
        ];

        let mut next_index = 0;
        for descriptor in self.memory_map.entries() {
            let kind = if usable_after_bootloader_exit(descriptor) {
                MemoryRegionUsage::Usable
            } else {
                MemoryRegionUsage::Unknown(descriptor.ty.0)
            };

            let start = PhysAddr::new(descriptor.phys_start);
            let end = PhysAddr::new(descriptor.phys_start) + descriptor.page_count * M4KiB::SIZE;

            let region = MemoryRegion::new(start.as_u64(), end.as_u64(), kind);

            if kind == MemoryRegionUsage::Usable {
                Self::split_region(region, regions, &mut next_index, &used_slices);
            } else {
                Self::add_region(region, regions, &mut next_index);
            }
        }

        &mut regions[..next_index]
    }

    fn split_region(
        mut region: MemoryRegion,
        regions: &mut [MemoryRegion],
        next_index: &mut usize,
        used_slices: &[(u64, u64)],
    ) {
        while region.start() != region.end() {
            // Check for overlaps with used slices.
            if let Some((overlap_start, overlap_end)) = used_slices
                .iter()
                .filter_map(|(start, end)| {
                    let overlap_start = region.start().max(*start);
                    let overlap_end = region.end().min(*end);
                    if overlap_start < overlap_end {
                        Some((overlap_start, overlap_end))
                    } else {
                        None
                    }
                })
                .min_by_key(|&(overlap_start, _)| overlap_start)
            {
                let usable =
                    MemoryRegion::new(region.start(), overlap_start, MemoryRegionUsage::Usable);
                let bootloader =
                    MemoryRegion::new(overlap_start, overlap_end, MemoryRegionUsage::Bootloader);

                Self::add_region(usable, regions, next_index);
                Self::add_region(bootloader, regions, next_index);

                region = MemoryRegion::new(overlap_end, region.end(), region.kind());
            } else {
                Self::add_region(region, regions, next_index);
                break;
            }
        }
    }

    fn add_region(region: MemoryRegion, regions: &mut [MemoryRegion], next_index: &mut usize) {
        if region.start() == region.end() {
            return;
        }

        assert!(
            *next_index < regions.len(),
            "Memory regions array is too small to hold all regions"
        );

        regions[*next_index] = region;

        *next_index += 1;
    }
}

unsafe impl FrameAllocator<Size4KiB> for EarlyFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        while let Some(&descriptor) = self.memory_map.get(self.current_entry_index) {
            if descriptor.ty == MemoryType::CONVENTIONAL {
                if let Some(frame) = self.allocate_frame_from_descriptor(&descriptor) {
                    return Some(unsafe { core::mem::transmute(frame) });
                }
            }
            self.current_entry_index += 1;
        }
        None
    }
}

#[must_use]
/// Returns whether the memory region is usable after the bootloader exits.
const fn usable_after_bootloader_exit(memory_descriptor: &MemoryDescriptor) -> bool {
    // TODO: Find a way to send ACPI_RECLAIM regions to the kernel?
    match memory_descriptor.ty {
        MemoryType::CONVENTIONAL
        | MemoryType::LOADER_CODE
        | MemoryType::LOADER_DATA
        | MemoryType::BOOT_SERVICES_CODE
        | MemoryType::BOOT_SERVICES_DATA => true,
        #[allow(clippy::match_same_arms)]
        MemoryType::RUNTIME_SERVICES_CODE | MemoryType::RUNTIME_SERVICES_DATA => {
            // According to the UEFI specification, these should be left
            // untouched by the operating system
            false
        }
        _ => false,
    }
}
