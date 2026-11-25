//! Dart formatter for fama, powered by real dart_style compiled to WASM.
//!
//! The module embedded at `wasm/dart_fmt.wasm` is dart_style (via the
//! wasm-fmt/dart_fmt binding) compiled with dart2wasm. It runs in-process on
//! wasmtime (Cranelift JIT) - no sidecar process, no temp files - exactly
//! like the clang-format crate.
//!
//! Hosting dart2wasm without a JS engine takes three pieces (see `shims`):
//!
//! 1. ~4200 constant strings imported as immutable `(ref extern)` globals
//!    whose import name is the string content (the js-string builtins
//!    "imported string constants" mechanism). Defined here per instantiation.
//! 2. The `wasm:js-string` builtin functions, implemented over UTF-16
//!    strings since wasmtime has no JS engine to provide them.
//! 3. The `dart2wasm::*` js-interop runtime helpers, implemented over a small
//!    Rust "JS value" model. The compiled `main` registers a Dart closure as
//!    `globalThis.format`; calling it through its trampoline funcref runs
//!    dart_style and returns a `{success, code, error}` object.

mod jsval;
mod shims;

use std::cell::RefCell;
use std::path::PathBuf;
use std::sync::OnceLock;

use fama_common::{FileType, LineEnding, CONFIG};
use jsval::JsVal;
use wasmtime::{
	AsContext, ExternRef, Global, Module, Mutability, Rooted, Store, Tag, Val,
	ValType,
};

/// Embedded dart_style WASM binary (dart2wasm output, ~900KB)
const DART_FMT_WASM: &[u8] = include_bytes!("../wasm/dart_fmt.wasm");

/// sha256 prefix of the embedded wasm bytes; binds precompiled artifacts to
/// the exact module they were compiled from
fn artifact_hash() -> String {
	use sha2::{Digest, Sha256};
	let mut hasher = Sha256::new();
	hasher.update(DART_FMT_WASM);
	let digest = hasher.finalize();
	digest
		.iter()
		.map(|b| format!("{b:02x}"))
		.collect::<String>()[..16]
		.to_string()
}

/// Filename of the precompiled module artifact for this fama build
pub fn precompiled_artifact_name() -> String {
	format!("dart_fmt-{}.wasmu", artifact_hash())
}

/// Candidate locations of a precompiled module: next to the fama binary
/// (shipped with releases) and the per-user cache (seeded by fama itself)
fn precompiled_artifact_candidates() -> Vec<PathBuf> {
	let name = precompiled_artifact_name();
	let mut paths = Vec::new();
	if let Ok(exe) = std::env::current_exe() {
		if let Some(dir) = exe.parent() {
			paths.push(dir.join(&name));
		}
	}
	let home =
		std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"));
	if let Some(home) = home {
		paths.push(PathBuf::from(home).join(".cache/fama").join(&name));
	}
	paths
}

/// Build the engine, compile the embedded module and return the serialized
/// precompiled artifact (used by release CI to ship per-platform `.wasmu`
/// files next to the fama binary)
pub fn serialize_compiled_module() -> Result<Vec<u8>, String> {
	let cfg =
		engine_config().map_err(|e| format!("Failed to build config: {e}"))?;
	let engine = wasmtime::Engine::new(&cfg)
		.map_err(|e| format!("Failed to create engine: {e}"))?;
	let module = wasmtime::Module::new(&engine, DART_FMT_WASM)
		.map_err(|e| format!("Failed to compile WASM module: {e:?}"))?;
	module
		.serialize()
		.map_err(|e| format!("Failed to serialize module: {e:?}"))
}

fn engine_config() -> wasmtime::Result<wasmtime::Config> {
	let mut cfg = wasmtime::Config::new();
	// dart2wasm output: wasm GC types plus standard `try_table` exceptions
	// (the legacy-exceptions output of older Dart SDKs is NOT compilable by
	// wasmtime; the vendored module was built with Dart >= 3.14)
	cfg.wasm_gc(true);
	cfg.wasm_exceptions(true);
	// Failures surface as formatting errors; backtrace capture would only
	// add per-call overhead
	cfg.wasm_backtrace_max_frames(None);
	Ok(cfg)
}

