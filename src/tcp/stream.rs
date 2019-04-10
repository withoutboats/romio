use std::fmt;
use std::io;
use std::mem;
use std::net::{Shutdown, SocketAddr};
use std::pin::Pin;
use std::time::Duration;

use async_ready::{AsyncReadReady, AsyncWriteReady};
use futures::io::{AsyncRead, AsyncWrite};
use futures::task::Waker;
use futures::{ready, Future, Poll};
use iovec::IoVec;
use mio;

use crate::raw::PollEvented;

/// A TCP stream between a local and a remote socket.
///
/// A `TcpStream` can either be created by connecting to an endpoint, via the
/// [`connect`] method, or by [accepting] a connection from a [listener].
/// It can be read or written to using the `AsyncRead`, `AsyncWrite`, and related
/// extension traits in `futures::io`.
///
/// The connection will be closed when the value is dropped. The reading and writing
/// portions of the connection can also be shut down individually with the [`shutdown`]
/// method.
///
/// [`connect`]: struct.TcpStream.html#method.connect
/// [accepting]: struct.TcpListener.html#method.accept
/// [listener]: struct.TcpListener.html
pub struct TcpStream {
    io: PollEvented<mio::net::TcpStream>,
}

/// The future returned by `TcpStream::connect`, which will resolve to a `TcpStream`
/// when the stream is connected.
#[must_use = "futures do nothing unless polled"]
#[derive(Debug)]
pub struct ConnectFuture {
    inner: ConnectFutureState,
}

#[must_use = "futures do nothing unless polled"]
#[derive(Debug)]
enum ConnectFutureState {
    Waiting(TcpStream),
    Error(io::Error),
    Empty,
}

impl TcpStream {
    /// Create a new TCP stream connected to the specified address.
    ///
    /// This function will create a new TCP socket and attempt to connect it to
    /// the `addr` provided. The returned future will be resolved once the
    /// stream has successfully connected, or it will return an error if one
    /// occurs.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// #![feature(async_await, await_macro, futures_api)]
    /// # use std::io;
    /// use romio::tcp::TcpStream;
    ///
    /// # async fn connect_localhost() -> io::Result<TcpStream> {
    /// let addr = "127.0.0.1".parse().unwrap();
    /// await!(TcpStream::connect(&addr))
    /// # }
    /// ```
    pub fn connect(addr: &SocketAddr) -> ConnectFuture {
        use self::ConnectFutureState::*;

        let inner = match mio::net::TcpStream::connect(addr) {
            Ok(tcp) => Waiting(TcpStream::new(tcp)),
            Err(e) => Error(e),
        };

        ConnectFuture { inner }
    }

    pub(crate) fn new(connected: mio::net::TcpStream) -> TcpStream {
        let io = PollEvented::new(connected);
        TcpStream { io }
    }

