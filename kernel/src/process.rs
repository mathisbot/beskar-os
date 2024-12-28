use core::sync::atomic::{AtomicU64, Ordering};

use alloc::{
    string::{String, ToString},
    sync::Arc,
};
use scheduler::priority;

use crate::mem::address_space::AddressSpace;

pub mod scheduler;

pub fn init() {
    // FIXME: One process per core ?
    let kernel_process = Arc::new(Process {
        name: "kernel".to_string(),
        pid: ProcessId::new(),
        address_space: *crate::mem::address_space::get_kernel_address_space(),
    });
    let current_thread = scheduler::thread::Thread::new(kernel_process, priority::Priority::High);

    unsafe { scheduler::init(current_thread) };
}

pub struct Process {
    name: String,
    pid: ProcessId,
    address_space: AddressSpace,
}

impl Process {
    #[must_use]
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
