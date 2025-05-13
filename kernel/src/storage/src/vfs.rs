use super::fs::{FileError, FileResult, FileSystem, PathBuf};
use crate::fs::Path;
use alloc::{boxed::Box, collections::BTreeMap};
use core::{
    marker::PhantomData,
    sync::atomic::{AtomicI64, Ordering},
};
use hyperdrive::locks::rw::RwLock;

pub trait VfsHelper {
    #[must_use]
    /// Returns the current process ID.
    fn get_current_process_id() -> u64;
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct Handle {
    id: i64,
}

impl Default for Handle {
    fn default() -> Self {
        Self::new()
    }
}

static HANDLE_COUNTER: AtomicI64 = AtomicI64::new(0);

impl Handle {
    pub const INVALID: Self = Self { id: -1 };

    #[must_use]
    #[inline]
    #[expect(clippy::missing_panics_doc, reason = "Never panics")]
    pub fn new() -> Self {
        let id = HANDLE_COUNTER.fetch_add(1, Ordering::Relaxed);

        // By opening 1 000 files a second, it would take 3 000 000 centuries to overflow,
        // so we can deliberately not handle the overflow.
        assert!(id >= 0, "Handle ID overflow");

        Self { id }
    }

    #[must_use]
    #[inline]
    /// Creates a new handle from a raw ID.
    ///
    /// # Safety
    ///
    /// The created handle should not be used to create **new** handles.
    /// It is only meant for comparison with other handles.
    ///
    /// The given ID should be positive.
    pub const unsafe fn from_raw(id: i64) -> Self {
        debug_assert!(id >= 0);
        Self { id }
    }

    #[must_use]
    #[inline]
    pub const fn id(&self) -> i64 {
        self.id
    }
}

type Mounts = BTreeMap<PathBuf, RwLock<Box<dyn FileSystem>>>;
type OpenFiles = BTreeMap<Handle, OpenFileInfo>;

#[derive(Default)]
pub struct Vfs<H: VfsHelper> {
    mounts: RwLock<Mounts>,
    open_handles: RwLock<OpenFiles>,
    _helper: PhantomData<H>,
}

struct OpenFileInfo {
    process_id: u64,
    path: PathBuf,
}

impl<H: VfsHelper> Vfs<H> {
    #[must_use]
    #[inline]
    /// Creates a new VFS instance.
    pub const fn new() -> Self {
        Self {
            mounts: RwLock::new(BTreeMap::new()),
            open_handles: RwLock::new(BTreeMap::new()),
            _helper: PhantomData,
        }
    }

    /// Mounts a filesystem at the given path.
    pub fn mount(&self, path: PathBuf, fs: Box<dyn FileSystem>) {
        self.mounts.write().insert(path, RwLock::new(fs));
    }

    /// Unmounts the filesystem at the given path.
    pub fn unmount(&self, path: &str) -> FileResult<Box<dyn FileSystem>> {
        self.mounts
            .write()
            .remove(path)
            .map(RwLock::into_inner)
            .ok_or(FileError::NotFound)
    }

    /// Checks if a file is opened.
    fn check_file_opened(&self, path: Path) -> bool {
        let current_pid = H::get_current_process_id();
        self.open_handles.read().values().any(|open_file| {
            open_file.path.as_path() == path && open_file.process_id == current_pid
        })
    }

    /// Creates a new handle.
    ///
    /// This function performs checks and adds the handle to the open handles list.
    fn new_handle(&self, path: Path) -> FileResult<Handle> {
        if self.check_file_opened(path) {
            return Err(FileError::PermissionDenied);
        }
        let handle = Handle::new();
        let open_file_info = OpenFileInfo {
            path: path.to_owned(),
            process_id: H::get_current_process_id(),
        };
        self.open_handles.write().insert(handle, open_file_info);
        Ok(handle)
    }

    fn delete_handle(&self, handle: Handle) -> FileResult<()> {
        self.open_handles
            .write()
            .remove(&handle)
            .ok_or(FileError::InvalidHandle)?;
        Ok(())
    }

