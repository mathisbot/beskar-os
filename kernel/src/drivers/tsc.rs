use crate::arch::cpuid;
use beskar_core::{
    arch::x86_64::port::{Port, ReadWrite, WriteOnly},
    drivers::{DriverError, DriverResult},
};
use core::sync::atomic::{AtomicU64, Ordering};

/// The TSC value at startup, when the TSC has been calibrated.
static STARTUP_TIME: AtomicU64 = AtomicU64::new(0);

/// The TSC frequency in MHz
///
/// The reason we are limiting to MHz is that the TSC
/// cannot provide a better resolution than that.
static TSC_MHZ: AtomicU64 = AtomicU64::new(0);

#[must_use]
#[inline]
fn read_tsc_fenced() -> u64 {
    #[cfg(target_arch = "x86_64")]
    unsafe {
        core::arch::x86_64::_mm_mfence();
        let tsc = core::arch::x86_64::_rdtsc();
        core::arch::x86_64::_mm_lfence();
        tsc
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        unimplemented!()
    }
}

#[must_use]
fn pit_period() -> f64 {
    const PIT_FREQUENCY: f64 = 1_193_182.0;
    const PIT_MAX_RELOAD: f64 = 65_535.0;
    const FINAL_PERIOD: f64 = PIT_MAX_RELOAD / PIT_FREQUENCY;

    let ctrl_reg = Port::<u8, WriteOnly>::new(0x43);
    let chan0_data = Port::<u8, ReadWrite>::new(0x40);

    unsafe {
        // Mode 0: Interrupt on terminal count
        ctrl_reg.write(0b0011_0000);

        // Set the reload value to 0xFFFF (65 535) to increase calibration precision.
        chan0_data.write(0xFF);
        chan0_data.write(0xFF);

        loop {
            // Issue read back command
            ctrl_reg.write(0b1110_0010);
            // Wait until the output is high (countdown finished)
            if chan0_data.read() >> 7 == 1 {
                break;
            }
        }
    }

    FINAL_PERIOD
}

fn calibrate_with_pit() -> bool {
    assert_eq!(TSC_MHZ.load(Ordering::Relaxed), 0);

    let start = read_tsc_fenced();
    let elapsed = pit_period();
    let end = read_tsc_fenced();

    let diff = u32::try_from(end - start).unwrap();
    // Round to the nearest 100MHz because of TSC limitations
    #[allow(clippy::cast_sign_loss)]
    #[allow(clippy::cast_possible_truncation)]
    let rate_mhz = (((f64::from(diff) / elapsed / 100_000_000.0) + 0.5) as u64) * 100;

    TSC_MHZ.store(rate_mhz, Ordering::Relaxed);

    rate_mhz != 0
}

/// Calibrate using CPUID, Intel only.
fn calibrate_with_rdtsc() -> bool {
    assert_eq!(TSC_MHZ.load(Ordering::Relaxed), 0);

    let highest_leaf = cpuid::get_highest_supported_leaf();
    if highest_leaf >= 0x15 {
        let cpuid_res = cpuid::cpuid(0x15);
        if cpuid_res.eax != 0 && cpuid_res.ebx != 0 && cpuid_res.ecx != 0 {
            let thc_hz =
                u64::from(cpuid_res.ecx) * u64::from(cpuid_res.ebx) / u64::from(cpuid_res.eax);
            TSC_MHZ.store(thc_hz / 1_000_000, Ordering::Relaxed);
            return true;
        }
    }
    false
}

fn calibrate_with_hpet() -> bool {
    const MS_PER_S: u64 = 1_000;
    const CALIBRATION_TIME_MS: u64 = 50;
    const HZ_PER_MHZ: u64 = 1_000_000;

    assert_eq!(TSC_MHZ.load(Ordering::Relaxed), 0);

    let Some(diff) = crate::drivers::hpet::try_with_hpet(|hpet| {
        let start = read_tsc_fenced();

        let hpet_end = hpet.main_counter_value().get_value()
            + crate::drivers::hpet::ticks_per_ms() * CALIBRATION_TIME_MS;
        while hpet.main_counter_value().get_value() < hpet_end {
            core::hint::spin_loop();
        }

        read_tsc_fenced() - start
    }) else {
        return false;
    };

    let rate_mhz = diff * (MS_PER_S / CALIBRATION_TIME_MS) / HZ_PER_MHZ;
    TSC_MHZ.store(rate_mhz, Ordering::Relaxed);

    true
}

pub fn init() -> DriverResult<()> {
    if !cfg!(target_arch = "x86_64") {
        return Err(DriverError::Absent);
    }

    if !cpuid::check_feature(cpuid::CpuFeature::TSC) {
        return Err(DriverError::Absent);
    }

    STARTUP_TIME.store(unsafe { core::arch::x86_64::_rdtsc() }, Ordering::Relaxed);

    if calibrate_with_rdtsc() || calibrate_with_hpet() || calibrate_with_pit() {
        crate::debug!("TSC calibration: {} MHz", TSC_MHZ.load(Ordering::Relaxed));
        Ok(())
    } else {
        Err(DriverError::Unknown)
    }
}

#[must_use]
#[inline]
pub fn main_counter_value() -> u64 {
    read_tsc_fenced()
}

#[must_use]
#[inline]
pub fn ticks_per_ms() -> u64 {
    const HZ_PER_MHZ: u64 = 1_000_000;
    const MS_PER_S: u64 = 1_000;
    TSC_MHZ.load(Ordering::Relaxed) * HZ_PER_MHZ / MS_PER_S
}
