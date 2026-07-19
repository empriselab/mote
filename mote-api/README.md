# mote-api

Message and wire-protocol definitions for the Mote firmware <--> host driver
link. `no_std` + `alloc`.

## Compatibility model

Two layers, matching how the crate handles growth:

- **Source level.** The `Message` enums, and the aggregate types that are
  expected to grow as new sensors or state fields land (`State`,
  `BitCollection`), are `#[non_exhaustive]`. Code that matches on them must
  handle unknown variants — new message types or state fields can be added
  without a semver-major bump.

  `BitResult` and `ConnectionError` are **not** `#[non_exhaustive]`: each is a
  closed, fully-enumerated set of outcomes, not expected to grow.

  Leaf payload structs (`Point`, `WheelJointState`, `SetUid`, and similar) are
  also **not** `#[non_exhaustive]`: every field is `pub` and they're built via
  struct-literal syntax, so there's no constructor to keep stable instead.
  Adding a field to one of these is a semver-major change, same as any other
  breaking change to a public struct.
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
