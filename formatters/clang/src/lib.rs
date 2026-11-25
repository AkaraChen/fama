//! Clang-format WASM-based formatter for C/C++/Objective-C/Java/Protobuf/C#
//!
//! This formatter uses a standalone WASM module compiled from clang-format
//! and runs it via wasmi with WASI support.

use std::cell::RefCell;
use std::sync::OnceLock;

use fama_common::{FileType, IndentStyle, CONFIG};
use wasmi::{Engine, Instance, Linker, Memory, Module, Store, TypedFunc};
use wasmi_wasi::{WasiCtx, WasiCtxBuilder};

/// Embedded clang-format WASM binary
const CLANG_FORMAT_WASM: &[u8] = include_bytes!("../wasm/clang-format.wasm");

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

/// Store context for WASI + our custom imports
struct StoreCtx {
	wasi: WasiCtx,
}

/// Cached WASM instance for reuse
struct CachedInstance {
	engine: Engine,
	module: Module,
}

static CACHED_MODULE: OnceLock<CachedInstance> = OnceLock::new();

fn get_cached_module() -> &'static CachedInstance {
	CACHED_MODULE.get_or_init(|| {
		let engine = Engine::default();
		let module = Module::new(&engine, CLANG_FORMAT_WASM)
			.expect("Failed to compile WASM module");
		CachedInstance { engine, module }
	})
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
	/// Per-thread instance pool (wasmi's `Store` is `Send` but not `Sync`).
	/// Reusing an instance across files skips instantiation, the Emscripten
	/// runtime start, `wasm_init` and `wasm_set_style` for every file after
	/// the first one on each rayon worker thread.
	static INSTANCE: RefCell<Option<ReusableInstance>> =
		const { RefCell::new(None) };
}

/// Counter of instance creations on this thread, for asserting reuse in tests
#[cfg(test)]
thread_local! {
	static INSTANCE_CREATIONS: std::cell::Cell<usize> =
		const { std::cell::Cell::new(0) };
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

	// Create WASI context
	let wasi = WasiCtxBuilder::new().inherit_stdio().build();

	let ctx = StoreCtx { wasi };
	let mut store = Store::new(&cached.engine, ctx);

	// Create linker with WASI
	let mut linker = <Linker<StoreCtx>>::new(&cached.engine);

	// Add WASI imports
	wasmi_wasi::add_to_linker(&mut linker, |ctx: &mut StoreCtx| &mut ctx.wasi)
		.map_err(|e| format!("Failed to add WASI to linker: {}", e))?;

	// Add Emscripten-specific stubs
	add_emscripten_stubs(&mut linker)?;

	// Instantiate the module
	let instance = linker
		.instantiate(&mut store, &cached.module)
		.map_err(|e| format!("Failed to instantiate module: {}", e))?
		.start(&mut store)
		.map_err(|e| format!("Failed to start module: {}", e))?;

	// Get memory export
	let memory = instance
		.get_memory(&store, "memory")
		.ok_or_else(|| "Failed to get memory export".to_string())?;

	// Initialize the formatter
	let init: TypedFunc<(), ()> = instance
		.get_typed_func(&store, "wasm_init")
		.map_err(|e| format!("Failed to get wasm_init: {}", e))?;

	init.call(&mut store, ())
		.map_err(|e| format!("Failed to call wasm_init: {}", e))?;

	// Set formatting style based on fama config
	let style = generate_style_config();
	let style_ptr =
		write_string_to_memory(&mut store, &memory, &instance, &style)?;
	let style_len = style.len() as i32;

	let set_style: TypedFunc<(i32, i32), i32> = instance
		.get_typed_func(&store, "wasm_set_style")
		.map_err(|e| format!("Failed to get wasm_set_style: {}", e))?;

	set_style
		.call(&mut store, (style_ptr, style_len))
		.map_err(|e| format!("Failed to set style: {}", e))?;

	// Free style string memory
	let free: TypedFunc<i32, ()> = instance
		.get_typed_func(&store, "free")
		.map_err(|e| format!("Failed to get free: {}", e))?;

	free.call(&mut store, style_ptr)
		.map_err(|e| format!("Failed to free style: {}", e))?;

	Ok((store, instance, memory))
}

/// Add Emscripten-specific stub functions
fn add_emscripten_stubs(linker: &mut Linker<StoreCtx>) -> Result<(), String> {
	// emscripten_notify_memory_growth - called when memory grows
	linker
		.func_wrap("env", "emscripten_notify_memory_growth", |_: i32| {
			// No-op - we don't need to do anything when memory grows
		})
		.map_err(|e| {
			format!("Failed to add emscripten_notify_memory_growth: {}", e)
		})?;

	// __syscall_getcwd - get current working directory (stub)
	linker
		.func_wrap("env", "__syscall_getcwd", |_buf: i32, _size: i32| -> i32 {
			-1i32 // Return error - we don't support this
		})
		.map_err(|e| format!("Failed to add __syscall_getcwd: {}", e))?;

	// __syscall_chdir - change directory (stub)
	linker
		.func_wrap("env", "__syscall_chdir", |_path: i32| -> i32 {
			-1i32 // Return error
		})
		.map_err(|e| format!("Failed to add __syscall_chdir: {}", e))?;

	// __syscall_faccessat - check file access (stub)
	linker
		.func_wrap(
			"env",
			"__syscall_faccessat",
			|_dirfd: i32, _path: i32, _mode: i32, _flags: i32| -> i32 {
				-1i32 // Return error
			},
		)
		.map_err(|e| format!("Failed to add __syscall_faccessat: {}", e))?;

	// __syscall_statfs64 - get filesystem stats (stub)
	linker
		.func_wrap(
			"env",
			"__syscall_statfs64",
			|_path: i32, _size: i32, _buf: i32| -> i32 {
				-1i32 // Return error
			},
		)
		.map_err(|e| format!("Failed to add __syscall_statfs64: {}", e))?;

	// __syscall_unlinkat - remove file (stub)
	linker
		.func_wrap(
			"env",
			"__syscall_unlinkat",
			|_dirfd: i32, _path: i32, _flags: i32| -> i32 {
				-1i32 // Return error
			},
		)
		.map_err(|e| format!("Failed to add __syscall_unlinkat: {}", e))?;

	// __syscall_readlinkat - read symbolic link (stub)
	linker
		.func_wrap(
			"env",
			"__syscall_readlinkat",
			|_dirfd: i32, _path: i32, _buf: i32, _bufsize: i32| -> i32 {
				-1i32 // Return error
			},
		)
		.map_err(|e| format!("Failed to add __syscall_readlinkat: {}", e))?;

	// __syscall_getdents64 - read directory entries (stub)
	linker
		.func_wrap(
			"env",
			"__syscall_getdents64",
			|_fd: i32, _dirp: i32, _count: i32| -> i32 {
				-1i32 // Return error
			},
		)
		.map_err(|e| format!("Failed to add __syscall_getdents64: {}", e))?;

	Ok(())
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
		.get_typed_func(&*store, "malloc")
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
