use std::io::{Read, Write};
use std::net::TcpStream;
use std::thread;

use futures::executor;
use futures::io::{AsyncReadExt, AsyncWriteExt};
use futures::StreamExt;

use romio::TcpListener;

const THE_WINTERS_TALE: &[u8] = b"
                    Each your doing,
    So singular in each particular,
    Crowns what you are doing in the present deed,
    That all your acts are queens.
";

#[test]
fn listener_reads() {
    drop(env_logger::try_init());
    let mut server = TcpListener::bind(&"127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = server.local_addr().unwrap();

    // client thread
    thread::spawn(move || {
        let mut client = TcpStream::connect(&addr).unwrap();
        client.write_all(THE_WINTERS_TALE).unwrap();
    });

    executor::block_on(async {
        let mut buf = vec![0; THE_WINTERS_TALE.len()];
        let mut incoming = server.incoming();
        let mut stream = incoming.next().await.unwrap().unwrap();
        stream.read_exact(&mut buf).await.unwrap();
        assert_eq!(buf, THE_WINTERS_TALE);
    });
}

#[test]
fn listener_writes() {
    drop(env_logger::try_init());
    let mut server = TcpListener::bind(&"127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = server.local_addr().unwrap();

    // client thread
    thread::spawn(move || {
        let mut buf = vec![0; THE_WINTERS_TALE.len()];
        let mut client = TcpStream::connect(&addr).unwrap();
        client.read_exact(&mut buf).unwrap();
        assert_eq!(buf, THE_WINTERS_TALE);
    });

    executor::block_on(async {
        let mut incoming = server.incoming();
        let mut stream = incoming.next().await.unwrap().unwrap();
        stream.write_all(THE_WINTERS_TALE).await.unwrap();
    });
}

#[test]
fn both_sides_async_using_threadpool() {
    drop(env_logger::try_init());
    let mut server = TcpListener::bind(&"127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = server.local_addr().unwrap();

    let mut pool = executor::ThreadPool::new().unwrap();

    pool.run(Box::pin(async move {
        let mut client = romio::TcpStream::connect(&addr).await.unwrap();
        client.write_all(THE_WINTERS_TALE).await.unwrap();
    }));

    pool.run(Box::pin(async {
        let mut buf = vec![0; THE_WINTERS_TALE.len()];
        let mut incoming = server.incoming();
        let mut stream = incoming.next().await.unwrap().unwrap();
        stream.read_exact(&mut buf).await.unwrap();
        assert_eq!(buf, THE_WINTERS_TALE);
    }));
}
