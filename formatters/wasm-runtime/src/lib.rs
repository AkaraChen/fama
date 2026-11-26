//! Shared wasmtime engine configuration, artifact caching, and precompilation
//! helpers for WASM-backed formatters (`fama-clang`, `fama-dart`, `fama-swift`).

use std::borrow::Cow;
use std::path::{Path, PathBuf};

use wasmtime::{Config, Engine, Module};

/// Default wasmtime configuration tuned for formatters.
///
/// Backtrace capture, address maps, and non-Windows unwind metadata add
/// significant `.wasmu` size and Cranelift compilation overhead without adding
/// value since formatting errors are surfaced explicitly.
pub fn default_engine_config() -> Config {
	let mut cfg = Config::new();
	cfg.wasm_backtrace_max_frames(None);
	cfg.generate_address_map(false);
	#[cfg(not(target_os = "windows"))]
	cfg.native_unwind_info(false);
	cfg
}

/// Filename of the precompiled module artifact for a given prefix and hash.
pub fn artifact_name(prefix: &str, hash: &str) -> String {
	format!("{prefix}-{hash}.wasmu")
}

/// Candidate locations for a precompiled module: next to the `fama` binary
/// (shipped with releases) followed by the per-user cache directory.
pub fn artifact_candidates(prefix: &str, hash: &str) -> Vec<PathBuf> {
	let name = artifact_name(prefix, hash);
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

/// Load a precompiled `.wasmu` module if available and valid, or compile the
/// embedded WASM bytes and cache the serialized artifact in `~/.cache/fama`.
pub fn load_or_compile<F>(
	engine: &Engine,
	prefix: &str,
	hash: &str,
	load_wasm: F,
) -> Result<Module, String>
where
	F: FnOnce() -> Cow<'static, [u8]>,
{
	let candidates = artifact_candidates(prefix, hash);
	for path in &candidates {
		if !path.is_file() {
			continue;
		}
		// SAFETY: the artifact was written by this same binary (its filename
		// is bound to the sha256 of the embedded module bytes) in a trusted
		// location, and deserialize validates the wasmtime version and host
		// feature compatibility; any mismatch falls back to compiling below.
		if let Ok(module) = unsafe { Module::deserialize_file(engine, path) } {
			return Ok(module);
		}
	}

	let wasm = load_wasm();
	let module = Module::new(engine, &wasm)
		.map_err(|e| format!("Failed to compile WASM module: {e:?}"))?;
	drop(wasm);

	// Best-effort atomic cache write to user cache directory (last candidate
	// when HOME/USERPROFILE is set, or skip if only exe dir was present).
	if let Some(home) =
		std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))
	{
		let cache_path = PathBuf::from(home)
			.join(".cache/fama")
			.join(artifact_name(prefix, hash));
		if let Some(dir) = cache_path.parent() {
			if let Ok(artifact) = module.serialize() {
				let _ = std::fs::create_dir_all(dir);
				let tmp = cache_path.with_extension("wasmu.tmp");
				if std::fs::write(&tmp, &artifact).is_ok() {
					let _ = std::fs::rename(&tmp, &cache_path);
				}
			}
		}
	}

	Ok(module)
}

/// Compile the WASM module with `engine`, serialize it, and write
/// `{prefix}-{hash}.wasmu` inside `output_dir`.
pub fn precompile_to_dir<F>(
	engine: &Engine,
	prefix: &str,
	hash: &str,
	load_wasm: F,
	output_dir: &Path,
) -> Result<PathBuf, String>
where
	F: FnOnce() -> Cow<'static, [u8]>,
{
	let wasm = load_wasm();
	let module = Module::new(engine, &wasm)
		.map_err(|e| format!("Failed to compile WASM module: {e:?}"))?;
	drop(wasm);
	let bytes = module
		.serialize()
		.map_err(|e| format!("Failed to serialize module: {e:?}"))?;
	std::fs::create_dir_all(output_dir).map_err(|e| {
		format!("Failed to create {}: {e}", output_dir.display())
	})?;
	let out_path = output_dir.join(artifact_name(prefix, hash));
	std::fs::write(&out_path, &bytes)
		.map_err(|e| format!("Failed to write {}: {e}", out_path.display()))?;
	Ok(out_path)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_artifact_name() {
		assert_eq!(
			artifact_name("clang-format", "abc123"),
			"clang-format-abc123.wasmu"
		);
	}

	#[test]
	fn test_default_engine_config_builds() {
		let engine = Engine::new(&default_engine_config());
		assert!(engine.is_ok());
	}
}
