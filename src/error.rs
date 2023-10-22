use fibers::sync::oneshot::MonitorError;
use std;
use trackable::error::{ErrorKindExt, Failed, TrackableError};

/// This crate specific `Error` type.
#[derive(Debug, Clone)]
pub struct Error(TrackableError<Failed>);
derive_traits_for_trackable_error_newtype!(Error, Failed);
impl From<std::io::Error> for Error {
    fn from(f: std::io::Error) -> Self {
        Failed.cause(f).into()
    }
}
impl From<std::net::AddrParseError> for Error {
    fn from(f: std::net::AddrParseError) -> Self {
        Failed.cause(f).into()
    }
}
impl From<std::num::ParseIntError> for Error {
    fn from(f: std::num::ParseIntError) -> Self {
        Failed.cause(f).into()
    }
}
impl From<MonitorError<Error>> for Error {
    fn from(f: MonitorError<Error>) -> Self {
        f.unwrap_or_else(|| Failed.cause("monitoring channel disconnected").into())
    }
}