/// Store context: state lives in the per-thread host registry instead
struct StoreCtx;

struct CachedModule {
	engine: wasmtime::Engine,
	module: wasmtime::Module,
}

static CACHED_MODULE: OnceLock<CachedModule> = OnceLock::new();

fn get_cached_module() -> &'static CachedModule {
	CACHED_MODULE.get_or_init(|| {
		let cfg = engine_config().expect("valid wasmtime config");
		let engine = wasmtime::Engine::new(&cfg)
			.expect("Failed to create wasmtime engine");

		// Prefer a precompiled artifact (shipped with releases or cached by
		// a previous run): deserializing takes milliseconds versus ~1.5s to
		// Cranelift-compile the ~900KB module
		for path in precompiled_artifact_candidates() {
			if !path.is_file() {
				continue;
			}
			// SAFETY: the artifact was written by this same binary (its
			// filename is bound to the sha256 of the embedded module bytes)
			// in a trusted location, and deserialize validates the wasmtime
			// version and host feature compatibility; any mismatch falls
			// back to compiling below
			if let Ok(module) =
				unsafe { wasmtime::Module::deserialize_file(&engine, &path) }
			{
				return CachedModule { engine, module };
			}
		}

		let module = Module::new(&engine, DART_FMT_WASM)
			.expect("Failed to compile WASM module");

		// Store the compiled module for future runs (best effort, user cache)
		if let Ok(artifact) = module.serialize() {
			let mut candidates = precompiled_artifact_candidates();
			if let Some(cache_path) = candidates.pop() {
				if let Some(dir) = cache_path.parent() {
					let _ = std::fs::create_dir_all(dir);
					let tmp = cache_path.with_extension("wasmu.tmp");
					if std::fs::write(&tmp, &artifact).is_ok() {
						let _ = std::fs::rename(&tmp, &cache_path);
					}
				}
			}
		}

		CachedModule { engine, module }
	})
}

/// Define the string-constant globals (import name == string content) and
/// the `WebAssembly.JSTag` the module imports
fn define_string_globals(
	store: &mut Store<StoreCtx>,
	linker: &mut wasmtime::Linker<StoreCtx>,
	module: &wasmtime::Module,
) -> Result<(), String> {
	for imp in module.imports() {
		match imp.ty() {
			wasmtime::ExternType::Global(gt) => {
				// Every global import is a js-string builtins string
				// constant: `(import "" "<content>" (global (ref extern)))`
				// (the empty name is the empty string constant). The
				// content is the non-nullable `(ref extern)`, a subtype of
				// `EXTERNREF`.
				if gt.mutability() != Mutability::Const
					|| !gt.content().matches(&ValType::EXTERNREF)
				{
					return Err(format!(
						"dart_fmt: unexpected global import {}::{}",
						imp.module(),
						imp.name()
					));
				}
				let value =
					ExternRef::new(&mut *store, JsVal::string(imp.name()))
						.map_err(|e| {
							format!("dart_fmt: allocating constant: {e}")
						})?;
				let global = Global::new(
					&mut *store,
					wasmtime::GlobalType::new(
						gt.content().clone(),
						Mutability::Const,
					),
					Val::ExternRef(Some(value)),
				)
				.map_err(|e| {
					format!("dart_fmt: creating constant global: {e}")
				})?;
				linker
					.define(&*store, imp.module(), imp.name(), global)
					.map_err(|e| {
						format!("dart_fmt: defining constant global: {e}")
					})?;
			}
			wasmtime::ExternType::Tag(tag_ty) => {
				// WebAssembly.JSTag, used to catch JS exceptions; this host
				// never throws through it, so Dart-level catches simply
				// never fire
				let tag = Tag::new(&mut *store, &tag_ty)
					.map_err(|e| format!("dart_fmt: creating JSTag: {e}"))?;
				linker
					.define(&*store, imp.module(), imp.name(), tag)
					.map_err(|e| format!("dart_fmt: defining JSTag: {e}"))?;
			}
			_ => {}
		}
	}
	Ok(())
}

