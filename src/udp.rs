//! Async UDP bindings.
//!
//! This module contains the UDP networking types, similar to those found in
//! `std::net`, but suitable for async programming via futures and
//! `async`/`await`.
//!
//! After creating a `UdpSocket` by [`bind`]ing it to a socket address, data can be
//! [sent to] and [received from] any other socket address.
//!
//! [`bind`]: #method.bind
//! [received from]: #method.poll_recv_from
//! [sent to]: #method.poll_send_to

use std::fmt;
use std::io;
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};
use std::pin::Pin;

use async_datagram::AsyncDatagram;
use async_ready::{AsyncReadReady, AsyncWriteReady};
use futures::task::Waker;
use futures::Future;
use futures::{ready, Poll};
use mio;

use crate::raw::PollEvented;

/// A UDP socket.
pub struct UdpSocket {
    io: PollEvented<mio::net::UdpSocket>,
}

impl UdpSocket {
    /// Creates a UDP socket from the given address.
    ///
    /// Binding with a port number of 0 will request that the OS assigns a port
    /// to this socket. The port allocated can be queried via the
    /// [`local_addr`] method.
    ///
    /// [`local_addr`]: #method.local_addr
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use romio::udp::UdpSocket;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let socket_addr = "127.0.0.1:0".parse()?;
    /// let socket = UdpSocket::bind(&socket_addr)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn bind(addr: &SocketAddr) -> io::Result<UdpSocket> {
        mio::net::UdpSocket::bind(addr).map(UdpSocket::new)
    }

    fn new(socket: mio::net::UdpSocket) -> UdpSocket {
        let io = PollEvented::new(socket);
        UdpSocket { io: io }
    }

    /// Returns the local address that this listener is bound to.
    ///
    /// This can be useful, for example, when binding to port 0 to figure out
    /// which port was actually bound.
    ///
    /// # Examples
    ///
    /// ```rust
    ///	use romio::udp::UdpSocket;
    ///
    ///	# fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let socket_addr = "127.0.0.1:0".parse()?;
    /// # let socket = UdpSocket::bind(&socket_addr)?;
    /// println!("Socket addr: {:?}", socket.local_addr());
    /// # Ok(())
    /// # }
    /// ```
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.io.get_ref().local_addr()
    }

    /// Sends data on the socket to the given address. On success, returns the
    /// number of bytes written.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// #![feature(async_await, await_macro, futures_api)]
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
    /// let addr = "127.0.0.1:0".parse()?;
    /// let target = "127.0.0.1:7878".parse()?;
    /// let mut socket = UdpSocket::bind(&addr)?;
    ///
    /// await!(socket.send_to(THE_MERCHANT_OF_VENICE, &target))?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn send_to<'a, 'b>(&'a mut self, buf: &'b [u8], target: &'b SocketAddr) -> SendTo<'a, 'b> {
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
    /// ```rust,no_run
    /// #![feature(futures_api, async_await, await_macro)]
    /// # use std::error::Error;
    /// use romio::udp::UdpSocket;
    ///
    /// # async fn recv_data() -> Result<Vec<u8>, Box<dyn Error + 'static>> {
    /// let addr = "127.0.0.1:0".parse()?;
    /// let mut socket = UdpSocket::bind(&addr)?;
    /// let mut buf = vec![0; 1024];
    ///
    /// await!(socket.recv_from(&mut buf))?;
    /// # Ok(buf)
    /// # }
    /// ```
    pub fn recv_from<'a, 'b>(&'a mut self, buf: &'b mut [u8]) -> RecvFrom<'a, 'b> {
        RecvFrom { buf, socket: self }
    }

    /// Gets the value of the `SO_BROADCAST` option for this socket.
    ///
    /// For more information about this option, see [`set_broadcast`].
    ///
    /// [`set_broadcast`]: #method.set_broadcast
    pub fn broadcast(&self) -> io::Result<bool> {
        self.io.get_ref().broadcast()
    }

    /// Sets the value of the `SO_BROADCAST` option for this socket.
    ///
    /// When enabled, this socket is allowed to send packets to a broadcast
    /// address.
    pub fn set_broadcast(&self, on: bool) -> io::Result<()> {
        self.io.get_ref().set_broadcast(on)
    }

    /// Gets the value of the `IP_MULTICAST_LOOP` option for this socket.
    ///
    /// For more information about this option, see [`set_multicast_loop_v4`].
    ///
    /// [`set_multicast_loop_v4`]: #method.set_multicast_loop_v4
    pub fn multicast_loop_v4(&self) -> io::Result<bool> {
        self.io.get_ref().multicast_loop_v4()
    }

    /// Sets the value of the `IP_MULTICAST_LOOP` option for this socket.
    ///
    /// If enabled, multicast packets will be looped back to the local socket.
    ///
    /// # Note
    ///
    /// This may not have any affect on IPv6 sockets.
    pub fn set_multicast_loop_v4(&self, on: bool) -> io::Result<()> {
        self.io.get_ref().set_multicast_loop_v4(on)
    }

    /// Gets the value of the `IP_MULTICAST_TTL` option for this socket.
    ///
    /// For more information about this option, see [`set_multicast_ttl_v4`].
    ///
    /// [`set_multicast_ttl_v4`]: #method.set_multicast_ttl_v4
    pub fn multicast_ttl_v4(&self) -> io::Result<u32> {
        self.io.get_ref().multicast_ttl_v4()
    }

    /// Sets the value of the `IP_MULTICAST_TTL` option for this socket.
    ///
    /// Indicates the time-to-live value of outgoing multicast packets for
    /// this socket. The default value is 1 which means that multicast packets
    /// don't leave the local network unless explicitly requested.
    ///
    /// # Note
    ///
    /// This may not have any affect on IPv6 sockets.
    pub fn set_multicast_ttl_v4(&self, ttl: u32) -> io::Result<()> {
        self.io.get_ref().set_multicast_ttl_v4(ttl)
    }

    /// Gets the value of the `IPV6_MULTICAST_LOOP` option for this socket.
    ///
    /// For more information about this option, see [`set_multicast_loop_v6`].
    ///
    /// [`set_multicast_loop_v6`]: #method.set_multicast_loop_v6
    pub fn multicast_loop_v6(&self) -> io::Result<bool> {
        self.io.get_ref().multicast_loop_v6()
    }

    /// Sets the value of the `IPV6_MULTICAST_LOOP` option for this socket.
    ///
    /// Controls whether this socket sees the multicast packets it sends itself.
    ///
    /// # Note
    ///
    /// This may not have any affect on IPv4 sockets.
    pub fn set_multicast_loop_v6(&self, on: bool) -> io::Result<()> {
        self.io.get_ref().set_multicast_loop_v6(on)
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

    /// Executes an operation of the `IP_ADD_MEMBERSHIP` type.
    ///
    /// This function specifies a new multicast group for this socket to join.
    /// The address must be a valid multicast address, and `interface` is the
    /// address of the local interface with which the system should join the
    /// multicast group. If it's equal to `INADDR_ANY` then an appropriate
    /// interface is chosen by the system.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use romio::udp::UdpSocket;
    /// use std::net::Ipv4Addr;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let socket_addr = "127.0.0.1:0".parse()?;
    /// let interface = Ipv4Addr::new(0, 0, 0, 0);
    /// let mdns_addr = Ipv4Addr::new(224, 0, 0, 123);
    ///
    /// let socket = UdpSocket::bind(&socket_addr)?;
    /// socket.join_multicast_v4(&mdns_addr, &interface)?;
    /// # Ok(()) }
    /// ```
    pub fn join_multicast_v4(&self, multiaddr: &Ipv4Addr, interface: &Ipv4Addr) -> io::Result<()> {
        self.io.get_ref().join_multicast_v4(multiaddr, interface)
    }

    /// Executes an operation of the `IPV6_ADD_MEMBERSHIP` type.
    ///
    /// This function specifies a new multicast group for this socket to join.
    /// The address must be a valid multicast address, and `interface` is the
    /// index of the interface to join/leave (or 0 to indicate any interface).
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use romio::udp::UdpSocket;
    /// use std::net::{Ipv6Addr, SocketAddr};
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let socket_addr = SocketAddr::new(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0).into(), 0);
    /// let mdns_addr = Ipv6Addr::new(0xFF02, 0, 0, 0, 0, 0, 0, 0x0123) ;
    /// let socket = UdpSocket::bind(&socket_addr)?;
    ///
    /// socket.join_multicast_v6(&mdns_addr, 0)?;
    /// # Ok(()) }
    /// ```
    pub fn join_multicast_v6(&self, multiaddr: &Ipv6Addr, interface: u32) -> io::Result<()> {
        self.io.get_ref().join_multicast_v6(multiaddr, interface)
    }

    /// Executes an operation of the `IP_DROP_MEMBERSHIP` type.
    ///
    /// For more information about this option, see [`join_multicast_v4`].
    ///
    /// [`join_multicast_v4`]: #method.join_multicast_v4
    pub fn leave_multicast_v4(&self, multiaddr: &Ipv4Addr, interface: &Ipv4Addr) -> io::Result<()> {
        self.io.get_ref().leave_multicast_v4(multiaddr, interface)
    }

    /// Executes an operation of the `IPV6_DROP_MEMBERSHIP` type.
    ///
    /// For more information about this option, see [`join_multicast_v6`].
    ///
    /// [`join_multicast_v6`]: #method.join_multicast_v6
    pub fn leave_multicast_v6(&self, multiaddr: &Ipv6Addr, interface: u32) -> io::Result<()> {
        self.io.get_ref().leave_multicast_v6(multiaddr, interface)
    }
}

