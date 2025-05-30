//! In order to be used in the `time` module,
//! Local APIC Timers must be a separate object
//! instead of being a method of the Local APIC.

use core::num::NonZeroU32;
use hyperdrive::ptrs::volatile::{ReadWrite, Volatile, WriteOnly};

const TIMER_DIVIDE_CONFIG_REG: usize = 0x3E0;
const TIMER_INIT_COUNT_REG: usize = 0x380;
const TIMER_CURR_COUNT_REG: usize = 0x390;
const TIMER_VECTOR_TABLE_REG: usize = 0x320;

const MASK_IRQ_DISABLE: u32 = 1 << 16;
const MASK_IRQ: u32 = 0xFF;
const MODE_MASK: u32 = 0b111 << 17;

pub struct LapicTimer {
    configuration: Configuration,
}

impl LapicTimer {
    #[must_use]
    #[inline]
    pub const fn new(configuration: Configuration) -> Self {
        Self { configuration }
    }

    #[must_use]
    pub const fn divider_config_reg(&mut self) -> Volatile<ReadWrite, u32> {
        unsafe {
            self.configuration
                .apic_base
                .byte_add(TIMER_DIVIDE_CONFIG_REG)
        }
    }

    #[must_use]
    pub const fn init_count_reg(&mut self) -> Volatile<WriteOnly, u32> {
        unsafe {
            self.configuration
                .apic_base
                .byte_add(TIMER_INIT_COUNT_REG)
                .change_access()
        }
    }

    #[must_use]
    pub fn read_curr_count_reg(&mut self) -> u32 {
        unsafe {
            self.configuration
                .apic_base
                .byte_add(TIMER_CURR_COUNT_REG)
                .read()
        }
    }

    #[must_use]
    pub const fn vector_table_reg(&mut self) -> Volatile<ReadWrite, u32> {
        unsafe {
            self.configuration
                .apic_base
                .byte_add(TIMER_VECTOR_TABLE_REG)
        }
    }

    /// Calibrate the timer using information from CPUID (Intel only).
    /// Returns the rate in MHz.
    fn calibrate_with_cpuid() -> Option<NonZeroU32> {
        use crate::arch::cpuid;

        let onboard = cpuid::check_feature(cpuid::CpuFeature::APIC_ONBOARD);
        let hsl = cpuid::get_highest_supported_leaf().as_u32();

        if onboard && hsl >= 0x15 {
            let core_crystal = cpuid::cpuid(cpuid::Leaf::new(0x15)).ecx;
            NonZeroU32::new(core_crystal / 1_000_000)
        } else if !onboard && hsl >= 0x16 {
            // Frequency is already given is MHz
            NonZeroU32::new(cpuid::cpuid(cpuid::Leaf::new(0x16)).ecx & 0xFFFF)
        } else {
            None
        }
    }

    fn calibrate_with_time(&mut self) -> Option<NonZeroU32> {
        self.set(Mode::OneShot(ModeConfiguration {
            divider: Divider::Two,
            duration: u32::MAX - 1,
        }));
        crate::time::wait(beskar_core::time::Duration::from_millis(50));
        let ticks = self.read_curr_count_reg();

        self.set(Mode::Inactive);

        let elapsed_ticks = (u32::MAX - 1) - ticks;

        let rate_mhz = u32::try_from(2 * 20 * u64::from(elapsed_ticks) / 1_000_000).unwrap();
        if rate_mhz == 0 {
            return None;
        }

        NonZeroU32::new(if rate_mhz > 14 {
            ((rate_mhz + 5) / 10) * 10
        } else {
            // Avoid 0
            10
        })
    }

    pub fn calibrate(&mut self) {
        if let Some(rate_mhz) = Self::calibrate_with_cpuid() {
            self.configuration.rate_mhz = rate_mhz.get();
        } else if let Some(rate_mhz) = self.calibrate_with_time() {
            self.configuration.rate_mhz = rate_mhz.get();
        } else {
            video::warn!("LAPIC timer calibration failed");
            return;
        }

        video::debug!(
            "LAPIC timer calibrated at {} MHz",
            self.configuration.rate_mhz
        );
    }

