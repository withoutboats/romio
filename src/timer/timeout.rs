//! Allows a future or stream to execute for a maximum amount of time.
//!
//! See [`Timeout`] documentation for more details.
//!
//! [`Timeout`]: struct.Timeout.html
use super::Delay;
use super::clock::now;

use futures::{Future, Stream, Poll};
use futures::task::LocalWaker;

use std::error;
use std::fmt;
use std::pin::Pin;
use std::time::{Instant, Duration};

/// Allows a `Future` or `Stream` to execute for a limited amount of time.
///
/// If the future or stream completes before the timeout has expired, then
/// `Timeout` returns the completed value. Otherwise, `Timeout` returns an
/// [`Error`].
///
/// # Futures and Streams
///
/// The exact behavor depends on if the inner value is a `Future` or a `Stream`.
/// In the case of a `Future`, `Timeout` will require the future to complete by
/// a fixed deadline. In the case of a `Stream`, `Timeout` will allow each item
/// to take the entire timeout before returning an error.
///
/// In order to set an upper bound on the processing of the *entire* stream,
/// then a timeout should be set on the future that processes the stream. For
/// example:
///
/// ```rust
/// # extern crate futures;
/// # extern crate tokio;
/// // import the `timeout` function, usually this is done
/// // with `use tokio::prelude::*`
/// use tokio::prelude::FutureExt;
/// use futures::Stream;
/// use futures::sync::mpsc;
/// use std::time::Duration;
///
/// # fn main() {
/// let (tx, rx) = mpsc::unbounded();
/// # tx.unbounded_send(()).unwrap();
/// # drop(tx);
///
/// let process = rx.for_each(|item| {
///     // do something with `item`
/// # drop(item);
/// # Ok(())
/// });
///
/// # tokio::runtime::current_thread::block_on_all(
/// // Wrap the future with a `Timeout` set to expire in 10 milliseconds.
/// process.timeout(Duration::from_millis(10))
/// # ).unwrap();
/// # }
/// ```
///
/// # Cancelation
///
/// Cancelling a `Timeout` is done by dropping the value. No additional cleanup
/// or other work is required.
///
/// The original future or stream may be obtained by calling [`into_inner`]. This
/// consumes the `Timeout`.
///
/// [`Error`]: struct.Error.html
#[must_use = "futures do nothing unless polled"]
#[derive(Debug)]
pub struct Timeout<T> {
    value: T,
    delay: Delay,
}

/// Error returned by `Timeout`.
#[derive(Debug)]
pub struct Error(Kind);

/// Timeout error variants
#[derive(Debug)]
enum Kind {
    /// The timeout elapsed.
    Elapsed,

    /// Timer returned an error.
    Timer(super::Error),
}

impl<T> Timeout<T> {
    /// Create a new `Timeout` that allows `value` to execute for a duration of
    /// at most `timeout`.
    ///
    /// The exact behavior depends on if `value` is a `Future` or a `Stream`.
    ///
    /// See [type] level documentation for more details.
    ///
    /// [type]: #
    ///
    /// # Examples
    ///
    /// Create a new `Timeout` set to expire in 10 milliseconds.
    ///
    /// ```rust
    /// # extern crate futures;
    /// # extern crate tokio;
    /// use tokio::timer::Timeout;
    /// use futures::Future;
    /// use futures::sync::oneshot;
    /// use std::time::Duration;
    ///
    /// # fn main() {
    /// let (tx, rx) = oneshot::channel();
    /// # tx.send(()).unwrap();
    ///
    /// # tokio::runtime::current_thread::block_on_all(
    /// // Wrap the future with a `Timeout` set to expire in 10 milliseconds.
    /// Timeout::new(rx, Duration::from_millis(10))
    /// # ).unwrap();
    /// # }
    /// ```
    pub fn new(value: T, timeout: Duration) -> Timeout<T> {
        let delay = Delay::new_timeout(now() + timeout, timeout);

        Timeout {
            value,
            delay,
        }
    }

    /// Gets a reference to the underlying value in this timeout.
    pub fn get_ref(&self) -> &T {
        &self.value
    }

