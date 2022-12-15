use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use async_std::task::block_on;
use fnv::FnvHashMap;
use ipfs_embed::Block;
use ipfs_embed::Cid;
use ipfs_embed::DefaultParams;
use libipld::IpldCodec;
use libp2p::Multiaddr;
use libp2p::Swarm;
use libp2p::Transport;
use libp2p::futures::StreamExt;
use libp2p::futures::select;
use libp2p::core::identity;
use libp2p::core::PeerId;
use libp2p::core::muxing::StreamMuxerBox;
use libp2p::core::transport::Boxed;
use libp2p::core::upgrade::Version;
use libp2p::noise;
use libp2p::ping;
use libp2p::pnet::PnetConfig;
use libp2p::swarm::NetworkBehaviour;
use libp2p::swarm::SwarmEvent;
use libp2p::swarm::behaviour;
use libp2p_bitswap::Bitswap;
use libp2p_bitswap::BitswapConfig;
use libp2p_bitswap::BitswapEvent;
use libp2p_bitswap::BitswapStore;
use libp2p_tcp;

use libp2p::swarm::{keep_alive,};
use async_std;
use libp2p::yamux::YamuxConfig;
use libp2p_tcp::async_io;
use clap::{arg, command, value_parser, ArgAction, Command};
use libipld::store::StoreParams;
use ipfs_sqlite_block_store::{BlockStore, DbPath};

pub fn build_transport(
    key_pair: identity::Keypair,
    
) -> Boxed<(PeerId, StreamMuxerBox)>{
    let base_transport = async_io::Transport::new(libp2p_tcp::Config::default().nodelay(true));
    let noise_config = noise::NoiseAuthenticated::xx(&key_pair).unwrap();
    let yamux_config = YamuxConfig::default();

    base_transport
    .upgrade(Version::V1)
    .authenticate(noise_config)
    .multiplex(yamux_config)
    .boxed()
}



fn main() {
    block_on(async_main());
}

#[derive(Debug, Clone, Default)]
struct Sp {

}




impl StoreParams for Sp {
    type Hashes = libipld::multihash::Code;
    type Codecs = IpldCodec;
    const MAX_BLOCK_SIZE: usize = 1024 * 1024 * 4;
}

#[derive(Debug, Clone, Default)]
struct S(FnvHashMap<Cid, Vec<u8>>);

impl BitswapStore for S {
    type Params = ipfs_embed::DefaultParams;

    fn contains(&mut self, cid: &Cid) -> libipld::Result<bool> {
        Ok(self.0.contains_key(&cid))
    }

    fn get(&mut self, cid: &Cid) -> libipld::Result<Option<Vec<u8>>> {
        Ok(self.0.get(cid).cloned())
    }

    fn insert(&mut self, block: &ipfs_embed::Block<Self::Params>) -> libipld::Result<()> {
        self.0
                .insert(*block.cid(), block.data().to_vec());
            Ok(())
    }

    fn missing_blocks(&mut self, cid: &ipfs_embed::Cid) -> libipld::Result<Vec<ipfs_embed::Cid>> {
        let mut stack = vec![*cid];
        let mut missing = vec![];
        while let Some(cid) = stack.pop() {
            if let Some(data) = self.get(&cid)? {
                let block = Block::<Self::Params>::new_unchecked(cid, data);
                block.references(&mut stack)?;
            } else {
                missing.push(cid);
            }
        }
        Ok(missing)
    }
}

#[derive(NetworkBehaviour)]
#[behaviour()]
struct MyBehaviour {
        keep_alive: keep_alive::Behaviour,
        bitswap: Bitswap<DefaultParams>,
        
}

async fn async_main() {

    let mut enabled = false;

    let matches = command!().
    arg(
        arg!(
            --dht <bool> "Argument to enable the DHT"
        )
    ).get_matches();

    if let Some(dht_enabled) = matches.get_one::<bool>("dht") {
        enabled = dht_enabled.clone();
    }

    let local_key = identity::Keypair::generate_ed25519();
    let local_peer_id = PeerId::from(local_key.public());
    println!("{:?}", local_peer_id);

    let transport = build_transport(local_key.clone());
   
    let mut store = S::default();

    

    let bs = Bitswap::<DefaultParams>::new(BitswapConfig::default(), store);

    let behaviour = MyBehaviour {
        keep_alive: keep_alive::Behaviour,
        bitswap: bs,
    };

    let mut swarm = {
        Swarm::with_async_std_executor(
            transport,
            behaviour,
            local_peer_id
        )
    };
    
    let cid: Cid  = "QmQbKSZKnDeR4bJT65hN9cjbJspheDHt9tZDqUMv7NBpMt".parse().unwrap();
    let peer: PeerId = "12D3KooWScdNmkkfXipjs6PHwFZAzMxce6Qhgn7WtFDNorrq2rcH".parse().unwrap();
    let addr: Multiaddr = "/ip4/192.168.1.24/tcp/4001".parse().unwrap();
    swarm.dial(addr.clone()).unwrap();

    let _ = swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse().unwrap());
    let peers = vec![peer];
    //let missing = store.missing_blocks(&cid).unwrap();
    swarm.behaviour_mut().bitswap.add_address(&peer, addr);
    //let updates = swarm.behaviour_mut().bitswap.sync(cid, peers, missing.into_iter());
    

    

    

    loop {
        select! {
            event = swarm.select_next_some() => {
                match event {
                    SwarmEvent::NewListenAddr { address, .. } => {
                        println!("Listening on {}", address);
                    }
                    SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                        println!("Connection established with rendezvous point {}", peer_id);
                    }
                    SwarmEvent::Behaviour(
                        MyBehaviour
                    ) => {

                    }  
                    other => {
                        println!("Unhandled {:?}", other);
                    }
                }
            },

        }
    }

}
