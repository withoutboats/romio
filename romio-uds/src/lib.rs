#![feature(pin, futures_api, arbitrary_self_types)]
#![cfg(unix)]
#![doc(html_root_url = "https://docs.rs/tokio-uds/0.2.3")]
#![deny(missing_docs, warnings, missing_debug_implementations)]

//! Unix Domain Sockets for Tokio.
//!
//! This crate provides APIs for using Unix Domain Sockets with Tokio.

mod datagram;
mod listener;
mod stream;
mod ucred;

pub use crate::datagram::{UnixDatagram, RecvDgram, SendDgram};
pub use crate::listener::{UnixListener, Incoming};
pub use crate::stream::{UnixStream, ConnectFuture};
pub use crate::ucred::UCred;
