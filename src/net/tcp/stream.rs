use std::fmt;
use std::io;
use std::mem;
use std::net::{SocketAddr, Shutdown};
use std::pin::Pin;
use std::time::Duration;

use futures::{ready, Future, Poll};
use futures::io::{AsyncRead, AsyncWrite};
use futures::task::LocalWaker;
use iovec::IoVec;
use mio;

use crate::reactor::PollEvented;

/// An I/O object representing a TCP stream connected to a remote endpoint.
///
/// A TCP stream can either be created by connecting to an endpoint, via the
/// [`connect`] method, or by [accepting] a connection from a [listener].
///
/// [`connect`]: struct.TcpStream.html#method.connect
/// [accepting]: struct.TcpListener.html#method.accept
/// [listener]: struct.TcpListener.html
pub struct TcpStream {
    io: PollEvented<mio::net::TcpStream>,
}

/// Future returned by `TcpStream::connect` which will resolve to a `TcpStream`
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

    /// Check the TCP stream's read readiness state.
    ///
    /// The mask argument allows specifying what readiness to notify on. This
    /// can be any value, including platform specific readiness, **except**
    /// `writable`. HUP is always implicitly included on platforms that support
    /// it.
    ///
    /// If the resource is not ready for a read then `Poll::Pending` is
    /// returned and the current task is notified once a new event is received.
    ///
    /// The stream will remain in a read-ready state until calls to `poll_read`
    /// return `NotReady`.
    ///
    /// # Panics
    ///
    /// This function panics if:
    ///
    /// * `ready` includes writable.
    /// * called from outside of a task context.
    pub fn poll_read_ready(&self, lw: &LocalWaker)
        -> Poll<io::Result<mio::Ready>>
    {
        self.io.poll_read_ready(lw)
    }

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
    pub fn poll_write_ready(&self, lw: &LocalWaker) -> Poll<io::Result<mio::Ready>> {
        self.io.poll_write_ready(lw)
    }

    /// Returns the local address that this stream is bound to.
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.io.get_ref().local_addr()
    }

    /// Returns the remote address that this stream is connected to.
    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        self.io.get_ref().peer_addr()
    }

    /// Receives data on the socket from the remote address to which it is
    /// connected, without removing that data from the queue. On success,
    /// returns the number of bytes peeked.
    ///
    /// Successive calls return the same data. This is accomplished by passing
    /// `MSG_PEEK` as a flag to the underlying recv system call.
    ///
    /// # Return
    ///
    /// On success, returns `Ok(Async::Ready(num_bytes_read))`.
    ///
    /// If no data is available for reading, the method returns
    /// `Ok(Poll::Pending)` and arranges for the current task to receive a
    /// notification when the socket becomes readable or is closed.
    ///
    /// # Panics
    ///
    /// This function will panic if called from outside of a task context.
    pub fn poll_peek(&mut self, buf: &mut [u8], lw: &LocalWaker) -> Poll<io::Result<usize>> {
        ready!(self.io.poll_read_ready(lw)?);

        match self.io.get_ref().peek(buf) {
            Ok(ret) => Poll::Ready(Ok(ret.into())),
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                self.io.clear_read_ready(lw)?;
                Poll::Pending
            }
            Err(e) => Poll::Ready(Err(e)),
        }
    }

    /// Shuts down the read, write, or both halves of this connection.
    ///
    /// This function will cause all pending and future I/O on the specified
    /// portions to return immediately with an appropriate value (see the
    /// documentation of `Shutdown`).
    pub fn shutdown(&self, how: Shutdown) -> io::Result<()> {
        self.io.get_ref().shutdown(how)
    }

    /// Gets the value of the `TCP_NODELAY` option on this socket.
    ///
    /// For more information about this option, see [`set_nodelay`].
    ///
    /// [`set_nodelay`]: #method.set_nodelay
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
    pub fn set_nodelay(&self, nodelay: bool) -> io::Result<()> {
        self.io.get_ref().set_nodelay(nodelay)
    }

    /// Gets the value of the `SO_RCVBUF` option on this socket.
    ///
    /// For more information about this option, see [`set_recv_buffer_size`].
    ///
    /// [`set_recv_buffer_size`]: #tymethod.set_recv_buffer_size
    pub fn recv_buffer_size(&self) -> io::Result<usize> {
        self.io.get_ref().recv_buffer_size()
    }

    /// Sets the value of the `SO_RCVBUF` option on this socket.
    ///
    /// Changes the size of the operating system's receive buffer associated
    /// with the socket.
    pub fn set_recv_buffer_size(&self, size: usize) -> io::Result<()> {
        self.io.get_ref().set_recv_buffer_size(size)
    }

    /// Gets the value of the `SO_SNDBUF` option on this socket.
    ///
    /// For more information about this option, see [`set_send_buffer`].
    ///
    /// [`set_send_buffer`]: #tymethod.set_send_buffer
    pub fn send_buffer_size(&self) -> io::Result<usize> {
        self.io.get_ref().send_buffer_size()
    }

    /// Sets the value of the `SO_SNDBUF` option on this socket.
    ///
    /// Changes the size of the operating system's send buffer associated with
    /// the socket.
    pub fn set_send_buffer_size(&self, size: usize) -> io::Result<()> {
        self.io.get_ref().set_send_buffer_size(size)
    }

    /// Returns whether keepalive messages are enabled on this socket, and if so
    /// the duration of time between them.
    ///
    /// For more information about this option, see [`set_keepalive`].
    ///
    /// [`set_keepalive`]: #tymethod.set_keepalive
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
    pub fn set_keepalive(&self, keepalive: Option<Duration>) -> io::Result<()> {
        self.io.get_ref().set_keepalive(keepalive)
    }

    /// Gets the value of the `IP_TTL` option for this socket.
    ///
    /// For more information about this option, see [`set_ttl`].
    ///
    /// [`set_ttl`]: #tymethod.set_ttl
    pub fn ttl(&self) -> io::Result<u32> {
        self.io.get_ref().ttl()
    }

    /// Sets the value for the `IP_TTL` option on this socket.
    ///
    /// This value sets the time-to-live field that is used in every packet sent
    /// from this socket.
    pub fn set_ttl(&self, ttl: u32) -> io::Result<()> {
        self.io.get_ref().set_ttl(ttl)
    }

    /// Reads the linger duration for this socket by getting the `SO_LINGER`
    /// option.
    ///
    /// For more information about this option, see [`set_linger`].
    ///
    /// [`set_linger`]: #tymethod.set_linger
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
    pub fn set_linger(&self, dur: Option<Duration>) -> io::Result<()> {
        self.io.get_ref().set_linger(dur)
    }
}

