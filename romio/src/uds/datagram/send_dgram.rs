use super::UnixDatagram;

use futures::{Future, Poll, ready};
use futures::task::LocalWaker;

use std::io;
use std::marker::Unpin;
use std::mem;
use std::path::Path;
use std::pin::Pin;

/// A future for writing a buffer to a Unix datagram socket.
#[derive(Debug)]
pub struct SendDgram<T, P> {
    st: State<T, P>,
}

#[derive(Debug)]
enum State<T, P> {
    /// current state is Sending
    Sending {
        /// the underlying socket
        sock: UnixDatagram,
        /// the buffer to send
        buf: T,
        /// the destination
        addr: P,
    },
    /// neutral state
    Empty,
}

impl<T, P> SendDgram<T, P>
where
    T: AsRef<[u8]>,
    P: AsRef<Path>,
{
    pub(crate) fn new(sock: UnixDatagram, buf: T, addr: P) -> SendDgram<T, P> {
        SendDgram {
            st: State::Sending {
                sock,
                buf,
                addr,
            }
        }
    }
}

impl<T, P> Future for SendDgram<T, P>
where
    T: AsRef<[u8]>,
    P: AsRef<Path>,
{
    /// Returns the underlying socket and the buffer that was sent.
    type Output = io::Result<(UnixDatagram, T)>;

    fn poll(mut self: Pin<&mut Self>, lw: &LocalWaker) -> Poll<Self::Output> {
        if let State::Sending {
            ref mut sock,
            ref buf,
            ref addr,
        } = self.st
        {
            let n = ready!(sock.poll_send_to(lw, buf.as_ref(), addr)?);
            if n < buf.as_ref().len() {
                return Poll::Ready(Err(io::Error::new(
                    io::ErrorKind::Other,
                    "Couldn't send whole buffer".to_string(),
                )));
            }
        } else {
            panic!()
        }
        if let State::Sending { sock, buf, addr: _ } =
            mem::replace(&mut self.st, State::Empty)
        {
            Poll::Ready(Ok((sock, buf)))
        } else {
            panic!()
        }
    }
}

// The existence of this impl means that we must never project from a pinned reference to
// `RecvDiagram` to a pinned reference of its `buf` or `addr` fields. Fortunately, we will
// never need to.
impl<T, P> Unpin for SendDgram<T, P> { }
