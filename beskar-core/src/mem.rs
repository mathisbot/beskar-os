pub mod ranges;

/// Represent a physical memory region.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct MemoryRegion {
    /// The physical start address of the region.
    start: u64,
    /// The physical end address (exclusive) of the region.
    end: u64,
    /// The memory usage of the memory region.
    kind: MemoryRegionUsage,
}

impl MemoryRegion {
    #[must_use]
    #[inline]
    /// Create a new memory region.
    pub const fn new(start: u64, end: u64, kind: MemoryRegionUsage) -> Self {
        Self { start, end, kind }
    }

    #[must_use]
    #[inline]
    pub const fn start(&self) -> u64 {
        self.start
    }

    #[must_use]
    #[inline]
    pub const fn end(&self) -> u64 {
        self.end
    }

    #[must_use]
    #[inline]
    pub const fn kind(&self) -> MemoryRegionUsage {
        self.kind
    }
}

/// Represents the different usage of memory.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum MemoryRegionUsage {
    /// Unused conventional memory, can be used by the kernel.
    Usable,
    /// Memory mappings created by the bootloader, including the page table and boot info mappings.
    Bootloader,
    /// An unknown memory region reported by the firmware, containing the UEFI memory type tag.
    Unknown(u32),
}
