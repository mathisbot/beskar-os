mod frame;
pub use frame::{Frame, FrameAllocator, FrameRangeInclusive};
mod page;
pub use page::{Page, PageRangeInclusive};

// TODO: Architecture agnostic flags
pub use crate::arch::x86_64::paging::page_table::Flags;

use super::{PhysAddr, VirtAddr};

pub trait MemSize: Copy + Eq + Ord + PartialOrd {
    const SIZE: u64;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct M4KiB {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct M2MiB {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct M1GiB {}

impl MemSize for M4KiB {
    const SIZE: u64 = 4096;
}

impl MemSize for M2MiB {
    const SIZE: u64 = M4KiB::SIZE * 512;
}

impl MemSize for M1GiB {
    const SIZE: u64 = M2MiB::SIZE * 512;
}

pub trait CacheFlush<S: MemSize> {
    fn flush(&self);
    #[allow(clippy::unused_self)]
    /// Ignore the flush operation on the TLB.
    ///
    /// ## Safety
    ///
    /// The page table containing the page must not be used at the moment,
    /// otherwise the CPU will not be aware of the changes.
    unsafe fn ignore_flush(&self) {}
    fn page(&self) -> Page<S>;
}

pub trait Mapper<S: MemSize> {
    fn map<A: FrameAllocator<M4KiB>>(
        &mut self,
        page: Page<S>,
        frame: Frame<S>,
        flags: Flags,
        fralloc: &mut A,
    ) -> impl CacheFlush<S>;
    fn unmap(&mut self, page: Page<S>) -> Option<(Frame<S>, impl CacheFlush<S>)>;
    fn update_flags(&mut self, page: Page<S>, flags: Flags) -> Option<impl CacheFlush<S>>;
    fn translate(&self, page: Page<S>) -> Option<(Frame<S>, Flags)>;
}

pub trait Translator {
    fn translate_addr(&self, addr: VirtAddr) -> Option<PhysAddr>;
}
