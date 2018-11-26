//! *O Romio, Romio, wherefore art thou Romio?*<br/>
//! *Deny thy father and refuse thy name;*<br/>
//! *Or if thou wilt not, be but sworn my love*<br/>
//! *And I'll no longer be asynchronous*
//!
//! # Asynchronous networking bindings for TCP, UDP, and UDS
//!
//! The types defined in this module are designed to closely follow the APIs of the
//! analogous types in `std::net`. But rather than implementing synchronous traits
//! like `std::io::{Read, Write}`, these types implement the asychronous versions
//! provided by the `futures-preview` crate, i.e. `futures::io::{AsyncRead, AsyncWrite}`.
//! When using `async`/`await` syntax, the experience should be quite similar to
//! traditional blocking code that uses `std::net`.
//!
//! Because futures-preview is currently unstable, this crate requires
//! nightly Rust.
//!
//! ## Examples
//! __TCP Server__
//! ```rust
//!#![feature(async_await)]
//! use romio::tcp::{TcpListener, TcpStream};
//! use futures::prelude::*;
//!
//! async fn say_hello(stream: TcpStream) {
//!     await!(stream.write_all(b"Hello, client!"));
//! }
//!
//! async fn listen() -> io::Result<()> {
//!     let listener = TcpListener::bind("127.0.0.1:80")?;
//!     let mut incoming = listener.incoming();
//!
//!     // accept connections and process them serially
//!     while let Some(stream) = await!(incoming.next()) {
//!         await!(say_hello(stream?));
//!     }
//!     Ok(())
//! }
//! ```

#![feature(futures_api, pin, arbitrary_self_types)]
#![doc(html_root_url = "https://docs.rs/tokio-reactor/0.1.6")]
#![deny(missing_docs, missing_debug_implementations)]
#![cfg_attr(test, deny(warnings))]

pub mod tcp;
pub mod udp;

#[cfg(unix)]
pub mod uds;

mod reactor;

#[doc(inline)]
pub use crate::tcp::{TcpStream, TcpListener};
#[doc(inline)]
pub use crate::udp::UdpSocket;
