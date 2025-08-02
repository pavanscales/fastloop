//! Task system — minimal, efficient async executor for fastloo.
//!
//! Uses lock-free queue and atomic flags to efficiently schedule and poll tasks.

use std::{
    future::Future,
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
};

use crossbeam::queue::SegQueue;
use parking_lot::Mutex;

use crate::reactor::Reactor;

/// Inner state of a task.
struct TaskInner {
    /// The future representing the async task.
    future: Mutex<Pin<Box<dyn Future<Output = ()> + Send>>>,

    /// Flag indicating if the task is currently scheduled.
    is_scheduled: AtomicBool,

    /// Reference back to the Reactor for scheduling.
    reactor: Arc<Reactor>,

    /// Optional token registered with the reactor.
    token: Option<mio::Token>,
}

#[derive(Clone)]
pub struct Task {
    inner: Arc<TaskInner>,
}

impl Task {
    /// Creates a new task wrapping a future and attached to a reactor.
    pub fn new(future: Pin<Box<dyn Future<Output = ()> + Send>>, reactor: Arc<Reactor>) -> Self {
        Self {
            inner: Arc::new(TaskInner {
                future: Mutex::new(future),
                is_scheduled: AtomicBool::new(false),
                reactor,
                token: None,
            }),
        }
    }

    /// Polls the task's future once.
    pub fn poll(self: Arc<Self>) {
        // If not scheduled, no need to poll.
        if !self
            .inner
            .is_scheduled
            .swap(false, Ordering::Acquire)
        {
            return;
        }

        // Create a waker for this task.
        let waker = self.clone().into_waker();
        let mut ctx = Context::from_waker(&waker);

        // Lock the future and poll.
        let mut future_slot = self.inner.future.lock();

        match future_slot.as_mut().poll(&mut ctx) {
            Poll::Pending => {
                // Future is not ready yet, it will be rescheduled by the waker.
            }
            Poll::Ready(()) => {
                // Task completed; deregister from reactor if needed.
                if let Some(token) = self.inner.token {
                    self.inner.reactor.deregister(token);
                }
            }
        }
    }

    /// Converts the task into a `Waker`.
    fn into_waker(self: Arc<Self>) -> Waker {
        unsafe { Waker::from_raw(Self::raw_waker(Arc::into_raw(self))) }
    }

    unsafe fn raw_waker(ptr: *const Task) -> RawWaker {
        RawWaker::new(
            ptr as *const (),
            &RawWakerVTable::new(
                Self::clone_waker,
                Self::wake,
                Self::wake_by_ref,
                Self::drop_waker,
            ),
        )
    }

    unsafe fn clone_waker(ptr: *const ()) -> RawWaker {
        let arc = Arc::<Task>::from_raw(ptr as *const Task);
        std::mem::forget(arc.clone()); // Increase ref count.
        RawWaker::new(ptr, &RawWakerVTable::new(Self::clone_waker, Self::wake, Self::wake_by_ref, Self::drop_waker))
    }

    unsafe fn wake(ptr: *const ()) {
        let arc = Arc::<Task>::from_raw(ptr as *const Task);
        arc.schedule();
        // Drop after waking.
    }

    unsafe fn wake_by_ref(ptr: *const ()) {
        let arc = Arc::<Task>::from_raw(ptr as *const Task);
        arc.schedule();
        std::mem::forget(arc); // Don't drop.
    }

    unsafe fn drop_waker(ptr: *const ()) {
        let _ = Arc::<Task>::from_raw(ptr as *const Task);
    }

    /// Schedule this task for polling if not already scheduled.
    fn schedule(&self) {
        // Only enqueue if not already scheduled.
        if !self.inner.is_scheduled.swap(true, Ordering::AcqRel) {
            self.inner.reactor.spawn_task(self.clone());
        }
    }
}

impl Reactor {
    /// Spawn a new task into the reactor's task queue.
    ///
    /// Assumes a lock-free queue `task_queue` and a method `wake_loop` to notify the event loop.
    pub fn spawn_task(&self, task: Arc<Task>) {
        self.task_queue.push(task);
        self.wake_loop();
    }
}
