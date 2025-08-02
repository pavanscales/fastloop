//! Reactor: high-performance event loop driver using slab and mio.

use std::{
    io,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    task::Waker,
};

use mio::{Token};
use slab::Slab;

use crate::poller::Poller;
use parking_lot::Mutex;

/// Internal reactor state, maps tokens to wakers.
struct ReactorState {
    wakers: Slab<Waker>,
}

/// Core reactor driving async task wakeups and event polling.
pub struct Reactor {
    poller: Arc<Poller>,
    state: Mutex<ReactorState>,
    next_token: AtomicUsize,
}

impl Reactor {
    /// Creates a new Reactor instance backed by the provided Poller.
    pub fn new(poller: Arc<Poller>) -> Arc<Self> {
        Arc::new(Self {
            poller,
            state: Mutex::new(ReactorState {
                wakers: Slab::with_capacity(1024),
            }),
            next_token: AtomicUsize::new(0),
        })
    }

    /// Returns the global singleton Reactor instance.
    pub fn global() -> Arc<Self> {
        use once_cell::sync::Lazy;
        static INSTANCE: Lazy<Arc<Reactor>> = Lazy::new(|| {
            let poller = Poller::new().expect("Failed to create Poller");
            Reactor::new(poller)
        });
        INSTANCE.clone()
    }

    /// Registers a waker and returns its unique token.
    ///
    /// Minimizes locked scope to reduce contention.
    pub fn register_waker(&self, waker: Waker) -> Token {
        let mut state = self.state.lock();
        Token(state.wakers.insert(waker))
    }

    /// Removes the waker associated with the token.
    pub fn deregister(&self, token: Token) {
        let mut state = self.state.lock();
        state.wakers.remove(token.0);
    }

    /// Polls OS events with an optional timeout and wakes the relevant tasks.
    ///
    /// Returns any I/O error encountered during polling.
    pub fn poll_events(&self, timeout_ms: Option<u64>) -> io::Result<()> {
        let events = self.poller.poll(timeout_ms)?;

        // Lock once for waker lookup to reduce locking overhead
        let mut state = self.state.lock();

        for event in events {
            if let Some(waker) = state.wakers.get(event.token().0) {
                waker.wake_by_ref();
            }
        }

        Ok(())
    }

    /// Spawns a new async task onto this Reactor.
    ///
    /// Relies on the Task abstraction for task scheduling.
    pub fn spawn(
        &self,
        future: std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 'static>>,
    ) {
        crate::task::Task::spawn(future, self.clone());
    }

    /// Runs the event loop until all tasks complete.
    ///
    /// This method blocks the current thread.
    pub fn run(&self) {
        loop {
            // Poll OS events and wake tasks with a short timeout
            if let Err(e) = self.poll_events(Some(100)) {
                eprintln!("Reactor polling error: {:?}", e);
            }

            // Drive all ready tasks; exit if none remain
            if !crate::task::Task::poll_tasks() {
                break;
            }
        }
    }

    /// Access the underlying poller.
    pub fn poller(&self) -> Arc<Poller> {
        self.poller.clone()
    }
}
