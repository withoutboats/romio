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
//! #![feature(async_await, await_macro, futures_api)]
//! use romio::tcp::{TcpListener, TcpStream};
//! use futures::prelude::*;
//!
//! async fn handle_client(mut stream: TcpStream) {
//!     await!(stream.write_all(b"Hello, client!"));
//! }
//!
//! async fn listen() -> Result<(), Box<dyn std::error::Error + 'static>> {
//!     let socket_addr = "127.0.0.1:80".parse()?;
//!     let listener = TcpListener::bind(&socket_addr)?;
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
