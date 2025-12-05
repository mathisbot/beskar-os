//! Abstraction of physical and virtual addresses.
use core::ops::{Add, Sub};
use num_enum::{IntoPrimitive, TryFromPrimitive};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, IntoPrimitive, TryFromPrimitive)]
#[repr(u64)]
pub enum Alignment {
    Align1 = 1 << 0,
    Align2 = 1 << 1,
    Align4 = 1 << 2,
    Align8 = 1 << 3,
    Align16 = 1 << 4,
    Align32 = 1 << 5,
    Align64 = 1 << 6,
    Align128 = 1 << 7,
    Align256 = 1 << 8,
    Align512 = 1 << 9,
    Align1K = 1 << 10,
    Align2K = 1 << 11,
    Align4K = 1 << 12,
    Align8K = 1 << 13,
    Align16K = 1 << 14,
    Align32K = 1 << 15,
    Align64K = 1 << 16,
    Align128K = 1 << 17,
    Align256K = 1 << 18,
    Align512K = 1 << 19,
    Align1M = 1 << 20,
    Align2M = 1 << 21,
    Align4M = 1 << 22,
    Align8M = 1 << 23,
    Align16M = 1 << 24,
    Align32M = 1 << 25,
    Align64M = 1 << 26,
    Align128M = 1 << 27,
    Align256M = 1 << 28,
    Align512M = 1 << 29,
    Align1G = 1 << 30,
    Align2G = 1 << 31,
    Align4G = 1 << 32,
    Align8G = 1 << 33,
    Align16G = 1 << 34,
    Align32G = 1 << 35,
    Align64G = 1 << 36,
    Align128G = 1 << 37,
    Align256G = 1 << 38,
    Align512G = 1 << 39,
}

impl Alignment {
    #[must_use]
    #[inline]
    pub const fn of<T>() -> Self {
        // Safety: `align_of` always returns a power of two alignment.
        // `Self` is `repr(u64)`, so transmuting from `u64` is safe.
        unsafe { core::mem::transmute(core::mem::align_of::<T>() as u64) }
    }
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
/// A virtual address.
pub struct VirtAddr(u64);

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
/// A physical address.
pub struct PhysAddr(u64);

impl VirtAddr {
    /// Maximum valid virtual address.
    pub const MAX: Self = Self(u64::MAX);
    /// Maximum valid virtual address in the lower half.
    pub const MAX_LOWER_HALF: Self = Self(0x0000_7FFF_FFFF_FFFF);
    /// Minimum valid virtual address in the upper half.
    pub const MIN_UPPER_HALF: Self = Self(0xFFFF_8000_0000_0000);
    /// Zero virtual address.
    pub const ZERO: Self = Self(0);

    const IDX_4_OFFSET: u32 = 39;
    const IDX_3_OFFSET: u32 = 30;
    const IDX_2_OFFSET: u32 = 21;
    const IDX_1_OFFSET: u32 = 12;

    #[must_use]
    #[inline]
    /// Canonicalize a raw virtual address by sign-extending bit 47.
    const fn canonicalize(raw: u64) -> u64 {
        ((raw << 16).cast_signed() >> 16).cast_unsigned()
    }

    #[must_use]
    #[inline]
    /// Try to create a new valid virtual address.
    ///
    /// Returns `None` if the given address is not a canonical virtual address.
    pub const fn try_new(addr: u64) -> Option<Self> {
        let extended = Self::new_extend(addr);
        if extended.as_u64() != addr {
            return None;
        }
        Some(extended)
    }

    #[must_use]
    #[inline]
    /// Create a new valid virtual address by sign extending the address.
    pub const fn new_extend(addr: u64) -> Self {
        Self(Self::canonicalize(addr))
    }

    #[must_use]
    #[inline]
    pub fn from_ptr<T: ?Sized>(ptr: *const T) -> Self {
        // Safety: pointers are always canonical addresses
        unsafe { Self::new_unchecked(ptr.cast::<()>() as u64) }
    }

