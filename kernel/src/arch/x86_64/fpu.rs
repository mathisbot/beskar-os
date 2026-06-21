use crate::process::scheduler::thread::Thread;
use beskar_hal::structures::AvxSave;

#[derive(Debug, Clone)]
pub struct FpuState {
    state: AvxSave,
    mask: u64,
}

impl FpuState {
    #[must_use]
    #[inline]
    pub const fn new(xcr0: u64) -> Self {
        Self {
            state: AvxSave::new(),
            mask: xcr0,
        }
    }

    #[inline]
    /// Saves the current FPU state into this structure.
    ///
    /// # Safety
    ///
    /// The caller must ensure that the FPU is in a valid state.
    pub unsafe fn save(&mut self) {
        unsafe { beskar_hal::instructions::xsavec(&mut self.state, self.mask) };
    }

    #[inline]
    /// Restores the FPU state from this structure.
    ///
    /// If the state has not been initialized, initializes the FPU instead.
    ///
    /// # Safety
    ///
    /// The caller must ensure that the FPU can be safely restored or initialized.
    pub unsafe fn restore(&self) {
        unsafe { beskar_hal::instructions::xrstor(&self.state, self.mask) };
    }
}

#[inline]
pub fn on_thread_switch(thread: &mut Thread) {
    let core_locals = crate::locals!();
    let owner = core_locals.fpu_owner();
    let tid = thread.id().as_u64();

    if owner == tid {
        unsafe { thread.fpu_state_mut().save() };
        // If a thread gets scheduled on core 0, uses the FPU,
        // then gets scheduled on 1, uses the FPU then goes back to 0,
        // the FPU would not get restored without this line.
        core_locals.set_fpu_owner(0);
    }
}

/// Handles the Device Not Available (#NM) exception.
///
/// This is called when a thread tries to use the FPU/SSE but the TS bit in CR0 is set.
/// We save the previous thread's FPU state (if any) and restore the current thread's state.
///
/// # Safety
///
/// This function should only be called from the #NM exception handler.
pub unsafe fn handle_device_not_available() {
    let core_locals = crate::locals!();
    let owner = core_locals.fpu_owner();

    let ts = crate::process::scheduler::current_thread_snapshot();
    let tid = ts.id().as_u64();
    let fpu_ptr = ts.fpu_state();

    unsafe { beskar_hal::registers::Cr0::clear_ts() };

    if owner != tid {
        unsafe { (&*fpu_ptr).restore() };
        core_locals.set_fpu_owner(tid);
    }
}