    /// Returns the local address that this stream is bound to.
    ///
    /// # Examples
    ///
    /// ```rust
    /// #![feature(async_await, await_macro, futures_api)]
    /// use romio::tcp::TcpStream;
    /// use std::net::{IpAddr, Ipv4Addr};
    ///
    /// # async fn run () -> Result<(), Box<dyn std::error::Error + 'static>> {
    /// let addr = "127.0.0.1:8080".parse()?;
    /// let stream = await!(TcpStream::connect(&addr))?;
    ///
    /// let expected = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
    /// assert_eq!(stream.local_addr()?.ip(), expected);
    /// # Ok(())}
    /// ```
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.io.get_ref().local_addr()
    }

    /// Returns the remote address that this stream is connected to.
    ///
    /// # Examples
    ///
    /// ```rust
    /// #![feature(async_await, await_macro, futures_api)]
    /// use romio::tcp::TcpStream;
    /// use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
    ///
    /// # async fn run () -> Result<(), Box<dyn std::error::Error + 'static>> {
    /// let addr = "127.0.0.1:8080".parse()?;
    /// let stream = await!(TcpStream::connect(&addr))?;
    ///
    /// let expected = SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 8080);
    /// assert_eq!(stream.peer_addr()?, SocketAddr::V4(expected));
    /// # Ok(())}
    /// ```
    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        self.io.get_ref().peer_addr()
    }

    /// Shuts down the read, write, or both halves of this connection.
    ///
    /// This function will cause all pending and future I/O on the specified
    /// portions to return immediately with an appropriate value (see the
    /// documentation of `Shutdown`).
    ///
    /// # Examples
    ///
    /// ```rust
    /// #![feature(async_await, await_macro, futures_api)]
    /// use romio::tcp::TcpStream;
    /// use std::net::Shutdown;
    ///
    /// # async fn run () -> Result<(), Box<dyn std::error::Error + 'static>> {
    /// let addr = "127.0.0.1:8080".parse()?;
    /// let stream = await!(TcpStream::connect(&addr))?;
    ///
    /// stream.shutdown(Shutdown::Both)?;
    /// # Ok(())}
    /// ```
    pub fn shutdown(&self, how: Shutdown) -> io::Result<()> {
        self.io.get_ref().shutdown(how)
    }

    /// Gets the value of the `TCP_NODELAY` option on this socket.
    ///
    /// For more information about this option, see [`set_nodelay`].
    ///
    /// [`set_nodelay`]: #method.set_nodelay
    ///
    /// # Examples
    ///
    /// ```rust
    /// #![feature(async_await, await_macro, futures_api)]
    /// use romio::tcp::TcpStream;
    ///
    /// # async fn run () -> Result<(), Box<dyn std::error::Error + 'static>> {
    /// let addr = "127.0.0.1:8080".parse()?;
    /// let stream = await!(TcpStream::connect(&addr))?;
    ///
    /// stream.set_nodelay(true)?;
    /// assert_eq!(stream.nodelay()?, true);
    /// # Ok(())}
    /// ```
    pub fn nodelay(&self) -> io::Result<bool> {
        self.io.get_ref().nodelay()
    }

    /// Sets the value of the `TCP_NODELAY` option on this socket.
    ///
    /// If set, this option disables the Nagle algorithm. This means that
    /// segments are always sent as soon as possible, even if there is only a
    /// small amount of data. When not set, data is buffered until there is a
    /// sufficient amount to send out, thereby avoiding the frequent sending of
    /// small packets.
    ///
    /// # Examples
    ///
    /// ```rust
    /// #![feature(async_await, await_macro, futures_api)]
    /// use romio::tcp::TcpStream;
    ///
    /// # async fn run () -> Result<(), Box<dyn std::error::Error + 'static>> {
    /// let addr = "127.0.0.1:8080".parse()?;
    /// let stream = await!(TcpStream::connect(&addr))?;
    ///
    /// stream.set_nodelay(true)?;
    /// # Ok(())}
    /// ```
    pub fn set_nodelay(&self, nodelay: bool) -> io::Result<()> {
        self.io.get_ref().set_nodelay(nodelay)
    }

    /// Gets the value of the `SO_RCVBUF` option on this socket.
    ///
    /// For more information about this option, see [`set_recv_buffer_size`].
    ///
    /// [`set_recv_buffer_size`]: #tymethod.set_recv_buffer_size
    ///
    /// # Examples
    ///
    /// ```rust
    /// #![feature(async_await, await_macro, futures_api)]
    /// use romio::tcp::TcpStream;
    ///
    /// # async fn run () -> Result<(), Box<dyn std::error::Error + 'static>> {
    /// let addr = "127.0.0.1:8080".parse()?;
    /// let stream = await!(TcpStream::connect(&addr))?;
    ///
    /// stream.set_recv_buffer_size(100);
    /// assert_eq!(stream.recv_buffer_size()?, 100);
    /// # Ok(())}
    /// ```
    pub fn recv_buffer_size(&self) -> io::Result<usize> {
        self.io.get_ref().recv_buffer_size()
    }

    /// Sets the value of the `SO_RCVBUF` option on this socket.
    ///
    /// Changes the size of the operating system's receive buffer associated
    /// with the socket.
    ///
    /// # Examples
    ///
    /// ```rust
    /// #![feature(async_await, await_macro, futures_api)]
    /// use romio::tcp::TcpStream;
    ///
    /// # async fn run () -> Result<(), Box<dyn std::error::Error + 'static>> {
    /// let addr = "127.0.0.1:8080".parse()?;
    /// let stream = await!(TcpStream::connect(&addr))?;
    ///
    /// stream.set_recv_buffer_size(100);
    /// # Ok(())}
    /// ```
    pub fn set_recv_buffer_size(&self, size: usize) -> io::Result<()> {
        self.io.get_ref().set_recv_buffer_size(size)
    }

    /// Gets the value of the `SO_SNDBUF` option on this socket.
    ///
    /// For more information about this option, see [`set_send_buffer`].
    ///
    /// [`set_send_buffer`]: #tymethod.set_send_buffer
    ///
    /// # Examples
    ///
    /// ```rust
    /// #![feature(async_await, await_macro, futures_api)]
    /// use romio::tcp::TcpStream;
    ///
    /// # async fn run () -> Result<(), Box<dyn std::error::Error + 'static>> {
    /// let addr = "127.0.0.1:8080".parse()?;
    /// let stream = await!(TcpStream::connect(&addr))?;
    ///
    /// stream.set_send_buffer_size(100);
    /// assert_eq!(stream.send_buffer_size()?, 100);
    /// # Ok(())}
    /// ```
    pub fn send_buffer_size(&self) -> io::Result<usize> {
        self.io.get_ref().send_buffer_size()
    }

    /// Sets the value of the `SO_SNDBUF` option on this socket.
    ///
    /// Changes the size of the operating system's send buffer associated with
    /// the socket.
    ///
    /// # Examples
    ///
    /// ```rust
    /// #![feature(async_await, await_macro, futures_api)]
    /// use romio::tcp::TcpStream;
    ///
    /// # async fn run () -> Result<(), Box<dyn std::error::Error + 'static>> {
    /// let addr = "127.0.0.1:8080".parse()?;
    /// let stream = await!(TcpStream::connect(&addr))?;
    ///
    /// stream.set_send_buffer_size(100);
    /// # Ok(())}
    /// ```
    pub fn set_send_buffer_size(&self, size: usize) -> io::Result<()> {
        self.io.get_ref().set_send_buffer_size(size)
    }

    /// Returns whether keepalive messages are enabled on this socket, and if so
    /// the duration of time between them.
    ///
    /// For more information about this option, see [`set_keepalive`].
    ///
    /// [`set_keepalive`]: #tymethod.set_keepalive
    ///
    /// # Examples
    ///
    /// ```rust
    /// #![feature(async_await, await_macro, futures_api)]
    /// use romio::tcp::TcpStream;
    /// use std::time::Duration;
    ///
    /// # async fn run () -> Result<(), Box<dyn std::error::Error + 'static>> {
    /// let addr = "127.0.0.1:8080".parse()?;
    /// let stream = await!(TcpStream::connect(&addr))?;
    ///
    /// stream.set_keepalive(Some(Duration::from_secs(60)))?;
    /// assert_eq!(stream.keepalive()?, Some(Duration::from_secs(60)));
    /// # Ok(())}
    /// ```
    pub fn keepalive(&self) -> io::Result<Option<Duration>> {
        self.io.get_ref().keepalive()
    }

    /// Sets whether keepalive messages are enabled to be sent on this socket.
    ///
    /// On Unix, this option will set the `SO_KEEPALIVE` as well as the
    /// `TCP_KEEPALIVE` or `TCP_KEEPIDLE` option (depending on your platform).
    /// On Windows, this will set the `SIO_KEEPALIVE_VALS` option.
    ///
    /// If `None` is specified then keepalive messages are disabled, otherwise
    /// the duration specified will be the time to remain idle before sending a
    /// TCP keepalive probe.
    ///
    /// Some platforms specify this value in seconds, so sub-second
    /// specifications may be omitted.
    ///
    /// # Examples
    ///
    /// ```rust
    /// #![feature(async_await, await_macro, futures_api)]
    /// use romio::tcp::TcpStream;
    /// use std::time::Duration;
    ///
    /// # async fn run () -> Result<(), Box<dyn std::error::Error + 'static>> {
    /// let addr = "127.0.0.1:8080".parse()?;
    /// let stream = await!(TcpStream::connect(&addr))?;
    ///
    /// stream.set_keepalive(Some(Duration::from_secs(60)))?;
    /// # Ok(())}
    /// ```
    pub fn set_keepalive(&self, keepalive: Option<Duration>) -> io::Result<()> {
        self.io.get_ref().set_keepalive(keepalive)
    }

    /// Gets the value of the `IP_TTL` option for this socket.
    ///
    /// For more information about this option, see [`set_ttl`].
    ///
    /// [`set_ttl`]: #tymethod.set_ttl
    ///
    /// # Examples
    ///
    /// ```rust
    /// #![feature(async_await, await_macro, futures_api)]
    /// use romio::tcp::TcpStream;
    ///
    /// # async fn run () -> Result<(), Box<dyn std::error::Error + 'static>> {
    /// let addr = "127.0.0.1:8080".parse()?;
    /// let stream = await!(TcpStream::connect(&addr))?;
    ///
    /// stream.set_ttl(100)?;
    /// assert_eq!(stream.ttl()?, 100);
    /// # Ok(())}
    /// ```
    pub fn ttl(&self) -> io::Result<u32> {
        self.io.get_ref().ttl()
    }

    /// Sets the value for the `IP_TTL` option on this socket.
    ///
    /// This value sets the time-to-live field that is used in every packet sent
    /// from this socket.
    ///
    /// # Examples
    ///
    /// ```rust
    /// #![feature(async_await, await_macro, futures_api)]
    /// use romio::tcp::TcpStream;
    ///
    /// # async fn run () -> Result<(), Box<dyn std::error::Error + 'static>> {
    /// let addr = "127.0.0.1:8080".parse()?;
    /// let stream = await!(TcpStream::connect(&addr))?;
    ///
    /// stream.set_ttl(100)?;
    /// # Ok(())}
    /// ```
    pub fn set_ttl(&self, ttl: u32) -> io::Result<()> {
        self.io.get_ref().set_ttl(ttl)
    }

    /// Reads the linger duration for this socket by getting the `SO_LINGER`
    /// option.
    ///
    /// For more information about this option, see [`set_linger`].
    ///
    /// [`set_linger`]: #tymethod.set_linger
    ///
    /// # Examples
    ///
    /// ```rust
    /// #![feature(async_await, await_macro, futures_api)]
    /// use romio::tcp::TcpStream;
    /// use std::time::Duration;
    ///
    /// # async fn run () -> Result<(), Box<dyn std::error::Error + 'static>> {
    /// let addr = "127.0.0.1:8080".parse()?;
    /// let stream = await!(TcpStream::connect(&addr))?;
    ///
    /// stream.set_linger(Some(Duration::from_millis(100)))?;
    /// assert_eq!(stream.linger()?, Some(Duration::from_millis(100)));
    /// # Ok(())}
    /// ```
    pub fn linger(&self) -> io::Result<Option<Duration>> {
        self.io.get_ref().linger()
    }

    /// Sets the linger duration of this socket by setting the `SO_LINGER`
    /// option.
    ///
    /// This option controls the action taken when a stream has unsent messages
    /// and the stream is closed. If `SO_LINGER` is set, the system
    /// shall block the process  until it can transmit the data or until the
    /// time expires.
    ///
    /// If `SO_LINGER` is not specified, and the stream is closed, the system
    /// handles the call in a way that allows the process to continue as quickly
    /// as possible.
    ///
    /// # Examples
    ///
    /// ```rust
    /// #![feature(async_await, await_macro, futures_api)]
    /// use romio::tcp::TcpStream;
    /// use std::time::Duration;
    ///
    /// # async fn run () -> Result<(), Box<dyn std::error::Error + 'static>> {
    /// let addr = "127.0.0.1:8080".parse()?;
    /// let stream = await!(TcpStream::connect(&addr))?;
    ///
    /// stream.set_linger(Some(Duration::from_millis(100)))?;
    /// # Ok(())}
    /// ```
    pub fn set_linger(&self, dur: Option<Duration>) -> io::Result<()> {
        self.io.get_ref().set_linger(dur)
    }
}

