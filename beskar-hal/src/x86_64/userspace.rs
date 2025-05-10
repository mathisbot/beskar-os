/// The ring of the CPU that the code is running in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ring {
    /// Ring 0 - Most privileged
    Kernel = 0,
    /// Ring 1 - Less privileged
    Driver = 1,
    /// Ring 2 - Less privileged
    Hypervisor = 2,
    /// Ring 3 - Least privileged
    User = 3,
}

impl Ring {
    #[must_use]
    #[inline]
    pub const fn as_u8(self) -> u8 {
        match self {
            Self::Kernel => 0,
            Self::Driver => 1,
            Self::Hypervisor => 2,
            Self::User => 3,
        }
    }

    #[must_use]
    #[inline]
    pub const fn from_u8(value: u8) -> Self {
        assert!(value <= 3);
        match value {
            0 => Self::Kernel,
            1 => Self::Driver,
            2 => Self::Hypervisor,
            3 => Self::User,
            _ => panic!("Invalid ring value"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ring() {
        assert_eq!(Ring::Kernel.as_u8(), 0);
        assert_eq!(Ring::Driver.as_u8(), 1);
        assert_eq!(Ring::Hypervisor.as_u8(), 2);
        assert_eq!(Ring::User.as_u8(), 3);

        assert_eq!(Ring::from_u8(0), Ring::Kernel);
        assert_eq!(Ring::from_u8(1), Ring::Driver);
        assert_eq!(Ring::from_u8(2), Ring::Hypervisor);
        assert_eq!(Ring::from_u8(3), Ring::User);
    }
}
