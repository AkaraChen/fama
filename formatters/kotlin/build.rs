use std::{env, fs, path::PathBuf};

fn main() {
	println!("cargo:rerun-if-env-changed=FAMA_KTFMT_BUNDLE");
	let target = env::var("TARGET").unwrap();
	let filename = if target.contains("windows") { "ktfmt.exe" } else { "ktfmt" };
	let binary = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap())
		.join("bin")
		.join(&target)
		.join(filename);
	println!("cargo:rerun-if-changed={}", binary.display());
	let bundled = binary.is_file();
	if env::var_os("FAMA_KTFMT_BUNDLE").is_some() && !bundled {
		panic!("Missing ktfmt for {target}; run python3 -I formatters/kotlin/download.py --target {target}");
	}
	let source = if bundled {
		format!("const BUNDLED_KTFMT: Option<&[u8]> = Some(include_bytes!({:?}));", binary)
	} else {
		"const BUNDLED_KTFMT: Option<&[u8]> = None;".to_string()
	};
	fs::write(PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("bundled.rs"), source).unwrap();
}
