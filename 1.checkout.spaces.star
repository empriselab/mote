"""
Checkout tools required for building and running the mote system
"""

load("//@star/packages/star/node.star", "node_add")
load("//@star/packages/star/python.star", "python_add_uv")
load("//@star/packages/star/rust.star", "rust_add")
load("//@star/packages/star/spaces-cli.star", "spaces_add_devutils", "spaces_add_star_formatter")
load("//@star/packages/star/starship.star", "starship_add_bash")
load("//@star/prelude/info.star", "info_is_ci")
load("//@star/prelude/rules/asset.star", "asset_hard_link")
load("//@star/prelude/rules/checkout.star", "checkout_add_any_assets")
load("//@star/prelude/rules/sdk.star", "sdk_is_owner")
load(
    "//@star/sdk/star/checkout-config.star",
    "checkout_config_load_enum",
)
load(
    "//@star/sdk/star/checkout.star",
    "checkout_add_cargo_bin",
)

if sdk_is_owner():
    spaces_add_devutils(
        "spaces0",
        "v0.22.3",
        devutils_version = "devutils-v0.1.16",
        system_paths = ["/usr/bin", "/bin"],
        is_activate_sccache = not info_is_ci(),
    )

    spaces_add_star_formatter(
        "star_formatter0",
        configure_zed = True,
    )

    starship_add_bash(
        "starship0",
        shortcuts = {},
        install_binary = False,
        deps = [":spaces0"],
    )

    rust_add(
        "rust_toolchain",
        version = "1.95",
        deps = [":rust_workspace_toolchain"],
    )

checkout_add_any_assets(
    "rust_workspace_toolchain",
    assets = [
        asset_hard_link(
            source = "//mote/mote-firmware/rust-toolchain.toml",
            destination = "//rust-toolchain.toml",
        ),
    ],
)

node_add(
    "node0",
    "v25.2.1",
)

python_add_uv(
    "python3",
    uv_version = "0.12.18",
    ruff_version = "0.16.9",
)

checkout_add_cargo_bin(
    "probe-rs-tools",
    crate = "probe-rs-tools",
    version = "0.32.0",
    bins = ["probe-rs", "cargo-embed", "cargo-flash"],
)

binutils_bins = [
    "cargo-cov",
    "cargo-nm",
    "cargo-objcopy",
    "cargo-objdump",
    "cargo-readobj",
    "cargo-profdata",
    "cargo-size",
    "cargo-strip",
    "rust-ar",
    "rust-cov",
    "rust-lld",
    "rust-nm",
    "rust-objcopy",
    "rust-objdump",
    "rust-readobj",
    "rust-size",
    "rust-strip",
]

checkout_add_cargo_bin(
    "cargo-binutils",
    crate = "cargo-binutils",
    version = "0.3.6",
    bins = binutils_bins,
)
