type Bitmap = u8;

#[derive(Debug)]
pub struct Permissions {
    bitmap: Bitmap,
}

impl Permissions {
    const POWER_MGMT: Bitmap = 1 << 0;

    const ALL: Bitmap = 0x1;

    #[must_use]
    #[inline]
    /// Creates a new `Permissions` instance with all permissions enabled.
    pub const fn all() -> Self {
        Self { bitmap: Self::ALL }
    }

    #[must_use]
    #[inline]
    /// Creates a new `Permissions` instance with common userspace root permissions enabled.
    pub const fn us_root() -> Self {
        Self { bitmap: Self::ALL }
    }

    #[must_use]
    #[inline]
    pub const fn none() -> Self {
        Self { bitmap: 0 }
    }

    #[must_use]
    #[inline]
    /// Creates a new `Permissions` with at most the permissions of `self` and `bitmap`.
    pub const fn inherit(&self, bitmap: Bitmap) -> Self {
        Self {
            bitmap: self.bitmap & bitmap,
        }
    }

    #[must_use]
    #[inline]
    /// Checks if the given permission bit is set in the permissions bitmap.
    const fn has(&self, bit: Bitmap) -> bool {
        debug_assert!(bit.count_ones() <= 1);
        self.bitmap & bit != 0
    }

    #[must_use]
    #[inline]
    /// Checks if the `POWER_MGMT` permission is set in the permissions bitmap.
    pub const fn power_mgmt(&self) -> bool {
        self.has(Self::POWER_MGMT)
    }
}
