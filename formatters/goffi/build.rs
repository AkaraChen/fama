use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn main() {
	let go_dir =
		PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("go");
	let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

	println!("cargo:rerun-if-changed={}", go_dir.display());
	println!("cargo:rerun-if-changed={}/formatter.go", go_dir.display());

	// Static library name
	let lib_name = "libgoffi.a";
	let lib_src = go_dir.join(lib_name);

	// Build the Go static library if it doesn't exist
	if !lib_src.exists() {
		println!(
            "cargo:warning=Go static library not found at {}, attempting to build...",
            lib_src.display()
        );

		let status = Command::new("go")
			.arg("build")
			.arg("-buildmode=c-archive")
			.arg("-o")
			.arg(&lib_src)
			.arg("formatter.go")
			.current_dir(&go_dir)
			.status();

		match status {
			Ok(s) if s.success() => {
				println!("cargo:warning=Successfully built Go static library");
			}
			_ => {
				panic!(
                    "Failed to build Go static library. Please run: cd {} && go build -buildmode=c-archive -o {} formatter.go",
                    go_dir.display(),
                    lib_name
                );
			}
		}
	}

	// Copy the library to OUT_DIR for linking
	let lib_dst = out_dir.join(lib_name);
	if lib_src.exists() {
		fs::copy(&lib_src, &lib_dst).expect("Failed to copy static library");
	}

	// Tell cargo where to find the library for linking
	println!("cargo:rustc-link-search=native={}", out_dir.display());
	println!("cargo:rustc-link-lib=static=goffi");

	// Link Go runtime dependencies
	#[cfg(target_os = "macos")]
	{
		println!("cargo:rustc-link-lib=framework=CoreFoundation");
		println!("cargo:rustc-link-lib=framework=Security");
		println!("cargo:rustc-link-lib=resolv");
	}

	#[cfg(target_os = "linux")]
	{
		println!("cargo:rustc-link-lib=pthread");
		println!("cargo:rustc-link-lib=dl");
		println!("cargo:rustc-link-lib=m");
	}

	#[cfg(target_os = "windows")]
	{
		println!("cargo:rustc-link-lib=ws2_32");
		println!("cargo:rustc-link-lib=userenv");
		println!("cargo:rustc-link-lib=bcrypt");
		println!("cargo:rustc-link-lib=ntdll");
	}
}
