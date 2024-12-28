use core::sync::atomic::AtomicU64;

use x86_64::instructions::port::{Port, PortWriteOnly};

use crate::cpu::cpuid;

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
    unsafe {
        core::arch::x86_64::_mm_mfence();
        let tsc = core::arch::x86_64::_rdtsc();
        core::arch::x86_64::_mm_lfence();
        tsc
    }
}

#[must_use]
fn calibrate_with_pit() -> f64 {
    let mut pit_ctrl = PortWriteOnly::<u8>::new(0x43);
    let mut pit_data = Port::<u8>::new(0x40);

    unsafe {
        // Set to mode 1 - hardware re-triggerable one-shot
        pit_ctrl.write(0b0011_0000);

        // Set the reload value to 0xFFFF (65 535) to increase calibration precision.
        pit_data.write(0xFF);
        pit_data.write(0xFF);

        loop {
            pit_ctrl.write(0b1110_0010);
            // Wait until the output is high (countdown finished)
            if pit_data.read() >> 7 == 1 {
                break;
            }
        }
    }

    65_535.0 / 1_193_182.0
}

pub fn calibrate() {
    assert!(
        cpuid::check_feature(cpuid::CpuFeature::TSC),
        "TSC not supported"
    );

    STARTUP_TIME.store(
        unsafe { core::arch::x86_64::_rdtsc() },
        core::sync::atomic::Ordering::Relaxed,
    );

    // If CPU supports it, use the RDTSC calibration (Intel only apparently)
    let highest_leaf = cpuid::get_highest_supported_leaf();
    if highest_leaf >= 0x15 {
        let cpuid_res = cpuid::cpuid(0x15);
        if cpuid_res.eax != 0 && cpuid_res.ebx != 0 && cpuid_res.ecx != 0 {
            let thc_hz =
                u64::from(cpuid_res.ecx) * u64::from(cpuid_res.ebx) / u64::from(cpuid_res.eax);
            crate::serdebug!("RDTSC calibration: {} MHz", thc_hz / 1_000_000);
            TSC_MHZ.store(thc_hz / 1_000_000, core::sync::atomic::Ordering::Relaxed);
            return;
        }
    } else {
        crate::serdebug!("CPU does not support RDTSC calibration");
    }

    // FIXME: PIT isn't guaranteed to be available on modern hardware
    // Maybe make sure the counter decrements ?

    // If the CPU doesn't support RDTSC calibration, manually calibrate it with PIT

    let start = read_tsc_fenced();
    let elapsed = calibrate_with_pit();
    let end = read_tsc_fenced();

    let diff = u32::try_from(end - start).unwrap();
    // Round to the nearest 100MHz because of TSC limitations
    #[allow(clippy::cast_sign_loss)]
    #[allow(clippy::cast_possible_truncation)]
    let rate_mhz = (((f64::from(diff) / elapsed / 100_000_000.0) + 0.5) as u64) * 100;

    crate::serdebug!("PIT TSC calibration: {} MHz", rate_mhz);

    TSC_MHZ.store(rate_mhz, core::sync::atomic::Ordering::Relaxed);
}

pub(super) fn wait_ms(count: u64) {
    let tsc_mhz = TSC_MHZ.load(core::sync::atomic::Ordering::Relaxed);

    // FIXME: Find a way to handle this
    assert_ne!(tsc_mhz, 0, "TSC not calibrated");

    let end = read_tsc_fenced() + (count * tsc_mhz * 1_000);

    while read_tsc_fenced() < end {
        core::hint::spin_loop();
    }
}

pub fn time_since_startup() -> core::time::Duration {
    let startup_time = STARTUP_TIME.load(core::sync::atomic::Ordering::Relaxed);

    // FIXME: Find a way to handle this
    assert_ne!(startup_time, 0, "TSC not calibrated");

    let ticks = read_tsc_fenced() - startup_time;
    let mhz = TSC_MHZ.load(core::sync::atomic::Ordering::Relaxed);

    let seconds = ticks / mhz / 1_000_000;
    let nanos = (ticks % (mhz * 1_000_000)) * 1_000 / mhz;

    core::time::Duration::new(seconds, u32::try_from(nanos).unwrap())
}
