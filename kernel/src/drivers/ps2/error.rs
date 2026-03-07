use thiserror::Error;

/// Errors that may occur during PS/2 controller or keyboard operations.
#[derive(Error, Debug, Clone, Copy)]
pub enum Ps2Error {
    #[error("PS/2 controller self-test failed")]
    SelfTest,

    #[error("PS/2 controller first port test failed")]
    FirstPortTest,

    #[error("PS/2 keyboard reset failed")]
    KeyboardReset,

    #[error("PS/2 controller does not support keyboard")]
    KeyboardUnsupported,

    #[error("PS/2 controller data send failed")]
    Sending,

    #[error("PS/2 controller data receive failed")]
    Receiving,
}

impl From<Ps2Error> for beskar_core::drivers::DriverError {
    fn from(error: Ps2Error) -> Self {
        match error {
            Ps2Error::KeyboardUnsupported => Self::Absent,
            Ps2Error::FirstPortTest | Ps2Error::KeyboardReset | Ps2Error::SelfTest => Self::Invalid,
            Ps2Error::Sending | Ps2Error::Receiving => Self::Unknown,
        }
    }
}

pub type Ps2Result<T> = Result<T, Ps2Error>;
