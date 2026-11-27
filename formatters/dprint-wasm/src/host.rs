//! dprint Schema v4 WASM plugin runtime over `wasmtime` (`fama-wasm`).

use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::sync::{Condvar, Mutex, OnceLock};

use fama_common::{IndentStyle, LineEnding, CONFIG};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use wasmtime::{
	Caller, Engine, Instance, Linker, Memory, Module, Store, TypedFunc,
};

use crate::wasi::{define_wasi, read_guest, write_guest, DprintStoreState};

const CONFIG_ID: u32 = 1;
const MAX_POOL_INSTANCES: usize = 4;

/// Metadata returned by a Schema v4 plugin's `get_plugin_info()` export.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginInfo {
	pub name: String,
	pub version: String,
	pub config_key: String,
	pub help_url: String,
	pub config_schema_url: String,
	pub update_url: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GlobalConfigPayload {
	line_width: u32,
	use_tabs: bool,
	indent_width: u8,
	new_line_kind: &'static str,
}

#[derive(Serialize)]
struct RawFormatConfigPayload<'a> {
	plugin: &'a serde_json::Map<String, serde_json::Value>,
	global: GlobalConfigPayload,
}

/// Source of a dprint Schema v4 `.wasm` plugin module.
#[derive(Clone, Copy)]
pub enum DprintWasmSource {
	/// Uncompressed `.wasm` bytes embedded in the binary.
	EmbeddedRaw(&'static [u8]),
	/// Zstd-compressed `.wasm.zst` bytes embedded in the binary.
	EmbeddedZstd(&'static [u8]),
	/// Path to a `.wasm` (or `.wasm.zst`) file on disk.
	FilePath(&'static str),
}

impl DprintWasmSource {
	fn load_bytes(&self) -> Result<Cow<'static, [u8]>, String> {
		match self {
			Self::EmbeddedRaw(bytes) => Ok(Cow::Borrowed(bytes)),
			Self::EmbeddedZstd(zst) => {
				fama_wasm::decompress_zstd(zst).map(Cow::Owned)
			}
			Self::FilePath(path) => {
				let raw = std::fs::read(path).map_err(|e| {
					format!("Failed to read dprint plugin {path}: {e}")
				})?;
				if path.ends_with(".zst") {
					fama_wasm::decompress_zstd(&raw).map(Cow::Owned)
				} else {
					Ok(Cow::Owned(raw))
				}
			}
		}
	}

	fn cache_hash(&self) -> Result<String, String> {
		let mut hasher = Sha256::new();
		match self {
			Self::EmbeddedRaw(bytes) | Self::EmbeddedZstd(bytes) => {
				hasher.update(bytes);
			}
			Self::FilePath(path) => {
				let bytes = std::fs::read(path).map_err(|e| {
					format!("Failed to hash dprint plugin {path}: {e}")
				})?;
				hasher.update(bytes);
			}
		}
		let digest = hasher.finalize();
		Ok(format!("{:016x}", u64::from_be_bytes(digest[..8].try_into().unwrap())))
	}
}

struct DprintInstance {
	store: Store<DprintStoreState>,
	memory: Memory,
	get_shared_bytes_ptr: TypedFunc<(), i32>,
	clear_shared_bytes: TypedFunc<i32, i32>,
	get_plugin_info: TypedFunc<(), i32>,
	register_config: TypedFunc<i32, ()>,
	set_file_path: TypedFunc<(), ()>,
	format: TypedFunc<i32, i32>,
	get_formatted_text: TypedFunc<(), i32>,
	get_error_text: TypedFunc<(), i32>,
}

impl DprintInstance {
	fn write_shared(&mut self, bytes: &[u8]) -> Result<(), String> {
		let len = i32::try_from(bytes.len())
			.map_err(|_| "input exceeds i32 length".to_string())?;
		let ptr = self
			.clear_shared_bytes
			.call(&mut self.store, len)
			.map_err(|e| format!("clear_shared_bytes trapped: {e}"))?;
		if !bytes.is_empty() {
			self.memory
				.write(&mut self.store, ptr as usize, bytes)
				.map_err(|e| format!("Failed to write shared bytes: {e}"))?;
		}
		Ok(())
	}

	fn read_shared(&mut self, len: i32) -> Result<Vec<u8>, String> {
		if len <= 0 {
			return Ok(Vec::new());
		}
		let ptr = self
			.get_shared_bytes_ptr
			.call(&mut self.store, ())
			.map_err(|e| format!("get_shared_bytes_ptr trapped: {e}"))?;
		let mut buf = vec![0u8; len as usize];
		self.memory
			.read(&self.store, ptr as usize, &mut buf)
			.map_err(|e| format!("Failed to read shared bytes: {e}"))?;
		Ok(buf)
	}

	fn plugin_info(&mut self) -> Result<PluginInfo, String> {
		let len = self
			.get_plugin_info
			.call(&mut self.store, ())
			.map_err(|e| format!("get_plugin_info trapped: {e}"))?;
		let bytes = self.read_shared(len)?;
		serde_json::from_slice(&bytes)
			.map_err(|e| format!("Invalid PluginInfo JSON: {e}"))
	}

	fn format_file(
		&mut self,
		content: &str,
		path: &Path,
	) -> Result<String, String> {
		let path_str = path.to_string_lossy();
		self.write_shared(path_str.as_bytes())?;
		self.set_file_path
			.call(&mut self.store, ())
			.map_err(|e| format!("set_file_path trapped: {e}"))?;

		self.write_shared(content.as_bytes())?;
		let status = self
			.format
			.call(&mut self.store, CONFIG_ID as i32)
			.map_err(|e| {
				let stderr =
					String::from_utf8_lossy(&self.store.data().stderr);
				if stderr.is_empty() {
					format!("dprint plugin format trapped: {e}")
				} else {
					format!("dprint plugin format trapped: {e} ({stderr})")
				}
			})?;

		match status {
			0 => Ok(content.to_string()),
			1 => {
				let len = self
					.get_formatted_text
					.call(&mut self.store, ())
					.map_err(|e| format!("get_formatted_text trapped: {e}"))?;
				let bytes = self.read_shared(len)?;
				String::from_utf8(bytes)
					.map_err(|e| format!("Plugin produced invalid UTF-8: {e}"))
			}
			2 => {
				let len = self
					.get_error_text
					.call(&mut self.store, ())
					.map_err(|e| format!("get_error_text trapped: {e}"))?;
				let bytes = self.read_shared(len)?;
				Err(String::from_utf8_lossy(&bytes).into_owned())
			}
			other => Err(format!("Unknown dprint format status code: {other}")),
		}
	}
}

struct PoolState {
	idle: Vec<DprintInstance>,
	in_use: usize,
}

/// Thread-safe runtime engine + bounded instance pool for a dprint Schema v4 plugin.
pub struct DprintWasmEngine {
	prefix: &'static str,
	source: DprintWasmSource,
	plugin_config: Option<serde_json::Map<String, serde_json::Value>>,
	engine: OnceLock<Result<(Engine, Module), String>>,
	pool_state: Mutex<PoolState>,
	available: Condvar,
	max_instances: usize,
}

impl DprintWasmEngine {
	pub const fn new(
		prefix: &'static str,
		source: DprintWasmSource,
	) -> Self {
		Self {
			prefix,
			source,
			plugin_config: None,
			engine: OnceLock::new(),
			pool_state: Mutex::new(PoolState {
				idle: Vec::new(),
				in_use: 0,
			}),
			available: Condvar::new(),
			max_instances: MAX_POOL_INSTANCES,
		}
	}

	/// Build a non-const engine with custom plugin-specific configuration keys.
	pub fn with_plugin_config(
		prefix: &'static str,
		source: DprintWasmSource,
		plugin_config: serde_json::Map<String, serde_json::Value>,
	) -> Self {
		let max_instances = std::thread::available_parallelism()
			.map(|n| n.get().clamp(1, MAX_POOL_INSTANCES))
			.unwrap_or(1);
		Self {
			prefix,
			source,
			plugin_config: Some(plugin_config),
			engine: OnceLock::new(),
			pool_state: Mutex::new(PoolState {
				idle: Vec::new(),
				in_use: 0,
			}),
			available: Condvar::new(),
			max_instances,
		}
	}

	fn compiled(&self) -> Result<&(Engine, Module), String> {
		self.engine
			.get_or_init(|| {
				let engine = Engine::new(&fama_wasm::default_engine_config())
					.map_err(|e| format!("Failed to create wasmtime engine: {e}"))?;
				let hash = self.source.cache_hash()?;
				let module = fama_wasm::load_or_compile(
					&engine,
					self.prefix,
					&hash,
					|| {
						self.source
							.load_bytes()
							.expect("dprint plugin source should load")
					},
				)?;
				Ok((engine, module))
			})
			.as_ref()
			.map_err(Clone::clone)
	}

	fn create_instance(&self) -> Result<DprintInstance, String> {
		let (engine, module) = self.compiled()?;
		let mut linker: Linker<DprintStoreState> = Linker::new(engine);
		define_dprint_imports(&mut linker)?;
		define_wasi(&mut linker, module)?;

		let mut store = Store::new(engine, DprintStoreState::new());
		let instance = linker
			.instantiate(&mut store, module)
			.map_err(|e| format!("Failed to instantiate dprint plugin: {e:#}"))?;

		// WASI reactor plugins export `_initialize`.
		if let Some(init) =
			instance.get_func(&mut store, "_initialize")
		{
			init.call(&mut store, &[], &mut [])
				.map_err(|e| format!("_initialize trapped: {e}"))?;
		}

		// Verify Schema v4 export.
		let version_fn: TypedFunc<(), i32> = instance
			.get_typed_func(&mut store, "dprint_plugin_version_4")
			.map_err(|e| {
				format!("Plugin is not dprint Schema v4 (missing dprint_plugin_version_4): {e}")
			})?;
		let version = version_fn
			.call(&mut store, ())
			.map_err(|e| format!("dprint_plugin_version_4 trapped: {e}"))?;
		if version != 4 {
			return Err(format!(
				"Unsupported dprint plugin schema version: {version}"
			));
		}

		let memory = instance
			.get_memory(&mut store, "memory")
			.ok_or_else(|| "Plugin missing `memory` export".to_string())?;

		let get_shared_bytes_ptr = typed(&instance, &mut store, "get_shared_bytes_ptr")?;
		let clear_shared_bytes = typed(&instance, &mut store, "clear_shared_bytes")?;
		let get_plugin_info = typed(&instance, &mut store, "get_plugin_info")?;
		let register_config = typed(&instance, &mut store, "register_config")?;
		let set_file_path = typed(&instance, &mut store, "set_file_path")?;
		let format = typed(&instance, &mut store, "format")?;
		let get_formatted_text = typed(&instance, &mut store, "get_formatted_text")?;
		let get_error_text = typed(&instance, &mut store, "get_error_text")?;

		let mut inst = DprintInstance {
			store,
			memory,
			get_shared_bytes_ptr,
			clear_shared_bytes,
			get_plugin_info,
			register_config,
			set_file_path,
			format,
			get_formatted_text,
			get_error_text,
		};

		let empty_map = serde_json::Map::new();
		let config_json = build_config_json(
			self.plugin_config.as_ref().unwrap_or(&empty_map),
		)?;
		inst.write_shared(config_json.as_bytes())?;
		inst.register_config
			.call(&mut inst.store, CONFIG_ID as i32)
			.map_err(|e| format!("register_config trapped: {e}"))?;

		Ok(inst)
	}

	fn acquire(&self) -> Result<DprintInstance, String> {
		let mut state = self
			.pool_state
			.lock()
			.unwrap_or_else(|p| p.into_inner());
		loop {
			if let Some(inst) = state.idle.pop() {
				state.in_use += 1;
				return Ok(inst);
			}
			if state.in_use < self.max_instances {
				state.in_use += 1;
				drop(state);
				match self.create_instance() {
					Ok(inst) => return Ok(inst),
					Err(e) => {
						let mut state = self
							.pool_state
							.lock()
							.unwrap_or_else(|p| p.into_inner());
						state.in_use -= 1;
						self.available.notify_one();
						return Err(e);
					}
				}
			}
			state = self
				.available
				.wait(state)
				.unwrap_or_else(|p| p.into_inner());
		}
	}

	fn release(&self, inst: Option<DprintInstance>) {
		let mut state = self
			.pool_state
			.lock()
			.unwrap_or_else(|p| p.into_inner());
		state.in_use -= 1;
		if let Some(inst) = inst {
			state.idle.push(inst);
		}
		self.available.notify_one();
	}

	fn with_instance<R>(
		&self,
		f: impl FnOnce(&mut DprintInstance) -> Result<R, String>,
	) -> Result<R, String> {
		let mut inst = self.acquire()?;
		let res = f(&mut inst);
		// Drop poisoned instances on error; return healthy ones to the pool.
		if res.is_ok() {
			self.release(Some(inst));
		} else {
			self.release(None);
		}
		res
	}

	/// Retrieve plugin metadata (`name`, `version`, `config_key`, etc.).
	pub fn plugin_info(&self) -> Result<PluginInfo, String> {
		self.with_instance(|inst| inst.plugin_info())
	}

	/// Format a single file using the hosted dprint Schema v4 plugin.
	pub fn format(&self, content: &str, path: &Path) -> Result<String, String> {
		self.with_instance(|inst| inst.format_file(content, path))
	}

	/// Precompile this plugin into a `.wasmu` artifact inside `output_dir`.
	pub fn precompile(&self, output_dir: &Path) -> Result<PathBuf, String> {
		let engine = Engine::new(&fama_wasm::default_engine_config())
			.map_err(|e| format!("Failed to create wasmtime engine: {e}"))?;
		let hash = self.source.cache_hash()?;
		let bytes = self.source.load_bytes()?;
		fama_wasm::precompile_to_dir(
			&engine,
			self.prefix,
			&hash,
			|| bytes,
			output_dir,
		)
	}
}

fn typed<P: wasmtime::WasmParams, R: wasmtime::WasmResults>(
	instance: &Instance,
	store: &mut Store<DprintStoreState>,
	name: &str,
) -> Result<TypedFunc<P, R>, String> {
	instance
		.get_typed_func::<P, R>(store, name)
		.map_err(|e| format!("Plugin missing `{name}` export: {e}"))
}

fn build_config_json(
	plugin: &serde_json::Map<String, serde_json::Value>,
) -> Result<String, String> {
	let payload = RawFormatConfigPayload {
		plugin,
		global: GlobalConfigPayload {
			line_width: CONFIG.line_width as u32,
			use_tabs: matches!(CONFIG.indent_style, IndentStyle::Tabs),
			indent_width: CONFIG.indent_width,
			new_line_kind: match CONFIG.line_ending {
				LineEnding::Lf => "lf",
				LineEnding::Crlf => "crlf",
			},
		},
	};
	serde_json::to_string(&payload)
		.map_err(|e| format!("Failed to serialize dprint config: {e}"))
}

fn define_dprint_imports(
	linker: &mut Linker<DprintStoreState>,
) -> Result<(), String> {
	linker
		.func_wrap("dprint", "host_has_cancelled", || -> i32 { 0 })
		.map_err(|e| format!("dprint host_has_cancelled: {e}"))?;

	linker
		.func_wrap(
			"dprint",
			"host_write_buffer",
			|mut caller: Caller<'_, DprintStoreState>, ptr: i32| {
				let bytes = std::mem::take(&mut caller.data_mut().pending_host_buf);
				write_guest(&mut caller, ptr, &bytes);
			},
		)
		.map_err(|e| format!("dprint host_write_buffer: {e}"))?;

	linker
		.func_wrap(
			"dprint",
			"host_format",
			|mut caller: Caller<'_, DprintStoreState>,
			 path_ptr: i32,
			 path_len: i32,
			 start: i32,
			 end: i32,
			 _override_ptr: i32,
			 _override_len: i32,
			 text_ptr: i32,
			 text_len: i32|
			 -> i32 {
				let path_bytes = read_guest(&mut caller, path_ptr, path_len);
				let text_bytes = read_guest(&mut caller, text_ptr, text_len);
				let Ok(path_str) = String::from_utf8(path_bytes) else {
					return 0;
				};
				let Ok(full_text) = String::from_utf8(text_bytes) else {
					return 0;
				};

				let start_idx = (start.max(0) as usize).min(full_text.len());
				let end_idx =
					(end.max(0) as usize).clamp(start_idx, full_text.len());
				let slice = &full_text[start_idx..end_idx];

				match fama_common::format_embedded(slice, Path::new(&path_str))
				{
					Some(Ok(formatted)) if formatted != slice => {
						let final_text =
							if start_idx == 0 && end_idx == full_text.len() {
								formatted
							} else {
								format!(
									"{}{}{}",
									&full_text[..start_idx],
									formatted,
									&full_text[end_idx..]
								)
							};
						caller.data_mut().pending_host_buf =
							final_text.into_bytes();
						1
					}
					Some(Ok(_)) | None => 0,
					Some(Err(err)) => {
						caller.data_mut().pending_host_buf = err.into_bytes();
						2
					}
				}
			},
		)
		.map_err(|e| format!("dprint host_format: {e}"))?;

	linker
		.func_wrap(
			"dprint",
			"host_get_formatted_text",
			|caller: Caller<'_, DprintStoreState>| -> i32 {
				caller.data().pending_host_buf.len() as i32
			},
		)
		.map_err(|e| format!("dprint host_get_formatted_text: {e}"))?;

	linker
		.func_wrap(
			"dprint",
			"host_get_error_text",
			|caller: Caller<'_, DprintStoreState>| -> i32 {
				caller.data().pending_host_buf.len() as i32
			},
		)
		.map_err(|e| format!("dprint host_get_error_text: {e}"))?;

	Ok(())
}
