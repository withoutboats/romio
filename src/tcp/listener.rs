use super::TcpStream;

use std::fmt;
use std::io;
use std::net::{self, SocketAddr};
use std::pin::Pin;

use futures::stream::Stream;
use futures::task::LocalWaker;
use futures::{ready, Poll};
use mio;

use crate::reactor::PollEvented;

/// An I/O object representing a TCP socket listening for incoming connections.
///
/// This object can be converted into a stream of incoming connections for
/// various forms of processing.
pub struct TcpListener {
    io: PollEvented<mio::net::TcpListener>,
}

impl TcpListener {
    /// Create a new TCP listener associated with this event loop.
    ///
    /// The TCP listener will bind to the provided `addr` address, if available.
    /// If the result is `Ok`, the socket has successfully bound.
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
    /// If aiming for production, decision what to do about them must be made. The
    /// [`tk-listen`](https://crates.io/crates/tk-listen) crate might be of some help.
    pub fn incoming(self) -> Incoming {
        Incoming::new(self)
    }

    /// Gets the value of the `IP_TTL` option for this socket.
    ///
    /// For more information about this option, see [`set_ttl`].
    ///
    /// [`set_ttl`]: #method.set_ttl
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

    fn poll_accept(&mut self, lw: &LocalWaker) -> Poll<io::Result<(TcpStream, SocketAddr)>> {
        let (io, addr) = ready!(self.poll_accept_std(lw)?);

        let io = mio::net::TcpStream::from_stream(io)?;
        let io = TcpStream::new(io);

        Poll::Ready(Ok((io, addr)))
    }

    fn poll_accept_std(
        &mut self,
        lw: &LocalWaker,
    ) -> Poll<io::Result<(net::TcpStream, SocketAddr)>> {
        ready!(self.io.poll_read_ready(lw)?);

        match self.io.get_ref().accept_std() {
            Ok(pair) => Poll::Ready(Ok(pair)),
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                self.io.clear_read_ready(lw)?;
                Poll::Pending
            }
            Err(e) => Poll::Ready(Err(e)),
        }
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

#[cfg(windows)]
mod sys {
    // TODO: let's land these upstream with mio and then we can add them here.
    //
    // use std::os::windows::prelude::*;
    // use super::{TcpListener;
    //
    // impl AsRawHandle for TcpListener {
    //     fn as_raw_handle(&self) -> RawHandle {
    //         self.listener.io().as_raw_handle()
    //     }
    // }
}

/// Stream returned by the `TcpListener::incoming` function representing the
/// stream of sockets received from a listener.
#[must_use = "streams do nothing unless polled"]
#[derive(Debug)]
pub struct Incoming {
    inner: TcpListener,
}

impl Incoming {
    pub(crate) fn new(listener: TcpListener) -> Incoming {
        Incoming { inner: listener }
    }
}

impl Stream for Incoming {
    type Item = io::Result<TcpStream>;

    fn poll_next(mut self: Pin<&mut Self>, lw: &LocalWaker) -> Poll<Option<Self::Item>> {
        let (socket, _) = ready!(self.inner.poll_accept(lw)?);
        Poll::Ready(Some(Ok(socket)))
    }
}