/// A fully initialized WASM instance ready to format files.
///
/// `main` has already run: the Dart `format` closure is captured in the
/// per-thread host registry, ready to be invoked through its trampoline.
struct ReusableInstance {
	store: Store<StoreCtx>,
}

thread_local! {
	/// Per-thread instance pool (wasmtime's `Store` is `Send` but not
	/// `Sync`). Reusing an instance across files skips instantiation and the
	/// ~4200 constant-string globals for every file after the first one on
	/// each rayon worker.
	static INSTANCE: RefCell<Option<ReusableInstance>> =
		const { RefCell::new(None) };
}

thread_local! {
	// Counter of instance creations on this thread, for asserting reuse in
	// tests
	#[cfg(test)]
	static INSTANCE_CREATIONS: std::cell::Cell<usize> =
		const { std::cell::Cell::new(0) };
}

/// Run `f` with a reusable per-thread instance.
///
/// The instance is kept for future calls on success. On error it is dropped,
/// so the next call starts from a fresh instance instead of a poisoned one.
fn with_reused_instance<R>(
	f: impl FnOnce(&mut Store<StoreCtx>) -> Result<R, String>,
) -> Result<R, String> {
	INSTANCE.with(|slot| {
		let mut slot = slot.borrow_mut();

		let mut reusable = match slot.take() {
			Some(reusable) => Some(reusable),
			None => {
				#[cfg(test)]
				INSTANCE_CREATIONS.with(|count| count.set(count.get() + 1));
				match create_instance() {
					Ok(instance) => Some(instance),
					Err(e) => return Err(e),
				}
			}
		};

		let ReusableInstance { store } = reusable.as_mut().unwrap();

		match f(store) {
			Ok(result) => {
				*slot = reusable.take();
				Ok(result)
			}
			Err(error) => Err(error),
		}
	})
}

/// Create a store and instance, running the module's `main` so the Dart
/// `format` closure registers itself with the host
fn create_instance() -> Result<ReusableInstance, String> {
	let cached = get_cached_module();

	// Drop handles belonging to a previous instance's GC heap
	shims::reset_host_state();

	let mut store = Store::new(&cached.engine, StoreCtx);

	let mut linker = wasmtime::Linker::new(&cached.engine);
	shims::define(&mut linker, &cached.engine)?;
	define_string_globals(&mut store, &mut linker, &cached.module)?;

	let instance = linker
		.instantiate(&mut store, &cached.module)
		.map_err(|e| format!("Failed to instantiate dart_fmt module: {e:#}"))?;

	// Invoke main with an empty JS arguments array; this registers the
	// `format` closure via the dart2wasm `globalThis.format` helper
	let invoke_main: wasmtime::TypedFunc<(Rooted<ExternRef>,), ()> = instance
		.get_typed_func(&mut store, "$invokeMain")
		.map_err(|e| format!("Failed to get $invokeMain: {e}"))?;
	let args = ExternRef::new(&mut store, JsVal::Array(Vec::new()))
		.map_err(|e| format!("dart_fmt: allocating main args: {e}"))?;
	invoke_main
		.call(&mut store, (args,))
		.map_err(|e| format!("Failed to run dart_fmt main: {e}"))?;

	if shims::format_fn_id().is_none() {
		return Err("dart_fmt: module main did not register a format closure"
			.to_string());
	}

	Ok(ReusableInstance { store })
}

