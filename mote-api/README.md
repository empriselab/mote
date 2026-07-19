# mote-api

sans-io message and wire-protocol definitions for the Mote firmware <--> host driver link. 
`no_std` + `alloc`.

## Usage

```rust
use mote_api::{HostLink, MoteLink};
use mote_api::messages::{host_to_mote, mote_to_host};

let mut host = HostLink::new();
let mut mote = MoteLink::new();

host.send(host_to_mote::Message::Ping).unwrap();
while let Some(packet) = host.poll_transmit() {
    mote.handle_receive(&packet);
}
assert_eq!(mote.poll_receive().unwrap(), Some(host_to_mote::Message::Ping));
```

See [docs.rs/mote-api](https://docs.rs/mote-api) for the full API.
