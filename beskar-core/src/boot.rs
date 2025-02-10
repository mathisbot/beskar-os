use crate::{
    arch::commons::{PhysAddr, VirtAddr},
    mem::MemoryRegion,
    video::FrameBuffer,
};

/// This structure represents the information that the bootloader passes to the kernel.
#[derive(Debug)]
pub struct BootInfo {
    /// A map of the physical memory regions.
    pub memory_regions: &'static mut [MemoryRegion],
    /// Framebuffer for screen output.
    pub framebuffer: FrameBuffer,
    /// The page index of the recursive level 4 table.
    pub recursive_index: u16,
    /// The address of the `RSDP`, used to find the ACPI tables (if reported).
    pub rsdp_paddr: Option<PhysAddr>,
    /// Physical address of the kernel ELF in memory.
    pub kernel_paddr: PhysAddr,
    /// Virtual address of the loaded kernel image.
    pub kernel_vaddr: VirtAddr,
    /// Size of the kernel ELF in memory.
    pub kernel_len: u64,
    /// An optional template for the thread local storage.
    pub tls_template: Option<TlsTemplate>,
    /// Number of enabled and healthy CPU cores in the system.
    pub cpu_count: usize,
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
