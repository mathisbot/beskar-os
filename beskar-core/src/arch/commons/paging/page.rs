//! Abstractions for default-sized and huge virtual memory pages.
use super::{super::VirtAddr, M1GiB, M2MiB, M4KiB, MemSize};
use core::marker::PhantomData;
use core::ops::{Add, Sub};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
/// A virtual memory page.
#[repr(C)]
pub struct Page<S: MemSize = M4KiB> {
    start_address: VirtAddr,
    size: PhantomData<S>,
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum PageError {
    #[error("Unaligned address")]
    UnalignedAddress,
}

impl<S: MemSize> Page<S> {
    #[inline]
    pub fn from_start_address(address: VirtAddr) -> Result<Self, PageError> {
        // Check that the address is correctly aligned.
        if address != address.align_down(S::SIZE) {
            return Err(PageError::UnalignedAddress);
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

impl Page<M1GiB> {
    #[must_use]
    #[inline]
    pub fn from_p4p3(p4: u16, p3: u16) -> Self {
        debug_assert!(p4 < 512);
        debug_assert!(p3 < 512);
        let addr = (u64::from(p4) << 39) | (u64::from(p3) << 30);
        let vaddr = VirtAddr::new_extend(addr);

        Self {
            start_address: vaddr,
            size: PhantomData,
        }
    }
}

impl Page<M2MiB> {
    #[must_use]
    #[inline]
    pub fn p2_index(self) -> u16 {
        self.start_address().p2_index()
    }

    #[must_use]
    #[inline]
    pub fn from_p4p3p2(p4: u16, p3: u16, p2: u16) -> Self {
        debug_assert!(p4 < 512);
        debug_assert!(p3 < 512);
        debug_assert!(p2 < 512);
        let addr = (u64::from(p4) << 39) | (u64::from(p3) << 30) | (u64::from(p2) << 21);
        let vaddr = VirtAddr::new_extend(addr);

        Self {
            start_address: vaddr,
            size: PhantomData,
        }
    }
}

impl Page<M4KiB> {
    #[must_use]
    #[inline]
    pub fn p2_index(self) -> u16 {
        self.start_address().p2_index()
    }

    #[must_use]
    #[inline]
    pub fn p1_index(self) -> u16 {
        self.start_address().p1_index()
    }

    #[must_use]
    #[inline]
    pub fn from_p4p3p2p1(p4: u16, p3: u16, p2: u16, p1: u16) -> Self {
        debug_assert!(p4 < 512);
        debug_assert!(p3 < 512);
        debug_assert!(p2 < 512);
        debug_assert!(p1 < 512);
        let addr = (u64::from(p4) << 39)
            | (u64::from(p3) << 30)
            | (u64::from(p2) << 21)
            | (u64::from(p1) << 12);
        let vaddr = VirtAddr::new_extend(addr);

        Self {
            start_address: vaddr,
            size: PhantomData,
        }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

impl<S: MemSize> IntoIterator for PageRangeInclusive<S> {
    type Item = Page<S>;
    type IntoIter = PageIterator<S>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        PageIterator {
            start: self.start,
            end: self.end,
        }
    }
}

#[derive(Clone)]
pub struct PageIterator<S: MemSize = M4KiB> {
    start: Page<S>,
    end: Page<S>,
}

impl<S: MemSize> Iterator for PageIterator<S> {
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

impl<S: MemSize> ExactSizeIterator for PageIterator<S> {
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

impl<S: MemSize> DoubleEndedIterator for PageIterator<S> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_p() {
        let page = Page::<M4KiB>::from_start_address(VirtAddr::new(0x1000)).unwrap();
        assert_eq!(page.size(), M4KiB::SIZE);
        assert_eq!(page.start_address(), VirtAddr::new(0x1000));

        let same_page = Page::<M4KiB>::containing_address(VirtAddr::new(0x1FFF));
        assert_eq!(page, same_page);
    }

    #[test]
    fn test_p_unaligned() {
        let unaligned_page = Page::<M4KiB>::from_start_address(VirtAddr::new(0x1001));
        assert!(unaligned_page == Err(PageError::UnalignedAddress));
    }

    #[test]
    fn test_p_op() {
        let page = Page::<M4KiB>::from_start_address(VirtAddr::new(0x2000)).unwrap();
        let next_page = Page::<M4KiB>::from_start_address(VirtAddr::new(0x3000)).unwrap();
        let previous_page = Page::<M4KiB>::from_start_address(VirtAddr::new(0x1000)).unwrap();

        assert_eq!(page + 1, next_page);
        assert_eq!(page - 1, previous_page);
        assert_eq!(next_page - page, 1);
    }

    #[test]
    fn test_p_range() {
        let start = Page::<M4KiB>::from_start_address(VirtAddr::new(0x1000)).unwrap();
        let end = Page::<M4KiB>::containing_address(VirtAddr::new(0x2FFF));
        let range = Page::range_inclusive(start, end);
        assert_eq!(range.len(), 2);
        assert_eq!(range.size(), 2 * M4KiB::SIZE);

        let mut iter = range.into_iter();
        let first = iter.next().unwrap();
        let second = iter.next().unwrap();
        assert_eq!(first, start);
        assert_eq!(second, end);
        assert!(iter.next().is_none());

        let empty_range = Page::range_inclusive(end, start);
        assert!(empty_range.is_empty());
    }
}
