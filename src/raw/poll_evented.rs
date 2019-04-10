//! Polling API bindings

use crate::reactor::platform;
use crate::reactor::registration::Registration;

use futures::io::{AsyncRead, AsyncWrite};
use futures::task::Waker;
use futures::{ready, Poll};
use mio;
use mio::event::Evented;

use std::fmt;
use std::io::{self, Read, Write};
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::Relaxed;

/// Associates an I/O resource that implements the [`std::io::Read`] and/or
/// [`std::io::Write`] traits with the reactor that drives it.
///
/// `PollEvented` uses [`Registration`] internally to take a type that
/// implements [`mio::Evented`] as well as [`std::io::Read`] and or
/// [`std::io::Write`] and associate it with a reactor that will drive it.
///
/// Once the [`mio::Evented`] type is wrapped by `PollEvented`, it can be
/// used from within the future's execution model. As such, the `PollEvented`
/// type provides [`AsyncRead`] and [`AsyncWrite`] implementations using the
/// underlying I/O resource as well as readiness events provided by the reactor.
///
/// **Note**: While `PollEvented` is `Sync` (if the underlying I/O type is
/// `Sync`), the caller must ensure that there are at most two tasks that use a
/// `PollEvented` instance concurrently. One for reading and one for writing.
/// While violating this requirement is "safe" from a Rust memory model point of
/// view, it will result in unexpected behavior in the form of lost
/// notifications and tasks hanging.
///
/// ## Readiness events
///
/// Besides just providing [`AsyncRead`] and [`AsyncWrite`] implementations,
/// this type also supports access to the underlying readiness event stream.
/// While similar in function to what [`Registration`] provides, the semantics
/// are a bit different.
///
/// Two functions are provided to access the readiness events:
/// [`poll_read_ready`] and [`poll_write_ready`]. These functions return the
/// current readiness state of the `PollEvented` instance. If
/// [`poll_read_ready`] indicates read readiness, immediately calling
/// [`poll_read_ready`] again will also indicate read readiness.
///
/// When the operation is attempted and is unable to succeed due to the I/O
/// resource not being ready, the caller must call [`clear_read_ready`] or
/// [`clear_write_ready`]. This clears the readiness state until a new readiness
/// event is received.
///
/// This allows the caller to implement additional functions. For example,
/// [`TcpListener`] implements poll_accept by using [`poll_read_ready`] and
/// [`clear_read_ready`].
///
/// ```rust,ignore
/// pub fn poll_accept(&mut self) -> Poll<(net::TcpStream, SocketAddr), io::Error> {
///     let ready = Ready::readable();
///
///     try_ready!(self.poll_evented.poll_read_ready(ready));
///
///     match self.poll_evented.get_ref().accept_std() {
///         Ok(pair) => Ok(Async::Ready(pair)),
///         Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
///             self.poll_evented.clear_read_ready(ready);
///             Ok(Async::NotReady)
///         }
///         Err(e) => Err(e),
///     }
/// }
/// ```
///
/// ## Platform-specific events
///
/// `PollEvented` also allows receiving platform-specific `mio::Ready` events.
/// These events are included as part of the read readiness event stream. The
/// write readiness event stream is only for `Ready::writable()` events.
///
/// [`std::io::Read`]: https://doc.rust-lang.org/std/io/trait.Read.html
/// [`std::io::Write`]: https://doc.rust-lang.org/std/io/trait.Write.html
/// [`AsyncRead`]: ../io/trait.AsyncRead.html
/// [`AsyncWrite`]: ../io/trait.AsyncWrite.html
/// [`mio::Evented`]: https://docs.rs/mio/0.6/mio/trait.Evented.html
/// [`Registration`]: struct.Registration.html
/// [`TcpListener`]: ../net/struct.TcpListener.html
/// [`clear_read_ready`]: #method.clear_read_ready
/// [`clear_write_ready`]: #method.clear_write_ready
/// [`poll_read_ready`]: #method.poll_read_ready
/// [`poll_write_ready`]: #method.poll_write_ready
pub struct PollEvented<E: Evented> {
    io: Option<E>,
    inner: Inner,
}

struct Inner {
    registration: Registration,

    /// Currently visible read readiness
    read_readiness: AtomicUsize,

    /// Currently visible write readiness
    write_readiness: AtomicUsize,
}

// ===== impl PollEvented =====

