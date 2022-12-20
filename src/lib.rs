//! A TCP proxy using [Consul][consul] for service discovery.
//!
//! [consul]: https://www.consul.io/
#![warn(missing_docs)]
extern crate fibers;
extern crate futures;
extern crate miasht;
extern crate serde;
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
    };
}

pub use consul::ConsulSettings;
pub use error::Error;
pub use proxy_server::{ProxyServer, ProxyServerBuilder};

mod consul;
mod error;
mod http;
mod proxy_channel;
mod proxy_server;

/// This crate specific `Result` type.
pub type Result<T> = std::result::Result<T, Error>;

type AsyncResult<T> = Box<dyn futures::Future<Item = T, Error = Error> + Send + 'static>;
