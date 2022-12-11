
use core::task;
use std::thread;

use async_std::{task::block_on, prelude::FutureExt, fs::File, io::WriteExt};
use futures::{join, select};
use ipfs_embed::{Config, Ipfs, DefaultParams, PeerId, Multiaddr, Cid, identity::PublicKey, SwarmEvents};
use libipld::{block, store::StoreParams, IpldCodec};
use futures::prelude::*;

const RAW: u64 = 0x55;

fn main() {
    block_on(run());
}
async fn run() {
    print!("start");
    let handle = thread::spawn(|| {
        block_on(async_main())
    });
    
    handle.join().expect("Failed");
    print!("here");
}
#[derive(Debug, Clone)]
struct Sp;


impl StoreParams for Sp {
    type Hashes = libipld::multihash::Code;
    type Codecs = IpldCodec;
    const MAX_BLOCK_SIZE: usize = 1024 * 1024 * 4;
}

async fn async_main() {
    
    let mut ipfs = Ipfs::<DefaultParams>::new(Config::default()).await.unwrap();
    
    let _ = ipfs.listen_on("/ip4/0.0.0.0/tcp/0".parse().unwrap());

    // Dial known go-ipfs Node
    let peer: PeerId = "12D3KooWScdNmkkfXipjs6PHwFZAzMxce6Qhgn7WtFDNorrq2rcH".parse().unwrap();
    let addr: Multiaddr = "/ip4/192.168.1.24/tcp/4001".parse().unwrap();
    ipfs.dial_address(peer, addr);

    
    
    let cid: Cid = "QmQbKSZKnDeR4bJT65hN9cjbJspheDHt9tZDqUMv7NBpMt".parse().unwrap();
    let mut providers = vec![];
    providers.push(peer);
    //ipfs.fetch(&cid, providers).await.expect("failed");
    //ipfs.sync(&cid, providers).await.expect("failed");

}
    
