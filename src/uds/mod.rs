//! Async UDS (Unix Domain Sockets) bindings.
//!
//! # Example
//!
//! ```no_run
//! #![feature(async_await, await_macro, futures_api)]
//! use romio::uds::{UnixListener, UnixStream};
//! use futures::prelude::*;
//!
//! async fn say_hello(mut stream: UnixStream) {
//!     await!(stream.write_all(b"Shall I hear more, or shall I speak at this?!"));
//! }
//!
//! async fn listen() -> Result<(), Box<dyn std::error::Error + 'static>> {
//!     let listener = UnixListener::bind("/tmp/sock")?;
//!     let mut incoming = listener.incoming();
//!
//!     // accept connections and process them serially
//!     while let Some(stream) = await!(incoming.next()) {
//!         await!(say_hello(stream?));
//!     }
//!     Ok(())
//! }
//! ```

mod datagram;
mod listener;
mod stream;
mod ucred;

pub use self::datagram::UnixDatagram;
pub use self::listener::{Incoming, UnixListener};
pub use self::stream::{ConnectFuture, UnixStream};
pub use self::ucred::UCred;
