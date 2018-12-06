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

        let mut listener = TcpListener::bind(&"127.0.0.1:7878".parse().unwrap())?;
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
