use core::ptr::NonNull;

use beskar_core::arch::commons::VirtAddr;
use hyperdrive::ptrs::volatile::{Volatile, WriteOnly};

#[derive(Clone, Copy)]
pub struct DoorbellRegisters {
    base: Volatile<WriteOnly, u32>,
    max_ports: u8,
}

impl DoorbellRegisters {
    #[must_use]
    pub const fn new(base: VirtAddr, max_ports: u8) -> Self {
        let base = Volatile::new(NonNull::new(base.as_mut_ptr()).unwrap());
        Self { base, max_ports }
    }

    #[must_use]
    pub fn db_reg(&self, slot: u8) -> DoorbellRegister {
        assert!(slot < self.max_ports);
        let port_reg_vaddr = unsafe { self.base.add(usize::from(slot)) };
        DoorbellRegister {
            base: port_reg_vaddr,
        }
    }
}

pub struct DoorbellRegister {
    base: Volatile<WriteOnly, u32>,
}

impl DoorbellRegister {
    pub fn write(&self, value: u32) {
        // FIXME: Valid values depend on the context (p.395)
        // let dg_target = value & 0xFF;
        // assert!(dg_target <= 31 && dg_target >= 1);
        // assert!((value >> 8) & 0xFF == 0);
        // let db_stream_id = (value >> 16) & 0xFFFF;

        unsafe { self.base.write(value) };
    }
}
