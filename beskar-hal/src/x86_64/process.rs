use super::userspace::Ring;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
#[repr(u8)]
pub enum Kind {
    /// Vital process kind.
    /// On panic, the system will be halted.
    Kernel,
    /// Driver process kind.
    /// These are Ring 0 processes that are not vital for the system.
    Driver,
    /// User process kind.
    /// These are Ring 3 processes.
    User,
}

impl Kind {
    #[must_use]
    #[inline]
    pub const fn new_kernel() -> Self {
        Self::Kernel
    }

    #[must_use]
    #[inline]
    pub const fn new_driver() -> Self {
        Self::Driver
    }

    #[must_use]
    #[inline]
    pub const fn new_user() -> Self {
        Self::User
    }

    #[must_use]
    #[inline]
    pub const fn ring(&self) -> Ring {
        match self {
            Self::Kernel | Self::Driver => Ring::Kernel,
            Self::User => Ring::User,
        }
    }
}
