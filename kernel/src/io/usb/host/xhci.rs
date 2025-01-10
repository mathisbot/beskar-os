// TODO: Remove
#![allow(dead_code)]

use core::ptr::NonNull;

use x86_64::{
    PhysAddr, VirtAddr,
    structures::paging::{PageTableFlags, Size4KiB},
};

use crate::mem::page_alloc::pmap::PhysicalMapping;
use hyperdrive::{
    locks::mcs::MUMcsLock,
    volatile::{ReadOnly, ReadWrite, Volatile, WriteOnly},
};

static XHCI: MUMcsLock<Xhci> = MUMcsLock::uninit();

pub fn init(mut xhci_paddrs: impl Iterator<Item = PhysAddr>) {
    // TODO: Support multiple xHCI controllers
    let Some(first_xhci_paddr) = xhci_paddrs.next() else {
        crate::warn!("No xHCI controller found");
        return;
    };

    let xhci = Xhci::new(first_xhci_paddr);
    xhci.reinitialize();
    XHCI.init(xhci);
}

#[derive(Debug)]
/// See <https://www.intel.com/content/dam/www/public/us/en/documents/technical-specifications/extensible-host-controler-interface-usb-xhci.pdf>
pub struct Xhci {
    cap: CapabilitiesRegisters,
    op1: OperationalRegisters,
    _physical_mapping: PhysicalMapping,
}

impl Xhci {
    #[must_use]
    pub fn new(paddr: PhysAddr) -> Self {
        let flags = PageTableFlags::PRESENT
            | PageTableFlags::WRITABLE
            | PageTableFlags::NO_EXECUTE
            | PageTableFlags::NO_CACHE;

        // At first, we only map enough memory to read the capabilities register
        let physical_mapping =
            PhysicalMapping::<Size4KiB>::new(paddr, CapabilitiesRegisters::MIN_LENGTH, flags);
        let vaddr = physical_mapping.translate(paddr).unwrap();

        let cap = CapabilitiesRegisters::new(vaddr);
        let cap_length = usize::from(cap.cap_length());

        let _ = (vaddr, cap); // We are about to unmap the memory, so it's best to shadow related variables

        // We can now map more memory to read the operational registers
        let physical_mapping =
            PhysicalMapping::new(paddr, cap_length + OperationalRegisters::LENGTH, flags);

        let reg_base_vaddr = physical_mapping.translate(paddr).unwrap();

        let cap = CapabilitiesRegisters::new(reg_base_vaddr);
        let op1 = OperationalRegisters::new(reg_base_vaddr + u64::try_from(cap_length).unwrap());

        Self {
            cap,
            op1,
            _physical_mapping: physical_mapping,
        }
    }

    pub fn reinitialize(&self) {
        self.op1.command().reset();
        self.op1.command().run_stop(true);
        assert!(self.op1.status().is_running());
        self.op1.command().set_interrupts(true);
        crate::debug!(
            "xHCI controller with version {} is ready",
            self.cap.hci_version()
        );
    }
}

#[derive(Debug)]
struct CapabilitiesRegisters {
    base: Volatile<ReadOnly, u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct HciVersion {
    major: u8,
    minor: u8,
}

impl core::fmt::Display for HciVersion {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "HCI {}.{}", self.major, self.minor)
    }
}

impl CapabilitiesRegisters {
    pub const MIN_LENGTH: usize = 0x20;

    const CAP_LENGTH: usize = 0x00;
    const HCI_VERSION: usize = 0x02;
    const HCS_PARAMS1: usize = 0x04;
    const HCS_PARAMS2: usize = 0x08;
    const HCS_PARAMS3: usize = 0x0C;
    const HCC_PARAMS1: usize = 0x10;
    const DBOFF: usize = 0x14;
    const RTSOFF: usize = 0x18;
    const HCC_PARAMS2: usize = 0x1C;

    #[must_use]
    pub const fn new(base: VirtAddr) -> Self {
        let base = Volatile::new(NonNull::new(base.as_mut_ptr()).unwrap());
        Self { base }
    }

    #[must_use]
    /// Offset of the first operational register from the base address
    pub fn cap_length(&self) -> u8 {
        unsafe { self.base.cast::<u8>().byte_add(Self::CAP_LENGTH).read() }
    }

