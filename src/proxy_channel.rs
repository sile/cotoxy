use slog::Logger;
use fibers::net::TcpStream;
use futures::{Future, Poll};

use Error;

#[derive(Debug)]
pub struct ProxyChannel {
    logger: Logger,
    client: TcpStream,
    server: TcpStream,
}
impl ProxyChannel {
    pub fn new(logger: Logger, client: TcpStream, server: TcpStream) -> Self {
        ProxyChannel {
            logger,
            client,
            server,
        }
    }
}
impl Future for ProxyChannel {
    type Item = ();
    type Error = Error;
    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        unimplemented!()
    }
}
