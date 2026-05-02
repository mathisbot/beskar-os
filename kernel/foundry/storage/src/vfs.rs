use super::fs::{FileError, FileResult, FileSystem, PathBuf};
use crate::fs::Path;
use alloc::{boxed::Box, vec::Vec};
use core::sync::atomic::{AtomicI64, Ordering};
use hashbrown::hash_map::{Entry, HashMap};
use hyperdrive::locks::rw::RwLock;

// FIXME: TOCTOU issues
// This requires changing the architecture a lot, so it will wait.

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Handle {
    id: i64,
}

impl Default for Handle {
    fn default() -> Self {
        Self::new()
    }
}

impl Handle {
    pub const INVALID: Self = Self { id: -1 };

    #[must_use]
    #[inline]
    pub fn new() -> Self {
        static HANDLE_COUNTER: AtomicI64 = AtomicI64::new(0);

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
        debug_assert!(id >= 0);
        Self { id }
    }

    #[must_use]
    #[inline]
    pub const fn id(&self) -> i64 {
        self.id
    }
}

#[derive(Debug, Clone)]
struct OpenFileInfo {
    process_id: u64,
    path: PathBuf,
}

type OpenFiles = HashMap<Handle, OpenFileInfo>;

/// Cached sorted mount list for efficient path matching.
/// Stored as `(path_length, path, filesystem)` sorted by length descending.
#[derive(Default)]
struct MountIndex {
    /// Sorted list of mounts by path length (longest first) for prefix matching
    sorted_mounts: Vec<PathBuf>,
    /// `HashMap` for filesystem lookup
    filesystems: HashMap<PathBuf, RwLock<Box<dyn FileSystem + Send + Sync>>>,
}

#[derive(Default)]
pub struct Vfs {
    mounts: RwLock<MountIndex>,
    open_handles: RwLock<OpenFiles>,
}

impl Vfs {
    #[must_use]
    /// Creates a new VFS instance.
    pub fn new() -> Self {
        Self {
            mounts: RwLock::new(MountIndex::default()),
            open_handles: RwLock::new(HashMap::new()),
        }
    }

    /// Mounts a filesystem at the given path.
    pub fn mount(&self, path: PathBuf, fs: Box<dyn FileSystem + Send + Sync>) -> FileResult<()> {
        let mut mounts = self.mounts.write();
        let path_len = path.len();

        // Insert into hashmap
        let res = mounts.filesystems.try_insert(path.clone(), RwLock::new(fs));
        if res.is_err() {
            return Err(FileError::AlreadyExists);
        }

        // Insert into sorted list maintaining descending order by length
        match mounts
            .sorted_mounts
            .binary_search_by(|path| path.len().cmp(&path_len).reverse())
        {
            Ok(idx) | Err(idx) => mounts.sorted_mounts.insert(idx, path),
        }

        Ok(())
    }

    /// Unmounts the filesystem at the given path.
    pub fn unmount(&self, path: Path) -> FileResult<Box<dyn FileSystem + Send + Sync>> {
        let mut mounts = self.mounts.write();

        mounts.sorted_mounts.retain(|p| p.as_path() != path);

        let fs = mounts.filesystems.remove(&path);
        fs.ok_or(FileError::NotFound).map(RwLock::into_inner)
    }

    /// Resolves a path to the corresponding filesystem and relative path, then applies the given function.
    fn path_to_fs<T>(
        &self,
        path: Path,
        f: impl FnOnce(&mut (dyn FileSystem + Send + Sync), Path) -> FileResult<T>,
    ) -> FileResult<T> {
        let mounts = self.mounts.read();
        let path = path.as_str();

        for mount_path in &mounts.sorted_mounts {
            let mount_len = mount_path.len();

            if path.get(..mount_len) == Some(mount_path.as_path().as_str())
                && (mount_len == path.len()
                    || path.as_bytes().get(mount_len) == Some(&b'/')
                    || mount_path.as_path().as_str().ends_with('/'))
            {
                let fs = mounts
                    .filesystems
                    .get(mount_path)
                    .ok_or(FileError::InvalidPath)?;
                let rel_path = Path::from(&path[mount_len..]);
                return f(&mut **fs.write(), rel_path);
            }
        }

        Err(FileError::InvalidPath)
    }

    /// Inserts a new handle for the given process and path, ensuring no duplicate opens.
    fn insert_handle(&self, pid: u64, path: Path) -> FileResult<Handle> {
        let mut open = self.open_handles.write();

        let already_opened_info = open.iter().find(|(_, info)| info.path.as_path() == path);
        if let Some((_, info)) = already_opened_info {
            let err = if info.process_id == pid {
                FileError::AlreadyExists
            } else {
                FileError::PermissionDenied
            };
            return Err(err);
        }

        let handle = Handle::new();
        open.insert(
            handle,
            OpenFileInfo {
                process_id: pid,
                path: path.to_owned(),
            },
        );
        Ok(handle)
    }

