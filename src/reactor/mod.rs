pub(crate) mod background;
pub(crate) mod registration;
mod sharded_rwlock;

// ===== Public re-exports =====

use self::background::Background;
pub use crate::raw::PollEvented;

// ===== Private imports =====

use self::sharded_rwlock::RwLock;

use std::cell::RefCell;
use std::io;
use std::mem;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::{Relaxed, SeqCst};
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};
use std::{fmt, usize};

use futures::task::{AtomicWaker, Waker};
use log::{debug, log_enabled, trace, Level};
use mio::event::Evented;
use slab::Slab;

/// The core reactor, or event loop.
///
/// The event loop is the main source of blocking in an application which drives
/// all other I/O events and notifications happening. Each event loop can have
/// multiple handles pointing to it, each of which can then be used to create
/// various I/O objects to interact with the event loop in interesting ways.
struct Reactor {
    /// Reuse the `mio::Events` value across calls to poll.
    events: mio::Events,

    /// State shared between the reactor and the handles.
    inner: Arc<Inner>,

    _wakeup_registration: mio::Registration,
}

/// A reference to a reactor.
///
/// A `Handle` is used for associating I/O objects with an event loop
/// explicitly. Typically though you won't end up using a `Handle` that often
/// and will instead use the default reactor for the execution context.
///
/// By default, most components bind lazily to reactors.
/// To get this behavior when manually passing a `Handle`, use `default()`.
#[derive(Clone)]
struct Handle {
    inner: Option<HandlePriv>,
}

/// Like `Handle`, but never `None`.
#[derive(Clone)]
struct HandlePriv {
    inner: Weak<Inner>,
}

/// Return value from the `turn` method on `Reactor`.
///
/// Currently this value doesn't actually provide any functionality, but it may
/// in the future give insight into what happened during `turn`.
#[derive(Debug)]
struct Turn {
    _priv: (),
}

#[test]
fn test_handle_size() {
    use std::mem;
    assert_eq!(mem::size_of::<Handle>(), mem::size_of::<HandlePriv>());
}

struct Inner {
    /// The underlying system event queue.
    io: mio::Poll,

    /// ABA guard counter
    next_aba_guard: AtomicUsize,

    /// Dispatch slabs for I/O and futures events
    io_dispatch: RwLock<Slab<ScheduledIo>>,

    /// Used to wake up the reactor from a call to `turn`
    wakeup: mio::SetReadiness,
}

struct ScheduledIo {
    aba_guard: usize,
    readiness: AtomicUsize,
    reader: AtomicWaker,
    writer: AtomicWaker,
}

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub(crate) enum Direction {
    Read,
    Write,
}

/// The global fallback reactor.
static HANDLE_FALLBACK: AtomicUsize = AtomicUsize::new(0);

// Tracks the reactor for the current execution context.
thread_local!(static CURRENT_REACTOR: RefCell<Option<HandlePriv>> = RefCell::new(None));

const TOKEN_SHIFT: usize = 22;

// Kind of arbitrary, but this reserves some token space for later usage.
const MAX_SOURCES: usize = (1 << TOKEN_SHIFT) - 1;
const TOKEN_WAKEUP: mio::Token = mio::Token(MAX_SOURCES);

fn _assert_kinds() {
    fn _assert<T: Send + Sync>() {}

    _assert::<Handle>();
}

// ===== impl Reactor =====

impl Reactor {
    /// Creates a new event loop, returning any error that happened during the
    /// creation.
    fn new() -> io::Result<Reactor> {
        let io = mio::Poll::new()?;
        let wakeup_pair = mio::Registration::new2();

        io.register(
            &wakeup_pair.0,
            TOKEN_WAKEUP,
            mio::Ready::readable(),
            mio::PollOpt::level(),
        )?;

        Ok(Reactor {
            events: mio::Events::with_capacity(1024),
            _wakeup_registration: wakeup_pair.0,
            inner: Arc::new(Inner {
                io: io,
                next_aba_guard: AtomicUsize::new(0),
                io_dispatch: RwLock::new(Slab::with_capacity(1)),
                wakeup: wakeup_pair.1,
            }),
        })
    }

