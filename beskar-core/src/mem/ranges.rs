use super::{MemoryRegion, MemoryRegionUsage};
use core::ops::{Index, IndexMut};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
/// Represents a range of memory addresses.
///
/// It is guaranteed that the range is valid, i.e. start <= end.
pub struct MemoryRange {
    /// The start address of the range.
    start: u64,
    /// The end (inclusive) address of the range.
    end: u64,
}

impl MemoryRange {
    #[must_use]
    #[inline]
    pub fn new(start: u64, end: u64) -> Self {
        assert!(start <= end, "Invalid range");
        Self { start, end }
    }

    #[must_use]
    pub fn overlaps(&self, other: &Self) -> Option<Self> {
        // 0-sized overlaps are useless
        if self.start >= other.end || self.end <= other.start {
            None
        } else {
            // The assumption that start <= end is valid:
            // - self.end >= self.start
            // - other.end >= other.start
            // - self.end > other.start (from the if condition)
            // - other.end > self.start (from the if condition)
            Some(Self {
                start: self.start.max(other.start),
                end: self.end.min(other.end),
            })
        }
    }

    #[must_use]
    #[inline]
    /// Returns true if the range is inside the other range.
    pub const fn is_inside(&self, other: &Self) -> bool {
        self.start >= other.start && self.end <= other.end
    }

    #[must_use]
    #[inline]
    pub const fn start(&self) -> u64 {
        self.start
    }

    #[must_use]
    #[inline]
    pub const fn end(&self) -> u64 {
        self.end
    }
}

impl From<MemoryRegion> for MemoryRange {
    fn from(region: MemoryRegion) -> Self {
        assert_eq!(
            region.kind(),
            MemoryRegionUsage::Usable,
            "Memory region is not usable"
        );
        assert!(region.start() < region.end(), "Invalid memory region");
        Self::new(region.start(), region.end() - 1)
    }
}

#[derive(Debug, Clone, Copy)]
/// An array-backed `Vec` (thus statically sized) of `MemoryRange`s.
pub struct MemoryRanges<const N: usize> {
    /// Array of ranges
    ranges: [MemoryRange; N],
    /// Number of ranges that are currently in use
    used: usize,
}

impl<const N: usize> Index<usize> for MemoryRanges<N> {
    type Output = MemoryRange;

    fn index(&self, index: usize) -> &Self::Output {
        assert!(index < self.len(), "Index out of bounds");
        &self.ranges[index]
    }
}

impl<const N: usize> IndexMut<usize> for MemoryRanges<N> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        assert!(index < self.len(), "Index out of bounds");
        &mut self.ranges[index]
    }
}

impl<const N: usize> Default for MemoryRanges<N> {
    fn default() -> Self {
        Self {
            ranges: [MemoryRange::default(); N],
            used: usize::default(),
        }
    }
}

impl<const N: usize> MemoryRanges<N> {
    const _N_VALID: () = assert!(N > 0 && N <= 0xFFFF);

    #[must_use]
    #[inline]
    pub const fn new() -> Self {
        Self {
            ranges: [MemoryRange { start: 0, end: 0 }; N],
            used: 0,
        }
    }

    #[must_use]
    pub fn from_regions(regions: &[MemoryRegion]) -> Self {
        let mut ranges = Self::new();

        for &region in regions
            .iter()
            .filter(|region| region.kind() == MemoryRegionUsage::Usable)
            .take(N)
        {
            ranges.insert(region.into());
        }

        ranges
    }

    #[must_use]
    #[inline]
    pub fn entries(&self) -> &[MemoryRange] {
        &self.ranges[..self.len()]
    }

    #[must_use]
    #[inline]
    pub const fn len(&self) -> usize {
        self.used
    }

    #[must_use]
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn delete(&mut self, index: usize) {
        assert!(index < self.len(), "Index out of bounds");

        // Note that self.used is not 0 because of the assert above
        self.used -= 1;
        self.ranges.swap(index, self.used);
    }

    pub fn insert(&mut self, mut range: MemoryRange) {
        if range.end == range.start {
            return;
        }

        assert!(self.used < N, "MemoryRanges is full");

        // Try merging the new range with the existing ones
        'outer: loop {
            for i in 0..self.len() {
                let current = &self.ranges[i];

                if range.overlaps(current).is_some() {
                    range.start = range.start.min(current.start);
                    range.end = range.end.max(current.end);
                    self.delete(i);
                    continue 'outer;
                }
            }

            break;
        }

