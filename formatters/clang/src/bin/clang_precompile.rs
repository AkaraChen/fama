//! Writes the precompiled clang-format module artifact used by releases.
//!
//! The artifact is platform-specific (Cranelift code for the compiling host);
//! run this on each release platform and ship the output next to the fama
//! binary. At runtime fama deserializes it in milliseconds instead of paying
//! the ~1.5s Cranelift compilation.

fn main() {
	let dir = std::env::args().nth(1).unwrap_or_else(|| ".".to_string());
	let bytes = fama_clang::serialize_compiled_module()
		.unwrap_or_else(|e| panic!("precompile failed: {e}"));
	let path = std::path::Path::new(&dir)
		.join(fama_clang::precompiled_artifact_name());
	std::fs::write(&path, &bytes)
		.unwrap_or_else(|e| panic!("failed to write {}: {e}", path.display()));
	println!("{}", path.display());
}
