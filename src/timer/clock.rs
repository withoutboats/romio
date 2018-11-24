//! TODO docs
use std::cell::Cell;
use std::ptr::{self, NonNull};
use std::time::Instant;

thread_local!(static CLOCK: Cell<Option<NonNull<dyn Now>>> = Cell::new(None));

/// TODO docs
pub trait Now: Send + Sync + 'static {
    /// TODO docs
    fn now(&self) -> Instant;
}

/// TODO docs
#[derive(Debug)]
pub struct Clock { _private: () }

impl Clock {
    /// TODO docs
    pub fn now() -> Instant {
        CLOCK.with(|clock| {
            match clock.get() {
                None            => Instant::now(),
                Some(clock_ptr) => {
                    unsafe { clock_ptr.as_ref().now() }
                }
            }
        })
    }

    /// TODO docs
    pub fn use_source(source: Box<dyn Now>) {
        CLOCK.with(|clock| {
            if let Some(now) = clock.get() {
                unsafe { ptr::drop_in_place(now.as_ptr()); }
            }
            clock.set(Some((&*source).into()))
        })
    }

    /// TODO docs
    pub fn use_system_time() {
        CLOCK.with(|clock| {
            if let Some(now) = clock.get() {
                unsafe { ptr::drop_in_place(now.as_ptr()); }
            }
            clock.set(None);
        })
    }

    /// TODO docs
    pub fn with_source<T, F: FnOnce() -> T>(source: &dyn Now, f: F) -> T {
        CLOCK.with(|clock| {
            // Ensure that the clock is reset when leaving the scope.
            // This handles cases that involve panicking.
            struct Reset<'a>(&'a Cell<Option<NonNull<dyn Now>>>, Option<NonNull<dyn Now>>);

            impl<'a> Drop for Reset<'a> {
                fn drop(&mut self) {
                    self.0.set(self.1);
                }
            }

            let _reset = Reset(clock, clock.get());

            clock.set(Some(source.into()));

            f()
        })
    }
}

/// TODO docs
pub fn now() -> Instant {
    Clock::now()
}
