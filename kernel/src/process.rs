use core::sync::atomic::{AtomicU64, Ordering};

use alloc::{
    string::{String, ToString},
    sync::Arc,
};
use beskar_core::arch::x86_64::userspace::Ring;
use hyperdrive::{once::Once, ptrs::view::View};

use crate::mem::address_space::{self, AddressSpace};

pub mod binary;
pub mod dummy;
pub mod scheduler;

static KERNEL_PROCESS: Once<Arc<Process>> = Once::uninit();

pub fn init() {
    KERNEL_PROCESS.call_once(|| {
        Arc::new(Process {
            name: "kernel".to_string(),
            pid: ProcessId::new(),
            address_space: View::Reference(address_space::get_kernel_address_space()),
            kind: Kind::Kernel,
            binary_data: None,
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
    address_space: View<'static, AddressSpace>,
    kind: Kind,
    // FIXME: Shouldn't be 'static
    binary_data: Option<BinaryData<'static>>,
}

impl Process {
    #[must_use]
    #[inline]
    pub fn new(name: &str, kind: Kind, binary: Option<binary::Binary<'static>>) -> Self {
        Self {
            name: name.to_string(),
            pid: ProcessId::new(),
            address_space: View::Owned(AddressSpace::new()),
            kind,
            binary_data: binary.map(BinaryData::new),
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
    pub fn address_space(&self) -> &AddressSpace {
        &self.address_space
    }

    #[must_use]
    #[inline]
    pub const fn kind(&self) -> Kind {
        self.kind
    }

    /// Loads the process binary into memory and returns its entry point.
    /// If the binary is already loaded, the only thing this function does is returning the entry point.
    ///
    /// # Panics
    ///
    /// Panics if there is no binary data associated with the process.
    fn load_binary(&self) -> extern "C" fn() {
        let binary_data = self.binary_data.as_ref().unwrap();
        binary_data.load();
        binary_data.loaded.get().unwrap().entry_point()
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
    #[must_use]
    #[inline]
    pub fn new() -> Self {
        Self(PID_COUNTER.fetch_add(1, Ordering::Relaxed))
    }

    #[must_use]
    #[inline]
    pub const fn as_u64(&self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
#[repr(u8)]
pub enum Kind {
    /// Vital process kind.
    /// On panic, the system will be halted.
    Kernel,
    /// Driver process kind.
    /// These are Ring 0 processes that are not vital for the system.
    Driver,
    /// User process kind.
    /// These are Ring 3 processes.
    User,
}

impl Kind {
    #[must_use]
    #[inline]
    pub const fn ring(&self) -> Ring {
        match self {
            Self::Kernel | Self::Driver => Ring::Kernel,
            Self::User => Ring::User,
        }
    }
}

struct BinaryData<'a> {
    input: binary::Binary<'a>,
    loaded: Once<binary::LoadedBinary>,
}

impl<'a> BinaryData<'a> {
    #[must_use]
    #[inline]
    /// Creates a new binary data.
    ///
    /// Calling this function does **not** load the binary into memory,
    /// it only tells the process that its binary exists and can be loaded.
    pub const fn new(input: binary::Binary<'a>) -> Self {
        Self {
            input,
            loaded: Once::uninit(),
        }
    }

    /// Loads the binary into memory.
    ///
    /// More precisely, the input binary will be loaded into the address space of the **current** process.
    ///
    /// # Panics
    ///
    /// Panics if the binary is invalid.
    pub fn load(&self) {
        self.loaded.call_once(|| self.input.load().unwrap());
    }
}

#[must_use]
#[inline]
pub fn current() -> Arc<Process> {
    scheduler::current_process()
}
