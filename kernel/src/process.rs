use crate::mem::address_space::{self, AddressSpace};
use alloc::{
    string::{String, ToString},
    sync::Arc,
};
use beskar_hal::process::Kind;
use binary::LoadedBinary;
use core::sync::atomic::{AtomicU16, AtomicU64, Ordering};
use hyperdrive::{once::Once, ptrs::view::ViewRef};

pub mod binary;
pub mod scheduler;

static KERNEL_PROCESS: Once<Arc<Process>> = Once::uninit();

pub fn init() {
    KERNEL_PROCESS.call_once(|| {
        Arc::new(Process {
            name: "kernel".to_string(),
            pid: ProcessId::new(),
            address_space: ViewRef::new_borrow(address_space::get_kernel_address_space()),
            kind: Kind::Kernel,
            binary_data: None,
        })
    });

    let kernel_process = KERNEL_PROCESS.get().unwrap().clone();
    debug_assert!(kernel_process.address_space().is_active());

    let current_thread = scheduler::thread::Thread::new_kernel(kernel_process);

    unsafe { scheduler::init(current_thread) };
}

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
    /// Creates a new process ID.
    pub fn new() -> Self {
        static PID_COUNTER: AtomicU64 = AtomicU64::new(0);
        Self(PID_COUNTER.fetch_add(1, Ordering::Relaxed))
    }

    #[must_use]
    #[inline]
    /// Creates a new process ID from a raw ID.
    ///
    /// # Safety
    ///
    /// The created process ID should not be used to create a process.
    /// It is only meant for internal/comparative purposes.
    pub const unsafe fn from_raw(id: u64) -> Self {
        Self(id)
    }

    #[must_use]
    #[inline]
    /// Returns the raw ID of the process.
    pub const fn as_u64(&self) -> u64 {
        self.0
    }
}

pub struct Process {
    name: String,
    pid: ProcessId,
    address_space: ViewRef<'static, AddressSpace>,
    kind: Kind,
    binary_data: Option<BinaryData<'static>>,
}

impl Process {
    #[must_use]
    #[inline]
    pub fn new(name: &str, kind: Kind, binary: Option<binary::Binary<'static>>) -> Self {
        Self {
            name: String::from(name),
            pid: ProcessId::new(),
            address_space: ViewRef::new_owned(AddressSpace::new()),
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

impl Drop for Process {
    fn drop(&mut self) {
        crate::storage::vfs().close_all_from_process(self.pid.as_u64());
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

impl Default for Pcid {
    fn default() -> Self {
        Self::new()
    }
}

impl Pcid {
    #[must_use]
    #[inline]
    pub fn new() -> Self {
        const MAX_PCID: u16 = 1 << 12;
        static PCID_COUNTER: AtomicU16 = AtomicU16::new(0);

        let raw: u16 = PCID_COUNTER.fetch_add(1, Ordering::Relaxed);

        if raw >= MAX_PCID {
            todo!("PCID recycling");
        }

        Self(raw % 4096)
    }

    #[must_use]
    #[inline]
    pub const fn as_u16(&self) -> u16 {
        self.0
    }
}

pub struct Stdout;

impl ::storage::KernelDevice for Stdout {
    fn read(&mut self, dst: &mut [u8], _offset: usize) -> Result<(), storage::BlockDeviceError> {
        if dst.is_empty() {
            return Ok(());
        }

        Err(::storage::BlockDeviceError::Unsupported)
    }

    fn write(&mut self, src: &[u8], _offset: usize) -> Result<(), storage::BlockDeviceError> {
        let text = core::str::from_utf8(src).map_err(|_| ::storage::BlockDeviceError::Io)?;

        // TODO: Send somewhere else than the kernel log.
        let tid = crate::process::scheduler::current_thread_id();
        video::info!("[Thread {}] {}", tid.as_u64(), text);

        Ok(())
    }
}

pub struct RandFile;

impl ::storage::KernelDevice for RandFile {
    fn read(&mut self, dst: &mut [u8], _offset: usize) -> Result<(), storage::BlockDeviceError> {
        if dst.is_empty() {
            Ok(())
        } else {
            crate::arch::rand::rand_bytes(dst).map_err(|_| ::storage::BlockDeviceError::Io)
        }
    }

    fn write(&mut self, _src: &[u8], _offset: usize) -> Result<(), storage::BlockDeviceError> {
        Err(::storage::BlockDeviceError::Unsupported)
    }
}
