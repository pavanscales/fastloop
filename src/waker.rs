//! Custom waker for fastloop — zero-allocation, ultra-fast task wakeup.

use std::{
    mem::ManuallyDrop,
    ptr::NonNull,
    sync::Arc,
    task::{RawWaker, RawWakerVTable, Waker},
};

use crate::task::Task;

/// Create a zero-cost Waker for a task.
pub fn waker(task: &Task) -> Waker {
    unsafe {
        Waker::from_raw(RawWaker::new(
            task as *const Task as *const (),
            &VTABLE,
        ))
    }
}

static VTABLE: RawWakerVTable = RawWakerVTable::new(
    clone,
    wake,
    wake_by_ref,
    drop,
);

unsafe fn clone(ptr: *const ()) -> RawWaker {
    let arc = ManuallyDrop::new(Arc::from_raw(ptr as *const Task));
    let _clone: Arc<Task> = arc.clone();
    RawWaker::new(ptr, &VTABLE)
}

unsafe fn wake(ptr: *const ()) {
    let task = Arc::from_raw(ptr as *const Task);
    task.schedule();
    // drop happens here
}

unsafe fn wake_by_ref(ptr: *const ()) {
    let arc = ManuallyDrop::new(Arc::from_ra//! Custom waker for fastloop — zero-allocation, ultra-fast task wakeup.

use std::{
    mem::ManuallyDrop,
    ptr::NonNull,
    sync::Arc,
    task::{RawWaker, RawWakerVTable, Waker},
};

use crate::task::Task;

/// Create a zero-cost Waker for a task.
pub fn waker(task: &Task) -> Waker {
    unsafe {
        Waker::from_raw(RawWaker::new(
            task as *const Task as *const (),
            &VTABLE,
        ))
    }
}

static VTABLE: RawWakerVTable = RawWakerVTable::new(
    clone,
    wake,
    wake_by_ref,
    drop,
);

unsafe fn clone(ptr: *const ()) -> RawWaker {
    let arc = ManuallyDrop::new(Arc::from_raw(ptr as *const Task));
    let _clone: Arc<Task> = arc.clone();
    RawWaker::new(ptr, &VTABLE)
}

unsafe fn wake(ptr: *const ()) {
    let task = Arc::from_raw(ptr as *const Task);
    task.schedule();
    // drop happens here
}

unsafe fn wake_by_ref(ptr: *const ()) {
    let arc = ManuallyDrop::new(Arc::from_raw(ptr as *const Task));
    arc.schedule();
}

unsafe fn drop(ptr: *const ()) {
    let _ = Arc::from_raw(ptr as *const Task);
}
w(ptr as *const Task));
    arc.schedule();
}

unsafe fn drop(ptr: *const ()) {
    let _ = Arc::from_raw(ptr as *const Task);
}
