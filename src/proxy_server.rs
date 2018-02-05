use std::net::SocketAddr;
use fibers::Spawn;
use fibers::net::{TcpListener, TcpStream};
use fibers::net::futures::TcpListenerBind;
use fibers::net::streams::Incoming;
use futures::{Async, Future, Poll, Stream};
use slog::{Discard, Logger};

use Error;
use consul::{ConsulClient, ConsulClientBuilder};
use proxy_channel::ProxyChannel;

#[derive(Debug, Clone)]
pub struct ProxyServerBuider {
    logger: Logger,
    bind_addr: SocketAddr,
    consul: ConsulClientBuilder,
}
impl ProxyServerBuider {
    pub const DEFAULT_BIND_ADDR: &'static str = "0.0.0.0:17382";

    pub fn new(service: &str) -> Self {
        ProxyServerBuider {
            logger: Logger::root(Discard, o!()),
            bind_addr: Self::DEFAULT_BIND_ADDR.parse().expect("Never fails"),
            consul: ConsulClientBuilder::new(service),
        }
    }

    pub fn logger(&mut self, logger: Logger) -> &mut Self {
        self.logger = logger;
        self
    }

    pub fn bind_addr(&mut self, addr: SocketAddr) -> &mut Self {
        self.bind_addr = addr;
        self
    }

    pub fn consul(&mut self) -> &mut ConsulClientBuilder {
        &mut self.consul
    }

    pub fn finish<S: Spawn>(&self, spawner: S) -> ProxyServer<S> {
        ProxyServer {
            logger: self.logger.clone(),
            spawner,
            consul: self.consul.finish(),
            bind: Some(TcpListener::bind(self.bind_addr)),
            incoming: None,
        }
    }
}

pub struct ProxyServer<S> {
    logger: Logger,
    spawner: S,
    consul: ConsulClient,
    bind: Option<TcpListenerBind>,
    incoming: Option<Incoming>,
}
impl<S: Spawn> ProxyServer<S> {
    pub fn new(spawner: S, service: &str) -> Self {
        ProxyServerBuider::new(service).finish(spawner)
    }
}
impl<S: Spawn> Future for ProxyServer<S> {
    type Item = ();
    type Error = Error;
    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        if let Async::Ready(Some(listener)) = track!(self.bind.poll().map_err(Error::from))? {
            info!(self.logger, "Proxy server started");
            self.incoming = Some(listener.incoming());
        }
        if let Some(ref mut incoming) = self.incoming {
            if let Async::Ready(Some((client, addr))) =
                track!(incoming.poll().map_err(Error::from))?
            {
                let logger = self.logger.new(o!("client" => addr.to_string()));
                let error_logger = logger.clone();
                let server = SelectServer::new(&self.consul);
                self.spawner.spawn(
                    track_err!(client)
                        .and_then(move |client| {
                            track_err!(server).and_then(move |(server, addr)| {
                                let logger = logger.new(o!("server" => addr.to_string()));
                                track_err!(ProxyChannel::new(logger, client, server))
                            })
                        })
                        .map_err(move |e| {
                            error!(error_logger, "{}", e);
                        }),
                );
            }
        }
        Ok(Async::NotReady)
    }
}

struct SelectServer {}
impl SelectServer {
    fn new(consul: &ConsulClient) -> Self {
        panic!()
    }
}
impl Future for SelectServer {
    type Item = (TcpStream, SocketAddr);
    type Error = Error;
    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        panic!()
    }
}
