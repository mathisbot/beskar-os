use crate::mem::AddressSpace;
use alloc::{
    string::{String, ToString},
    sync::Arc,
};
use beskar_hal::process::Kind;
use core::sync::atomic::{AtomicU64, Ordering};
use hyperdrive::{once::Once, queues::mpmc::MpmcQueue};
use storage::fs::{Path, PathBuf};

pub mod binary;
pub mod scheduler;
pub mod sync;

const MAX_SURFACES_PER_PROCESS: usize = 2;

pub fn init() {
    static KERNEL_PROCESS: Once<Arc<Process>> = Once::uninit();

    KERNEL_PROCESS.call_once(|| {
        Arc::new(Process {
            name: "kernel".to_string(),
            pid: ProcessId::new(),
            address_space: AddressSpace::new(),
            kind: Kind::Kernel,
            binary: None,
            surfaces: MpmcQueue::new(),
        })
    });

    let kernel_process = KERNEL_PROCESS.get().unwrap().clone();
    // Safety: the kernel process address space has just been constructed and is valid.
    unsafe { kernel_process.address_space().activate() };

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
    address_space: AddressSpace,
    kind: Kind,
    binary: Option<PathBuf>,
    /// Surfaces allocated by this process (interior mutability for registration)
    surfaces: MpmcQueue<MAX_SURFACES_PER_PROCESS, crate::video::SurfaceGuard>,
}

impl Process {
    #[must_use]
    #[inline]
    pub fn new(name: &str, kind: Kind, binary: Option<PathBuf>) -> Self {
        Self {
            name: String::from(name),
            pid: ProcessId::new(),
            address_space: AddressSpace::new(),
            kind,
            binary,
            surfaces: MpmcQueue::new(),
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
    pub(crate) const fn address_space(&self) -> &AddressSpace {
        &self.address_space
    }

    #[must_use]
    #[inline]
    pub const fn kind(&self) -> Kind {
        self.kind
    }

    #[must_use]
    #[inline]
    pub fn binary(&self) -> Option<Path<'_>> {
        self.binary.as_ref().map(PathBuf::as_path)
    }

    /// Register a surface as belonging to this process
    pub fn register_surface(&self, guard: crate::video::SurfaceGuard) -> bool {
        let res = self.surfaces.try_push(guard);
        res.is_ok()
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        // Close all file descriptors
        crate::storage::vfs().close_all_from_process(self.pid.as_u64());
    }
}

#[must_use]
#[inline]
pub fn current() -> Arc<Process> {
    scheduler::current_process()
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
        crate::info!("[Thread {}] {}", tid.as_u64(), text);

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

pub struct SeedFile;

impl ::storage::KernelDevice for SeedFile {
    fn read(&mut self, dst: &mut [u8], _offset: usize) -> Result<(), storage::BlockDeviceError> {
        if dst.is_empty() {
            Ok(())
        } else {
            crate::arch::rand::rand_seed_bytes(dst).map_err(|_| ::storage::BlockDeviceError::Io)
        }
    }

    fn write(&mut self, _src: &[u8], _offset: usize) -> Result<(), storage::BlockDeviceError> {
        Err(::storage::BlockDeviceError::Unsupported)
    }
}
