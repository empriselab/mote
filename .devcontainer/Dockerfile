# Devcontainer image for mote-core.
#
# Covers mote-api, mote-ffi (rust/python/wasm/cxx), mote-configuration
# (node/svelte), mote-book (mdbook), and a compile-capable mote-firmware
# (nightly toolchain + thumbv8m target). It does NOT cover:
#   - mote-hardware (KiCAD) — CI builds that in a separate
#     ghcr.io/inti-cmnb/kicad9_auto container, not a general dev workflow.
#   - Docker-in-Docker for mote-ffi's release-build-cxx-linux[-aarch64] tasks
#     (they shell out to `docker run` against manylinux images) — release-only.
#
# Firmware *flashing* (task firmware:provision/deploy, via probe-rs) needs raw
# USB access to the debug probe. That only works reliably with Docker on a
# native Linux host — see docker-compose.yml for the commented-out device
# passthrough, and mote-book/src/advanced/contributing/dev_env.md.
FROM mcr.microsoft.com/devcontainers/base:ubuntu-24.04

# python3 is required as a bare `python3` on PATH: mote-ffi/Taskfile.yml's
# generate-python-types task runs `python3 scripts/generate_python_types.py`
# directly (not via `uv run`), since that script is stdlib-only.
# libudev-dev/cmake/pkg-config are probe-rs's documented build dependencies;
# libusb-1.0-0 is picotool's runtime dependency for talking to the RP2350 over USB.
RUN apt-get update && apt-get install -y --no-install-recommends \
        build-essential \
        pkg-config \
        python3 \
        python3-dev \
        python3-pip \
        pipx \
        jq \
        libudev-dev \
        libusb-1.0-0 \
        cmake \
    && rm -rf /var/lib/apt/lists/*

# Node.js (mote-configuration's Svelte/Vite/TS build), matching CI's lts/*.
RUN curl -fsSL https://deb.nodesource.com/setup_lts.x | bash - \
    && apt-get install -y --no-install-recommends nodejs \
    && rm -rf /var/lib/apt/lists/*

USER vscode
ENV CARGO_HOME=/home/vscode/.cargo \
    RUSTUP_HOME=/home/vscode/.rustup \
    PATH=/home/vscode/.cargo/bin:/home/vscode/.local/bin:$PATH

# Pre-create $CARGO_HOME/{registry,git} owned by vscode so that when
# docker-compose's named volumes are first mounted over these paths, Docker's
# copy-up preserves vscode ownership instead of defaulting to root (registry
# gets populated below by `cargo install`, but git is otherwise never created
# in the image — mote-firmware is the only crate with a git dependency, and
# it's only fetched at runtime via rust-toolchain.toml's lazy nightly install).
RUN mkdir -p $CARGO_HOME/registry $CARGO_HOME/git

# Rust (stable) — mote-firmware's own nightly-2025-10-18 pin (rust-toolchain.toml)
# is fetched lazily by rustup the first time cargo runs in that directory; see
# devcontainer.json's postCreateCommand, which pre-warms it.
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | \
        sh -s -- -y --profile default --default-toolchain stable \
    && rustup target add wasm32-unknown-unknown

# uv (Python package/project manager, used by mote-ffi)
RUN curl -LsSf https://astral.sh/uv/install.sh | sh

# go-task (task runner used to orchestrate every crate/package)
RUN sh -c "$(curl --location https://taskfile.dev/install.sh)" -- -d -b /home/vscode/.local/bin

# wasm-pack, pinned to match .github/workflows/build-test.yaml's
# jetli/wasm-pack-action@v0.4.0 (version v0.15.0).
RUN cargo install wasm-pack --version 0.15.0 --locked

# mdbook (documentation generator for mote-book). CI installs "latest" via
# peaceiris/actions-mdbook, so leave this unpinned to match.
RUN cargo install mdbook --locked

# probe-rs (flash/debug embedded targets over SWD — needs USB passthrough, see
# docker-compose.yml).
RUN curl --proto '=https' --tlsv1.2 -LsSf \
        https://github.com/probe-rs/probe-rs/releases/latest/download/probe-rs-tools-installer.sh | sh

# picotool (for `task firmware:uf2`), mirroring
# .github/workflows/release-firmware.yaml's install step but via the public
# GitHub Releases API instead of `gh`/a token. x86_64 Linux only — arm64
# devcontainer hosts (e.g. Apple Silicon) will need a different asset pattern.
RUN set -e; \
    url=$(curl -sfL https://api.github.com/repos/raspberrypi/pico-sdk-tools/releases/latest \
        | grep -o '"browser_download_url": *"[^"]*picotool-[^"]*-x86_64-lin\.tar\.gz"' \
        | head -1 | sed 's/.*"\(https[^"]*\)"/\1/'); \
    curl -sfL -o /tmp/picotool.tar.gz "$url"; \
    tar -xzf /tmp/picotool.tar.gz -C /tmp; \
    mkdir -p /home/vscode/.local/bin; \
    install -m 755 "$(find /tmp -maxdepth 2 -name picotool -type f)" /home/vscode/.local/bin/picotool; \
    rm -rf /tmp/picotool.tar.gz /tmp/picotool*

# codespell (task spell). Ubuntu 24.04's system pip is externally-managed, so
# install via pipx instead of `pip install --user`.
RUN pipx install codespell && pipx ensurepath

# Vale (task grammar), pinned to match
# .github/workflows/grammar-spelling.yaml's version (v3.15.1).
RUN curl -sfL https://github.com/errata-ai/vale/releases/download/v3.15.1/vale_3.15.1_Linux_64-bit.tar.gz \
        | tar -xz -C /home/vscode/.local/bin vale

WORKDIR /workspaces/mote-core
