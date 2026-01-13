use beskar_core::arch::VirtAddr;
use core::ptr::NonNull;
use driver_shared::mmio::MmioRegister;
use hyperdrive::ptrs::volatile::{ReadOnly, ReadWrite};

/// Port Register Set
///
/// Each port has a set of registers that control its operation.
#[derive(Clone, Copy)]
pub struct PortRegistersSet {
    base: MmioRegister<ReadWrite, u32>,
    max_ports: u8,
}

impl PortRegistersSet {
    /// Size of each port register set in bytes
    pub const PORT_REG_SIZE: usize = 0x10;

    #[must_use]
    pub const fn new(base: VirtAddr, max_ports: u8) -> Self {
        let base = MmioRegister::new(NonNull::new(base.as_mut_ptr()).unwrap());
        Self { base, max_ports }
    }

    #[must_use]
    pub fn port_regs(&self, port: u8) -> PortRegisters {
        assert!(port < self.max_ports, "Port index out of bounds");
        // Each port has 4 registers (16 bytes)
        let port_offset = usize::from(port) * Self::PORT_REG_SIZE / 4;
        let port_reg_vaddr = unsafe { self.base.add(port_offset) };
        PortRegisters {
            base: port_reg_vaddr,
        }
    }

    /// Get the maximum number of ports
    #[must_use]
    pub const fn max_ports(&self) -> u8 {
        self.max_ports
    }
}

/// Port Status and Control Register offsets
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortRegOffset {
    /// Port Status and Control
    StsCtrl = 0x0,
    /// Port Power Management Status and Control
    Pmsc = 0x4,
    /// Port Link Info
    LinkInfo = 0x8,
    /// Port Hardware LPM Control
    Hlpmc = 0xC,
}

/// Port Speed ID values
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortSpeed {
    /// Full Speed (12 Mbps)
    FullSpeed = 1,
    /// Low Speed (1.5 Mbps)
    LowSpeed = 2,
    /// High Speed (480 Mbps)
    HighSpeed = 3,
    /// SuperSpeed (5 Gbps)
    SuperSpeed = 4,
    /// SuperSpeedPlus (10 Gbps)
    SuperSpeedPlus = 5,
}

/// Port Status and Control Register
pub struct PortRegisters {
    base: MmioRegister<ReadWrite, u32>,
}

impl PortRegisters {
    #[must_use]
    #[inline]
    /// Get the raw register at the specified offset
    pub const fn reg_at_offset(&self, offset: PortRegOffset) -> MmioRegister<ReadWrite, u32> {
        unsafe { self.base.byte_add(offset as usize) }
    }

    #[must_use]
    #[inline]
    /// Get the Port Status and Control register
    pub const fn port_sc(&self) -> PortStatusControl {
        PortStatusControl(self.reg_at_offset(PortRegOffset::StsCtrl))
    }

    #[must_use]
    #[inline]
    /// Get the Port Power Management Status and Control register
    pub const fn port_pmsc(&self) -> MmioRegister<ReadWrite, u32> {
        self.reg_at_offset(PortRegOffset::Pmsc)
    }

    #[must_use]
    #[inline]
    /// Get the Port Link Info register
    pub const fn port_li(&self) -> MmioRegister<ReadOnly, u32> {
        self.reg_at_offset(PortRegOffset::LinkInfo).lower_access()
    }

    #[must_use]
    #[inline]
    /// Get the Port Hardware LPM Control register
    pub const fn port_hlpmc(&self) -> MmioRegister<ReadWrite, u32> {
        self.reg_at_offset(PortRegOffset::Hlpmc)
    }
}

/// Port Status and Control Register
pub struct PortStatusControl(MmioRegister<ReadWrite, u32>);

impl PortStatusControl {
    // Port Status bits
    const CURRENT_CONNECT_STATUS: u32 = 1 << 0;
    const PORT_ENABLED: u32 = 1 << 1;
    const PORT_RESET: u32 = 1 << 4;
    const PORT_POWER: u32 = 1 << 9;
    const PORT_SPEED_MASK: u32 = 0xF << 10;
    const PORT_SPEED_SHIFT: u32 = 10;

