//! Compatibility probe and release-mode benchmark comparing official dprint
//! Schema v4 WASM plugins against Fama's existing formatters.
//!
//! Run manually after downloading plugins to `/tmp/fama-dprint-bench/`:
//!   cargo test -p fama-dprint-wasm --release --test benchmark -- --ignored --nocapture

use fama_common::Formatter;
use fama_dprint_wasm::{DprintWasmEngine, DprintWasmSource};
use std::path::Path;
use std::time::{Duration, Instant};

struct BenchCase {
	label: &'static str,
	wasm_path: &'static str,
	sample_path: &'static str,
	sample_code: &'static str,
	existing: Option<&'static dyn Formatter>,
}

fn avg_duration<F: FnMut() -> Result<String, String>>(
	mut f: F,
	iters: u32,
) -> Result<Duration, String> {
	// Warm up once
	let _ = f()?;
	let start = Instant::now();
	for _ in 0..iters {
		let _ = f()?;
	}
	Ok(start.elapsed() / iters)
}

#[test]
#[ignore = "benchmark"]
fn probe_and_benchmark_dprint_plugins() {
	let cases = [
		BenchCase {
			label: "csharpier (C#)",
			wasm_path: "/tmp/fama-dprint-bench/csharpier.wasm",
			sample_path: "Program.cs",
			sample_code: "namespace App{public class Greeter{public static string Hello(string name){return $\"Hi, {name}!\";}}}",
			existing: Some(&fama_clang::FORMATTER),
		},
		BenchCase {
			label: "roslyn (VB.NET)",
			wasm_path: "/tmp/fama-dprint-bench/roslyn.wasm",
			sample_path: "Module1.vb",
			sample_code: "Module Module1\nSub Main()\nConsole.WriteLine(\"Hi\")\nEnd Sub\nEnd Module\n",
			existing: None,
		},
		BenchCase {
			label: "roslyn (C#)",
			wasm_path: "/tmp/fama-dprint-bench/roslyn.wasm",
			sample_path: "Program.cs",
			sample_code: "namespace App{public class Greeter{public static string Hello(string name){return $\"Hi, {name}!\";}}}",
			existing: Some(&fama_clang::FORMATTER),
		},
		BenchCase {
			label: "clang-format (C++)",
			wasm_path: "/tmp/fama-dprint-bench/clang-format.wasm",
			sample_path: "main.cpp",
			sample_code: "#include <vector>\nint sum(const std::vector<int>& v){int s=0;for(auto x:v)s+=x;return s;}\n",
			existing: Some(&fama_clang::FORMATTER),
		},
		BenchCase {
			label: "zig (Zig)",
			wasm_path: "/tmp/fama-dprint-bench/zig.wasm",
			sample_path: "main.zig",
			sample_code: "const std=@import(\"std\");pub fn add(a:i32,b:i32)i32{return a+b;}\n",
			existing: None,
		},
		BenchCase {
			label: "terraform (HCL)",
			wasm_path: "/tmp/fama-dprint-bench/terraform.wasm",
			sample_path: "main.tf",
			sample_code: "resource \"aws_instance\"   \"web\"{\nami=\"ami-123\"\ninstance_type=\"t3.micro\"\n}\n",
			existing: Some(&goffi::FORMATTER),
		},
		BenchCase {
			label: "buildifier (Starlark)",
			wasm_path: "/tmp/fama-dprint-bench/buildifier.wasm",
			sample_path: "BUILD.bazel",
			sample_code: "rust_binary(\nname=\"app\",\nsrcs=[\"main.rs\",\"lib.rs\"],\n)\n",
			existing: Some(&goffi::FORMATTER),
		},
		BenchCase {
			label: "ruff (Python)",
			wasm_path: "/tmp/fama-dprint-bench/ruff.wasm",
			sample_path: "app.py",
			sample_code: "def   add( x:int,y:int )->int:\n  return  x+y\n",
			existing: Some(&fama_builtin::python::PythonFormatter),
		},
		BenchCase {
			label: "typstyle (Typst)",
			wasm_path: "/tmp/fama-dprint-bench/typstyle.wasm",
			sample_path: "doc.typ",
			sample_code: "#let  add( x , y )={x+y}\n= Heading\nHello   world!\n",
			existing: Some(&fama_builtin::typst::TypstFormatter),
		},
		BenchCase {
			label: "alejandra (Nix)",
			wasm_path: "/tmp/fama-dprint-bench/alejandra.wasm",
			sample_path: "default.nix",
			sample_code: "{pkgs}:pkgs.mkShell{buildInputs=[pkgs.ripgrep pkgs.fd];}",
			existing: Some(&fama_builtin::nix::NixFormatter),
		},
		BenchCase {
			label: "json (JSON5)",
			wasm_path: "/tmp/fama-dprint-bench/json.wasm",
			sample_path: "cfg.json5",
			sample_code: "{unquotedKey: 'single', hex: 0x10, arr: [1,2,]}\n",
			existing: Some(&fama_web::json5::Json5Formatter),
		},
	];

	let precompile_dir = tempfile::TempDir::new().unwrap();

	println!("\n| Case | Raw | Zst | Wasmu | Cold Load | Warm Load | dprint WASM / file | Fama Existing / file | Status |");
	println!("| :--- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | :--- |");

	for case in cases {
		if !Path::new(case.wasm_path).exists() {
			println!(
				"| {} | missing | - | - | - | - | - | - | skipped |",
				case.label
			);
			continue;
		}
		let raw_bytes = std::fs::metadata(case.wasm_path).map(|m| m.len()).unwrap_or(0);
		let zst_path = format!("{}.zst", case.wasm_path);
		let zst_bytes = std::fs::metadata(&zst_path).map(|m| m.len()).unwrap_or(0);

		let engine = DprintWasmEngine::new(
			"bench",
			DprintWasmSource::FilePath(case.wasm_path),
		);

		let t0 = Instant::now();
		let info_res = engine.plugin_info();
		let cold = t0.elapsed();

		let wasmu_bytes = match engine.precompile(precompile_dir.path()) {
			Ok(p) => std::fs::metadata(p).map(|m| m.len()).unwrap_or(0),
			Err(_) => 0,
		};

		let warm = if info_res.is_ok() {
			let engine2 = DprintWasmEngine::new(
				"bench",
				DprintWasmSource::FilePath(case.wasm_path),
			);
			let t1 = Instant::now();
			let _ = engine2.plugin_info();
			t1.elapsed()
		} else {
			Duration::ZERO
		};

		let dprint_per_file = match info_res {
			Ok(ref info) => {
				match avg_duration(
					|| engine.format(case.sample_code, Path::new(case.sample_path)),
					20,
				) {
					Ok(d) => Ok((info.name.clone(), info.version.clone(), d)),
					Err(e) => Err(e),
				}
			}
			Err(e) => Err(e),
		};

		let existing_per_file = case.existing.map(|fmt| {
			avg_duration(
				|| fmt.format(case.sample_code, Path::new(case.sample_path)),
				20,
			)
		});

		let existing_str = match existing_per_file {
			Some(Ok(d)) => format!("{:.2}ms", d.as_secs_f64() * 1000.0),
			Some(Err(e)) => format!("err: {e}"),
			None => "N/A".to_string(),
		};

		match dprint_per_file {
			Ok((name, ver, d)) => {
				println!(
					"| {} | {:.2}MB | {:.2}MB | {:.2}MB | {:.0}ms | {:.1}ms | {:.2}ms | {} | ok ({name} {ver}) |",
					case.label,
					raw_bytes as f64 / 1_048_576.0,
					zst_bytes as f64 / 1_048_576.0,
					wasmu_bytes as f64 / 1_048_576.0,
					cold.as_secs_f64() * 1000.0,
					warm.as_secs_f64() * 1000.0,
					d.as_secs_f64() * 1000.0,
					existing_str,
				);
			}
			Err(e) => {
				println!(
					"| {} | {:.2}MB | {:.2}MB | {:.2}MB | {:.0}ms | {:.1}ms | err | {} | FAIL: {e} |",
					case.label,
					raw_bytes as f64 / 1_048_576.0,
					zst_bytes as f64 / 1_048_576.0,
					wasmu_bytes as f64 / 1_048_576.0,
					cold.as_secs_f64() * 1000.0,
					warm.as_secs_f64() * 1000.0,
					existing_str,
				);
			}
		}
	}
}
