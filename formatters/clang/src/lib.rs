//! Clang-format WASM-based formatter for C/C++/Objective-C/Java/Protobuf/C#
//!
//! This formatter uses a standalone WASM module compiled from clang-format
//! and runs it via wasmtime (Cranelift JIT). clang-format execution is
//! compute-intensive work that repeats for every file, which makes a JIT
//! runtime the right fit; measured 13x faster per file than the wasmi
//! interpreter this crate used previously.

use std::borrow::Cow;
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use fama_common::{Formatter, IndentStyle, CONFIG};
use wasmtime::{Engine, Linker, Memory, Module, Store, TypedFunc};

/// Zstd-compressed embedded clang-format WASM binary
const CLANG_FORMAT_WASM_ZST: &[u8] =
	include_bytes!("../wasm/clang-format.wasm.zst");

const ARTIFACT_PREFIX: &str = "clang-format";

/// Compile-time sha256 prefix of `wasm/clang-format.wasm.zst`; binds precompiled
/// artifacts to the exact module with zero runtime decompression or hashing.
fn artifact_hash() -> &'static str {
	env!("CLANG_FORMAT_WASM_HASH")
}

fn decompress_wasm() -> Cow<'static, [u8]> {
	Cow::Owned(
		fama_wasm::decompress_zstd(CLANG_FORMAT_WASM_ZST)
			.expect("failed to decompress clang-format module"),
	)
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
	linker: Linker<StoreCtx>,
}

static CACHED_MODULE: OnceLock<CachedInstance> = OnceLock::new();

fn get_cached_module() -> &'static CachedInstance {
	CACHED_MODULE.get_or_init(|| {
		let engine = Engine::new(&fama_wasm::default_engine_config())
			.expect("Failed to create wasmtime engine");
		let mut linker: Linker<StoreCtx> = Linker::new(&engine);
		define_imports(&mut linker)
			.expect("Failed to register clang-format WASI imports");

		// Prefer a precompiled artifact (shipped with releases or cached by
		// a previous run): deserializing takes milliseconds versus ~1.5s to
		// compile the 2.7MB module with Cranelift
		let module = fama_wasm::load_or_compile(
			&engine,
			ARTIFACT_PREFIX,
			artifact_hash(),
			decompress_wasm,
		)
		.expect("Failed to compile WASM module");

		CachedInstance {
			engine,
			module,
			linker,
		}
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
	memory: Memory,
	malloc: TypedFunc<i32, i32>,
	free: TypedFunc<i32, ()>,
	wasm_format: TypedFunc<(i32, i32, i32, i32), i32>,
	get_result_ptr: TypedFunc<(), i32>,
	get_result_len: TypedFunc<(), i32>,
	free_result: TypedFunc<(), ()>,
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
	f: impl FnOnce(&mut ReusableInstance) -> Result<R, String>,
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

		let result = f(reusable.as_mut().unwrap());
		if result.is_ok() {
			*slot = reusable.take();
		}
		result
	})
}

