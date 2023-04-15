use std::{net::SocketAddr, thread, sync::Arc};

//use async_std::{stream::StreamExt, task::block_on};
use futures::executor::block_on;
use quinn::{ Endpoint, ServerConfig};
use anyhow::Result;

#[tokio::main]
async fn main() {
    block_on(async_main());
}

async fn async_main() {
    let (server, _certs) = make_server_endpoint("0.0.0.0:0".parse().unwrap()).unwrap();
    println!("Listening on: {:?}", server.local_addr().unwrap());
    loop {
        while let Some(con) = server.accept().await {}
    }
}

fn make_server_endpoint(bind_addr: SocketAddr) -> Result<(Endpoint, Vec<u8>)> {
    let (server_config, server_cert) = configure_server()?;
    let endpoint = Endpoint::server(server_config, bind_addr)?;
    Ok((endpoint, server_cert))
}

fn configure_server() -> Result<(ServerConfig, Vec<u8>)> {
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let cert_der = cert.serialize_der().unwrap();
    let priv_key = cert.serialize_private_key_der();
    let priv_key = rustls::PrivateKey(priv_key);
    let cert_chain = vec![rustls::Certificate(cert_der.clone())];

    let mut server_config = ServerConfig::with_single_cert(cert_chain, priv_key).unwrap();
    let transport_config = Arc::get_mut(&mut server_config.transport).unwrap();
    transport_config.max_concurrent_uni_streams(0_u8.into());
    #[cfg(any(windows, os = "linux"))]
    transport_config.mtu_discovery_config(Some(quinn::MtuDiscoveryConfig::default()));

    Ok((server_config, cert_der))
}
