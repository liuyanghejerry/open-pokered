fn main() {
    // c++_shared / android / log are Android NDK libraries. Only emit the link
    // directives when actually building for Android — emitting them
    // unconditionally breaks host builds and tests (e.g. `cargo test
    // --workspace` on macOS/Linux) with `ld: library 'c++_shared' not found`,
    // even though the Android-specific code in lib.rs is gated behind
    // `#[cfg(target_os = "android")]` and compiles to an empty lib off-target.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("android") {
        println!("cargo:rustc-link-lib=c++_shared");
        println!("cargo:rustc-link-lib=android");
        println!("cargo:rustc-link-lib=log");
    }
}
