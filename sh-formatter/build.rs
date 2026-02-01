use std::env;
use std::path::PathBuf;

fn main() {
    // Build the Go shared library
    let go_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("../sh-formatter-go");

    println!("cargo:rerun-if-changed={}", go_dir.display());

    // Determine the output library name based on the platform
    let lib_name = if cfg!(target_os = "macos") {
        "libshformatter.dylib"
    } else if cfg!(target_os = "linux") {
        "libshformatter.so"
    } else if cfg!(target_os = "windows") {
        "libshformatter.dll"
    } else {
        panic!("Unsupported platform for sh-formatter");
    };

    let lib_path = go_dir.join(lib_name);

    // Check if the shared library exists, if not, try to build it
    if !lib_path.exists() {
        println!(
            "cargo:warning=Go shared library not found at {}, attempting to build...",
            lib_path.display()
        );

        // Try to build using go command
        let status = std::process::Command::new("go")
            .arg("build")
            .arg("-buildmode=c-shared")
            .arg("-o")
            .arg(&lib_path)
            .arg(go_dir.join("formatter.go"))
            .current_dir(&go_dir)
            .status();

        match status {
            Ok(s) if s.success() => {
                println!("cargo:warning=Successfully built Go shared library");
            }
            _ => {
                println!("cargo:warning=Failed to build Go shared library. Please run: cd {} && go build -buildmode=c-shared -o {} formatter.go",
                         go_dir.display(), lib_name);
            }
        }
    }

    // Link the shared library
    println!("cargo:rustc-link-search={}", go_dir.display());
    println!("cargo:rustc-link-lib=shformatter");

    // On macOS, we need to ensure the library path is set for runtime
    if cfg!(target_os = "macos") {
        println!("cargo:rustc-env=DYLD_LIBRARY_PATH={}", go_dir.display());
    }
}
