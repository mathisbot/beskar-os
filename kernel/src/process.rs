use core::sync::atomic::{AtomicU64, Ordering};

use alloc::{
    string::{String, ToString},
    sync::Arc,
};
use hyperdrive::once::Once;

use crate::mem::address_space::{self, AddressSpace};

pub mod dummy;
pub mod elf;
pub mod scheduler;

static KERNEL_PROCESS: Once<Arc<Process>> = Once::uninit();

pub fn init() {
    KERNEL_PROCESS.call_once(|| {
        Arc::new(Process {
            name: "kernel".to_string(),
            pid: ProcessId::new(),
            address_space: *address_space::get_kernel_address_space(),
            kind: Kind::Kernel,
        })
    });

    let kernel_process = KERNEL_PROCESS.get().unwrap().clone();
    debug_assert!(kernel_process.address_space().is_active());

    let current_thread = scheduler::thread::Thread::new_kernel(kernel_process);

    unsafe { scheduler::init(current_thread) };
}

pub struct Process {
    name: String,
    pid: ProcessId,
    address_space: AddressSpace,
    kind: Kind,
}

impl Process {
    #[must_use]
    #[inline]
    pub fn new(name: &str, kind: Kind) -> Self {
        Self {
            name: name.to_string(),
            pid: ProcessId::new(),
            address_space: AddressSpace::new(),
            kind,
        }
    }

    #[must_use]
    #[inline]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    #[inline]
    pub const fn pid(&self) -> ProcessId {
        self.pid
    }

    #[must_use]
    #[inline]
    pub const fn address_space(&self) -> &AddressSpace {
        &self.address_space
    }

    #[must_use]
    #[inline]
    pub const fn kind(&self) -> Kind {
        self.kind
    }
}

static PID_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ProcessId(u64);

impl core::ops::Deref for ProcessId {
    type Target = u64;

    fn deref(&self) -> &u64 {
        &self.0
    }
}

impl Default for ProcessId {
    fn default() -> Self {
        Self::new()
    }
}

impl ProcessId {
    pub fn new() -> Self {
        Self(PID_COUNTER.fetch_add(1, Ordering::Relaxed))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
#[repr(u8)]
pub enum Kind {
    /// Vital process kind.
    /// On panic or other critical errors, the while system will be halted.
    Kernel,
    /// Driver process kind.
    /// These are Ring 0 processes that are not vital for the system.
    Driver,
    /// User process kind.
    /// These are Ring 3 processes.
    User,
}
