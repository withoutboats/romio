#![cfg(unix)]
#![feature(async_await, await_macro, pin)]
use std::io::{Read, Write};
use std::os::unix::net::UnixStream as StdStream;
use std::thread;

use futures::executor;
use futures::future::FutureObj;
use futures::io::{AsyncReadExt, AsyncWriteExt};
use futures::StreamExt;
use tempdir::TempDir;

use romio::uds::{UnixListener, UnixStream};

type Error = Box<dyn std::error::Error + 'static>;

const THE_WINTERS_TALE: &[u8] = b"
                    Each your doing,
    So singular in each particular,
    Crowns what you are doing in the present deed,
    That all your acts are queens.
";

#[test]
fn listener_reads() -> Result<(), Error> {
    drop(env_logger::try_init());
    let tmp_dir = TempDir::new("listener_reads")?;
    let file_path = tmp_dir.path().join("sock");

    let listener = UnixListener::bind(&file_path)?;
    let file_path = listener.local_addr()?;

    // client thread
    thread::spawn(move || {
        let file_path = file_path.as_pathname().unwrap();
        let mut client = StdStream::connect(&file_path).unwrap();
        client.write_all(THE_WINTERS_TALE).unwrap();
    });

    executor::block_on(async {
        let mut buf = vec![0; THE_WINTERS_TALE.len()];
        let mut incoming = listener.incoming();
        let mut stream = await!(incoming.next()).unwrap().unwrap();
        await!(stream.read_exact(&mut buf)).unwrap();
        assert_eq!(buf, THE_WINTERS_TALE);
    });

    Ok(())
}

#[test]
fn listener_writes() -> Result<(), Error> {
    drop(env_logger::try_init());
    let tmp_dir = TempDir::new("listener_writes")?;
    let file_path = tmp_dir.path().join("sock");

    let listener = UnixListener::bind(&file_path)?;
    let file_path = listener.local_addr()?;

    // client thread
    thread::spawn(move || {
        let mut buf = vec![0; THE_WINTERS_TALE.len()];
        let file_path = file_path.as_pathname().unwrap();
        let mut client = StdStream::connect(&file_path).unwrap();
        client.read_exact(&mut buf).unwrap();
        assert_eq!(buf, THE_WINTERS_TALE);
    });

    executor::block_on(async {
        let mut incoming = listener.incoming();
        let mut stream = await!(incoming.next()).unwrap().unwrap();
        await!(stream.write_all(THE_WINTERS_TALE)).unwrap();
    });

    Ok(())
}

#[test]
fn both_sides_async_using_threadpool() -> Result<(), Error>{
    drop(env_logger::try_init());
    let tmp_dir = TempDir::new("listener_writes")?;
    let file_path = tmp_dir.path().join("sock");

    let listener = UnixListener::bind(&file_path)?;
    let file_path = listener.local_addr()?;

    let mut pool = executor::ThreadPool::new().unwrap();

    pool.run(FutureObj::from(Box::pinned(async move {
        let file_path = file_path.as_pathname().unwrap();
        let mut client = await!(UnixStream::connect(&file_path)).unwrap();
        await!(client.write_all(THE_WINTERS_TALE)).unwrap();
    })));

    pool.run(FutureObj::from(Box::pinned(async {
        let mut buf = vec![0; THE_WINTERS_TALE.len()];
        let mut incoming = listener.incoming();
        let mut stream = await!(incoming.next()).unwrap().unwrap();
        await!(stream.read_exact(&mut buf)).unwrap();
        assert_eq!(buf, THE_WINTERS_TALE);
    })));

    Ok(())
}
