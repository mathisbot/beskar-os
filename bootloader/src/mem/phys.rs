use beskar_core::{
    arch::{
        PhysAddr, VirtAddr,
        paging::{Frame, FrameAllocator, M4KiB, MemSize as _},
    },
    mem::ranges::MemoryRange,
};
use uefi::{
    boot::{MemoryDescriptor, MemoryType},
    mem::memory_map::{MemoryMap, MemoryMapOwned},
};

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
        // Bootloader memory allocator can end up creating 2
        // more memory regions when splitting the memory map.
        self.memory_map.len() + 2
    }

    #[must_use]
    /// Convert the allocator into a memory map understandable by the kernel.
    pub fn construct_memory_map(
        self,
        start_vaddr: VirtAddr,
        max_regions: usize,
    ) -> &'static mut [MemoryRange] {
        let regions =
            unsafe { core::slice::from_raw_parts_mut(start_vaddr.as_mut_ptr(), max_regions) };

        let allocated_slice = (
            self.min_frame.start_address().as_u64(), // Is aligned to 0x1000
            self.next_frame.start_address().as_u64(), // Is aligned to 0x1000
        );

        let mut next_index = 0;
        for descriptor in self.memory_map.entries() {
            let start = PhysAddr::new(descriptor.phys_start);
            let end = PhysAddr::new(descriptor.phys_start) + descriptor.page_count * M4KiB::SIZE;

            let region = MemoryRange::new(start.as_u64(), end.as_u64().checked_sub(1).unwrap());

            if usable_after_bootloader_exit(descriptor) {
                Self::split_region(region, regions, &mut next_index, allocated_slice);
            }
        }

        &mut regions[..next_index]
    }

    fn split_region(
        region: MemoryRange,
        regions: &mut [MemoryRange],
        next_index: &mut usize,
        allocated_slice: (u64, u64),
    ) {
        let overlap_start = region.start().max(allocated_slice.0);
        let overlap_end = region.end().min(allocated_slice.1);
        if overlap_start < overlap_end {
            if region.start() < overlap_start.checked_sub(1).unwrap() {
                // Add the region before the overlap.
                Self::add_region(
                    MemoryRange::new(region.start(), overlap_start.checked_sub(1).unwrap()),
                    regions,
                    next_index,
                );
            }
            if overlap_end.checked_add(1).unwrap() < region.end() {
                // Add the region after the overlap.
                Self::add_region(
                    MemoryRange::new(overlap_end.checked_add(1).unwrap(), region.end()),
                    regions,
                    next_index,
                );
            }
        } else {
            // No overlap, add the region as is.
            Self::add_region(region, regions, next_index);
        }
    }

    fn add_region(region: MemoryRange, regions: &mut [MemoryRange], next_index: &mut usize) {
        if region.start() == region.end() {
            return;
        }

        assert!(
            *next_index < regions.len(),
            "Memory range array is too small to hold all regions"
        );

        regions[*next_index] = region;

        *next_index += 1;
    }
}

impl FrameAllocator<M4KiB> for EarlyFrameAllocator {
    fn allocate_frame(&mut self) -> Option<Frame<M4KiB>> {
        while let Some(&descriptor) = self.memory_map.get(self.current_entry_index) {
            if descriptor.ty == MemoryType::CONVENTIONAL
                && let Some(frame) = self.allocate_frame_from_descriptor(&descriptor)
            {
                return Some(frame);
            }
            self.current_entry_index += 1;
        }
        None
    }

    fn deallocate_frame(&mut self, _frame: Frame<M4KiB>) {
        // No-op, as we don't support deallocation in this allocator.
        // This is a simple allocator that only allocates frames.
        // Deallocation is not supported.
    }
}

#[must_use]
/// Returns whether the memory region is usable after the bootloader exits.
const fn usable_after_bootloader_exit(memory_descriptor: &MemoryDescriptor) -> bool {
    match memory_descriptor.ty {
        MemoryType::CONVENTIONAL
        | MemoryType::LOADER_CODE
        | MemoryType::BOOT_SERVICES_CODE
        | MemoryType::BOOT_SERVICES_DATA => true,
        #[expect(
            clippy::match_same_arms,
            reason = "Differentiate reasons for not usable"
        )]
        MemoryType::RUNTIME_SERVICES_CODE | MemoryType::RUNTIME_SERVICES_DATA => {
            // According to the UEFI specification, these should be left
            // untouched by the operating system
            false
        }
        _ => false,
    }
}
