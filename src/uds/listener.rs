use super::UnixStream;

use crate::reactor::PollEvented;

use futures::task::LocalWaker;
use futures::{ready, Poll, Stream};
use mio_uds;

use std::fmt;
use std::io;
use std::os::unix::io::{AsRawFd, RawFd};
use std::os::unix::net::{self, SocketAddr};
use std::path::Path;
use std::pin::Pin;

/// A Unix socket which can accept connections from other Unix sockets.
pub struct UnixListener {
    io: PollEvented<mio_uds::UnixListener>,
}

impl UnixListener {
    /// Creates a new `UnixListener` bound to the specified path.
    pub fn bind<P>(path: P) -> io::Result<UnixListener>
    where
        P: AsRef<Path>,
    {
        let listener = mio_uds::UnixListener::bind(path)?;
        let io = PollEvented::new(listener);
        Ok(UnixListener { io })
    }

    /// Returns the local socket address of this listener.
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.io.get_ref().local_addr()
    }

    /// Returns the value of the `SO_ERROR` option.
    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        self.io.get_ref().take_error()
    }

    /// Consumes this listener, returning a stream of the sockets this listener
    /// accepts.
    ///
    /// This method returns an implementation of the `Stream` trait which
    /// resolves to the sockets the are accepted on this listener.
    pub fn incoming(self) -> Incoming {
        Incoming::new(self)
    }

    fn poll_accept(&self, lw: &LocalWaker) -> Poll<io::Result<(UnixStream, SocketAddr)>> {
        let (io, addr) = ready!(self.poll_accept_std(lw)?);

        let io = mio_uds::UnixStream::from_stream(io)?;
        Poll::Ready(Ok((UnixStream::new(io), addr)))
    }

    fn poll_accept_std(&self, lw: &LocalWaker) -> Poll<io::Result<(net::UnixStream, SocketAddr)>> {
        ready!(self.io.poll_read_ready(lw)?);

        match self.io.get_ref().accept_std() {
            Ok(Some((sock, addr))) => Poll::Ready(Ok((sock, addr))),
            Ok(None) => {
                self.io.clear_read_ready(lw)?;
                Poll::Pending
            }
            Err(ref err) if err.kind() == io::ErrorKind::WouldBlock => {
                self.io.clear_read_ready(lw)?;
                Poll::Pending
            }
            Err(err) => Poll::Ready(Err(err)),
        }
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

    fn poll_next(self: Pin<&mut Self>, lw: &LocalWaker) -> Poll<Option<Self::Item>> {
        let (socket, _) = ready!(self.inner.poll_accept(lw)?);
        Poll::Ready(Some(Ok(socket)))
    }
}
