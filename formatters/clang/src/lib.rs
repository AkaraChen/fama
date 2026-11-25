//! Clang-format WASM-based formatter for C/C++/Objective-C/Java/Protobuf/C#
//!
//! This formatter uses a standalone WASM module compiled from clang-format
//! and runs it via wasmtime (Cranelift JIT). clang-format execution is
//! compute-intensive work that repeats for every file, which makes a JIT
//! runtime the right fit; measured 13x faster per file than the wasmi
//! interpreter this crate used previously.

use std::cell::RefCell;
use std::path::PathBuf;
use std::sync::OnceLock;

use fama_common::{FileType, IndentStyle, CONFIG};
use wasmtime::{Engine, Instance, Linker, Memory, Module, Store, TypedFunc};

/// Embedded clang-format WASM binary
const CLANG_FORMAT_WASM: &[u8] = include_bytes!("../wasm/clang-format.wasm");

/// sha256 prefix of the embedded wasm bytes; binds precompiled artifacts to
/// the exact module they were compiled from
fn artifact_hash() -> String {
	use sha2::{Digest, Sha256};
	let mut hasher = Sha256::new();
	hasher.update(CLANG_FORMAT_WASM);
	let digest = hasher.finalize();
	digest.iter().map(|b| format!("{b:02x}")).collect::<String>()
		[..16].to_string()
}

/// Filename of the precompiled module artifact for this fama build
pub fn precompiled_artifact_name() -> String {
	format!("clang-format-{}.wasmu", artifact_hash())
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
	let home = std::env::var_os("HOME")
		.or_else(|| std::env::var_os("USERPROFILE"));
	if let Some(home) = home {
		paths.push(PathBuf::from(home).join(".cache/fama").join(&name));
	}
	paths
}

/// Build the engine, compile the embedded module and return the serialized
/// precompiled artifact (used by release CI to ship per-platform `.wasmu`
/// files next to the fama binary)
pub fn serialize_compiled_module() -> Result<Vec<u8>, String> {
	let mut cfg = wasmtime::Config::new();
	cfg.wasm_backtrace_max_frames(None);
	let engine = wasmtime::Engine::new(&cfg)
		.expect("Failed to create wasmtime engine");
	let module = Module::new(&engine, CLANG_FORMAT_WASM)
		.expect("Failed to compile WASM module");
	module.serialize().map_err(|e| format!("Failed to serialize module: {e:?}"))
}

/// Generate clang-format style configuration based on fama's FormatConfig
fn generate_style_config() -> String {
	let use_tab = matches!(CONFIG.indent_style, IndentStyle::Tabs);
	let indent_width = CONFIG.indent_width;
	let column_limit = CONFIG.line_width;

	// YAML-style inline config for clang-format
	format!(
		"{{BasedOnStyle: LLVM, \
		UseTab: {}, \
		IndentWidth: {}, \
		TabWidth: {}, \
		ColumnLimit: {}}}",
		if use_tab { "Always" } else { "Never" },
		indent_width,
		indent_width,
		column_limit,
	)
}

/// Store context for the WASM instance
struct StoreCtx;

// Counter of instance creations on this thread, for asserting reuse in tests
#[cfg(test)]
thread_local! {
	static INSTANCE_CREATIONS: std::cell::Cell<usize> =
		const { std::cell::Cell::new(0) };
}

struct CachedInstance {
	engine: Engine,
	module: Module,
}

static CACHED_MODULE: OnceLock<CachedInstance> = OnceLock::new();

