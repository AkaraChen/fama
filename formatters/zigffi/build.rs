use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn main() {
	let zig_dir =
		PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("zig");
	let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

	println!("cargo:rerun-if-changed={}", zig_dir.display());
	println!("cargo:rerun-if-changed={}/root.zig", zig_dir.display());
	println!("cargo:rerun-if-changed={}/build.zig", zig_dir.display());

	// Determine target architecture for cross-compilation
	let target = env::var("TARGET").unwrap();
	let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();
	let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();

	// Map Rust target to Zig target triple
	let zig_target = match (target_os.as_str(), target_arch.as_str()) {
		("macos", "aarch64") => "aarch64-macos",
		("macos", "x86_64") => "x86_64-macos",
		("linux", "aarch64") => "aarch64-linux-gnu",
		("linux", "x86_64") => "x86_64-linux-gnu",
		("windows", "x86_64") => "x86_64-windows-gnu",
		("windows", "aarch64") => "aarch64-windows-gnu",
		_ => panic!("Unsupported target: {}-{}", target_os, target_arch),
	};

	// Use target-specific library name to avoid conflicts
	let lib_name = format!("libzigfmt-{}.a", target);
	let lib_src = zig_dir.join(&lib_name);

	// Always rebuild when target changes or library doesn't exist
	if !lib_src.exists() {
		println!(
			"cargo:warning=Building Zig static library for target {} (zig target: {})...",
			target, zig_target
		);

		let zig_out_dir = zig_dir.join("zig-out").join("lib");

		let mut cmd = Command::new("zig");
		cmd.arg("build")
			.arg("-Doptimize=ReleaseFast")
			.arg(format!("-Dtarget={}", zig_target))
			.current_dir(&zig_dir);

		let output = cmd.output();

		match output {
			Ok(o) if o.status.success() => {
				// Try different library names (Unix vs Windows conventions)
				let possible_names = ["libzigfmt.a", "zigfmt.lib"];
				let mut found = false;

				for name in &possible_names {
					let built_lib = zig_out_dir.join(name);
					if built_lib.exists() {
						fs::copy(&built_lib, &lib_src)
							.expect("Failed to copy static library");
						println!(
							"cargo:warning=Successfully built Zig static library"
						);
						found = true;
						break;
					}
				}

				if !found {
					// List what's in the directory for debugging
					if let Ok(entries) = fs::read_dir(&zig_out_dir) {
						let files: Vec<_> = entries
							.filter_map(|e| e.ok())
							.map(|e| e.file_name().to_string_lossy().to_string())
							.collect();
						panic!(
							"Zig build succeeded but library not found. Files in {:?}: {:?}",
							zig_out_dir, files
						);
					} else {
						panic!(
							"Zig build succeeded but output directory not found: {:?}",
							zig_out_dir
						);
					}
				}
			}
			Ok(o) => {
				let stderr = String::from_utf8_lossy(&o.stderr);
				let stdout = String::from_utf8_lossy(&o.stdout);
				panic!(
					"Failed to build Zig static library.\nstdout: {}\nstderr: {}\nPlease run: cd {} && zig build -Doptimize=ReleaseFast",
					stdout,
					stderr,
					zig_dir.display()
				);
			}
			Err(e) => {
				panic!(
					"Failed to execute Zig build: {}\nPlease ensure Zig is installed and in PATH.\nThen run: cd {} && zig build -Doptimize=ReleaseFast",
					e,
					zig_dir.display()
				);
			}
		}
	}

	// Copy the library to OUT_DIR for linking
	let lib_dst = out_dir.join("libzigfmt.a");
	if lib_src.exists() {
		fs::copy(&lib_src, &lib_dst).expect("Failed to copy static library");
	}

	// Tell cargo where to find the library for linking
	println!("cargo:rustc-link-search=native={}", out_dir.display());
	println!("cargo:rustc-link-lib=static=zigfmt");

	// Link system dependencies that Zig runtime might need
	#[cfg(target_os = "macos")]
	{
		println!("cargo:rustc-link-lib=c");
	}

	#[cfg(target_os = "linux")]
	{
		println!("cargo:rustc-link-lib=c");
		println!("cargo:rustc-link-lib=pthread");
		println!("cargo:rustc-link-lib=m");
	}

	#[cfg(target_os = "windows")]
	{
		println!("cargo:rustc-link-lib=kernel32");
		println!("cargo:rustc-link-lib=ntdll");
	}
}