// ===== impl Read / Write =====

impl AsyncRead for TcpStream {
    fn poll_read(&mut self, waker: &Waker, buf: &mut [u8]) -> Poll<io::Result<usize>> {
        <&TcpStream>::poll_read(&mut &*self, waker, buf)
    }

    fn poll_vectored_read(
        &mut self,
        waker: &Waker,
        vec: &mut [&mut IoVec],
    ) -> Poll<io::Result<usize>> {
        <&TcpStream>::poll_vectored_read(&mut &*self, waker, vec)
    }
}

impl AsyncWrite for TcpStream {
    fn poll_write(&mut self, waker: &Waker, buf: &[u8]) -> Poll<io::Result<usize>> {
        <&TcpStream>::poll_write(&mut &*self, waker, buf)
    }

    fn poll_vectored_write(&mut self, waker: &Waker, vec: &[&IoVec]) -> Poll<io::Result<usize>> {
        <&TcpStream>::poll_vectored_write(&mut &*self, waker, vec)
    }

    fn poll_flush(&mut self, waker: &Waker) -> Poll<io::Result<()>> {
        <&TcpStream>::poll_flush(&mut &*self, waker)
    }

    fn poll_close(&mut self, waker: &Waker) -> Poll<io::Result<()>> {
        <&TcpStream>::poll_close(&mut &*self, waker)
    }
}

