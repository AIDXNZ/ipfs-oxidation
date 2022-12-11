use std::env;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use async_std::task::block_on;
use ipfs_embed::DefaultParams;
use ipfs_embed::PeerRecord;
use ipfs_embed::Record;
use libipld::Cid;
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
use libp2p::identify;
use libp2p::identify::Identify;
use libp2p::identify::IdentifyConfig;
use libp2p::identify::IdentifyEvent;
use libp2p::kad::AddProviderOk;
use libp2p::kad::GetClosestPeersOk;
use libp2p::kad::GetClosestPeersError;
use libp2p::kad::GetProvidersOk;
use libp2p::kad::GetRecordOk;
use libp2p::kad::Kademlia;
use libp2p::kad::KademliaEvent;
use libp2p::kad::PutRecordOk;
use libp2p::kad::QueryResult;
use libp2p::kad::store::MemoryStore;
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
use void::Void;
use libp2p::dns::{DnsConfig};

const BOOTNODES: [&str; 2] = [
    "QmNnooDu7bfjPFoTZYxMNLWUQJyrVwtbZg5gBMjTezGAJN",
    "QmaCpDMGvV2BGHeYERUEnRQAwe3N8SzbUtfsmvsqQLuvuJ",
];


pub async fn build_transport(
    key_pair: identity::Keypair,
    
) -> Boxed<(PeerId, StreamMuxerBox)>{
    let base_transport = DnsConfig::system(async_io::Transport::new(
        libp2p_tcp::Config::default().nodelay(true),
    )).await.unwrap();
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


#[derive(NetworkBehaviour)]
#[behaviour(out_event="MyBehaviourEvent")]
struct MyBehaviour {
        keep_alive: keep_alive::Behaviour,
        kad: Kademlia<MemoryStore>,
        identify: identify::Behaviour
}

#[allow(clippy::large_enum_variant)]
enum MyBehaviourEvent {
    Kademlia(KademliaEvent),
    Identify(identify::Event),
}

impl From<identify::Event> for MyBehaviourEvent {
        fn from(event: identify::Event) -> Self {
            MyBehaviourEvent::Identify(event)
        }
}

impl From<KademliaEvent> for MyBehaviourEvent {
    fn from(event: KademliaEvent) -> Self {
        MyBehaviourEvent::Kademlia(event)
    }
}

impl From<Void> for MyBehaviourEvent {
    fn from(event: Void) -> Self {
        void::unreachable(event)
    }
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

    let transport = build_transport(local_key.clone()).await;
   
    let store = MemoryStore::new(local_peer_id);

    let kademlia = Kademlia::new(local_peer_id, store);

    let mut behaviour = MyBehaviour {
        keep_alive: keep_alive::Behaviour,
        kad:  kademlia,
        identify: identify::Behaviour::new(identify::Config::new("/ipfs/id/1.0.0".to_string(), local_key.public()))
    };
    

    let mut swarm = {
        Swarm::with_async_std_executor(
            transport,
            behaviour,
            local_peer_id
        )
    };

    for peer in &BOOTNODES {
        swarm.behaviour_mut().kad.add_address(&peer.parse().unwrap(), "/dnsaddr/bootstrap.libp2p.io".parse().unwrap());
    }
    
    //let cid  = "QmQbKSZKnDeR4bJT65hN9cjbJspheDHt9tZDqUMv7NBpMt".parse().unwrap();
    let peer: PeerId = "12D3KooWScdNmkkfXipjs6PHwFZAzMxce6Qhgn7WtFDNorrq2rcH".parse().unwrap();
    let addr: Multiaddr = "/ip4/192.168.1.24/tcp/4001/p2p/12D3KooWScdNmkkfXipjs6PHwFZAzMxce6Qhgn7WtFDNorrq2rcH".parse().unwrap();

    let _ = swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse().unwrap());
    
    let key: PeerId = "QmV7PGs6w8dCcubt1oN35oVosAvhtCxErYrT3tkf36DuXB".parse().unwrap();

    //swarm.behaviour_mut().kad.add_address(&peer, addr);

    swarm.dial(addr).unwrap();
    swarm.behaviour_mut().kad.get_closest_peers(key);
    
    
    loop {
        select! {
            event = swarm.select_next_some() => {
                match event {
                    SwarmEvent::Behaviour(MyBehaviourEvent::Kademlia(KademliaEvent::OutboundQueryProgressed {
                         id, result, stats, step 
                        })) => {
                        match result {
                            QueryResult::GetClosestPeers(Ok(GetClosestPeersOk {key, peers})) => {
                                for peer in peers {
                                    println!(
                                        "Found {:?}",
                                        peer.to_string()
                                    )
                                }
                            }
                            QueryResult::GetClosestPeers(Err(GetClosestPeersError::Timeout {peers, ..})) => {
                                for peer in peers {
                                    println!(
                                        "Found {:?}",
                                        peer.to_string()
                                    )
                                }
                                println!("Timed Out");
                                break;
                            }
                            _ => {}
                        };
                    }
                    SwarmEvent::Behaviour(MyBehaviourEvent::Identify(identify::Event::Received {info, ..})) => {
                        println!("{:?}", info);
                    } 
                    
                    SwarmEvent::NewListenAddr {address, ..} => {
                        println!("Listening on {:?}", address);
                    }
                    SwarmEvent::ConnectionEstablished {peer_id, endpoint, ..} => {
                        println!("Connected to {} at {:?}", peer_id, endpoint)
                    }
                    SwarmEvent::Dialing(PeerId {..}) => {
                        println!("Attempting to Dial..");
                    }
                    SwarmEvent::OutgoingConnectionError {peer_id, error} => {
                        println!("Connection Error: {:?}", error);
                    }
                    
                    _ => (),
                }
            }
        }
        
    }

}
