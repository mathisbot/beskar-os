use crate::drivers::acpi;
use ::acpi::sdt::AddressSpace;
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

    let address = reset_reg.register();
    match address.address_space() {
        AddressSpace::SystemIO => {
            let reset_port = u16::try_from(address.address()).unwrap();

            let port = port::Port::<u8, port::WriteOnly>::new(reset_port);
            let reset_value = reset_reg.value();
            unsafe { port.write(reset_value) };
        }
        AddressSpace::SystemMemory => {
            // TODO: Implement memory-mapped reboot
        }
        _ => unreachable!(),
    }

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

fn prepare_shutdown() {
    // TODO: Whatever is needed to prepare the system for shutdown, like waiting for IO completion, ...
}

/// Triggers a system shutdown.
///
/// # Safety
///
/// The caller must ensure it is safe to shut down the system.
pub unsafe fn shutdown() -> ! {
    prepare_shutdown();

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
    prepare_shutdown();

    acpi_reboot();

    legacy_reboot();

    loop {
        crate::arch::halt();
    }
}
