//! Hermetic `wasi_snapshot_preview1` host for dprint Schema v4 WASM plugins.
//!
//! Filesystem-touching WASI calls return `ENOENT`/`ENOSYS` so plugins remain
//! deterministic and side-effect-free; unknown WASI imports are auto-stubbed
//! from the module's own import descriptors so Rust, TinyGo, Emscripten, and
//! .NET NativeAOT-LLVM WASI reactors all instantiate cleanly.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use wasmtime::{Caller, Linker, Module, Val, ValType};

const ERRNO_SUCCESS: i32 = 0;
const ERRNO_BADF: i32 = 8;
const ERRNO_NOENT: i32 = 44;
const ERRNO_NOSYS: i32 = 52;
const ERRNO_NOTSUP: i32 = 58;

const PREOPEN_PATH: &[u8] = b"/";

const DEFINED: &[&str] = &[
	"fd_write",
	"fd_read",
	"fd_close",
	"fd_seek",
	"fd_fdstat_get",
	"fd_filestat_get",
	"fd_prestat_get",
	"fd_prestat_dir_name",
	"path_open",
	"path_filestat_get",
	"path_unlink_file",
	"fd_readdir",
	"fd_pwrite",
	"fd_pread",
	"fd_tell",
	"fd_filestat_set_size",
	"clock_time_get",
	"clock_res_get",
	"random_get",
	"environ_get",
	"environ_sizes_get",
	"args_get",
	"args_sizes_get",
	"proc_exit",
	"sched_yield",
	"poll_oneoff",
];

/// Host state held inside every `wasmtime::Store` for a dprint plugin instance.
pub struct DprintStoreState {
	/// Bytes written to stderr (surfaced in trap diagnostics).
	pub stderr: Vec<u8>,
	/// Staging buffer written to guest linear memory by `dprint::host_write_buffer`.
	pub pending_host_buf: Vec<u8>,
	rand_state: u64,
}

impl DprintStoreState {
	pub fn new() -> Self {
		Self {
			stderr: Vec::new(),
			pending_host_buf: Vec::new(),
			rand_state: 0x243F_6A88_85A3_08D3,
		}
	}
}

impl Default for DprintStoreState {
	fn default() -> Self {
		Self::new()
	}
}

fn now_ns() -> u64 {
	SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.unwrap_or(Duration::ZERO)
		.as_nanos() as u64
}

pub fn read_guest(
	caller: &mut Caller<'_, DprintStoreState>,
	ptr: i32,
	len: i32,
) -> Vec<u8> {
	let Some(m) = caller.get_export("memory").and_then(|e| e.into_memory())
	else {
		return Vec::new();
	};
	let data = m.data(&*caller);
	let start = ptr as usize;
	let end = start + len.max(0) as usize;
	if len >= 0 && end <= data.len() {
		data[start..end].to_vec()
	} else {
		Vec::new()
	}
}

pub fn write_guest(
	caller: &mut Caller<'_, DprintStoreState>,
	ptr: i32,
	bytes: &[u8],
) -> bool {
	let Some(m) = caller.get_export("memory").and_then(|e| e.into_memory())
	else {
		return false;
	};
	let data = m.data_mut(&mut *caller);
	let start = ptr as usize;
	let end = start + bytes.len();
	if end <= data.len() {
		data[start..end].copy_from_slice(bytes);
		true
	} else {
		false
	}
}

fn parse_iovs(
	caller: &mut Caller<'_, DprintStoreState>,
	iovs: i32,
	iovs_len: i32,
) -> Vec<(usize, usize)> {
	let mut out = Vec::new();
	for i in 0..iovs_len.max(0) {
		let base = iovs as usize + (i as usize) * 8;
		let pair = read_guest(&mut *caller, base as i32, 8);
		if pair.len() < 8 {
			break;
		}
		let p = i32::from_le_bytes(pair[0..4].try_into().unwrap()) as usize;
		let l =
			i32::from_le_bytes(pair[4..8].try_into().unwrap()).max(0) as usize;
		out.push((p, l));
	}
	out
}

