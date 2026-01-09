//! Paging module for memory management.
//!
//! It defines the `Frame` and `Page` types, which represent physical memory frames and virtual memory pages, respectively,
//! as well as traits for memory mapping and translation.
use super::{Alignment, PhysAddr, VirtAddr};
use thiserror::Error;

mod frame;
pub use frame::{Frame, FrameAllocator, FrameRangeInclusive};
mod page;
pub use page::{Page, PageRangeInclusive};

trait Sealed {}
impl Sealed for M4KiB {}
impl Sealed for M2MiB {}
impl Sealed for M1GiB {}

#[expect(private_bounds, reason = "Forbid impl `MemSize`")]
pub trait MemSize: Sealed + Copy + Eq + Ord + PartialOrd {
    const SIZE: u64;
    const ALIGNMENT: Alignment;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct M4KiB {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct M2MiB {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct M1GiB {}

impl MemSize for M4KiB {
    const SIZE: u64 = 4096;
    const ALIGNMENT: Alignment = Alignment::Align4K;
}

impl MemSize for M2MiB {
    const SIZE: u64 = M4KiB::SIZE * 512;
    const ALIGNMENT: Alignment = Alignment::Align2M;
}

impl MemSize for M1GiB {
    const SIZE: u64 = M2MiB::SIZE * 512;
    const ALIGNMENT: Alignment = Alignment::Align1G;
}

pub trait Flags {}

pub trait CacheFlush<S: MemSize> {
    fn flush(&self);
    /// Ignore the flush operation on the TLB.
    ///
    /// # Safety
    ///
    /// The page table containing the page must not be used at the moment,
    /// otherwise the CPU will not be aware of the changes.
    unsafe fn ignore_flush(&self) {}
    fn page(&self) -> Page<S>;
}

pub trait Mapper<S: MemSize, F: Flags> {
    fn map<A: FrameAllocator<M4KiB>>(
        &mut self,
        page: Page<S>,
        frame: Frame<S>,
        flags: F,
        fralloc: &mut A,
    ) -> Result<impl CacheFlush<S>, MappingError<S>>;
    fn unmap(&mut self, page: Page<S>) -> Result<(Frame<S>, impl CacheFlush<S>), MappingError<S>>;
    fn update_flags(
        &mut self,
        page: Page<S>,
        flags: F,
    ) -> Result<impl CacheFlush<S>, MappingError<S>>;
    fn translate(&self, page: Page<S>) -> Option<(Frame<S>, F)>;
}

pub trait Translator<F: Flags> {
    fn translate_addr(&self, addr: VirtAddr) -> Option<(PhysAddr, F)>;
}

#[derive(Error, Debug, Clone, Copy, PartialEq, Eq)]
pub enum MappingError<S: MemSize> {
    #[error("Page is already mapped")]
    AlreadyMapped(Frame<S>),
    #[error("Frame allocation failed")]
    FrameAllocationFailed,
    #[error("Page is not mapped")]
    NotMapped,
    #[error("Unexpected large page encountered")]
    UnexpectedLargePage,
    #[error("Unexpected not large page encountered")]
    UnexpectedNotLargePage,
}