    /// Returns a handle to this event loop which can be sent across threads
    /// and can be used as a proxy to the event loop itself.
    ///
    /// Handles are cloneable and clones always refer to the same event loop.
    /// This handle is typically passed into functions that create I/O objects
    /// to bind them to this event loop.
    fn handle(&self) -> Handle {
        Handle {
            inner: Some(HandlePriv {
                inner: Arc::downgrade(&self.inner),
            }),
        }
    }

    /// Performs one iteration of the event loop, blocking on waiting for events
    /// for at most `max_wait` (forever if `None`).
    ///
    /// This method is the primary method of running this reactor and processing
    /// I/O events that occur. This method executes one iteration of an event
    /// loop, blocking at most once waiting for events to happen.
    ///
    /// If a `max_wait` is specified then the method should block no longer than
    /// the duration specified, but this shouldn't be used as a super-precise
    /// timer but rather a "ballpark approximation"
    ///
    /// # Return value
    ///
    /// This function returns an instance of `Turn`
    ///
    /// `Turn` as of today has no extra information with it and can be safely
    /// discarded.  In the future `Turn` may contain information about what
    /// happened while this reactor blocked.
    ///
    /// # Errors
    ///
    /// This function may also return any I/O error which occurs when polling
    /// for readiness of I/O objects with the OS. This is quite unlikely to
    /// arise and typically mean that things have gone horribly wrong at that
    /// point. Currently this is primarily only known to happen for internal
    /// bugs to `tokio` itself.
    fn turn(&mut self, max_wait: Option<Duration>) -> io::Result<Turn> {
        self.poll(max_wait)?;
        Ok(Turn { _priv: () })
    }

    /// Returns true if the reactor is currently idle.
    ///
    /// Idle is defined as all tasks that have been spawned have completed,
    /// either successfully or with an error.
    fn is_idle(&self) -> bool {
        self.inner.io_dispatch.read().is_empty()
    }

    /// Run this reactor on a background thread.
    ///
    /// This function takes ownership, spawns a new thread, and moves the
    /// reactor to this new thread. It then runs the reactor, driving all
    /// associated I/O resources, until the `Background` handle is dropped or
    /// explicitly shutdown.
    fn background(self) -> io::Result<Background> {
        Background::new(self)
    }

    fn poll(&mut self, max_wait: Option<Duration>) -> io::Result<()> {
        // Block waiting for an event to happen, peeling out how many events
        // happened.
        match self.inner.io.poll(&mut self.events, max_wait) {
            Ok(_) => {}
            Err(e) => return Err(e),
        }

        let start = if log_enabled!(Level::Debug) {
            Some(Instant::now())
        } else {
            None
        };

        // Process all the events that came in, dispatching appropriately
        let mut events = 0;
        for event in self.events.iter() {
            events += 1;
            let token = event.token();
            trace!("event {:?} {:?}", event.readiness(), event.token());

            if token == TOKEN_WAKEUP {
                self.inner
                    .wakeup
                    .set_readiness(mio::Ready::empty())
                    .unwrap();
            } else {
                self.dispatch(token, event.readiness());
            }
        }

        if let Some(start) = start {
            let dur = start.elapsed();
            trace!(
                "loop process - {} events, {}.{:03}s",
                events,
                dur.as_secs(),
                dur.subsec_nanos() / 1_000_000
            );
        }

        Ok(())
    }

    fn dispatch(&self, token: mio::Token, ready: mio::Ready) {
        let aba_guard = token.0 & !MAX_SOURCES;
        let token = token.0 & MAX_SOURCES;

        let mut rd = None;
        let mut wr = None;

        // Create a scope to ensure that notifying the tasks stays out of the
        // lock's critical section.
        {
            let io_dispatch = self.inner.io_dispatch.read();

            let io = match io_dispatch.get(token) {
                Some(io) => io,
                None => return,
            };

            if aba_guard != io.aba_guard {
                return;
            }

            io.readiness.fetch_or(ready.as_usize(), Relaxed);

            if ready.is_writable() || platform::is_hup(&ready) {
                wr = io.writer.take();
            }

            if !(ready & (!mio::Ready::writable())).is_empty() {
                rd = io.reader.take();
            }
        }

        if let Some(task) = rd {
            task.wake();
        }

        if let Some(task) = wr {
            task.wake();
        }
    }
}

