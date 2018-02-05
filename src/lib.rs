extern crate fibers;
extern crate futures;
extern crate miasht;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serdeconv;
#[macro_use]
extern crate trackable;

pub type Error = trackable::error::Failure;

pub mod consul;
pub mod proxy;

pub type Result<T> = std::result::Result<T, Error>;
pub type AsyncResult<T> = Box<futures::Future<Item = T, Error = Error> + Send + 'static>;