/// Create a new store and instance for formatting
fn create_instance() -> Result<ReusableInstance, String> {
	let cached = get_cached_module();

	let mut store = Store::new(&cached.engine, StoreCtx);

	// Instantiate the module (the start function runs automatically)
	let instance = cached
		.linker
		.instantiate(&mut store, &cached.module)
		.map_err(|e| format!("Failed to instantiate module: {}", e))?;
	let memory = instance
		.get_memory(&mut store, "memory")
		.ok_or_else(|| "Failed to get memory export".to_string())?;

	macro_rules! typed {
		($name:literal, $params:ty => $results:ty) => {
			instance
				.get_typed_func::<$params, $results>(&mut store, $name)
				.map_err(|e| format!("Failed to get {}: {}", $name, e))?
		};
	}

	let init: TypedFunc<(), ()> = typed!("wasm_init", () => ());
	let set_style: TypedFunc<(i32, i32), i32> =
		typed!("wasm_set_style", (i32, i32) => i32);
	let malloc: TypedFunc<i32, i32> = typed!("malloc", i32 => i32);
	let free: TypedFunc<i32, ()> = typed!("free", i32 => ());
	let wasm_format: TypedFunc<(i32, i32, i32, i32), i32> =
		typed!("wasm_format", (i32, i32, i32, i32) => i32);
	let get_result_ptr: TypedFunc<(), i32> =
		typed!("wasm_get_result_ptr", () => i32);
	let get_result_len: TypedFunc<(), i32> =
		typed!("wasm_get_result_len", () => i32);
	let free_result: TypedFunc<(), ()> = typed!("wasm_free_result", () => ());

	init.call(&mut store, ())
		.map_err(|e| format!("Failed to call wasm_init: {}", e))?;

	let mut reusable = ReusableInstance {
		store,
		memory,
		malloc,
		free,
		wasm_format,
		get_result_ptr,
		get_result_len,
		free_result,
	};

	// Set formatting style based on fama config
	let style = generate_style_config();
	let style_ptr = write_string_to_memory(&mut reusable, &style)?;
	let style_len = style.len() as i32;

	reusable
		.set_style_and_free(style_ptr, style_len, set_style)?;

	Ok(reusable)
}

impl ReusableInstance {
	fn set_style_and_free(
		&mut self,
		style_ptr: i32,
		style_len: i32,
		set_style: TypedFunc<(i32, i32), i32>,
	) -> Result<(), String> {
		set_style
			.call(&mut self.store, (style_ptr, style_len))
			.map_err(|e| format!("Failed to set style: {}", e))?;
		self.free
			.call(&mut self.store, style_ptr)
			.map_err(|e| format!("Failed to free style: {}", e))
	}
}

/// Write a string to WASM memory and return the pointer
fn write_string_to_memory(
	inst: &mut ReusableInstance,
	s: &str,
) -> Result<i32, String> {
	let bytes = s.as_bytes();
	let len = bytes.len() as i32;

	let ptr = inst
		.malloc
		.call(&mut inst.store, len)
		.map_err(|e| format!("Failed to allocate memory: {}", e))?;

	if ptr == 0 {
		return Err("malloc returned null".to_string());
	}

	// Write bytes to memory
	inst.memory
		.write(&mut inst.store, ptr as usize, bytes)
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

/// Formatter implementation for C-family languages via clang-format WASM.
pub struct ClangFormatter;

/// Static formatter instance registered by `fama`.
pub static FORMATTER: ClangFormatter = ClangFormatter;

impl Formatter for ClangFormatter {
	fn name(&self) -> &'static str {
		"clang"
	}

	fn extensions(&self) -> &'static [&'static str] {
		&[
			"c", "h", "cpp", "cc", "cxx", "hpp", "hxx", "hh", "m", "mm",
			"java", "proto", "cu", "cuh", "hlsl", "hlsli", "metal", "glsl",
			"vert", "frag", "comp", "geom", "tesc", "tese", "sv", "svh", "vh",
			"v",
		]
	}

	fn needs_content_check(&self, ext: &str) -> bool {
		matches!(ext, "v" | "m")
	}

	fn match_confidence(&self, path: &Path, content: &str) -> u8 {
		match path.extension().and_then(|e| e.to_str()) {
			Some("v") => verilog_confidence(content),
			Some("m") => objc_confidence(content),
			_ => 50,
		}
	}

	fn format(&self, content: &str, path: &Path) -> Result<String, String> {
		let ext = path.extension().and_then(|e| e.to_str());
		// LLVM clang-format maps .v/.sv/.svh to Verilog, while .vh may need an
		// explicit .svh hint so it doesn't fall back to C++.
		let assumed_path = if ext == Some("vh") {
			Cow::Owned(path.with_extension("svh"))
		} else {
			Cow::Borrowed(path)
		};
		format_file(content, assumed_path.to_str().unwrap_or(""))
	}

	fn precompile(&self, output_dir: &Path) -> Option<Result<PathBuf, String>> {
		let result = Engine::new(&fama_wasm::default_engine_config())
			.map_err(|e| format!("Failed to create wasmtime engine: {e}"))
			.and_then(|engine| {
				fama_wasm::precompile_to_dir(
					&engine,
					ARTIFACT_PREFIX,
					artifact_hash(),
					decompress_wasm,
					output_dir,
				)
			});
		Some(result)
	}
}

