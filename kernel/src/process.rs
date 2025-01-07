use core::sync::atomic::{AtomicU64, Ordering};

use alloc::{
    boxed::Box,
    string::{String, ToString},
    sync::Arc,
};
use scheduler::priority;

use crate::mem::address_space::AddressSpace;

pub mod scheduler;

pub fn init() {
    let kernel_process = Arc::new(Process {
        name: "kernel".to_string(),
        pid: ProcessId::new(),
        address_space: *crate::mem::address_space::get_kernel_address_space(),
    });

    debug_assert!(kernel_process.address_space().is_active());

    let current_thread = scheduler::thread::Thread::new_kernel(kernel_process.clone());

    unsafe { scheduler::init(current_thread) };

    let test_thread = scheduler::thread::Thread::new(
        kernel_process.clone(),
        priority::Priority::Normal,
        alloc::vec![0; 1024 * 256], // 256 KiB
        test1 as *const (),
    );
    scheduler::spawn_thread(Box::pin(test_thread));
    let test_thread = scheduler::thread::Thread::new(
        kernel_process,
        priority::Priority::Normal,
        alloc::vec![0; 1024 * 256], // 256 KiB
        test2 as *const (),
    );
    scheduler::spawn_thread(Box::pin(test_thread));
}

fn test1() {
    let mut counter = 0_u32;
    let core_id = crate::locals!().core_id();

    loop {
        crate::info!("Hello, thread 1 on core {}! counter={}", core_id, counter);
        counter += 1;
        crate::time::wait_ms(500);
    }
}

fn test2() {
    let mut counter = 0_u32;
    let core_id = crate::locals!().core_id();

    loop {
        crate::info!("Hello, thread 2 on core {}! counter={}", core_id, counter);
        counter += 1;
        crate::time::wait_ms(500);
    }
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
