use super::fs::{FileSystem, PathBuf};
use alloc::{boxed::Box, collections::BTreeMap};
use hyperdrive::locks::rw::RwLock;

static VFS: Vfs = Vfs::new();

#[derive(Default)]
pub struct Vfs {
    mounts: RwLock<BTreeMap<PathBuf, RwLock<Box<dyn FileSystem>>>>,
}

impl Vfs {
    #[must_use]
    #[inline]
    pub const fn new() -> Self {
        Self {
            mounts: RwLock::new(BTreeMap::new()),
        }
    }
}
