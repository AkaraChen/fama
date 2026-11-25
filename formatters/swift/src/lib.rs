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

use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use fama_common::{FileType, IndentStyle, LineEnding, CONFIG};
use wasmtime::{Engine, Instance, Memory, Module, Store, TypedFunc};

/// zstd-compressed swift-format WASM module (see `build-wasm.sh`)
const SWIFTFORMAT_WASM_ZST: &[u8] = include_bytes!("../wasm/swiftformat.wasm.zst");

/// Decompress the vendored module (once per process)
fn module_bytes() -> &'static [u8] {
	static MODULE: OnceLock<Vec<u8>> = OnceLock::new();
	MODULE.get_or_init(|| {
		let cursor = std::io::Cursor::new(SWIFTFORMAT_WASM_ZST);
		let mut decoder =
			ruzstd::StreamingDecoder::new(cursor).expect("valid zstd stream");
		let mut out = Vec::with_capacity(70 * 1024 * 1024);
		std::io::Read::read_to_end(&mut decoder, &mut out)
			.expect("failed to decompress swift-format module");
		out
	})
}

/// sha256 prefix of the decompressed module bytes; binds precompiled
/// artifacts to the exact module they were compiled from
fn artifact_hash() -> &'static str {
	use sha2::{Digest, Sha256};
	static HASH: OnceLock<String> = OnceLock::new();
	HASH.get_or_init(|| {
		let digest = Sha256::digest(module_bytes());
		digest.iter().map(|b| format!("{b:02x}")).collect::<String>()
			[..16]
			.to_string()
	})
	.as_str()
}

/// Filename of the precompiled module artifact for this fama build
pub fn precompiled_artifact_name() -> String {
	format!("swiftformat-{}.wasmu", artifact_hash())
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
	let engine = Engine::new(&engine_config())
		.map_err(|e| format!("Failed to create engine: {e}"))?;
	let module = Module::new(&engine, module_bytes())
		.map_err(|e| format!("Failed to compile WASM module: {e:?}"))?;
	module
		.serialize()
		.map_err(|e| format!("Failed to serialize module: {e:?}"))
}

fn engine_config() -> wasmtime::Config {
	// Wasmtime defaults: backtraces off (a 23MB code section would make
	// capture expensive, and traps already carry the guest stderr tail)
	wasmtime::Config::new()
}

/// A fully initialized WASM instance ready to format files. One instance
/// serves all threads through a mutex: wasm execution is single-threaded
/// anyway, and each instance carries ~50MB of guest linear memory.
struct SwiftInstance {
	store: Store<wasi::WasiState>,
	memory: Memory,
	alloc: TypedFunc<i32, i32>,
	free: TypedFunc<(i32, i32), ()>,
	format: TypedFunc<(i32, i32, i32, i32), i64>,
	out_ptr: TypedFunc<(), i32>,
	err_ptr: TypedFunc<(), i32>,
	err_len: TypedFunc<(), i32>,
}

/// Shared instance slot. `None` means "dropped after a trap; rebuild on
/// next use".
static INSTANCE: OnceLock<Mutex<Option<SwiftInstance>>> = OnceLock::new();

fn engine() -> &'static Engine {
	static ENGINE: OnceLock<Engine> = OnceLock::new();
	ENGINE.get_or_init(|| {
		Engine::new(&engine_config()).expect("Failed to create wasmtime engine")
	})
}

