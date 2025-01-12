use core::ptr::NonNull;

use hyperdrive::volatile::{ReadWrite, Volatile};
use x86_64::VirtAddr;

#[derive(Clone, Copy)]
pub struct PortRegistersSet {
    base: Volatile<ReadWrite, u32>,
    max_ports: u8,
}

impl PortRegistersSet {
    #[must_use]
    pub const fn new(base: VirtAddr, max_ports: u8) -> Self {
        let base = Volatile::new(NonNull::new(base.as_mut_ptr()).unwrap());
        Self { base, max_ports }
    }

    #[must_use]
    pub fn port_regs(&self, port: u8) -> PortRegisters {
        assert!(port < self.max_ports);
        let port_reg_vaddr = unsafe { self.base.add(usize::from(port)) };
        PortRegisters {
            base: port_reg_vaddr,
        }
    }
}

pub struct PortRegisters {
    base: Volatile<ReadWrite, u32>,
}

impl PortRegisters {
    // TODO: Better API
    pub const fn as_raw(&self) -> Volatile<ReadWrite, u32> {
        self.base
    }
}
