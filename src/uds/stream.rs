use super::ucred::{self, UCred};

use crate::raw::PollEvented;

use async_ready::{AsyncReadReady, AsyncWriteReady, TakeError};
use futures::io::{AsyncRead, AsyncWrite};
use futures::{ready, Future, Poll};
use iovec::IoVec;

use std::fmt;
use std::io;
use std::net::Shutdown;
use std::os::unix::io::{AsRawFd, RawFd};
use std::os::unix::net::SocketAddr;
use std::path::Path;
use std::pin::Pin;
use std::task::Context;

/// A structure representing a connected Unix socket.
///
/// This socket can be connected directly with `UnixStream::connect` or accepted
/// from a listener with `UnixListener::incoming`. Additionally, a pair of
/// anonymous Unix sockets can be created with `UnixStream::pair`.
pub struct UnixStream {
    io: PollEvented<mio_uds::UnixStream>,
}

/// Future returned by `UnixStream::connect` which will resolve to a
/// `UnixStream` when the stream is connected.
#[derive(Debug)]
pub struct ConnectFuture {
    inner: State,
}

#[derive(Debug)]
enum State {
    Waiting(UnixStream),
    Error(io::Error),
    Empty,
}

impl Unpin for UnixStream {}

impl UnixStream {
    /// Connects to the socket named by `path`.
    ///
    /// This function will create a new Unix socket and connect to the path
    /// specified, associating the returned stream with the default event loop's
    /// handle.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// #![feature(async_await, await_macro)]
    /// use romio::uds::UnixStream;
    ///
    /// # async fn run() -> std::io::Result<()> {
    /// let stream = await!(UnixStream::connect("/tmp/sock"));
    /// # Ok(()) }
    /// ```
    pub fn connect(path: impl AsRef<Path>) -> ConnectFuture {
        let res = mio_uds::UnixStream::connect(path).map(UnixStream::new);

        let inner = match res {
            Ok(stream) => State::Waiting(stream),
            Err(e) => State::Error(e),
        };

        ConnectFuture { inner }
    }

    /// Creates an unnamed pair of connected sockets.
    ///
    /// This function will create a pair of interconnected Unix sockets for
    /// communicating back and forth between one another. Each socket will be
    /// associated with the event loop whose handle is also provided.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// #![feature(async_await, await_macro)]
    /// use romio::uds::UnixStream;
    ///
    /// # async fn run() -> std::io::Result<()> {
    /// let (sock1, sock2) = UnixStream::pair()?;
    /// # Ok(()) }
    /// ```
    pub fn pair() -> io::Result<(UnixStream, UnixStream)> {
        let (a, b) = mio_uds::UnixStream::pair()?;
        let a = UnixStream::new(a);
        let b = UnixStream::new(b);

        Ok((a, b))
    }

    pub(crate) fn new(stream: mio_uds::UnixStream) -> UnixStream {
        let io = PollEvented::new(stream);
        UnixStream { io }
    }

    /// Returns the socket address of the local half of this connection.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// #![feature(async_await, await_macro)]
    /// use romio::uds::UnixStream;
    ///
    /// # async fn run() -> std::io::Result<()> {
    /// let stream = await!(UnixStream::connect("/tmp/sock"))?;
    /// let addr = stream.local_addr()?;
    /// # Ok(()) }
    /// ```
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.io.get_ref().local_addr()
    }

    /// Returns the socket address of the remote half of this connection.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// #![feature(async_await, await_macro)]
    /// use romio::uds::UnixStream;
    ///
    /// # async fn run() -> std::io::Result<()> {
    /// let stream = await!(UnixStream::connect("/tmp/sock"))?;
    /// let addr = stream.peer_addr()?;
    /// # Ok(()) }
    /// ```
    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        self.io.get_ref().peer_addr()
    }

    /// Returns effective credentials of the process which called `connect` or `socketpair`.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// #![feature(async_await, await_macro)]
    /// use romio::uds::UnixStream;
    ///
    /// # async fn run() -> std::io::Result<()> {
    /// let stream = await!(UnixStream::connect("/tmp/sock"))?;
    /// let cred = stream.peer_cred()?;
    /// # Ok(()) }
    /// ```
    pub fn peer_cred(&self) -> io::Result<UCred> {
        ucred::get_peer_cred(self)
    }

    /// Shuts down the read, write, or both halves of this connection.
    ///
    /// This function will cause all pending and future I/O calls on the
    /// specified portions to immediately return with an appropriate value
    /// (see the documentation of `Shutdown`).
    ///
    /// ```rust
    /// #![feature(async_await, await_macro)]
    /// use romio::uds::UnixStream;
    /// use std::net::Shutdown;
    ///
    /// # async fn run () -> Result<(), Box<dyn std::error::Error + 'static>> {
    /// let stream = await!(UnixStream::connect("/tmp/sock"))?;
    /// stream.shutdown(Shutdown::Both)?;
    /// # Ok(())}
    /// ```
    pub fn shutdown(&self, how: Shutdown) -> io::Result<()> {
        self.io.get_ref().shutdown(how)
    }
}

