use std::io::{self, Read, Write};
use std::net::SocketAddr;
use fibers::net::TcpStream;
use futures::{Async, Future, Poll};
use trackable::error::{ErrorKindExt, Failed};

use {AsyncResult, Error};
use consul::{ConsulClient, ServiceNode};

pub struct Proxy {
    client: TcpStream,
    peer: Option<TcpStream>,
    future0: Option<AsyncResult<Vec<ServiceNode>>>,
    future1: Option<AsyncResult<TcpStream>>,
    candidates: Vec<SocketAddr>,
    buf0: Vec<u8>,
    buf1: Vec<u8>,
}
impl Proxy {
    pub fn new(client: TcpStream, consul_addr: SocketAddr, service: String) -> Self {
        let future0 = ConsulClient::new(consul_addr).find_service_nodes(&service);
        Proxy {
            client,
            peer: None,
            future0: Some(future0),
            future1: None,
            candidates: Vec::new(),
            buf0: Vec::with_capacity(4096),
            buf1: Vec::with_capacity(4096),
        }
    }
}
impl Future for Proxy {
    type Item = ();
    type Error = Error;
    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        loop {
            if let Async::Ready(Some(nodes)) = track!(self.future0.poll())? {
                self.candidates = nodes.into_iter().map(|n| n.socket_addr()).collect();
                track_assert_ne!(self.candidates.len(), 0, Failed);

                self.candidates.reverse();
                println!("# CANDIDATES: {:?}", self.candidates);
                self.future0 = None;
            }
            if self.future0.is_some() {
                break;
            }

            if self.peer.is_none() {
                match track!(self.future1.poll())? {
                    Async::Ready(Some(stream)) => {
                        println!("# CONNECTED: {:?}", stream);
                        self.candidates.clear();
                        self.future1 = None;
                        self.peer = Some(stream);
                        continue;
                    }
                    Async::Ready(None) => {
                        if let Some(c) = self.candidates.pop() {
                            println!("# NEXT: {:?}", c);
                            let future = TcpStream::connect(c)
                                .map_err(|e| track!(Error::from(Failed.cause(e))));
                            self.future1 = Some(Box::new(future));
                            continue;
                        } else {
                            track_panic!(Failed);
                        }
                    }
                    Async::NotReady => {}
                }
            }
            if self.peer.is_none() {
                break;
            }

            if self.buf0.is_empty() {
                self.buf0.resize(4096, 0);
                match self.client.read(&mut self.buf0) {
                    Err(e) => {
                        if e.kind() == io::ErrorKind::WouldBlock {
                            self.buf0.truncate(0);
                        } else {
                            return Err(track!(Error::from_error(e)));
                        }
                    }
                    Ok(0) => {
                        // eof
                        return Ok(Async::Ready(()));
                    }
                    Ok(size) => {
                        self.buf0.truncate(size);
                        continue;
                    }
                }
            } else {
                match self.peer.as_mut().unwrap().write(&self.buf0) {
                    Err(e) => {
                        if e.kind() != io::ErrorKind::WouldBlock {
                            return Err(track!(Error::from_error(e)));
                        }
                    }
                    Ok(size) => {
                        self.buf0.drain(0..size);
                        continue;
                    }
                }
            }

            if self.buf1.is_empty() {
                self.buf1.resize(4096, 0);
                match self.peer.as_mut().unwrap().read(&mut self.buf1) {
                    Err(e) => {
                        if e.kind() == io::ErrorKind::WouldBlock {
                            self.buf1.truncate(0);
                        } else {
                            return Err(track!(Error::from_error(e)));
                        }
                    }
                    Ok(0) => {
                        // eof
                        return Ok(Async::Ready(()));
                    }
                    Ok(size) => {
                        self.buf1.truncate(size);
                        continue;
                    }
                }
            } else {
                match self.client.write(&self.buf1) {
                    Err(e) => {
                        if e.kind() != io::ErrorKind::WouldBlock {
                            return Err(track!(Error::from_error(e)));
                        }
                    }
                    Ok(size) => {
                        self.buf1.drain(0..size);
                        continue;
                    }
                }
            }

            break;
        }
        Ok(Async::NotReady)
    }
}
