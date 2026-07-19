fn main() {
    generate_schemas();
    #[cfg(feature = "cxx-build")]
    generate_cxx_bridge();
}

fn generate_schemas() {
    use mote_api::messages::{host_to_mote, mote_to_host};
    use schemars::schema_for;
    use std::path::PathBuf;

    let crate_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let schemas_dir = PathBuf::from(&crate_dir).join("schemas");
    std::fs::create_dir_all(&schemas_dir).unwrap();

    let host_to_mote = serde_json::to_string_pretty(&schema_for!(host_to_mote::Message)).unwrap();
    let mote_to_host = serde_json::to_string_pretty(&schema_for!(mote_to_host::Message)).unwrap();

    std::fs::write(schemas_dir.join("host_to_mote.json"), host_to_mote).unwrap();
    std::fs::write(schemas_dir.join("mote_to_host.json"), mote_to_host).unwrap();

    println!("cargo:rerun-if-changed=../mote-api/src/messages");
}

#[cfg(feature = "cxx-build")]
fn generate_cxx_bridge() {
    use std::path::PathBuf;

    println!("cargo:rerun-if-changed=src/cpp.rs");

    cxx_build::bridge("src/cpp.rs")
        .std("c++17")
        .compile("mote-ffi-cxx");

    // cxx_build writes generated headers under a build-script-instance-specific OUT_DIR
    // (target/<profile>/build/mote-ffi-<hash>/out/cxxbridge/...), which isn't a stable path
    // downstream packaging can reference (the hash changes across builds/toolchains). Copy
    // the two headers consumers need into a fixed, crate-relative location instead.
    let crate_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let dest_root = PathBuf::from(&crate_dir).join("include");

    copy_generated_header(
        &out_dir.join("cxxbridge/include/mote-ffi/src/cpp.rs.h"),
        &dest_root.join("mote-ffi/src/cpp.rs.h"),
    );
    copy_generated_header(
        &out_dir.join("cxxbridge/include/rust/cxx.h"),
        &dest_root.join("rust/cxx.h"),
    );
}

#[cfg(feature = "cxx-build")]
fn copy_generated_header(src: &std::path::Path, dest: &std::path::Path) {
    std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
    std::fs::copy(src, dest).unwrap_or_else(|e| {
        panic!(
            "failed to copy {} -> {}: {e}",
            src.display(),
            dest.display()
        )
    });
}
