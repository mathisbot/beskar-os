use super::FileSystem;
use crate::KernelDevice;
use alloc::{boxed::Box, vec::Vec};

struct DeviceFile {
    path: super::PathBuf,
    device: Box<dyn KernelDevice>,
}

#[derive(Default)]
/// A pass-through file system for device files.
pub struct DeviceFS {
    devices: Vec<DeviceFile>,
}

impl DeviceFS {
    #[must_use]
    #[inline]
    /// Creates a new `DeviceFS` instance.
    pub const fn new() -> Self {
        Self {
            devices: Vec::new(),
        }
    }

    #[inline]
    /// Adds a new device to the file system.
    pub fn add_device(&mut self, path: super::PathBuf, device: Box<dyn KernelDevice>) {
        self.devices.push(DeviceFile { path, device });
    }

    #[inline]
    /// Removes a device from the file system.
    pub fn remove_device(&mut self, path: super::Path) -> Option<Box<dyn KernelDevice>> {
        self.devices
            .iter()
            .position(|device| device.path.as_path() == path)
            .map(|pos| self.devices.remove(pos).device)
    }
}

impl FileSystem for DeviceFS {
    #[inline]
    fn close(&mut self, _path: super::Path) -> super::FileResult<()> {
        // No-op
        Ok(())
    }

    #[inline]
    fn create(&mut self, _path: super::Path) -> super::FileResult<()> {
        // DeviceFS does not support creating files
        Err(super::FileError::UnsupportedOperation)
    }

    #[inline]
    fn delete(&mut self, _path: super::Path) -> super::FileResult<()> {
        // DeviceFS does not support deleting files
        Err(super::FileError::UnsupportedOperation)
    }

    fn exists(&mut self, path: super::Path) -> super::FileResult<bool> {
        Ok(self
            .devices
            .iter()
            .any(|device| device.path.as_path() == path))
    }

    #[inline]
    fn open(&mut self, _path: super::Path) -> super::FileResult<()> {
        // No-op
        Ok(())
    }

    fn read(
        &mut self,
        path: super::Path,
        buffer: &mut [u8],
        offset: usize,
    ) -> super::FileResult<usize> {
        // Find the device associated with the given path.
        for device in &mut self.devices {
            if device.path.as_path() == path {
                device.device.read(buffer, offset)?;
                return Ok(buffer.len());
            }
        }
        Err(super::FileError::NotFound)
    }

    fn write(
        &mut self,
        path: super::Path,
        buffer: &[u8],
        offset: usize,
    ) -> super::FileResult<usize> {
        // Find the device associated with the given path.
        for device in &mut self.devices {
            if device.path.as_path() == path {
                device.device.write(buffer, offset)?;
                return Ok(buffer.len());
            }
        }
        Err(super::FileError::NotFound)
    }
}
