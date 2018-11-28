#![feature(async_await, await_macro, futures_api)]

use std::io;

use futures::StreamExt;
use futures::executor::{self, ThreadPool};
use futures::io::AsyncReadExt;
use futures::task::{SpawnExt};

use romio::{TcpListener, TcpStream};

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

                await!(echo_on(stream, addr)).unwrap();

                println!("Closing stream from: {}", addr);
            }).unwrap();
        }

        Ok(())
    })
}

async fn echo_on(stream: TcpStream) -> io::Result<()> {
    let (mut reader, mut writer) = stream.split();
    await!(reader.copy_into(&mut writer))?;
    Ok(())
}
