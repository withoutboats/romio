use super::UnixStream;

use crate::raw::PollEvented;

use async_ready::{AsyncReady, TakeError};
use futures::task::Waker;
use futures::{ready, Poll, Stream};
use mio_uds;

use std::fmt;
use std::io;
use std::os::unix::io::{AsRawFd, RawFd};
use std::os::unix::net::{self, SocketAddr};
use std::path::Path;
use std::pin::Pin;

/// A Unix socket which can accept connections from other Unix sockets.
///
/// # Examples
///
/// ```no_run
/// #![feature(async_await, await_macro, futures_api)]
/// use romio::uds::{UnixListener, UnixStream};
/// use futures::prelude::*;
///
/// async fn say_hello(mut stream: UnixStream) {
///     await!(stream.write_all(b"Shall I hear more, or shall I speak at this?!"));
/// }
///
/// async fn listen() -> Result<(), Box<dyn std::error::Error + 'static>> {
///     let listener = UnixListener::bind("/tmp/sock")?;
///     let mut incoming = listener.incoming();
///
///     // accept connections and process them serially
///     while let Some(stream) = await!(incoming.next()) {
///         await!(say_hello(stream?));
///     }
///     Ok(())
/// }
/// ```
pub struct UnixListener {
    io: PollEvented<mio_uds::UnixListener>,
}

impl UnixListener {
    /// Creates a new `UnixListener` bound to the specified path.
    ///
    /// # Examples
    /// Create a Unix Domain Socket on `/tmp/sock`.
    ///
    /// ```rust,no_run
    /// use romio::uds::UnixListener;
    ///
    /// # fn main () -> Result<(), Box<dyn std::error::Error + 'static>> {
    /// let socket = UnixListener::bind("/tmp/sock")?;
    /// # Ok(())}
    /// ```
    ///
    pub fn bind(path: impl AsRef<Path>) -> io::Result<UnixListener> {
        let listener = mio_uds::UnixListener::bind(path)?;
        let io = PollEvented::new(listener);
        Ok(UnixListener { io })
    }

    /// Returns the local socket address of this listener.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use romio::uds::UnixListener;
    ///
    /// # fn main () -> Result<(), Box<dyn std::error::Error + 'static>> {
    /// let socket = UnixListener::bind("/tmp/sock")?;
    /// let addr = socket.local_addr()?;
    /// # Ok(())}
    /// ```
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.io.get_ref().local_addr()
    }

    /// Consumes this listener, returning a stream of the sockets this listener
    /// accepts.
    ///
    /// This method returns an implementation of the `Stream` trait which
    /// resolves to the sockets the are accepted on this listener.
    ///
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// #![feature(async_await, await_macro, futures_api)]
    /// use romio::uds::UnixListener;
    /// use futures::prelude::*;
    ///
    /// # async fn run () -> Result<(), Box<dyn std::error::Error + 'static>> {
    /// let listener = UnixListener::bind("/tmp/sock")?;
    /// let mut incoming = listener.incoming();
    ///
    /// // accept connections and process them serially
    /// while let Some(stream) = await!(incoming.next()) {
    ///     match stream {
    ///         Ok(stream) => {
    ///             println!("new client!");
    ///         },
    ///         Err(e) => { /* connection failed */ }
    ///     }
    /// }
    /// # Ok(())}
    /// ```
    pub fn incoming(self) -> Incoming {
        Incoming::new(self)
    }

    fn poll_accept_std(&self, waker: &Waker) -> Poll<io::Result<(net::UnixStream, SocketAddr)>> {
        ready!(self.io.poll_read_ready(waker)?);

        match self.io.get_ref().accept_std() {
            Ok(Some((sock, addr))) => Poll::Ready(Ok((sock, addr))),
            Ok(None) => {
                self.io.clear_read_ready(waker)?;
                Poll::Pending
            }
            Err(ref err) if err.kind() == io::ErrorKind::WouldBlock => {
                self.io.clear_read_ready(waker)?;
                Poll::Pending
            }
            Err(err) => Poll::Ready(Err(err)),
        }
    }
}

impl AsyncReady for UnixListener {
    type Ok = (UnixStream, SocketAddr);
    type Err = std::io::Error;

    /// Check if the stream can be read from.
    fn poll_ready(&self, waker: &Waker) -> Poll<Result<Self::Ok, Self::Err>> {
        let (io, addr) = ready!(self.poll_accept_std(waker)?);
        let io = mio_uds::UnixStream::from_stream(io)?;
        Poll::Ready(Ok((UnixStream::new(io), addr)))
    }
}

impl TakeError for UnixListener {
    type Ok = io::Error;
    type Err = io::Error;

    /// Returns the value of the `SO_ERROR` option.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use romio::uds::UnixListener;
    /// use romio::async_ready::TakeError;
    ///
    /// # fn main () -> Result<(), Box<dyn std::error::Error + 'static>> {
    /// let listener = UnixListener::bind("/tmp/sock")?;
    /// if let Ok(Some(err)) = listener.take_error() {
    ///     println!("Got error: {:?}", err);
    /// }
    /// # Ok(())}
    /// ```
    fn take_error(&self) -> Result<Option<Self::Ok>, Self::Err> {
        self.io.get_ref().take_error()
    }
}

impl fmt::Debug for UnixListener {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.io.get_ref().fmt(f)
    }
}

impl AsRawFd for UnixListener {
    fn as_raw_fd(&self) -> RawFd {
        self.io.get_ref().as_raw_fd()
    }
}

/// Stream of listeners
#[derive(Debug)]
pub struct Incoming {
    inner: UnixListener,
}

impl Incoming {
    pub(crate) fn new(listener: UnixListener) -> Incoming {
        Incoming { inner: listener }
    }
}

impl Stream for Incoming {
    type Item = io::Result<UnixStream>;

    fn poll_next(self: Pin<&mut Self>, waker: &Waker) -> Poll<Option<Self::Item>> {
        let (socket, _) = ready!(self.inner.poll_ready(waker)?);
        Poll::Ready(Some(Ok(socket)))
    }
}
