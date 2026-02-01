use std::path::PathBuf;

fn main() {
    // Set rpath for the sh-formatter Go shared library
    // This is needed because sh-formatter is a Rust library (rlib) that links to a C library,
    // and the rpath from the sh-formatter build.rs doesn't propagate to the final binary.
    let go_dir = PathBuf::from("..").join("formatters/shfmt/go");

    if cfg!(target_os = "macos") {
        // Add absolute rpath for development builds
        let go_dir_abs = std::fs::canonicalize(&go_dir).unwrap();
        println!("cargo:rustc-link-arg=-Wl,-rpath,{}", go_dir_abs.display());
    }
}
