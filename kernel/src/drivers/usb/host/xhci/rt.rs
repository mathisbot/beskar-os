use core::ptr::NonNull;

use beskar_core::arch::commons::VirtAddr;
use hyperdrive::volatile::{ReadOnly, Volatile};

#[derive(Clone, Copy)]
pub struct RuntimeRegisters {
    base: Volatile<ReadOnly, u32>,
}

impl RuntimeRegisters {
    #[must_use]
    pub const fn new(base: VirtAddr) -> Self {
        let base = Volatile::new(NonNull::new(base.as_mut_ptr()).unwrap());
        Self { base }
    }

    #[must_use]
    /// This value changes every 125 microseconds
    pub fn microframe_idx(self) -> u16 {
        u16::try_from(unsafe { self.base.read() & 0x3FFF }).unwrap()
    }

    // TODO: RT Regs
}