impl AsyncDatagram for UdpSocket {
    type Sender = SocketAddr;
    type Receiver = SocketAddr;
    type Err = io::Error;

    fn poll_send_to(
        &mut self,
        waker: &Waker,
        buf: &[u8],
        receiver: &Self::Receiver,
    ) -> Poll<io::Result<usize>> {
        ready!(self.io.poll_write_ready(waker)?);

        match self.io.get_ref().send_to(buf, receiver) {
            Ok(n) => Poll::Ready(Ok(n)),
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                self.io.clear_write_ready(waker)?;
                Poll::Pending
            }
            Err(e) => Poll::Ready(Err(e)),
        }
    }

    fn poll_recv_from(
        &mut self,
        waker: &Waker,
        buf: &mut [u8],
    ) -> Poll<io::Result<(usize, Self::Sender)>> {
        ready!(self.io.poll_read_ready(waker)?);

        match self.io.get_ref().recv_from(buf) {
            Ok(n) => Poll::Ready(Ok(n)),
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                self.io.clear_read_ready(waker)?;
                Poll::Pending
            }
            Err(e) => Poll::Ready(Err(e)),
        }
    }
}

impl AsyncReadReady for UdpSocket {
    type Ok = mio::Ready;
    type Err = io::Error;

    /// Check the UDP socket's read readiness state.
    ///
    /// If the socket is not ready for receiving then `Poll::Pending` is
    /// returned and the current task is notified once a new event is received.
    ///
    /// The socket will remain in a read-ready state until calls to `poll_recv`
    /// return `Pending`.
    fn poll_read_ready(&self, waker: &Waker) -> Poll<Result<Self::Ok, Self::Err>> {
        self.io.poll_read_ready(waker)
    }
}

