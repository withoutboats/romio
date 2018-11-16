# Romio

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

[![MIT licensed][mit-badge]][mit-url]

[mit-badge]: https://img.shields.io/badge/license-MIT-blue.svg
[mit-url]: LICENSE-MIT

The API docs for the master branch are published [here][master-dox].

## Example


## Relationship to Tokio

Romio is a fork of another Rust project called [Tokio][tokio]. The Tokio
project uses an outdated version of the futures API, and has not yet ported to
the API currently under development. This means that Tokio, and projects built
on top of it, are not directly compatible with Rust's "async/await" syntax for
writing asynchronous code, and require compatibility shims.

Romio forks the most essential parts of tokio - the reactor & IO primitives -
to the current futures API, so that we can get more user feedback on the
async/await feature and the current futures APIs. You should use romio if you
want to try using async/await.

**However**, you should prefer to use tokio if you want to use the stable
version of Rust, or because you want to use libraries like hyper, actix, or
tower (which all depend on tokio).

It is our intention that in the future the changes made in romio to be
compatible with futures 0.3 will eventually be upstreamed back to tokio and the
romio fork will cease to be maintained.

## License

This project is licensed under the [MIT license](LICENSE).

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in Tokio by you, shall be licensed as MIT, without any additional
terms or conditions.

[futures]: https://crates.io/crates/futures
[mio]: https://crates.io/crates/mio
[tokio]: https://crates.io/crates/tokio
