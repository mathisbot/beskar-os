use x86_64::{
    VirtAddr,
    registers::{
        control::{Cr4, Cr4Flags},
        segmentation::{GS, Segment64},
    },
};

#[cold]
pub fn store_locals(locals: &crate::locals::CoreLocalsInfo) {
    unsafe {
        Cr4::update(|cr4| cr4.insert(Cr4Flags::FSGSBASE));
    }

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
