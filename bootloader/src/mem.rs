use x86_64::structures::paging::{OffsetPageTable, PhysFrame};

mod phys;
pub use phys::EarlyFrameAllocator;

mod virt;
pub use virt::Level4Entries;

/// Provides access to the page tables of the bootloader and kernel address space.
pub struct PageTables {
    /// Provides access to the page tables of the bootloader address space.
    pub bootloader: OffsetPageTable<'static>,
    /// Provides access to the page tables of the kernel address space (not active).
    pub kernel: OffsetPageTable<'static>,
    /// The physical frame where the level 4 page table of the kernel address space is stored.
    ///
    /// Must be the page table that the `kernel` field of this struct refers to.
    pub kernel_level_4_frame: PhysFrame,
}
