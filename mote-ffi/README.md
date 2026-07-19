# mote-ffi

FFI glue exposing [mote-api](../mote-api)'s message protocol to Python, WASM, and C++.
One Rust crate (`mote-ffi`), three binding surfaces built from the same source via
Cargo features (`python_ffi`, `wasm_ffi`, `cxx_ffi`).

At build time, `build.rs` generates JSON Schemas for both message directions into
`schemas/*.json` (from mote-api's types via [`schemars`](https://docs.rs/schemars)).
Both the Python bindings (`mote_link/_generated.py`) and the TypeScript bindings used
by `mote-configuration` (`mote_api_types.ts`) are generated from these schemas, so
message types can't drift out of sync with mote-api's Rust definitions.

## Python: `mote_link`

The Python package. See [`mote_link/README.md`](mote_link/README.md) for
installation and usage.

```bash
task dev-setup   # builds the extension and regenerates mote_link/_generated.py
task test        # cargo test + wasm-pack test + pytest
```

## C++

```bash
task build-cxx
```

Builds a static library (`target/release/libmote_ffi.a`) and cxx-generated headers
(`include/mote-ffi/src/cpp.rs.h`, `include/rust/cxx.h`). `#include "mote-ffi/src/cpp.rs.h"`
to get the `mote::MoteLink` class, `mote::SendResult`/`mote::ReceiveResult`, and
`mote::MoteLinkErrorCode`. See the doc comments in `src/cpp.rs` for the bridge's error
convention — every fallible call returns a result struct pairing a `MoteLinkErrorCode`
with either an `error_message` or the requested data; `MoteLinkErrorCode::None` means
success.

Note: passing a non-UTF-8 C++ string into `send()` throws `std::invalid_argument` at the
call site (part of `cxx`'s `rust::Str` binding), rather than returning an error code.

## WASM / TypeScript

```bash
task build-wasm
```

Builds a `wasm-pack` package (`target/pkg-node`) exposing a `Link` class. See
`mote-configuration`'s `src/lib/link.ts` for an example consumer, and
`mote-configuration/scripts/generate-types.mjs` for how `mote_api_types.ts` is
generated from `schemas/*.json`.

The WASM binding wraps mote-api's serial-MTU `MoteConfigLink`, not the UDP-MTU
`MoteLink` that Python and C++ use — it's built for `mote-configuration`'s USB-serial
wifi-setup flow, not the network link.
