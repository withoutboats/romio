use crate::raw::PollEvented;

use async_datagram::AsyncDatagram;
use async_ready::{AsyncReadReady, AsyncWriteReady, TakeError};
use futures::{ready, Poll};
use mio_uds;

use std::fmt;
use std::io;
use std::net::Shutdown;
use std::os::unix::io::{AsRawFd, RawFd};
use std::os::unix::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::task::Context;
use std::future::Future;

/// An I/O object representing a Unix datagram socket.
pub struct UnixDatagram {
    io: PollEvented<mio_uds::UnixDatagram>,
}

/// The future returned by `UnixDatagram::send_to`.
#[derive(Debug)]
pub struct SendTo<'a, 'b> {
    socket: &'a mut UnixDatagram,
    buf: &'b [u8],
    target: &'b PathBuf,
}

impl<'a, 'b> Future for SendTo<'a, 'b> {
    type Output = io::Result<usize>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let SendTo {
            socket,
            buf,
            target,
        } = &mut *self;
        Pin::new(&mut **socket).poll_send_to(cx, buf, target)
    }
}

/// The future returned by `UnixDatagram::recv_from`.
#[derive(Debug)]
pub struct RecvFrom<'a, 'b> {
    socket: &'a mut UnixDatagram,
    buf: &'b mut [u8],
}

impl<'a, 'b> Future for RecvFrom<'a, 'b> {
    type Output = io::Result<(usize, SocketAddr)>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let RecvFrom { socket, buf } = &mut *self;
        Pin::new(&mut **socket).poll_recv_from(cx, buf)
    }
}

impl UnixDatagram {
    /// Creates a new `UnixDatagram` bound to the specified path.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use romio::uds::UnixDatagram;
    ///
    /// # fn run() -> std::io::Result<()> {
    /// let sock = UnixDatagram::bind("/tmp/sock")?;
    /// # Ok(()) }
    /// ```
    pub fn bind(path: impl AsRef<Path>) -> io::Result<UnixDatagram> {
        let socket = mio_uds::UnixDatagram::bind(path)?;
        Ok(UnixDatagram::new(socket))
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
    /// use romio::uds::UnixDatagram;
    ///
    /// # fn run() -> std::io::Result<()> {
    /// let (sock1, sock2) = UnixDatagram::pair()?;
    /// # Ok(()) }
    /// ```
    pub fn pair() -> io::Result<(UnixDatagram, UnixDatagram)> {
        let (a, b) = mio_uds::UnixDatagram::pair()?;
        let a = UnixDatagram::new(a);
        let b = UnixDatagram::new(b);

        Ok((a, b))
    }

    fn new(socket: mio_uds::UnixDatagram) -> UnixDatagram {
        let io = PollEvented::new(socket);
        UnixDatagram { io }
    }

    /// Creates a new `UnixDatagram` which is not bound to any address.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use romio::uds::UnixDatagram;
    ///
    /// # fn run() -> std::io::Result<()> {
    /// let sock = UnixDatagram::unbound()?;
    /// # Ok(()) }
    /// ```
    pub fn unbound() -> io::Result<UnixDatagram> {
        let socket = mio_uds::UnixDatagram::unbound()?;
        Ok(UnixDatagram::new(socket))
    }

    /// Returns the local address that this socket is bound to.
    /// # Examples
    ///
    /// ```rust,no_run
    /// use romio::uds::UnixDatagram;
    ///
    /// # fn run() -> std::io::Result<()> {
    /// let stream = UnixDatagram::bind("/tmp/sock")?;
    /// let addr = stream.local_addr()?;
    /// # Ok(()) }
    /// ```
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.io.get_ref().local_addr()
    }

