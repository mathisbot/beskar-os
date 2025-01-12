use core::ptr::NonNull;

use hyperdrive::volatile::{ReadWrite, Volatile};
use x86_64::VirtAddr;

#[derive(Clone, Copy)]
pub struct DoorbellRegistersSet {
    base: Volatile<ReadWrite, u32>,
    max_slots: u8,
}

impl DoorbellRegistersSet {
    #[must_use]
    pub const fn new(base: VirtAddr, max_ports: u8) -> Self {
        let base = Volatile::new(NonNull::new(base.as_mut_ptr()).unwrap());
        Self { base, max_slots: max_ports }
    }

    #[must_use]
    pub fn db_reg(&self, slot: u8) -> DoorbellRegisters {
        assert!(slot < self.max_slots);
        let port_reg_vaddr = unsafe { self.base.add(usize::from(slot)) };
        DoorbellRegisters {
            base: port_reg_vaddr,
        }
    }
}

pub struct DoorbellRegisters {
    base: Volatile<ReadWrite, u32>,
}

impl DoorbellRegisters {
    // TODO: Better API
    #[must_use]
    pub const fn as_raw(&self) -> Volatile<ReadWrite, u32> {
        self.base
    }
}
