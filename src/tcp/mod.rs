//! Async TCP bindings.
//!
//! This module contains the TCP networking types, similar to those found in
//! `std::net`, but suitable for async programming via futures and
//! `async`/`await`.
//!
//! - To connect to an address via TCP, use [`TcpStream::connect`].
//! - To listen for TCP connection, use [`TcpListener::bind`] and then
//!   [`TcpListener::incoming`].
//! - Once you have a [`TcpStream`], you can use methods from `AsyncRead`,
//!   `AsyncWrite`, and their extension traits (`AsyncReadExt`, `AsyncWriteExt`)
//!   to send and receive data.
//!
//! [`TcpStream`]: struct.TcpStream.html
//! [`TcpStream::connect`]: struct.TcpStream.html#method.connect
//! [`TcpListener::bind`]: struct.TcpListener.html#method.bind
//! [`TcpListener::incoming`]: struct.TcpListener.html#method.incoming
//!
//! # Example
//!
//! ```no_run
//! #![feature(async_await)]
//!
//! use romio::tcp::{TcpListener, TcpStream};
//!
//! async fn handle_client(stream: TcpStream) {
//!     // ...
//! }
//!
//! async fn main() -> io::Result<()> {
//!     let listener = TcpListener::bind("127.0.0.1:80")?;
//!     let mut incoming = listener.incoming();
//!
//!     // accept connections and process them serially
//!     while let Some(stream) = await!(incoming.next()) {
//!         await!(handle_client(stream?));
//!     }
//!     Ok(())
//! }
//! ```

mod listener;
mod stream;

pub use self::listener::{Incoming, TcpListener};
pub use self::stream::{ConnectFuture, TcpStream};