    /// Gets a mutable reference to the underlying value in this timeout.
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.value
    }

    /// Consumes this timeout, returning the underlying value.
    pub fn into_inner(self) -> T {
        self.value
    }

    fn value<'a>(self: Pin<&'a mut Self>) -> Pin<&'a mut T> {
        unsafe {
            Pin::map_unchecked_mut(self, |this| &mut this.value)
        }
    }

    fn delay<'a>(self: Pin<&'a mut Self>) -> Pin<&'a mut Delay> {
        unsafe {
            Pin::map_unchecked_mut(self, |this| &mut this.delay)
        }
    }
}

impl<T: Future> Timeout<T> {
    /// Create a new `Timeout` that completes when `future` completes or when
    /// `deadline` is reached.
    ///
    /// This function differs from `new` in that:
    ///
    /// * It only accepts `Future` arguments.
    /// * It sets an explicit `Instant` at which the timeout expires.
    pub fn new_at(future: T, deadline: Instant) -> Timeout<T> {
        let delay = Delay::new(deadline);

        Timeout {
            value: future,
            delay,
        }
    }
}

impl<T> Future for Timeout<T>
where T: Future,
{
    type Output = Result<T::Output, Error>;

    fn poll(mut self: Pin<&mut Self>, lw: &LocalWaker) -> Poll<Self::Output> {
        // First, try polling the future
        if let Poll::Ready(v) = self.as_mut().value().poll(lw) {
            return Poll::Ready(Ok(v))
        }

        // Now check the timer
        match self.as_mut().delay().poll(lw)? {
            Poll::Ready(_)  => Poll::Ready(Err(Error::elapsed())),
            Poll::Pending   => Poll::Pending,
        }
    }
}

impl<T> Stream for Timeout<T>
where T: Stream,
{
    type Item = Result<T::Item, Error>;

    fn poll_next(mut self: Pin<&mut Self>, lw: &LocalWaker) -> Poll<Option<Result<T::Item, Error>>> {
        // First, try polling the future
        match self.as_mut().value().poll_next(lw) {
            Poll::Ready(Some(v)) => {
                self.as_mut().delay().reset_timeout();
                return Poll::Ready(Some(Ok(v)));
            }
            Poll::Ready(None) => {
                return Poll::Ready(None)
            }
            Poll::Pending => { }
        }

        // Now check the timer
        match self.as_mut().delay().poll(lw) {
            Poll::Ready(_)  => {
                self.as_mut().delay().reset_timeout();
                Poll::Ready(Some(Err(Error::elapsed())))
            }
            Poll::Pending   => Poll::Pending,
        }
    }
}

// ===== impl Error =====

impl Error {
    /// Create a new `Error` representing the inner value not completing before
    /// the deadline is reached.
    pub fn elapsed() -> Error {
        Error(Kind::Elapsed)
    }

    /// Returns `true` if the error was caused by the inner value not completing
    /// before the deadline is reached.
    pub fn is_elapsed(&self) -> bool {
        match self.0 {
            Kind::Elapsed => true,
            _ => false,
        }
    }

    /// Creates a new `Error` representing an error encountered by the timer
    /// implementation
    pub fn timer(err: super::Error) -> Error {
        Error(Kind::Timer(err))
    }

    /// Returns `true` if the error was caused by the timer.
    pub fn is_timer(&self) -> bool {
        match self.0 {
            Kind::Timer(_) => true,
            _ => false,
        }
    }

    /// Consumes `self`, returning the error raised by the timer implementation.
    pub fn into_timer(self) -> Option<super::Error> {
        match self.0 {
            Kind::Timer(err) => Some(err),
            _ => None,
        }
    }
}

impl error::Error for Error {
    fn description(&self) -> &str {
        use self::Kind::*;

        match self.0 {
            Elapsed => "deadline has elapsed",
            Timer(ref e) => e.description(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        use self::Kind::*;

        match self.0 {
            Elapsed => "deadline has elapsed".fmt(fmt),
            Timer(ref e) => e.fmt(fmt),
        }
    }
}

impl From<super::Error> for Error {
    fn from(error: super::Error) -> Error {
        Error::timer(error)
    }
}
