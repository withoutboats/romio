use super::TcpStream;

use std::fmt;
use std::io;
use std::net::{self, SocketAddr};
use std::pin::Pin;

use async_ready::AsyncReady;
use futures::stream::Stream;
use futures::task::Waker;
use futures::{ready, Poll};
use mio;

use crate::raw::PollEvented;

/// A TCP socket server, listening for connections.
///
/// After creating a `TcpListener` by [`bind`]ing it to a socket address, it listens
/// for incoming TCP connections. These can be accepted by awaiting elements from the
/// async stream of incoming connections, [`incoming`][`TcpListener::incoming`].
///
/// The socket will be closed when the value is dropped.
///
/// [`bind`]: #method.bind
/// [`TcpListener::incoming`]: #method.incoming
///
/// # Examples
///
/// ```rust,no_run
/// #![feature(async_await, await_macro, futures_api)]
/// use std::error::Error;
///
/// use romio::tcp::{TcpListener, TcpStream};
/// use futures::prelude::*;
///
/// async fn recite_shakespeare(mut stream: TcpStream) {
///     await!(stream.write_all(b"Shall I hear more, or shall I speak at this?"));
/// }
///
/// async fn listen() -> Result<(), Box<dyn Error + 'static>> {
///     let socket_addr = "127.0.0.1:80".parse()?;
///     let mut listener = TcpListener::bind(&socket_addr)?;
///     let mut incoming = listener.incoming();
///
///     // accept connections and process them serially
///     while let Some(stream) = await!(incoming.next()) {
///         await!(recite_shakespeare(stream?));
///     }
///     Ok(())
/// }
/// ```
pub struct TcpListener {
    io: PollEvented<mio::net::TcpListener>,
}

impl TcpListener {
    /// Creates a new `TcpListener` which will be bound to the specified
    /// address.
    ///
    /// The returned listener is ready for accepting connections.
    ///
    /// Binding with a port number of 0 will request that the OS assigns a port
    /// to this listener. The port allocated can be queried via the
    /// [`local_addr`] method.
    ///
    /// # Examples
    /// Create a TCP listener bound to 127.0.0.1:80:
    ///
    /// ```rust,no_run
    /// use romio::tcp::TcpListener;
    ///
    /// # fn main () -> Result<(), Box<dyn std::error::Error + 'static>> {
    /// let socket_addr = "127.0.0.1:80".parse()?;
    /// let listener = TcpListener::bind(&socket_addr)?;
    /// # Ok(())}
    /// ```
    ///
    /// [`local_addr`]: #method.local_addr
    pub fn bind(addr: &SocketAddr) -> io::Result<TcpListener> {
        let l = mio::net::TcpListener::bind(addr)?;
        Ok(TcpListener::new(l))
    }

    fn new(listener: mio::net::TcpListener) -> TcpListener {
        let io = PollEvented::new(listener);
        TcpListener { io }
    }

    /// Returns the local address that this listener is bound to.
    ///
    /// This can be useful, for example, when binding to port 0 to figure out
    /// which port was actually bound.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use romio::tcp::TcpListener;
    /// use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
    ///
    /// # fn main () -> Result<(), Box<dyn std::error::Error + 'static>> {
    /// let socket_addr = "127.0.0.1:8080".parse()?;
    /// let listener = TcpListener::bind(&socket_addr)?;
    ///
    /// let expected = SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 8080);
    /// assert_eq!(listener.local_addr()?, SocketAddr::V4(expected));
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
    /// # Errors
    ///
    /// Note that accepting a connection can lead to various errors and not all of them are
    /// necessarily fatal ‒ for example having too many open file descriptors or the other side
    /// closing the connection while it waits in an accept queue. These would terminate the stream
    /// if not handled in any way.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// #![feature(async_await, await_macro, futures_api)]
    /// use futures::prelude::*;
    /// use romio::tcp::TcpListener;
    ///
    /// # async fn work () -> Result<(), Box<dyn std::error::Error + 'static>> {
    /// let socket_addr = "127.0.0.1:80".parse()?;
    /// let mut listener = TcpListener::bind(&socket_addr)?;
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
    pub fn incoming(&mut self) -> Incoming<'_> {
        Incoming { inner: self }
    }

    /// Gets the value of the `IP_TTL` option for this socket.
    ///
    /// For more information about this option, see [`set_ttl`].
    ///
    /// [`set_ttl`]: #method.set_ttl
    ///
    /// # Examples
    ///
    /// ```rust
    /// #![feature(async_await, await_macro, futures_api)]
    /// use futures::prelude::*;
    /// use romio::tcp::TcpListener;
    ///
    /// # fn main () -> Result<(), Box<dyn std::error::Error + 'static>> {
    /// let socket_addr = "127.0.0.1:0".parse()?;
    /// let listener = TcpListener::bind(&socket_addr)?;
    /// listener.set_ttl(100)?;
    /// assert_eq!(listener.ttl()?, 100);
    /// # Ok(()) }
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
    /// ```rust,no_run
    /// #![feature(async_await, await_macro, futures_api)]
    /// use futures::prelude::*;
    /// use romio::tcp::TcpListener;
    ///
    /// # fn main () -> Result<(), Box<dyn std::error::Error + 'static>> {
    /// let socket_addr = "127.0.0.1:0".parse()?;
    /// let listener = TcpListener::bind(&socket_addr)?;
    /// listener.set_ttl(100)?;
    /// # Ok(()) }
    /// ```
    pub fn set_ttl(&self, ttl: u32) -> io::Result<()> {
        self.io.get_ref().set_ttl(ttl)
    }

    fn poll_accept_std(&self, waker: &Waker) -> Poll<io::Result<(net::TcpStream, SocketAddr)>> {
        ready!(self.io.poll_read_ready(waker)?);

        match self.io.get_ref().accept_std() {
            Ok(pair) => Poll::Ready(Ok(pair)),
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                self.io.clear_read_ready(waker)?;
                Poll::Pending
            }
            Err(e) => Poll::Ready(Err(e)),
        }
    }
}

impl AsyncReady for TcpListener {
    type Ok = (TcpStream, SocketAddr);
    type Err = std::io::Error;

    /// Check if the stream can be read from.
    fn poll_ready(&self, waker: &Waker) -> Poll<Result<Self::Ok, Self::Err>> {
        let (io, addr) = ready!(self.poll_accept_std(waker)?);
        let io = mio::net::TcpStream::from_stream(io)?;
        let io = TcpStream::new(io);
        Poll::Ready(Ok((io, addr)))
    }
}

impl fmt::Debug for TcpListener {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.io.get_ref().fmt(f)
    }
}

#[cfg(unix)]
mod sys {
    use super::TcpListener;
    use std::os::unix::prelude::*;

    impl AsRawFd for TcpListener {
        fn as_raw_fd(&self) -> RawFd {
            self.io.get_ref().as_raw_fd()
        }
    }
}

/// Stream returned by the `TcpListener::incoming` function representing the
/// stream of sockets received from a listener.
#[must_use = "streams do nothing unless polled"]
#[derive(Debug)]
pub struct Incoming<'a> {
    inner: &'a mut TcpListener,
}

impl<'a> Stream for Incoming<'a> {
    type Item = io::Result<TcpStream>;

    fn poll_next(self: Pin<&mut Self>, waker: &Waker) -> Poll<Option<Self::Item>> {
        let (socket, _) = ready!(self.inner.poll_ready(waker)?);
        Poll::Ready(Some(Ok(socket)))
    }
}
