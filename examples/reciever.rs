use std::collections::hash_map::DefaultHasher;
use std::env;
use std::time::Duration;
use async_std::task::block_on;
use ipfs_embed::PeerRecord;
use ipfs_embed::Record;
use libp2p::Multiaddr;
use libp2p::Swarm;
use libp2p::Transport;
use libp2p::core::multiaddr::Protocol;
use libp2p::core::transport::OrTransport;
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
use libp2p::gossipsub::ValidationMode;
use libp2p::identify;
use libp2p::kad::GetClosestPeersOk;
use libp2p::kad::GetClosestPeersError;
use libp2p::kad::GetProvidersOk;
use libp2p::kad::GetRecordOk;
use libp2p::kad::KademliaEvent;
use libp2p::kad::PutRecordOk;
use libp2p::kad::QueryResult;
use libp2p::noise;
use libp2p::relay::v2::{relay, client};
use libp2p::swarm::NetworkBehaviour;
use libp2p::swarm::SwarmEvent;
use libp2p::swarm::behaviour;
use libp2p_relay::v2::client::Client;
use libp2p_relay::v2::client::transport::ClientTransport;
use libp2p_tcp;
use libp2p::swarm::{keep_alive,};
use async_std;
use libp2p::yamux::YamuxConfig;
use libp2p_tcp::async_io;
use clap::{arg, command, value_parser, ArgAction, Command};
use void::Void;
use libp2p::dns::{DnsConfig};
use libp2p_dcutr as dcutr;
use std::hash::{Hash, Hasher};


const BOOTNODES: [&str; 2] = [
    "QmNnooDu7bfjPFoTZYxMNLWUQJyrVwtbZg5gBMjTezGAJN",
    "QmaCpDMGvV2BGHeYERUEnRQAwe3N8SzbUtfsmvsqQLuvuJ",
];


pub async fn build_transport(
    key_pair: identity::Keypair,
    relay_transport: ClientTransport
) -> Boxed<(PeerId, StreamMuxerBox)>{
    let transport = OrTransport::new(
        relay_transport,
        block_on(DnsConfig::system(async_io::Transport::new(
            libp2p_tcp::Config::default().nodelay(true).port_reuse(true),
        )))
        .unwrap(),
    )
    .upgrade(Version::V1)
    .authenticate(
        noise::NoiseAuthenticated::xx(&key_pair)
            .expect("Signing libp2p-noise static DH keypair failed."),
    )
    .multiplex(YamuxConfig::default())
    .boxed();
    transport
}



fn main() {
    block_on(async_main());
}


#[derive(NetworkBehaviour)]
#[behaviour(out_event="MyBehaviourEvent")]
struct MyBehaviour {
        keep_alive: keep_alive::Behaviour,
        //kad: Kademlia<MemoryStore>,
        identify: identify::Behaviour,
        //autonat: autonat::Behaviour,
        relay: Client,
        dcutr: dcutr::behaviour::Behaviour,
        gossipsub: Gossipsub,

}

#[allow(clippy::large_enum_variant)]
enum MyBehaviourEvent {
    Kademlia(KademliaEvent),
    Identify(identify::Event),
    Relay(client::Event),
    //Autonat(autonat::Event),
    Dcutr(dcutr::behaviour::Event),
    Gossipsub(GossipsubEvent),
}

impl From<GossipsubEvent> for MyBehaviourEvent {
    fn from(event: GossipsubEvent) -> Self {
        MyBehaviourEvent::Gossipsub(event)
    }
}

impl From<dcutr::behaviour::Event> for MyBehaviourEvent {
    fn from(event: dcutr::behaviour::Event) -> Self {
        MyBehaviourEvent::Dcutr(event)
    }
}

impl From<client::Event> for MyBehaviourEvent {
    fn from(event: client::Event) -> Self {
        MyBehaviourEvent::Relay(event)
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




async fn async_main() {


    let matches = command!().
    args(
        [arg!(
            --relay <str> "relay"
        ),
        arg!(--secret <str> "Secret Used for Auth")
        
        ]
    ).get_matches();
    

    let local_key = identity::Keypair::generate_ed25519();
    let local_peer_id = PeerId::from(local_key.public());
    println!("{:?}", local_peer_id);

    let (relay_transport, client) = Client::new_transport_and_behaviour(local_peer_id);

    let transport = build_transport(local_key.clone(), relay_transport).await;

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

    let behaviour = MyBehaviour {
        keep_alive: keep_alive::Behaviour,
        relay: client,
        identify: identify::Behaviour::new(identify::Config::new(
            "/TODO/0.0.1".to_string(),
            local_key.public(),
        )),
        dcutr: dcutr::behaviour::Behaviour::new(),
        gossipsub
    };

    

    let mut swarm = {
        Swarm::with_async_std_executor(
            transport,
            behaviour,
            local_peer_id
        )
    };

    
    let addr: Multiaddr = matches.get_one::<String>("relay").unwrap().parse().unwrap();


    // Listen on the given relay address
    let _ = swarm.listen_on(addr.clone().with(Protocol::P2pCircuit)).unwrap();
     

    // Dial Relay address to kick off identify TODO: Wait for identify 
    swarm.dial(addr.clone()).unwrap();
    

    loop {
        select! {
            event = swarm.select_next_some() => {
                match event {
                    SwarmEvent::Behaviour(MyBehaviourEvent::Identify(identify::Event::Received {info, ..})) => {
                        println!("{:?}", info);
                    } 
                    
                    SwarmEvent::NewListenAddr {address, ..} => {
                        println!(
                            "Peer that act as Client Listen can access on: `{:?}'",
                            address
                        );
                    }
                    SwarmEvent::ConnectionEstablished {peer_id, endpoint, ..} => {
                        println!("Connected to {} at {:?}", peer_id, endpoint)
                    }
                    SwarmEvent::OutgoingConnectionError {error, ..} => {
                        println!("Connection Error: {:?}", error);
                    }
                    SwarmEvent::Behaviour(MyBehaviourEvent::Gossipsub(GossipsubEvent::Message {
                        propagation_source: peer_id,
                        message_id: id,
                        message,
                    })) => {
                        println!(
                            "Got message: {} with id: {} from peer: {:?}",
                            String::from_utf8_lossy(&message.data),
                            id,
                            peer_id
                        )
                    }
                    _ => (),
                }
            }
        }
        
    }

}
