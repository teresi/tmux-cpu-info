use std::time::Duration;
use std::time::SystemTimeError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CpuBarError {
    #[error("Not implemented")]
    NotImplemented,

    #[error("Access denied: insufficient permissions.")]
    PermissionDenied,

    #[error("I/O error: {0}")]
    IOError(String),

    #[error("File Does Not Exist at {0}")]
    FileNotFound(String),

    #[error("Server not running")]
    ServerNotFound,

    #[error("Another client is starting the server")]
    ClientBusy,

    #[error("Clock is before the UNIX EPOCH")]
    TimeSinceEpoch(Duration),
}

impl From<SystemTimeError> for CpuBarError {
    fn from(terr: SystemTimeError) -> Self {
        CpuBarError::TimeSinceEpoch(terr.duration())
    }
}
