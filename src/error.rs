use std;
use fibers::sync::oneshot::MonitorError;
use handy_async::future::Phase;
use sloggers;
use trackable::error::{ErrorKindExt, Failed, TrackableError};

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
impl From<sloggers::Error> for Error {
    fn from(f: sloggers::Error) -> Self {
        Failed.cause(f).into()
    }
}
impl From<MonitorError<Error>> for Error {
    fn from(f: MonitorError<Error>) -> Self {
        f.unwrap_or_else(|| Failed.cause("monitoring channel disconnected").into())
    }
}
impl<A, B, C, D, E> From<Phase<A, B, C, D, E>> for Error
where
    Self: From<A> + From<B> + From<C> + From<D> + From<E>,
{
    fn from(f: Phase<A, B, C, D, E>) -> Self {
        match f {
            Phase::A(e) => track!(Error::from(e), "Phase A"),
            Phase::B(e) => track!(Error::from(e), "Phase B"),
            Phase::C(e) => track!(Error::from(e), "Phase C"),
            Phase::D(e) => track!(Error::from(e), "Phase D"),
            Phase::E(e) => track!(Error::from(e), "Phase E"),
        }
    }
}
