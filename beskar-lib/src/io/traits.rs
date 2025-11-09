use crate::error::{IoError, IoErrorKind, IoResult};

/// A trait for objects that can be read from
pub trait Read {
    /// Read some bytes into the specified buffer, returning how many bytes were read
    ///
    /// # Errors
    ///
    /// Returns an error if the read operation fails
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize>;

    /// Read the exact number of bytes required to fill the buffer
    ///
    /// # Errors
    ///
    /// Returns an error if the read operation fails or if EOF is reached before the buffer is filled
    fn read_exact(&mut self, mut buf: &mut [u8]) -> IoResult<()> {
        while !buf.is_empty() {
            match self.read(buf) {
                Ok(0) => break,
                Ok(n) => buf = &mut buf[n..],
                Err(e) => return Err(e),
            }
        }
        if buf.is_empty() {
            Ok(())
        } else {
            Err(IoError::new(IoErrorKind::UnexpectedEof))
        }
    }
}

/// A trait for objects that can be written to
pub trait Write {
    /// Write some bytes from the specified buffer, returning how many bytes were written
    ///
    /// # Errors
    ///
    /// Returns an error if the write operation fails
    fn write(&mut self, buf: &[u8]) -> IoResult<usize>;

    /// Write the entire buffer
    ///
    /// # Errors
    ///
    /// Returns an error if the write operation fails
    fn write_all(&mut self, mut buf: &[u8]) -> IoResult<()> {
        while !buf.is_empty() {
            match self.write(buf) {
                Ok(0) => {
                    return Err(IoError::new(IoErrorKind::Other));
                }
                Ok(n) => buf = &buf[n..],
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    /// Flush any buffered data to the underlying device
    ///
    /// # Errors
    ///
    /// Returns an error if the flush operation fails
    fn flush(&mut self) -> IoResult<()> {
        Ok(())
    }
}

/// A trait for objects that can seek within themselves
pub trait Seek {
    /// Seek to an offset, in bytes, in the stream
    ///
    /// # Errors
    ///
    /// Returns an error if the seek operation fails
    fn seek(&mut self, pos: SeekFrom) -> IoResult<u64>;
}

/// Enumeration of possible methods to seek within an I/O object
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum SeekFrom {
    /// Sets the offset to the provided number of bytes from the start
    Start(u64),
    /// Sets the offset to the provided number of bytes from the current position
    Current(i64),
    /// Sets the offset to the provided number of bytes from the end
    End(i64),
}

/// A trait for objects which are byte-oriented sinks
pub trait BufRead: Read {
    /// Returns the contents of the internal buffer as a slice
    ///
    /// # Errors
    ///
    /// Returns an error if filling the buffer fails
    fn fill_buf(&mut self) -> IoResult<&[u8]>;

    /// Tells this buffer that `amt` bytes have been consumed from the buffer
    fn consume(&mut self, amt: usize);
}
