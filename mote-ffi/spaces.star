"""
Spaces rules for managing mote-ffi
"""

load("//@star/prelude/rules/run.star", "run_add", "run_add_exec", "run_log_level_passthrough")

run_add_exec(
    "build_wasm",
    command = "wasm-pack",
    args = [
        "build",
        "--out-dir",
        "target/pkg-node",
        "--target",
        "bundler",
        "--features",
        "wasm_ffi",
    ],
    working_directory = ".",
)

run_add_exec(
    "build_cxx",
    command = "cargo",
    args = [
        "build",
        "--release",
        "--no-default-features",
        "--features",
        "cxx_ffi",
    ],
    working_directory = ".",
)

run_add_exec(
    "test_wasm",
    command = "wasm-pack",
    args = [
        "test",
        "--node",
        "--features",
        "wasm_ffi",
    ],
    working_directory = ".",
)

run_add_exec(
    "generate_python_types",
    command = "sh",
    args = [
        "-c",
        """
        set -e
        cargo build --quiet
        uv run scripts/generate_python_types.py
        """,
    ],
    working_directory = ".",
)

run_add_exec(
    "dev_setup",
    command = "sh",
    args = [
        "-c",
        """
        set -e
        uv sync --all-extras --cache-dir .uv_cache
        uv run --with "maturin>=1.0,<2.0" maturin develop --features python_ffi
        """,
    ],
    deps = [":generate_python_types"],
    working_directory = ".",
)

run_add_exec(
    "prod_setup",
    command = "uv",
    args = [
        "sync",
        "--all-extras",
        "--no-dev",
        "--cache-dir",
        ".uv_cache",
    ],
    working_directory = ".",
    log_level = run_log_level_passthrough(),
)

run_add_exec(
    "test",
    command = "sh",
    args = [
        "-c",
        """
        set -e
        cargo test --all-features
        wasm-pack test --node --features wasm_ffi
        uv run pytest
        """,
    ],
    deps = [":generate_python_types"],
    working_directory = ".",
    log_level = run_log_level_passthrough(),
)

run_add_exec(
    "format",
    command = "sh",
    args = [
        "-c",
        """
        set -e
        cargo fmt
        uv --preview format
        """,
    ],
    working_directory = ".",
    log_level = run_log_level_passthrough(),
)

run_add_exec(
    "lint",
    command = "sh",
    args = [
        "-c",
        """
        set -e
        echo "Linting mote-api"
        uv run --all-extras ty check
        cargo clippy --all-features -- -D warnings
        """,
    ],
    deps = [":generate_python_types"],
    working_directory = ".",
    log_level = run_log_level_passthrough(),
)

run_add(
    "build",
    deps = [":build_wasm"],
)

run_add_exec(
    "rerun_demo",
    command = "uv",
    args = [
        "run",
        "rerun-demo",
    ],
    deps = [":dev_setup"],
    working_directory = ".",
    log_level = run_log_level_passthrough(),
)

run_add_exec(
    "format_check",
    command = "sh",
    args = [
        "-c",
        """
        set -e
        cargo fmt --check
        uv --preview format --check
        """,
    ],
    working_directory = ".",
    log_level = run_log_level_passthrough(),
)

run_add_exec(
    "upgrade",
    command = "sh",
    args = [
        "-c",
        """
        set -e
        cargo update
        uv lock --upgrade
        """,
    ],
    working_directory = ".",
    log_level = run_log_level_passthrough(),
)

run_add(
    "ci",
    deps = [
        ":build",
        ":lint",
        ":test",
        ":format_check",
    ],
)

run_add_exec(
    "release_build_cxx_linux",
    command = "sh",
    args = [
        "-c",
        """
        set -e
        docker run --rm --platform linux/amd64 \
          -v "$(pwd)/..":/src -w /src/mote-ffi \
          quay.io/pypa/manylinux_2_28_x86_64 bash -c '
            set -e
            dnf install -y -q gcc-toolset-9-gcc-c++
            source /opt/rh/gcc-toolset-9/enable
            curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain stable
            source "$HOME/.cargo/env"
            cargo build --release --no-default-features --features cxx_ffi
          '
        """,
    ],
    working_directory = ".",
    log_level = run_log_level_passthrough(),
)

run_add_exec(
    "release_build_cxx_macos",
    command = "cargo",
    args = [
        "build",
        "--release",
        "--no-default-features",
        "--features",
        "cxx_ffi",
    ],
    env = {
        "MACOSX_DEPLOYMENT_TARGET": "12.0",
    },
    working_directory = ".",
    log_level = run_log_level_passthrough(),
)

run_add_exec(
    "release_package_cxx_macos",
    command = "sh",
    args = [
        "-c",
        """
        set -e
        VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/version = "\\(.*\\)"/\001/')
        mkdir -p dist dist/_pkg/include dist/_pkg/schemas
        cp target/release/libmote_ffi.a dist/_pkg/
        cp -r include/. dist/_pkg/include/
        cp schemas/*.json dist/_pkg/schemas/
        tar -czf dist/mote-ffi-cxx-macos-aarch64-v${VERSION}.tar.gz -C dist/_pkg .
        rm -rf dist/_pkg
        """,
    ],
    deps = [":release_build_cxx_macos"],
    working_directory = ".",
    log_level = run_log_level_passthrough(),
)