fn verilog_confidence(content: &str) -> u8 {
	const COQ_MARKERS: &[&str] = &[
		"Theorem ",
		"Lemma ",
		"Proof.",
		"Qed.",
		"Require Import",
		"Inductive ",
		"Definition ",
		"Fixpoint ",
	];
	if COQ_MARKERS.iter().any(|m| content.contains(m)) {
		return 0;
	}

	// V language uses `module <name>` too, so check V markers before `module `.
	let has_verilog_end = content.contains("endmodule");
	if !has_verilog_end
		&& (content.contains("fn main(")
			|| content.contains("pub fn ")
			|| (content.lines().any(|l| l.trim_start().starts_with("import "))
				&& !content.contains("import pkg")))
	{
		return 0;
	}

	const VERILOG_MARKERS: &[&str] = &[
		"module ",
		"endmodule",
		"wire ",
		"reg ",
		"logic ",
		"always",
		"assign ",
		"`timescale",
		"`include",
		"input ",
		"output ",
		"parameter ",
		"localparam ",
	];
	if VERILOG_MARKERS.iter().any(|m| content.contains(m)) {
		return 90;
	}

	50
}

fn objc_confidence(content: &str) -> u8 {
	const OBJC_MARKERS: &[&str] = &[
		"@interface",
		"@implementation",
		"@protocol",
		"#import",
		"@property",
		"@end",
		"NSString",
		"NSObject",
		"#include",
	];
	if OBJC_MARKERS.iter().any(|m| content.contains(m)) {
		return 90;
	}

	let has_matlab_comment =
		content.lines().any(|l| l.trim_start().starts_with('%'));
	let has_matlab_end = content.lines().any(|l| l.trim() == "end");
	let has_c_syntax = content.contains('{')
		|| content.contains('}')
		|| content.contains('@')
		|| content.contains("#include");
	if !has_c_syntax
		&& (has_matlab_comment
			|| (content.contains("function ") && has_matlab_end))
	{
		return 0;
	}

	50
}