/// Register `wasi_snapshot_preview1` functions and auto-stub any extra WASI imports.
pub fn define_wasi(
	linker: &mut Linker<DprintStoreState>,
	module: &Module,
) -> Result<(), String> {
	linker
		.func_wrap(
			"wasi_snapshot_preview1",
			"fd_write",
			|mut caller: Caller<'_, DprintStoreState>,
			 fd: i32,
			 iovs: i32,
			 iovs_len: i32,
			 nwritten_ptr: i32|
			 -> i32 {
				if fd != 1 && fd != 2 {
					return ERRNO_BADF;
				}
				let pairs = parse_iovs(&mut caller, iovs, iovs_len);
				let mut total: u32 = 0;
				for (p, l) in pairs {
					let chunk = read_guest(&mut caller, p as i32, l as i32);
					total = total.saturating_add(chunk.len() as u32);
					if fd == 2 {
						let buf = &mut caller.data_mut().stderr;
						if buf.len() < 4096 {
							buf.extend_from_slice(&chunk);
						}
					}
				}
				write_guest(&mut caller, nwritten_ptr, &total.to_le_bytes());
				ERRNO_SUCCESS
			},
		)
		.map_err(|e| format!("wasi fd_write: {e}"))?;

	linker
		.func_wrap(
			"wasi_snapshot_preview1",
			"fd_read",
			|mut caller: Caller<'_, DprintStoreState>,
			 _fd: i32,
			 _iovs: i32,
			 _iovs_len: i32,
			 nread_ptr: i32|
			 -> i32 {
				write_guest(&mut caller, nread_ptr, &0u32.to_le_bytes());
				ERRNO_SUCCESS
			},
		)
		.map_err(|e| format!("wasi fd_read: {e}"))?;

	linker
		.func_wrap("wasi_snapshot_preview1", "fd_close", |_: i32| -> i32 {
			ERRNO_SUCCESS
		})
		.map_err(|e| format!("wasi fd_close: {e}"))?;

	linker
		.func_wrap(
			"wasi_snapshot_preview1",
			"fd_seek",
			|mut caller: Caller<'_, DprintStoreState>,
			 _fd: i32,
			 _off: i64,
			 _whence: i32,
			 newoff_ptr: i32|
			 -> i32 {
				write_guest(&mut caller, newoff_ptr, &0u64.to_le_bytes());
				ERRNO_NOSYS
			},
		)
		.map_err(|e| format!("wasi fd_seek: {e}"))?;

	linker
		.func_wrap(
			"wasi_snapshot_preview1",
			"fd_fdstat_get",
			|mut caller: Caller<'_, DprintStoreState>,
			 fd: i32,
			 buf_ptr: i32|
			 -> i32 {
				let filetype: u8 = if fd <= 2 { 2 } else { 3 };
				let mut buf = [0u8; 24];
				buf[0] = filetype;
				write_guest(&mut caller, buf_ptr, &buf);
				ERRNO_SUCCESS
			},
		)
		.map_err(|e| format!("wasi fd_fdstat_get: {e}"))?;

	linker
		.func_wrap(
			"wasi_snapshot_preview1",
			"fd_filestat_get",
			|_: i32, _: i32| -> i32 { ERRNO_BADF },
		)
		.map_err(|e| format!("wasi fd_filestat_get: {e}"))?;

	linker
		.func_wrap(
			"wasi_snapshot_preview1",
			"fd_prestat_get",
			|mut caller: Caller<'_, DprintStoreState>,
			 fd: i32,
			 buf_ptr: i32|
			 -> i32 {
				if fd == 3 {
					let mut buf = [0u8; 8];
					buf[4..8].copy_from_slice(
						&(PREOPEN_PATH.len() as u32).to_le_bytes(),
					);
					write_guest(&mut caller, buf_ptr, &buf);
					ERRNO_SUCCESS
				} else {
					ERRNO_BADF
				}
			},
		)
		.map_err(|e| format!("wasi fd_prestat_get: {e}"))?;

	linker
		.func_wrap(
			"wasi_snapshot_preview1",
			"fd_prestat_dir_name",
			|mut caller: Caller<'_, DprintStoreState>,
			 fd: i32,
			 path_ptr: i32,
			 path_len: i32|
			 -> i32 {
				if fd == 3 && (path_len as usize) >= PREOPEN_PATH.len() {
					write_guest(&mut caller, path_ptr, PREOPEN_PATH);
					ERRNO_SUCCESS
				} else {
					ERRNO_BADF
				}
			},
		)
		.map_err(|e| format!("wasi fd_prestat_dir_name: {e}"))?;

	linker
		.func_wrap(
			"wasi_snapshot_preview1",
			"path_open",
			|_: i32,
			 _: i32,
			 _: i32,
			 _: i32,
			 _: i32,
			 _: i64,
			 _: i64,
			 _: i32,
			 _: i32|
			 -> i32 { ERRNO_NOENT },
		)
		.map_err(|e| format!("wasi path_open: {e}"))?;

	linker
		.func_wrap(
			"wasi_snapshot_preview1",
			"path_filestat_get",
			|_: i32, _: i32, _: i32, _: i32, _: i32| -> i32 { ERRNO_NOENT },
		)
		.map_err(|e| format!("wasi path_filestat_get: {e}"))?;

	linker
		.func_wrap(
			"wasi_snapshot_preview1",
			"path_unlink_file",
			|_: i32, _: i32, _: i32| -> i32 { ERRNO_NOENT },
		)
		.map_err(|e| format!("wasi path_unlink_file: {e}"))?;

	linker
		.func_wrap(
			"wasi_snapshot_preview1",
			"fd_readdir",
			|_: i32, _: i32, _: i32, _: i64, _: i32| -> i32 { ERRNO_BADF },
		)
		.map_err(|e| format!("wasi fd_readdir: {e}"))?;

	linker
		.func_wrap(
			"wasi_snapshot_preview1",
			"fd_pwrite",
			|_: i32, _: i32, _: i32, _: i64, _: i32| -> i32 { ERRNO_BADF },
		)
		.map_err(|e| format!("wasi fd_pwrite: {e}"))?;

	linker
		.func_wrap(
			"wasi_snapshot_preview1",
			"fd_pread",
			|_: i32, _: i32, _: i32, _: i64, _: i32| -> i32 { ERRNO_BADF },
		)
		.map_err(|e| format!("wasi fd_pread: {e}"))?;

	linker
		.func_wrap(
			"wasi_snapshot_preview1",
			"fd_tell",
			|_: i32, _: i32| -> i32 { ERRNO_BADF },
		)
		.map_err(|e| format!("wasi fd_tell: {e}"))?;

	linker
		.func_wrap(
			"wasi_snapshot_preview1",
			"fd_filestat_set_size",
			|_: i32, _: i64| -> i32 { ERRNO_BADF },
		)
		.map_err(|e| format!("wasi fd_filestat_set_size: {e}"))?;

	linker
		.func_wrap(
			"wasi_snapshot_preview1",
			"clock_time_get",
			|mut caller: Caller<'_, DprintStoreState>,
			 _id: i32,
			 _prec: i64,
			 out_ptr: i32|
			 -> i32 {
				write_guest(&mut caller, out_ptr, &now_ns().to_le_bytes());
				ERRNO_SUCCESS
			},
		)
		.map_err(|e| format!("wasi clock_time_get: {e}"))?;

	linker
		.func_wrap(
			"wasi_snapshot_preview1",
			"clock_res_get",
			|mut caller: Caller<'_, DprintStoreState>,
			 _id: i32,
			 out_ptr: i32|
			 -> i32 {
				write_guest(&mut caller, out_ptr, &1000u64.to_le_bytes());
				ERRNO_SUCCESS
			},
		)
		.map_err(|e| format!("wasi clock_res_get: {e}"))?;

	linker
		.func_wrap(
			"wasi_snapshot_preview1",
			"random_get",
			|mut caller: Caller<'_, DprintStoreState>,
			 buf_ptr: i32,
			 len: i32|
			 -> i32 {
				let n = len.max(0) as usize;
				let mut bytes = Vec::with_capacity(n);
				let mut s = caller.data().rand_state;
				for _ in 0..n {
					s ^= s << 13;
					s ^= s >> 7;
					s ^= s << 17;
					bytes.push(s as u8);
				}
				caller.data_mut().rand_state = s;
				write_guest(&mut caller, buf_ptr, &bytes);
				ERRNO_SUCCESS
			},
		)
		.map_err(|e| format!("wasi random_get: {e}"))?;

	linker
		.func_wrap(
			"wasi_snapshot_preview1",
			"environ_get",
			|_: i32, _: i32| -> i32 { ERRNO_SUCCESS },
		)
		.map_err(|e| format!("wasi environ_get: {e}"))?;

	linker
		.func_wrap(
			"wasi_snapshot_preview1",
			"environ_sizes_get",
			|mut caller: Caller<'_, DprintStoreState>,
			 count_ptr: i32,
			 size_ptr: i32|
			 -> i32 {
				write_guest(&mut caller, count_ptr, &0u32.to_le_bytes());
				write_guest(&mut caller, size_ptr, &0u32.to_le_bytes());
				ERRNO_SUCCESS
			},
		)
		.map_err(|e| format!("wasi environ_sizes_get: {e}"))?;

	linker
		.func_wrap(
			"wasi_snapshot_preview1",
			"args_get",
			|_: i32, _: i32| -> i32 { ERRNO_SUCCESS },
		)
		.map_err(|e| format!("wasi args_get: {e}"))?;

	linker
		.func_wrap(
			"wasi_snapshot_preview1",
			"args_sizes_get",
			|mut caller: Caller<'_, DprintStoreState>,
			 argc_ptr: i32,
			 argv_buf_ptr: i32|
			 -> i32 {
				write_guest(&mut caller, argc_ptr, &0u32.to_le_bytes());
				write_guest(&mut caller, argv_buf_ptr, &0u32.to_le_bytes());
				ERRNO_SUCCESS
			},
		)
		.map_err(|e| format!("wasi args_sizes_get: {e}"))?;

	linker
		.func_wrap(
			"wasi_snapshot_preview1",
			"proc_exit",
			|caller: Caller<'_, DprintStoreState>,
			 code: i32|
			 -> wasmtime::Result<()> {
				let stderr =
					String::from_utf8_lossy(&caller.data().stderr).into_owned();
				if stderr.is_empty() {
					wasmtime::bail!("wasi proc_exit({code})");
				} else {
					wasmtime::bail!("wasi proc_exit({code}): {stderr}");
				}
			},
		)
		.map_err(|e| format!("wasi proc_exit: {e}"))?;

	linker
		.func_wrap("wasi_snapshot_preview1", "sched_yield", || -> i32 {
			ERRNO_SUCCESS
		})
		.map_err(|e| format!("wasi sched_yield: {e}"))?;

	linker
		.func_wrap(
			"wasi_snapshot_preview1",
			"poll_oneoff",
			|_: i32, _: i32, _: i32, _: i32| -> i32 { ERRNO_NOTSUP },
		)
		.map_err(|e| format!("wasi poll_oneoff: {e}"))?;

	// Auto-stub any remaining WASI imports declared by the module.
	for imp in module.imports() {
		if imp.module() != "wasi_snapshot_preview1" {
			continue;
		}
		if DEFINED.contains(&imp.name()) {
			continue;
		}
		let wasmtime::ExternType::Func(func_ty) = imp.ty() else {
			continue;
		};
		let results_ty: Vec<ValType> = func_ty.results().collect();
		linker
			.func_new(
				imp.module(),
				imp.name(),
				func_ty.clone(),
				move |_caller, _params, results| {
					for (slot, ty) in results.iter_mut().zip(&results_ty) {
						*slot = match ty {
							ValType::I32 => Val::I32(ERRNO_NOSYS),
							ValType::I64 => Val::I64(0),
							ValType::F32 => Val::F32(0),
							ValType::F64 => Val::F64(0),
							_ => Val::I32(0),
						};
					}
					Ok(())
				},
			)
			.map_err(|e| format!("wasi stub {}: {e}", imp.name()))?;
	}

	Ok(())
}
