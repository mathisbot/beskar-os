use crate::error::IoResult;
use alloc::{vec, vec::Vec};
use core::fmt::Write as _;

mod traits;
pub use traits::{BufRead, Read, Seek, SeekFrom, Write};

mod file;
pub use file::File;
pub mod keyboard;
pub mod screen;

/// A buffered reader that implements `BufRead`
pub struct BufReader<R> {
    inner: R,
    buf: Vec<u8>,
    pos: usize,
    cap: usize,
}

impl<R: Read> BufReader<R> {
    #[must_use]
    #[inline]
    /// Creates a new `BufReader` with a default buffer capacity
    pub fn new(inner: R) -> Self {
        const DEFAULT_CAPACITY: usize = 8 * 1024;
        Self::with_capacity(DEFAULT_CAPACITY, inner)
    }

    #[must_use]
    #[inline]
    /// Creates a new `BufReader` with the specified buffer capacity
    pub fn with_capacity(capacity: usize, inner: R) -> Self {
        Self {
            inner,
            buf: vec![0; capacity],
            pos: 0,
            cap: 0,
        }
    }

    #[must_use]
    #[inline]
    /// Gets a reference to the underlying reader
    pub const fn get_ref(&self) -> &R {
        &self.inner
    }

    #[must_use]
    #[inline]
    /// Gets a mutable reference to the underlying reader
    pub const fn get_mut(&mut self) -> &mut R {
        &mut self.inner
    }

    /// Unwraps this `BufReader`, returning the underlying reader
    #[must_use]
    #[inline]
    pub fn into_inner(self) -> R {
        self.inner
    }
}

impl<R: Read> Read for BufReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        if self.pos >= self.cap {
            self.cap = self.inner.read(&mut self.buf)?;
            self.pos = 0;
        }

        let amt = buf.len().min(self.cap - self.pos);
        buf[..amt].copy_from_slice(&self.buf[self.pos..self.pos + amt]);
        self.pos += amt;

        Ok(amt)
    }
}

impl<R: Read> BufRead for BufReader<R> {
    fn fill_buf(&mut self) -> IoResult<&[u8]> {
        if self.pos >= self.cap {
            self.cap = self.inner.read(&mut self.buf)?;
            self.pos = 0;
        }
        Ok(&self.buf[self.pos..self.cap])
    }

    #[inline]
    fn consume(&mut self, amt: usize) {
        self.pos = self.cap.min(self.pos + amt);
    }
}

/// A buffered writer
pub struct BufWriter<W> {
    inner: W,
    buf: Vec<u8>,
    pos: usize,
}

impl<W: Write> BufWriter<W> {
    /// Creates a new `BufWriter` with a default buffer capacity
    #[must_use]
    #[inline]
    pub fn new(inner: W) -> Self {
        Self::with_capacity(8 * 1024, inner)
    }

    #[must_use]
    #[inline]
    /// Creates a new `BufWriter` with the specified buffer capacity
    pub fn with_capacity(capacity: usize, inner: W) -> Self {
        Self {
            inner,
            buf: vec![0; capacity],
            pos: 0,
        }
    }

    /// Gets a reference to the underlying writer
    #[must_use]
    #[inline]
    pub const fn get_ref(&self) -> &W {
        &self.inner
    }

    /// Gets a mutable reference to the underlying writer
    #[must_use]
    #[inline]
    pub const fn get_mut(&mut self) -> &mut W {
        &mut self.inner
    }

    /// Unwraps this `BufWriter`, returning the underlying writer
    ///
    /// # Errors
    ///
    /// Returns an error if flushing the buffer fails
    #[inline]
    pub fn into_inner(mut self) -> IoResult<W> {
        self.flush()?;
        Ok(self.inner)
    }
}

impl<W: Write> Write for BufWriter<W> {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        if self.pos + buf.len() > self.buf.len() {
            self.flush()?;
            if buf.len() >= self.buf.len() {
                return self.inner.write(buf);
            }
        }

        let amt = buf.len().min(self.buf.len() - self.pos);
        self.buf[self.pos..self.pos + amt].copy_from_slice(&buf[..amt]);
        self.pos += amt;

        Ok(amt)
    }

    fn flush(&mut self) -> IoResult<()> {
        if self.pos > 0 {
            self.inner.write_all(&self.buf[..self.pos])?;
            self.pos = 0;
        }
        self.inner.flush()
    }
}

/// A convenience type for reading from strings
pub struct Cursor<T> {
    inner: T,
    pos: u64,
}

impl<T> Cursor<T> {
    /// Creates a new cursor wrapping the provided data
    #[must_use]
    #[inline]
    pub const fn new(inner: T) -> Self {
        Self { inner, pos: 0 }
    }

    /// Gets the current position of the cursor
    #[must_use]
    #[inline]
    pub const fn position(&self) -> u64 {
        self.pos
    }

    /// Sets the position of the cursor
    #[inline]
    pub const fn set_position(&mut self, pos: u64) {
        self.pos = pos;
    }

    /// Gets a reference to the underlying data
    #[must_use]
    #[inline]
    pub const fn get_ref(&self) -> &T {
        &self.inner
    }

    /// Gets a mutable reference to the underlying data
    #[must_use]
    #[inline]
    pub const fn get_mut(&mut self) -> &mut T {
        &mut self.inner
    }

    /// Consumes the cursor, returning the underlying data
    #[must_use]
    #[inline]
    pub fn into_inner(self) -> T {
        self.inner
    }
}

impl<T: AsRef<[u8]>> Read for Cursor<T> {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        let data = self.inner.as_ref();

        let pos = usize::try_from(self.pos).unwrap();
        if pos >= data.len() {
            return Ok(0);
        }

        let amt = buf.len().min(data.len() - pos);
        buf[..amt].copy_from_slice(&data[pos..pos + amt]);
        self.pos += u64::try_from(amt).unwrap();

        Ok(amt)
    }
}

impl<T: AsRef<[u8]>> Seek for Cursor<T> {
    fn seek(&mut self, style: SeekFrom) -> IoResult<u64> {
        let data_len = u64::try_from(self.inner.as_ref().len()).unwrap();
        self.pos = match style {
            SeekFrom::Start(n) => n,
            SeekFrom::Current(n) => {
                if n >= 0 {
                    self.pos.saturating_add(n.cast_unsigned())
                } else {
                    self.pos.saturating_sub((-n).cast_unsigned())
                }
            }
            SeekFrom::End(n) => {
                if n >= 0 {
                    data_len.saturating_add(n.cast_unsigned())
                } else {
                    data_len.saturating_sub((-n).cast_unsigned())
                }
            }
        };
        Ok(self.pos)
    }
}

#[inline]
#[doc(hidden)]
/// Print a message to the console
///
/// # Panics
///
/// Panics if writing to stdout fails
pub fn print(args: core::fmt::Arguments) {
    const STDOUT_FILE: &str = "/dev/stdout";

    // TODO: Cache the stdout file handle
    let mut file = File::open(STDOUT_FILE).expect("failed to open stdout");
    file.write_fmt(args).expect("failed to write to stdout");
}

#[macro_export]
/// Print a message to the console with a newline
macro_rules! println {
    () => {
        $crate::io::print(format_args!("\n"));
    };
    ($fmt:expr) => {
        $crate::io::print(format_args!(concat!($fmt, "\n")));
    };
    ($fmt:expr, $($arg:tt)*) => {
        $crate::io::print(format_args!(concat!($fmt, "\n"), $($arg)*));
    };
}
pub use println;
