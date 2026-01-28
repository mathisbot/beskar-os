pub use crate::arch::locals::CoreLocalsInfo;
use core::sync::atomic::{AtomicUsize, Ordering};
use hyperdrive::once::Once;

/// Distributes core IDs
static CORE_ID: AtomicUsize = AtomicUsize::new(0);

/// This array holds the core locals for each core, so that it is accessible from any core.
static ALL_CORE_LOCALS: [Once<&'static CoreLocalsInfo>; 256] = [const { Once::uninit() }; 256];

pub fn init() {
    let core_id = CORE_ID.fetch_add(1, Ordering::Relaxed);

    let core_locals = crate::arch::locals::init(core_id);

    ALL_CORE_LOCALS[core_id].call_once(|| core_locals);
}

#[must_use]
#[inline]
/// Returns the number of currently active cores
pub fn core_count() -> usize {
    CORE_ID.load(core::sync::atomic::Ordering::Acquire)
}

#[must_use]
#[inline]
/// Returns a specific core local info
pub fn get_specific_core_locals(core_id: usize) -> Option<&'static CoreLocalsInfo> {
    ALL_CORE_LOCALS[core_id].get().copied()
}

#[must_use]
#[inline]
/// Returns this core's local info.
pub fn get_core_locals() -> &'static CoreLocalsInfo {
    crate::arch::locals::load_locals()
}

/// A macro returning this core's local info.
#[macro_export]
macro_rules! locals {
    () => {
        $crate::locals::get_core_locals()
    };
}