/// Format code using clang-format WASM
///
/// # Arguments
/// * `content` - The source code to format
/// * `path` - The file path (used to determine language)
///
/// # Returns
/// * `Ok(String)` - The formatted code
/// * `Err(String)` - Error message if formatting failed
pub fn format_file(content: &str, path: &str) -> Result<String, String> {
	with_reused_instance(|inst| {
		// Write input strings to WASM memory
		let code_ptr = write_string_to_memory(inst, content)?;
		let code_len = content.len() as i32;

		let filename_ptr = write_string_to_memory(inst, path)?;
		let filename_len = path.len() as i32;

		// Call format
		let status = inst
			.wasm_format
			.call(&mut inst.store, (code_ptr, code_len, filename_ptr, filename_len))
			.map_err(|e| format!("Failed to call wasm_format: {}", e))?;

		// Free input memory
		inst.free
			.call(&mut inst.store, code_ptr)
			.map_err(|e| format!("Failed to free code: {}", e))?;
		inst.free
			.call(&mut inst.store, filename_ptr)
			.map_err(|e| format!("Failed to free filename: {}", e))?;

		match status {
			0 => {
				// Success - get the result
				read_result(inst)
			}
			1 => {
				// Error - get error message
				let error_msg = read_result(inst)?;
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
fn read_result(inst: &mut ReusableInstance) -> Result<String, String> {
	let result_ptr = inst
		.get_result_ptr
		.call(&mut inst.store, ())
		.map_err(|e| format!("Failed to get result ptr: {}", e))?;
	let result_len = inst
		.get_result_len
		.call(&mut inst.store, ())
		.map_err(|e| format!("Failed to get result len: {}", e))?;

	let result = read_string_from_memory(
		&inst.store,
		&inst.memory,
		result_ptr,
		result_len,
	)?;

	inst.free_result
		.call(&mut inst.store, ())
		.map_err(|e| format!("Failed to free result: {}", e))?;

	Ok(result)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_format_simple_c() {
		let input = "int main(){return 0;}";
		let result = format_file(input, "test.c");
		assert!(result.is_ok(), "Format failed: {:?}", result);
		let formatted = result.unwrap();
		assert!(formatted.contains("int main()"), "Output: {}", formatted);
	}

	#[test]
	fn test_format_cpp() {
		let input = "class Foo{public:void bar(){}};";
		let result = format_file(input, "test.cpp");
		assert!(result.is_ok(), "Format failed: {:?}", result);
	}

	#[test]
	fn test_format_uses_tabs() {
		// Test that tabs are used for indentation (per CONFIG)
		let input = "int main() {\nint x = 1;\nreturn x;\n}";
		let result = format_file(input, "test.c");
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
			let formatted = format_file(&input, "test.c")
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
			let formatted = format_file(&input, "test.c")
				.unwrap_or_else(|e| panic!("call {i} failed: {e}"));
			assert!(
				formatted.contains(&format!("y = {i};")),
				"call {i} wrong body: {formatted}"
			);
		});
	}

	#[test]
	fn test_format_cuda_hlsl_metal_glsl_verilog() {
		let cases = [
			(
				"kernel.cu",
				"__global__ void k(int*a){int i=threadIdx.x;a[i]=i;}",
				"threadIdx.x",
			),
			(
				"shader.hlsl",
				"float4 main(float4 pos:SV_POSITION):SV_TARGET{return pos;}",
				"SV_POSITION",
			),
			(
				"shader.metal",
				"kernel void add(device float*a[[buffer(0)]],uint i[[thread_position_in_grid]]){a[i]+=1;}",
				"buffer(0)",
			),
			(
				"shader.frag",
				"void main(){vec4 c=vec4(1.0);gl_FragColor=c;}",
				"gl_FragColor",
			),
			(
				"top.sv",
				"module top(input logic a,output logic b);assign b=a;endmodule",
				"endmodule",
			),
			(
				"defs.vh",
				"module defs(input wire a,output wire b);assign b=a;endmodule",
				"endmodule",
			),
		];
		for (name, input, expected) in cases {
			let once = ClangFormatter
				.format(input, Path::new(name))
				.unwrap_or_else(|e| panic!("{name} failed: {e}"));
			assert!(once.contains(expected), "{name}: {once}");
			let twice = ClangFormatter
				.format(&once, Path::new(name))
				.unwrap_or_else(|e| panic!("{name} idempotence failed: {e}"));
			assert_eq!(once, twice, "{name} not idempotent");
		}
	}

	#[test]
	fn test_disambiguation_confidence_for_v_and_m() {
		assert!(ClangFormatter.needs_content_check("v"));
		assert!(ClangFormatter.needs_content_check("m"));
		assert!(!ClangFormatter.needs_content_check("cpp"));

		assert!(
			ClangFormatter.match_confidence(
				Path::new("top.v"),
				"module top(input wire a); endmodule"
			) > 0
		);
		assert_eq!(
			ClangFormatter.match_confidence(
				Path::new("proof.v"),
				"Theorem foo : 1 = 1. Proof. reflexivity. Qed."
			),
			0
		);
		assert_eq!(
			ClangFormatter.match_confidence(
				Path::new("main.v"),
				"module main\nfn main() {\n\tprintln('hi')\n}"
			),
			0
		);

		assert!(
			ClangFormatter.match_confidence(
				Path::new("App.m"),
				"#import <Foundation/Foundation.h>\n@interface App : NSObject\n@end"
			) > 0
		);
		assert_eq!(
			ClangFormatter.match_confidence(
				Path::new("calc.m"),
				"% MATLAB script\nfunction y = calc(x)\n  y = x + 1;\nend"
			),
			0
		);
	}
}
