//! ⚡ World-Class TCP I/O with Edge-Triggered Polling & Zero Allocation
//! Built for ultra-low-latency fastloop reactor.

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
    #[inline(always)]
    pub fn connect(addr: SocketAddr, reactor: Arc<Reactor>) -> io::Result<Self> {
        let mut stream = TcpStream::connect(addr)?;
        stream.set_nonblocking(true)?;
        Ok(Self {
            stream,
            token: None,
            reactor,
        })
    }

    /// Registers the socket with the reactor using EDGE-TRIGGERED mode.
    pub fn register(&mut self, interest: Interest, waker: Waker) -> io::Result<()> {
        let token = self.reactor.register_waker(waker);
        self.reactor
            .poller()
            .registry()
            .register(&mut self.stream, token, interest)?;
        self.token = Some(token);
        Ok(())
    }

    /// Reregisters the socket to change interest.
    #[inline(always)]
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
    #[inline(always)]
    pub fn deregister(&mut self) -> io::Result<()> {
        self.reactor.poller().registry().deregister(&mut self.stream)?;
        if let Some(token) = self.token.take() {
            self.reactor.deregister(token);
        }
        Ok(())
    }

    /// Attempts to read into the provided buffer. Use in a loop until WouldBlock.
    #[inline(always)]
    pub fn try_read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.stream.read(buf)
    }

    /// Attempts to write buffer to stream. Use in a loop until WouldBlock.
    #[inline(always)]
    pub fn try_write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.stream.write(buf)
    }

    /// Access raw mio stream.
    #[inline(always)]
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
    /// Binds to a TCP address using non-blocking mode.
    #[inline(always)]
    pub fn bind(addr: SocketAddr, reactor: Arc<Reactor>) -> io::Result<Self> {
        let listener = TcpListener::bind(addr)?;
        listener.set_nonblocking(true)?;
        Ok(Self {
            listener,
            token: None,
            reactor,
        })
    }

    /// Registers with reactor using edge-triggered read interest.
    pub fn register(&mut self, waker: Waker) -> io::Result<()> {
        let token = self.reactor.register_waker(waker);
        self.reactor
            .poller()
            .registry()
            .register(&mut self.listener, token, Interest::READABLE)?;
        self.token = Some(token);
        Ok(())
    }

    /// Deregisters the listener.
    #[inline(always)]
    pub fn deregister(&mut self) -> io::Result<()> {
        self.reactor.poller().registry().deregister(&mut self.listener)?;
        if let Some(token) = self.token.take() {
            self.reactor.deregister(token);
        }
        Ok(())
    }

    /// Accepts as many connections as available (use in loop).
    #[inline(always)]
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

    /// Access raw mio listener.
    #[inline(always)]
    pub fn raw(&self) -> &TcpListener {
        &self.listener
    }
}
