//! Swift formatter for fama, powered by real swift-format compiled to WASM.
//!
//! The module vendored at `wasm/swiftformat.wasm.zst` is Apple's swift-format
//! 603.0.0 (library `SwiftFormat` plus a small C-ABI shim) built for
//! `wasm32-unknown-wasip1` with the official SwiftWasm 6.3 SDK — see
//! `build-wasm.sh` for the exact, reproducible recipe. It runs in-process on
//! wasmtime, like the clang-format and dart_style crates: no sidecar process,
//! no toolchain on the host.
//!
//! The vendored module is zstd-compressed (~15MB for 62MB of wasm); it is
//! decompressed once per process. Because the module is ~40x larger than
//! clang-format's, a precompiled `.wasmu` artifact matters far more here:
//! deserializing one takes ~70ms versus ~5s to Cranelift-compile. Releases
//! ship per-platform artifacts (see `swift-precompile`); the first run on an
//! unsupported platform JITs once and caches the artifact to
//! `~/.cache/fama`.
//!
//! The guest import surface is pure WASI p1 (34 imports, zero JS). The
//! formatting path touches no filesystem (see `wasi`): the shim hands the
//! formatter a `nil` file URL, so diagnostics carry no path but formatting
//! stays hermetic; the runtime-init stat probes that remain are answered
//! ENOENT without disk access. Parse errors surface as errors (603.0.0 only
//! reports them through the shim's diagnostics handler — without one,
//! swift-format would format garbage silently).

mod wasi;

use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::sync::{Condvar, Mutex, OnceLock};

use fama_common::{
	ExportedConfig, FormatConfig, Formatter, IndentStyle, LineEnding, CONFIG,
};
use wasmtime::{Engine, Instance, Linker, Memory, Module, Store, TypedFunc};

/// zstd-compressed swift-format WASM module (see `build-wasm.sh`)
const SWIFTFORMAT_WASM_ZST: &[u8] = include_bytes!("../wasm/swiftformat.wasm.zst");

const ARTIFACT_PREFIX: &str = "swiftformat";

/// Maximum number of concurrent guest instances kept in the pool.
///
/// Wasmtime maps read-only linear memory pages copy-on-write across instances,
/// so a small bounded pool unlocks multi-core formatting on large repos without
/// scaling RSS linearly with high core counts.
const MAX_POOL_INSTANCES: usize = 4;

/// Decompress the vendored module on demand (only needed when no precompiled
/// `.wasmu` artifact is available, or when generating one). The returned buffer
/// is intentionally NOT cached in a global so the 15-60 MiB raw WASM bytes are
/// freed immediately after Cranelift compilation finishes.
fn decompress_module_bytes() -> Vec<u8> {
	let cursor = std::io::Cursor::new(SWIFTFORMAT_WASM_ZST);
	let mut decoder =
		ruzstd::StreamingDecoder::new(cursor).expect("valid zstd stream");
	let mut out =
		Vec::with_capacity(SWIFTFORMAT_WASM_ZST.len().saturating_mul(4));
	std::io::Read::read_to_end(&mut decoder, &mut out)
		.expect("failed to decompress swift-format module");
	out
}

/// Compile-time sha256 prefix of `wasm/swiftformat.wasm.zst`; binds precompiled
/// artifacts to the exact embedded module with zero runtime decompression or
/// hashing on the warm `.wasmu` path.
fn artifact_hash() -> &'static str {
	env!("SWIFTFORMAT_WASM_HASH")
}

/// A fully initialized WASM instance ready to format files.
struct SwiftInstance {
	store: Store<wasi::WasiState>,
	memory: Memory,
	alloc: TypedFunc<i32, i32>,
	free: TypedFunc<(i32, i32), ()>,
	format: TypedFunc<(i32, i32, i32, i32), i64>,
	out_ptr: TypedFunc<(), i32>,
	err_ptr: TypedFunc<(), i32>,
	err_len: TypedFunc<(), i32>,
	/// True when the guest exported `swiftformat_set_config` and has already
	/// cached the formatted `Configuration` + `SwiftFormatter`.
	config_preloaded: bool,
}

struct PoolState {
	idle: Vec<SwiftInstance>,
	in_use: usize,
}

struct InstancePool {
	state: Mutex<PoolState>,
	available: Condvar,
	max_instances: usize,
}

impl InstancePool {
	fn new() -> Self {
		let max_instances = std::thread::available_parallelism()
			.map(|n| n.get().clamp(1, MAX_POOL_INSTANCES))
			.unwrap_or(1);
		InstancePool {
			state: Mutex::new(PoolState {
				idle: Vec::new(),
				in_use: 0,
			}),
			available: Condvar::new(),
			max_instances,
		}
	}

