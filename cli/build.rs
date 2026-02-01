use std::path::PathBuf;

fn main() {
	// Set rpath for the goffi Go shared library
	// This is needed because goffi is a Rust library (rlib) that links to a C library,
	// and the rpath from the goffi build.rs doesn't propagate to the final binary.
	let go_dir = PathBuf::from("..").join("formatters/goffi/go");

	if cfg!(target_os = "macos") {
		// Add absolute rpath for development builds
		let go_dir_abs = std::fs::canonicalize(&go_dir).unwrap();
		println!("cargo:rustc-link-arg=-Wl,-rpath,{}", go_dir_abs.display());
	}
}
