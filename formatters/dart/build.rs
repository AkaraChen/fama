use std::fs;
use std::path::Path;

use sha2::{Digest, Sha256};

fn main() {
	let wasm_path = Path::new("wasm/dart_fmt.wasm");
	println!("cargo:rerun-if-changed={}", wasm_path.display());

	let bytes = fs::read(wasm_path)
		.expect("failed to read formatters/dart/wasm/dart_fmt.wasm");
	let digest = Sha256::digest(&bytes);
	let hash: String = digest.iter().take(8).map(|b| format!("{b:02x}")).collect();
	println!("cargo:rustc-env=DART_FMT_WASM_HASH={hash}");
}