	fn acquire(&self) -> Result<SwiftInstance, String> {
		let mut state = self
			.state
			.lock()
			.unwrap_or_else(|poisoned| poisoned.into_inner());
		loop {
			if let Some(instance) = state.idle.pop() {
				state.in_use += 1;
				return Ok(instance);
			}
			if state.in_use < self.max_instances {
				state.in_use += 1;
				drop(state);
				match create_instance() {
					Ok(instance) => return Ok(instance),
					Err(e) => {
						let mut state = self
							.state
							.lock()
							.unwrap_or_else(|poisoned| poisoned.into_inner());
						state.in_use -= 1;
						self.available.notify_one();
						return Err(e);
					}
				}
			}
			state = self
				.available
				.wait(state)
				.unwrap_or_else(|poisoned| poisoned.into_inner());
		}
	}

	fn release(&self, instance: Option<SwiftInstance>) {
		let mut state = self
			.state
			.lock()
			.unwrap_or_else(|poisoned| poisoned.into_inner());
		state.in_use -= 1;
		if let Some(instance) = instance {
			state.idle.push(instance);
		}
		self.available.notify_one();
	}
}

/// Shared bounded instance pool. Trapped instances are dropped rather than
/// returned so subsequent calls start from a clean guest heap.
static POOL: OnceLock<InstancePool> = OnceLock::new();

fn pool() -> &'static InstancePool {
	POOL.get_or_init(InstancePool::new)
}

fn engine() -> &'static Engine {
	static ENGINE: OnceLock<Engine> = OnceLock::new();
	ENGINE.get_or_init(|| {
		Engine::new(&fama_wasm::default_engine_config())
			.expect("Failed to create wasmtime engine")
	})
}

fn cached_module() -> &'static Module {
	static CACHED: OnceLock<Module> = OnceLock::new();
	CACHED.get_or_init(|| {
		fama_wasm::load_or_compile(
			engine(),
			ARTIFACT_PREFIX,
			artifact_hash(),
			|| Cow::Owned(decompress_module_bytes()),
		)
		.expect("Failed to compile swift-format WASM module")
	})
}

fn cached_linker() -> &'static Linker<wasi::WasiState> {
	static LINKER: OnceLock<Linker<wasi::WasiState>> = OnceLock::new();
	LINKER.get_or_init(|| {
		let engine = engine();
		let module = cached_module();
		let mut linker = wasi::build_linker(engine);
		wasi::stub_remaining_imports(&mut linker, module);
		linker
	})
}

/// Outcome of a call against the guest, distinguishing "guest is fine,
/// formatting failed" from "guest state is unknowable".
enum CallResult<R> {
	/// Formatting succeeded
	Good(R),
	/// The guest returned a structured error code; its state is clean and
	/// the instance stays warm
	GuestError(String),
	/// A wasm trap: the instance is dropped and rebuilt on next use
	Trapped(String),
}

/// Run `f` with a pooled instance, creating one if needed (up to
/// `MAX_POOL_INSTANCES`). A guest-level error keeps the instance warm; a trap
/// drops it so the next call starts from a fresh instance.
fn with_instance<R>(
	f: impl FnOnce(&mut SwiftInstance) -> CallResult<R>,
) -> Result<R, String> {
	let pool = pool();
	let mut instance = pool.acquire()?;

	match f(&mut instance) {
		CallResult::Good(result) => {
			pool.release(Some(instance));
			Ok(result)
		}
		CallResult::GuestError(message) => {
			pool.release(Some(instance));
			Err(message)
		}
		CallResult::Trapped(message) => {
			pool.release(None);
			Err(message)
		}
	}
}

