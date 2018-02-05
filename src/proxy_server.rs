use std::net::SocketAddr;
use fibers::Spawn;
use fibers::net::TcpListener;
use fibers::net::futures::TcpListenerBind;
use fibers::net::streams::Incoming;
use futures::{Async, Future, Poll, Stream};
use slog::{Discard, Logger};

use Error;
use consul::ConsulConfig;
use proxy_channel::ProxyChannel;

#[derive(Debug, Clone)]
pub struct ProxyServerBuider {
    logger: Logger,
    bind_addr: SocketAddr,
    consul: ConsulConfig,
}
impl ProxyServerBuider {
    pub const DEFAULT_BIND_ADDR: &'static str = "0.0.0.0:17382";

    pub fn new(service: &str) -> Self {
        ProxyServerBuider {
            logger: Logger::root(Discard, o!()),
            bind_addr: Self::DEFAULT_BIND_ADDR.parse().expect("Never fails"),
            consul: ConsulConfig::new(service),
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

    pub fn finish<S: Spawn>(&self, spawner: S) -> ProxyServer<S> {
        ProxyServer {
            logger: self.logger.clone(),
            spawner,
            consul: self.consul.clone(),
            bind: Some(TcpListener::bind(self.bind_addr)),
            incoming: None,
        }
    }
}

pub struct ProxyServer<S> {
    logger: Logger,
    spawner: S,
    consul: ConsulConfig,
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
                // let service = self.service.clone();
                self.spawner.spawn(
                    track_err!(client)
                        .and_then(move |client| {
                            let server = panic!();
                            track_err!(ProxyChannel::new(logger, client, server))
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
