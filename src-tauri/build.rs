fn main() {
    // The macOS development icon is embedded into the Rust binary by
    // `generate_context!`, so Cargo must rebuild when the source icon changes.
    println!("cargo:rerun-if-changed=icons");
    tauri_build::build()
}