impl AsyncWriteReady for UdpSocket {
    type Ok = mio::Ready;
    type Err = io::Error;

    /// Check the UDP socket's write readiness state.
    ///
    /// If the socket is not ready for sending then `Poll::Pending` is
    /// returned and the current task is notified once a new event is received.
    ///
    /// The I/O resource will remain in a write-ready state until calls to
    /// `poll_send` return `Pending`.
    fn poll_write_ready(&self, waker: &Waker) -> Poll<Result<Self::Ok, Self::Err>> {
        self.io.poll_write_ready(waker)
    }
}

impl fmt::Debug for UdpSocket {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.io.get_ref().fmt(f)
    }
}

#[cfg(all(unix))]
mod sys {
    use super::UdpSocket;
    use std::os::unix::prelude::*;

    impl AsRawFd for UdpSocket {
        fn as_raw_fd(&self) -> RawFd {
            self.io.get_ref().as_raw_fd()
        }
    }
}

/// The future returned by `UdpSocket::send_to`
#[derive(Debug)]
pub struct SendTo<'a, 'b> {
    socket: &'a mut UdpSocket,
    buf: &'b [u8],
    target: &'b SocketAddr,
}

impl<'a, 'b> Future for SendTo<'a, 'b> {
    type Output = io::Result<usize>;

    fn poll(mut self: Pin<&mut Self>, waker: &Waker) -> Poll<Self::Output> {
        let SendTo {
            socket,
            buf,
            target,
        } = &mut *self;
        socket.poll_send_to(waker, buf, target)
    }
}

/// The future returned by `UdpSocket::recv_from`
#[derive(Debug)]
pub struct RecvFrom<'a, 'b> {
    socket: &'a mut UdpSocket,
    buf: &'b mut [u8],
}

impl<'a, 'b> Future for RecvFrom<'a, 'b> {
    type Output = io::Result<(usize, SocketAddr)>;

    fn poll(mut self: Pin<&mut Self>, waker: &Waker) -> Poll<Self::Output> {
        let RecvFrom { socket, buf } = &mut *self;
        socket.poll_recv_from(waker, buf)
    }
}
