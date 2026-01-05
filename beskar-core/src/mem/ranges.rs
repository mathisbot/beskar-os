use crate::arch::Alignment;
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
    pub const fn new(start: u64, end: u64) -> Self {
        debug_assert!(start <= end, "Invalid range");
        Self { start, end }
    }

    #[must_use]
    #[inline]
    pub const fn overlaps(&self, other: &Self) -> Option<Self> {
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
                start: if self.start > other.start {
                    self.start
                } else {
                    other.start
                },
                end: if self.end < other.end {
                    self.end
                } else {
                    other.end
                },
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

    #[must_use]
    #[inline]
    pub const fn size(&self) -> u64 {
        self.end - self.start + 1
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
    #[must_use]
    #[inline]
    pub const fn new() -> Self {
        Self {
            ranges: [MemoryRange { start: 0, end: 0 }; N],
            used: 0,
        }
    }

    #[must_use]
    #[inline]
    pub fn entries(&self) -> &[MemoryRange] {
        &self.ranges[..self.used]
    }

    #[must_use]
    #[inline]
    pub const fn len(&self) -> usize {
        self.used
    }

    #[must_use]
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.used == 0
    }

    fn delete(&mut self, index: usize) {
        debug_assert!(index < self.used, "Index out of bounds");

        if index < self.used {
            // Note that self.used is not 0
            self.used -= 1;
            self.ranges.swap(index, self.used);
        }
    }

    fn create(&mut self, range: MemoryRange) {
        debug_assert!(self.used < N, "MemoryRanges is full");

        if self.used < N {
            self.ranges[self.used] = range;
            self.used += 1;
        }
    }

    pub fn insert(&mut self, mut range: MemoryRange) {
        // Early return for zero-sized ranges
        if range.start > range.end {
            return;
        }

        // Try merging the new range with existing ones
        let mut i = 0;
        while i < self.used {
            let current = self.ranges[i];

            // Check for overlap or adjacency
            if range.start <= current.end.saturating_add(1)
                && current.start <= range.end.saturating_add(1)
            {
                // Merge ranges
                range.start = range.start.min(current.start);
                range.end = range.end.max(current.end);

                self.delete(i);
                // Don't increment i to check the swapped element
            } else {
                i += 1;
            }
        }

        self.create(range);
    }

    /// Only removes the specified range if it is present in the set or if it is a subset of an existing range.
    ///
    /// Returns the outer range that was removed, if any.
    pub fn try_remove(&mut self, range: MemoryRange) -> Option<MemoryRange> {
        for i in 0..self.used {
            let current = self.ranges[i];

            // Exact match, remove it
            if range == current {
                self.delete(i);
                return Some(current);
            }

            // Check if it's a subset
            if !range.is_inside(&current) {
                continue;
            }

            // Subset found, split or trim
            if range.start == current.start {
                // current.end > current.start = range.start because of the `range == current` check
                self.ranges[i].start = range.end + 1;
            } else if range.end == current.end {
                // current.start < current.end = range.end because of the `range == current` check
                self.ranges[i].end = range.start - 1;
            } else {
                // Both comments above apply here
                self.ranges[i].end = range.start - 1;
                self.create(MemoryRange::new(range.end.saturating_add(1), current.end));
            }
            return Some(current);
        }

        None
    }

    /// Anihilates the specified range from the set, trimming other ranges if necessary.
    pub fn remove(&mut self, range: MemoryRange) {
        let mut i = 0;
        while i < self.used {
            let current = &mut self.ranges[i];

            if range.overlaps(current).is_none() {
                i += 1;
                continue;
            }

            if current.is_inside(&range) {
                self.delete(i);
                break;
            }

            // Same statements as in `try_remove`
            if range.start <= current.start {
                current.start = range.end + 1;
            } else if range.end >= current.end {
                current.end = range.start - 1;
            } else {
                // `range` is strictly inside of `current`
                let old_end = current.end;
                current.end = range.start - 1;

                // Insert the second part
                self.create(MemoryRange::new(range.end.saturating_add(1), old_end));
                break;
            }
            i += 1;
        }
    }

    #[must_use]
    #[inline]
    pub fn sum(&self) -> u64 {
        self.entries()
            .iter()
            .map(|range| range.end - range.start + 1)
            .sum::<u64>()
    }

    #[must_use]
    #[inline]
    pub fn allocate(&mut self, size: u64, alignment: Alignment) -> Option<u64> {
        if size == 0 {
            return None;
        }

        let alignment_mask = alignment.mask();
        let mut best_fit: Option<(u64, u64, u64)> = None;

        for range in self.entries() {
            // Calculate aligned start address
            let offset = range.start & alignment_mask;
            let alignment_offset = (alignment.as_u64() - offset) & alignment_mask;
            let Some(aligned_start) = range.start.checked_add(alignment_offset) else {
                continue;
            };

            // Check if allocation fits
            let Some(end) = aligned_start.checked_add(size - 1) else {
                continue;
            };
            if end > range.end {
                continue;
            }

            let waste = alignment_offset + (range.end - end);
            if best_fit.is_none_or(|(_, _, best_size)| waste < best_size) {
                best_fit = Some((aligned_start, end, waste));
            }
        }

        best_fit.map(|(start, end, _)| {
            self.remove(MemoryRange::new(start, end));
            start
        })
    }

    #[must_use]
    #[inline]
    pub fn allocate_req<const M: usize>(
        &mut self,
        size: u64,
        alignment: Alignment,
        req_ranges: &MemoryRanges<M>,
    ) -> Option<u64> {
        // Validate inputs
        if size == 0 {
            return None;
        }

        let alignment_mask = alignment.mask();
        let mut best_fit: Option<(u64, u64, u64)> = None;

        for range in &self.ranges[..self.used] {
            for req_range in &req_ranges.ranges[..req_ranges.used] {
                // Calculate overlap
                let overlap_start = range.start.max(req_range.start);
                let overlap_end = range.end.min(req_range.end);

                if overlap_start >= overlap_end {
                    continue;
                }

                // Calculate aligned start within overlap
                let offset = overlap_start & alignment_mask;
                let alignment_offset = (alignment.as_u64() - offset) & alignment_mask;
                let aligned_start = match overlap_start.checked_add(alignment_offset) {
                    Some(a) if a <= overlap_end => a,
                    _ => continue,
                };

                // Check if allocation fits
                let end = match aligned_start.checked_add(size - 1) {
                    Some(e) if e <= overlap_end => e,
                    _ => continue,
                };

                let waste = alignment_offset + (range.end - end);
                if best_fit.is_none_or(|(_, _, best_size)| waste < best_size) {
                    best_fit = Some((aligned_start, end, waste));
                }
            }
        }

        best_fit.map(|(start, end, _)| {
            self.remove(MemoryRange::new(start, end));
            start
        })
    }

    #[must_use]
    pub fn intersection(&self, other: &Self) -> Self {
        let mut result = Self::new();

        for range in self.entries() {
            for other_range in other.entries() {
                if let Some(overlap) = range.overlaps(other_range) {
                    result.insert(overlap);
                }
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_range_new() {
        let range = MemoryRange::new(0, 10);
        assert_eq!(range.start(), 0);
        assert_eq!(range.end(), 10);
        assert_eq!(range.size(), 11);
    }

    #[test]
    fn test_memory_range_overlaps() {
        let range1 = MemoryRange::new(0, 10);
        let range2 = MemoryRange::new(5, 15);
        let range3 = MemoryRange::new(20, 30);

        assert_eq!(range1.overlaps(&range2), Some(MemoryRange::new(5, 10)));
        assert_eq!(range2.overlaps(&range1), Some(MemoryRange::new(5, 10)));
        assert_eq!(range1.overlaps(&range3), None);
    }

    #[test]
    fn test_memory_range_is_inside() {
        let outer = MemoryRange::new(0, 20);
        let inner = MemoryRange::new(5, 15);
        let partial = MemoryRange::new(10, 25);

        assert!(inner.is_inside(&outer));
        assert!(!outer.is_inside(&inner));
        assert!(!partial.is_inside(&outer));
    }

    #[test]
    fn test_memory_ranges_insert_merge() {
        let mut ranges = MemoryRanges::<10>::new();

        ranges.insert(MemoryRange::new(0, 10));
        assert_eq!(ranges.len(), 1);

        // Overlapping range should merge
        ranges.insert(MemoryRange::new(5, 15));
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0], MemoryRange::new(0, 15));

        // Adjacent range should merge
        ranges.insert(MemoryRange::new(16, 20));
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0], MemoryRange::new(0, 20));

        // Non-adjacent range
        ranges.insert(MemoryRange::new(30, 40));
        assert_eq!(ranges.len(), 2);
    }

    #[test]
    fn test_memory_ranges_remove() {
        let mut ranges = MemoryRanges::<10>::new();
        ranges.insert(MemoryRange::new(0, 100));

        // Remove from middle (split)
        ranges.remove(MemoryRange::new(40, 60));
        assert_eq!(ranges.len(), 2);

        // Remove from start
        ranges.remove(MemoryRange::new(0, 10));

        // Remove from end
        ranges.remove(MemoryRange::new(90, 100));
    }

    #[test]
    fn test_memory_ranges_try_remove() {
        let mut ranges = MemoryRanges::<10>::new();
        ranges.insert(MemoryRange::new(0, 100));

        // Remove exact range
        let removed = ranges.try_remove(MemoryRange::new(0, 100));
        assert_eq!(removed, Some(MemoryRange::new(0, 100)));
        assert_eq!(ranges.len(), 0);

        ranges.insert(MemoryRange::new(0, 100));

        // Remove subset
        let removed = ranges.try_remove(MemoryRange::new(40, 60));
        assert_eq!(removed, Some(MemoryRange::new(0, 100)));
        assert_eq!(ranges.len(), 2);

        // Try to remove non-existent
        let removed = ranges.try_remove(MemoryRange::new(200, 300));
        assert_eq!(removed, None);
    }

    #[test]
    fn test_memory_ranges_allocate() {
        let mut ranges = MemoryRanges::<10>::new();
        ranges.insert(MemoryRange::new(0, 100));
        ranges.insert(MemoryRange::new(200, 300));

        // Allocate with alignment
        let addr = ranges.allocate(10, Alignment::Align8);
        assert!(addr.is_some());
        let addr = addr.unwrap();
        assert_eq!(addr % 8, 0);

        // Allocate too large
        let addr = ranges.allocate(1000, Alignment::Align1);
        assert!(addr.is_none());
    }

    #[test]
    fn test_memory_ranges_allocate_req() {
        let mut ranges = MemoryRanges::<10>::new();
        ranges.insert(MemoryRange::new(0, 1000));

        let mut req_ranges = MemoryRanges::<5>::new();
        req_ranges.insert(MemoryRange::new(100, 200));
        req_ranges.insert(MemoryRange::new(500, 600));

        // Allocate in required range
        let addr = ranges.allocate_req(50, Alignment::Align8, &req_ranges);
        assert!(addr.is_some());
        let addr = addr.unwrap();
        assert!(addr >= 100 && addr <= 200 || addr >= 500 && addr <= 600);
        assert_eq!(addr % 8, 0);
    }

    #[test]
    fn test_memory_ranges_intersection() {
        let mut ranges1 = MemoryRanges::<10>::new();
        ranges1.insert(MemoryRange::new(0, 100));
        ranges1.insert(MemoryRange::new(200, 300));

        let mut ranges2 = MemoryRanges::<10>::new();
        ranges2.insert(MemoryRange::new(50, 150));
        ranges2.insert(MemoryRange::new(250, 350));

        let intersection = ranges1.intersection(&ranges2);
        assert_eq!(intersection.len(), 2);
    }

    #[test]
    fn test_memory_ranges_sum() {
        let mut ranges = MemoryRanges::<10>::new();
        ranges.insert(MemoryRange::new(0, 10));
        ranges.insert(MemoryRange::new(20, 30));

        // Size is inclusive: (10-0+1) + (30-20+1) = 11 + 11 = 22
        assert_eq!(ranges.sum(), 22);
    }

    #[test]
    fn test_edge_cases() {
        let mut ranges = MemoryRanges::<10>::new();

        // Single address range (start == end)
        ranges.insert(MemoryRange::new(10, 10));
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0].size(), 1);

        // Adjacent ranges should merge
        ranges.insert(MemoryRange::new(11, 20));
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0], MemoryRange::new(10, 20));
    }
}
