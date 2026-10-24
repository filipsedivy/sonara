//! Compiles Sonara's C shim (`csrc/shim.c`) and the vendored miniaudio implementation, and links
//! the system frameworks they need.

use std::env;
use std::path::PathBuf;

fn main() {
    let manifest_dir =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"));
    // The Rust tree owns this vendored copy (third_party/miniaudio, docs/dependencies.md).
    let miniaudio = manifest_dir.join("../../third_party/miniaudio");
    let header = miniaudio.join("miniaudio.h");
    assert!(
        header.is_file(),
        "vendored miniaudio not found at {}",
        header.display()
    );

    for file in [
        "csrc/miniaudio.c",
        "csrc/miniaudio_config.h",
        "csrc/shim.c",
        "csrc/shim.h",
    ] {
        println!("cargo::rerun-if-changed={file}");
    }
    println!("cargo::rerun-if-changed={}", header.display());

    // No fast-math flags in either library (RT-11). Apple Silicon MacBooks are the product
    // platform, matching the Rust `target-cpu` in `.cargo/config.toml`.
    let apple_silicon = env::var("TARGET").as_deref() == Ok("aarch64-apple-darwin");

    // Our own C code: every warning is an error. Compiled before miniaudio, so that a linker that
    // resolves static libraries left to right finds miniaudio's symbols after the shim's uses.
    let mut shim = cc::Build::new();
    shim.file("csrc/shim.c")
        .include(&miniaudio)
        .include("csrc")
        .std("c11")
        .warnings(true)
        .extra_warnings(true)
        .warnings_into_errors(true);
    if apple_silicon {
        shim.flag_if_supported("-mcpu=apple-m1");
    }
    shim.compile("sonara_shim");

    // Third-party code: its warnings are not ours to fix.
    let mut implementation = cc::Build::new();
    implementation
        .file("csrc/miniaudio.c")
        .include(&miniaudio)
        .include("csrc")
        .warnings(false);
    if apple_silicon {
        implementation.flag_if_supported("-mcpu=apple-m1");
    }
    implementation.compile("miniaudio");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        for framework in ["CoreFoundation", "CoreAudio", "AudioToolbox"] {
            println!("cargo::rustc-link-lib=framework={framework}");
        }
    }
}
