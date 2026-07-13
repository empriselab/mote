# mote-api

Message and wire-protocol definitions for the Mote firmware <--> host driver
link. `no_std` + `alloc`.

## Compatibility model

Two layers, matching how the crate handles growth:

- **Source level.** `Message` enums (and a few result types like `BITResult`)
  are `#[non_exhaustive]`. Code that matches on them must handle unknown
  variants — new message types can be added without a semver-major bump.
- **Wire level.** Messages are encoded with [`postcard`](https://docs.rs/postcard),
  which varint-encodes enum discriminants and collection lengths. Appending a
  new field or a new enum variant at the end doesn't change how existing data
  decodes: old decoders reject an out-of-range variant instead of misreading
  bits, and new decoders can still read old messages if only additions were
  made. This is what makes `#[non_exhaustive]` meaningful at the wire level,
  not just the source level.

  Anything that isn't purely additive (removing or reordering fields, a major
  version bump) isn't safe to decode across versions. Every frame carries a
  6-byte header with the sender's crate version; on mismatch, `poll_receive`
  returns `Error::VersionMismatch` instead of attempting to decode — a hard
  reject, not a partial/best-effort decode.

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
