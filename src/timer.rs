use std::{
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    task::Waker,
    time::Duration,
};
use crossbeam::queue::SegQueue;
use array_init::array_init;

const WHEEL_SIZE: usize = 256;
const SLOT_DURATION_MS: u64 = 10;

pub type TimerId = u64;

#[derive(Debug)]
pub struct TimerEntry {
    id: TimerId,
    expiration_slot: usize,
    waker: Waker, // ✅ No Arc
    canceled: AtomicBool,
}

impl TimerEntry {
    pub fn cancel(&self) {
        self.canceled.store(true, Ordering::Relaxed); // ✅ Relaxed is fine here
    }
}

pub struct TimerWheel {
    current_slot: AtomicU64,
    slots: [SegQueue<Arc<TimerEntry>>; WHEEL_SIZE], // ✅ Fixed-size array
    next_id: AtomicU64,
}

impl TimerWheel {
    pub fn new() -> Arc<Self> {
        let slots = array_init(|_| SegQueue::new());

        Arc::new(Self {
            current_slot: AtomicU64::new(0),
            slots,
            next_id: AtomicU64::new(1),
        })
    }

    /// Schedule a timer to fire after `delay`. Returns TimerId and Arc handle to cancel if needed.
    pub fn schedule(&self, delay: Duration, waker: Waker) -> (TimerId, Arc<TimerEntry>) {
        let ticks = (delay.as_millis() / SLOT_DURATION_MS as u128) as usize;
        let current = self.current_slot.load(Ordering::Relaxed) as usize;
        let expiration_slot = (current + ticks) % WHEEL_SIZE;

        let id = self.next_id.fetch_add(1, Ordering::Relaxed);

        let entry = Arc::new(TimerEntry {
            id,
            expiration_slot,
            waker,
            canceled: AtomicBool::new(false),
        });

        self.slots[expiration_slot].push(entry.clone());
        (id, entry)
    }

    /// Advance wheel by one slot and fire all timers in the current slot.
    pub fn tick(&self) {
        let current_slot = self
            .current_slot
            .fetch_update(Ordering::AcqRel, Ordering::Relaxed, |val| {
                Some((val + 1) % WHEEL_SIZE as u64)
            })
            .unwrap_or(0) as usize;

        let slot = &self.slots[current_slot];

        while let Some(timer) = slot.pop() {
            if !timer.canceled.load(Ordering::Relaxed) {
                timer.waker.wake_by_ref();
            }
            // ✅ You can recycle Arc<TimerEntry> here if needed
        }
    }
}
