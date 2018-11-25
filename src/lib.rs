//! O Romio, Romio, wherefore art thou Romio?
//! Deny thy father and refuse thy name;
//! Or if thou wilt not, be but sworn my love
//! And I'll no longer be asynchronous
#![feature(futures_api, pin, arbitrary_self_types)]
#![doc(html_root_url = "https://docs.rs/tokio-reactor/0.1.6")]
#![deny(missing_docs, missing_debug_implementations)]
#![cfg_attr(test, deny(warnings))]

pub mod net;

#[cfg(unix)]
pub mod uds;

mod reactor;
