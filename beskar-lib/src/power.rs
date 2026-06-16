use crate::sys::sc_powermgt;

/// Shutdown the system.
pub fn shutdown() -> ! {
    sc_powermgt(beskar_core::syscall::consts::POWERMGT_SHUTDOWN);
    unreachable!();
}

/// Reboot the system.
pub fn reboot() -> ! {
    sc_powermgt(beskar_core::syscall::consts::POWERMGT_REBOOT);
    unreachable!();
}