fn create_instance() -> Result<SwiftInstance, String> {
	let module = cached_module();
	let engine = engine();
	let linker = cached_linker();
	let mut store = Store::new(engine, wasi::WasiState::new());

	let instance: Instance = linker
		.instantiate(&mut store, module)
		.map_err(|e| format!("Failed to instantiate swift-format module: {e:#}"))?;

	macro_rules! typed {
		($name:literal, $params:ty => $results:ty) => {
			instance
				.get_typed_func::<$params, $results>(&mut store, $name)
				.map_err(|e| format!("missing export {}: {e}", $name))?
		};
	}

	let alloc: TypedFunc<i32, i32> = typed!("swiftformat_alloc", i32 => i32);
	let free: TypedFunc<(i32, i32), ()> =
		typed!("swiftformat_free", (i32, i32) => ());
	let format: TypedFunc<(i32, i32, i32, i32), i64> =
		typed!("swiftformat_format", (i32, i32, i32, i32) => i64);
	let out_ptr: TypedFunc<(), i32> = typed!("swiftformat_out_ptr", () => i32);
	let err_ptr: TypedFunc<(), i32> = typed!("swiftformat_err_ptr", () => i32);
	let err_len: TypedFunc<(), i32> = typed!("swiftformat_err_len", () => i32);
	let set_config = instance
		.get_typed_func::<(i32, i32, i32), ()>(&mut store, "swiftformat_set_config")
		.ok();
	let memory = instance
		.get_export(&mut store, "memory")
		.and_then(|e| e.into_memory())
		.ok_or("swift-format module has no memory export")?;

	let mut swift = SwiftInstance {
		store,
		memory,
		alloc,
		free,
		format,
		out_ptr,
		err_ptr,
		err_len,
		config_preloaded: false,
	};

	// Run the WASI command entry point to fully initialize the Swift
	// runtime; the shim's main returns immediately. An exit-code-0 trap is
	// the normal way a WASI command "finishes main".
	if let Some(start) = instance.get_func(&mut swift.store, "_start") {
		if let Err(e) = start.call(&mut swift.store, &[], &mut []) {
			let text = format!("{e:?}");
			if !text.contains("proc_exit(0)") {
				return Err(format!("swift-format _start failed: {text}"));
			}
		}
	}

	// If the guest exposes the direct config setter, push fama's constant
	// config once so per-file calls skip JSON allocation and JSONDecoder.
	if let Some(set_config) = set_config {
		let use_tabs = i32::from(matches!(CONFIG.indent_style, IndentStyle::Tabs));
		set_config
			.call(
				&mut swift.store,
				(CONFIG.line_width as i32, use_tabs, CONFIG.indent_width as i32),
			)
			.map_err(|e| format!("swiftformat_set_config failed: {e:?}"))?;
		swift.config_preloaded = true;
	}

	Ok(swift)
}

impl SwiftInstance {
	fn read(&self, ptr: i32, len: i32) -> &[u8] {
		if len <= 0 {
			return &[];
		}
		let data = self.memory.data(&self.store);
		let start = ptr as usize;
		let end = (start + len as usize).min(data.len());
		&data[start..end]
	}

	fn write(&mut self, ptr: i32, bytes: &[u8]) -> Result<(), String> {
		let data = self.memory.data_mut(&mut self.store);
		let start = ptr as usize;
		let end = start + bytes.len();
		if end > data.len() {
			return Err("guest memory write out of bounds".to_string());
		}
		data[start..end].copy_from_slice(bytes);
		Ok(())
	}

	fn format_once(
		&mut self,
		source: &[u8],
		config: &[u8],
	) -> CallResult<String> {
		let trap = |e: wasmtime::Error, what: &str| {
			CallResult::Trapped(format!("swift-format {what} trapped: {e:?}"))
		};
		let src_ptr = match self.alloc.call(&mut self.store, source.len() as i32)
		{
			Ok(p) => p.max(1),
			Err(e) => return trap(e, "alloc"),
		};
		if let Err(e) = self.write(src_ptr, source) {
			return CallResult::Trapped(e);
		}

		let effective_cfg = if self.config_preloaded { &[][..] } else { config };
		let cfg_ptr = if effective_cfg.is_empty() {
			0
		} else {
			match self.alloc.call(&mut self.store, effective_cfg.len() as i32) {
				Ok(p) => {
					let p = p.max(1);
					if let Err(e) = self.write(p, effective_cfg) {
						return CallResult::Trapped(e);
					}
					p
				}
				Err(e) => return trap(e, "alloc"),
			}
		};

		let code = self.format.call(
			&mut self.store,
			(
				src_ptr,
				source.len() as i32,
				cfg_ptr,
				effective_cfg.len() as i32,
			),
		);

		// Free the input buffers regardless of outcome (guest heap hygiene
		// across thousands of files)
		let free_src = self.free.call(&mut self.store, (src_ptr, 0));
		if cfg_ptr != 0 {
			let _ = self.free.call(&mut self.store, (cfg_ptr, 0));
		}

		let code = match code {
			Ok(code) => code,
			Err(e) => {
				let note =
					String::from_utf8_lossy(&self.store.data().stderr);
				return CallResult::Trapped(format!(
					"swift-format trapped: {e:?} (guest stderr: {note})"
				));
			}
		};
		if free_src.is_err() {
			return CallResult::Trapped(
				"guest free of input buffer failed".to_string(),
			);
		}

		if code < 0 {
			let len = match self.err_len.call(&mut self.store, ()) {
				Ok(len) => len,
				Err(e) => return trap(e, "err_len"),
			};
			let ptr = match self.err_ptr.call(&mut self.store, ()) {
				Ok(ptr) => ptr,
				Err(e) => return trap(e, "err_ptr"),
			};
			let message =
				String::from_utf8_lossy(self.read(ptr, len)).into_owned();
			return CallResult::GuestError(if message.is_empty() {
				format!("swift-format error code {code}")
			} else {
				message
			});
		}

		let len = code as i32;
		let ptr = match self.out_ptr.call(&mut self.store, ()) {
			Ok(ptr) => ptr,
			Err(e) => return trap(e, "out_ptr"),
		};
		let bytes = self.read(ptr, len).to_vec();
		String::from_utf8(bytes).map(CallResult::Good).unwrap_or_else(|_| {
			CallResult::GuestError(
				"swift-format emitted invalid UTF-8".to_string(),
			)
		})
	}
}