// ===== impl Read / Write for &'a =====

impl<'a> AsyncRead for &'a TcpStream {
    fn poll_read(&mut self, waker: &Waker, buf: &mut [u8]) -> Poll<io::Result<usize>> {
        (&self.io).poll_read(waker, buf)
    }

    fn poll_vectored_read(
        &mut self,
        waker: &Waker,
        bufs: &mut [&mut IoVec],
    ) -> Poll<io::Result<usize>> {
        ready!(self.poll_read_ready(waker)?);

        let r = self.io.get_ref().read_bufs(bufs);

        if is_wouldblock(&r) {
            self.io.clear_read_ready(waker)?;
            Poll::Pending
        } else {
            Poll::Ready(r)
        }
    }
}

impl<'a> AsyncWrite for &'a TcpStream {
    fn poll_write(&mut self, waker: &Waker, buf: &[u8]) -> Poll<io::Result<usize>> {
        (&self.io).poll_write(waker, buf)
    }

    fn poll_vectored_write(&mut self, waker: &Waker, bufs: &[&IoVec]) -> Poll<io::Result<usize>> {
        ready!(self.poll_write_ready(waker)?);

        let r = self.io.get_ref().write_bufs(bufs);

        if is_wouldblock(&r) {
            self.io.clear_write_ready(waker)?;
        }

        return Poll::Ready(r);
    }