impl<E> PollEvented<E>
where
    E: Evented,
{
    /// Creates a new `PollEvented` associated with the default reactor.
    pub fn new(io: E) -> PollEvented<E> {
        PollEvented {
            io: Some(io),
            inner: Inner {
                registration: Registration::new(),
                read_readiness: AtomicUsize::new(0),
                write_readiness: AtomicUsize::new(0),
            },
        }
    }

    /// Returns a shared reference to the underlying I/O object this readiness
    /// stream is wrapping.
    pub fn get_ref(&self) -> &E {
        self.io.as_ref().unwrap()
    }

    /// Returns a mutable reference to the underlying I/O object this readiness
    /// stream is wrapping.
    pub fn get_mut(&mut self) -> &mut E {
        self.io.as_mut().unwrap()
    }

    // TODO: restore this once we make reactor::poll_evented public
    // /// Consumes self, returning the inner I/O object
    // ///
    // /// This function will deregister the I/O resource from the reactor before
    // /// returning. If the deregistration operation fails, an error is returned.
    // ///
    // /// Note that deregistering does not guarantee that the I/O resource can be
    // /// registered with a different reactor. Some I/O resource types can only be
    // /// associated with a single reactor instance for their lifetime.
    // pub fn into_inner(mut self) -> io::Result<E> {
    //     let io = self.io.take().unwrap();
    //     self.inner.registration.deregister(&io)?;
    //     Ok(io)
    // }

    /// Check the I/O resource's read readiness state.
    ///
    /// The mask argument allows specifying what readiness to notify on. This
    /// can be any value, including platform specific readiness, **except**
    /// `writable`. HUP is always implicitly included on platforms that support
    /// it.
    ///
    /// If the resource is not ready for a read then `Async::NotReady` is
    /// returned and the current task is notified once a new event is received.
    ///
    /// The I/O resource will remain in a read-ready state until readiness is
    /// cleared by calling [`clear_read_ready`].
    ///
    /// [`clear_read_ready`]: #method.clear_read_ready
    pub fn poll_read_ready(&self, waker: &Waker) -> Poll<io::Result<mio::Ready>> {
        self.register()?;

        // Load cached & encoded readiness.
        let mut cached = self.inner.read_readiness.load(Relaxed);
        let mask = mio::Ready::readable() | platform::hup();

        // See if the current readiness matches any bits.
        let mut ret = mio::Ready::from_usize(cached) & mio::Ready::readable();

        if ret.is_empty() {
            // Readiness does not match, consume the registration's readiness
            // stream. This happens in a loop to ensure that the stream gets
            // drained.
            loop {
                let ready = ready!(self.inner.registration.poll_read_ready(waker)?);
                cached |= ready.as_usize();

                // Update the cache store
                self.inner.read_readiness.store(cached, Relaxed);

                ret |= ready & mask;

                if !ret.is_empty() {
                    return Poll::Ready(Ok(ret));
                }
            }
        } else {
            // Check what's new with the registration stream. This will not
            // request to be notified
            if let Some(ready) = self.inner.registration.take_read_ready()? {
                cached |= ready.as_usize();
                self.inner.read_readiness.store(cached, Relaxed);
            }

            Poll::Ready(Ok(mio::Ready::from_usize(cached)))
        }
    }

    /// Clears the I/O resource's read readiness state and registers the current
    /// task to be notified once a read readiness event is received.
    ///
    /// After calling this function, `poll_read_ready` will return `NotReady`
    /// until a new read readiness event has been received.
    ///
    /// The `mask` argument specifies the readiness bits to clear. This may not
    /// include `writable` or `hup`.
    pub fn clear_read_ready(&self, waker: &Waker) -> io::Result<()> {
        self.inner
            .read_readiness
            .fetch_and(!mio::Ready::readable().as_usize(), Relaxed);

        if self.poll_read_ready(waker)?.is_ready() {
            // Notify the current task
            waker.wake();
        }

        Ok(())
    }

    /// Check the I/O resource's write readiness state.
    ///
    /// This always checks for writable readiness and also checks for HUP
    /// readiness on platforms that support it.
    ///
    /// If the resource is not ready for a write then `Async::NotReady` is
    /// returned and the current task is notified once a new event is received.
    ///
    /// The I/O resource will remain in a write-ready state until readiness is
    /// cleared by calling [`clear_write_ready`].
    ///
    /// [`clear_write_ready`]: #method.clear_write_ready
    ///
    /// # Panics
    ///
    /// This function panics if:
    ///
    /// * `ready` contains bits besides `writable` and `hup`.
    /// * called from outside of a task context.
    pub fn poll_write_ready(&self, waker: &Waker) -> Poll<Result<mio::Ready, io::Error>> {
        self.register()?;

        // Load cached & encoded readiness.
        let mut cached = self.inner.write_readiness.load(Relaxed);
        let mask = mio::Ready::writable() | platform::hup();

        // See if the current readiness matches any bits.
        let mut ret = mio::Ready::from_usize(cached) & mio::Ready::writable();

        if ret.is_empty() {
            // Readiness does not match, consume the registration's readiness
            // stream. This happens in a loop to ensure that the stream gets
            // drained.
            loop {
                let ready = ready!(self.inner.registration.poll_write_ready(waker)?);
                cached |= ready.as_usize();

                // Update the cache store
                self.inner.write_readiness.store(cached, Relaxed);

                ret |= ready & mask;

                if !ret.is_empty() {
                    return Poll::Ready(Ok(ret));
                }
            }
        } else {
            // Check what's new with the registration stream. This will not
            // request to be notified
            if let Some(ready) = self.inner.registration.take_write_ready()? {
                cached |= ready.as_usize();
                self.inner.write_readiness.store(cached, Relaxed);
            }

            Poll::Ready(Ok(mio::Ready::from_usize(cached)))
        }
    }

    /// Resets the I/O resource's write readiness state and registers the current
    /// task to be notified once a write readiness event is received.
    ///
    /// This only clears writable readiness. HUP (on platforms that support HUP)
    /// cannot be cleared as it is a final state.
    ///
    /// After calling this function, `poll_write_ready(Ready::writable())` will
    /// return `NotReady` until a new write readiness event has been received.
    ///
    /// # Panics
    ///
    /// This function will panic if called from outside of a task context.
    pub fn clear_write_ready(&self, waker: &Waker) -> io::Result<()> {
        self.inner
            .write_readiness
            .fetch_and(!mio::Ready::writable().as_usize(), Relaxed);

        if self.poll_write_ready(waker)?.is_ready() {
            // Notify the current task
            waker.wake();
        }

        Ok(())
    }

    /// Ensure that the I/O resource is registered with the reactor.
    fn register(&self) -> io::Result<()> {
        self.inner
            .registration
            .register(self.io.as_ref().unwrap())?;
        Ok(())
    }
}

