use beskar_hal::structures::SseSave;

#[derive(Debug, Clone)]
pub struct FpuState(Option<SseSave>);

impl Default for FpuState {
    fn default() -> Self {
        Self::new()
    }
}

impl FpuState {
    #[must_use]
    #[inline]
    pub const fn new() -> Self {
        Self(None)
    }

    #[inline]
    /// Saves the current FPU state into this structure.
    ///
    /// # Safety
    ///
    /// The caller must ensure that the FPU is in a valid state.
    pub unsafe fn save(&mut self) {
        let mut state = SseSave::new();
        unsafe { beskar_hal::instructions::fpu_save(&mut state) };
        self.0 = Some(state);
    }

    #[inline]
    /// Restores the FPU state from this structure.
    ///
    /// If the state has not been initialized, initializes the FPU instead.
    ///
    /// # Safety
    ///
    /// The caller must ensure that no other thread is using the FPU.
    pub unsafe fn restore(&self) {
        if let Some(state) = &self.0 {
            unsafe { beskar_hal::instructions::fpu_restore(state) };
        } else {
            unsafe { beskar_hal::instructions::fpu_init() };
        }
    }
}

#[expect(
    unreachable_code,
    reason = "FPU/SIMD state saving/restoring is not implemented yet"
)]
/// Handles the Device Not Available (#NM) exception.
///
/// This is called when a thread tries to use the FPU/SSE but the TS bit in CR0 is set.
/// We save the previous thread's FPU state (if any) and restore the current thread's state.
///
/// # Safety
///
/// This function should only be called from the #NM exception handler.
pub unsafe fn handle_device_not_available() {
    use beskar_hal::registers::Cr0;

    todo!("FPU/SIMD state saving/restoring");

    unsafe { Cr0::clear_ts() };
}
