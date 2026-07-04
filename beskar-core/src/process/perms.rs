use core::ops::{Add, AddAssign, BitAnd, BitOr, Sub, SubAssign};

type Bitmap = u8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
// Invariant: self.0 always has exactly one bit set.
pub struct Permission(Bitmap);

impl Permission {
    /// Ability to manage power, e.g. shut down the system.
    pub const POWER_MGMT: Self = Self(1 << 0);
    /// Ability to acces the Internet.
    pub const NET_CLIENT: Self = Self(1 << 1);
    /// Ability to run a network server.
    pub const NET_SERVER: Self = Self(1 << 2);
}

#[derive(Debug)]
pub struct Permissions(Bitmap);

impl Permissions {
    const ALL: Bitmap = 0x7;
    const US_ROOT: Bitmap = 0x7;

    #[must_use]
    #[inline]
    /// Creates a new `Permissions` instance with all permissions enabled.
    pub const fn all() -> Self {
        Self(Self::ALL)
    }

    #[must_use]
    #[inline]
    /// Creates a new `Permissions` instance with common userspace root permissions enabled.
    pub const fn us_root() -> Self {
        Self(Self::US_ROOT)
    }

    #[must_use]
    #[inline]
    pub const fn none() -> Self {
        Self(0)
    }

    #[must_use]
    #[inline]
    /// Creates a new `Permissions` with at most the permissions of `self` and `bitmap`.
    pub const fn inherit(&self, wanted: &Self) -> Self {
        Self(self.0 & wanted.0)
    }

    #[must_use]
    #[inline]
    /// Checks if the given permission(s) are set in the permissions bitmap.
    const fn has(&self, p: Permission) -> bool {
        (self.0 & p.0) != 0
    }

    // #[must_use]
    // #[inline]
    // /// Checks if all of the given permission(s) are set in the permissions bitmap.
    // const fn has_many(&self, p: &Permissions) -> bool {
    //     (self.0 & p.0) == p.0
    // }

    #[must_use]
    #[inline]
    /// Checks if the `POWER_MGMT` permission is set in the permissions bitmap.
    pub const fn power_mgmt(&self) -> bool {
        self.has(Permission::POWER_MGMT)
    }
}

impl From<Permission> for Permissions {
    fn from(p: Permission) -> Self {
        Self(p.0)
    }
}

impl BitOr for Permissions {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}
impl BitOr<Permission> for Permissions {
    type Output = Self;

    fn bitor(self, rhs: Permission) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}
impl BitOr<Permissions> for Permission {
    type Output = Permissions;

    fn bitor(self, rhs: Permissions) -> Self::Output {
        Permissions(self.0 | rhs.0)
    }
}
impl BitOr for Permission {
    type Output = Permissions;

    fn bitor(self, rhs: Self) -> Self::Output {
        Permissions(self.0 | rhs.0)
    }
}

impl BitAnd for Permissions {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        Self(self.0 & rhs.0)
    }
}
impl Sub<Permission> for Permissions {
    type Output = Self;

    fn sub(self, rhs: Permission) -> Self::Output {
        Self(self.0 & !rhs.0)
    }
}
impl SubAssign<Permission> for Permissions {
    fn sub_assign(&mut self, rhs: Permission) {
        self.0 &= !rhs.0;
    }
}

impl Add<Permission> for Permissions {
    type Output = Self;

    #[expect(clippy::suspicious_arithmetic_impl)]
    fn add(self, rhs: Permission) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}
impl AddAssign<Permission> for Permissions {
    #[expect(clippy::suspicious_op_assign_impl)]
    fn add_assign(&mut self, rhs: Permission) {
        self.0 |= rhs.0;
    }
}

impl TryFrom<u64> for Permissions {
    type Error = ();

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        if let Ok(value) = Bitmap::try_from(value)
            && value & !Self::ALL == 0
        {
            Ok(Self(value))
        } else {
            Err(())
        }
    }
}
impl From<Permissions> for u64 {
    fn from(p: Permissions) -> Self {
        Self::from(p.0)
    }
}