// ===== AsyncRead / AsyncWrite impls =====

impl<E> AsyncRead for PollEvented<E>
where
    E: Evented + Read,
{
    fn poll_read(&mut self, waker: &Waker, buf: &mut [u8]) -> Poll<io::Result<usize>> {
        ready!(self.poll_read_ready(waker)?);

        let r = self.get_mut().read(buf);

        if is_wouldblock(&r) {
            self.clear_read_ready(waker)?;
            Poll::Pending
        } else {
            Poll::Ready(r)
        }
    }
}

impl<E> AsyncWrite for PollEvented<E>
where
    E: Evented + Write,
{
    fn poll_write(&mut self, waker: &Waker, buf: &[u8]) -> Poll<io::Result<usize>> {
        ready!(self.poll_write_ready(waker)?);

        let r = self.get_mut().write(buf);

        if is_wouldblock(&r) {
            self.clear_write_ready(waker)?;
            Poll::Pending
        } else {
            Poll::Ready(r)
        }
    }

    fn poll_flush(&mut self, waker: &Waker) -> Poll<io::Result<()>> {
        ready!(self.poll_write_ready(waker)?);

        let r = self.get_mut().flush();

        if is_wouldblock(&r) {
            self.clear_write_ready(waker)?;
            Poll::Pending
        } else {
            Poll::Ready(r)
        }
    }

    fn poll_close(&mut self, _: &Waker) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

// ===== &'a AsyncRead / &'a AsyncWrite impls =====

impl<'a, E> AsyncRead for &'a PollEvented<E>
where
    E: Evented,
    &'a E: Read,
{
    fn poll_read(&mut self, waker: &Waker, buf: &mut [u8]) -> Poll<io::Result<usize>> {
        ready!(self.poll_read_ready(waker)?);

        let r = self.get_ref().read(buf);

        if is_wouldblock(&r) {
            self.clear_read_ready(waker)?;
            Poll::Pending
        } else {
            Poll::Ready(r)
        }
    }
}

impl<'a, E> AsyncWrite for &'a PollEvented<E>
where
    E: Evented,
    &'a E: Write,
{
    fn poll_write(&mut self, waker: &Waker, buf: &[u8]) -> Poll<io::Result<usize>> {
        ready!(self.poll_write_ready(waker)?);

        let r = self.get_ref().write(buf);

        if is_wouldblock(&r) {
            self.clear_write_ready(waker)?;
            Poll::Pending
        } else {
            Poll::Ready(r)
        }
    }

    fn poll_flush(&mut self, waker: &Waker) -> Poll<io::Result<()>> {
        ready!(self.poll_write_ready(waker)?);

        let r = self.get_ref().flush();

        if is_wouldblock(&r) {
            self.clear_write_ready(waker)?;
            Poll::Pending
        } else {
            Poll::Ready(r)
        }
    }

    fn poll_close(&mut self, _: &Waker) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

fn is_wouldblock<T>(r: &io::Result<T>) -> bool {
    match *r {
        Ok(_) => false,
        Err(ref e) => e.kind() == io::ErrorKind::WouldBlock,
    }
}

impl<E: Evented + fmt::Debug> fmt::Debug for PollEvented<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PollEvented").field("io", &self.io).finish()
    }
}

impl<E: Evented> Drop for PollEvented<E> {
    fn drop(&mut self) {
        if let Some(io) = self.io.take() {
            // Ignore errors
            let _ = self.inner.registration.deregister(&io);
        }
    }
}
