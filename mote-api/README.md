# mote-api

sans-io message and wire-protocol definitions for the Mote firmware <--> host driver link. 
`no_std` + `alloc`.

## Usage

```rust
use mote_api::{HostLink, MoteLink};
use mote_api::messages::host_to_mote;

let mut host_side = MoteLink::new();
let mut mote_side = HostLink::new();

host_side.send(host_to_mote::Message::Ping).unwrap();
while let Some(packet) = host_side.poll_transmit() {
    mote_side.handle_receive(&packet);
}
assert_eq!(mote_side.poll_receive().unwrap(), Some(host_to_mote::Message::Ping));
```

See [docs.rs/mote-api](https://docs.rs/mote-api) for the full API.