fn cached_module() -> &'static Module {
	static CACHED: OnceLock<Module> = OnceLock::new();
	CACHED.get_or_init(|| {
		let engine = engine();

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
				unsafe { Module::deserialize_file(engine, &path) }
			{
				return module;
			}
		}

		let module = Module::new(engine, module_bytes())
			.expect("Failed to compile swift-format WASM module");

		// Seed the artifact cache for future runs (best effort). This write
		// is ~150MB on first run on a platform without a shipped artifact.
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

		module
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

/// Run `f` with the shared instance, creating it if needed. A guest-level
/// error keeps the instance warm; a trap drops it so the next call starts
/// from a fresh instance (guest heap state after a trap is unknowable).
fn with_instance<R>(
	f: impl FnOnce(&mut SwiftInstance) -> CallResult<R>,
) -> Result<R, String> {
	let slot = INSTANCE.get_or_init(|| Mutex::new(None));
	let mut slot = slot
		.lock()
		.unwrap_or_else(|poisoned| poisoned.into_inner());

	let mut instance = match slot.take() {
		Some(instance) => instance,
		None => create_instance()?,
	};

	match f(&mut instance) {
		CallResult::Good(result) => {
			*slot = Some(instance);
			Ok(result)
		}
		CallResult::GuestError(message) => {
			*slot = Some(instance);
			Err(message)
		}
		CallResult::Trapped(message) => Err(message),
	}
}

fn create_instance() -> Result<SwiftInstance, String> {
	let module = cached_module();
	let engine = engine();
	let mut store = Store::new(engine, wasi::WasiState::new());

	let mut linker = wasi::build_linker(engine);
	wasi::stub_remaining_imports(&mut linker, module);

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

		let cfg_ptr = if config.is_empty() {
			0
		} else {
			match self.alloc.call(&mut self.store, config.len() as i32) {
				Ok(p) => {
					let p = p.max(1);
					if let Err(e) = self.write(p, config) {
						return CallResult::Trapped(e);
					}
					p
				}
				Err(e) => return trap(e, "alloc"),
			}
		};

		let code = self.format.call(
			&mut self.store,
			(src_ptr, source.len() as i32, cfg_ptr, config.len() as i32),
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

/// swift-format `Configuration` JSON built from fama's config: line width,
/// indentation style (swift-format speaks tabs natively) and tab width.
/// Line endings are normalized host-side because 603.0.0's Configuration has
/// no line-ending knob (it only has `respectsExistingLineEndings`).
fn build_config_json() -> String {
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
}

/// Format Swift code using swift-format compiled to WASM
///
/// # Arguments
/// * `content` - The Swift source code to format
/// * `_path` - The file path (unused: the guest formats strings and gets no
///   file URL, so output never embeds the path)
/// * `_file_type` - The detected file type (must be `FileType::Swift`)
///
/// # Returns
/// * `Ok(String)` - The formatted code
/// * `Err(String)` - Error message if formatting failed (e.g. the source
///   does not parse)
pub fn format_file(
	content: &str,
	_path: &str,
	_file_type: FileType,
) -> Result<String, String> {
	if content.is_empty() {
		return Ok(String::new());
	}
	// Normalize to LF for the guest; swift-format is line-ending agnostic
	// when fed pure LF, and the target ending is applied afterwards
	let normalized = content.replace("\r\n", "\n").replace('\r', "\n");
	let config = build_config_json();

	let formatted = with_instance(|instance| {
		instance.format_once(normalized.as_bytes(), config.as_bytes())
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
		let result = format_file(input, "test.swift", FileType::Swift);
		assert!(result.is_ok(), "Format failed: {:?}", result);
		assert_eq!(result.unwrap(), "func f() -> Int { return 42 }\n");

		// Multi-statement bodies get real indentation, tabs per fama default
		let input = "func f() {\nlet a = 1\nlet b = 2\n}";
		let formatted =
			format_file(input, "test.swift", FileType::Swift).unwrap();
		assert_eq!(
			formatted, "func f() {\n\tlet a = 1\n\tlet b = 2\n}\n",
			"tab indentation wrong"
		);
	}

	#[test]
	fn test_idempotent() {
		let input = "struct S{var x:Int\nvar y:String}";
		let first =
			format_file(input, "test.swift", FileType::Swift).unwrap();
		let second =
			format_file(&first, "test.swift", FileType::Swift).unwrap();
		assert_eq!(first, second, "formatting is not idempotent");
	}

	#[test]
	fn test_syntax_error_returns_err() {
		let input = "func f() { let x = ";
		let result = format_file(input, "bad.swift", FileType::Swift);
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
		assert!(format_file("func f(){", "bad.swift", FileType::Swift).is_err());
		let ok = format_file("func f(){}", "good.swift", FileType::Swift);
		assert!(ok.is_ok(), "Instance did not recover: {:?}", ok);
		assert_eq!(ok.unwrap(), "func f() {}\n");
	}

	#[test]
	fn test_formatting_touches_no_filesystem() {
		format_file("let x=1", "probe.swift", FileType::Swift).unwrap();
		format_file("let y=2", "probe.swift", FileType::Swift).unwrap();
		let slot = INSTANCE.get_or_init(|| Mutex::new(None));
		let guard = slot.lock().unwrap();
		let instance = guard.as_ref().expect("instance should be alive");
		// The guest may stat a path or two during runtime init (answered
		// ENOENT without disk access), but a string-in/string-out formatter
		// must never open or unlink anything
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

	#[test]
	fn test_empty_input_untouched() {
		assert_eq!(
			format_file("", "empty.swift", FileType::Swift).unwrap(),
			""
		);
	}

	#[test]
	fn test_reused_instance_formats_repeatedly() {
		for i in 0..10 {
			let input = format!("func f()->Int{{return {i}}}");
			let formatted =
				format_file(&input, "test.swift", FileType::Swift)
					.unwrap_or_else(|e| panic!("call {i} failed: {e}"));
			assert!(
				formatted.contains(&format!("return {i}")),
				"call {i} wrong body: {formatted}"
			);
		}
	}
}
