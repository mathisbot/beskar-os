//! Abstraction of physical and virtual addresses.
use core::ops::{Add, Sub};

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
/// A virtual address.
pub struct VirtAddr(u64);

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
/// A physical address.
pub struct PhysAddr(u64);

impl VirtAddr {
    pub const MAX: Self = Self(u64::MAX);
    pub const ZERO: Self = Self(0);

    #[must_use]
    #[inline]
    const fn canonicalize(raw: u64) -> u64 {
        ((raw << 16).cast_signed() >> 16).cast_unsigned()
    }

    #[must_use]
    #[track_caller]
    #[inline]
    pub const fn new(addr: u64) -> Self {
        Self::try_new(addr).expect("Invalid virtual address")
    }

    #[must_use]
    #[inline]
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
        // Safety: We made sure the address is canonical
        unsafe { Self::new_unchecked(Self::canonicalize(addr)) }
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
        let addr = (u64::from(p4_index & 0x1FF) << 39)
            | (u64::from(p3_index & 0x1FF) << 30)
            | (u64::from(p2_index & 0x1FF) << 21)
            | (u64::from(p1_index & 0x1FF) << 12)
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
    pub const fn align_down(self, align: u64) -> Self {
        assert!(align.is_power_of_two());
        Self::new_extend(self.0 & !(align - 1))
    }

    #[must_use]
    #[inline]
    pub const fn align_up(self, align: u64) -> Self {
        assert!(align.is_power_of_two());
        Self::new_extend((self.0.checked_add(align - 1).unwrap()) & !(align - 1))
    }

    #[must_use]
    #[inline]
    pub const fn is_aligned(self, align: u64) -> bool {
        assert!(align.is_power_of_two());
        self.0 & (align - 1) == 0
    }

    #[must_use]
    #[inline]
    pub fn p4_index(self) -> u16 {
        u16::try_from((self.0 >> 39) & 0x1FF).unwrap()
    }

    #[must_use]
    #[inline]
    pub fn p3_index(self) -> u16 {
        u16::try_from((self.0 >> 30) & 0x1FF).unwrap()
    }

    #[must_use]
    #[inline]
    pub fn p2_index(self) -> u16 {
        u16::try_from((self.0 >> 21) & 0x1FF).unwrap()
    }

    #[must_use]
    #[inline]
    pub fn p1_index(self) -> u16 {
        u16::try_from((self.0 >> 12) & 0x1FF).unwrap()
    }
}

impl PhysAddr {
    pub const MAX: Self = Self(0x000F_FFFF_FFFF_FFFF);
    pub const ZERO: Self = Self(0);

    #[must_use]
    #[track_caller]
    #[inline]
    pub const fn new(addr: u64) -> Self {
        Self::try_new(addr).expect("Invalid physical address")
    }

    #[must_use]
    #[inline]
    pub const fn try_new(addr: u64) -> Option<Self> {
        let truncated = Self::new_truncate(addr);
        if truncated.as_u64() != addr {
            return None;
        }
        Some(truncated)
    }

    #[must_use]
    #[inline]
    pub const fn new_truncate(addr: u64) -> Self {
        let truncated = addr & Self::MAX.0;
        // Safety: We just truncated the address to fit in the valid range
        unsafe { Self::new_unchecked(truncated) }
    }

    #[must_use]
    #[inline]
    /// # Safety
    ///
    /// The given address must be a valid physical address.
    pub const unsafe fn new_unchecked(addr: u64) -> Self {
        Self(addr)
    }

    #[must_use]
    #[inline]
    pub const fn as_u64(self) -> u64 {
        self.0
    }

    #[must_use]
    #[inline]
    pub const fn align_down(self, align: u64) -> Self {
        assert!(align.is_power_of_two());
        Self::new(self.0 & !(align - 1))
    }

    #[must_use]
    #[inline]
    pub const fn align_up(self, align: u64) -> Self {
        assert!(align.is_power_of_two());
        Self::new((self.0.checked_add(align - 1).unwrap()) & !(align - 1))
    }

    #[must_use]
    #[inline]
    pub const fn is_aligned(self, align: u64) -> bool {
        assert!(align.is_power_of_two());
        self.0 & (align - 1) == 0
    }
}

impl Add<u64> for VirtAddr {
    type Output = Self;