    /// Retrieves the open file info for a given handle, ensuring it belongs to the requesting process.
    fn get_open_info(&self, pid: u64, handle: Handle) -> FileResult<OpenFileInfo> {
        let open_handles = self.open_handles.read();
        let info = open_handles.get(&handle).ok_or(FileError::InvalidHandle)?;

        if info.process_id != pid {
            return Err(FileError::PermissionDenied);
        }

        // FIXME: Avoid cloning here
        Ok(info.clone())
    }

    /// Removes a handle from the open handles, ensuring it belongs to the requesting process.
    fn remove_handle(&self, pid: u64, handle: Handle) -> FileResult<()> {
        let mut open = self.open_handles.write();

        let entry = open.entry(handle);
        match entry {
            Entry::Occupied(e) => {
                if e.get().process_id != pid {
                    return Err(FileError::PermissionDenied);
                }
                e.remove();
                Ok(())
            }
            Entry::Vacant(_) => Err(FileError::InvalidHandle),
        }
    }

    #[inline]
    /// Creates a new file at the given path.
    pub fn create(&self, path: Path) -> FileResult<()> {
        self.path_to_fs(path, |fs, rel_path| fs.create(rel_path))
    }

    #[inline]
    /// Opens a file at the given path for the specified process, returning a handle.
    pub fn open(&self, pid: u64, path: Path) -> FileResult<Handle> {
        let handle = self.insert_handle(pid, path)?;
        self.path_to_fs(path, |fs, rel_path| fs.open(rel_path))?;
        Ok(handle)
    }

    #[inline]
    /// Closes a file handle for the specified process.
    pub fn close(&self, pid: u64, handle: Handle) -> FileResult<()> {
        let info = self.get_open_info(pid, handle)?;
        self.path_to_fs(info.path.as_path(), |fs, rel_path| fs.close(rel_path))?;
        self.remove_handle(pid, handle)
    }

    /// Closes all file handles associated with the specified process.
    fn close_all_with<F: Fn(&OpenFileInfo) -> bool>(&self, f: F) -> FileResult<()> {
        let paths_to_close: Vec<PathBuf> = {
            let mut open = self.open_handles.write();
            open.extract_if(|_, info| f(info))
                .map(|(_, info)| info.path)
                .collect()
        };

        let mut error_encountered = false;

        for path in paths_to_close {
            let close_res = self.path_to_fs(path.as_path(), |fs, rel_path| fs.close(rel_path));
            if close_res.is_err() {
                error_encountered = true;
            }
        }

        if error_encountered {
            Err(FileError::CorruptedFS)
        } else {
            Ok(())
        }
    }

    #[inline]
    /// Closes all file handles associated with the specified process.
    pub fn close_all_from_process(&self, pid: u64) -> FileResult<()> {
        self.close_all_with(|info| info.process_id == pid)
    }

    /// Deletes a file at the given path, ensuring no open handles exist for it.
    pub fn delete(&self, _pid: u64, path: Path) -> FileResult<()> {
        {
            let open = self.open_handles.read();
            if open.values().any(|info| info.path.as_path() == path) {
                return Err(FileError::PermissionDenied);
            }
        }

        self.path_to_fs(path, |fs, rel_path| fs.delete(rel_path))
    }

    /// Checks if a file exists at the given path.
    pub fn exists(&self, path: Path) -> FileResult<bool> {
        self.path_to_fs(path, |fs, rel_path| fs.exists(rel_path))
    }

    /// Reads from a file handle for the specified process into the provided buffer, starting at the given offset.
    pub fn read(
        &self,
        pid: u64,
        handle: Handle,
        buffer: &mut [u8],
        offset: usize,
    ) -> FileResult<usize> {
        let info = self.get_open_info(pid, handle)?;
        self.path_to_fs(info.path.as_path(), |fs, rel_path| {
            fs.read(rel_path, buffer, offset)
        })
    }

    /// Writes to a file handle for the specified process from the provided buffer, starting at the given offset.
    pub fn write(
        &self,
        pid: u64,
        handle: Handle,
        buffer: &[u8],
        offset: usize,
    ) -> FileResult<usize> {
        let info = self.get_open_info(pid, handle)?;
        self.path_to_fs(info.path.as_path(), |fs, rel_path| {
            fs.write(rel_path, buffer, offset)
        })
    }

    /// Retrieves metadata for a file at the given path.
    pub fn metadata(&self, path: Path) -> FileResult<crate::fs::FileMetadata> {
        self.path_to_fs(path, |fs, rel_path| fs.metadata(rel_path))
    }

    /// Reads the contents of a directory at the given path, returning a list of entries.
    pub fn read_dir(&self, path: Path) -> FileResult<Vec<PathBuf>> {
        self.path_to_fs(path, |fs, rel_path| fs.read_dir(rel_path))
    }
}
