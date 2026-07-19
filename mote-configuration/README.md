# mote-configuration

Svelte + Vite web app for configuring a Mote over Web Serial. Talks to the
robot using [mote-ffi](../mote-ffi)'s WASM bindings to set up Wi-Fi networks,
identification, and view live diagnostics.

## Usage

```bash
task run-dev   # build mote-ffi's wasm target, then start the dev server
```

## Testing

```bash
task test   # vitest
task ci     # svelte-check + vitest
```

## Release

```bash
task release-package
```

Builds and packages the app into `mote-configuration-v<version>.tar.gz`,
versioned from `package.json`.
