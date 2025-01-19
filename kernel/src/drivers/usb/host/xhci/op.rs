use core::ptr::NonNull;

use crate::arch::commons::VirtAddr;
use hyperdrive::volatile::{ReadOnly, ReadWrite, Volatile, WriteOnly};

#[derive(Clone, Copy)]
pub struct OperationalRegisters {
    base: Volatile<ReadWrite, u32>,
}

#[derive(Debug, Clone, Copy)]
pub struct StatusRegister(Volatile<ReadOnly, u32>);

impl StatusRegister {
    const HALT: u32 = 1 << 0;
    const HOST_SYSTEM_ERROR: u32 = 1 << 2;
    const EVENT_INTERRUPT: u32 = 1 << 3;
    const PORT_CHANGE_DETECT: u32 = 1 << 4;
    const SAVE_STATE_STATUS: u32 = 1 << 8;
    const RESTORE_STATE_STATUS: u32 = 1 << 9;
    const SAVE_RESTORE_ERROR: u32 = 1 << 10;
    const CONTROLLER_NOT_READY: u32 = 1 << 11;
    const HOST_CONTROLLER_ERROR: u32 = 1 << 12;

    #[must_use]
    pub fn is_running(self) -> bool {
        unsafe { self.0.read() & Self::HALT == 0 }
    }

    #[must_use]
    pub fn host_system_error(self) -> bool {
        unsafe { self.0.read() & Self::HOST_SYSTEM_ERROR != 0 }
    }

    #[must_use]
    pub fn event_interrupt(self) -> bool {
        unsafe { self.0.read() & Self::EVENT_INTERRUPT != 0 }
    }

    #[must_use]
    pub fn port_change_detect(self) -> bool {
        unsafe { self.0.read() & Self::PORT_CHANGE_DETECT != 0 }
    }

    #[must_use]
    pub fn save_state_status(self) -> bool {
        unsafe { self.0.read() & Self::SAVE_STATE_STATUS != 0 }
    }

    #[must_use]
    pub fn restore_state_status(self) -> bool {
        unsafe { self.0.read() & Self::RESTORE_STATE_STATUS != 0 }
    }

    #[must_use]
    pub fn save_restore_error(self) -> bool {
        unsafe { self.0.read() & Self::SAVE_RESTORE_ERROR != 0 }
    }

    #[must_use]
    /// While the controller is not ready, software shall not write any Doorbell or
    /// Operational register of the xHC, other than the USBSTS register
    pub fn controller_ready(self) -> bool {
        unsafe { self.0.read() & Self::CONTROLLER_NOT_READY != 0 }
    }