// ===== impl Read / Write =====

impl AsyncRead for TcpStream {
    fn poll_read(&mut self, lw: &LocalWaker, buf: &mut [u8]) -> Poll<io::Result<usize>> {
        <&TcpStream>::poll_read(&mut &*self, lw, buf)
    }

    fn poll_vectored_read(&mut self, lw: &LocalWaker, vec: &mut [&mut IoVec])
        -> Poll<io::Result<usize>>
    {
        <&TcpStream>::poll_vectored_read(&mut &*self, lw, vec)
    }
}

impl AsyncWrite for TcpStream {
    fn poll_write(&mut self, lw: &LocalWaker, buf: &[u8]) -> Poll<io::Result<usize>> {
        <&TcpStream>::poll_write(&mut &*self, lw, buf)
    }

    fn poll_vectored_write(&mut self, lw: &LocalWaker, vec: &[&IoVec])
        -> Poll<io::Result<usize>>
    {
        <&TcpStream>::poll_vectored_write(&mut &*self, lw, vec)
    }

    fn poll_flush(&mut self, lw: &LocalWaker) -> Poll<io::Result<()>> {
        <&TcpStream>::poll_flush(&mut &*self, lw)
    }

    fn poll_close(&mut self, lw: &LocalWaker) -> Poll<io::Result<()>> {
        <&TcpStream>::poll_close(&mut &*self, lw)
    }
}