fn get_cached_module() -> &'static CachedInstance {
	CACHED_MODULE.get_or_init(|| {
		let mut cfg = wasmtime::Config::new();
		// wasmtime's default backtrace capture adds per-call overhead on
		// this hot path and clang-format failures are surfaced as errors
		// anyway; the diagnostics value does not justify it here
		cfg.wasm_backtrace_max_frames(None);
		let engine = wasmtime::Engine::new(&cfg)
			.expect("Failed to create wasmtime engine");

		// Prefer a precompiled artifact (shipped with releases or cached by
		// a previous run): deserializing takes milliseconds versus ~1.5s to
		// compile the 2.7MB module with Cranelift
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
				unsafe { Module::deserialize_file(&engine, &path) }
			{
				return CachedInstance { engine, module };
			}
		}

		let module = Module::new(&engine, CLANG_FORMAT_WASM)
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

		CachedInstance { engine, module }
	})
}

/// The WASM module's imports. clang-format only touches WASI for stdio and
/// filesystem access, which never happens on the success path (results come
/// back through linear memory), so every host function is a stub.
fn define_imports(linker: &mut Linker<StoreCtx>) -> Result<(), String> {
	// wasi_snapshot_preview1
	linker
		.func_wrap("wasi_snapshot_preview1", "proc_exit", proc_exit)
		.map_err(|e| format!("Failed to add proc_exit: {}", e))?;
	linker
		.func_wrap("wasi_snapshot_preview1", "fd_close", fd1)
		.map_err(|e| format!("Failed to add fd_close: {}", e))?;
	linker
		.func_wrap("wasi_snapshot_preview1", "fd_write", fd4)
		.map_err(|e| format!("Failed to add fd_write: {}", e))?;
	linker
		.func_wrap("wasi_snapshot_preview1", "fd_read", fd4)
		.map_err(|e| format!("Failed to add fd_read: {}", e))?;
	linker
		.func_wrap("wasi_snapshot_preview1", "fd_pread", fd_pread)
		.map_err(|e| format!("Failed to add fd_pread: {}", e))?;
	linker
		.func_wrap("wasi_snapshot_preview1", "fd_seek", fd_seek)
		.map_err(|e| format!("Failed to add fd_seek: {}", e))?;
	linker
		.func_wrap("wasi_snapshot_preview1", "fd_fdstat_get", fd2)
		.map_err(|e| format!("Failed to add fd_fdstat_get: {}", e))?;
	linker
		.func_wrap("wasi_snapshot_preview1", "environ_get", fd2)
		.map_err(|e| format!("Failed to add environ_get: {}", e))?;
	linker
		.func_wrap("wasi_snapshot_preview1", "environ_sizes_get", fd2)
		.map_err(|e| format!("Failed to add environ_sizes_get: {}", e))?;
	linker
		.func_wrap("wasi_snapshot_preview1", "clock_time_get", clock_time_get)
		.map_err(|e| format!("Failed to add clock_time_get: {}", e))?;

	// Emscripten runtime hooks
	linker
		.func_wrap("env", "emscripten_notify_memory_growth", |_: i32| {})
		.map_err(|e| format!("Failed to add emscripten_notify_memory_growth: {}", e))?;
	linker
		.func_wrap("env", "__syscall_getcwd", fd2)
		.map_err(|e| format!("Failed to add __syscall_getcwd: {}", e))?;
	linker
		.func_wrap("env", "__syscall_chdir", fd1)
		.map_err(|e| format!("Failed to add __syscall_chdir: {}", e))?;
	linker
		.func_wrap("env", "__syscall_faccessat", fd4)
		.map_err(|e| format!("Failed to add __syscall_faccessat: {}", e))?;
	linker
		.func_wrap("env", "__syscall_statfs64", fd3)
		.map_err(|e| format!("Failed to add __syscall_statfs64: {}", e))?;
	linker
		.func_wrap("env", "__syscall_unlinkat", fd3)
		.map_err(|e| format!("Failed to add __syscall_unlinkat: {}", e))?;
	linker
		.func_wrap("env", "__syscall_readlinkat", fd4)
		.map_err(|e| format!("Failed to add __syscall_readlinkat: {}", e))?;
	linker
		.func_wrap("env", "__syscall_getdents64", fd3)
		.map_err(|e| format!("Failed to add __syscall_getdents64: {}", e))?;

	Ok(())
}

