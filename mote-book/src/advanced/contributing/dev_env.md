# Development Environment

## DevContainer

DevContainer support coming in [#13](https://github.com/empriselab/mote/issues/13). DevContainers do not support USB passthrough (outside of Linux), so you'll need to follow the local install directions if you wish to develop firmware.

## Local Install

Linux and MacOS are officially supported development platforms. Developing on Windows should be possible with some tinkering. If you would like to improve Windows support, please open a [pull request](https://github.com/empriselab/mote/pulls).

Install the following tools:

| Tool | Purpose | Installation Method | 
|---|---|---|
| rust | cargo (package manager), rustc (compiler), rust-analyzer (language server) | [https://rustup.rs/](https://rustup.rs/) |
| just | task runner | [https://just.systems/man/en/introduction.html](https://just.systems/man/en/introduction.html) |
| uv | python package and project manager | [https://docs.astral.sh/uv/getting-started/installation/](https://docs.astral.sh/uv/getting-started/installation/) |
| node | build / run configuration webpage via typescript, vite, and svelte | [https://nodejs.org/en/download](https://nodejs.org/en/download) |
| probe-rs | flash and debug embedded systems | [https://probe.rs/docs/getting-started/installation/](https://probe.rs/docs/getting-started/installation/) |
| wasm-pack | used for TS - rust interop | `cargo install wasm-pack` |
| mdBook | documentation generator | [https://rust-lang.github.io/mdBook/guide/installation.html](https://rust-lang.github.io/mdBook/guide/installation.html) |
