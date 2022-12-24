

# Usage 

Start the server
```cargo run --example server -- --relay "/ip4/0.0.0.0/tcp/0"```

Start the Client Reciever
```cargo run --example reciever -- --relay "/ip4/RELAYIP/tcp/5773/p2p/12D3KooWL4Toh5Z4fpWgDN12pRdn9pVAhBBTqeGELUyBUYBg6ziW"```


start the Sender Client
```cargo run --example sender -- --addr "/ip4/RELAYIP/tcp/5773/p2p/12D3KooWL4Toh5Z4fpWgDN12pRdn9pVAhBBTqeGELUyBUYBg6ziW/p2p-circuit/p2p/12D3KooWRkT4VCQLQdvjNR7MpTkoPQPVWphHzW9qZ4E7fswB8ow8"```
