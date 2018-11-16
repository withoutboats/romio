use super::socket::UdpSocket;

use std::io;
use std::marker::Unpin;
use std::net::SocketAddr;
use std::pin::Pin;

use futures::{Future, Poll, ready};
use futures::task::LocalWaker;

/// A future used to receive a datagram from a UDP socket.
///
/// This is created by the `UdpSocket::recv_dgram` method.
#[must_use = "futures do nothing unless polled"]
#[derive(Debug)]
pub struct RecvDgram<T> {
    /// None means future was completed
    state: Option<RecvDgramInner<T>>
}

/// A struct is used to represent the full info of RecvDgram.
#[derive(Debug)]
struct RecvDgramInner<T> {
    /// Rx socket
    socket: UdpSocket,
    /// The received data will be put in the buffer
    buffer: T
}

impl<T> RecvDgram<T> {
    /// Create a new future to receive UDP Datagram
    pub(crate) fn new(socket: UdpSocket, buffer: T) -> RecvDgram<T> {
        let inner = RecvDgramInner { socket: socket, buffer: buffer };
        RecvDgram { state: Some(inner) }
    }
}

impl<T> Future for RecvDgram<T>
    where T: AsMut<[u8]>,
{
    type Output = io::Result<(UdpSocket, T, usize, SocketAddr)>;

    fn poll(mut self: Pin<&mut Self>, lw: &LocalWaker) -> Poll<Self::Output> {
        let (n, addr) = {
            let ref mut inner =
                self.state.as_mut().expect("RecvDgram polled after completion");

            ready!(inner.socket.poll_recv_from(lw, inner.buffer.as_mut())?)
        };

        let inner = self.state.take().unwrap();
        Poll::Ready(Ok((inner.socket, inner.buffer, n, addr)))
    }
}

// The existence of this impl means that we must never project from a pinned reference to
// `RecvDiagram` to a pinned reference of its `buffer`. Fortunately, we will
// never need to.
impl<T> Unpin for RecvDgram<T> { }
