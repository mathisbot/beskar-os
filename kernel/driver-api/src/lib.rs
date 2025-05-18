#![no_std]
#![forbid(unsafe_op_in_unsafe_fn)]
#![warn(clippy::pedantic, clippy::nursery)]

use beskar_core::arch::{
    PhysAddr, VirtAddr,
    paging::{M4KiB, MemSize},
};
use beskar_hal::paging::page_table::Flags;

pub use beskar_core::drivers::{DriverError, DriverResult};

/// Physical Mapping trait
///
/// Be careful to only use the original mapped length, as accessing outside
/// could result in undefined behavior if the memory is used by another mapping.
pub trait PhysicalMappingTrait<S: MemSize = M4KiB> {
    /// Create a new physical mapping.
    fn new(paddr: PhysAddr, length: usize, flags: Flags) -> Self;

    /// Translates a physical address to a virtual address using the current mapping.
    fn translate(&self, paddr: PhysAddr) -> Option<VirtAddr>;
}