    /// Returns the address of this socket's peer.
    ///
    /// The `connect` method will connect the socket to a peer.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use romio::uds::UnixDatagram;
    ///
    /// # fn run() -> std::io::Result<()> {
    /// let stream = UnixDatagram::bind("/tmp/sock")?;
    /// let addr = stream.peer_addr()?;
    /// # Ok(()) }
    /// ```
    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        self.io.get_ref().peer_addr()
    }

    /// Shut down the read, write, or both halves of this connection.
    ///
    /// This function will cause all pending and future I/O calls on the
    /// specified portions to immediately return with an appropriate value
    /// (see the documentation of `Shutdown`).
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use romio::uds::UnixDatagram;
    /// use std::net::Shutdown;
    ///
    /// # fn run () -> Result<(), Box<dyn std::error::Error + 'static>> {
    /// let stream = UnixDatagram::bind("/tmp/sock")?;
    /// stream.shutdown(Shutdown::Both)?;
    /// # Ok(())}
    /// ```
    pub fn shutdown(&self, how: Shutdown) -> io::Result<()> {
        self.io.get_ref().shutdown(how)
    }

    /// Sends data on the socket to the given address. On success, returns the
    /// number of bytes written.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// #![feature(async_await)]
    /// # use std::error::Error;
    /// use romio::udp::UdpSocket;
    ///
    /// const THE_MERCHANT_OF_VENICE: &[u8] = b"
    ///     If you prick us, do we not bleed?
    ///     If you tickle us, do we not laugh?
    ///     If you poison us, do we not die?
    ///     And if you wrong us, shall we not revenge?
    /// ";
    ///
    /// # async fn send_data() -> Result<(), Box<dyn Error + 'static>> {
    /// let addr = "/tmp/out.socket".parse()?;
    /// let target = "/tmp/in.socket".parse()?;
    /// let mut socket = UdpSocket::bind(&addr)?;
    ///
    /// socket.send_to(THE_MERCHANT_OF_VENICE, &target).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn send_to<'a, 'b>(&'a mut self, buf: &'b [u8], target: &'b PathBuf) -> SendTo<'a, 'b> {
        SendTo {
            buf,
            target,
            socket: self,
        }
    }

    /// Receives data from the socket. On success, returns the number of bytes
    /// read and the address from whence the data came.
    ///
    /// # Exampes
    ///
    /// ```no_run
    /// #![feature(async_await)]
    /// # use std::error::Error;
    /// use romio::udp::UdpSocket;
    ///
    /// # async fn recv_data() -> Result<Vec<u8>, Box<dyn Error + 'static>> {
    /// let addr = "/tmp/in.socket".parse()?;
    /// let mut socket = UdpSocket::bind(&addr)?;
    /// let mut buf = vec![0; 1024];
    ///
    /// socket.recv_from(&mut buf).await?;
    /// # Ok(buf)
    /// # }
    /// ```
    pub fn recv_from<'a, 'b>(&'a mut self, buf: &'b mut [u8]) -> RecvFrom<'a, 'b> {
        RecvFrom { buf, socket: self }
    }
}

impl AsyncDatagram for UnixDatagram {
    type Sender = SocketAddr;
    type Receiver = PathBuf;
    type Err = io::Error;

    fn poll_send_to(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
        receiver: &Self::Receiver,
    ) -> Poll<io::Result<usize>> {
        ready!(self.io.poll_write_ready(cx)?);

        match self.io.get_ref().send_to(buf, receiver) {
            Ok(n) => Poll::Ready(Ok(n)),
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                Pin::new(&mut self.io).clear_write_ready(cx)?;
                Poll::Pending
            }
            Err(e) => Poll::Ready(Err(e)),
        }
    }

    fn poll_recv_from(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<(usize, Self::Sender)>> {
        ready!(Pin::new(&mut self.io).poll_read_ready(cx)?);

        let r = self.io.get_ref().recv_from(buf);

        if is_wouldblock(&r) {
            Pin::new(&mut self.io).clear_read_ready(cx)?;
            Poll::Pending
        } else {
            Poll::Ready(r)
        }
    }
}

impl AsyncReadReady for UnixDatagram {
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

impl AsyncWriteReady for UnixDatagram {
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

impl TakeError for UnixDatagram {
    type Ok = io::Error;
    type Err = io::Error;

    /// Returns the value of the `SO_ERROR` option.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use romio::raw::TakeError;
    /// use romio::uds::UnixDatagram;
    ///
    /// # fn run() -> std::io::Result<()> {
    /// let stream = UnixDatagram::bind("/tmp/sock")?;
    /// if let Ok(Some(err)) = stream.take_error() {
    ///     println!("Got error: {:?}", err);
    /// }
    /// # Ok(()) }
    /// ```
    fn take_error(&self) -> Result<Option<Self::Ok>, Self::Err> {
        self.io.get_ref().take_error()
    }
}

impl fmt::Debug for UnixDatagram {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.io.get_ref().fmt(f)
    }
}

impl AsRawFd for UnixDatagram {
    fn as_raw_fd(&self) -> RawFd {
        self.io.get_ref().as_raw_fd()
    }
}

fn is_wouldblock<T>(r: &io::Result<T>) -> bool {
    match *r {
        Ok(_) => false,
        Err(ref e) => e.kind() == io::ErrorKind::WouldBlock,
    }
}