impl AsyncRead for UnixStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.io).poll_read(cx, buf)
    }

    fn poll_vectored_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &mut [&mut IoVec],
    ) -> Poll<io::Result<usize>> {
        ready!(Pin::new(&mut *self).poll_read_ready(cx)?);

        let r = self.io.get_ref().read_bufs(bufs);

        if is_wouldblock(&r) {
            Pin::new(&mut self.io).clear_read_ready(cx)?;
            Poll::Pending
        } else {
            Poll::Ready(r)
        }
    }
}

impl AsyncWrite for UnixStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.io).poll_write(cx, buf)
    }

    fn poll_vectored_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[&IoVec],
    ) -> Poll<io::Result<usize>> {
        ready!(Pin::new(&mut *self).poll_write_ready(cx)?);

        let r = self.io.get_ref().write_bufs(bufs);

        if is_wouldblock(&r) {
            Pin::new(&mut self.io).clear_write_ready(cx)?;
        }

        return Poll::Ready(r);
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.io).poll_flush(cx)
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.io).poll_close(cx)
    }
}

impl AsyncReadReady for UnixStream {
    type Ok = mio::Ready;
    type Err = io::Error;

    /// Test whether this socket is ready to be read or not.
    fn poll_read_ready(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Self::Ok, Self::Err>> {
        Pin::new(&mut self.io).poll_read_ready(cx)
    }
}

impl AsyncWriteReady for UnixStream {
    type Ok = mio::Ready;
    type Err = io::Error;

    /// Test whether this socket is ready to be written to or not.
    fn poll_write_ready(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Self::Ok, Self::Err>> {
        Pin::new(&mut self.io).poll_write_ready(cx)
    }
}

impl TakeError for UnixStream {
    type Ok = io::Error;
    type Err = io::Error;

    /// Returns the value of the `SO_ERROR` option.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// #![feature(async_await, await_macro)]
    /// use romio::uds::UnixStream;
    /// use romio::raw::TakeError;
    ///
    /// # async fn run() -> std::io::Result<()> {
    /// let stream = await!(UnixStream::connect("/tmp/sock"))?;
    /// if let Ok(Some(err)) = stream.take_error() {
    ///     println!("Got error: {:?}", err);
    /// }
    /// # Ok(()) }
    /// ```
    fn take_error(&self) -> Result<Option<Self::Ok>, Self::Err> {
        self.io.get_ref().take_error()
    }
}

impl fmt::Debug for UnixStream {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.io.get_ref().fmt(f)
    }
}

impl AsRawFd for UnixStream {
    fn as_raw_fd(&self) -> RawFd {
        self.io.get_ref().as_raw_fd()
    }
}

impl Future for ConnectFuture {
    type Output = io::Result<UnixStream>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<UnixStream>> {
        use std::mem;

        match self.inner {
            State::Waiting(ref mut stream) => {
                ready!(stream.io.poll_write_ready(cx)?);

                if let Some(e) = stream.io.get_ref().take_error()? {
                    return Poll::Ready(Err(e));
                }
            }
            State::Error(_) => {
                let e = match mem::replace(&mut self.inner, State::Empty) {
                    State::Error(e) => e,
                    _ => unreachable!(),
                };

                return Poll::Ready(Err(e));
            }
            State::Empty => panic!("can't poll stream twice"),
        }

        match mem::replace(&mut self.inner, State::Empty) {
            State::Waiting(stream) => Poll::Ready(Ok(stream)),
            _ => unreachable!(),
        }
    }
}

fn is_wouldblock<T>(r: &io::Result<T>) -> bool {
    match *r {
        Ok(_) => false,
        Err(ref e) => e.kind() == io::ErrorKind::WouldBlock,
    }
}