/// Options JSON consumed by dart_fmt's binding: `pageWidth` from fama's
/// line width, `lineEnding` from fama's line ending setting (dart_style
/// expects the literal line-ending characters)
fn build_options_json() -> String {
	let line_ending = match CONFIG.line_ending {
		LineEnding::Lf => "\\n",
		LineEnding::Crlf => "\\r\\n",
	};
	format!(
		"{{\"pageWidth\":{},\"lineEnding\":\"{}\"}}",
		CONFIG.line_width, line_ending
	)
}

/// Call the captured Dart `format` closure through its trampoline funcref
/// with (source, filename, optionsJson) and decode the FormatResult object
fn format_via_wasm(
	store: &mut Store<StoreCtx>,
	source: &str,
	filename: &str,
	options_json: &str,
) -> Result<String, String> {
	let format_fn = shims::format_fn_id()
		.ok_or_else(|| "dart_fmt: format closure not registered".to_string())?;
	let (trampoline, closure) =
		shims::registered_fn(format_fn).ok_or_else(|| {
			"dart_fmt: format closure lost from registry".to_string()
		})?;
	let closure = closure.ok_or_else(|| {
		"dart_fmt: format closure without Dart object".to_string()
	})?;

	// Sanity-check the trampoline signature: (closure, argc, arg0, arg1, arg2)
	let ty = trampoline.ty(&*store);
	let params: Vec<ValType> = ty.params().collect();
	let externref_ok = |p: &ValType| p.matches(&ValType::EXTERNREF);
	if params.len() != 5
		|| !externref_ok(&params[0])
		|| !ValType::eq(&params[1], &ValType::I32)
		|| !externref_ok(&params[2])
		|| !externref_ok(&params[3])
		|| !externref_ok(&params[4])
	{
		return Err(format!(
			"dart_fmt: unexpected format trampoline signature ({params:?})"
		));
	}

	let mut args = vec![
		Val::ExternRef(Some(closure.to_rooted(&mut *store))),
		Val::I32(3),
	];
	for value in [source, filename, options_json] {
		let externref = ExternRef::new(&mut *store, JsVal::string(value))
			.map_err(|e| format!("dart_fmt: allocating argument: {e}"))?;
		args.push(Val::ExternRef(Some(externref)));
	}

	let mut results = [Val::ExternRef(None)];
	trampoline
		.call(&mut *store, &args, &mut results)
		.map_err(|e| format!("dart_fmt: format call failed: {e:#?}"))?;

	let Val::ExternRef(Some(result)) = &results[0] else {
		return Err("dart_fmt: format returned null".to_string());
	};
	let payload = result
		.data::<StoreCtx>(store.as_context())
		.map_err(|e| format!("dart_fmt: reading result: {e}"))?
		.ok_or_else(|| "dart_fmt: result is not an externref".to_string())?;
	let payload = payload
		.downcast_ref::<JsVal>()
		.ok_or_else(|| "dart_fmt: result payload mismatch".to_string())?;

	let JsVal::Object(fields) = payload else {
		return Err(format!("dart_fmt: unexpected result {payload:?}"));
	};

	// dart2wasm may pass small values (including the success flag) as i31
	// references, which read back as JS numbers
	let is_truthy = |v: &JsVal| match v {
		JsVal::Bool(b) => *b,
		JsVal::Number(n) => *n != 0.0,
		_ => false,
	};

	if is_truthy(&fields.get("success")) {
		match fields.get("code") {
			JsVal::String(code) => Ok(code.to_rust()),
			other => Err(format!("dart_fmt: success without code: {other:?}")),
		}
	} else {
		match fields.get("error") {
			JsVal::String(error) => Err(error.to_rust()),
			_ => Err("dart_fmt: formatting failed".to_string()),
		}
	}
}