        self.ranges[self.len()] = range;
        self.used += 1;
    }

    /// Only removes the specified range if it is present in the set or if it is a subset of an existing range.
    ///
    /// Returns the outer range that was removed, if any.
    pub fn try_remove(&mut self, range: MemoryRange) -> Option<MemoryRange> {
        for i in 0..self.len() {
            let current = self.ranges[i];

            // If the range is the same as the current one, we can remove it
            if range == current {
                self.delete(i);
                return Some(current);
            }

            // Else, check if subset and split the current range
            if !range.is_inside(&current) {
                continue;
            }

            if range.start == current.start {
                // current.end > current.start = range.start because of the `range == current` check
                self.ranges[i].start = range.end + 1;
            } else if range.end == current.end {
                // current.start < current.end = range.end because of the `range == current` check
                self.ranges[i].end = range.start - 1;
            } else {
                // Both comments above apply here
                self.ranges[i].end = range.start - 1;
                self.insert(MemoryRange::new(range.end + 1, current.end));
            }
            return Some(current);
        }

        None
    }

    /// Anihilates the specified range from the set, trimming other ranges if necessary.
    pub fn remove(&mut self, range: MemoryRange) {
        let mut i = 0;
        while i < self.len() {
            let current = self.ranges[i];

            if range.overlaps(&current).is_none() {
                i += 1;
                continue;
            }

            if current.is_inside(&range) {
                self.delete(i);
                break;
            }

            // Same statements as in `try_remove`
            if range.start <= current.start {
                self.ranges[i].start = range.end + 1;
            } else if range.end >= current.end {
                self.ranges[i].end = range.start - 1;
            } else {
                // `range` is strictly inside of `current`
                let old_end = self.ranges[i].end;
                self.ranges[i].end = range.start - 1;
                self.insert(MemoryRange::new(range.end + 1, old_end));
            }

            i += 1;
        }
    }

    #[must_use]
    #[inline]
    pub fn sum(&self) -> u64 {
        self.entries()
            .iter()
            .map(|range| range.end - range.start)
            .sum::<u64>()
    }

    #[must_use]
    #[inline]
    pub fn allocate<const M: usize>(
        &mut self,
        size: u64,
        alignment: u64,
        request: &MemoryRangeRequest<M>,
    ) -> Option<usize> {
        // 0-sized allocations are not allowed,
        // and alignment must be a power of 2 because of memory constraints
        if size == 0 || !alignment.is_power_of_two() {
            return None;
        }

        // Alignment is a power of 2, so alignment - 1 is all zeroes followed by ones
        let alignment_mask = alignment - 1;

        let mut allocation = None;
        for range in self.entries() {
            let alignment_offset = (alignment - (range.start & alignment_mask)) & alignment_mask;

            let start = range.start;
            let end = start.checked_add(size - 1)?.checked_add(alignment_offset)?;

            if end > range.end {
                continue;
            }

            match request {
                MemoryRangeRequest::MustBeWithin(req_ranges) => {
                    for req_range in req_ranges.entries() {
                        if let Some(overlap) = range.overlaps(req_range) {
                            let alignment_overlap =
                                (overlap.start.wrapping_add(alignment_mask)) & !alignment_mask;

                            if alignment_overlap >= overlap.start
                                && alignment_overlap <= overlap.end
                                && (overlap.end - alignment_overlap) >= (size - 1)
                            {
                                let overlap_end = alignment_overlap + (size - 1);

                                let prev_size = allocation.map(|(start, end, _)| end - start);

                                if allocation.is_none()
                                    || overlap_end - alignment_overlap < prev_size.unwrap()
                                {
                                    allocation = Some((
                                        alignment_overlap,
                                        overlap_end,
                                        usize::try_from(alignment_overlap).unwrap(),
                                    ));
                                }
                            }
                        }
                    }
                }
                MemoryRangeRequest::DontCare => {
                    let prev_size = allocation.map(|(start, end, _)| end - start);

                    if allocation.is_none() || end - start < prev_size.unwrap() {
                        allocation = Some((
                            start,
                            end,
                            usize::try_from(start + alignment_offset).unwrap(),
                        ));
                    }
                }
            }
        }

        allocation.map(|(start, end, addr)| {
            self.remove(MemoryRange::new(start, end));
            addr
        })
    }
}

#[non_exhaustive]
pub enum MemoryRangeRequest<'a, const N: usize> {
    DontCare,
    MustBeWithin(&'a MemoryRanges<N>),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_range() {
        let range = MemoryRange::new(0, 10);
        assert_eq!(range.start, 0);
        assert_eq!(range.end, 10);

        let range2 = MemoryRange::new(5, 15);
        assert_eq!(range2.start, 5);
        assert_eq!(range2.end, 15);

        let range3 = MemoryRange::new(6, 14);

        assert!(range.overlaps(&range2).is_some());
        assert!(range2.overlaps(&range).is_some());
        assert!(range3.is_inside(&range2));
    }

    #[test]
    fn test_memory_ranges() {
        let mut ranges = MemoryRanges::<10>::new();
        assert_eq!(ranges.len(), 0);

        ranges.insert(MemoryRange::new(0, 10));
        assert_eq!(ranges.len(), 1);

        ranges.insert(MemoryRange::new(5, 15));
        assert_eq!(ranges.len(), 1);

        ranges.insert(MemoryRange::new(20, 30));
        assert_eq!(ranges.len(), 2);

        ranges.insert(MemoryRange::new(25, 35));
        assert_eq!(ranges.len(), 2);

        ranges.insert(MemoryRange::new(40, 50));
        assert_eq!(ranges.len(), 3);

        ranges.insert(MemoryRange::new(45, 55));
        assert_eq!(ranges.len(), 3);

        ranges.insert(MemoryRange::new(60, 70));
        assert_eq!(ranges.len(), 4);

        ranges.insert(MemoryRange::new(65, 75));
        assert_eq!(ranges.len(), 4);

        ranges.insert(MemoryRange::new(80, 90));
        assert_eq!(ranges.len(), 5);

        ranges.insert(MemoryRange::new(85, 95));
        assert_eq!(ranges.len(), 5);

        ranges.insert(MemoryRange::new(100, 110));
        assert_eq!(ranges.len(), 6);

        ranges.insert(MemoryRange::new(105, 115));
        assert_eq!(ranges.len(), 6);
    }

    // FIXME: Write more tests
}