    fn poll_flush(&mut self, waker: &Waker) -> Poll<io::Result<()>> {
        (&self.io).poll_flush(waker)
    }

    fn poll_close(&mut self, waker: &Waker) -> Poll<io::Result<()>> {
        (&self.io).poll_close(waker)
    }
}

impl AsyncReadReady for TcpStream {
    type Ok = mio::Ready;
    type Err = io::Error;

    /// Poll the TCP stream's readiness for reading.
    ///
    /// If the stream is not ready for a read then the method will return `Poll::Pending`
    /// and schedule the current task for wakeup upon read-readiness.
    ///
    /// Once the stream is ready for reading, it will remain so until all available
    /// bytes have been extracted (via `futures::io::AsyncRead` and related traits).
    fn poll_read_ready(&self, waker: &Waker) -> Poll<Result<Self::Ok, Self::Err>> {
        self.io.poll_read_ready(waker)
    }
}

impl AsyncWriteReady for TcpStream {
    type Ok = mio::Ready;
    type Err = io::Error;

    /// Check the TCP stream's write readiness state.
    ///
    /// This always checks for writable readiness and also checks for HUP
    /// readiness on platforms that support it.
    ///
    /// If the resource is not ready for a write then `Poll::Pending` is
    /// returned and the current task is notified once a new event is received.
    ///
    /// The I/O resource will remain in a write-ready state until calls to
    /// `poll_write` return `NotReady`.
    ///
    /// # Panics
    ///
    /// This function panics if called from outside of a task context.
    fn poll_write_ready(&self, waker: &Waker) -> Poll<Result<Self::Ok, Self::Err>> {
        self.io.poll_write_ready(waker)
    }
}