/// swift-format `Configuration` JSON built once from fama's config: line width,
/// indentation style (swift-format speaks tabs natively) and tab width.
/// Line endings are normalized host-side because 603.0.0's Configuration has
/// no line-ending knob (it only has `respectsExistingLineEndings`).
fn config_json() -> &'static [u8] {
	static JSON: OnceLock<String> = OnceLock::new();
	JSON.get_or_init(|| {
		let indentation = match CONFIG.indent_style {
			IndentStyle::Tabs => "{\"tabs\":1}".to_string(),
			IndentStyle::Spaces => {
				format!("{{\"spaces\":{}}}", CONFIG.indent_width)
			}
		};
		format!(
			"{{\"lineLength\":{},\"indentation\":{},\"tabWidth\":{}}}",
			CONFIG.line_width, indentation, CONFIG.indent_width
		)
	})
	.as_bytes()
}

fn render_swift_format(config: &FormatConfig) -> String {
	// The guest decodes exactly this JSON (swift-format's Configuration
	// allows JSON5, so the header comment is fine), so editor integrations
	// running real swift-format agree with fama's output. Line endings are
	// handled by fama at format time; the config file has no such knob.
	let indentation = match config.indent_style {
		IndentStyle::Tabs => "\"tabs\": 1".to_string(),
		IndentStyle::Spaces => format!("\"spaces\": {}", config.indent_width),
	};
	format!(
		r#"// Generated by fama — https://github.com/AkaraChen/fama
{{
  "lineLength": {line_length},
  "tabWidth": {tab_width},
  "indentation": {{{indentation}}}
}}
"#,
		line_length = config.line_width,
		tab_width = config.indent_width,
		indentation = indentation,
	)
}

/// Formatter implementation for Swift via swift-format compiled to WASM.
pub struct SwiftFormatter;

/// Static formatter instance registered by `fama`.
pub static FORMATTER: SwiftFormatter = SwiftFormatter;

impl Formatter for SwiftFormatter {
	fn name(&self) -> &'static str {
		"swift"
	}

	fn extensions(&self) -> &'static [&'static str] {
		&["swift"]
	}

	fn format(&self, content: &str, path: &Path) -> Result<String, String> {
		format_file(content, path.to_str().unwrap_or(""))
	}

	fn exported_config(&self) -> Option<ExportedConfig> {
		Some(ExportedConfig {
			marker_file: "Package.swift",
			output_file: ".swift-format",
			render: render_swift_format,
		})
	}

	fn precompile(&self, output_dir: &Path) -> Option<Result<PathBuf, String>> {
		let result = Engine::new(&fama_wasm::default_engine_config())
			.map_err(|e| format!("Failed to create wasmtime engine: {e}"))
			.and_then(|engine| {
				fama_wasm::precompile_to_dir(
					&engine,
					ARTIFACT_PREFIX,
					artifact_hash(),
					|| Cow::Owned(decompress_module_bytes()),
					output_dir,
				)
			});
		Some(result)
	}
}