    #[must_use]
    #[inline]
    /// Create a new virtual address from page table indices.
    ///
    /// Indices will be truncated to 9 bits each, as per the x86-64 paging scheme.
    pub fn from_pt_indices(
        p4_index: u16,
        p3_index: u16,
        p2_index: u16,
        p1_index: u16,
        offset: u16,
    ) -> Self {
        let addr = (u64::from(p4_index & 0x1FF) << Self::IDX_4_OFFSET)
            | (u64::from(p3_index & 0x1FF) << Self::IDX_3_OFFSET)
            | (u64::from(p2_index & 0x1FF) << Self::IDX_2_OFFSET)
            | (u64::from(p1_index & 0x1FF) << Self::IDX_1_OFFSET)
            | u64::from(offset & 0xFFF);
        Self::new_extend(addr)
    }

    #[must_use]
    #[inline]
    /// # Safety
    ///
    /// The given address must be a canonical virtual address.
    pub const unsafe fn new_unchecked(addr: u64) -> Self {
        debug_assert!(Self::canonicalize(addr) == addr);
        Self(addr)
    }

    #[must_use]
    #[inline]
    pub const fn as_u64(self) -> u64 {
        self.0
    }

    #[must_use]
    #[inline]
    pub const fn as_ptr<T>(self) -> *const T {
        self.0 as _
    }

    #[must_use]
    #[inline]
    pub const fn as_mut_ptr<T>(self) -> *mut T {
        self.0 as _
    }

    #[must_use]
    #[inline]
    pub const fn aligned_down(self, align: Alignment) -> Self {
        Self(self.0 & !(align as u64 - 1))
    }

    #[must_use]
    #[inline]
    pub const fn aligned_up(self, align: Alignment) -> Self {
        let align_m1 = align as u64 - 1;
        let aligned_up = (self.0 + align_m1) & !align_m1;
        Self::new_extend(aligned_up)
    }

    #[must_use]
    #[inline]
    pub const fn is_aligned(self, align: Alignment) -> bool {
        self.0 & (align as u64 - 1) == 0
    }

    #[must_use]
    #[inline]
    pub fn p4_index(self) -> u16 {
        u16::try_from((self.0 >> Self::IDX_4_OFFSET) & 0x1FF).unwrap()
    }

    #[must_use]
    #[inline]
    pub fn p3_index(self) -> u16 {
        u16::try_from((self.0 >> Self::IDX_3_OFFSET) & 0x1FF).unwrap()
    }

    #[must_use]
    #[inline]
    pub fn p2_index(self) -> u16 {
        u16::try_from((self.0 >> Self::IDX_2_OFFSET) & 0x1FF).unwrap()
    }

    #[must_use]
    #[inline]
    pub fn p1_index(self) -> u16 {
        u16::try_from((self.0 >> Self::IDX_1_OFFSET) & 0x1FF).unwrap()
    }

    #[must_use]
    #[inline]
    pub fn page_offset(self) -> u16 {
        u16::try_from(self.0 & 0xFFF).unwrap()
    }
}

impl PhysAddr {
    /// Maximum valid physical address.
    pub const MAX: Self = Self(0x000F_FFFF_FFFF_FFFF);
    /// Zero physical address.
    pub const ZERO: Self = Self(0);

    #[must_use]
    #[inline]
    /// Try to create a new valid physical address.
    ///
    /// Returns `None` if the given address is not a valid physical address.
    pub const fn try_new(addr: u64) -> Option<Self> {
        let truncated = Self::new_truncate(addr);
        if truncated.as_u64() != addr {
            return None;
        }
        Some(truncated)
    }

    #[must_use]
    #[inline]
    /// Create a new physical address, truncating any bits beyond the maximum.
    pub const fn new_truncate(addr: u64) -> Self {
        Self(addr & Self::MAX.0)
    }

    #[must_use]
    #[inline]
    /// # Safety
    ///
    /// The given address must be a valid physical address.
    pub const unsafe fn new_unchecked(addr: u64) -> Self {
        debug_assert!(addr <= Self::MAX.0);
        Self(addr)
    }

    #[must_use]
    #[inline]
    /// Get the raw address as a `u64`.
    pub const fn as_u64(self) -> u64 {
        self.0
    }

    #[must_use]
    #[inline]
    pub const fn aligned_down(self, align: Alignment) -> Self {
        Self(self.0 & !(align as u64 - 1))
    }

    #[must_use]
    #[inline]
    pub const fn aligned_up(self, align: Alignment) -> Self {
        let align_m1 = align as u64 - 1;
        let aligned_up = (self.0 + align_m1) & !align_m1;
        Self::new_truncate(aligned_up)
    }

