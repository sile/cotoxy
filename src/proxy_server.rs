use std::net::SocketAddr;
use fibers::Spawn;
use fibers::net::{TcpListener, TcpStream};
use fibers::net::futures::{Connect, TcpListenerBind};
use fibers::net::streams::Incoming;
use futures::{Async, Future, Poll, Stream};
use slog::{Discard, Logger};
use trackable::error::Failed;

use {AsyncResult, Error};
use consul::{ConsulClient, ConsulClientBuilder, ServiceNode};
use proxy_channel::ProxyChannel;

#[derive(Debug, Clone)]
pub struct ProxyServerBuider {
    logger: Logger,
    bind_addr: SocketAddr,
    consul: ConsulClientBuilder,
    service_port: Option<u16>,
}
impl ProxyServerBuider {
    pub const DEFAULT_BIND_ADDR: &'static str = "0.0.0.0:17382";

    pub fn new(service: &str) -> Self {
        ProxyServerBuider {
            logger: Logger::root(Discard, o!()),
            bind_addr: Self::DEFAULT_BIND_ADDR.parse().expect("Never fails"),
            consul: ConsulClientBuilder::new(service),
            service_port: None,
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

    pub fn service_port(&mut self, port: u16) -> &mut Self {
        self.service_port = Some(port);
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
            service_port: self.service_port,
        }
    }
}

pub struct ProxyServer<S> {
    logger: Logger,
    spawner: S,
    consul: ConsulClient,
    bind: Option<TcpListenerBind>,
    incoming: Option<Incoming>,
    service_port: Option<u16>,
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
            self.bind = None;
        }
        if let Some(ref mut incoming) = self.incoming {
            if let Async::Ready(Some((client, addr))) =
                track!(incoming.poll().map_err(Error::from))?
            {
                let logger = self.logger.new(o!("client" => addr.to_string()));
                let error_logger = logger.clone();
                let server =
                    SelectServer::new(self.logger.clone(), &self.consul, self.service_port);
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

struct SelectServer {
    logger: Logger,
    collect_candidates: Option<AsyncResult<Vec<ServiceNode>>>,
    connect: Option<Connect>,
    server: SocketAddr,
    candidates: Vec<ServiceNode>,
    service_port: Option<u16>,
}
impl SelectServer {
    fn new(logger: Logger, consul: &ConsulClient, service_port: Option<u16>) -> Self {
        SelectServer {
            logger,
            collect_candidates: Some(consul.find_candidates()),
            connect: None,
            server: "127.0.0.1:80".parse().expect("Never fails"), // dummy
            candidates: Vec::new(),
            service_port,
        }
    }
}
impl Future for SelectServer {
    type Item = (TcpStream, SocketAddr);
    type Error = Error;
    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        if let Async::Ready(Some(candidates)) = track!(self.collect_candidates.poll())? {
            debug!(self.logger, "Candidate servers: {:?}", candidates);
            self.candidates = candidates;
            self.candidates.reverse();
            self.collect_candidates = None;
        }
        if self.collect_candidates.is_none() && self.connect.is_none() {
            let candidate = track_assert_some!(self.candidates.pop(), Failed);
            debug!(self.logger, "Next candidate: {:?}", candidate);
            self.server = candidate.socket_addr();
            if let Some(port) = self.service_port {
                self.server.set_port(port);
            }
            self.connect = Some(TcpStream::connect(self.server));
        }
        match track!(self.connect.poll().map_err(Error::from)) {
            Err(e) => {
                warn!(self.logger, "Cannot connect to a server: {}", e);
                self.connect = None;
                self.poll()
            }
            Ok(Async::Ready(Some(stream))) => {
                info!(self.logger, "Connected to the server {}", self.server);
                Ok(Async::Ready((stream, self.server)))
            }
            _ => Ok(Async::NotReady),
        }
    }
}
