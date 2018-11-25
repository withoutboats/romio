//! Networking primitives

pub mod tcp;
pub mod udp;

pub use self::tcp::{TcpListener, TcpStream};
#[doc(inline)]
pub use self::udp::UdpSocket;
