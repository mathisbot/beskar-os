use crate::sys::sc_powermgt;

/// Shutdown the system.
///
/// This function only returns if the process does not have permission to shutdown the system.
pub fn shutdown() {
    sc_powermgt(beskar_core::syscall::consts::POWERMGT_SHUTDOWN);
}

/// Reboot the system.
///
/// This function only returns if the process does not have permission to reboot the system.
pub fn reboot() {
    sc_powermgt(beskar_core::syscall::consts::POWERMGT_REBOOT);
}
