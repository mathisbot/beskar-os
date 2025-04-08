use crate::arch::commons::{PhysAddr, VirtAddr, paging::Frame};

pub struct Cr0;

impl Cr0 {
    pub const TASK_SWITCHED: u64 = 1 << 3;
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

pub struct Cr2;

impl Cr2 {
    #[must_use]
    #[inline]
    pub fn read() -> VirtAddr {
        let value: u64;
        unsafe {
            core::arch::asm!("mov {}, cr2", out(reg) value, options(nomem, nostack, preserves_flags));
        }
        VirtAddr::new(value)
    }
}

pub struct Cr3;

impl Cr3 {
    /// Use a writethrough caching policy
    /// (default to writeback).
    pub const CACHE_WRITETHROUGH: u16 = 1 << 3;
    /// Completely disable caching for the whole table.
    pub const CACHE_DISABLE: u16 = 1 << 4;

    #[must_use]
    #[inline]
    pub fn read_raw() -> u64 {
        let value: u64;
        unsafe {
            core::arch::asm!("mov {}, cr3", lateout(reg) value, options(nomem, nostack, preserves_flags));
        }
        value
    }

    #[must_use]
    pub fn read() -> (Frame, u16) {
        let value = Self::read_raw();
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

pub struct Cr4;

impl Cr4 {
    pub const TSD: u64 = 1 << 2;
    pub const PAE: u64 = 1 << 5;
    pub const OSFXSR: u64 = 1 << 9;
    pub const SMXE: u64 = 1 << 14;
    pub const FSGSBASE: u64 = 1 << 16;
    pub const PCIDE: u64 = 1 << 17;

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

pub struct Efer;

impl Efer {
    pub const SYSTEM_CALL_EXTENSIONS: u64 = 1 << 0;
    pub const NO_EXECUTE_ENABLE: u64 = 1 << 11;
    pub const TRANSLATION_CACHE_EXTENSION: u64 = 1 << 15;

    const MSR: Msr<0xC000_0080> = Msr;

    #[must_use]
    #[inline]
    pub fn read() -> u64 {
        Self::MSR.read()
    }

    #[inline]
    /// ## Safety
    ///
    /// The value written must be a valid EFER value.
    pub unsafe fn write(value: u64) {
        unsafe { Self::MSR.write(value) };
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

pub struct Star;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StarSelectors {
    cs_syscall: u16,
    ss_syscall: u16,
    cs_sysret: u16,
    ss_sysret: u16,
}

impl StarSelectors {
    #[must_use]
    #[inline]
    pub const fn new(cs_syscall: u16, ss_syscall: u16, cs_sysret: u16, ss_sysret: u16) -> Self {
        Self {
            cs_syscall,
            ss_syscall,
            cs_sysret,
            ss_sysret,
        }
    }

    #[must_use]
    #[inline]
    pub const fn cs_syscall(&self) -> u16 {
        self.cs_syscall
    }

    #[must_use]
    #[inline]
    pub const fn ss_syscall(&self) -> u16 {
        self.ss_syscall
    }

    #[must_use]
    #[inline]
    pub const fn cs_sysret(&self) -> u16 {
        self.cs_sysret
    }

    #[must_use]
    #[inline]
    pub const fn ss_sysret(&self) -> u16 {
        self.ss_sysret
    }
}

impl Star {
    const MSR: Msr<0xC000_0081> = Msr;

    #[must_use]
    #[inline]
    pub fn read() -> StarSelectors {
        let raw = Self::MSR.read();
        let sysret_base = u16::try_from(raw >> 48).unwrap();
        let syscall_base = u16::try_from((raw >> 32) & 0xFFFF).unwrap();

        StarSelectors {
            cs_syscall: syscall_base,
            ss_syscall: syscall_base + 8,
            cs_sysret: sysret_base + 16,
            ss_sysret: sysret_base + 8,
        }
    }

    #[inline]
    /// ## Safety
    ///
    /// The values written must be valid STAR values.
    pub fn write(selectors: StarSelectors) {
        assert_eq!(selectors.cs_syscall() + 8, selectors.ss_syscall());
        assert_eq!(selectors.cs_sysret(), selectors.ss_sysret() + 8);

        let syscall_ring = selectors.ss_syscall() & 0b11;
        let sysret_ring = selectors.ss_sysret() & 0b11;

        assert_eq!(syscall_ring, 0, "Syscall selectors must be ring 0");
        assert_eq!(sysret_ring, 3, "Sysret selectors must be ring 3");

        let sysret_base = selectors.ss_sysret().checked_sub(8).unwrap();
        let syscall_base = selectors.cs_syscall();

        unsafe { Self::MSR.write(u64::from(sysret_base) << 48 | u64::from(syscall_base) << 32) };
    }
}

pub struct LStar;

impl LStar {
    const MSR: Msr<0xC000_0082> = Msr;

    #[inline]
    pub fn write(f: unsafe extern "sysv64" fn()) {
        unsafe { Self::MSR.write(u64::try_from(f as usize).unwrap()) };
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

pub struct SFMask;

impl SFMask {
    const MSR: Msr<0xC000_0084> = Msr;

    #[must_use]
    #[inline]
    pub fn read() -> u64 {
        Self::MSR.read()
    }

    #[inline]
    /// ## Safety
    ///
    /// The value written must be a valid SFMASK value.
    pub unsafe fn write(value: u64) {
        unsafe { Self::MSR.write(value) };
    }

    #[inline]
    /// ## Safety
    ///
    /// The value written must be a valid SFMASK flag.
    pub unsafe fn insert_flags(flag: u64) {
        let mut value = Self::read();
        value |= flag;
        unsafe { Self::write(value) };
    }
}

pub struct Msr<const P: u32>;

impl<const P: u32> Msr<P> {
    #[must_use]
    #[inline]
    pub fn read(&self) -> u64 {
        let low: u32;
        let high: u32;
        unsafe {
            core::arch::asm!(
                "rdmsr",
                in("ecx") P,
                lateout("eax") low,
                lateout("edx") high,
                options(nomem, nostack, preserves_flags)
            );
        }
        (u64::from(high) << 32) | u64::from(low)
    }

    #[inline]
    pub unsafe fn write(&self, value: u64) {
        let low = u32::try_from(value & 0xFFFF_FFFF).unwrap();
        let high = u32::try_from(value >> 32).unwrap();
        unsafe {
            core::arch::asm!(
                "wrmsr",
                in("ecx") P,
                in("eax") low,
                in("edx") high,
                options(nostack, preserves_flags)
            );
        }
    }
}

pub struct GS;

impl GS {
    const MSR: Msr<0xC000_0101> = Msr;

    #[must_use]
    #[inline]
    pub fn read_base() -> VirtAddr {
        let base = Self::MSR.read();
        VirtAddr::new(base)
    }

    #[inline]
    pub unsafe fn write_base(base: VirtAddr) {
        unsafe { Self::MSR.write(base.as_u64()) };
    }
}

pub struct FS;

impl FS {
    const MSR: Msr<0xC000_0100> = Msr;

    #[must_use]
    #[inline]
    pub fn read_base() -> VirtAddr {
        let base = Self::MSR.read();
        VirtAddr::new(base)
    }

    #[inline]
    pub unsafe fn write_base(base: VirtAddr) {
        unsafe { Self::MSR.write(base.as_u64()) };
    }
}
