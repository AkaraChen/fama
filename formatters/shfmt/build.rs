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
        "shformatter.dll"
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

    // On Windows, we need to create an import library (.lib) for MSVC linker
    #[cfg(target_os = "windows")]
    {
        use std::io::Write;

        let def_path = out_dir.join("shformatter.def");
        let lib_path = out_dir.join("shformatter.lib");

        // Create a .def file with the exported functions
        let def_content = r#"LIBRARY shformatter
EXPORTS
    FormatShell
    FormatShellBatch
    FreeString
    FreeStringArray
"#;
        let mut def_file = fs::File::create(&def_path).expect("Failed to create .def file");
        def_file
            .write_all(def_content.as_bytes())
            .expect("Failed to write .def file");

        // Use lib.exe to create the import library
        // lib.exe is part of MSVC toolchain
        let status = Command::new("lib.exe")
            .arg(format!("/DEF:{}", def_path.display()))
            .arg(format!("/OUT:{}", lib_path.display()))
            .arg("/MACHINE:X64")
            .status();

        match status {
            Ok(s) if s.success() => {
                println!("cargo:warning=Successfully created import library");
            }
            _ => {
                // Try dlltool as fallback (MinGW)
                let status = Command::new("dlltool")
                    .arg("-d")
                    .arg(&def_path)
                    .arg("-l")
                    .arg(&lib_path)
                    .status();

                match status {
                    Ok(s) if s.success() => {
                        println!("cargo:warning=Successfully created import library using dlltool");
                    }
                    _ => {
                        panic!(
                            "Failed to create import library. Ensure lib.exe (MSVC) or dlltool (MinGW) is available."
                        );
                    }
                }
            }
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