    #[must_use]
    pub fn host_controller_error(self) -> bool {
        unsafe { self.0.read() & Self::HOST_CONTROLLER_ERROR != 0 }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CommandRegister(Volatile<ReadWrite, u32>);

impl CommandRegister {
    const RUN_STOP: u32 = 1 << 0;
    const RESET: u32 = 1 << 1;
    const INT_E: u32 = 1 << 2;
    const HSE_E: u32 = 1 << 3;
    const LIGHT_HC_RESET: u32 = 1 << 7;
    const CSS: u32 = 1 << 8;
    const CRS: u32 = 1 << 9;
    const E_WRAP_EVENT: u32 = 1 << 10;
    const E_U3MFINDEX: u32 = 1 << 11;
    const CEM_E: u32 = 1 << 13;
    const EXTENDED_TBC_E: u32 = 1 << 14;
    const EXTENDED_TBC_TRB_E: u32 = 1 << 15;
    const VTIO_E: u32 = 1 << 16;

    #[must_use]
    pub const fn as_raw(self) -> Volatile<WriteOnly, u32> {
        self.0.change_access()
    }

    #[must_use]
    #[inline]
    const fn update_bit_masked(value: u32, mask: u32, enable: bool) -> u32 {
        if enable { value | mask } else { value & !mask }
    }

    pub fn run_stop(self, run: bool) {
        unsafe {
            self.0
                .update(|c| Self::update_bit_masked(c, Self::RUN_STOP, run));
        }
    }

    /// The function will block until the reset is complete.
    pub fn reset(self) {
        unsafe {
            self.0
                .update(|c| Self::update_bit_masked(c, Self::RESET, true));
        }
        while unsafe { self.0.read() & Self::RESET != 0 } {
            core::hint::spin_loop();
        }
    }

    pub fn set_interrupts(self, enable: bool) {
        unsafe {
            self.0
                .update(|c| Self::update_bit_masked(c, Self::INT_E, enable));
        }
    }

    pub fn set_host_system_error(self, enable: bool) {
        unsafe {
            self.0
                .update(|c| Self::update_bit_masked(c, Self::HSE_E, enable));
        }
    }

    /// Performs a "light" reset, which is a reset that does not affect the state of the ports.
    ///
    /// ## Safety
    ///
    /// To perform such a reset, the Light HC Reset Capability bit must be set in HCCPARAMS1.
    pub unsafe fn light_reset(self) {
        unsafe {
            self.0
                .update(|c| Self::update_bit_masked(c, Self::LIGHT_HC_RESET, true));
        }
    }

    /// Save the state of the controller.
    ///
    /// This function will stop the controller to perform the save operation.
    /// If the controller was previously running, it will be restarted after the operation.
    pub fn save_state(self) {
        let was_running = unsafe { self.0.read() } & Self::RUN_STOP == 0;
        self.run_stop(false);
        unsafe {
            self.0
                .update(|c| Self::update_bit_masked(c, Self::CSS, true));
        }
        if was_running {
            self.run_stop(true);
        }
    }

    /// Restore the state of the controller.
    ///
    /// This function will stop the controller to perform the restore operation.
    /// If the controller was previously running, it will be restarted after the operation.
    pub fn restore_state(self) {
        let was_running = unsafe { self.0.read() } & Self::RUN_STOP == 0;
        self.run_stop(false);
        unsafe {
            self.0
                .update(|c| Self::update_bit_masked(c, Self::CRS, true));
        }
        if was_running {
            self.run_stop(true);
        }
    }

    pub fn set_wrap_event(self, enable: bool) {
        unsafe {
            self.0
                .update(|c| Self::update_bit_masked(c, Self::E_WRAP_EVENT, enable));
        }
    }

    pub fn set_u3mfindex_stop(self, enable: bool) {
        unsafe {
            self.0
                .update(|c| Self::update_bit_masked(c, Self::E_U3MFINDEX, enable));
        }
    }

    pub fn set_cem(self, enable: bool) {
        unsafe {
            self.0
                .update(|c| Self::update_bit_masked(c, Self::CEM_E, enable));
        }
    }

    pub fn set_extended_tbc(self, enable: bool) {
        unsafe {
            self.0
                .update(|c| Self::update_bit_masked(c, Self::EXTENDED_TBC_E, enable));
        }
    }

    pub fn set_extended_tbc_trb(self, enable: bool) {
        unsafe {
            self.0
                .update(|c| Self::update_bit_masked(c, Self::EXTENDED_TBC_TRB_E, enable));
        }
    }

    pub fn set_vtioc(self, enable: bool) {
        unsafe {
            self.0
                .update(|c| Self::update_bit_masked(c, Self::VTIO_E, enable));
        }
    }
}

impl OperationalRegisters {
    pub const LENGTH: usize = 0x3C;

    const COMMAND: usize = 0x00;
    const STATUS: usize = 0x04;
    const PAGE_SIZE: usize = 0x08;
    const DEV_NOTIFICATION: usize = 0x14;
    const CMD_RING: usize = 0x18;
    const DCBAAP: usize = 0x30;
    const CONFIGURE: usize = 0x38;

    #[must_use]
    pub const fn new(base: VirtAddr) -> Self {
        let base = Volatile::new(NonNull::new(base.as_mut_ptr()).unwrap());
        Self { base }
    }

    #[must_use]
    pub const fn command(self) -> CommandRegister {
        CommandRegister(unsafe { self.base.byte_add(Self::COMMAND) })
    }

    #[must_use]
    pub const fn status(self) -> StatusRegister {
        StatusRegister(unsafe { self.base.byte_add(Self::STATUS).change_access() })
    }

    #[must_use]
    /// If bit `i` is set, the controller supports a page size of 2^(12 + i) bytes.
    pub fn page_size(self) -> u16 {
        // Bits 16-31 are reserved
        u16::try_from(unsafe { self.base.byte_add(Self::PAGE_SIZE).read() } & 0xFFFF).unwrap()
    }

    #[must_use]
    pub const fn dev_notification(self) -> Volatile<ReadWrite, u32> {
        // Bits 16-31 are reserved but writes must be DWORDs
        unsafe { self.base.byte_add(Self::DEV_NOTIFICATION) }
    }

    #[must_use]
    pub const fn cmd_ring(self) -> Volatile<WriteOnly, u64> {
        unsafe {
            self.base
                .cast::<u64>()
                .byte_add(Self::CMD_RING)
                .change_access()
        }
    }

    #[must_use]
    pub const fn dcbaap(self) -> Volatile<ReadWrite, u64> {
        unsafe { self.base.cast::<u64>().byte_add(Self::DCBAAP) }
    }

    #[must_use]
    pub const fn configure(self) -> Volatile<ReadWrite, u32> {
        unsafe { self.base.byte_add(Self::CONFIGURE) }
    }
}
