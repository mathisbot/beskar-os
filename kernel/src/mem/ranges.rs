use core::{
    cmp::Ordering,
    ops::{Index, IndexMut},
};

use beskar_core::mem::{MemoryRegion, MemoryRegionUsage};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
pub struct MemoryRanges<const N: usize> {
    /// Array of ranges
    ranges: [MemoryRange; N],
    /// Number of ranges that are currently in use
    used: u16,
}

impl<const N: usize> Index<usize> for MemoryRanges<N> {
    type Output = MemoryRange;

    fn index(&self, index: usize) -> &Self::Output {
        &self.ranges[index]
    }
}

impl<const N: usize> IndexMut<usize> for MemoryRanges<N> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.ranges[index]
    }
}

impl<const N: usize> Default for MemoryRanges<N> {
    fn default() -> Self {
        Self::new()
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
        &self.ranges[..usize::from(self.len())]
    }

    #[must_use]
    #[inline]
    pub const fn len(&self) -> u16 {
        self.used
    }

    fn delete(&mut self, index: usize) {
        assert!(index < usize::from(self.len()), "Index out of bounds");

        // The deleted range is put at the end of the array like a bubble
        for i in index..usize::from(self.len()) - 1 {
            self.ranges.swap(i, i + 1);
        }

        // Note that self.used is not 0 because of the assert above
        self.used -= 1;
    }

    pub fn insert(&mut self, mut range: MemoryRange) {
        if range.end == range.start {
            return;
        }

        assert!(
            self.used < u16::try_from(N).unwrap(),
            "MemoryRanges is full"
        );

        // Try merging the new range with the existing ones
        'outer: loop {
            for i in 0..usize::from(self.len()) {
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

        self.ranges[usize::from(self.len())] = range;
        self.used += 1;
    }

    /// Only removes the specified range if it is present in the set or if it is a subset of an existing range.
    ///
    /// Returns the outer range that was removed, if any.
    pub fn try_remove(&mut self, range: MemoryRange) -> Option<MemoryRange> {
        for i in 0..usize::from(self.len()) {
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
        while i < usize::from(self.len()) {
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
        if size == 0 || alignment.count_ones() != 1 {
            return None;
        }

        // Because alignment is a power of 2, alignment - 1 is all zeroes followed by ones
        let alignment_mask = alignment - 1;

        let mut allocation = None;
        for range in self.entries() {
            let alignment_offset = (alignment - (range.start & alignment_mask)) & alignment_mask;

            let start = range.start;
            let end = start.checked_add(size - 1)?.checked_add(alignment_offset)?;

            if end > range.end {
                continue;
            }

            if request.strength() >= MemoryRangeRequestStrength::HopefullyHere {
                // Try to find a suitable region in the range
                let req_ranges = match request {
                    MemoryRangeRequest::MustBeWithin(ranges) => ranges,
                    // MemoryRangeRequest::HopefullyHere(ranges) => ranges,
                    MemoryRangeRequest::DontCare => unreachable!(),
                };

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

            if request.strength() <= MemoryRangeRequestStrength::DontCare {
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

impl<const N: usize> MemoryRangeRequest<'_, N> {
    #[must_use]
    #[inline]
    const fn strength(&self) -> MemoryRangeRequestStrength {
        match self {
            MemoryRangeRequest::DontCare => MemoryRangeRequestStrength::DontCare,
            MemoryRangeRequest::MustBeWithin(_) => MemoryRangeRequestStrength::MustBeHere,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MemoryRangeRequestStrength {
    DontCare,
    HopefullyHere,
    MustBeHere,
}

impl PartialOrd for MemoryRangeRequestStrength {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        if self == other {
            return Some(Ordering::Equal);
        }
        match self {
            Self::DontCare => Some(Ordering::Less),
            Self::MustBeHere => Some(Ordering::Greater),
            Self::HopefullyHere => unreachable!(),
        }
    }
}
