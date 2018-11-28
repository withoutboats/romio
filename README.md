# Romio

[![MIT licensed][mit-badge]][mit-url]

Asynchronous network primitives in Rust.

Romio combines the powerful [`futures`][futures] abstractions with the
nonblocking IO primitives of [`mio`][mio] to provide efficient and ergonomic
asynchronous IO primitives for the Rust asynchronous networking ecosystem.
Romio's primitives are:

* **Fast**: The zero-cost `Future` abstractions give you bare metal
  performance, despite the higher level API.
* **Reliable**: Romio leverages Rust's type system to reduce bugs and ensure
  thread safety among concurrently executing asynchronous functions.
* **Scalable**: Romio has a minimal footprint and handles backpressure and
  cancellation naturally.

Romio is based on the [Tokio][tokio] crate, porting components from it to a
newer version of the futures crate.

[mit-badge]: https://img.shields.io/badge/license-MIT-blue.svg
[mit-url]: LICENSE-MIT

## Examples

TODO add example

## Relationship to Tokio

Romio is a fork of another Rust project called [Tokio][tokio]. The Tokio
project uses an older version of the futures API which is not compatible with
the new "async/await" syntax. In order to enable people to experiment with
"async/await," Romio ports parts of the tokio project to the newer futures API
which is compatible with that syntax.

Romio is not a complete port of tokio: it only contains a small part of the
entire tokio code base: the IO primitives necessary for writing asynchronous
networking code. It does not expose low level control of the core "reactor" -
instead, all async IO primitives use the default reactor set up - and it
doesn't contain many other parts of tokio that are not directly related to
asynchronous IO.

You should use romio if you want to experiment with writing networking code
using the new async/await syntax. However, romio is not directly compatible
with other libraries built on top of tokio - like hyper, actix, and tower - so
if you want to use those, romio not be a good fit for you.

Romio is intended to unblock people trying to experiment with async/await,
which is why it exposes such a minimal API. It's not intended to be a full
fledged "competitor" to tokio, which we expect will eventually move to the
newer futures API and be compatible with async/await syntax.

## License

This project is licensed under the [MIT license](LICENSE).

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in romio by you, shall be licensed as MIT, without any additional
terms or conditions.

[futures]: https://crates.io/crates/futures
[mio]: https://crates.io/crates/mio
[tokio]: https://crates.io/crates/tokio
