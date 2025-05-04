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
    #[must_use]
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
        #[expect(
            clippy::cast_sign_loss,
            clippy::cast_possible_wrap,
            reason = "Sign extension"
        )]
        // Perform sign extension
        let extended = ((addr << 16) as i64 >> 16) as u64;
        // Safety: We made sure the address is canonical
        unsafe { Self::new_unchecked(extended) }
    }

    #[must_use]
    #[inline]
    pub fn from_ptr<T: ?Sized>(ptr: *const T) -> Self {
        // Safety: pointers are always canonical addresses
        unsafe { Self::new_unchecked(ptr.cast::<()>() as u64) }
    }

    #[must_use]
    #[inline]
    /// # Safety
    ///
    /// The given address must be a canonical virtual address.
    pub const unsafe fn new_unchecked(addr: u64) -> Self {
        #[expect(
            clippy::cast_sign_loss,
            clippy::cast_possible_wrap,
            reason = "Sign extension"
        )]
        {
            debug_assert!(((addr << 16) as i64 >> 16) as u64 == addr);
        }
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
    pub const MAX_VALID: u64 = 0x000F_FFFF_FFFF_FFFF;

    #[must_use]
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
        let truncated = addr & Self::MAX_VALID;
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

impl Add<Self> for VirtAddr {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self {
        Self::new_extend(self.0.checked_add(rhs.0).unwrap())
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

impl Add<Self> for PhysAddr {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self {
        Self::new(self.0.checked_add(rhs.0).unwrap())
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
        let addr = PhysAddr::new(0x18000031060);
        assert_eq!(addr.as_u64(), 0x18000031060);
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
