use crate::drivers::acpi;
use beskar_hal::port;

fn acpi_shutdown() {
    let acpi = acpi::acpi();

    let Some(aml) = acpi.dsdt().aml() else {
        crate::warn!("DSDT does not expose an _S5 package, cannot perform ACPI shutdown");
        return;
    };

    acpi.fadt()
        .pm1_cnt()
        .shutdown(aml.s5_sleep_type_a(), aml.s5_sleep_type_b());
}

fn acpi_reboot() {
    let acpi = acpi::acpi();

    let Some(reset_reg) = acpi.fadt().reset_reg() else {
        crate::warn!("ACPI reset register unavailable, falling back to legacy reboot");
        return;
    };

    let reset_port = reset_reg.reset_port();
    let port = port::Port::<u8, port::WriteOnly>::new(reset_port);

    let reset_value = reset_reg.value();

    unsafe { port.write(reset_value) };
    // Unreachable
}

fn legacy_reboot() {
    // Q35 reset control register.
    let reset_control = port::Port::<u8, port::WriteOnly>::new(0xCF9);
    unsafe {
        reset_control.write(0x02);
        reset_control.write(0x06);
    }
}

/// Triggers a system shutdown.
///
/// # Safety
///
/// The caller must ensure it is safe to shut down the system.
pub unsafe fn shutdown() -> ! {
    acpi_shutdown();

    loop {
        crate::arch::halt();
    }
}

/// Triggers a system reboot.
///
/// # Safety
///
/// The caller must ensure it is safe to reboot the system.
pub unsafe fn reboot() -> ! {
    acpi_reboot();

    legacy_reboot();

    loop {
        crate::arch::halt();
    }
}