    #[must_use]
    pub fn hci_version(&self) -> HciVersion {
        // unsafe { self.base.cast::<u16>().byte_add(Self::HCI_VERSION).read() }
        // There currently is a bug in QEMU, where xHCI registers do not support DWORD reads.
        // This is a workaround to read the register as a QWORD and extract the bytes.
        //
        // According to the xHCI specification, these fields should allow 1-4 bytes reads,
        // so it is safe to do so even on real hardware.
        let qword = unsafe { self.base.read() };
        let [_cap_len, _reserved, lo, hi] = qword.to_le_bytes();

        HciVersion {
            major: hi,
            minor: lo,
        }
    }

    #[must_use]
    pub fn hcs_params1(&self) -> u32 {
        unsafe { self.base.byte_add(Self::HCS_PARAMS1).read() }
    }

    #[must_use]
    pub fn hcs_params2(&self) -> u32 {
        unsafe { self.base.byte_add(Self::HCS_PARAMS2).read() }
    }

    #[must_use]
    pub fn hcs_params3(&self) -> u32 {
        unsafe { self.base.byte_add(Self::HCS_PARAMS3).read() }
    }

    #[must_use]
    pub fn hcc_params1(&self) -> u32 {
        unsafe { self.base.byte_add(Self::HCC_PARAMS1).read() }
    }

    #[must_use]
    /// Doorbell array offset
    pub fn dboff(&self) -> u32 {
        unsafe { self.base.byte_add(Self::DBOFF).read() }
    }

    #[must_use]
    /// Runtime register space offset
    pub fn rtsoff(&self) -> u32 {
        unsafe { self.base.byte_add(Self::RTSOFF).read() }
    }

    #[must_use]
    pub fn hcc_params2(&self) -> u32 {
        unsafe { self.base.byte_add(Self::HCC_PARAMS2).read() }
    }
}

#[derive(Debug)]
struct OperationalRegisters {
    base: Volatile<ReadWrite, u32>,
}

#[derive(Debug)]
struct StatusRegister(Volatile<ReadOnly, u32>);

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
    pub const fn as_raw(&self) -> Volatile<ReadOnly, u32> {
        self.0
    }

    #[must_use]
    pub fn is_running(&self) -> bool {
        unsafe { self.0.read() & Self::HALT == 0 }
    }

    #[must_use]
    pub fn host_system_error(&self) -> bool {
        unsafe { self.0.read() & Self::HOST_SYSTEM_ERROR != 0 }
    }

    #[must_use]
    pub fn event_interrupt(&self) -> bool {
        unsafe { self.0.read() & Self::EVENT_INTERRUPT != 0 }
    }

    #[must_use]
    pub fn port_change_detect(&self) -> bool {
        unsafe { self.0.read() & Self::PORT_CHANGE_DETECT != 0 }
    }

    #[must_use]
    pub fn save_state_status(&self) -> bool {
        unsafe { self.0.read() & Self::SAVE_STATE_STATUS != 0 }
    }

    #[must_use]
    pub fn restore_state_status(&self) -> bool {
        unsafe { self.0.read() & Self::RESTORE_STATE_STATUS != 0 }
    }

    #[must_use]
    pub fn save_restore_error(&self) -> bool {
        unsafe { self.0.read() & Self::SAVE_RESTORE_ERROR != 0 }
    }

    #[must_use]
    /// While the controller is not ready, "software shall not write any Doorbell or
    /// Operational register of the xHC, other than the USBSTS register"
    ///
    /// Intel xHCI specification rev. 1.2b, section 5.4.2
    pub fn controller_ready(&self) -> bool {
        unsafe { self.0.read() & Self::CONTROLLER_NOT_READY != 0 }
    }

    #[must_use]
    pub fn host_controller_error(&self) -> bool {
        unsafe { self.0.read() & Self::HOST_CONTROLLER_ERROR != 0 }
    }
}

