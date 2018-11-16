use super::UnixDatagram;

use futures::{Future, Poll, ready};
use futures::task::LocalWaker;

use std::io;
use std::marker::Unpin;
use std::mem;
use std::pin::Pin;

/// A future for receiving datagrams from a Unix datagram socket.
///
/// An example that uses UDP sockets but is still applicable can be found at
/// https://gist.github.com/dermesser/e331094c2ab28fc7f6ba8a16183fe4d5.
#[derive(Debug)]
pub struct RecvDgram<T> {
    st: State<T>,
}

/// A future similar to RecvDgram, but without allocating and returning the peer's address.
///
/// This can be used if the peer's address is of no interest, so the allocation overhead can be
/// avoided.
#[derive(Debug)]
enum State<T> {
    Receiving {
        sock: UnixDatagram,
        buf: T,
    },
    Empty,
}

impl<T> RecvDgram<T>
where
    T: AsMut<[u8]>
{
    pub(crate) fn new(sock: UnixDatagram, buf: T) -> RecvDgram<T> {
        RecvDgram {
            st: State::Receiving {
                sock,
                buf,
            },
        }
    }
}

impl<T> Future for RecvDgram<T>
where
    T: AsMut<[u8]>,
{
    /// RecvDgram yields a tuple of the underlying socket, the receive buffer, how many bytes were
    /// received, and the address (path) of the peer sending the datagram. If the buffer is too small, the
    /// datagram is truncated.
    type Output = io::Result<(UnixDatagram, T, usize, String)>;

    fn poll(mut self: Pin<&mut Self>, lw: &LocalWaker) -> Poll<Self::Output> {
        let received;
        let peer;

        if let State::Receiving {
            ref mut sock,
            ref mut buf,
        } = self.st
        {
            let (n, p) = ready!(sock.poll_recv_from(lw, buf.as_mut())?);
            received = n;

            peer = p.as_pathname().map_or(String::new(), |p| {
                p.to_str().map_or(String::new(), |s| s.to_string())
            });
        } else {
            panic!()
        }

        if let State::Receiving { sock, buf } =
            mem::replace(&mut self.st, State::Empty)
        {
            Poll::Ready(Ok((sock, buf, received, peer)))
        } else {
            panic!()
        }
    }
}

// The existence of this impl means that we must never project from a pinned reference to
// `RecvDiagram` to a pinned reference of its `buf` field. Fortunately, we will never need to.
impl<T> Unpin for RecvDgram<T> { }
