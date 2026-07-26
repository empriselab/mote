# Development Environment

## DevContainer

A [DevContainer](https://containers.dev/) is provided in [`.devcontainer/`](https://github.com/empriselab/mote/tree/main/.devcontainer).
Open the repo in VS Code with the [Dev Containers extension](https://marketplace.visualstudio.com/items?itemName=ms-vscode-remote.remote-containers) and choose "Reopen in Container"

> [!TIP]
> `probe-rs` firmware deployments will not work on MacOS or Windows due to USB passthrough limitations with Docker Desktop.
>
> On native Linux, USB passthrough can be enabled by uncommenting the relevant lines in [`.devcontainer/docker-compose.yml`](https://github.com/empriselab/mote/tree/main/.devcontainer/docker-compose.yml).
>
> On MacOS and Windows, the firmware binary can be built inside of the dev container with `task firmware:build` and subsequently deployed using a local `probe-rs` install.

## Local Install

Linux and MacOS are officially supported development platforms. Developing on Windows should be possible with some tinkering. If you would like to improve Windows support, please open a [pull request](https://github.com/empriselab/mote/pulls).

Install the following tools:

| Tool | Purpose | Installation Method | 
|---|---|---|
| rust | cargo (package manager), rustc (compiler), rust-analyzer (language server) | [https://rustup.rs/](https://rustup.rs/) |
| go-task | task runner | [https://taskfile.dev/installation/](https://taskfile.dev/installation/) |
| uv | python package and project manager | [https://docs.astral.sh/uv/getting-started/installation/](https://docs.astral.sh/uv/getting-started/installation/) |
| node | build / run configuration webpage via typescript, vite, and svelte | [https://nodejs.org/en/download](https://nodejs.org/en/download) |
| probe-rs | flash and debug embedded systems | [https://probe.rs/docs/getting-started/installation/](https://probe.rs/docs/getting-started/installation/) |
| wasm-pack | used for TS - rust interop | `cargo install wasm-pack` |
| mdBook | documentation generator | [https://rust-lang.github.io/mdBook/guide/installation.html](https://rust-lang.github.io/mdBook/guide/installation.html) |
