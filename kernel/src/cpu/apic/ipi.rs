//! Inter-Processor Interrupts (IPIs)

use hyperdrive::volatile::Volatile;

use crate::cpu::interrupts::Irq;

/// Represents the delivery mode of an IPI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryMode {
    Fixed(Irq),
    /// This mode may not be available on all processors.
    LowestPriority(Irq),
    /// This IPI is mainly used by UEFI firmware to signal the processor to enter System Management Mode (SMM).
    ///
    /// For now, it shouldn't be used by the kernel.
    Smi,
    Nmi,
    /// This IPI should only be used once at the beginning of the processor's execution.
    Init,
    /// The `u8` contains the physical address of the payload.
    ///
    /// This IPI should only be used once at the beginning of the processor's execution.
    Sipi(u8),
}

/// Represents the destination of an IPI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Destination {
    /// The IPI is sent to all processors, including the current one.
    All,
    /// The IPI is sent to all processors except the current one.
    AllExcludingSelf,
    /// The IPI is sent to the specified processor.
    One(u8),
    /// The IPI is sent to the current processor.
    OneSelf,
}

/// Represents an Inter-Processor Interrupt (IPI).
pub struct Ipi {
    delivery_mode: DeliveryMode,
    destination: Destination,
}

/// Represents the raw format of an IPI.
struct RawIpi {
    low: u32,
    high: u32,
}

impl Ipi {
    #[must_use]
    #[inline]
    /// Creates a new IPI with the specified delivery mode and destination.
    pub const fn new(delivery_mode: DeliveryMode, destination: Destination) -> Self {
        Self {
            delivery_mode,
            destination,
        }
    }

    #[must_use]
    /// Converts the IPI to its raw format.
    fn to_raw(&self) -> RawIpi {
        let mut low = 0;
        let mut high = 0;

        low |= 1 << 14; // IPI assert bit

        let mode = match self.delivery_mode {
            DeliveryMode::Fixed(irq) => {
                low |= u32::from(irq as u8);
                0b000
            }
            DeliveryMode::LowestPriority(irq) => {
                low |= u32::from(irq as u8);
                0b001
            }
            DeliveryMode::Smi => 0b010,
            DeliveryMode::Nmi => 0b100,
            DeliveryMode::Init => 0b101,
            DeliveryMode::Sipi(payload) => {
                low |= u32::from(payload);
                0b110
            }
        };
        low |= mode << 8;

        let destination = match self.destination {
            Destination::All => 0b10,
            Destination::AllExcludingSelf => 0b11,
            Destination::One(cpu) => {
                high |= u32::from(cpu) << 24;
                0b00
            }
            Destination::OneSelf => 0b01,
        };
        low |= destination << 18;

        RawIpi { low, high }
    }

    /// Sends the IPI using the specified ICR registers.
    ///
    /// ## Safety
    ///
    /// ICR pointers must be valid.
    pub(super) unsafe fn send(&self, icr_low: Volatile<u32>, icr_high: Volatile<u32>) {
        // Assert Interrupts are ready to be received
        while unsafe { icr_low.read() >> 12 } & 1 == 1 {
            // FIXME: Fail with a timeout?
            core::hint::spin_loop();
        }

        let RawIpi { low, high } = self.to_raw();

        if let Destination::One(_) = self.destination {
            unsafe { icr_high.write(high) };
        }
        unsafe { icr_low.write(low) };
    }
}