#[derive(Debug)]
struct CommandRegister(Volatile<ReadWrite, u32>);

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
    pub const fn as_raw(&self) -> Volatile<WriteOnly, u32> {
        self.0.change_access()
    }

    #[must_use]
    #[inline]
    const fn update_bit_masked(value: u32, mask: u32, enable: bool) -> u32 {
        if enable { value | mask } else { value & !mask }
    }

    pub fn run_stop(&self, run: bool) {
        unsafe {
            self.0
                .update(|c| Self::update_bit_masked(c, Self::RUN_STOP, run));
        }
    }

    /// The function will block until the reset is complete.
    pub fn reset(&self) {
        unsafe {
            self.0
                .update(|c| Self::update_bit_masked(c, Self::RESET, true));
        }
        while unsafe { self.0.read() & Self::RESET != 0 } {
            core::hint::spin_loop();
        }
    }

    pub fn set_interrupts(&self, enable: bool) {
        unsafe {
            self.0
                .update(|c| Self::update_bit_masked(c, Self::INT_E, enable));
        }
    }

    pub fn set_host_system_error(&self, enable: bool) {
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
    pub unsafe fn light_reset(&self) {
        unsafe {
            self.0
                .update(|c| Self::update_bit_masked(c, Self::LIGHT_HC_RESET, true));
        }
    }

    /// Save the state of the controller.
    ///
    /// This function will stop the controller to perform the save operation.
    /// If the controller was previously running, it will be restarted after the operation.
    pub fn save_state(&self) {
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
    pub fn restore_state(&self) {
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

    pub fn set_wrap_event(&self, enable: bool) {
        unsafe {
            self.0
                .update(|c| Self::update_bit_masked(c, Self::E_WRAP_EVENT, enable));
        }
    }

    pub fn set_u3mfindex_stop(&self, enable: bool) {
        unsafe {
            self.0
                .update(|c| Self::update_bit_masked(c, Self::E_U3MFINDEX, enable));
        }
    }

    pub fn set_cem(&self, enable: bool) {
        unsafe {
            self.0
                .update(|c| Self::update_bit_masked(c, Self::CEM_E, enable));
        }
    }

    pub fn set_extended_tbc(&self, enable: bool) {
        unsafe {
            self.0
                .update(|c| Self::update_bit_masked(c, Self::EXTENDED_TBC_E, enable));
        }
    }

    pub fn set_extended_tbc_trb(&self, enable: bool) {
        unsafe {
            self.0
                .update(|c| Self::update_bit_masked(c, Self::EXTENDED_TBC_TRB_E, enable));
        }
    }

    pub fn set_vtioc(&self, enable: bool) {
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
    pub const fn command(&self) -> CommandRegister {
        CommandRegister(unsafe { self.base.byte_add(Self::COMMAND) })
    }

    #[must_use]
    pub const fn status(&self) -> StatusRegister {
        StatusRegister(unsafe { self.base.byte_add(Self::STATUS).change_access() })
    }

    #[must_use]
    /// If bit `i` is set, the controller supports a page size of 2^(12 + i) bytes.
    pub fn page_size(&self) -> u16 {
        // Bits 16-31 are reserved
        u16::try_from(unsafe { self.base.byte_add(Self::PAGE_SIZE).read() } & 0xFFFF).unwrap()
    }

    #[must_use]
    pub const fn dev_notification(&self) -> Volatile<ReadWrite, u16> {
        // Bits 16-31 are reserved
        unsafe { self.base.cast::<u16>().byte_add(Self::DEV_NOTIFICATION) }
    }

    #[must_use]
    pub const fn cmd_ring(&self) -> Volatile<WriteOnly, u64> {
        unsafe {
            self.base
                .cast::<u64>()
                .byte_add(Self::CMD_RING)
                .change_access()
        }
    }

    #[must_use]
    pub const fn dcbaap(&self) -> Volatile<ReadWrite, u64> {
        unsafe { self.base.cast::<u64>().byte_add(Self::DCBAAP) }
    }

    #[must_use]
    pub const fn configure(&self) -> Volatile<ReadWrite, u32> {
        unsafe { self.base.byte_add(Self::CONFIGURE) }
    }
}

#[inline]
pub fn with_xhci<T, F: FnOnce(&mut Xhci) -> T>(f: F) -> Option<T> {
    XHCI.try_with_locked(f)
}
