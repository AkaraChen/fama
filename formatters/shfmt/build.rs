use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let go_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("go");
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    println!("cargo:rerun-if-changed={}", go_dir.display());

    // Determine the library name based on platform
    let lib_name = if cfg!(target_os = "macos") {
        "libshformatter.dylib"
    } else if cfg!(target_os = "linux") {
        "libshformatter.so"
    } else if cfg!(target_os = "windows") {
        "libshformatter.dll"
    } else {
        panic!("Unsupported platform for sh-formatter");
    };

    let lib_src = go_dir.join(lib_name);

    // Build the Go library if it doesn't exist
    if !lib_src.exists() {
        println!(
            "cargo:warning=Go shared library not found at {}, attempting to build...",
            lib_src.display()
        );

        let status = Command::new("go")
            .arg("build")
            .arg("-buildmode=c-shared")
            .arg("-o")
            .arg(&lib_src)
            .arg("formatter.go")
            .current_dir(&go_dir)
            .status();

        match status {
            Ok(s) if s.success() => {
                println!("cargo:warning=Successfully built Go shared library");
            }
            _ => {
                panic!(
                    "Failed to build Go shared library. Please run: cd {} && go build -buildmode=c-shared -o {} formatter.go",
                    go_dir.display(),
                    lib_name
                );
            }
        }
    }

    // Copy the library to OUT_DIR for linking
    let lib_dst = out_dir.join(lib_name);
    if lib_src.exists() {
        fs::copy(&lib_src, &lib_dst).expect("Failed to copy shared library");

        // On macOS, fix the install_name to use @rpath
        #[cfg(target_os = "macos")]
        {
            let _ = Command::new("install_name_tool")
                .arg("-id")
                .arg(format!("@rpath/{}", lib_name))
                .arg(&lib_dst)
                .status();
        }
    }

    // Tell cargo where to find the library for linking
    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-lib=dylib=shformatter");

    // Set rpath for runtime library loading
    #[cfg(target_os = "macos")]
    {
        // Add rpath to the go directory (for development)
        println!("cargo:rustc-link-arg=-Wl,-rpath,{}", go_dir.display());
    }

    #[cfg(target_os = "linux")]
    {
        println!("cargo:rustc-link-arg=-Wl,-rpath,{}", go_dir.display());
        println!("cargo:rustc-link-arg=-Wl,-rpath,$ORIGIN");
    }
}
