#![feature(async_await, await_macro)]

use std::io;

use futures::executor;
use futures::io::{AllowStdIo, AsyncReadExt};

use romio::TcpStream;

fn main() -> io::Result<()> {
    executor::block_on(async {
        let mut stream = await!(TcpStream::connect(&"127.0.0.1:7878".parse().unwrap()))?;
        let mut stdout = AllowStdIo::new(io::stdout());
        await!(stream.copy_into(&mut stdout))?;
        Ok(())
    })
}
