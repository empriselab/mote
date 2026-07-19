# mote-ffi

FFI glue exposing [mote-api](../mote-api)'s message protocol to Python, WASM, and C.
One Rust crate (`mote-ffi`), three binding surfaces built from the same source via
Cargo features (`python_ffi`, `wasm_ffi`, `c_ffi`).

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

## C

```bash
task build-c
```

Builds a static library (`target/release/libmote_ffi.a`) and header
(`include/mote_link.h`). See the header's return-code convention comment before
calling into it — the four functions don't all use the same convention for what a
given return value means.

## WASM / TypeScript

```bash
task build-wasm
```

Builds a `wasm-pack` package (`target/pkg-node`) exposing a `Link` class. See
`mote-configuration`'s `src/lib/link.ts` for an example consumer, and
`mote-configuration/scripts/generate-types.mjs` for how `mote_api_types.ts` is
generated from `schemas/*.json`.

The WASM binding wraps mote-api's serial-MTU `MoteConfigLink`, not the UDP-MTU
`MoteLink` that Python and C use — it's built for `mote-configuration`'s USB-serial
wifi-setup flow, not the network link.
