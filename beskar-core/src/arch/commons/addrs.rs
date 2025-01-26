//! Abstraction of physical and virtual addresses.
use core::ops::{Add, Sub};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// A virtual address.
pub struct VirtAddr(u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// A physical address.
pub struct PhysAddr(u64);

impl VirtAddr {
    #[must_use]
    #[inline]
    pub const fn new(addr: u64) -> Self {
        #[allow(clippy::cast_sign_loss, clippy::cast_possible_wrap)]
        // Perform sign extension
        let virt_addr = ((addr << 16) as i64 >> 16) as u64;
        assert!(virt_addr == addr);
        Self(virt_addr)
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
        Self::new(self.0 & !(align - 1))
    }

    #[must_use]
    #[inline]
    pub const fn align_up(self, align: u64) -> Self {
        assert!(align.is_power_of_two());
        Self::new((self.0 + (align - 1)) & !(align - 1))
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
    #[must_use]
    #[inline]
    pub const fn new(addr: u64) -> Self {
        let phys_addr = addr % (1 << 52);
        assert!(phys_addr == addr);
        Self(phys_addr)
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
        Self::new((self.0 + (align - 1)) & !(align - 1))
    }
}

impl Add<u64> for VirtAddr {
    type Output = Self;

    #[must_use]
    #[inline]
    fn add(self, rhs: u64) -> Self {
        Self::new(self.0 + rhs)
    }
}

impl Add<Self> for VirtAddr {
    type Output = Self;

    #[must_use]
    #[inline]
    fn add(self, rhs: Self) -> Self {
        Self::new(self.0 + rhs.0)
    }
}

impl Sub<u64> for VirtAddr {
    type Output = Self;

    #[must_use]
    #[inline]
    fn sub(self, rhs: u64) -> Self {
        Self::new(self.0 - rhs)
    }
}

impl Sub<Self> for VirtAddr {
    type Output = u64;

    #[must_use]
    #[inline]
    fn sub(self, rhs: Self) -> u64 {
        self.0 - rhs.0
    }
}

impl Add<u64> for PhysAddr {
    type Output = Self;

    #[must_use]
    #[inline]
    fn add(self, rhs: u64) -> Self {
        Self::new(self.0 + rhs)
    }
}

impl Add<Self> for PhysAddr {
    type Output = Self;

    #[must_use]
    #[inline]
    fn add(self, rhs: Self) -> Self {
        Self::new(self.0 + rhs.0)
    }
}

impl Sub<u64> for PhysAddr {
    type Output = Self;

    #[must_use]
    #[inline]
    fn sub(self, rhs: u64) -> Self {
        Self::new(self.0 - rhs)
    }
}

impl Sub<Self> for PhysAddr {
    type Output = u64;

    #[must_use]
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
        let addr = PhysAddr::new(0x18000031060);
        assert_eq!(addr.as_u64(), 0x18000031060);
    }

    #[test]
    #[should_panic = "assertion failed: phys_addr == addr"]
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
    fn test_v() {
        let addr = PhysAddr::new(0x18000031060);
        assert_eq!(addr.as_u64(), 0x18000031060);
    }

    #[test]
    #[should_panic = "assertion failed: virt_addr == addr"]
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
}
