use beskar_core::process::ProcessId;
use storage::{
    BlockDevice,
    fs::{FileError, FileResult, FileSystem, Path, PathBuf},
    vfs::{Vfs, VfsHelper},
};

struct MockBlockDevice {
    data: Vec<u8>,
}

impl MockBlockDevice {
    fn new(size: usize) -> Self {
        MockBlockDevice {
            data: vec![0; size],
        }
    }
}

impl BlockDevice for MockBlockDevice {
    const BLOCK_SIZE: usize = 1;

    fn read(
        &mut self,
        dst: &mut [u8],
        offset: usize,
        count: usize,
    ) -> Result<(), storage::DeviceError> {
        if offset + count > self.data.len() {
            return Err(storage::DeviceError::OutOfBounds);
        }
        dst.copy_from_slice(&self.data[offset..offset + count]);
        Ok(())
    }

    fn write(&mut self, src: &[u8], offset: usize) -> Result<(), storage::DeviceError> {
        if offset + src.len() > self.data.len() {
            return Err(storage::DeviceError::OutOfBounds);
        }
        self.data[offset..offset + src.len()].copy_from_slice(src);
        Ok(())
    }
}

struct MockFile {
    name: String,
    start: usize,
    length: usize,
}

struct MockFS<B: BlockDevice> {
    device: B,
    files: Vec<MockFile>,
}

impl<B: BlockDevice> FileSystem for MockFS<B> {
    fn read(&mut self, path: Path, buffer: &mut [u8], offset: usize) -> FileResult<usize> {
        for file in &self.files {
            if file.name == path.as_str() {
                let bytes_to_read =
                    core::cmp::min(buffer.len(), file.length.saturating_add(offset));

                let offset_in_blocks = (file.start + offset) / B::BLOCK_SIZE;
                let offset_in_block = (file.start + offset) % B::BLOCK_SIZE;

                let block_count = (offset_in_block + bytes_to_read).div_ceil(B::BLOCK_SIZE);
                let mut block_buffer = vec![0; block_count * B::BLOCK_SIZE];

                // Read the data from the block device.
                self.device
                    .read(&mut block_buffer, offset_in_blocks, block_count)
                    .map_err(|_| FileError::Io)?;

                buffer[..bytes_to_read].copy_from_slice(
                    &block_buffer[offset_in_block..offset_in_block + bytes_to_read],
                );

                return Ok(bytes_to_read);
            }
        }
        Err(FileError::NotFound)
    }

    fn write(&mut self, path: Path, buffer: &[u8], offset: usize) -> FileResult<usize> {
        for file in &mut self.files {
            if file.name == path.as_str() {
                let bytes_to_write =
                    core::cmp::min(buffer.len(), file.length.saturating_sub(offset));

                let offset_in_blocks = (file.start + offset) / B::BLOCK_SIZE;
                let offset_in_block = (file.start + offset) % B::BLOCK_SIZE;

                let block_count = (offset_in_block + bytes_to_write).div_ceil(B::BLOCK_SIZE);
                let mut block_buffer = vec![0; block_count * B::BLOCK_SIZE];

                block_buffer[offset_in_block..offset_in_block + bytes_to_write]
                    .copy_from_slice(&buffer[..bytes_to_write]);

                // Write the data to the block device.
                // FIXME: This overwrites the whole block (other files data will be overwitten with zeroes).
                self.device
                    .write(&block_buffer, offset_in_blocks)
                    .map_err(|_| FileError::Io)?;

                return Ok(bytes_to_write);
            }
        }
        Err(FileError::NotFound)
    }

    fn close(&mut self, path: Path) -> FileResult<()> {
        for file in &self.files {
            if file.name == path.as_str() {
                // In a real filesystem, this would do something meaningful.
                return Ok(());
            }
        }
        Err(FileError::NotFound)
    }

