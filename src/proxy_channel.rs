use fibers::net::TcpStream;
use futures::{Async, Future, Poll};
use slog::Logger;
use std::io::{self, Read, Write};

use {Error, Result};

#[derive(Debug)]
struct Buffer {
    inner: Vec<u8>,
    write_start: usize,
    read_start: usize,
}
impl Buffer {
    fn new(capacity: usize) -> Self {
        Buffer {
            inner: vec![0; capacity],
            write_start: 0,
            read_start: 0,
        }
    }
    fn read_from<R: Read + ::std::fmt::Debug>(
        &mut self,
        reader: &mut R,
    ) -> Result<Async<Option<usize>>> {
        if self.read_start == self.inner.len() {
            return Ok(Async::NotReady);
        }
        match reader.read(&mut self.inner[self.read_start..]) {
            Err(e) => {
                if e.kind() == io::ErrorKind::WouldBlock {
                    Ok(Async::NotReady)
                } else {
                    Err(track!(Error::from(e)))
                }
            }
            Ok(0) => Ok(Async::Ready(None)),
            Ok(size) => {
                self.read_start += size;
                Ok(Async::Ready(Some(size)))
            }
        }
    }
    fn write_to<W: Write + ::std::fmt::Debug>(
        &mut self,
        writer: &mut W,
    ) -> Result<Async<Option<usize>>> {
        if self.write_start == self.read_start {
            return Ok(Async::NotReady);
        }
        match writer.write(&self.inner[self.write_start..self.read_start]) {
            Err(e) => {
                if e.kind() == io::ErrorKind::WouldBlock {
                    Ok(Async::NotReady)
                } else {
                    Err(track!(Error::from(e)))
                }
            }
            Ok(0) => Ok(Async::Ready(None)),
            Ok(size) => {
                self.write_start += size;
                if self.write_start == self.read_start {
                    self.write_start = 0;
                    self.read_start = 0;
                }
                Ok(Async::Ready(Some(size)))
            }
        }
    }
}

#[derive(Debug)]
pub struct ProxyChannel {
    logger: Logger,
    client: TcpStream,
    client_buf: Buffer,
    server: TcpStream,
    server_buf: Buffer,
}
impl ProxyChannel {
    pub const DEFAULT_BUFFER_SIZE: usize = 8 * 1024;

    pub fn new(logger: Logger, client: TcpStream, server: TcpStream) -> Self {
        unsafe {
            let _ = client.with_inner(|socket| socket.set_nodelay(true));
            let _ = server.with_inner(|socket| socket.set_nodelay(true));
        }
        ProxyChannel {
            logger,
            client,
            client_buf: Buffer::new(Self::DEFAULT_BUFFER_SIZE),
            server,
            server_buf: Buffer::new(Self::DEFAULT_BUFFER_SIZE),
        }
    }
}
impl Future for ProxyChannel {
    type Item = ();
    type Error = Error;
    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        loop {
            match track!(self.client_buf.read_from(&mut self.client))? {
                Async::NotReady => {}
                Async::Ready(None) => {
                    info!(self.logger, "Connection closed by client while reading");
                    return Ok(Async::Ready(()));
                }
                Async::Ready(Some(size)) => {
                    debug!(self.logger, "Received {} bytes from client", size);
                    continue;
                }
            }
            match track!(self.client_buf.write_to(&mut self.server))? {
                Async::NotReady => {}
                Async::Ready(None) => {
                    info!(self.logger, "Connection closed by server while writing");
                    return Ok(Async::Ready(()));
                }
                Async::Ready(Some(size)) => {
                    debug!(self.logger, "Sent {} bytes to server", size);
                    continue;
                }
            }
            match track!(self.server_buf.read_from(&mut self.server))? {
                Async::NotReady => {}
                Async::Ready(None) => {
                    info!(self.logger, "Connection closed by server while reading");
                    return Ok(Async::Ready(()));
                }
                Async::Ready(Some(size)) => {
                    debug!(self.logger, "Received {} bytes from server", size);
                    continue;
                }
            }
            match track!(self.server_buf.write_to(&mut self.client))? {
                Async::NotReady => {}
                Async::Ready(None) => {
                    info!(self.logger, "Connection closed by client while writing");
                    return Ok(Async::Ready(()));
                }
                Async::Ready(Some(size)) => {
                    debug!(self.logger, "Sent {} bytes to client", size);
                    continue;
                }
            }
            break;
        }
        Ok(Async::NotReady)
    }
}