fn proc_exit(_code: i32) -> wasmtime::Result<()> {
	wasmtime::bail!("proc_exit called")
}

fn fd1(_a: i32) -> i32 {
	-1
}

fn fd2(_a: i32, _b: i32) -> i32 {
	-1
}

fn fd3(_a: i32, _b: i32, _c: i32) -> i32 {
	-1
}

fn fd4(_a: i32, _b: i32, _c: i32, _d: i32) -> i32 {
	-1
}

fn fd_pread(_a: i32, _b: i32, _c: i32, _d: i64, _e: i32) -> i32 {
	-1
}

fn fd_seek(_a: i32, _b: i64, _c: i32, _d: i32) -> i32 {
	-1
}

fn clock_time_get(_a: i32, _b: i64, _c: i32) -> i32 {
	-1
}

/// A fully initialized WASM instance ready to format files.
///
/// The WASM side keeps a global `ClangFormat` formatter and frees the previous
/// result at the start of every `wasm_format` call, so one instance can serve
/// any number of files sequentially.
struct ReusableInstance {
	store: Store<StoreCtx>,
	instance: Instance,
	memory: Memory,
}

thread_local! {
	/// Per-thread instance pool (wasmtime's `Store` is `Send` but not `Sync`).
	/// Reusing an instance across files skips instantiation and the Emscripten
	/// runtime start for every file after the first one on each rayon worker.
	static INSTANCE: RefCell<Option<ReusableInstance>> =
		const { RefCell::new(None) };
}

/// Run `f` with a reusable per-thread instance.
///
/// The instance is kept for future calls on success. On error it is dropped,
/// so the next call starts from a fresh instance instead of a poisoned one.
fn with_reused_instance<R>(
	f: impl FnOnce(&mut Store<StoreCtx>, &Instance, &Memory) -> Result<R, String>,
) -> Result<R, String> {
	INSTANCE.with(|slot| {
		let mut slot = slot.borrow_mut();

		let mut reusable = match slot.take() {
			Some(reusable) => Some(reusable),
			None => {
				#[cfg(test)]
				INSTANCE_CREATIONS.with(|count| count.set(count.get() + 1));
				match create_instance() {
					Ok((store, instance, memory)) => Some(ReusableInstance {
						store,
						instance,
						memory,
					}),
					Err(e) => return Err(e),
				}
			}
		};

		// Destructure into disjoint field references for the call
		let ReusableInstance {
			store,
			instance,
			memory,
		} = reusable.as_mut().unwrap();

		match f(store, instance, memory) {
			Ok(result) => {
				*slot = reusable.take();
				Ok(result)
			}
			Err(error) => Err(error),
		}
	})
}

/// Create a new store and instance for formatting
fn create_instance() -> Result<(Store<StoreCtx>, Instance, Memory), String> {
	let cached = get_cached_module();

	let mut store = Store::new(&cached.engine, StoreCtx);

	let mut linker: Linker<StoreCtx> = Linker::new(&cached.engine);
	define_imports(&mut linker)?;

	// Instantiate the module (the start function runs automatically)
	let instance = linker
		.instantiate(&mut store, &cached.module)
		.map_err(|e| format!("Failed to instantiate module: {}", e))?;
	let memory = instance
		.get_memory(&mut store, "memory")
		.ok_or_else(|| "Failed to get memory export".to_string())?;

	// Initialize the formatter
	let init: TypedFunc<(), ()> = instance
		.get_typed_func(&mut store, "wasm_init")
		.map_err(|e| format!("Failed to get wasm_init: {}", e))?;

	init.call(&mut store, ())
		.map_err(|e| format!("Failed to call wasm_init: {}", e))?;

	// Set formatting style based on fama config
	let style = generate_style_config();
	let style_ptr =
		write_string_to_memory(&mut store, &memory, &instance, &style)?;
	let style_len = style.len() as i32;

	let set_style: TypedFunc<(i32, i32), i32> = instance
		.get_typed_func(&mut store, "wasm_set_style")
		.map_err(|e| format!("Failed to get wasm_set_style: {}", e))?;

	set_style
		.call(&mut store, (style_ptr, style_len))
		.map_err(|e| format!("Failed to set style: {}", e))?;

	// Free style string memory
	let free: TypedFunc<i32, ()> = instance
		.get_typed_func(&mut store, "free")
		.map_err(|e| format!("Failed to get free: {}", e))?;

	free.call(&mut store, style_ptr)
		.map_err(|e| format!("Failed to free style: {}", e))?;

	Ok((store, instance, memory))
}