    fn create(&mut self, path: Path) -> FileResult<()> {
        if self.files.iter().any(|file| file.name == path.as_str()) {
            return Err(FileError::AlreadyExists);
        }
        let file_count = self.files.len();
        self.files.push(MockFile {
            name: String::from(path.as_str()),
            start: file_count * Self::FILE_SIZE,
            length: Self::FILE_SIZE,
        });
        Ok(())
    }

    fn delete(&mut self, path: Path) -> FileResult<()> {
        if let Some(pos) = self
            .files
            .iter()
            .position(|file| file.name == path.as_str())
        {
            self.files.remove(pos);
            return Ok(());
        }
        Err(FileError::NotFound)
    }

    fn exists(&mut self, path: Path) -> FileResult<bool> {
        for file in &self.files {
            if file.name == path.as_str() {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn open(&mut self, path: Path) -> FileResult<()> {
        for file in &self.files {
            if file.name == path.as_str() {
                // In a real filesystem, this would do something meaningful.
                return Ok(());
            }
        }
        Err(FileError::NotFound)
    }
}

impl<B: BlockDevice> MockFS<B> {
    /// Default size for mock files.
    const FILE_SIZE: usize = 32;

    fn new(device: B) -> Self {
        MockFS {
            device,
            files: Vec::new(),
        }
    }
}

struct MockVFSHelper;

impl VfsHelper for MockVFSHelper {
    fn get_current_process_id() -> ProcessId {
        // Mock implementation, returning a dummy process ID.
        unsafe { ProcessId::from_raw(0) }
    }
}

#[test]
fn mock() {
    static VFS: Vfs<MockVFSHelper> = Vfs::new();

    // Initialize the VFS with a mock filesystem.
    let device = MockBlockDevice::new(1024);
    let fs = MockFS::new(device);
    VFS.mount(PathBuf::new("/"), Box::new(fs));

    // Create files.
    VFS.create(Path::from("/test.txt")).unwrap();
    VFS.create(Path::from("/sw.txt")).unwrap();

    // Check if the files exist.
    assert!(VFS.exists(Path::from("/test.txt")).unwrap());
    assert!(VFS.exists(Path::from("/sw.txt")).unwrap());
    assert!(!VFS.exists(Path::from("/nonexistent.txt")).unwrap());

    // Open the files.
    let handle1 = VFS.open(Path::from("/test.txt")).unwrap();
    assert!(VFS.open(Path::from("/test.txt")).is_err());
    let handle2 = VFS.open(Path::from("/sw.txt")).unwrap();

    // Write to the files.
    let data1 = b"Hello, world!";
    assert_eq!(VFS.write(handle1, data1, 0).unwrap(), 13);
    let data2 = b"May the force be with you!";
    assert_eq!(VFS.write(handle2, data2, 0).unwrap(), 26);

    // Read from the files.
    let mut buffer1 = [0; 13];
    assert_eq!(VFS.read(handle1, &mut buffer1, 0).unwrap(), 13);
    assert_eq!(&buffer1, data1);
    let mut buffer2 = [0; 26];
    assert_eq!(VFS.read(handle2, &mut buffer2, 0).unwrap(), 26);
    assert_eq!(&buffer2, data2);

    // Try to delete the files.
    assert!(VFS.delete(Path::from("/test.txt")).is_err());
    assert!(VFS.delete(Path::from("/sw.txt")).is_err());

    // Close the files.
    VFS.close(handle1).unwrap();
    assert!(VFS.close(handle1).is_err());
    VFS.close(handle2).unwrap();

    // Check if the files exist.
    assert!(VFS.exists(Path::from("/test.txt")).unwrap());
    assert!(VFS.exists(Path::from("/test.txt")).unwrap());

    // Delete the file.
    VFS.delete(Path::from("/test.txt")).unwrap();
    VFS.delete(Path::from("/sw.txt")).unwrap();
    assert!(!VFS.exists(Path::from("/test.txt")).unwrap());
    assert!(VFS.delete(Path::from("/test.txt")).is_err());
}
