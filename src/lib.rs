extern crate fibers;
extern crate futures;
extern crate handy_async;
extern crate miasht;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serdeconv;
#[macro_use]
extern crate slog;
extern crate sloggers;
#[macro_use]
extern crate trackable;
extern crate url;

macro_rules! track_err {
    ($future:expr) => {
        $future.map_err(|e| track!(::Error::from(e)))
    }
}

pub use error::Error;
pub use proxy_server::{ProxyServer, ProxyServerBuider};

mod consul;
mod error;
mod proxy_channel;
mod proxy_server;

pub type Result<T> = std::result::Result<T, Error>;
pub type AsyncResult<T> = Box<futures::Future<Item = T, Error = Error> + Send + 'static>;