    #[inline]
    fn add(self, rhs: u64) -> Self {
        Self::new_extend(self.0.checked_add(rhs).unwrap())
    }
}

impl Sub<u64> for VirtAddr {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: u64) -> Self {
        Self::new_extend(self.0.checked_sub(rhs).unwrap())
    }
}

impl Sub<Self> for VirtAddr {
    type Output = u64;

    #[inline]
    fn sub(self, rhs: Self) -> u64 {
        self.0.checked_sub(rhs.0).unwrap()
    }
}

impl Add<u64> for PhysAddr {
    type Output = Self;

    #[inline]
    fn add(self, rhs: u64) -> Self {
        Self::new(self.0.checked_add(rhs).unwrap())
    }
}

impl Sub<u64> for PhysAddr {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: u64) -> Self {
        Self::new(self.0.checked_sub(rhs).unwrap())
    }
}

impl Sub<Self> for PhysAddr {
    type Output = u64;

    #[inline]
    fn sub(self, rhs: Self) -> u64 {
        self.0.checked_sub(rhs.0).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_p() {
        let addr = PhysAddr::new(0x18000031060);
        assert_eq!(addr.as_u64(), 0x18000031060);
    }

    #[test]
    #[should_panic = "Invalid physical address"]
    fn test_p_reject() {
        let _ = PhysAddr::new(0x1234567890ABCDEF);
    }

    #[test]
    fn test_p_align() {
        let addr = PhysAddr::new(0x18000031060);
        assert_eq!(addr.align_down(0x1000).as_u64(), 0x18000031000);
        assert_eq!(addr.align_up(0x1000).as_u64(), 0x18000032000);
        assert!(addr.is_aligned(0x20));
    }

    #[test]
    #[should_panic = "assertion failed: align.is_power_of_two()"]
    fn test_p_align_down_unaligned() {
        let addr = PhysAddr::new(0x18000031060);
        let _ = addr.align_down(0x1001);
    }

    #[test]
    #[should_panic = "called `Option::unwrap()` on a `None` value"]
    fn test_p_underflow() {
        let addr = PhysAddr::new(0x1000);
        let _ = addr - 0x1001;
    }

    #[test]
    fn test_v() {
        let addr = VirtAddr::new(0x18000031060);
        assert_eq!(addr.as_u64(), 0x18000031060);
    }

    #[test]
    fn test_v_from_ptr() {
        let x = 42u64;
        let addr = VirtAddr::from_ptr(&x);
        assert_eq!(addr.as_ptr(), core::ptr::from_ref(&x));
        assert!(addr.is_aligned(u64::try_from(core::mem::align_of::<u64>()).unwrap()));
    }

    #[test]
    fn test_v_from_idx() {
        let addr = VirtAddr::new(0x18000031060);
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
    #[should_panic = "Invalid virtual address"]
    fn test_v_reject() {
        let _ = VirtAddr::new(0x1234567890ABCDEF);
    }

    #[test]
    fn test_v_align() {
        let addr = VirtAddr::new(0x18000031060);
        assert_eq!(addr.align_down(0x1000).as_u64(), 0x18000031000);
        assert_eq!(addr.align_up(0x1000).as_u64(), 0x18000032000);
    }

    #[test]
    fn test_v_page_index() {
        let addr = VirtAddr::new(0x18000031060);
        assert_eq!(addr.p4_index(), 3);
        assert_eq!(addr.p3_index(), 0);
        assert_eq!(addr.p2_index(), 0);
        assert_eq!(addr.p1_index(), 49);
    }

    #[test]
    #[should_panic = "assertion failed: align.is_power_of_two()"]
    fn test_v_align_down_unaligned() {
        let addr = VirtAddr::new_extend(0x18000031060);
        let _ = addr.align_down(0x1001);
    }

    #[test]
    #[should_panic = "called `Option::unwrap()` on a `None` value"]
    fn test_v_underflow() {
        let addr = VirtAddr::new(0x1000);
        let _ = addr - 0x1001;
    }

    #[test]
    #[should_panic = "called `Option::unwrap()` on a `None` value"]
    fn test_v_overflow() {
        let addr = VirtAddr::new(0xFFFF_FFFF_FFFF_FFFF);
        let _ = addr + 1;
    }
}
