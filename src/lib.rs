//! *O Romio, Romio, wherefore art thou Romio?*<br/>
//! *Deny thy father and refuse thy name;*<br/>
//! *Or if thou wilt not, be but sworn my love*<br/>
//! *And I'll no longer be so synchronous.*
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
//! # Examples
//! __TCP Server__
//! ```rust
//! #![feature(async_await, await_macro, futures_api)]
//! use romio::tcp::{TcpListener, TcpStream};
//! use futures::prelude::*;
//!
//! async fn say_hello(mut stream: TcpStream) {
//!     await!(stream.write_all(b"Shall I hear more, or shall I speak at this?"));
//! }
//!
//! async fn listen() -> Result<(), Box<dyn std::error::Error + 'static>> {
//!     let socket_addr = "127.0.0.1:8080".parse()?;
//!     let mut listener = TcpListener::bind(&socket_addr)?;
//!     let mut incoming = listener.incoming();
//!
//!     // accept connections and process them serially
//!     while let Some(stream) = await!(incoming.next()) {
//!         await!(say_hello(stream?));
//!     }
//!     Ok(())
//! }
//! ```
//! __TCP Client__
//! ```rust,no_run
//! #![feature(async_await, await_macro, futures_api)]
//! use std::error::Error;
//! use futures::prelude::*;
//! use romio::tcp::{TcpListener, TcpStream};
//!
//! async fn receive_sonnet() -> Result<(), Box<dyn Error + 'static>> {
//!     let socket_addr = "127.0.0.1:8080".parse()?;
//!     let mut buffer = vec![];
//!     let mut stream = await!(TcpStream::connect(&socket_addr))?;
//!
//!     await!(stream.read(&mut buffer))?;
//!     println!("{:?}", buffer);
//!     Ok(())
//! }
//! ```

#![feature(futures_api)]
#![deny(missing_docs, missing_debug_implementations)]
#![cfg_attr(test, deny(warnings))]

pub mod tcp;
pub mod udp;

pub use async_ready;

#[cfg(unix)]
pub mod uds;

pub mod raw;

mod reactor;

#[doc(inline)]
pub use crate::tcp::{TcpListener, TcpStream};
#[doc(inline)]
pub use crate::udp::UdpSocket;