impl fmt::Debug for Reactor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Reactor")
    }
}

// ===== impl Handle =====

impl Handle {
    fn as_priv(&self) -> Option<&HandlePriv> {
        self.inner.as_ref()
    }

    fn into_priv(self) -> Option<HandlePriv> {
        self.inner
    }

    fn wakeup(&self) {
        if let Some(handle) = self.as_priv() {
            handle.wakeup();
        }
    }
}

impl Default for Handle {
    /// Returns a "default" handle, i.e., a handle that lazily binds to a reactor.
    fn default() -> Handle {
        Handle { inner: None }
    }
}

impl fmt::Debug for Handle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Handle")
    }
}

fn set_fallback(handle: HandlePriv) -> Result<(), ()> {
    unsafe {
        let val = handle.into_usize();
        match HANDLE_FALLBACK.compare_exchange(0, val, SeqCst, SeqCst) {
            Ok(_) => Ok(()),
            Err(_) => {
                drop(HandlePriv::from_usize(val));
                Err(())
            }
        }
    }
}

// ===== impl HandlePriv =====

impl HandlePriv {
    /// Try to get a handle to the current reactor.
    ///
    /// Returns `Err` if no handle is found.
    pub(crate) fn try_current() -> io::Result<HandlePriv> {
        CURRENT_REACTOR.with(|current| match *current.borrow() {
            Some(ref handle) => Ok(handle.clone()),
            None => HandlePriv::fallback(),
        })
    }

    /// Returns a handle to the fallback reactor.
    fn fallback() -> io::Result<HandlePriv> {
        let mut fallback = HANDLE_FALLBACK.load(SeqCst);

        // If the fallback hasn't been previously initialized then let's spin
        // up a helper thread and try to initialize with that. If we can't
        // actually create a helper thread then we'll just return a "defunct"
        // handle which will return errors when I/O objects are attempted to be
        // associated.
        if fallback == 0 {
            let reactor = match Reactor::new() {
                Ok(reactor) => reactor,
                Err(_) => {
                    return Err(io::Error::new(
                        io::ErrorKind::Other,
                        "failed to create reactor",
                    ))
                }
            };

            // If we successfully set ourselves as the actual fallback then we
            // want to `forget` the helper thread to ensure that it persists
            // globally. If we fail to set ourselves as the fallback that means
            // that someone was racing with this call to `Handle::default`.
            // They ended up winning so we'll destroy our helper thread (which
            // shuts down the thread) and reload the fallback.
            if set_fallback(reactor.handle().into_priv().unwrap()).is_ok() {
                let ret = reactor.handle().into_priv().unwrap();

                match reactor.background() {
                    Ok(bg) => bg.forget(),
                    // The global handle is fubar, but y'all probably got bigger
                    // problems if a thread can't spawn.
                    Err(_) => {}
                }

                return Ok(ret);
            }

            fallback = HANDLE_FALLBACK.load(SeqCst);
        }

        // At this point our fallback handle global was configured so we use
        // its value to reify a handle, clone it, and then forget our reified
        // handle as we don't actually have an owning reference to it.
        assert!(fallback != 0);

        let ret = unsafe {
            let handle = HandlePriv::from_usize(fallback);
            let ret = handle.clone();

            // This prevents `handle` from being dropped and having the ref
            // count decremented.
            drop(handle.into_usize());

            ret
        };

        Ok(ret)
    }

