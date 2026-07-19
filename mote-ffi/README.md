# mote-ffi

FFI glue exposing [mote-api](../mote-api)'s message protocol to Python, WASM, and C++.

## Python: `mote_link`

The Python package. See [`mote_link/README.md`](mote_link/README.md) for
installation and usage.

```bash
task dev-setup   # builds the extension and regenerates types
task test        # cargo test + wasm-pack test + pytest
```

## C++

```bash
task build-cxx
```

Builds a static library (`target/release/libmote_ffi.a`) and cxx-generated headers
(`include/mote-ffi/src/mote_cxx.rs.h`, `include/rust/cxx.h`). `#include "mote-ffi/src/mote_cxx.rs.h"`
to get the `mote::MoteLink` class, `mote::SendResult`/`mote::ReceiveResult`, and
`mote::MoteLinkErrorCode`. #

# WASM / TypeScript

```bash
task build-wasm
```

Builds a `wasm-pack` package (`target/pkg-node`) exposing a `Link` class. See
`mote-configuration`'s `src/lib/link.ts` for an example consumer.