// ===== impl Read / Write for &'a =====

impl<'a> AsyncRead for &'a TcpStream {
    fn poll_read(&mut self, lw: &LocalWaker, buf: &mut [u8]) -> Poll<io::Result<usize>> {
        (&self.io).poll_read(lw, buf)
    }

    fn poll_vectored_read(&mut self, lw: &LocalWaker, bufs: &mut [&mut IoVec])
        -> Poll<io::Result<usize>>
    {
        ready!(self.poll_read_ready(lw)?);

        let r = self.io.get_ref().read_bufs(bufs);

        if is_wouldblock(&r) {
            self.io.clear_read_ready(lw)?;
            Poll::Pending
        } else {
            Poll::Ready(r)
        }
    }
}

impl<'a> AsyncWrite for &'a TcpStream {
    fn poll_write(&mut self, lw: &LocalWaker, buf: &[u8]) -> Poll<io::Result<usize>> {
        (&self.io).poll_write(lw, buf)
    }

    fn poll_vectored_write(&mut self, lw: &LocalWaker, bufs: &[&IoVec])
        -> Poll<io::Result<usize>>
    {
        ready!(self.poll_write_ready(lw)?);

        let r = self.io.get_ref().write_bufs(bufs);

        if is_wouldblock(&r) {
            self.io.clear_write_ready(lw)?;
        }

        return Poll::Ready(r)
    }

    fn poll_flush(&mut self, lw: &LocalWaker) -> Poll<io::Result<()>> {
        (&self.io).poll_flush(lw)
    }

    fn poll_close(&mut self, lw: &LocalWaker) -> Poll<io::Result<()>> {
        (&self.io).poll_close(lw)
    }
}

impl fmt::Debug for TcpStream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.io.get_ref().fmt(f)
    }
}

impl Future for ConnectFuture {
    type Output = io::Result<TcpStream>;

    fn poll(mut self: Pin<&mut Self>, lw: &LocalWaker) -> Poll<io::Result<TcpStream>> {
        Pin::new(&mut self.inner).poll(lw)
    }
}

impl ConnectFutureState {
    fn poll_inner<F>(&mut self, f: F) -> Poll<io::Result<TcpStream>>
        where F: FnOnce(&mut PollEvented<mio::net::TcpStream>) -> Poll<io::Result<mio::Ready>>
    {
        {
            let stream = match *self {
                ConnectFutureState::Waiting(ref mut s) => s,
                ConnectFutureState::Error(_) => {
                    let e = match mem::replace(self, ConnectFutureState::Empty) {
                        ConnectFutureState::Error(e) => e,
                        _ => panic!(),
                    };
                    return Poll::Ready(Err(e))
                }
                ConnectFutureState::Empty => panic!("can't poll TCP stream twice"),
            };

            // Once we've connected, wait for the stream to be writable as
            // that's when the actual connection has been initiated. Once we're
            // writable we check for `take_socket_error` to see if the connect
            // actually hit an error or not.
            //
            // If all that succeeded then we ship everything on up.
            if let Poll::Pending = f(&mut stream.io)? {
                return Poll::Pending
            }

            if let Some(e) = stream.io.get_ref().take_error()? {
                return Poll::Ready(Err(e))
            }
        }

        match mem::replace(self, ConnectFutureState::Empty) {
            ConnectFutureState::Waiting(stream) => Poll::Ready(Ok(stream)),
            _ => panic!(),
        }
    }
}

impl Future for ConnectFutureState {
    type Output = io::Result<TcpStream>;

    fn poll(mut self: Pin<&mut Self>, lw: &LocalWaker) -> Poll<io::Result<TcpStream>> {
        self.poll_inner(|io| io.poll_write_ready(lw))
    }
}

#[cfg(unix)]
mod sys {
    use std::os::unix::prelude::*;
    use super::TcpStream;

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
