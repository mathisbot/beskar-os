#![expect(dead_code, reason = "Some ACPI fields are not used yet")]
use crate::sdt::{AccessSize, AddressSpace, GenericAddress};
use beskar_core::static_assert;

#[derive(Debug, Clone, Copy)]
/// The PM1 Control Register contain the fixed hardware feature control bits.
/// These bits can be split between two registers:
/// - `PM1a` Control Register
/// - `PM1b` Control Register
///
/// The values for these pointers to the register space are found in the FADT.
/// Accesses to PM1 Control Registers are done through Byte/Word accesses.
///
/// Note that Address Space should be either System I/O or System Memory.
pub struct Pm1ControlRegister {
    pm1a: GenericAddress,
    pm1b: Option<GenericAddress>,
}

#[derive(Debug, Clone, Copy)]
/// ACPI reset port and value pair.
pub struct ResetRegister {
    port: u16,
    value: u8,
}

/// A value that can be written to the PM1 Control Register.
pub struct Pm1ControlValue {
    /// The raw value
    ///
    /// Bit 0: `SCI_EN` - Should be preserved
    /// Bit 1: `BM_RLD` - Bus Master Request pull C3 (sleep) CPUs to C0 (wake)
    /// Bit 2: `GBL_RLS` - Indicate a release of the global lock
    /// Bits 3-8: Reserved
    /// Bit 9: Ignored
    /// Bits 10-12: `SLP_TYPx` - Sleep Type
    /// Bit 13: `SLP_EN` - Sleep Enable
    /// Bits 14-15: Reserved
    raw: u16,
}
static_assert!(size_of::<Pm1ControlValue>() == 2);

impl ResetRegister {
    #[must_use]
    pub fn new(register: GenericAddress, value: u8) -> Self {
        debug_assert!(
            (register.address_space() == AddressSpace::SystemIO
                || register.address_space() == AddressSpace::SystemMemory)
                && register.bit_width() == 8
                && register.bit_offset() == 0
                && (register.access_size() == AccessSize::Byte
                    || register.access_size() == AccessSize::Undefined)
        );
        let port = u16::try_from(register.address()).unwrap();
        Self { port, value }
    }

    #[must_use]
    #[inline]
    pub const fn reset_port(self) -> u16 {
        self.port
    }

    #[must_use]
    #[inline]
    pub const fn value(self) -> u8 {
        self.value
    }
}

impl Pm1ControlRegister {
    #[must_use]
    #[inline]
    /// Create a new PM1 Control Register.
    pub fn new(pm1a: GenericAddress, pm1b: Option<GenericAddress>) -> Self {
        assert!(
            (pm1a.address_space() == AddressSpace::SystemIO
                || pm1a.address_space() == AddressSpace::SystemMemory)
                && (pm1a.access_size() == AccessSize::Word
                    || pm1a.access_size() == AccessSize::Byte)
                && pm1a.bit_width() == u8::try_from(8 * size_of::<Pm1ControlValue>()).unwrap()
        );

        assert!(
            pm1b.is_none()
                || ((pm1b.unwrap().address_space() == AddressSpace::SystemIO
                    || pm1b.unwrap().address_space() == AddressSpace::SystemMemory)
                    && (pm1b.unwrap().access_size() == AccessSize::Word
                        || pm1b.unwrap().access_size() == AccessSize::Byte)
                    && pm1b.unwrap().bit_width()
                        == u8::try_from(8 * size_of::<Pm1ControlValue>()).unwrap())
        );

        Self { pm1a, pm1b }
    }

    fn read_pm1a(&self) -> u16 {
        match self.pm1a.address_space() {
            AddressSpace::SystemIO => {
                use beskar_hal::port;
                let port = port::Port::<u16, port::ReadOnly>::new(
                    u16::try_from(self.pm1a.address()).unwrap(),
                );
                unsafe { port.read() }
            }
            AddressSpace::SystemMemory => {
                todo!("Read from System Memory");
            }
            _ => unreachable!("Invalid address space"),
        }
    }

    fn read_pm1b(&self) -> u16 {
        let pm1b = self.pm1b.unwrap();

        match pm1b.address_space() {
            AddressSpace::SystemIO => {
                use beskar_hal::port;
                let port =
                    port::Port::<u16, port::ReadOnly>::new(u16::try_from(pm1b.address()).unwrap());
                unsafe { port.read() }
            }
            AddressSpace::SystemMemory => {
                todo!("Read from System Memory");
            }
            _ => unreachable!("Invalid address space"),
        }
    }

    fn write_pm1a(&self, value: u16) {
        match self.pm1a.address_space() {
            AddressSpace::SystemIO => {
                use beskar_hal::port;
                let port = port::Port::<u16, port::WriteOnly>::new(
                    u16::try_from(self.pm1a.address()).unwrap(),
                );
                unsafe { port.write(value) }
            }
            AddressSpace::SystemMemory => {
                todo!("Write to System Memory");
            }
            _ => unreachable!("Invalid address space"),
        }
    }

    fn write_pm1b(&self, value: u16) {
        let pm1b = self.pm1b.unwrap();

        match pm1b.address_space() {
            AddressSpace::SystemIO => {
                use beskar_hal::port;
                let port =
                    port::Port::<u16, port::WriteOnly>::new(u16::try_from(pm1b.address()).unwrap());
                unsafe { port.write(value) }
            }
            AddressSpace::SystemMemory => {
                todo!("Write to System Memory");
            }
            _ => unreachable!("Invalid address space"),
        }
    }

    /// Triggers ACPI S5 shutdown through PM1 control registers.
    pub fn shutdown(&self, sleep_type_a: u8, sleep_type_b: u8) {
        // What a very nice name!
        const SLEEP_MASK: u16 = 0b0011_1100_0000_0000;

        assert!(
            sleep_type_a <= 0b111 && sleep_type_b <= 0b111,
            "ACPI SLP_TYPx must fit in 3 bits"
        );

        let prev_pm1a = self.read_pm1a() & !SLEEP_MASK;
        let pm1a_value = prev_pm1a | (u16::from(sleep_type_a) << 10) | (1 << 13);
        self.write_pm1a(pm1a_value);

        if self.pm1b.is_some() {
            let prev_pm1b = self.read_pm1b() & !SLEEP_MASK;
            let pm1b_value = prev_pm1b | (u16::from(sleep_type_b) << 10) | (1 << 13);
            self.write_pm1b(pm1b_value);
        }
    }
}
