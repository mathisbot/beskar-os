use core::ptr::NonNull;

use beskar_core::{arch::commons::VirtAddr, static_assert};
use hyperdrive::ptrs::volatile::{ReadOnly, ReadWrite, Volatile};

#[derive(Clone, Copy)]
pub struct RuntimeRegisters {
    microframe_idx: Volatile<ReadOnly, u32>,
    /// Interrupt registers base, offset by 0x20 from the RT base
    ir_base: Volatile<ReadWrite, InterruptRegisterSetSnapshot>,
}

impl RuntimeRegisters {
    #[must_use]
    #[inline]
    pub const fn new(base: VirtAddr) -> Self {
        let base = Volatile::new(NonNull::new(base.as_mut_ptr()).unwrap());
        let ir_base = unsafe { base.add(0x20) }.cast();
        Self {
            microframe_idx: base.change_access(),
            ir_base,
        }
    }

    #[must_use]
    #[inline]
    /// Current periodic frame index.
    ///
    /// This value is incremented every 125 microseconds.
    pub fn microframe_idx(&self) -> u16 {
        u16::try_from(unsafe { self.microframe_idx.read() & 0x3FFF }).unwrap()
    }

    #[must_use]
    #[inline]
    pub fn irs(&self, idx: u8) -> InterruptRegisters {
        InterruptRegisters::new(self.ir_base, idx)
    }
}

#[derive(Clone, Copy)]
/// Interrupt registers for a specific interrupter
///
/// Offset by 0x20 from the RT base plus the index of the interrupter.
pub struct InterruptRegisters {
    base: Volatile<ReadWrite, InterruptRegisterSetSnapshot>,
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
/// Value of the interrupt register set read at some point in time
pub struct InterruptRegisterSetSnapshot {
    /// Interrupt Management
    ///
    /// Enable, disable and detect interrupts from the xHC.
    ///
    /// Bit 0: Interrupt Pending
    /// Bit 1: Interrupt Enable
    /// Bits 2-31: Reserved
    iman: u32,
    /// Interrupt Moderation
    ///
    /// Control the interrupt moderation feature (throttle of interrupts).
    ///
    /// Bits 0-15: Number of 250ns intervals to wait before sending an interrupt.
    ///            Defaults to 4000 (1ms)
    /// Bits 16-31: Counter. Can be written to alter the time to wait.
    imod: u32,
    /// Event Ring Segment Table Size
    ///
    /// Bits 0-15: Number of valid ERST entires. Should be between 1 and ERST Max (HCSPARAMS2).
    ///            This value can be 0 for secondary interrupters.
    /// Bits 16-31: Reserved
    pub erstsz: u32,
    _reserved: u32,
    /// Event Ring Segment Table Base Address
    ///
    /// Bits 0-5: Reserved
    /// Bits 6-63: Address of the ERST.
    ///            For primary interrupters, can only be written to if HCHalt is set.
    pub erstba: u64,
    /// Event Ring Dequeue Pointer
    ///
    /// Software updates this pointer when it is finished processing an event.
    ///
    /// Bits 0-2: Low order 3 bits of the offset of the ERST entry which defines the ER segment
    ///           that the ER Dequeue Pointer resides in.
    /// Bit 3: Set to 1 when Interrupt Pending is set (IMAN).
    ///        Should be cleared when Dequeue Pointer is updated.
    pub erdp: u64,
}
static_assert!(size_of::<InterruptRegisterSetSnapshot>() == 32);

impl InterruptRegisters {
    #[must_use]
    #[inline]
    pub fn new(base: Volatile<ReadWrite, InterruptRegisterSetSnapshot>, index: u8) -> Self {
        Self {
            base: unsafe {
                base.add(usize::from(index) * size_of::<InterruptRegisterSetSnapshot>())
            },
        }
    }

    #[must_use]
    #[inline]
    pub fn read(self) -> InterruptRegisterSetSnapshot {
        unsafe { self.base.read() }
    }

    #[inline]
    pub fn write(self, value: InterruptRegisterSetSnapshot) {
        unsafe { self.base.write(value) }
    }
}

impl InterruptRegisterSetSnapshot {
    #[must_use]
    #[inline]
    pub const fn iman(&self) -> u32 {
        self.iman
    }

    #[must_use]
    #[inline]
    pub const fn imod(&self) -> u32 {
        self.imod
    }

    #[must_use]
    #[inline]
    pub const fn erstsz(&self) -> u32 {
        self.erstsz
    }

    #[must_use]
    #[inline]
    pub const fn erstba(&self) -> u64 {
        self.erstba
    }

    #[must_use]
    #[inline]
    pub const fn erdp(&self) -> u64 {
        self.erdp
    }

    #[inline]
    pub const fn set_interrupt_enable(&mut self, enable: bool) {
        if enable {
            self.iman |= 1 << 1;
        } else {
            self.iman &= !(1 << 1);
        }
    }
}