impl fmt::Debug for TcpStream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.io.get_ref().fmt(f)
    }
}

impl Future for ConnectFuture {
    type Output = io::Result<TcpStream>;

    fn poll(mut self: Pin<&mut Self>, waker: &Waker) -> Poll<io::Result<TcpStream>> {
        match mem::replace(&mut self.inner, ConnectFutureState::Empty) {
            ConnectFutureState::Waiting(stream) => {
                // Once we've connected, wait for the stream to be writable as
                // that's when the actual connection has been initiated. Once we're
                // writable we check for `take_socket_error` to see if the connect
                // actually hit an error or not.
                //
                // If all that succeeded then we ship everything on up.
                if let Poll::Pending = stream.io.poll_write_ready(waker)? {
                    self.inner = ConnectFutureState::Waiting(stream);
                    return Poll::Pending;
                }

                if let Some(e) = stream.io.get_ref().take_error()? {
                    return Poll::Ready(Err(e));
                }

                Poll::Ready(Ok(stream))
            }
            ConnectFutureState::Error(e) => Poll::Ready(Err(e)),
            ConnectFutureState::Empty => panic!("can't poll TCP stream twice"),
        }
    }
}

impl std::convert::TryFrom<std::net::TcpStream> for TcpStream {
    type Error = io::Error;

    fn try_from(stream: std::net::TcpStream) -> Result<Self, Self::Error> {
        let tcp = mio::net::TcpStream::from_stream(stream)?;
        Ok(TcpStream::new(tcp))
    }
}

impl std::convert::TryFrom<&std::net::SocketAddr> for TcpStream {
    type Error = io::Error;

    fn try_from(addr: &std::net::SocketAddr) -> Result<Self, Self::Error> {
        let tcp = mio::net::TcpStream::connect(&addr)?;
        Ok(TcpStream::new(tcp))
    }
}

#[cfg(unix)]
mod sys {
    use super::TcpStream;
    use std::os::unix::prelude::*;

    impl AsRawFd for TcpStream {
        fn as_raw_fd(&self) -> RawFd {
            self.io.get_ref().as_raw_fd()
        }
    }
}

fn is_wouldblock<T>(r: &io::Result<T>) -> bool {
    match *r {
        Ok(_) => false,
        Err(ref e) => e.kind() == io::ErrorKind::WouldBlock,
    }
}
