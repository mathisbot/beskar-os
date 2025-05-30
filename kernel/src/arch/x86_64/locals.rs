use crate::process::scheduler::thread::Tls;
use beskar_core::arch::VirtAddr;
use beskar_hal::registers::{FS, GS};

#[cold]
pub fn store_locals(locals: &'static crate::locals::CoreLocalsInfo) {
    unsafe {
        GS::write_base(VirtAddr::from_ptr(core::ptr::from_ref(locals)));
    }
}

#[must_use]
#[inline]
/// Returns this core's local info.
pub fn load_locals() -> &'static crate::locals::CoreLocalsInfo {
    // Safety:
    // The GS register is set to point to the core's local info.
    unsafe { &*GS::read_base().as_ptr() }
}

#[inline]
/// Store the thread's local info.
pub fn store_thread_locals(tls: Tls) {
    unsafe { FS::write_base(tls.addr()) };
}
