use uefi::{
    Result,
    boot::{self, ScopedProtocol},
    proto::rng::Rng,
};

/// Returns a buffer filled with random bytes from the UEFI RNG protocol.
///
/// # Errors
///
/// Returns an error if the RNG protocol is not available or if the random number generation fails.
pub fn get_random_bytes(buffer: &mut [u8]) -> Result {
    RandomGenerator::new()?.fill_bytes(buffer)
}

/// A wrapper around the UEFI RNG protocol that provides random number generation functionality.
pub struct RandomGenerator {
    rng: ScopedProtocol<Rng>,
}

impl RandomGenerator {
    /// Creates a new `RandomGenerator` by acquiring the UEFI RNG protocol.
    ///
    /// # Errors
    ///
    /// Returns an error if the RNG protocol is not available.
    pub fn new() -> Result<Self> {
        let handle = boot::get_handle_for_protocol::<Rng>()?;
        let rng = boot::open_protocol_exclusive::<Rng>(handle)?;
        Ok(Self { rng })
    }

    #[inline]
    /// Fills the provided buffer with random bytes.
    ///
    /// # Errors
    ///
    /// Returns an error if the random number generation fails.
    pub fn fill_bytes(&mut self, buffer: &mut [u8]) -> Result {
        self.rng.get_rng(None, buffer)
    }

    /// Generate a random `u16` value.
    ///
    /// # Errors
    ///
    /// Returns an error if the random number generation fails.
    pub fn next_u16(&mut self) -> Result<u16> {
        let mut bytes = [0u8; 2];
        self.fill_bytes(&mut bytes)?;
        Ok(u16::from_le_bytes(bytes))
    }

    /// Generate a random `u32` value.
    ///
    /// # Errors
    ///
    /// Returns an error if the random number generation fails.
    pub fn next_u32(&mut self) -> Result<u32> {
        let mut bytes = [0u8; 4];
        self.fill_bytes(&mut bytes)?;
        Ok(u32::from_le_bytes(bytes))
    }

    /// Generate a random `u64` value.
    ///
    /// # Errors
    ///
    /// Returns an error if the random number generation fails.
    pub fn next_u64(&mut self) -> Result<u64> {
        let mut bytes = [0u8; 8];
        self.fill_bytes(&mut bytes)?;
        Ok(u64::from_le_bytes(bytes))
    }
}

/// A wrapper around the UEFI RNG protocol that provides random number generation functionality,
/// but ignores any errors that may occur during random number generation.
pub struct BestEffortRandomGenerator {
    rng: Option<RandomGenerator>,
}

impl Default for BestEffortRandomGenerator {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl BestEffortRandomGenerator {
    #[must_use]
    /// Creates a new `BestEffortRandomGenerator` by acquiring the UEFI RNG protocol.
    ///
    /// # Errors
    ///
    /// Returns an error if the RNG protocol is not available.
    pub fn new() -> Self {
        let rng = RandomGenerator::new().ok();
        Self { rng }
    }

    /// Fills the provided buffer with random bytes, ignoring any errors.
    pub fn fill_bytes(&mut self, buffer: &mut [u8]) {
        if let Some(rng) = &mut self.rng {
            let _ = rng.fill_bytes(buffer);
        }
    }

    #[must_use]
    /// Generate a random `u16` value, returning 0 if random number generation fails.
    pub fn next_u16(&mut self) -> u16 {
        let mut bytes = [0u8; 2];
        self.fill_bytes(&mut bytes);
        u16::from_le_bytes(bytes)
    }

    #[must_use]
    /// Generate a random `u32` value, returning 0 if random number generation fails.
    pub fn next_u32(&mut self) -> u32 {
        let mut bytes = [0u8; 4];
        self.fill_bytes(&mut bytes);
        u32::from_le_bytes(bytes)
    }

    #[must_use]
    /// Generate a random `u64` value, returning 0 if random number generation fails.
    pub fn next_u64(&mut self) -> u64 {
        let mut bytes = [0u8; 8];
        self.fill_bytes(&mut bytes);
        u64::from_le_bytes(bytes)
    }
}
