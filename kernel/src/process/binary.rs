mod elf;

use beskar_core::process::binary::BinaryResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum BinaryType {
    Elf,
}

pub struct Binary<'a> {
    input: &'a [u8],
    kind: BinaryType,
}

impl<'a> Binary<'a> {
    #[must_use]
    #[inline]
    pub const fn new(input: &'a [u8], kind: BinaryType) -> Self {
        Self { input, kind }
    }

    /// Load the binary into memory.
    ///
    /// # Errors
    ///
    /// Returns an error if the binary is not valid.
    #[must_use]
    pub fn load(&self) -> BinaryResult<LoadedBinary> {
        match self.kind {
            BinaryType::Elf => elf::load(self.input),
        }
    }
}

pub struct LoadedBinary {
    entry_point: extern "C" fn(),
    // TODO: Add information about the binary in memory to impl `Drop`
}

impl LoadedBinary {
    #[must_use]
    #[inline]
    pub const fn entry_point(&self) -> extern "C" fn() {
        self.entry_point
    }
}
