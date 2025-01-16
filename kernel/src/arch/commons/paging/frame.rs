//! Abstractions for default-sized and huge physical memory frames.

use super::super::PhysAddr;
use core::marker::PhantomData;
use core::ops::{Add, Sub};

use super::{MemSize, M4KiB};

/// A physical memory frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Frame<S: MemSize = M4KiB> {
    start_address: PhysAddr,
    size: PhantomData<S>,
}

impl<S: MemSize> Frame<S> {
    #[inline]
    pub fn from_start_address(address: PhysAddr) -> Result<Self, ()> {
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
    pub const fn containing_address(address: PhysAddr) -> Self {
        Self {
            start_address: address.align_down(S::SIZE),
            size: PhantomData,
        }
    }

    #[must_use]
    #[inline]
    pub const fn start_address(self) -> PhysAddr {
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
    pub const fn range_inclusive(start: Self, end: Self) -> FrameRangeInclusive<S> {
        FrameRangeInclusive { start, end }
    }
}

impl<S: MemSize> Add<u64> for Frame<S> {
    type Output = Self;
    #[inline]
    fn add(self, rhs: u64) -> Self::Output {
        Self::containing_address(self.start_address() + rhs * S::SIZE)
    }
}

impl<S: MemSize> Sub<u64> for Frame<S> {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: u64) -> Self::Output {
        Self::containing_address(self.start_address() - rhs * S::SIZE)
    }
}

impl<S: MemSize> Sub<Self> for Frame<S> {
    type Output = u64;
    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        (self.start_address - rhs.start_address) / S::SIZE
    }
}

#[derive(Clone, PartialEq, Eq)]
#[repr(C)]
pub struct FrameRangeInclusive<S: MemSize = M4KiB> {
    pub start: Frame<S>,
    pub end: Frame<S>,
}

impl<S: MemSize> FrameRangeInclusive<S> {
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.start > self.end
    }

    #[inline]
    pub fn len(&self) -> u64 {
        if self.is_empty() {
            0
        } else {
            self.end - self.start + 1
        }
    }

    /// Returns the size in bytes of all frames within the range.
    #[inline]
    pub fn size(&self) -> u64 {
        S::SIZE * self.len()
    }
}

impl<S: MemSize> Iterator for FrameRangeInclusive<S> {
    type Item = Frame<S>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.start <= self.end {
            let frame = self.start;
            self.start = frame + 1;
            Some(frame)
        } else {
            None
        }
    }
}

impl<S: MemSize> ExactSizeIterator for FrameRangeInclusive<S> {
    fn len(&self) -> usize {
        usize::try_from(self.len()).unwrap()
    }
}

impl<S: MemSize> DoubleEndedIterator for FrameRangeInclusive<S> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.start <= self.end {
            let frame = self.end;
            
            // Avoid underflow
            if self.end.start_address().as_u64() == 0 {
                self.start = frame + 1;
            } else {
                self.end = frame - 1;
            }
            Some(frame)
        } else {
            None
        }
    }
}
