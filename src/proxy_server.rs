use std::net::SocketAddr;
use std::time::Duration;
use fibers::Spawn;
use fibers::net::{TcpListener, TcpStream};
use fibers::net::futures::{Connect, TcpListenerBind};
use fibers::net::streams::Incoming;
use fibers::time::timer::{TimeoutAfter, TimerExt};
use futures::{Async, Future, Poll, Stream};
use slog::{Discard, Logger};
use trackable::error::Failed;

use {AsyncResult, Error};
use consul::{ConsulClient, ConsulClientBuilder, ServiceNode};
use proxy_channel::ProxyChannel;

/// A builder for `ProxyServer`.
#[derive(Debug, Clone)]
pub struct ProxyServerBuilder {
    logger: Logger,
    bind_addr: SocketAddr,
    consul: ConsulClientBuilder,
    service_port: Option<u16>,
    connect_timeout: Duration,
}
impl ProxyServerBuilder {
    /// The default address to which the proxy server bind.
    pub const DEFAULT_BIND_ADDR: &'static str = "0.0.0.0:17382";

    /// The default timeout of a TCP connect operation.
    pub const DEFAULT_CONNECT_TIMEOUT_MS: u64 = 1000;

    /// Makes a new `ProxyServerBuilder` for the given service.
    pub fn new(service: &str) -> Self {
        ProxyServerBuilder {
            logger: Logger::root(Discard, o!()),
            bind_addr: Self::DEFAULT_BIND_ADDR.parse().expect("Never fails"),
            consul: ConsulClientBuilder::new(service),
            service_port: None,
            connect_timeout: Duration::from_millis(Self::DEFAULT_CONNECT_TIMEOUT_MS),
        }
    }

    /// Sets the logger of the server.
    ///
    /// The default value is `Logger::root(Discard, o!())`.
    pub fn logger(&mut self, logger: Logger) -> &mut Self {
        self.logger = logger;
        self
    }

    /// Sets the address to which the server bind.
    ///
    /// The default value is `ProxyServerBuilder::DEFAULT_BIND_ADDR`.
    pub fn bind_addr(&mut self, addr: SocketAddr) -> &mut Self {
        self.bind_addr = addr;
        self
    }

    /// Sets the port number of the service handled by the proxy server.
    ///
    /// If omitted, the value of the selected node's `ServicePort` field registered in Consul will be used.
    pub fn service_port(&mut self, port: u16) -> &mut Self {
        self.service_port = Some(port);
        self
    }

    /// Sets the timeout of a TCP connect operation.
    ///
    /// The default value is `Duration::from_millis(ProxyServerBuilder::DEFAULT_CONNECT_TIMEOUT_MS)`.
    pub fn connect_timeout(&mut self, timeout: Duration) -> &mut Self {
        self.connect_timeout = timeout;
        self
    }

    /// Returns the mutable reference to `ConsulClientBuilder`.
    pub fn consul(&mut self) -> &mut ConsulClientBuilder {
        &mut self.consul
    }

    /// Builds a new proxy server with the specified settings.
    pub fn finish<S: Spawn>(&self, spawner: S) -> ProxyServer<S> {
        ProxyServer {
            logger: self.logger.clone(),
            spawner,
            consul: self.consul.finish(),
            bind: Some(TcpListener::bind(self.bind_addr)),
            incoming: None,
            service_port: self.service_port,
            connect_timeout: self.connect_timeout,
        }
    }
}

/// Proxy server.
pub struct ProxyServer<S> {
    logger: Logger,
    spawner: S,
    consul: ConsulClient,
    bind: Option<TcpListenerBind>,
    incoming: Option<Incoming>,
    service_port: Option<u16>,
    connect_timeout: Duration,
}
impl<S: Spawn> ProxyServer<S> {
    /// Makes a new `ProxyServer` for the given service with the default settings.
    ///
    /// This is equivalent to `ProxyServerBuilder::new(service).finish(spawner)`.
    pub fn new(spawner: S, service: &str) -> Self {
        ProxyServerBuilder::new(service).finish(spawner)
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
                let server = SelectServer::new(
                    logger.clone(),
                    &self.consul,
                    self.service_port,
                    self.connect_timeout,
                );
                self.spawner.spawn(
                    track_err!(client)
                        .and_then(move |client| {
                            track_err!(server).and_then(move |(server, addr)| {
                                let logger = logger.new(o!("server" => addr.to_string()));
                                track_err!(ProxyChannel::new(logger, client, server))
                            })
                        })
                        .map_err(move |e| {
                            error!(error_logger, "Proxy channel terminated abnormally: {}", e);
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
    connect: Option<TimeoutAfter<Connect>>,
    candidates: Vec<ServiceNode>,
    server: Option<ServiceNode>,
    service_port: Option<u16>,
    connect_timeout: Duration,
}
impl SelectServer {
    fn new(
        logger: Logger,
        consul: &ConsulClient,
        service_port: Option<u16>,
        connect_timeout: Duration,
    ) -> Self {
        SelectServer {
            logger,
            collect_candidates: Some(consul.find_candidates()),
            connect: None,
            candidates: Vec::new(),
            server: None,
            service_port,
            connect_timeout,
        }
    }
}
impl Future for SelectServer {
    type Item = (TcpStream, SocketAddr);
    type Error = Error;
    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        if let Async::Ready(Some(candidates)) = track!(self.collect_candidates.poll())? {
            debug!(self.logger, "Candidates: {:?}", candidates);
            self.candidates = candidates;
            self.candidates.reverse();
            self.collect_candidates = None;
        }
        if self.collect_candidates.is_none() && self.connect.is_none() {
            let candidate = track_assert_some!(
                self.candidates.pop(),
                Failed,
                "No available service servers"
            );
            let addr = candidate.socket_addr(self.service_port);
            debug!(self.logger, "Next candidate server is {}", addr);
            self.connect = Some(TcpStream::connect(addr).timeout_after(self.connect_timeout));
            self.server = Some(candidate);
        }
        match self.connect.poll() {
            Err(e) => {
                let server = self.server.take().expect("Never fails");
                warn!(
                    self.logger,
                    "Cannot connect to the server {}; {}",
                    server.socket_addr(self.service_port),
                    e.map(|e| e.to_string())
                        .unwrap_or_else(|| "Connection timeout".to_owned())
                );
                self.connect = None;
                self.poll()
            }
            Ok(Async::Ready(Some(stream))) => {
                let server = self.server.as_ref().take().expect("Never fails");
                let addr = server.socket_addr(self.service_port);
                info!(self.logger, "Connected to the server {}", addr);
                Ok(Async::Ready((stream, addr)))
            }
            _ => Ok(Async::NotReady),
        }
    }
}
