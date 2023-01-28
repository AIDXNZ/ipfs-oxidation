

use std::thread;

use async_std::{task::block_on, stream::StreamExt};
use futures::prelude;
use ipfs_embed::{Ipfs, Config, DefaultParams, PeerId, Multiaddr, Cid};

fn main() {
    let handler = thread::spawn(||{
        block_on(async_main());
    });
    handler.join().unwrap();
}

async fn async_main()  {
    let config = Config::default();
    let mut ipfs = Ipfs::<DefaultParams>::new(config).await.unwrap();

    let peer_id: PeerId = "12D3KooWScdNmkkfXipjs6PHwFZAzMxce6Qhgn7WtFDNorrq2rcH".parse().unwrap();
    let addr: Multiaddr = "/ip4/192.168.1.24/tcp/4001".parse().unwrap();

    ipfs.dial_address(peer_id, addr);

    let cid: Cid = "QmVML2curkDqg1qJ1aDzzhS7oALeERM5yDhSB4vKxsokHx".parse().unwrap();

    let mut events =  ipfs.sync(&cid, vec![peer_id]).await.unwrap();

    while let Some(event) = events.next().await {
        println!("{:?}", event);
    }

    let block = ipfs.fetch(&cid, vec![peer_id]).await.unwrap();
    println!("{:?}", block.data());
    

}