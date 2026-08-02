//! Networking.
//!
//! Only ICMP echo is exposed for now: the kernel has no socket layer yet, so
//! there is nothing to bind or connect to.

use crate::sys::sc_ping;
use beskar_core::{syscall::consts, time::Duration};
pub use core::net::Ipv4Addr;

/// Why an echo request did not produce a reply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PingError {
    /// No network interface is up.
    NoInterface,
    /// The destination has no route, or did not answer before the timeout.
    Unreachable,
    /// The request was malformed.
    Invalid,
}

impl core::fmt::Display for PingError {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        let message = match self {
            Self::NoInterface => "no network interface",
            Self::Unreachable => "destination unreachable",
            Self::Invalid => "invalid request",
        };
        f.write_str(message)
    }
}

/// Send one ICMP echo request to `addr` and wait for its reply.
///
/// A `timeout` of `None` selects the kernel default.
///
/// # Errors
///
/// See [`PingError`].
pub fn ping(addr: Ipv4Addr, timeout: Option<Duration>) -> Result<Duration, PingError> {
    let timeout_millis = timeout.map_or(0, |timeout| timeout.total_millis());

    match sc_ping(addr.to_bits(), timeout_millis) {
        micros if micros >= 0 => Ok(Duration::from_micros(micros.cast_unsigned())),
        consts::PING_NO_INTERFACE => Err(PingError::NoInterface),
        consts::PING_UNREACHABLE => Err(PingError::Unreachable),
        _ => Err(PingError::Invalid),
    }
}