    /// Set the timer to a specific mode.
    pub fn set(&mut self, mode: Mode) {
        self.configuration.mode = mode;
        self.write_config();
    }

    fn write_config(&mut self) {
        match self.configuration.mode {
            Mode::Inactive => {
                let apic_timer_vte = self.vector_table_reg();
                let old_vte = unsafe { apic_timer_vte.read() };
                // Keep IRQ set but disable it
                let new_vte = old_vte | MASK_IRQ_DISABLE;
                unsafe { apic_timer_vte.write(new_vte) };

                unsafe { self.init_count_reg().write(0) };
            }
            Mode::OneShot(config) | Mode::Periodic(config) => {
                let apic_timer_divide = self.divider_config_reg();
                let old_divide = unsafe { apic_timer_divide.read() };
                let new_divide = (old_divide & !0xF) | config.divider as u32;
                unsafe { apic_timer_divide.write(new_divide) };

                let apic_timer_vt = self.vector_table_reg();
                let old_vte = unsafe { apic_timer_vt.read() };
                // Write IRQ and mode bits
                let new_vte = (old_vte & !(MASK_IRQ | MASK_IRQ_DISABLE | MODE_MASK))
                    | u32::from(self.configuration.ivt)
                    | (self.configuration.mode.as_vte_bits() << 17);
                unsafe { apic_timer_vt.write(new_vte) };

                unsafe { self.init_count_reg().write(config.duration) };
            }
            Mode::TscDeadline => {
                unimplemented!("TSC_DEADLINE is not supported");
            }
        }
    }

    #[must_use]
    pub const fn rate_mhz(&self) -> Option<NonZeroU32> {
        NonZeroU32::new(self.configuration.rate_mhz)
    }
}

#[derive(Debug, Clone)]
pub struct Configuration {
    apic_base: Volatile<ReadWrite, u32>,
    rate_mhz: u32,
    ivt: u8,
    mode: Mode,
}

impl Configuration {
    #[must_use]
    pub const fn new(apic_base: Volatile<ReadWrite, u32>, ivt: u8) -> Self {
        Self {
            apic_base,
            rate_mhz: 0,
            ivt,
            mode: Mode::Inactive,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Inactive,
    OneShot(ModeConfiguration),
    Periodic(ModeConfiguration),
    /// Only supported on newer CPUs.
    TscDeadline,
}

impl Default for Mode {
    fn default() -> Self {
        Self::Inactive
    }
}

impl Mode {
    fn as_vte_bits(&self) -> u32 {
        match self {
            Self::Inactive => panic!("Inactive mode has no VTE bits"),
            Self::OneShot(_) => 0b00,
            Self::Periodic(_) => 0b01,
            Self::TscDeadline => 0b10,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Divider {
    /// Untouched
    ///
    /// A divider of one is often unimplemented on hardware.
    One = 0b1011,
    /// Divide by 2
    Two = 0b0000,
    /// Divide by 4
    Four = 0b0001,
    /// Divide by 8
    Eight = 0b0010,
    /// Divide by 16
    Sixteen = 0b0011,
    /// Divide by 32
    ThirtyTwo = 0b1000,
    /// Divide by 64
    SixtyFour = 0b1001,
    /// Divide by 128
    OneTwentyEight = 0b1010,
}

impl Divider {
    #[must_use]
    pub const fn as_u32(self) -> u32 {
        match self {
            Self::One => 1,
            Self::Two => 2,
            Self::Four => 4,
            Self::Eight => 8,
            Self::Sixteen => 16,
            Self::ThirtyTwo => 32,
            Self::SixtyFour => 64,
            Self::OneTwentyEight => 128,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModeConfiguration {
    divider: Divider,
    duration: u32,
}

impl ModeConfiguration {
    #[must_use]
    #[inline]
    pub const fn new(divider: Divider, duration: u32) -> Self {
        Self { divider, duration }
    }
}
