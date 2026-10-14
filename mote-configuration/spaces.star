"""
Spaces rules for managing mote configuration.
"""

load("//@star/prelude/rules/run.star", "run_add_exec", "run_inputs_once", "run_log_level_passthrough")


run_add_exec(
    "install_wasm_pack",
    command = "cargo",
    args = [
        "install",
        "wasm-pack",
        "--version",
        "0.15.0",
        "--locked",
    ],
    inputs = run_inputs_once(),
    working_directory = ".",
    log_level = run_log_level_passthrough(),
    help = "Install wasm-pack via cargo",
)

run_add_exec(
    "build",
    command = "sh",
    args = [
        "-c",
        """
        set -e
        cd ../mote-ffi
        wasm-pack build --out-dir target/pkg-node --target bundler --features wasm_ffi
        cd ../mote-configuration
        npm install
        npm run generate-types
        """,
    ],
    deps = [":install_wasm_pack"],
    working_directory = ".",
    log_level = run_log_level_passthrough(),
)

run_add_exec(
    "run_dev",
    command = "npx",
    args = [
        "vite",
    ],
    deps = [":build"],
    working_directory = ".",
    log_level = run_log_level_passthrough(),
)

run_add_exec(
    "test",
    command = "npm",
    args = [
        "run",
        "test:run",
    ],
    deps = [":build"],
    working_directory = ".",
    log_level = run_log_level_passthrough(),
)
