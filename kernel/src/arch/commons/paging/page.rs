use super::super::VirtAddr;
use core::marker::PhantomData;
use core::ops::{Add, Sub};

use super::{M2MiB, M4KiB, MemSize};

/// A virtual memory page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(C)]
pub struct Page<S: MemSize = M4KiB> {
    start_address: VirtAddr,
    size: PhantomData<S>,
}

impl<S: MemSize> Page<S> {
    #[inline]
    pub fn from_start_address(address: VirtAddr) -> Result<Self, ()> {
        // Check that the address is correctly aligned.
        if address != address.align_down(S::SIZE) {
            return Err(());
        }
        Ok(Self {
            start_address: address,
            size: PhantomData,
        })
    }

    #[must_use]
    #[inline]
    pub const fn containing_address(address: VirtAddr) -> Self {
        Self {
            start_address: address.align_down(S::SIZE),
            size: PhantomData,
        }
    }

    #[must_use]
    #[inline]
    pub const fn start_address(self) -> VirtAddr {
        self.start_address
    }

    #[allow(clippy::unused_self)]
    #[must_use]
    #[inline]
    pub const fn size(self) -> u64 {
        S::SIZE
    }

    #[must_use]
    #[inline]
    pub fn p4_index(self) -> u16 {
        self.start_address().p4_index()
    }

    #[must_use]
    #[inline]
    pub fn p3_index(self) -> u16 {
        self.start_address().p3_index()
    }

    #[must_use]
    #[inline]
    pub const fn range_inclusive(start: Self, end: Self) -> PageRangeInclusive<S> {
        PageRangeInclusive { start, end }
    }
}

impl Page<M2MiB> {
    #[must_use]
    pub fn p2_index(self) -> u16 {
        self.start_address().p2_index()
    }
}

impl Page<M4KiB> {
    #[must_use]
    pub fn p2_index(self) -> u16 {
        self.start_address().p2_index()
    }

    #[must_use]
    pub fn p1_index(self) -> u16 {
        self.start_address().p1_index()
    }
}

impl<S: MemSize> Add<u64> for Page<S> {
    type Output = Self;
    #[inline]
    fn add(self, rhs: u64) -> Self::Output {
        Self::containing_address(self.start_address() + rhs * S::SIZE)
    }
}

impl<S: MemSize> Sub<u64> for Page<S> {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: u64) -> Self::Output {
        Self::containing_address(self.start_address() - rhs * S::SIZE)
    }
}

impl<S: MemSize> Sub<Self> for Page<S> {
    type Output = u64;
    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        (self.start_address - rhs.start_address) / S::SIZE
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct PageRangeInclusive<S: MemSize = M4KiB> {
    pub start: Page<S>,
    pub end: Page<S>,
}

impl<S: MemSize> PageRangeInclusive<S> {
    #[must_use]
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.start > self.end
    }

    #[must_use]
    #[inline]
    pub fn len(&self) -> u64 {
        if self.is_empty() {
            0
        } else {
            self.end - self.start + 1
        }
    }

    #[must_use]
    #[inline]
    pub fn size(&self) -> u64 {
        S::SIZE * self.len()
    }
}

impl<S: MemSize> Iterator for PageRangeInclusive<S> {
    type Item = Page<S>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.start <= self.end {
            let page = self.start;
            self.start = page + 1;
            Some(page)
        } else {
            None
        }
    }
}

impl<S: MemSize> ExactSizeIterator for PageRangeInclusive<S> {
    fn len(&self) -> usize {
        usize::try_from(self.len()).unwrap()
    }
}

impl<S: MemSize> DoubleEndedIterator for PageRangeInclusive<S> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.start <= self.end {
            let page = self.end;
            // This cannot underflow, as Page(0) is VirtAddr(0),
            // which is invalid in Rust.
            self.end = page - 1;
            Some(page)
        } else {
            None
        }
    }
}
