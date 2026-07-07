# Building, Testing, Running, and Releasing

## Setup

Follow the instructions in [Development Environment](advanced/contributing/dev_env.html) to set up the required tools.

Make a clone of the repository: 
```bash
git clone git@github.com:empriselab/mote-core.git
```
or 
```bash
git clone https://github.com/empriselab/mote-core.git
```

## Build

Compile source and generate executable artifacts.

```bash
# Build Mote's firmware
just firmware::build 
# Build the configuration webpage
just config::build 
# Build the book
just book::build 
```

## Test

Run unit tests.

```bash
# Run api test cases
just api::test
# Run ffi test cases
just ffi::test
# Test code examples in the book
just book::test
```

## Run

Run a target.

Running firmware requires connecting to Mote using [a SWD debug probe](https://www.raspberrypi.com/documentation/microcontrollers/debug-probe.html).

```bash
# Deploy firmware to Mote (first time doing so)
just firmware::provision
# Deploy firmware to Mote (any time after)
just firmware::deploy
# Serve the configuration page
just config::run-dev
# Serve / open the book
just book::open
```

## Release

Release artifacts are built and uploaded automatically via continuous integration.

* `mote-firmware`
    * Released on any tag to `mote-core` matching the pattern `mote-firmware-vX.X.X`, where `vX.X.X` matches the semantic version of the `mote-firmware` crate.
    * Automated via [this GitHub Action](https://github.com/empriselab/mote-core/blob/main/.github/workflows/release-firmware.yaml).
* `mote-c`
    * Released on any tag to `mote-core` matching the pattern `mote-c-vX.X.X`, where `vX.X.X` matches the semantic version of the `mote-ffi` crate.
    * The static library, header, and schemas are attached to the release via [this GitHub Action](https://github.com/empriselab/mote-core/blob/main/.github/workflows/release-c.yaml).
* `mote-python`
    * Released on any tag to `mote-core` matching the pattern `mote-python-vX.X.X`, where `vX.X.X` matches the version in `mote-ffi/pyproject.toml`.
    * Cross-platform wheels are built and published to [PyPI](https://pypi.org/project/mote_link/) via [this GitHub Action](https://github.com/empriselab/mote-core/blob/main/.github/workflows/release-python.yaml).
* `mote-rust` (the `mote-api` crate)
    * Released on any tag to `mote-core` matching the pattern `mote-rust-vX.X.X`, where `vX.X.X` matches the semantic version of the `mote-api` crate.
    * The crate is published to [crates.io](https://crates.io/crates/mote-api) via [this GitHub Action](https://github.com/empriselab/mote-core/blob/main/.github/workflows/release-rust.yaml).
* `mote-configuration`
    * Released on any tag to `mote-core` matching the pattern `mote-configuration-vX.X.X`, where `vX.X.X` matches the version in `mote-configuration/package.json`.
    * The built site is attached to the release via [this GitHub Action](https://github.com/empriselab/mote-core/blob/main/.github/workflows/release-configuration.yaml).
* `mote-book`
    * Built and deployed to GitHub pages on every commit to main via the [GitHub Action](https://github.com/empriselab/mote-core/blob/main/.github/workflows/deploy.yaml).
