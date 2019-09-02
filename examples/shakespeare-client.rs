#![feature(async_await)]

use std::io;

use futures::executor;
use futures::io::{AllowStdIo, AsyncReadExt};

use romio::TcpStream;

fn main() -> io::Result<()> {
    executor::block_on(async {
        let stream = TcpStream::connect(&"127.0.0.1:7878".parse().unwrap()).await?;
        let mut stdout = AllowStdIo::new(io::stdout());
        stream.copy_into(&mut stdout).await?;
        Ok(())
    })
}
