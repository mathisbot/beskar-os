use beskar_core::arch::{
    commons::VirtAddr,
    x86_64::registers::{Cr4, GS},
};

#[cold]
pub fn store_locals(locals: &crate::locals::CoreLocalsInfo) {
    unsafe { Cr4::insert_flags(Cr4::FSGSBASE) };

    unsafe {
        GS::write_base(VirtAddr::new(core::ptr::from_ref(locals) as u64));
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