/// Write a string to WASM memory and return the pointer
fn write_string_to_memory(
	store: &mut Store<StoreCtx>,
	memory: &Memory,
	instance: &Instance,
	s: &str,
) -> Result<i32, String> {
	let bytes = s.as_bytes();
	let len = bytes.len() as i32;

	// Allocate memory using malloc
	let malloc: TypedFunc<i32, i32> = instance
		.get_typed_func(&mut *store, "malloc")
		.map_err(|e| format!("Failed to get malloc: {}", e))?;

	let ptr = malloc
		.call(&mut *store, len)
		.map_err(|e| format!("Failed to allocate memory: {}", e))?;

	if ptr == 0 {
		return Err("malloc returned null".to_string());
	}

	// Write bytes to memory
	memory
		.write(&mut *store, ptr as usize, bytes)
		.map_err(|e| format!("Failed to write to memory: {}", e))?;

	Ok(ptr)
}

/// Read a string from WASM memory
fn read_string_from_memory(
	store: &Store<StoreCtx>,
	memory: &Memory,
	ptr: i32,
	len: i32,
) -> Result<String, String> {
	if ptr == 0 || len == 0 {
		return Ok(String::new());
	}

	let mut buffer = vec![0u8; len as usize];
	memory
		.read(store, ptr as usize, &mut buffer)
		.map_err(|e| format!("Failed to read from memory: {}", e))?;

	String::from_utf8(buffer).map_err(|e| format!("Invalid UTF-8: {}", e))
}

/// Format code using clang-format WASM
///
/// # Arguments
/// * `content` - The source code to format
/// * `path` - The file path (used to determine language)
/// * `file_type` - The detected file type
///
/// # Returns
/// * `Ok(String)` - The formatted code
/// * `Err(String)` - Error message if formatting failed
pub fn format_file(
	content: &str,
	path: &str,
	_file_type: FileType,
) -> Result<String, String> {
	with_reused_instance(|store, instance, memory| {
		// Write input strings to WASM memory
		let code_ptr =
			write_string_to_memory(&mut *store, memory, instance, content)?;
		let code_len = content.len() as i32;

		let filename_ptr =
			write_string_to_memory(&mut *store, memory, instance, path)?;
		let filename_len = path.len() as i32;

		// Get the format function
		let format: TypedFunc<(i32, i32, i32, i32), i32> = instance
			.get_typed_func(&mut *store, "wasm_format")
			.map_err(|e| format!("Failed to get wasm_format: {}", e))?;

		// Call format
		let status = format
			.call(&mut *store, (code_ptr, code_len, filename_ptr, filename_len))
			.map_err(|e| format!("Failed to call wasm_format: {}", e))?;

		// Free input memory
		let free: TypedFunc<i32, ()> = instance
			.get_typed_func(&mut *store, "free")
			.map_err(|e| format!("Failed to get free: {}", e))?;

		free.call(&mut *store, code_ptr)
			.map_err(|e| format!("Failed to free code: {}", e))?;
		free.call(&mut *store, filename_ptr)
			.map_err(|e| format!("Failed to free filename: {}", e))?;

		match status {
			0 => {
				// Success - get the result
				let result = read_result(&mut *store, memory, instance)?;
				Ok(result)
			}
			1 => {
				// Error - get error message
				let error_msg = read_result(&mut *store, memory, instance)?;
				Err(error_msg)
			}
			2 => {
				// Unchanged - return original content
				Ok(content.to_string())
			}
			_ => Err(format!("Unknown status code: {}", status)),
		}
	})
}

