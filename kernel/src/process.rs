use alloc::{
    string::{String, ToString},
    sync::Arc,
};
use beskar_core::process::{Kind, ProcessId};
use binary::LoadedBinary;
use core::sync::atomic::{AtomicU16, Ordering};
use hyperdrive::{once::Once, ptrs::view::View};

use crate::mem::address_space::{self, AddressSpace};

pub mod binary;
pub mod scheduler;

static KERNEL_PROCESS: Once<Arc<Process>> = Once::uninit();

pub fn init() {
    KERNEL_PROCESS.call_once(|| {
        Arc::new(Process {
            name: "kernel".to_string(),
            pid: ProcessId::new(),
            address_space: View::new_borrow(address_space::get_kernel_address_space()),
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
    binary_data: Option<BinaryData<'static>>,
}

impl Process {
    #[must_use]
    #[inline]
    pub fn new(name: &str, kind: Kind, binary: Option<binary::Binary<'static>>) -> Self {
        Self {
            name: name.to_string(),
            pid: ProcessId::new(),
            address_space: View::new_owned(AddressSpace::new()),
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
    fn load_binary(&self) -> LoadedBinary {
        let binary_data = self.binary_data.as_ref().unwrap();
        binary_data.load();
        *binary_data.loaded.get().unwrap()
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

/// A struct representing a PCID.
///
/// Its valid values are 0 to 4095.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pcid(u16);

static PCID_COUNTER: AtomicU16 = AtomicU16::new(0);

impl Default for Pcid {
    fn default() -> Self {
        Self::new()
    }
}

impl Pcid {
    #[must_use]
    #[inline]
    pub fn new() -> Self {
        let raw: u16 = PCID_COUNTER.fetch_add(1, Ordering::Relaxed);

        if raw > 4095 {
            todo!("PCID recycling");
        }

        Self(raw % 4096)
    }

    #[must_use]
    #[inline]
    pub const fn as_u16(&self) -> u16 {
        debug_assert!(self.0 <= 4095, "PCID out of bounds");
        self.0
    }
}

pub struct Stdout;

impl ::storage::KernelDevice for Stdout {
    fn read(&mut self, dst: &mut [u8], _offset: usize) -> Result<(), storage::DeviceError> {
        if dst.len() == 0 {
            return Ok(());
        }

        Err(::storage::DeviceError::Unsupported)
    }

    fn write(&mut self, src: &[u8], _offset: usize) -> Result<(), storage::DeviceError> {
        let text = core::str::from_utf8(src).map_err(|_| ::storage::DeviceError::Io)?;

        // TODO: Send somewhere else than the kernel log.
        let tid = crate::process::scheduler::current_thread_id();
        video::info!("[Thread {}] {}", tid.as_u64(), text);

        Ok(())
    }
}