    /// Converts a handle to a path, checking the handle validity.
    fn handle_to_path(&self, handle: Handle) -> FileResult<PathBuf> {
        let open_files = self.open_handles.read();
        let open_file = open_files.get(&handle).ok_or(FileError::InvalidHandle)?;
        (open_file.process_id == H::get_current_process_id())
            .then(|| open_file.path.clone())
            .ok_or(FileError::PermissionDenied)
    }

    /// Converts a path to a filesystem, checking the path validity.
    ///
    /// The given function `f` is called with the filesystem and the relative path.
    /// The function returns the result of `f`.
    fn path_to_fs<T>(
        &self,
        path: Path,
        f: impl FnOnce(&mut dyn FileSystem, Path) -> FileResult<T>,
    ) -> FileResult<T> {
        let mounts = self.mounts.read();

        let mut best_match: Option<(Path, _)> = None;
        for (mount_path, fs) in mounts.iter() {
            if path.starts_with(mount_path.as_path().as_str())
                && best_match
                    .is_none_or(|(best_path, _)| mount_path.as_path().len() > best_path.len())
            {
                best_match = Some((mount_path.as_path(), fs));
            }
        }
        let (mount_path, fs) = best_match.ok_or(FileError::InvalidPath)?;
        let rel_path = Path::from(&path[mount_path.len()..]);
        f(&mut **fs.write(), rel_path)
    }

    #[inline]
    /// Creates a new file at the given path.
    pub fn create(&self, path: Path) -> FileResult<()> {
        self.path_to_fs(path, |fs, rel_path| fs.create(rel_path))
    }

    #[inline]
    /// Opens a file at the given path.
    pub fn open(&self, path: Path) -> FileResult<Handle> {
        let handle = self.new_handle(path)?;
        self.path_to_fs(path, |fs, rel_path| fs.open(rel_path))?;
        Ok(handle)
    }

    #[inline]
    /// Closes a file associated with the given handle.
    pub fn close(&self, handle: Handle) -> FileResult<()> {
        let path = self.handle_to_path(handle)?;
        self.delete_handle(handle)?;
        self.path_to_fs(path.as_path(), |fs, rel_path| fs.close(rel_path))?;
        Ok(())
    }

    #[inline]
    /// Closes all files opened by the given process ID.
    ///
    /// # Safety
    ///
    /// This function should only be called with a `u64` of a process that has completed its execution.
    pub unsafe fn close_all_from_process(&self, pid: u64) {
        let mut open_handles = self.open_handles.write();
        open_handles.retain(|_handle, open_file| open_file.process_id != pid);
    }

    /// Deletes a file at the given path.
    pub fn delete(&self, path: Path) -> FileResult<()> {
        if self.check_file_opened(path) {
            return Err(FileError::PermissionDenied);
        }
        // FIXME: TOCTOU vulnerability?
        self.path_to_fs(path, |fs, rel_path| fs.delete(rel_path))
    }

    /// Deletes a file at the given path.
    pub fn exists(&self, path: Path) -> FileResult<bool> {
        self.path_to_fs(path, |fs, rel_path| fs.exists(rel_path))
    }

    /// Reads from a file associated with the given handle into the given buffer.
    pub fn read(&self, handle: Handle, buffer: &mut [u8], offset: usize) -> FileResult<usize> {
        let path = self.handle_to_path(handle)?;
        self.path_to_fs(path.as_path(), |fs, rel_path| {
            fs.read(rel_path, buffer, offset)
        })
    }

    /// Writes the given buffer to a file at the given path.
    pub fn write(&self, handle: Handle, buffer: &[u8], offset: usize) -> FileResult<usize> {
        let path = self.handle_to_path(handle)?;
        self.path_to_fs(path.as_path(), |fs, rel_path| {
            fs.write(rel_path, buffer, offset)
        })
    }
}