/// Read the result (or error message) written by the last `wasm_format` call
/// and free it on the WASM side
fn read_result(
	store: &mut Store<StoreCtx>,
	memory: &Memory,
	instance: &Instance,
) -> Result<String, String> {
	let get_ptr: TypedFunc<(), i32> = instance
		.get_typed_func(&mut *store, "wasm_get_result_ptr")
		.map_err(|e| format!("Failed to get wasm_get_result_ptr: {}", e))?;
	let get_len: TypedFunc<(), i32> = instance
		.get_typed_func(&mut *store, "wasm_get_result_len")
		.map_err(|e| format!("Failed to get wasm_get_result_len: {}", e))?;
	let free_result: TypedFunc<(), ()> = instance
		.get_typed_func(&mut *store, "wasm_free_result")
		.map_err(|e| format!("Failed to get wasm_free_result: {}", e))?;

	let result_ptr = get_ptr
		.call(&mut *store, ())
		.map_err(|e| format!("Failed to get result ptr: {}", e))?;
	let result_len = get_len
		.call(&mut *store, ())
		.map_err(|e| format!("Failed to get result len: {}", e))?;

	let result =
		read_string_from_memory(&*store, memory, result_ptr, result_len)?;

	free_result
		.call(&mut *store, ())
		.map_err(|e| format!("Failed to free result: {}", e))?;

	Ok(result)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_format_simple_c() {
		let input = "int main(){return 0;}";
		let result = format_file(input, "test.c", FileType::Unknown);
		assert!(result.is_ok(), "Format failed: {:?}", result);
		let formatted = result.unwrap();
		assert!(formatted.contains("int main()"), "Output: {}", formatted);
	}

	#[test]
	fn test_format_cpp() {
		let input = "class Foo{public:void bar(){}};";
		let result = format_file(input, "test.cpp", FileType::Unknown);
		assert!(result.is_ok(), "Format failed: {:?}", result);
	}

	#[test]
	fn test_format_uses_tabs() {
		// Test that tabs are used for indentation (per CONFIG)
		let input = "int main() {\nint x = 1;\nreturn x;\n}";
		let result = format_file(input, "test.c", FileType::C);
		assert!(result.is_ok(), "Format failed: {:?}", result);
		let formatted = result.unwrap();
		// Check that output uses tabs for indentation
		assert!(
			formatted.contains("\tint x"),
			"Expected tab indentation, got: {}",
			formatted
		);
	}

	#[test]
	fn test_style_config_generation() {
		let style = generate_style_config();
		// Verify config matches fama settings
		assert!(style.contains("UseTab: Always"), "Style: {}", style);
		assert!(style.contains("IndentWidth: 4"), "Style: {}", style);
		assert!(style.contains("ColumnLimit: 80"), "Style: {}", style);
	}

	#[test]
	fn test_reused_instance_formats_repeatedly() {
		// The per-thread instance is reused across calls; every call must
		// still produce a correct, independent result.
		let creations_before = INSTANCE_CREATIONS.with(|count| count.get());
		for i in 0..25 {
			let input = format!("int main(){{int x={i};return x;}}");
			let formatted = format_file(&input, "test.c", FileType::C)
				.unwrap_or_else(|e| panic!("call {i} failed: {e}"));
			assert!(
				formatted.contains("int main()"),
				"call {i} lost main: {formatted}"
			);
			assert!(
				formatted.contains(&format!("x = {i};")),
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
			let input = format!("int main(){{int y={i};return y;}}");
			let formatted =
				format_file(&input, "test.c", FileType::C)
					.unwrap_or_else(|e| panic!("call {i} failed: {e}"));
			assert!(
				formatted.contains(&format!("y = {i};")),
				"call {i} wrong body: {formatted}"
			);
		});
	}
}