    /// Forces a reactor blocked in a call to `turn` to wakeup, or otherwise
    /// makes the next call to `turn` return immediately.
    ///
    /// This method is intended to be used in situations where a notification
    /// needs to otherwise be sent to the main reactor. If the reactor is
    /// currently blocked inside of `turn` then it will wake up and soon return
    /// after this method has been called. If the reactor is not currently
    /// blocked in `turn`, then the next call to `turn` will not block and
    /// return immediately.
    fn wakeup(&self) {
        if let Some(inner) = self.inner() {
            inner.wakeup.set_readiness(mio::Ready::readable()).unwrap();
        }
    }

    fn into_usize(self) -> usize {
        unsafe { mem::transmute::<Weak<Inner>, usize>(self.inner) }
    }

    unsafe fn from_usize(val: usize) -> HandlePriv {
        let inner = mem::transmute::<usize, Weak<Inner>>(val);;
        HandlePriv { inner }
    }

    fn inner(&self) -> Option<Arc<Inner>> {
        self.inner.upgrade()
    }
}

impl fmt::Debug for HandlePriv {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "HandlePriv")
    }
}

// ===== impl Inner =====

impl Inner {
    /// Register an I/O resource with the reactor.
    ///
    /// The registration token is returned.
    fn add_source(&self, source: &dyn Evented) -> io::Result<usize> {
        // Get an ABA guard value
        let aba_guard = self.next_aba_guard.fetch_add(1 << TOKEN_SHIFT, Relaxed);

        let mut io_dispatch = self.io_dispatch.write();

        if io_dispatch.len() == MAX_SOURCES {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "reactor at max \
                 registered I/O resources",
            ));
        }

        // Acquire a write lock
        let key = io_dispatch.insert(ScheduledIo {
            aba_guard,
            readiness: AtomicUsize::new(0),
            reader: AtomicWaker::new(),
            writer: AtomicWaker::new(),
        });

        self.io.register(
            source,
            mio::Token(aba_guard | key),
            mio::Ready::all(),
            mio::PollOpt::edge(),
        )?;

        Ok(key)
    }

    /// Deregisters an I/O resource from the reactor.
    fn deregister_source(&self, source: &dyn Evented) -> io::Result<()> {
        self.io.deregister(source)
    }

    fn drop_source(&self, token: usize) {
        debug!("dropping I/O source: {}", token);
        self.io_dispatch.write().remove(token);
    }

    /// Registers interest in the I/O resource associated with `token`.
    fn register(&self, waker: &Waker, token: usize, dir: Direction) {
        debug!("scheduling direction for: {}", token);
        let io_dispatch = self.io_dispatch.read();
        let sched = io_dispatch.get(token).unwrap();

        let (atomic_waker, ready) = match dir {
            Direction::Read => (&sched.reader, !mio::Ready::writable()),
            Direction::Write => (&sched.writer, mio::Ready::writable()),
        };

        atomic_waker.register(waker);

        if sched.readiness.load(SeqCst) & ready.as_usize() != 0 {
            atomic_waker.wake();
        }
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        // When a reactor is dropped it needs to wake up all blocked tasks as
        // they'll never receive a notification, and all connected I/O objects
        // will start returning errors pretty quickly.
        let io = self.io_dispatch.read();
        for (_, io) in io.iter() {
            io.writer.wake();
            io.reader.wake();
        }
    }
}

impl Direction {
    fn mask(&self) -> mio::Ready {
        match *self {
            Direction::Read => {
                // Everything except writable is signaled through read.
                mio::Ready::all() - mio::Ready::writable()
            }
            Direction::Write => mio::Ready::writable() | platform::hup(),
        }
    }
}

#[cfg(unix)]
pub(crate) mod platform {
    use mio::unix::UnixReady;
    use mio::Ready;

    pub fn hup() -> Ready {
        UnixReady::hup().into()
    }

    pub fn is_hup(ready: &Ready) -> bool {
        UnixReady::from(*ready).is_hup()
    }
}

#[cfg(windows)]
pub(crate) mod platform {
    use mio::Ready;

    pub fn hup() -> Ready {
        Ready::empty()
    }

    pub fn is_hup(_: &Ready) -> bool {
        false
    }
}
