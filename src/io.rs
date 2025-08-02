//! Non-blocking TCP I/O built on mio, integrated with fastloop reactor.

use std::{
    io::{self, Read, Write},
    net::SocketAddr,
    sync::Arc,
    task::Waker,
};

use mio::{
    event::Source,
    net::{TcpListener, TcpStream},
    Interest, Registry, Token,
};

use crate::reactor::Reactor;

/// Wrapper around a non-blocking TCP stream.
pub struct FastSocket {
    stream: TcpStream,
    token: Option<Token>,
    reactor: Arc<Reactor>,
}

impl FastSocket {
    /// Connects to a remote address using non-blocking socket.
    pub fn connect(addr: SocketAddr, reactor: Arc<Reactor>) -> io::Result<Self> {
        let mut stream = TcpStream::connect(addr)?;
        stream.set_nonblocking(true)?;
        Ok(Self {
            stream,
            token: None,
            reactor,
        })
    }

    /// Registers this socket with the reactor and interest (read/write).
    pub fn register(&mut self, interest: Interest, waker: Waker) -> io::Result<()> {
        let token = self.reactor.register_waker(waker);
        self.reactor
            .poller()
            .registry()
            .register(&mut self.stream, token, interest)?;
        self.token = Some(token);
        Ok(())
    }

    /// Reregisters this socket to modify interest (e.g. switching between read/write).
    pub fn reregister(&mut self, interest: Interest) -> io::Result<()> {
        if let Some(token) = self.token {
            self.reactor
                .poller()
                .registry()
                .reregister(&mut self.stream, token, interest)?;
        }
        Ok(())
    }

    /// Deregisters this socket from the reactor.
    pub fn deregister(&mut self) -> io::Result<()> {
        self.reactor.poller().registry().deregister(&mut self.stream)?;
        if let Some(token) = self.token.take() {
            self.reactor.deregister(token);
        }
        Ok(())
    }

    /// Attempts to read into the provided buffer.
    pub fn try_read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.stream.read(buf)
    }

    /// Attempts to write the provided buffer to the stream.
    pub fn try_write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.stream.write(buf)
    }

    /// Returns reference to raw mio stream.
    pub fn raw(&self) -> &TcpStream {
        &self.stream
    }
}

/// Wrapper around a non-blocking TCP listener (server).
pub struct FastListener {
    listener: TcpListener,
    token: Option<Token>,
    reactor: Arc<Reactor>,
}

impl FastListener {
    /// Binds a non-blocking TCP listener to the given address.
    pub fn bind(addr: SocketAddr, reactor: Arc<Reactor>) -> io::Result<Self> {
        let listener = TcpListener::bind(addr)?;
        listener.set_nonblocking(true)?;
        Ok(Self {
            listener,
            token: None,
            reactor,
        })
    }

    /// Registers the listener with the reactor for accept readiness.
    pub fn register(&mut self, waker: Waker) -> io::Result<()> {
        let token = self.reactor.register_waker(waker);
        self.reactor
            .poller()
            .registry()
            .register(&mut self.listener, token, Interest::READABLE)?;
        self.token = Some(token);
        Ok(())
    }

    /// Deregisters the listener from the reactor.
    pub fn deregister(&mut self) -> io::Result<()> {
        self.reactor
            .poller()
            .registry()
            .deregister(&mut self.listener)?;
        if let Some(token) = self.token.take() {
            self.reactor.deregister(token);
        }
        Ok(())
    }

    /// Attempts to accept a new client connection.
    pub fn try_accept(&mut self) -> io::Result<(FastSocket, SocketAddr)> {
        let (mut stream, addr) = self.listener.accept()?;
        stream.set_nonblocking(true)?;
        Ok((
            FastSocket {
                stream,
                token: None,
                reactor: self.reactor.clone(),
            },
            addr,
        ))
    }

    /// Returns reference to raw mio listener.
    pub fn raw(&self) -> &TcpListener {
        &self.listener
    }
}
