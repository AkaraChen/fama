//! Hand-rolled `wasi_snapshot_preview1` host for the swift-format module.
//!
//! The module only uses WASI at startup (runtime init, preopen scan). The
//! format path itself is pure string-in/string-out: the shim passes `nil`
//! for `assumingFileURL`, so no file is ever opened. Filesystem entry
//! points therefore answer "not there" instead of touching the real disk,
//! keeping formatting hermetic and side-effect free. Anything the module
//! imports beyond this set gets an automatic errno stub derived from the
//! module's own import descriptor, so signatures can never drift.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use wasmtime::{Caller, Linker, Module, Val, ValType};

// WASI p1 errno table (NOT POSIX values; verified against wasi-libc
// wasip1.h. An off-by-one here makes wasi-libc _Exit() mid-startup.)
const ERRNO_SUCCESS: i32 = 0;
const ERRNO_BADF: i32 = 8;
const ERRNO_NOENT: i32 = 44;
const ERRNO_NOSYS: i32 = 52;
const ERRNO_NOTSUP: i32 = 58;

const PREOPEN_PATH: &[u8] = b"/";

/// Names defined below; everything else the module imports is auto-stubbed.
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

/// Per-store WASI state.
pub struct WasiState {
	/// Bytes the guest wrote to stderr (surfaced in trap diagnostics)
	pub stderr: Vec<u8>,
	/// Names of filesystem-flavored imports the guest invoked; pure
	/// formatting must keep this to at most the one startup probe
	pub fs_calls: Vec<&'static str>,
	rand_state: u64,
}

impl WasiState {
	pub fn new() -> Self {
		WasiState {
			stderr: Vec::new(),
			fs_calls: Vec::new(),
			rand_state: 0x243F_6A88_85A3_08D3,
		}
	}
}

fn now_ns() -> u64 {
	SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.unwrap_or(Duration::ZERO)
		.as_nanos() as u64
}