    #[must_use]
    #[inline]
    pub const fn is_aligned(self, align: Alignment) -> bool {
        self.0 & (align as u64 - 1) == 0
    }
}

impl Add<u64> for VirtAddr {
    type Output = Self;

    #[inline]
    fn add(self, rhs: u64) -> Self {
        Self::new_extend(self.0 + rhs)
    }
}

impl Sub<u64> for VirtAddr {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: u64) -> Self {
        Self::new_extend(self.0 - rhs)
    }
}

impl Sub<Self> for VirtAddr {
    type Output = u64;

    #[inline]
    fn sub(self, rhs: Self) -> u64 {
        self.0 - rhs.0
    }
}

impl Add<u64> for PhysAddr {
    type Output = Self;

    #[inline]
    fn add(self, rhs: u64) -> Self {
        Self::new_truncate(self.0 + rhs)
    }
}

impl Sub<u64> for PhysAddr {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: u64) -> Self {
        Self::new_truncate(self.0 - rhs)
    }
}

impl Sub<Self> for PhysAddr {
    type Output = u64;

    #[inline]
    fn sub(self, rhs: Self) -> u64 {
        self.0 - rhs.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_p() {
        let addr = PhysAddr::new_truncate(0x18000031060);
        assert_eq!(addr.as_u64(), 0x18000031060);
    }

    #[test]
    fn test_p_reject() {
        let paddr = PhysAddr::try_new(0x1234567890ABCDEF);
        assert!(paddr.is_none());
    }

    #[test]
    fn test_p_align() {
        let addr = PhysAddr::new_truncate(0x18000031060);
        assert_eq!(
            addr.aligned_down(Alignment::Align4K).as_u64(),
            0x18000031000
        );
        assert_eq!(addr.aligned_up(Alignment::Align4K).as_u64(), 0x18000032000);
        assert!(addr.is_aligned(Alignment::Align32));
    }

    #[test]
    fn test_v() {
        let addr = VirtAddr::new_extend(0x18000031060);
        assert_eq!(addr.as_u64(), 0x18000031060);
    }

    #[test]
    fn test_v_from_ptr() {
        let x = 42u64;
        let addr = VirtAddr::from_ptr(&x);
        assert_eq!(addr.as_ptr(), core::ptr::from_ref(&x));
        let alignment = Alignment::of::<u64>();
        assert!(addr.is_aligned(alignment));
    }

    #[test]
    fn test_v_from_idx() {
        let addr = VirtAddr::new_extend(0x18000031060);
        let p4_index = addr.p4_index();
        let p3_index = addr.p3_index();
        let p2_index = addr.p2_index();
        let p1_index = addr.p1_index();
        let offset = u16::try_from(addr.as_u64() & 0xFFF).unwrap();
        let same_addr = VirtAddr::from_pt_indices(p4_index, p3_index, p2_index, p1_index, offset);
        assert_eq!(addr, same_addr);
    }

    #[test]
    fn test_v_extends() {
        let addr = VirtAddr::new_extend(0xFFFF_FFFF_FFFF);
        assert_eq!(addr.as_u64(), 0xFFFF_FFFF_FFFF_FFFF);
        let addr = VirtAddr::new_extend(0x3FFF_FFFF_FFFF);
        assert_eq!(addr.as_u64(), 0x3FFF_FFFF_FFFF);
    }

    #[test]
    fn test_v_reject() {
        let vaddr = VirtAddr::try_new(0x1234567890ABCDEF);
        assert!(vaddr.is_none());
    }

    #[test]
    fn test_v_align() {
        let addr = VirtAddr::new_extend(0x18000031060);
        assert_eq!(
            addr.aligned_down(Alignment::Align4K).as_u64(),
            0x18000031000
        );
        assert_eq!(addr.aligned_up(Alignment::Align4K).as_u64(), 0x18000032000);
    }

    #[test]
    fn test_v_page_index() {
        let addr = VirtAddr::new_extend(0x18000031060);
        assert_eq!(addr.p4_index(), 3);
        assert_eq!(addr.p3_index(), 0);
        assert_eq!(addr.p2_index(), 0);
        assert_eq!(addr.p1_index(), 49);
    }
}
