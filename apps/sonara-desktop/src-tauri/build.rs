//! Generates the Tauri context: the configuration, the capabilities and, in release bundles,
//! the embedded frontend.
fn main() {
    tauri_build::build();
}
