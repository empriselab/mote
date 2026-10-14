"""
Spaces rules for managing mote firmware
"""

load("//@star/prelude/rules/run.star", "run_add_exec", "run_log_level_passthrough")

_CHIP = "RP235x"

cargo_env = {
    "RUSTUP_TOOLCHAIN": "",
}

run_add_exec(
    "build",
    command = "cargo",
    args = [
        "build",
        "--release",
    ],
    working_directory = ".",
    log_level = run_log_level_passthrough(),
    env = cargo_env,
    help = "Build the mote-firmware using cargo build",
)

run_add_exec(
    "provision",
    command = "sh",
    args = [
        "-c",
        """
        set -e
        probe-rs download cyw43-firmware/43439A0.bin --binary-format bin --chip RP235x --base-address 0x10100000
        probe-rs download cyw43-firmware/43439A0_clm.bin --binary-format bin --chip RP235x --base-address 0x10140000
        cargo run --release
        """,
    ],
    working_directory = ".",
    env = cargo_env,
    log_level = run_log_level_passthrough(),
)

run_add_exec(
    "deploy",
    command = "cargo",
    args = [
        "run",
        "--release",
    ],
    working_directory = ".",
    env = {
        "DEFMT_LOG": "info",
    } | cargo_env,
    log_level = run_log_level_passthrough(),
    help = """
    Build and flash the mote-firmware to the device using the RPI debug probe.

    This will also allow debugging using defmt::info!(), warn!() and error!().
    """,
)

run_add_exec(
    "debug",
    command = "cargo",
    args = [
        "run",
        "--release",
    ],
    working_directory = ".",
    env = {
        "DEFMT_LOG": "debug",
    } | cargo_env,
    log_level = run_log_level_passthrough(),
    help = """
    Build and flash the mote-firmware to the device using the RPI debug probe.

    This will also allow debugging using defmt::debug!(), info!(), warn!() and error!().
    """,
)

run_add_exec(
    "trace",
    command = "cargo",
    args = [
        "run",
        "--release",
    ],
    env = {
        "DEFMT_LOG": "trace",
    } | cargo_env,
    working_directory = ".",
    log_level = run_log_level_passthrough(),
    help = """
    Build and flash the mote-firmware to the device using the RPI debug probe.

    This will also allow debugging using defmt::trace!(), debug!(), info!(), warn!() and error!().

    This will have a ridiculous level of output messages.
    """,
)

run_add_exec(
    "format",
    command = "cargo",
    args = [
        "fmt",
    ],
    working_directory = ".",
    log_level = run_log_level_passthrough(),
    help = "Format the code in the mote-firmware project",
)

run_add_exec(
    "lint",
    command = "cargo",
    args = [
        "clippy",
        "--all-features",
        "--",
        "-D",
        "warnings",
    ],
    env = cargo_env,
    working_directory = ".",
    log_level = run_log_level_passthrough(),
    help = "Run cargo clippy (linter) on the mote-firmware project",
)

run_add_exec(
    "format_check",
    command = "cargo",
    args = [
        "fmt",
        "--check",
    ],
    env = cargo_env,
    working_directory = ".",
    log_level = run_log_level_passthrough(),
    help = "Check that the formatting is correct.",
)

run_add_exec(
    "uf2",
    command = "bash",
    args = [
        "-c",
        """
        set -euo pipefail
        VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/version = "\\(.*\\)"/\\1/')
        cargo build --release --features bake-cyw43-firmware
        picotool uf2 convert target/thumbv8m.main-none-eabihf/release/mote-firmware -t elf \"mote-firmware-v${VERSION//./_}.uf2\" --family rp2350-arm-s
        """,
    ],
    env = cargo_env,
    working_directory = ".",
    log_level = run_log_level_passthrough(),
    help = "Flashes the firmware on the Wifi chip",
)

run_add_exec(
    "probe_list",
    command = "probe-rs",
    args = [
        "list",
    ],
    log_level = run_log_level_passthrough(),
    help = """
    Show information about the connected probe

    If the message \"No debug probes were found.\" shows up, check the USB connection.

    You should see something like:

    The following debug probes were found:
    [0]: Debug Probe (CMSIS-DAP) -- 2e8a:000c-0:E665B838875A2132 (CMSIS-DAP)
    """,
)

run_add_exec(
    "probe_info",
    command = "probe-rs",
    args = [
        "info",
        "--protocol=swd",
        "--verbose",
    ],
    log_level = run_log_level_passthrough(),
    help = """
    Show information about the connected probe and target device
    """,
)

run_add_exec(
    "probe_attach",
    command = "probe-rs",
    args = [
        "attach",
        "--chip={}".format(_CHIP),
        "--protocol=swd",
        "target/thumbv8m.main-none-eabihf/debug/mote-firmware",
    ],
    deps = [":build"],
    working_directory = ".",
    log_level = run_log_level_passthrough(),
    help = """
    Run the firmware currently installed on the target and attach to the debug output stream
    """,
)
