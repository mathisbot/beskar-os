//! Abstractions for default-sized and huge physical memory frames.
use super::super::PhysAddr;
use core::marker::PhantomData;
use core::ops::{Add, Sub};

use super::{M4KiB, MemSize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
/// A physical memory frame.
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
    /// Returns the size in bytes of all frames within the range.
    pub fn size(&self) -> u64 {
        S::SIZE * self.len()
    }
}

impl<S: MemSize> IntoIterator for FrameRangeInclusive<S> {
    type Item = Frame<S>;
    type IntoIter = FrameIterator<S>;

    #[must_use]
    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        FrameIterator {
            start: self.start,
            end: self.end,
        }
    }
}

#[derive(Clone)]
pub struct FrameIterator<S: MemSize = M4KiB> {
    pub start: Frame<S>,
    pub end: Frame<S>,
}

impl<S: MemSize> Iterator for FrameIterator<S> {
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

impl<S: MemSize> ExactSizeIterator for FrameIterator<S> {
    #[must_use]
    #[inline]
    fn len(&self) -> usize {
        usize::try_from(if self.start > self.end {
            0
        } else {
            self.end - self.start + 1
        })
        .unwrap()
    }
}

impl<S: MemSize> DoubleEndedIterator for FrameIterator<S> {
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

pub trait FrameAllocator<S: MemSize> {
    fn allocate_frame(&mut self) -> Option<Frame<S>>;
    fn deallocate_frame(&mut self, frame: Frame<S>);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_f() {
        let frame = Frame::<M4KiB>::from_start_address(PhysAddr::new(0x1000)).unwrap();
        assert_eq!(frame.size(), M4KiB::SIZE);
        assert_eq!(frame.start_address(), PhysAddr::new(0x1000));
    }

    #[test]
    fn test_f_unaligned() {
        assert!(Frame::<M4KiB>::from_start_address(PhysAddr::new(0x1001)).is_err());
    }

    #[test]
    fn test_f_range() {
        let start = Frame::<M4KiB>::from_start_address(PhysAddr::new(0x1000)).unwrap();
        let end = Frame::<M4KiB>::containing_address(PhysAddr::new(0x2FFF));
        let range = Frame::range_inclusive(start, end);
        assert_eq!(range.len(), 2);
        assert_eq!(range.size(), 2 * M4KiB::SIZE);
    }
}
