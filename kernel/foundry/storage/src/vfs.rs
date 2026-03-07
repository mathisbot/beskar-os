use super::fs::{FileError, FileResult, FileSystem, PathBuf};
use crate::fs::Path;
use alloc::{boxed::Box, vec::Vec};
use core::{
    marker::PhantomData,
    sync::atomic::{AtomicI64, Ordering},
};
use hashbrown::HashMap;
use hyperdrive::locks::rw::RwLock;

pub trait VfsHelper {
    #[must_use]
    /// Returns the current process ID.
    fn get_current_process_id() -> u64;
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
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
    pub fn new() -> Self {
        // By opening 1 000 files a second, it would take 3 000 000 centuries to overflow,
        // so we can deliberately not handle the overflow.
        let id = HANDLE_COUNTER.fetch_add(1, Ordering::Relaxed);
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
        debug_assert!(id >= 1);
        Self { id }
    }

    #[must_use]
    #[inline]
    pub const fn id(&self) -> i64 {
        self.id
    }
}

type OpenFiles = HashMap<Handle, OpenFileInfo>;

#[derive(Default)]
pub struct Vfs<H: VfsHelper> {
    mounts: RwLock<MountIndex>,
    open_handles: RwLock<OpenFiles>,
    _helper: PhantomData<H>,
}

struct OpenFileInfo {
    process_id: u64,
    path: PathBuf,
}

/// Cached sorted mount list for efficient path matching.
/// Stored as `(path_length, path, filesystem)` sorted by length descending.
#[derive(Default)]
struct MountIndex {
    /// Sorted list of mounts by path length (longest first) for prefix matching
    sorted_mounts: Vec<(usize, PathBuf)>,
    /// `HashMap` for filesystem lookup
    filesystems: HashMap<PathBuf, RwLock<Box<dyn FileSystem + Send + Sync>>>,
}

impl<H: VfsHelper> Vfs<H> {
    #[must_use]
    /// Creates a new VFS instance.
    pub fn new() -> Self {
        Self {
            mounts: RwLock::new(MountIndex::default()),
            open_handles: RwLock::new(HashMap::new()),
            _helper: PhantomData,
        }
    }

    /// Mounts a filesystem at the given path.
    pub fn mount(&self, path: PathBuf, fs: Box<dyn FileSystem + Send + Sync>) {
        let mut mounts = self.mounts.write();
        let path_len = path.as_path().len();

        // Insert into hashmap
        mounts.filesystems.insert(path.clone(), RwLock::new(fs));

        // Insert into sorted list maintaining descending order by length
        match mounts
            .sorted_mounts
            .binary_search_by(|&(len, _)| len.cmp(&path_len).reverse())
        {
            Ok(idx) => {
                // Find the correct position (may have duplicates)
                mounts.sorted_mounts.insert(idx, (path_len, path));
            }
            Err(idx) => mounts.sorted_mounts.insert(idx, (path_len, path)),
        }
    }

    /// Unmounts the filesystem at the given path.
    pub fn unmount(&self, path: Path) -> FileResult<Box<dyn FileSystem + Send + Sync>> {
        let mut mounts = self.mounts.write();

        // Remove from sorted list
        mounts.sorted_mounts.retain(|(_, p)| p.as_path() != path);

        // Remove from hashmap and return the filesystem
        // Try to find matching PathBuf by string comparison
        let path_buf = mounts
            .filesystems
            .iter()
            .find(|(p, _)| p.as_path() == path)
            .map(|(p, _)| p.clone());

        path_buf.map_or(Err(FileError::NotFound), |path_buf| {
            mounts
                .filesystems
                .remove(&path_buf)
                .map(RwLock::into_inner)
                .ok_or(FileError::NotFound)
        })
    }

    /// Creates a new handle.
    ///
    /// This function performs checks and adds the handle to the open handles list.
    fn new_handle(&self, path: Path) -> FileResult<Handle> {
        let current_pid = H::get_current_process_id();

        // Check if already opened by this process
        {
            let open_handles = self.open_handles.read();
            if open_handles.values().any(|open_file| {
                open_file.path.as_path() == path && open_file.process_id == current_pid
            }) {
                return Err(FileError::PermissionDenied);
            }
        }

        let handle = Handle::new();
        let open_file_info = OpenFileInfo {
            path: path.to_owned(),
            process_id: current_pid,
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
        f: impl FnOnce(&mut (dyn FileSystem + Send + Sync), Path) -> FileResult<T>,
    ) -> FileResult<T> {
        let mounts = self.mounts.read();
        let path_str = path.as_str();

        for &(mount_len, ref mount_path) in &mounts.sorted_mounts {
            if mount_len > path_str.len() {
                continue;
            }

            // Check if path starts with mount point
            if &path_str[..mount_len] == mount_path.as_path().as_str() {
                // Ensure we match at path boundaries to avoid partial matches
                // e.g., /dev should not match /device
                if mount_len == path_str.len()
                    || path_str.as_bytes().get(mount_len) == Some(&b'/')
                    || mount_path.as_path().as_str().ends_with('/')
                {
                    // Found match - get filesystem from hashmap for O(1) lookup
                    let fs = mounts
                        .filesystems
                        .get(mount_path)
                        .ok_or(FileError::InvalidPath)?;
                    let rel_path = Path::from(&path_str[mount_len..]);
                    return f(&mut **fs.write(), rel_path);
                }
            }
        }

        Err(FileError::InvalidPath)
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
    /// This function should only be called with a `u64` of a process that has completed its execution.
    pub fn close_all_from_process(&self, pid: u64) {
        self.open_handles.write().retain(|_handle, open_file| {
            let retained = open_file.process_id != pid;
            if !retained {
                let res =
                    self.path_to_fs(open_file.path.as_path(), |fs, rel_path| fs.close(rel_path));
                debug_assert!(res.is_ok(), "Failed to close file during process cleanup");
            }
            retained
        });
    }

    /// Deletes a file at the given path.
    pub fn delete(&self, path: Path) -> FileResult<()> {
        let current_pid = H::get_current_process_id();

        // Check if file is opened with lock to prevent TOCTOU issues
        {
            let open_handles = self.open_handles.read();
            if open_handles.values().any(|open_file| {
                open_file.path.as_path() == path && open_file.process_id == current_pid
            }) {
                return Err(FileError::PermissionDenied);
            }
        }

        // Delete the file from the filesystem
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

    pub fn metadata(&self, path: Path) -> FileResult<crate::fs::FileMetadata> {
        self.path_to_fs(path, |fs, rel_path| fs.metadata(rel_path))
    }

    pub fn read_dir(&self, path: Path) -> FileResult<Vec<PathBuf>> {
        self.path_to_fs(path, |fs, rel_path| fs.read_dir(rel_path))
    }
}
