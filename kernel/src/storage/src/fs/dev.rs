use super::FileSystem;
use crate::BlockDevice;
use alloc::vec::Vec;

#[derive(Debug)]
struct DeviceFile<D: BlockDevice> {
    path: super::PathBuf,
    device: D,
}

#[derive(Debug, Default)]
/// A pass-through file system for device files.
pub struct DeviceFS<D: BlockDevice> {
    devices: Vec<DeviceFile<D>>,
}

impl<D: BlockDevice> DeviceFS<D> {
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
    pub fn add_device(&mut self, path: super::PathBuf, device: D) {
        self.devices.push(DeviceFile { path, device });
    }

    #[inline]
    /// Adds a new device to the file system.
    pub fn remove_device(&mut self, path: super::Path) -> Option<D> {
        if let Some(pos) = self
            .devices
            .iter()
            .position(|device| device.path.as_path() == path)
        {
            let device = self.devices.remove(pos);
            Some(device.device)
        } else {
            None
        }
    }
}

impl<D: BlockDevice> FileSystem for DeviceFS<D> {
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
                let offset_in_blocks = offset / D::BLOCK_SIZE;
                let offset_in_bytes = offset % D::BLOCK_SIZE;

                let block_count = (buffer.len() + offset_in_bytes).div_ceil(D::BLOCK_SIZE);

                if buffer.len() == block_count * D::BLOCK_SIZE {
                    // Read the entire buffer in one go.
                    device.device.read(buffer, offset_in_blocks, block_count)?;
                } else {
                    // FIXME: Avoid using the heap, find another solution to cover the non-block-aligned
                    // bytes (other than calling 2 device reads).
                    let mut around_buffer = alloc::vec![0; block_count * D::BLOCK_SIZE];
                    device
                        .device
                        .read(&mut around_buffer, offset_in_blocks, block_count)?;

                    buffer.copy_from_slice(
                        &around_buffer[offset_in_bytes..offset_in_bytes + buffer.len()],
                    );
                }

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
                let offset_in_blocks = offset / D::BLOCK_SIZE;
                let offset_in_bytes = offset % D::BLOCK_SIZE;

                let block_count = (buffer.len() + offset_in_bytes).div_ceil(D::BLOCK_SIZE);

                if buffer.len() == block_count * D::BLOCK_SIZE {
                    // Write the entire buffer in one go.
                    device.device.write(buffer, offset_in_blocks)?;
                } else {
                    // FIXME: Avoid using the heap and 2 devices operations, find another solution to
                    // cover the non-block-aligned bytes.
                    let mut around_buffer = alloc::vec![0; block_count * D::BLOCK_SIZE];
                    // This avoids overwriting the data around the offset.
                    device
                        .device
                        .read(&mut around_buffer, offset_in_blocks, block_count)?;
                    around_buffer[offset_in_bytes..offset_in_bytes + buffer.len()]
                        .copy_from_slice(buffer);
                    device.device.write(&around_buffer, offset_in_blocks)?;
                }

                return Ok(buffer.len());
            }
        }
        Err(super::FileError::NotFound)
    }
}
