//! A custom, realy simple read-only file system suitable for e.g. ramdisks.

use super::FileSystem;
use alloc::vec::Vec;

#[derive(Default, Debug, Clone, Eq, PartialEq)]
#[repr(C, packed)]
pub struct RawHeader {
    /// Should be a 32 byte long ASCII name.
    name: [u8; 32],
    size: usize,
}

impl RawHeader {
    #[must_use]
    #[inline]
    /// Creates a new `RawHeader` with the given size and name.
    pub const fn new(size: usize, name: [u8; 32]) -> Self {
        Self { name, size }
    }

    #[must_use]
    #[inline]
    /// Returns the size of the file.
    pub const fn size(&self) -> usize {
        self.size
    }

    #[must_use]
    #[inline]
    /// Returns the name of the file.
    pub const fn name(&self) -> &[u8; 32] {
        &self.name
    }
}

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct FileInfo {
    /// Should be a 32 byte long ASCII name.
    name: [u8; 32],
    size: usize,
    offset: usize,
}

impl FileInfo {
    #[must_use]
    #[inline]
    /// Creates a new `RawHeader` with the given size and name.
    pub const fn new(raw_header: &RawHeader, offset: usize) -> Self {
        Self {
            name: raw_header.name,
            size: raw_header.size,
            offset,
        }
    }

    #[must_use]
    #[inline]
    /// Returns the size of the file.
    pub const fn size(&self) -> usize {
        self.size
    }

    #[must_use]
    #[inline]
    /// Returns the name of the file as a string slice.
    pub fn name(&self) -> &str {
        core::str::from_utf8(&self.name)
            .unwrap()
            .trim_end_matches('\0')
    }
}

#[derive(Debug, Clone, Eq, PartialEq, thiserror::Error)]
pub enum InMemoryFSError {
    #[error("Buffer is too small")]
    BufferTooSmall,
    #[error("Invalid header size")]
    InvalidHeaderSize,
    #[error("Invalid header name")]
    InvalidHeaderName,
}

#[derive(Default)]
/// A pass-through file system for device files.
pub struct InMemoryFS<'a> {
    raw: &'a [u8],
    infos: Vec<FileInfo>,
}

impl<'a> InMemoryFS<'a> {
    #[inline]
    /// Creates a new `InMemoryFS` instance with the given data.
    pub fn new(data: &'a [u8]) -> Result<Self, InMemoryFSError> {
        let mut infos = Vec::new();

        if !data.is_empty() {
            let mut cursor = 0;
            while cursor < data.len() {
                if data.len() < cursor.saturating_add(size_of::<RawHeader>()) {
                    return Err(InMemoryFSError::BufferTooSmall);
                }

                let raw_header = unsafe {
                    // SAFETY: We made sure that the buffer is large enough.
                    data[cursor..]
                        .as_ptr()
                        .try_cast_aligned::<RawHeader>()
                        .unwrap()
                        .as_ref()
                        .unwrap()
                };

                if raw_header.size().saturating_add(cursor) > data.len() {
                    return Err(InMemoryFSError::InvalidHeaderSize);
                }
                if core::str::from_utf8(raw_header.name()).is_err() {
                    return Err(InMemoryFSError::InvalidHeaderName);
                }

                infos.push(FileInfo::new(raw_header, cursor));

                cursor += size_of::<RawHeader>() + raw_header.size();
            }
        }

        Ok(Self { raw: data, infos })
    }
}

impl FileSystem for InMemoryFS<'_> {
    fn close(&mut self, _path: super::Path) -> super::FileResult<()> {
        Ok(())
    }

    #[inline]
    fn create(&mut self, _path: super::Path) -> super::FileResult<()> {
        // InMemoryFS does not support creating files
        Err(super::FileError::UnsupportedOperation)
    }

    #[inline]
    fn delete(&mut self, _path: super::Path) -> super::FileResult<()> {
        // InMemoryFS does not support deleting files
        Err(super::FileError::UnsupportedOperation)
    }

    fn exists(&mut self, path: super::Path) -> super::FileResult<bool> {
        Ok(self.infos.iter().any(|file| file.name() == path.as_str()))
    }

    fn open(&mut self, _path: super::Path) -> super::FileResult<()> {
        Ok(())
    }

    fn read(
        &mut self,
        path: super::Path,
        buffer: &mut [u8],
        offset: usize,
    ) -> super::FileResult<usize> {
        // Find the device associated with the given path.
        let Some(file) = self.infos.iter().find(|file| file.name() == path.as_str()) else {
            return Err(super::FileError::NotFound);
        };

        let read_bytes = file.size().saturating_sub(offset).min(buffer.len());

        let src = {
            let start_offset = file.offset + offset;
            &self.raw[start_offset..start_offset + read_bytes]
        };
        let dst = &mut buffer[..read_bytes];

        dst.copy_from_slice(src);

        Ok(read_bytes)
    }

    fn write(
        &mut self,
        _path: super::Path,
        _buffer: &[u8],
        _offset: usize,
    ) -> super::FileResult<usize> {
        // InMemoryFS does not support writing to files
        Err(super::FileError::UnsupportedOperation)
    }

    fn metadata(&mut self, path: super::Path) -> super::FileResult<super::FileMetadata> {
        let Some(file) = self.infos.iter().find(|file| file.name() == path.as_str()) else {
            return Err(super::FileError::NotFound);
        };

        Ok(super::FileMetadata::new(file.size(), super::FileType::File))
    }

    fn read_dir(&mut self, path: super::Path) -> super::FileResult<Vec<super::PathBuf>> {
        if path.as_str() != "/" {
            return Err(super::FileError::NotFound);
        }

        Ok(self
            .infos
            .iter()
            .map(|file| super::PathBuf::new(file.name()))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs::Path;

    #[test]
    fn test_in_memory_fs() {
        let data = [
            RawHeader::new(
                0,
                *b"file1\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
            ),
            RawHeader::new(
                0,
                *b"file2\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
            ),
        ];
        let mut fs = InMemoryFS::new(unsafe {
            core::slice::from_raw_parts(
                data.as_ptr() as *const u8,
                data.len() * size_of::<RawHeader>(),
            )
        })
        .unwrap();

        assert!(fs.exists(Path("file1")).unwrap());
        assert!(fs.exists(Path("file2")).unwrap());
        assert!(!fs.exists(Path("file3")).unwrap());

        let mut buffer = [0; 5];

        let bytes_read = fs.read(Path("file1"), &mut buffer, 0).unwrap();
        assert_eq!(bytes_read, 0);
    }
}
