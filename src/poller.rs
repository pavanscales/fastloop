//! Poller — High-Performance Event Polling Abstraction
//!
//! Wraps `mio::Poll` for scalable, thread-safe, minimal-latency event polling.

use mio::{Events, Interest, Poll, Token, Registry};
use mio::event::Source;
use std::io;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Tunable max number of events per poll call.
/// Larger value means less syscalls but more memory.
const MAX_EVENTS: usize = 1024;

/// Poller wraps OS event queue for efficient async I/O polling.
/// Designed to be used concurrently via Arc.
#[derive(Debug)]
pub struct Poller {
    poll: Poll,
    // Using Mutex because mio::Events is not thread-safe
    events: Mutex<Events>,
}

impl Poller {
    /// Creates a new Poller instance.
    /// Returns Arc for easy sharing between threads.
    pub fn new() -> io::Result<Arc<Self>> {
        Ok(Arc::new(Self {
            poll: Poll::new()?,
            events: Mutex::new(Events::with_capacity(MAX_EVENTS)),
        }))
    }

    /// Access the underlying Registry for source registration.
    #[inline(always)]
    pub fn registry(&self) -> &Registry {
        self.poll.registry()
    }

    /// Registers a new I/O source with specified interest and token.
    #[inline(always)]
    pub fn register<S: Source>(&self, source: &mut S, token: Token, interest: Interest) -> io::Result<()> {
        self.registry().register(source, token, interest)
    }

    /// Modifies registration of an existing source.
    #[inline(always)]
    pub fn reregister<S: Source>(&self, source: &mut S, token: Token, interest: Interest) -> io::Result<()> {
        self.registry().reregister(source, token, interest)
    }

    /// Deregisters a source from polling.
    #[inline(always)]
    pub fn deregister<S: Source>(&self, source: &mut S) -> io::Result<()> {
        self.registry().deregister(source)
    }

    /// Polls for events.
    ///
    /// # Arguments
    /// - `timeout_ms`: Timeout in milliseconds; None to block indefinitely.
    ///
    /// # Returns
    /// Vector of events triggered by the OS.
    pub fn poll(&self, timeout_ms: Option<u64>) -> io::Result<Vec<mio::event::Event>> {
        let timeout = timeout_ms.map(Duration::from_millis);

        // Lock once per poll to avoid repeated locking
        let mut events = self.events.lock().expect("Poller mutex poisoned");

        // Clear old events for reuse (important for performance)
        events.clear();

        // Poll OS event queue with optional timeout
        self.poll.poll(&mut events, timeout)?;

        // Collect events efficiently without reallocations
        let mut collected = Vec::with_capacity(events.iter().count());
        for event in events.iter() {
            collected.push(event);
        }

        Ok(collected)
    }
}