/// Format Dart code using dart_style compiled to WASM
///
/// # Arguments
/// * `content` - The Dart source code to format
/// * `path` - The file path (passed to dart_style as the source URI)
/// * `file_type` - The detected file type (must be `FileType::Dart`)
///
/// # Returns
/// * `Ok(String)` - The formatted code
/// * `Err(String)` - Error message if formatting failed
pub fn format_file(
	content: &str,
	path: &str,
	_file_type: FileType,
) -> Result<String, String> {
	with_reused_instance(|store| {
		format_via_wasm(store, content, path, &build_options_json())
	})
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_format_simple_main() {
		let input = "void main(){print('hello');}";
		let result = format_file(input, "test.dart", FileType::Dart);
		assert!(result.is_ok(), "Format failed: {:?}", result);
		assert_eq!(result.unwrap(), "void main() {\n  print('hello');\n}\n");
	}

	#[test]
	fn test_golden_class() {
		let input = "class Foo{final int x;Foo(this.x);int bar()=>x*2;}";
		let result = format_file(input, "test.dart", FileType::Dart);
		assert!(result.is_ok(), "Format failed: {:?}", result);
		assert_eq!(
			result.unwrap(),
			"class Foo {\n  final int x;\n  Foo(this.x);\n  int bar() => x * 2;\n}\n"
		);
	}

	#[test]
	fn test_golden_trailing_commas() {
		// dart_style's tall style only splits collection literals when the
		// source already splits them (trailing comma alone does not force it)
		let input = "var list=[1,2,3];var map={'a':1,'b':2};";
		let result = format_file(input, "test.dart", FileType::Dart);
		assert!(result.is_ok(), "Format failed: {:?}", result);
		assert_eq!(
			result.unwrap(),
			"var list = [1, 2, 3];\nvar map = {'a': 1, 'b': 2};\n"
		);
	}

	#[test]
	fn test_golden_split_collections() {
		// Verified against `dart format` (Dart 3.14): list literals do not
		// split on trailing commas, unlike map literals
		let input = "var list=[\n1,2,3,\n];";
		let result = format_file(input, "test.dart", FileType::Dart);
		assert!(result.is_ok(), "Format failed: {:?}", result);
		assert_eq!(result.unwrap(), "var list = [1, 2, 3];\n");
	}

	#[test]
	fn test_golden_generics_and_interpolation() {
		let input =
			"Map<String,List<int>>m={'x':[1,2],};void f(){print('${m.length}!');}";
		let result = format_file(input, "test.dart", FileType::Dart);
		assert!(result.is_ok(), "Format failed: {:?}", result);
		assert_eq!(
			result.unwrap(),
			"Map<String, List<int>> m = {\n  'x': [1, 2],\n};\nvoid f() {\n  print('${m.length}!');\n}\n"
		);
	}

	#[test]
	fn test_golden_cascade() {
		let input = "var w=Widget()..text='hi'..build();";
		let result = format_file(input, "test.dart", FileType::Dart);
		assert!(result.is_ok(), "Format failed: {:?}", result);
		assert_eq!(
			result.unwrap(),
			"var w = Widget()\n  ..text = 'hi'\n  ..build();\n"
		);
	}

	#[test]
	fn test_line_width_config() {
		// fama's default line width is 80: a short call must stay on one line
		let input =
			"void main(){someFunction(argumentOne,argumentTwo,argumentThree);}";
		let result = format_file(input, "test.dart", FileType::Dart);
		assert!(result.is_ok(), "Format failed: {:?}", result);
		let formatted = result.unwrap();
		assert!(
			formatted.starts_with("void main() {\n  someFunction("),
			"Unexpected formatting: {formatted}"
		);
	}

	#[test]
	fn test_syntax_error_returns_err() {
		let input = "void main(){print('unclosed}";
		let result = format_file(input, "bad.dart", FileType::Dart);
		assert!(result.is_err(), "Expected error for invalid syntax");
		let message = result.unwrap_err();
		assert!(
			message.contains("Error") || !message.is_empty(),
			"Error message should describe the problem: {message}"
		);
	}

	#[test]
	fn test_instance_survives_errors() {
		// A syntax error must not poison the instance: the next call formats
		// correctly (a fresh instance is created if the old one is dropped)
		assert!(format_file("void f(){", "bad.dart", FileType::Dart).is_err());
		let ok = format_file("void f(){}", "good.dart", FileType::Dart);
		assert!(ok.is_ok(), "Instance did not recover: {:?}", ok);
		assert_eq!(ok.unwrap(), "void f() {}\n");
	}

	#[test]
	fn test_reused_instance_formats_repeatedly() {
		// The per-thread instance is reused across calls; every call must
		// still produce a correct, independent result.
		let creations_before = INSTANCE_CREATIONS.with(|count| count.get());
		for i in 0..25 {
			let input = format!("void f(){{var x={i};print(x);}}");
			let formatted = format_file(&input, "test.dart", FileType::Dart)
				.unwrap_or_else(|e| panic!("call {i} failed: {e}"));
			assert!(
				formatted.contains(&format!("var x = {i};")),
				"call {i} wrong body: {formatted}"
			);
		}

		// 25 sequential calls on this thread must create exactly one instance
		let creations_after = INSTANCE_CREATIONS.with(|count| count.get());
		assert_eq!(
			creations_after - creations_before,
			1,
			"instance was not reused across sequential calls"
		);
	}

	#[test]
	fn test_reused_instance_parallel() {
		// Exercise the per-thread reuse across rayon worker threads
		use rayon::prelude::*;

		(0..32).into_par_iter().for_each(|i| {
			let input = format!("void g(){{var y={i};print(y);}}");
			let formatted = format_file(&input, "test.dart", FileType::Dart)
				.unwrap_or_else(|e| panic!("call {i} failed: {e}"));
			assert!(
				formatted.contains(&format!("var y = {i};")),
				"call {i} wrong body: {formatted}"
			);
		});
	}

	#[test]
	fn test_options_json_shape() {
		let options = build_options_json();
		assert!(options.contains("\"pageWidth\":80"), "Options: {options}");
		assert!(
			options.contains("\"lineEnding\":\"\\n\""),
			"Options: {options}"
		);
	}

	/// Measures per-file latency across small/mid/large Dart sources.
	/// Run with: cargo test -p fama-dart --release -- --ignored --nocapture
	#[test]
	#[ignore = "benchmark"]
	fn bench_per_file_latency() {
		let small = "void f(){print('x');}";
		let mid: String = (0..100)
			.map(|i| {
				format!("class C{i} {{\n  void m{i}() {{print({i});}}\n}}\n")
			})
			.collect();
		let large: String = (0..1000)
			.map(|i| {
				format!(
					"class Widget{i} extends StatelessWidget {{\n  final int id;\n  Widget build(BuildContext ctx) => Container(child: Text('{i}'));\n}}\n"
				)
			})
			.collect();

		// Warm up (engine compile + instantiation)
		format_file(small, "warm.dart", FileType::Dart).unwrap();
		shims::profile_reset();

		let runs = 1;
		for (name, source) in [
			("small", small),
			("mid", mid.as_str()),
			("large", large.as_str()),
		] {
			shims::profile_reset();
			let start = std::time::Instant::now();
			for _ in 0..runs {
				format_file(source, "bench.dart", FileType::Dart)
					.unwrap_or_else(|e| panic!("{name} bench failed: {e}"));
			}
			let elapsed = start.elapsed() / runs;
			println!(
				"{name:>5} ({} bytes): {:?} per file",
				source.len(),
				elapsed
			);
			let mut stats: Vec<_> = shims::profile_snapshot();
			stats.sort_by_key(|(_, _calls, nanos)| std::cmp::Reverse(*nanos));
			for (k, calls, nanos) in stats.iter().take(12) {
				println!(
					"      {k}: {calls} calls, {:.1}ms total",
					*nanos as f64 / 1_000_000.0
				);
			}
		}
	}
}