fn read_guest(caller: &mut Caller<'_, WasiState>, ptr: i32, len: i32) -> Vec<u8> {
	let Some(m) = caller.get_export("memory").and_then(|e| e.into_memory()) else {
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

fn write_guest(caller: &mut Caller<'_, WasiState>, ptr: i32, bytes: &[u8]) -> bool {
	let Some(m) = caller.get_export("memory").and_then(|e| e.into_memory()) else {
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

fn parse_iovs(caller: &mut Caller<'_, WasiState>, iovs: i32, iovs_len: i32) -> Vec<(usize, usize)> {
	let mut out = Vec::new();
	for i in 0..iovs_len.max(0) {
		let base = iovs as usize + (i as usize) * 8;
		let pair = read_guest(&mut *caller, base as i32, 8);
		if pair.len() < 8 {
			break;
		}
		let p = i32::from_le_bytes(pair[0..4].try_into().unwrap()) as usize;
		let l = i32::from_le_bytes(pair[4..8].try_into().unwrap()).max(0) as usize;
		out.push((p, l));
	}
	out
}

fn read_iovs_flat(caller: &mut Caller<'_, WasiState>, iovs: i32, iovs_len: i32) -> Vec<u8> {
	parse_iovs(caller, iovs, iovs_len)
		.into_iter()
		.flat_map(|(p, l)| read_guest(&mut *caller, p as i32, l as i32))
		.collect()
}

pub fn build_linker(engine: &wasmtime::Engine) -> Linker<WasiState> {
	let mut linker: Linker<WasiState> = Linker::new(engine);
	let wasi = "wasi_snapshot_preview1";

	linker
		.func_wrap(wasi, "fd_write", |mut cx: Caller<'_, WasiState>, fd: i32, iovs: i32, iovs_len: i32, nwritten: i32| -> i32 {
			match fd {
				1 | 2 => {
					let flat = read_iovs_flat(&mut cx, iovs, iovs_len);
					let total = flat.len();
					if fd == 2 {
						cx.data_mut().stderr.extend(flat);
					}
					write_guest(&mut cx, nwritten, &(total as u32).to_le_bytes());
					ERRNO_SUCCESS
				}
				_ => ERRNO_BADF,
			}
		})
		.unwrap();

	// No files can be opened (the format path is FS-free), so the bulk of
	// the table below is "consistent nothing" plumbing that keeps wasi-libc's
	// startup scan and Foundation's env probing happy.
	linker
		.func_wrap(wasi, "fd_read", |mut cx: Caller<'_, WasiState>, _fd: i32, _iovs: i32, _len: i32, nread: i32| -> i32 {
			write_guest(&mut cx, nread, &0u32.to_le_bytes());
			ERRNO_SUCCESS
		})
		.unwrap();

	linker
		.func_wrap(wasi, "fd_close", |_cx: Caller<'_, WasiState>, fd: i32| -> i32 {
			match fd {
				0..=3 => ERRNO_SUCCESS,
				_ => ERRNO_BADF,
			}
		})
		.unwrap();

	linker
		.func_wrap(wasi, "fd_seek", |_cx: Caller<'_, WasiState>, _fd: i32, _delta: i64, _whence: i32, _newoff: i32| -> i32 { ERRNO_BADF })
		.unwrap();

	linker
		.func_wrap(wasi, "fd_fdstat_get", |mut cx: Caller<'_, WasiState>, fd: i32, out: i32| -> i32 {
			let mut buf = [0u8; 24];
			match fd {
				0..=2 => buf[0] = 2, // character device
				3 => buf[0] = 3,     // directory (preopen)
				_ => return ERRNO_BADF,
			}
			buf[8..16].copy_from_slice(&u64::MAX.to_le_bytes());
			buf[16..24].copy_from_slice(&u64::MAX.to_le_bytes());
			write_guest(&mut cx, out, &buf);
			ERRNO_SUCCESS
		})
		.unwrap();

	linker
		.func_wrap(wasi, "fd_filestat_get", |mut cx: Caller<'_, WasiState>, fd: i32, out: i32| -> i32 {
			// __wasi_filestat_t: dev@0 ino@8 filetype@16 nlink@24 size@32 …
			let mut buf = [0u8; 64];
			match fd {
				0..=2 => buf[16] = 2,
				3 => buf[16] = 3,
				_ => return ERRNO_BADF,
			}
			buf[24..32].copy_from_slice(&1u64.to_le_bytes());
			write_guest(&mut cx, out, &buf);
			ERRNO_SUCCESS
		})
		.unwrap();

	linker
		.func_wrap(wasi, "fd_prestat_get", |mut cx: Caller<'_, WasiState>, fd: i32, out: i32| -> i32 {
			if fd != 3 {
				return ERRNO_BADF;
			}
			let mut buf = [0u8; 8];
			buf[4..8].copy_from_slice(&(PREOPEN_PATH.len() as u32).to_le_bytes());
			write_guest(&mut cx, out, &buf);
			ERRNO_SUCCESS
		})
		.unwrap();

	linker
		.func_wrap(wasi, "fd_prestat_dir_name", |mut cx: Caller<'_, WasiState>, fd: i32, path: i32, _len: i32| -> i32 {
			if fd != 3 || !write_guest(&mut cx, path, PREOPEN_PATH) {
				return ERRNO_BADF;
			}
			ERRNO_SUCCESS
		})
		.unwrap();

	// The format path never opens files; record the touch so tests can prove
	// formatting stays hermetic.
	linker
		.func_wrap(wasi, "path_open", |mut cx: Caller<'_, WasiState>, _dirfd: i32, _flags: i32, _path: i32, _len: i32, _oflags: i32, _r: i64, _ri: i64, _f: i32, _out: i32| -> i32 {
			cx.data_mut().fs_calls.push("path_open");
			ERRNO_NOENT
		})
		.unwrap();

	linker
		.func_wrap(wasi, "path_filestat_get", |mut cx: Caller<'_, WasiState>, _dirfd: i32, _flags: i32, _path: i32, _len: i32, _out: i32| -> i32 {
			cx.data_mut().fs_calls.push("path_filestat_get");
			ERRNO_NOENT
		})
		.unwrap();

	linker
		.func_wrap(wasi, "path_unlink_file", |mut cx: Caller<'_, WasiState>, _dirfd: i32, _path: i32, _len: i32| -> i32 {
			cx.data_mut().fs_calls.push("path_unlink_file");
			ERRNO_NOENT
		})
		.unwrap();

	linker
		.func_wrap(wasi, "fd_readdir", |mut cx: Caller<'_, WasiState>, _fd: i32, _buf: i32, _buflen: i32, _cookie: i64, bufused: i32| -> i32 {
			write_guest(&mut cx, bufused, &0u32.to_le_bytes());
			ERRNO_SUCCESS
		})
		.unwrap();

	linker
		.func_wrap(wasi, "fd_pwrite", |_cx: Caller<'_, WasiState>, _fd: i32, _iovs: i32, _l: i32, _off: i64, _n: i32| -> i32 { ERRNO_BADF })
		.unwrap();

	linker
		.func_wrap(wasi, "fd_pread", |_cx: Caller<'_, WasiState>, _fd: i32, _iovs: i32, _l: i32, _off: i64, _n: i32| -> i32 { ERRNO_BADF })
		.unwrap();

	linker
		.func_wrap(wasi, "fd_tell", |mut cx: Caller<'_, WasiState>, _fd: i32, out: i32| -> i32 {
			write_guest(&mut cx, out, &0u64.to_le_bytes());
			ERRNO_SUCCESS
		})
		.unwrap();

	linker
		.func_wrap(wasi, "fd_filestat_set_size", |_cx: Caller<'_, WasiState>, _fd: i32, _size: i64| -> i32 { ERRNO_SUCCESS })
		.unwrap();

	linker
		.func_wrap(wasi, "clock_time_get", |mut cx: Caller<'_, WasiState>, _id: i32, _prec: i64, out: i32| -> i32 {
			write_guest(&mut cx, out, &now_ns().to_le_bytes());
			ERRNO_SUCCESS
		})
		.unwrap();

	linker
		.func_wrap(wasi, "clock_res_get", |mut cx: Caller<'_, WasiState>, _id: i32, out: i32| -> i32 {
			write_guest(&mut cx, out, &1u64.to_le_bytes());
			ERRNO_SUCCESS
		})
		.unwrap();

	linker
		.func_wrap(wasi, "random_get", |mut cx: Caller<'_, WasiState>, buf: i32, len: i32| -> i32 {
			let mut bytes = vec![0u8; len.max(0) as usize];
			let st = &mut cx.data_mut().rand_state;
			for b in bytes.iter_mut() {
				*st ^= *st << 13;
				*st ^= *st >> 7;
				*st ^= *st << 17;
				*b = (*st >> 24) as u8;
			}
			if !write_guest(&mut cx, buf, &bytes) {
				return ERRNO_NOENT;
			}
			ERRNO_SUCCESS
		})
		.unwrap();

	linker
		.func_wrap(wasi, "environ_get", |_cx: Caller<'_, WasiState>, _argv: i32, _buf: i32| -> i32 { ERRNO_SUCCESS })
		.unwrap();

	linker
		.func_wrap(wasi, "environ_sizes_get", |mut cx: Caller<'_, WasiState>, count: i32, size: i32| -> i32 {
			write_guest(&mut cx, count, &0u32.to_le_bytes());
			write_guest(&mut cx, size, &0u32.to_le_bytes());
			ERRNO_SUCCESS
		})
		.unwrap();

	linker
		.func_wrap(wasi, "args_get", |_cx: Caller<'_, WasiState>, _argv: i32, _buf: i32| -> i32 { ERRNO_SUCCESS })
		.unwrap();

	// One argument ("/") totaling 8 bytes including NUL padding
	linker
		.func_wrap(wasi, "args_sizes_get", |mut cx: Caller<'_, WasiState>, count: i32, size: i32| -> i32 {
			write_guest(&mut cx, count, &1u32.to_le_bytes());
			write_guest(&mut cx, size, &8u32.to_le_bytes());
			ERRNO_SUCCESS
		})
		.unwrap();

	// proc_exit never returns; model "exit" as a host error trap
	linker
		.func_wrap(wasi, "proc_exit", |mut cx: Caller<'_, WasiState>, code: i32| -> Result<(), wasmtime::Error> {
			cx.data_mut().stderr.extend(format!("[fama-swift] proc_exit({code})\n").into_bytes());
			Err(wasmtime::Error::msg(format!("proc_exit({code})")))
		})
		.unwrap();

	linker
		.func_wrap(wasi, "poll_oneoff", |mut cx: Caller<'_, WasiState>, _a: i32, _b: i32, _c: i32, out: i32| -> i32 {
			write_guest(&mut cx, out, &0u32.to_le_bytes());
			ERRNO_SUCCESS
		})
		.unwrap();

	linker
		.func_wrap(wasi, "sched_yield", |_cx: Caller<'_, WasiState>| -> i32 { ERRNO_SUCCESS })
		.unwrap();

	linker
}

