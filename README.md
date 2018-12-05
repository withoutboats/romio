# Romio

[![build status][travis-badge]][travis-url]
[![crates.io version][crates-badge]][crates-url]
[![docs.rs docs][docs-badge]][docs-url]
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

[travis-badge]: https://img.shields.io/travis/withoutboats/romio/master.svg?style=flat-square
[travis-url]: https://travis-ci.org/withoutboats/romio
[crates-badge]: https://img.shields.io/crates/v/romio.svg?style=flat-square
[crates-url]: https://crates.io/crates/romio
[docs-badge]: https://img.shields.io/badge/docs-latest-blue.svg?style=flat-square
[docs-url]: https://withoutboats.github.io/romio/romio
[mit-badge]: https://img.shields.io/badge/license-MIT-blue.svg?style=flat-square
[mit-url]: LICENSE-MIT

## Examples

Here are two example programs using romio: a TCP server which serves a random
quote of Shakespeare, and a TCP client which connects to that server and prints
the quote it received to standard out.

### Shakespeare Server

```rust
#![feature(async_await, await_macro, futures_api)]

use std::io;

use futures::StreamExt;
use futures::executor::{self, ThreadPool};
use futures::io::AsyncWriteExt;
use futures::task::{SpawnExt};

use rand::seq::SliceRandom;

use romio::{TcpListener, TcpStream};

const SHAKESPEARE: &[&[u8]] = &[
    b"Now is the winter of our discontent\nMade glorious summer by this sun of York.\n",
    b"Some are born great, some achieve greatness\nAnd some have greatness thrust upon them.\n",
    b"Friends, Romans, countrymen - lend me your ears!\nI come not to praise Caesar, but to bury him.\n",
    b"The evil that men do lives after them\nThe good is oft interred with their bones.\n",
    b"                  It is a tale\nTold by an idiot, full of sound and fury\nSignifying nothing.\n",
    b"Ay me! For aught that I could ever read,\nCould ever hear by tale or history,\nThe course of true love never did run smooth.\n",
    b"I have full cause of weeping, but this heart\nShall break into a hundred thousand flaws,\nOr ere I'll weep.-O Fool, I shall go mad!\n",
    b"                  Each your doing,\nSo singular in each particular,\nCrowns what you are doing in the present deed,\nThat all your acts are queens.\n",
];

fn main() -> io::Result<()> {
    executor::block_on(async {
        let mut threadpool = ThreadPool::new()?;

        let listener = TcpListener::bind(&"127.0.0.1:7878".parse().unwrap())?;
        let mut incoming = listener.incoming();

        println!("Listening on 127.0.0.1:7878");

        while let Some(stream) = await!(incoming.next()) {
            let stream = stream?;
            let addr = stream.peer_addr()?;

            threadpool.spawn(async move {
                println!("Accepting stream from: {}", addr);

                await!(recite_shakespeare(stream)).unwrap();

                println!("Closing stream from: {}", addr);
            }).unwrap();
        }

        Ok(())
    })
}

async fn recite_shakespeare(mut stream: TcpStream) -> io::Result<()> {
    //stream.set_keepalive(None);
    let &quote = SHAKESPEARE.choose(&mut rand::thread_rng()).unwrap();
    await!(stream.write_all(quote))?;
    Ok(())
}
```

### Shakespeare Client

```rust
#![feature(async_await, await_macro, futures_api)]

use std::io;

use futures::executor;
use futures::io::{AsyncReadExt, AllowStdIo};

use romio::TcpStream;

fn main() -> io::Result<()> {
    executor::block_on(async {
        let mut stream = await!(TcpStream::connect(&"127.0.0.1:7878".parse().unwrap()))?;
        let mut stdout = AllowStdIo::new(io::stdout());
        await!(stream.copy_into(&mut stdout))?;
        Ok(())
    })
}
```

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
if you want to use those, romio might not be a good fit for you.

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
