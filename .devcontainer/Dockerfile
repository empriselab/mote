# Devcontainer image for mote.

FROM mcr.microsoft.com/devcontainers/base:ubuntu-24.04

# libudev-dev/cmake/pkg-config are probe-rs build dependencies;
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

# Node.js
RUN curl -fsSL https://deb.nodesource.com/setup_lts.x | bash - \
    && apt-get install -y --no-install-recommends nodejs \
    && rm -rf /var/lib/apt/lists/*

USER vscode
ENV CARGO_HOME=/home/vscode/.cargo \
    RUSTUP_HOME=/home/vscode/.rustup \
    PATH=/home/vscode/.cargo/bin:/home/vscode/.local/bin:$PATH

# Pre-create volumes so they are owned by the container user
RUN mkdir -p $CARGO_HOME/registry $CARGO_HOME/git

# Rust
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | \
        sh -s -- -y --profile default --default-toolchain stable \
    && rustup target add wasm32-unknown-unknown

# UV
RUN curl -LsSf https://astral.sh/uv/install.sh | sh

# Go Task
RUN sh -c "$(curl --location https://taskfile.dev/install.sh)" -- -d -b /home/vscode/.local/bin

# WASM Pack
RUN cargo install wasm-pack --version 0.15.0 --locked

# MdBook
RUN cargo install mdbook --locked

# probe-rs
RUN curl --proto '=https' --tlsv1.2 -LsSf \
        https://github.com/probe-rs/probe-rs/releases/latest/download/probe-rs-tools-installer.sh | sh

# picotool
RUN set -e; \
    url=$(curl -sfL https://api.github.com/repos/raspberrypi/pico-sdk-tools/releases/latest \
        | grep -o '"browser_download_url": *"[^"]*picotool-[^"]*-x86_64-lin\.tar\.gz"' \
        | head -1 | sed 's/.*"\(https[^"]*\)"/\1/'); \
    curl -sfL -o /tmp/picotool.tar.gz "$url"; \
    tar -xzf /tmp/picotool.tar.gz -C /tmp; \
    mkdir -p /home/vscode/.local/bin; \
    install -m 755 "$(find /tmp -maxdepth 2 -name picotool -type f)" /home/vscode/.local/bin/picotool; \
    rm -rf /tmp/picotool.tar.gz /tmp/picotool*

# codespell
RUN uv tool install codespell

# vale
RUN curl -sfL https://github.com/errata-ai/vale/releases/download/v3.15.1/vale_3.15.1_Linux_64-bit.tar.gz \
        | tar -xz -C /home/vscode/.local/bin vale

WORKDIR /workspaces/mote

# Pre-install mote-firmware's toolchain
COPY --chown=vscode:vscode mote-firmware/rust-toolchain.toml mote-firmware/rust-toolchain.toml
RUN cd mote-firmware && rustup show