/// Define errno stubs for imports outside the list above. The stub's
/// FuncType comes from the module's own descriptor, so it always matches.
pub fn stub_remaining_imports(linker: &mut Linker<WasiState>, module: &Module) {
	for imp in module.imports() {
		if DEFINED.contains(&imp.name()) {
			continue;
		}
		if let Some(ty) = imp.ty().func() {
			let results: Vec<ValType> = ty.results().collect();
			let code: i32 = match imp.name() {
				// fire-and-forget: pretend success
				"path_filestat_set_times"
				| "fd_sync"
				| "fd_datasync"
				| "fd_advise"
				| "fd_fdstat_set_flags"
				| "fd_filestat_set_times"
				| "fd_renumber" => ERRNO_SUCCESS,
				// mutations we never want honored (a formatter creates nothing)
				"path_create_directory"
				| "path_rename"
				| "path_link"
				| "path_symlink"
				| "path_readlink"
				| "path_remove_directory" => ERRNO_NOTSUP,
				_ => ERRNO_NOSYS,
			};
			linker
				.func_new(
					imp.module(),
					imp.name(),
					ty.clone(),
					move |_cx: Caller<'_, WasiState>, _args: &[Val], out: &mut [Val]| -> Result<(), wasmtime::Error> {
						for (i, r) in results.iter().enumerate() {
							out[i] = match r {
								ValType::I32 => Val::I32(code),
								ValType::I64 => Val::I64(0),
								ValType::F32 => Val::F32(0),
								ValType::F64 => Val::F64(0),
								_ => Val::I32(code),
							};
						}
						Ok(())
					},
				)
				.unwrap();
		}
	}
}
