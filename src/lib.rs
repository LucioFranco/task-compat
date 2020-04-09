//! Utilities for compating between futures 0.1 and futures 0.3 tasks.

use futures01::{
    executor::{Notify, NotifyHandle, UnsafeNotify},
    Async, Poll as Poll01,
};
use futures03::task::{ArcWake, WakerRef};
use std::{
    sync::Arc,
    task::{Context, Poll as Poll03, RawWaker, RawWakerVTable, Waker},
};

/// Map a `std::task::Context` into the proper context
/// for `futures 0.1`.
pub fn with_notify<F, R>(cx: &mut Context<'_>, f: F) -> R
where
    F: FnOnce() -> R,
{
    let mut spawn = futures01::task::spawn(());

    // TODO: We should find a way to take advantage of the
    // `&static Notify` convert.
    let notify = Arc::new(NotifyWaker(cx.waker().clone()));

    spawn.poll_fn_notify(&notify, 0, |_| f())
}

/// Map a `futures 0.1` notify to a `std::task::Context`.
pub fn with_context<F, R>(f: F) -> R
where
    F: FnOnce(&mut Context<'_>) -> R,
{
    let current = Current::new();
    let waker = current.as_waker();
    let mut cx = Context::from_waker(&waker);

    f(&mut cx)
}

/// Map `std::task::Poll` to a `futures 0.1` poll.
pub fn poll_03_to_01<T, E>(x: Poll03<Result<T, E>>) -> Poll01<T, E> {
    match x? {
        Poll03::Ready(t) => Ok(Async::Ready(t)),
        Poll03::Pending => Ok(Async::NotReady),
    }
}

/// Map `futures 0.1` poll to `std::task::Poll`.
pub fn poll_01_to_03<T, E>(x: Poll01<T, E>) -> Poll03<Result<T, E>> {
    match x {
        Ok(Async::Ready(v)) => Poll03::Ready(Ok(v)),
        Ok(Async::NotReady) => Poll03::Pending,
        Err(e) => Poll03::Ready(Err(e)),
    }
}

// Waker adapters ported from `futures-util/src/compat`

struct NotifyWaker(Waker);

#[derive(Clone)]
struct WakerToHandle<'a>(&'a Waker);

impl From<WakerToHandle<'_>> for NotifyHandle {
    fn from(handle: WakerToHandle<'_>) -> NotifyHandle {
        let ptr = Box::new(NotifyWaker(handle.0.clone()));

        unsafe { NotifyHandle::new(Box::into_raw(ptr)) }
    }
}

impl Notify for NotifyWaker {
    fn notify(&self, _: usize) {
        self.0.wake_by_ref();
    }
}

unsafe impl UnsafeNotify for NotifyWaker {
    unsafe fn clone_raw(&self) -> NotifyHandle {
        WakerToHandle(&self.0).into()
    }

    unsafe fn drop_raw(&self) {
        let ptr: *const dyn UnsafeNotify = self;
        drop(Box::from_raw(ptr as *mut dyn UnsafeNotify));
    }
}

#[derive(Clone)]
struct Current(futures01::task::Task);

impl Current {
    fn new() -> Current {
        Current(futures01::task::current())
    }

    fn as_waker(&self) -> WakerRef<'_> {
        unsafe fn ptr_to_current<'a>(ptr: *const ()) -> &'a Current {
            &*(ptr as *const Current)
        }
        fn current_to_ptr(current: &Current) -> *const () {
            current as *const Current as *const ()
        }

        unsafe fn clone(ptr: *const ()) -> RawWaker {
            // Lazily create the `Arc` only when the waker is actually cloned.
            // FIXME: remove `transmute` when a `Waker` -> `RawWaker` conversion
            // function is landed in `core`.
            std::mem::transmute::<Waker, RawWaker>(futures03::task::waker(Arc::new(
                ptr_to_current(ptr).clone(),
            )))
        }
        unsafe fn drop(_: *const ()) {}
        unsafe fn wake(ptr: *const ()) {
            ptr_to_current(ptr).0.notify()
        }

        let ptr = current_to_ptr(self);
        let vtable = &RawWakerVTable::new(clone, wake, wake, drop);
        futures03::task::WakerRef::new_unowned(std::mem::ManuallyDrop::new(unsafe {
            futures03::task::Waker::from_raw(RawWaker::new(ptr, vtable))
        }))
    }
}

impl ArcWake for Current {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        arc_self.0.notify();
    }
}
