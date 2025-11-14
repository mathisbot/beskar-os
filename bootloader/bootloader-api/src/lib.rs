#![no_std]
#![forbid(unsafe_op_in_unsafe_fn)]
#![warn(clippy::pedantic, clippy::nursery)]

use beskar_core::{
    arch::{PhysAddr, VirtAddr},
    mem::ranges::MemoryRange,
    video::FrameBuffer,
};

#[macro_export]
/// This macro defines the entry point of the kernel.
///
/// This will be called by the bootloader.
///
/// You can pass additional arguments that will be forwarded to your entry point function.
macro_rules! entry_point {
    ($path:path $(, $arg:expr)*) => {
        #[unsafe(export_name = "_start")]
        extern "C" fn __kernel_entry(boot_info: &'static mut $crate::BootInfo) -> ! {
            ($path)(boot_info $(, $arg)*)
        }
    };
}

/// This structure represents the information that the bootloader passes to the kernel.
#[derive(Debug)]
pub struct BootInfo {
    /// A map of the physical memory regions.
    pub memory_regions: &'static mut [MemoryRange],
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

impl BootInfo {
    #[must_use]
    #[inline]
    pub const fn memory_regions(&'static mut self) -> &'static mut [MemoryRange] {
        self.memory_regions
    }

    #[must_use]
    #[inline]
    /// Returns the framebuffer for screen output.
    pub const fn framebuffer(&self) -> &FrameBuffer {
        &self.framebuffer
    }

    #[must_use]
    #[inline]
    /// Returns the page index of the recursive level 4 table.
    pub const fn recursive_index(&self) -> u16 {
        self.recursive_index
    }

    #[must_use]
    #[inline]
    /// Returns the address of the `RSDP`, used to find the ACPI tables (if reported).
    pub const fn rsdp_paddr(&self) -> Option<PhysAddr> {
        self.rsdp_paddr
    }

    #[must_use]
    #[inline]
    /// Returns the information about the kernel ELF.
    pub const fn kernel_info(&self) -> &KernelInfo {
        &self.kernel_info
    }

    #[must_use]
    #[inline]
    /// Returns the information about the ramdisk.
    pub const fn ramdisk_info(&self) -> Option<&RamdiskInfo> {
        self.ramdisk_info.as_ref()
    }

    #[must_use]
    #[inline]
    /// Returns the number of enabled and healthy CPU cores in the system.
    pub const fn cpu_count(&self) -> usize {
        self.cpu_count
    }
}

#[derive(Debug, Clone, Copy)]
pub struct KernelInfo {
    /// Physical address of the kernel ELF in memory.
    paddr: PhysAddr,
    /// Virtual address of the loaded kernel image,
    /// in the kernel's address space.
    vaddr: VirtAddr,
    /// Size of the kernel ELF in memory.
    size: u64,
}

impl KernelInfo {
    #[must_use]
    #[inline]
    pub const fn new(paddr: PhysAddr, vaddr: VirtAddr, size: u64) -> Self {
        Self { paddr, vaddr, size }
    }

    #[must_use]
    #[inline]
    /// Returns the physical address of the kernel ELF in memory.
    pub const fn paddr(&self) -> PhysAddr {
        self.paddr
    }

    #[must_use]
    #[inline]
    /// Returns the virtual address of the loaded kernel image,
    /// in the kernel's address space.
    pub const fn vaddr(&self) -> VirtAddr {
        self.vaddr
    }

    #[must_use]
    #[inline]
    /// Returns the size of the kernel ELF in memory.
    pub const fn size(&self) -> u64 {
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
    pub const fn new(vaddr: VirtAddr, size: u64) -> Self {
        Self { vaddr, size }
    }

    #[must_use]
    #[inline]
    /// Returns the virtual address of the ramdisk.
    pub const fn vaddr(&self) -> VirtAddr {
        self.vaddr
    }

    #[must_use]
    #[inline]
    /// Returns the size of the ramdisk in memory.
    pub const fn size(&self) -> u64 {
        self.size
    }
}

/// Kernel space starting page table entry.
pub const KERNEL_PT_START_ENTRY: u16 = 256;
/// Kernel space starting page table entry.
pub const KERNEL_PT_RECURSIVE_INDEX: u16 = 256;
/// Kernel higher half base virtual address.
pub const KERNEL_AS_BASE: VirtAddr = VirtAddr::new_extend(256 << 39);

/// Kernel image base virtual address.
pub const KERNEL_IMAGE_BASE: VirtAddr = VirtAddr::new_extend(257 << 39);

/// Ramdisk base virtual address.
pub const RAMDISK_BASE: VirtAddr = VirtAddr::new_extend(258 << 39);

/// Kernel stack base virtual address.
pub const KERNEL_STACK_BASE: VirtAddr = VirtAddr::new_extend(259 << 39);
/// Boot information base virtual address.
pub const BOOT_INFO_BASE: VirtAddr = VirtAddr::new_extend((259 << 39) | (256 << 21));
/// Framebuffer base virtual address.
pub const FRAMEBUFFER_BASE: VirtAddr = VirtAddr::new_extend((259 << 39) | (257 << 21));

/// Kernel pool base virtual address.
///
/// It ranges from `KERNEL_POOL_BASE` to the end of the virtual address space.
pub const KERNEL_POOL_BASE: VirtAddr = VirtAddr::new_extend(260 << 39);
