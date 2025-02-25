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
    /// Information about the kernel ELF.
    pub kernel_info: KernelInfo,
    /// Information about the ramdisk.
    pub ramdisk_info: Option<RamdiskInfo>,
    /// Number of enabled and healthy CPU cores in the system.
    pub cpu_count: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct KernelInfo {
    /// Physical address of the kernel ELF in memory.
    paddr: PhysAddr,
    /// Virtual address of the loaded kernel image.
    vaddr: VirtAddr,
    /// Size of the kernel ELF in memory.
    size: u64,
}

impl KernelInfo {
    #[must_use]
    #[inline]
    pub fn new(paddr: PhysAddr, vaddr: VirtAddr, size: u64) -> Self {
        Self { paddr, vaddr, size }
    }

    #[must_use]
    #[inline]
    /// Returns the physical address of the kernel ELF in memory.
    pub fn paddr(&self) -> PhysAddr {
        self.paddr
    }

    #[must_use]
    #[inline]
    /// Returns the virtual address of the loaded kernel image.
    pub fn vaddr(&self) -> VirtAddr {
        self.vaddr
    }

    #[must_use]
    #[inline]
    /// Returns the size of the kernel ELF in memory.
    pub fn size(&self) -> u64 {
        self.size
    }
}
#[derive(Debug, Clone, Copy)]
pub struct RamdiskInfo {
    /// Virtual address of the ramdisk.
    vaddr: VirtAddr,
    /// Size of the ramdisk in memory.
    size: u64,
}

impl RamdiskInfo {
    #[must_use]
    #[inline]
    pub fn new(vaddr: VirtAddr, size: u64) -> Self {
        Self { vaddr, size }
    }

    #[must_use]
    #[inline]
    /// Returns the virtual address of the ramdisk.
    pub fn vaddr(&self) -> VirtAddr {
        self.vaddr
    }

    #[must_use]
    #[inline]
    /// Returns the size of the ramdisk in memory.
    pub fn size(&self) -> u64 {
        self.size
    }
}