    // Port Status Change bits
    const CONNECT_STATUS_CHANGE: u32 = 1 << 17;
    const PORT_ENABLED_CHANGE: u32 = 1 << 18;
    const PORT_RESET_CHANGE: u32 = 1 << 21;

    #[must_use]
    #[inline]
    /// Read the raw register value
    pub fn read_raw(&self) -> u32 {
        unsafe { self.0.read() }
    }

    #[inline]
    /// Write the raw register value
    pub fn write_raw(&self, value: u32) {
        unsafe { self.0.write(value) }
    }

    #[must_use]
    #[inline]
    /// Check if a device is connected to the port
    pub fn connected(&self) -> bool {
        self.read_raw() & Self::CURRENT_CONNECT_STATUS != 0
    }

    #[must_use]
    #[inline]
    /// Check if the port is enabled
    pub fn enabled(&self) -> bool {
        self.read_raw() & Self::PORT_ENABLED != 0
    }

    #[must_use]
    #[inline]
    /// Check if the port is in reset
    pub fn in_reset(&self) -> bool {
        self.read_raw() & Self::PORT_RESET != 0
    }

    #[must_use]
    #[inline]
    /// Check if the port is powered
    pub fn powered(&self) -> bool {
        self.read_raw() & Self::PORT_POWER != 0
    }

    #[must_use]
    #[inline]
    /// Get the port speed
    pub fn speed(&self) -> Option<PortSpeed> {
        let speed_id = (self.read_raw() & Self::PORT_SPEED_MASK) >> Self::PORT_SPEED_SHIFT;
        match speed_id {
            1 => Some(PortSpeed::FullSpeed),
            2 => Some(PortSpeed::LowSpeed),
            3 => Some(PortSpeed::HighSpeed),
            4 => Some(PortSpeed::SuperSpeed),
            5 => Some(PortSpeed::SuperSpeedPlus),
            _ => None,
        }
    }

    #[must_use]
    #[inline]
    /// Check if the connect status has changed
    pub fn connect_status_change(&self) -> bool {
        self.read_raw() & Self::CONNECT_STATUS_CHANGE != 0
    }

    #[must_use]
    #[inline]
    /// Check if the port enabled status has changed
    pub fn port_enabled_change(&self) -> bool {
        self.read_raw() & Self::PORT_ENABLED_CHANGE != 0
    }

    #[must_use]
    #[inline]
    /// Check if the port reset status has changed
    pub fn port_reset_change(&self) -> bool {
        self.read_raw() & Self::PORT_RESET_CHANGE != 0
    }

    #[inline]
    /// Clear the connect status change bit
    pub fn clear_connect_status_change(&self) {
        self.write_raw(Self::CONNECT_STATUS_CHANGE);
    }

    #[inline]
    /// Clear the port enabled change bit
    pub fn clear_port_enabled_change(&self) {
        self.write_raw(Self::PORT_ENABLED_CHANGE);
    }

    #[inline]
    /// Clear the port reset change bit
    pub fn clear_port_reset_change(&self) {
        self.write_raw(Self::PORT_RESET_CHANGE);
    }

    #[inline]
    /// Clear all status change bits
    pub fn clear_all_change_bits(&self) {
        self.write_raw(
            Self::CONNECT_STATUS_CHANGE | Self::PORT_ENABLED_CHANGE | Self::PORT_RESET_CHANGE,
        );
    }

    #[inline]
    /// Power on the port
    pub fn power_on(&self) {
        let value = self.read_raw();
        self.write_raw((value & !Self::PORT_POWER) | Self::PORT_POWER);
    }

    #[inline]
    /// Power off the port
    pub fn power_off(&self) {
        let value = self.read_raw();
        self.write_raw(value & !Self::PORT_POWER);
    }

    #[inline]
    /// Reset the port
    pub fn reset(&self) {
        let value = self.read_raw();
        self.write_raw((value & !Self::PORT_RESET) | Self::PORT_RESET);
    }

    #[inline]
    /// Clear the port reset bit
    pub fn clear_reset(&self) {
        let value = self.read_raw();
        self.write_raw(value & !Self::PORT_RESET);
    }
}
