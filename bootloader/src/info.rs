use crate::FrameBuffer;

/// This structure represents the information that the bootloader passes to the kernel.
#[derive(Debug)]
#[non_exhaustive]
pub struct BootInfo {
    /// A map of the physical memory regions.
    pub memory_regions: &'static mut [MemoryRegion],
    /// Framebuffer for screen output.
    pub framebuffer: FrameBuffer,
    /// The page index of the recursive level 4 table.
    pub recursive_index: u16,
    /// The address of the `RSDP`, used to find the ACPI tables (if reported).
    pub rsdp_paddr: Option<u64>,
    /// Physical address of the kernel ELF in memory.
    pub kernel_vaddr: u64,
    /// Size of the kernel ELF in memory.
    pub kernel_len: u64,
    /// Virtual address of the loaded kernel image.
    pub kernel_image_offset: u64,
    /// An optional template for the thread local storage.
    pub tls_template: Option<TlsTemplate>,
    /// Number of enabled and healthy CPU cores in the system.
    pub cpu_count: u8,
}

/// Represent a physical memory region.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct MemoryRegion {
    /// The physical start address of the region.
    pub start: u64,
    /// The physical end address (exclusive) of the region.
    pub end: u64,
    /// The memory usage of the memory region.
    pub kind: MemoryRegionUsage,
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

/// Information about the thread local storage template.
///
/// This template can be used to set up thread local storage for threads. For
/// each thread, a new memory location of size `mem_size` must be initialized.
/// Then the first `file_size` bytes of this template needs to be copied to the
/// location. The additional `mem_size - file_size` bytes must be initialized with
/// zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TlsTemplate {
    /// The virtual start address of the thread local storage template.
    pub start_addr: u64,
    /// The number of data bytes in the template.
    ///
    /// Corresponds to the length of the `.tdata` section.
    pub file_size: u64,
    /// The total number of bytes that the TLS segment should have in memory.
    ///
    /// Corresponds to the combined length of the `.tdata` and `.tbss` sections.
    pub mem_size: u64,
}

/// Various information about the system.
#[derive(Debug)]
pub struct EarlySystemInfo {
    /// Framebuffer for screen output.
    pub framebuffer: FrameBuffer,
    /// The address of the `RSDP`, used to find the ACPI tables (if reported).
    pub rsdp_paddr: Option<x86_64::PhysAddr>,
    /// Number of CPU cores in the system.
    ///
    /// It is guaranteed that the BSP is ID 0 and that all
    /// other core are enabled.
    pub cpu_count: u8,
}
