mod frame;
pub use frame::{Frame, FrameRangeInclusive};
mod page;
pub use page::{Page, PageRangeInclusive};

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