/// Format Swift code using swift-format compiled to WASM
///
/// # Arguments
/// * `content` - The Swift source code to format
/// * `_path` - The file path (unused: the guest formats strings and gets no
///   file URL, so output never embeds the path)
///
/// # Returns
/// * `Ok(String)` - The formatted code
/// * `Err(String)` - Error message if formatting failed (e.g. the source
///   does not parse)
pub fn format_file(content: &str, _path: &str) -> Result<String, String> {
	if content.is_empty() {
		return Ok(String::new());
	}
	// Normalize to LF for the guest without allocating on already-LF inputs;
	// swift-format is line-ending agnostic when fed pure LF, and the target
	// ending is applied afterwards.
	let normalized: Cow<'_, str> = if content.contains('\r') {
		Cow::Owned(content.replace("\r\n", "\n").replace('\r', "\n"))
	} else {
		Cow::Borrowed(content)
	};
	let config = config_json();

	let formatted = with_instance(|instance| {
		instance.format_once(normalized.as_bytes(), config)
	})?;

	Ok(match CONFIG.line_ending {
		LineEnding::Lf => formatted,
		LineEnding::Crlf => formatted.replace('\n', "\r\n"),
	})
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_format_simple_function() {
		// swift-format keeps short declarations on one line (real swift-format
		// semantics, not fama's opinion): width and structure decide
		let input = "func f()->Int{return 42}";
		let result = format_file(input, "test.swift");
		assert!(result.is_ok(), "Format failed: {:?}", result);
		assert_eq!(result.unwrap(), "func f() -> Int { return 42 }\n");

		// Multi-statement bodies get real indentation, tabs per fama default
		let input = "func f() {\nlet a = 1\nlet b = 2\n}";
		let formatted = format_file(input, "test.swift").unwrap();
		assert_eq!(
			formatted, "func f() {\n\tlet a = 1\n\tlet b = 2\n}\n",
			"tab indentation wrong"
		);
	}

	#[test]
	fn test_idempotent() {
		let input = "struct S{var x:Int\nvar y:String}";
		let first = format_file(input, "test.swift").unwrap();
		let second = format_file(&first, "test.swift").unwrap();
		assert_eq!(first, second, "formatting is not idempotent");
	}

	#[test]
	fn test_syntax_error_returns_err() {
		let input = "func f() { let x = ";
		let result = format_file(input, "bad.swift");
		assert!(result.is_err(), "Expected error for invalid syntax");
		let message = result.unwrap_err();
		assert!(
			message.contains("fileContainsInvalidSyntax")
				|| message.contains("format error"),
			"Error message should describe the problem: {message}"
		);
	}

	#[test]
	fn test_instance_survives_errors() {
		assert!(format_file("func f(){", "bad.swift").is_err());
		let ok = format_file("func f(){}", "good.swift");
		assert!(ok.is_ok(), "Instance did not recover: {:?}", ok);
		assert_eq!(ok.unwrap(), "func f() {}\n");
	}

	#[test]
	fn test_formatting_touches_no_filesystem() {
		format_file("let x=1", "probe.swift").unwrap();
		format_file("let y=2", "probe.swift").unwrap();
		let guard = pool().state.lock().unwrap();
		assert!(
			!guard.idle.is_empty(),
			"at least one instance should be alive in the pool"
		);
		// The guest may stat a path or two during runtime init (answered
		// ENOENT without disk access), but a string-in/string-out formatter
		// must never open or unlink anything
		for instance in &guard.idle {
			let opens: Vec<&str> = instance
				.store
				.data()
				.fs_calls
				.iter()
				.filter(|c| **c != "path_filestat_get")
				.copied()
				.collect();
			assert!(
				opens.is_empty(),
				"formatting touched the filesystem beyond stat probes: {opens:?}"
			);
		}
	}

	#[test]
	fn test_empty_input_untouched() {
		assert_eq!(format_file("", "empty.swift").unwrap(), "");
	}

	#[test]
	fn test_reused_instance_formats_repeatedly() {
		for i in 0..10 {
			let input = format!("func f()->Int{{return {i}}}");
			let formatted = format_file(&input, "test.swift")
				.unwrap_or_else(|e| panic!("call {i} failed: {e}"));
			assert!(
				formatted.contains(&format!("return {i}")),
				"call {i} wrong body: {formatted}"
			);
		}
	}

	#[test]
	fn test_parallel_pool_formatting() {
		let handles: Vec<_> = (0..16)
			.map(|i| {
				std::thread::spawn(move || {
					let input = format!("func g{i}()->Int{{return {i}}}");
					let formatted = format_file(&input, "parallel.swift")
						.unwrap_or_else(|e| panic!("thread {i} failed: {e}"));
					assert_eq!(formatted, format!("func g{i}() -> Int {{ return {i} }}\n"));
				})
			})
			.collect();
		for h in handles {
			h.join().unwrap();
		}
		let guard = pool().state.lock().unwrap();
		assert!(
			guard.idle.len() <= pool().max_instances,
			"pool exceeded max_instances: {} > {}",
			guard.idle.len(),
			pool().max_instances
		);
	}
}
