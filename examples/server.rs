use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::Duration;
use std::env;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use async_std::task::block_on;
use ipfs_embed::DefaultParams;
use ipfs_embed::Key;
use ipfs_embed::PeerRecord;
use ipfs_embed::Record;
use libipld::Cid;
use libipld::IpldCodec;
use libp2p::{Multiaddr, mdns};
use libp2p::Swarm;
use libp2p::Transport;
use libp2p::futures::StreamExt;
use libp2p::futures::select;
use libp2p::core::identity;
use libp2p::core::PeerId;
use libp2p::core::muxing::StreamMuxerBox;
use libp2p::core::transport::Boxed;
use libp2p::core::upgrade::Version;
use libp2p::gossipsub;
use libp2p::gossipsub::Gossipsub;
use libp2p::gossipsub::GossipsubEvent;
use libp2p::gossipsub::GossipsubMessage;
use libp2p::gossipsub::MessageAuthenticity;
use libp2p::gossipsub::MessageId;
use libp2p::gossipsub::Topic;
use libp2p::gossipsub::ValidationMode;
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
use libp2p::kad::KademliaConfig;
use libp2p::kad::KademliaEvent;
use libp2p::kad::PutRecordOk;
use libp2p::kad::QueryResult;
use libp2p::kad::store::MemoryStore;
use libp2p::noise;
use libp2p::ping;
use libp2p::pnet::PnetConfig;
use libp2p::relay::v2::relay;
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
        identify: identify::Behaviour,
        //autonat: autonat::Behaviour,
        relay: relay::Relay,
        gossipsub: Gossipsub,
        mdns: mdns::async_io::Behaviour,

}

#[allow(clippy::large_enum_variant)]
enum MyBehaviourEvent {
    Kademlia(KademliaEvent),
    Identify(identify::Event),
    Relay(relay::Event),
    Gossipsub(GossipsubEvent),
    Mdns(mdns::Event)
    //Autonat(autonat::Event),
}

impl From<mdns::Event> for MyBehaviourEvent {
    fn from(value: mdns::Event) -> Self {
        MyBehaviourEvent::Mdns(value)
    }
}

impl From<GossipsubEvent> for MyBehaviourEvent {
    fn from(event: GossipsubEvent) -> Self {
        MyBehaviourEvent::Gossipsub(event)
    }
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

impl From<relay::Event> for MyBehaviourEvent {
    fn from(event: relay::Event) -> Self {
        MyBehaviourEvent::Relay(event)
    }
}



async fn async_main() {


    let matches = command!().
    arg(
        arg!(
            --relay <str> "Argument to enable the DHT"
        )
    ).get_matches();

    let local_key = identity::Keypair::generate_ed25519();
    let local_peer_id = PeerId::from(local_key.clone().public());
    println!("{:?}", local_peer_id);

    let transport = build_transport(local_key.clone()).await;

    let message_id_fn = |message: &GossipsubMessage| {
        let mut s = DefaultHasher::new();
        message.data.hash(&mut s);
        MessageId::from(s.finish().to_string())
    };

    // Set a custom gossipsub configuration
    let gossipsub_config = gossipsub::GossipsubConfigBuilder::default()
        .heartbeat_interval(Duration::from_secs(10)) // This is set to aid debugging by not cluttering the log space
        .validation_mode(ValidationMode::Strict) // This sets the kind of message validation. The default is Strict (enforce message signing)
        .message_id_fn(message_id_fn) // content-address messages. No two messages of the same content will be propagated.
        .build()
        .expect("Valid config");

    // build a gossipsub network behaviour
    let mut gossipsub = Gossipsub::new(MessageAuthenticity::Signed(local_key.clone()), gossipsub_config)
        .expect("Correct configuration");

    // Create a Gossipsub topic
    let topic = gossipsub::IdentTopic::new("test-net");
    gossipsub.subscribe(&topic).unwrap();
   
    let store = MemoryStore::new(local_peer_id);
    let mut cfg = KademliaConfig::default();
    cfg.set_query_timeout(Duration::from_secs(1 * 60));
    let kademlia = Kademlia::with_config(local_peer_id, store, cfg);

    let behaviour = MyBehaviour {
        keep_alive: keep_alive::Behaviour,
        kad:  kademlia,
        identify: identify::Behaviour::new(identify::Config::new("/ipfs/id/1.0.0".to_string(), local_key.clone().public())),
        relay: relay::Relay::new(local_peer_id, relay::Config::default()),
        gossipsub,
        mdns: mdns::Behaviour::new(mdns::Config::default()).unwrap(),
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
    
    let addr: Multiaddr = matches.get_one::<String>("relay").unwrap().parse().unwrap();

    let _ = swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse().unwrap());
    
    let key: Key = Key::new(&"12D3KooWScdNmkkfXipjs6PHwFZAzMxce6Qhgn7WtFDNorrq2rcH".as_bytes());

    //swarm.behaviour_mut().kad.add_address(&peer, addr);

    swarm.dial(addr).unwrap();
    //swarm.behaviour_mut().kad.get_closest_peers(peer);
    //swarm.behaviour_mut().kad.get_providers(key);
    
    
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
                                        peer.to_string(),
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
                            QueryResult::GetProviders(Ok(GetProvidersOk::FoundProviders {key, providers, ..})) => {
                                for peer in providers {
                                    println!(
                                        "Peer {peer:?} provides key {:?}",
                                        std::str::from_utf8(key.as_ref()).unwrap()
                                    );
                                } 
                            }
                            QueryResult::GetProviders(Ok(_)) => {}
                            QueryResult::GetProviders(Err(err)) => {
                                eprintln!("Failed to get providers: {err:?}");
                            }
                            QueryResult::GetRecord(Ok(
                                GetRecordOk::FoundRecord(PeerRecord {
                                    record: Record { key, value, .. },
                                    ..
                                })
                            )) => {
                                println!(
                                    "Got record {:?} {:?}",
                                    std::str::from_utf8(key.as_ref()).unwrap(),
                                    std::str::from_utf8(&value).unwrap(),
                                );
                            }
                            QueryResult::GetRecord(Ok(_)) => {}
                            QueryResult::GetRecord(Err(err)) => {
                                eprintln!("Failed to get record: {err:?}");
                            }
                            _ => {}
                        };
                    }
                    //SwarmEvent::Behaviour(MyBehaviourEvent::Identify(identify::Event::Received {info, ..})) => {
                    //    println!("{:?}", info);
                    //} 
                    SwarmEvent::Behaviour(MyBehaviourEvent::Mdns(mdns::Event::Discovered(peers))) => {
                        for (peer, addr) in peers {
                            print!("{:?}", peer);
                            //logger::send_event(LogEntry { msg: format!("{}, {:?}", peer, addr) ,tag: "Discovered".to_string() });
                        }
                    }
                    SwarmEvent::NewListenAddr {address, ..} => {
                        println!(
                            "Peer that act as Relay can access on: `{}/p2p/{}/p2p-circuit`",
                            address, local_peer_id
                        );
                    }
                    SwarmEvent::ConnectionEstablished {peer_id, endpoint, ..} => {
                        println!("Connected to {} at {:?}", peer_id, endpoint)
                    }
                    //SwarmEvent::Dialing(PeerId {..}) => {
                    //    println!("Attempting to Dial..");
                    //}
                    //SwarmEvent::OutgoingConnectionError {peer_id, error} => {
                    //    println!("Connection Error: {:?}", error);
                    //}
                    
                    _ => (),
                }
            }
        }
        
    }

}
