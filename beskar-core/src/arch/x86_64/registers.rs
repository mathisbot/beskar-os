use crate::arch::commons::{PhysAddr, paging::Frame};

pub struct Cr0;

impl Cr0 {
    pub const WRITE_PROTECT: u64 = 1 << 16;
    pub const CACHE_DISABLE: u64 = 1 << 30;

    #[must_use]
    #[inline]
    pub fn read() -> u64 {
        let value: u64;
        unsafe {
            core::arch::asm!("mov {}, cr0", out(reg) value, options(nomem, nostack, preserves_flags));
        }

        value
    }

    #[inline]
    /// ## Safety
    ///
    /// The value written must be a valid CR0 value.
    pub unsafe fn write(value: u64) {
        unsafe {
            core::arch::asm!("mov cr0, {}", in(reg) value, options(nomem, nostack, preserves_flags));
        }
    }

    #[inline]
    /// ## Safety
    ///
    /// The value written must be a valid CR0 flag.
    pub unsafe fn insert_flags(flag: u64) {
        let mut value = Self::read();
        value |= flag;
        unsafe { Self::write(value) };
    }
}

pub struct Cr4;

impl Cr4 {
    pub const FSGSBASE: u64 = 1 << 16;

    #[must_use]
    #[inline]
    pub fn read() -> u64 {
        let value: u64;
        unsafe {
            core::arch::asm!("mov {}, cr4", out(reg) value, options(nomem, nostack, preserves_flags));
        }

        value
    }

    #[inline]
    /// ## Safety
    ///
    /// The value written must be a valid CR4 value.
    pub unsafe fn write(value: u64) {
        unsafe {
            core::arch::asm!("mov cr4, {}", in(reg) value, options(nomem, nostack, preserves_flags));
        }
    }

    #[inline]
    /// ## Safety
    ///
    /// The value written must be a valid CR4 flag.
    pub unsafe fn insert_flags(flag: u64) {
        let mut value = Self::read();
        value |= flag;
        unsafe { Self::write(value) };
    }
}

pub struct Cr3;

impl Cr3 {
    #[must_use]
    #[inline]
    pub fn read() -> (Frame, u16) {
        let value: u64;

        unsafe {
            core::arch::asm!("mov {}, cr3", out(reg) value, options(nomem, nostack, preserves_flags));
        }

        let addr = PhysAddr::new(value & 0x_000f_ffff_ffff_f000);
        let frame = Frame::containing_address(addr);
        (frame, (value & 0xFFF) as u16)
    }

    #[inline]
    pub fn write(frame: Frame, flags: u16) {
        assert_eq!(frame.start_address().as_u64() & 0xFFF0_0000_0000_0FFF, 0);
        let value = frame.start_address().as_u64() | u64::from(flags);

        unsafe {
            core::arch::asm!("mov cr3, {}", in(reg) value, options(nomem, nostack, preserves_flags));
        }
    }
}

pub struct Efer;

impl Efer {
    pub const SYSTEM_CALL_EXTENSIONS: u64 = 1 << 0;
    pub const NO_EXECUTE_ENABLE: u64 = 1 << 11;

    #[must_use]
    #[inline]
    pub fn read() -> u64 {
        let low: u32;
        let high: u32;
        unsafe {
            core::arch::asm!("rdmsr", in("ecx") 0xC000_0080_u32, lateout("eax") low, lateout("edx") high, options(nomem, nostack, preserves_flags));
        }

        (u64::from(high) << 32) | u64::from(low)
    }

    #[inline]
    /// ## Safety
    ///
    /// The value written must be a valid EFER value.
    pub unsafe fn write(value: u64) {
        let low = u32::try_from(value & 0xFFFF_FFFF).unwrap();
        let high = u32::try_from(value >> 32).unwrap();

        unsafe {
            core::arch::asm!("wrmsr", in("ecx") 0xC000_0080_u32, in("eax") low, in("edx") high, options(nomem, nostack, preserves_flags));
        }
    }

    #[inline]
    /// ## Safety
    ///
    /// The value written must be a valid EFER flag.
    pub unsafe fn insert_flags(flag: u64) {
        let mut value = Self::read();
        value |= flag;
        unsafe { Self::write(value) };
    }
}

pub struct Rflags;

impl Rflags {
    pub const ID: u64 = 1 << 21;
    pub const IF: u64 = 1 << 9;
    pub const IOPL_LOW: u64 = 1 << 12;
    pub const IOPL_HIGH: u64 = 1 << 13;

    #[must_use]
    #[inline]
    pub fn read() -> u64 {
        let rf: u64;
        unsafe {
            core::arch::asm!("pushfq", "pop {}", lateout(reg) rf, options(nomem, preserves_flags));
        }
        rf
    }

    #[inline]
    /// ## Safety
    ///
    /// The value written must be a valid RFLAGS value.
    pub unsafe fn write(value: u64) {
        unsafe {
            core::arch::asm!("push {}", "popfq", in(reg) value, options(nomem, preserves_flags));
        }
    }

    #[inline]
    /// ## Safety
    ///
    /// The value written must be a valid RFLAGS flag.
    pub unsafe fn insert_flags(flag: u64) {
        let mut value = Self::read();
        value |= flag;
        unsafe { Self::write(value) };
    }
}
